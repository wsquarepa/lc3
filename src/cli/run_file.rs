//! Parsing and validation of run files (TOML) into a [`RunSpec`] without any I/O.

use super::literal::{self, ADDRESS_RANGE, LiteralError, WORD_RANGE};
use lc3::{Case, MemoryPoke, Pokes, Register, Trigger};
use serde::Deserialize;
use serde::de::{self, Deserializer, MapAccess, SeqAccess, Visitor};
use std::fmt;
use std::marker::PhantomData;
use std::num::NonZeroU32;
use std::path::PathBuf;

const DEFAULT_MEM_LATENCY: NonZeroU32 = NonZeroU32::new(4).unwrap();

/// Where the console bytes of a run come from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConsoleInput {
    Bytes(Vec<u8>),
    File(PathBuf),
    /// Raw bytes of the process stdin, read to end of file before the run.
    Stdin,
    Empty,
}

/// An inclusive range of addresses whose final words are reported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DumpRange {
    pub start: u16,
    pub end: u16,
}

/// A validated run: every path is exactly as the user wrote it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunSpec {
    /// `None` selects the bundled OS.
    pub os: Option<PathBuf>,
    /// Non-empty, in loading order.
    pub images: Vec<PathBuf>,
    pub mem_latency: NonZeroU32,
    pub cycle_cap: u64,
    pub input: ConsoleInput,
    pub pokes: Pokes,
    pub triggers: Vec<Trigger>,
    pub dumps: Vec<DumpRange>,
}

impl RunSpec {
    /// The run of a bare source: bundled OS, default latency and cap, console
    /// input from stdin, no pokes, triggers or dumps.
    pub fn bare_source(path: PathBuf) -> Self {
        Self {
            os: None,
            images: vec![path],
            mem_latency: DEFAULT_MEM_LATENCY,
            cycle_cap: Case::default().cycle_cap,
            input: ConsoleInput::Stdin,
            pokes: Pokes::default(),
            triggers: Vec::new(),
            dumps: Vec::new(),
        }
    }
}

/// A run-file problem; the message names the run file and the offending key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunFileError(pub String);

impl fmt::Display for RunFileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Parses `text`, which `name` identifies in error messages. `unspecified_input`
/// is the console input when the file gives neither `input` nor `input_file`.
/// Errors cover TOML syntax, unknown or missing keys, wrong types, malformed or
/// out-of-range values, an empty `images`, and `input` together with `input_file`.
pub fn parse(
    text: &str,
    name: &str,
    unspecified_input: ConsoleInput,
) -> Result<RunSpec, RunFileError> {
    let raw: RawRunFile = toml::from_str(text).map_err(|error| fail(name, error))?;
    convert(raw, unspecified_input).map_err(|message| fail(name, message))
}

fn fail(name: &str, message: impl fmt::Display) -> RunFileError {
    RunFileError(format!("{name}: {message}"))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRunFile {
    os: Option<PathBuf>,
    images: Vec<PathBuf>,
    mem_latency: Option<i64>,
    cycle_cap: Option<i64>,
    input: Option<String>,
    input_file: Option<PathBuf>,
    registers: Option<Pairs<Scalar>>,
    memory: Option<Pairs<Words>>,
    dump: Option<Vec<String>>,
    #[serde(default)]
    at: Vec<RawTrigger>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTrigger {
    pc: Scalar,
    registers: Option<Pairs<Scalar>>,
    memory: Option<Pairs<Words>>,
}

/// A TOML integer or a literal string, kept raw until the key is known.
#[derive(Debug)]
enum Scalar {
    Integer(i64),
    Text(String),
}

/// One word or an array of words.
#[derive(Debug)]
struct Words(Vec<Scalar>);

/// A TOML table's entries as a list of key and value pairs, so that the keys
/// can be validated one by one. TOML tables are unordered and duplicate keys
/// are rejected, so no order is promised; overlapping pokes are rejected later.
struct Pairs<T>(Vec<(String, T)>);

struct ScalarVisitor;

impl<'de> Visitor<'de> for ScalarVisitor {
    type Value = Scalar;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("an integer or an LC-3 literal string")
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Scalar, E> {
        Ok(Scalar::Integer(value))
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Scalar, E> {
        i64::try_from(value)
            .map(Scalar::Integer)
            .map_err(|_| E::custom(format!("integer {value} is out of range")))
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Scalar, E> {
        Ok(Scalar::Text(value.to_owned()))
    }
}

impl<'de> Deserialize<'de> for Scalar {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(ScalarVisitor)
    }
}

struct WordsVisitor;

impl<'de> Visitor<'de> for WordsVisitor {
    type Value = Words;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a word or an array of words (integers or LC-3 literal strings)")
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Words, E> {
        ScalarVisitor
            .visit_i64(value)
            .map(|scalar| Words(vec![scalar]))
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Words, E> {
        ScalarVisitor
            .visit_u64(value)
            .map(|scalar| Words(vec![scalar]))
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Words, E> {
        ScalarVisitor
            .visit_str(value)
            .map(|scalar| Words(vec![scalar]))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Words, A::Error> {
        let mut words = Vec::new();
        while let Some(scalar) = seq.next_element()? {
            words.push(scalar);
        }
        Ok(Words(words))
    }
}

impl<'de> Deserialize<'de> for Words {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(WordsVisitor)
    }
}

struct PairsVisitor<T>(PhantomData<T>);

impl<'de, T: Deserialize<'de>> Visitor<'de> for PairsVisitor<T> {
    type Value = Pairs<T>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a table")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Pairs<T>, A::Error> {
        let mut pairs = Vec::new();
        while let Some(pair) = map.next_entry()? {
            pairs.push(pair);
        }
        Ok(Pairs(pairs))
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Pairs<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_map(PairsVisitor(PhantomData))
    }
}

#[derive(Clone, Copy)]
enum Width {
    Word,
    Address,
}

impl Width {
    fn narrow(self, value: i64) -> Result<u16, LiteralError> {
        match self {
            Width::Word => literal::word(value),
            Width::Address => literal::address(value),
        }
    }

    fn range(self) -> &'static str {
        match self {
            Width::Word => WORD_RANGE,
            Width::Address => ADDRESS_RANGE,
        }
    }
}

/// Resolves a literal to a `u16`; `shown` is the value as the user wrote it.
fn resolve(width: Width, parsed: Result<i64, LiteralError>, shown: &str) -> Result<u16, String> {
    parsed
        .and_then(|value| width.narrow(value))
        .map_err(|error| match error {
            LiteralError::Malformed => format!(
                "value {shown} is not an LC-3 literal (use x3000, #-5, 12 or a TOML integer)"
            ),
            LiteralError::OutOfRange => {
                format!("value {shown} is out of range ({})", width.range())
            }
        })
}

impl Scalar {
    fn resolve(&self, width: Width) -> Result<u16, String> {
        match self {
            Scalar::Integer(value) => resolve(width, Ok(*value), &value.to_string()),
            Scalar::Text(text) => {
                resolve(width, literal::parse_literal(text), &format!("{text:?}"))
            }
        }
    }
}

fn convert(raw: RawRunFile, unspecified_input: ConsoleInput) -> Result<RunSpec, String> {
    if raw.images.is_empty() {
        return Err("images must be a non-empty array of source paths".to_owned());
    }
    let input = match (raw.input, raw.input_file) {
        (Some(_), Some(_)) => {
            return Err("input and input_file are both given; use only one".to_owned());
        }
        (Some(text), None) => ConsoleInput::Bytes(text.into_bytes()),
        (None, Some(path)) => ConsoleInput::File(path),
        (None, None) => unspecified_input,
    };
    let pokes = convert_pokes(raw.registers, raw.memory, "")?;
    let triggers = raw
        .at
        .into_iter()
        .enumerate()
        .map(|(index, trigger)| convert_trigger(index, trigger))
        .collect::<Result<_, _>>()?;
    let dumps = raw
        .dump
        .unwrap_or_default()
        .iter()
        .map(|range| parse_dump(range))
        .collect::<Result<_, _>>()?;
    Ok(RunSpec {
        os: raw.os,
        images: raw.images,
        mem_latency: raw
            .mem_latency
            .map_or(Ok(DEFAULT_MEM_LATENCY), convert_latency)?,
        cycle_cap: raw
            .cycle_cap
            .map_or_else(|| Ok(Case::default().cycle_cap), convert_cycle_cap)?,
        input,
        pokes,
        triggers,
        dumps,
    })
}

fn convert_latency(value: i64) -> Result<NonZeroU32, String> {
    u32::try_from(value)
        .ok()
        .and_then(NonZeroU32::new)
        .ok_or_else(|| {
            format!(
                "mem_latency must be an integer from 1 to {} (got {value})",
                u32::MAX
            )
        })
}

fn convert_cycle_cap(value: i64) -> Result<u64, String> {
    u64::try_from(value)
        .ok()
        .filter(|cap| *cap >= 1)
        .ok_or_else(|| format!("cycle_cap must be an integer >= 1 (got {value})"))
}

fn convert_trigger(index: usize, trigger: RawTrigger) -> Result<Trigger, String> {
    let place = format!("[[at]] entry {}", index + 1);
    let pc = trigger
        .pc
        .resolve(Width::Address)
        .map_err(|problem| format!("{place}, key pc: {problem}"))?;
    let pokes = convert_pokes(trigger.registers, trigger.memory, &place)?;
    Ok(Trigger { pc, pokes })
}

/// `place` prefixes the table names in messages: empty for the top level.
fn convert_pokes(
    registers: Option<Pairs<Scalar>>,
    memory: Option<Pairs<Words>>,
    place: &str,
) -> Result<Pokes, String> {
    let prefix = if place.is_empty() {
        String::new()
    } else {
        format!("{place}, ")
    };
    let registers = convert_registers(registers.map_or_else(Vec::new, |pairs| pairs.0))
        .map_err(|problem| format!("{prefix}[registers]: {problem}"))?;
    let memory = convert_memory_table(memory.map_or_else(Vec::new, |pairs| pairs.0))
        .map_err(|problem| format!("{prefix}[memory]: {problem}"))?;
    Ok(Pokes { registers, memory })
}

/// Converts one `[memory]` table. TOML does not define key order, so entries
/// whose word ranges overlap (counting wrap from xFFFF to x0000) are rejected
/// rather than resolved by order.
fn convert_memory_table(pairs: Vec<(String, Words)>) -> Result<Vec<MemoryPoke>, String> {
    let keyed = pairs
        .into_iter()
        .map(|(key, words)| convert_memory(&key, words).map(|poke| (key, poke)))
        .collect::<Result<Vec<_>, _>>()?;
    for (index, (first_key, first)) in keyed.iter().enumerate() {
        for (second_key, second) in &keyed[index + 1..] {
            if let Some(address) = first_overlap(first, second) {
                return Err(format!(
                    "keys `{first_key}` and `{second_key}` both write x{address:04X}; split them into non-overlapping entries"
                ));
            }
        }
    }
    Ok(keyed.into_iter().map(|(_, poke)| poke).collect())
}

/// The first address written by both pokes, if any. Two circular ranges
/// overlap exactly when one's start lies inside the other.
fn first_overlap(first: &MemoryPoke, second: &MemoryPoke) -> Option<u16> {
    let inside = |start: u16, poke: &MemoryPoke| {
        usize::from(start.wrapping_sub(poke.address)) < poke.words.len()
    };
    if first.words.is_empty() || second.words.is_empty() {
        None
    } else if inside(second.address, first) {
        Some(second.address)
    } else if inside(first.address, second) {
        Some(first.address)
    } else {
        None
    }
}

fn convert_registers(pairs: Vec<(String, Scalar)>) -> Result<Vec<(Register, u16)>, String> {
    let mut given: Vec<(String, String)> = Vec::new();
    let mut registers = Vec::new();
    for (key, scalar) in pairs {
        let canonical = key.to_ascii_uppercase();
        let register = register_named(&canonical)
            .ok_or_else(|| format!("unknown register key `{key}`; expected R0-R7, PC or PSR"))?;
        if let Some((_, first)) = given.iter().find(|(name, _)| *name == canonical) {
            return Err(format!(
                "register `{key}` is already given as `{first}`; keys are case-insensitive"
            ));
        }
        let word = scalar
            .resolve(Width::Word)
            .map_err(|problem| format!("key `{key}`: {problem}"))?;
        given.push((canonical, key));
        registers.push((register, word));
    }
    Ok(registers)
}

fn register_named(canonical: &str) -> Option<Register> {
    Some(match canonical {
        "R0" => Register::R0,
        "R1" => Register::R1,
        "R2" => Register::R2,
        "R3" => Register::R3,
        "R4" => Register::R4,
        "R5" => Register::R5,
        "R6" => Register::R6,
        "R7" => Register::R7,
        "PC" => Register::Pc,
        "PSR" => Register::Psr,
        _ => return None,
    })
}

fn convert_memory(key: &str, words: Words) -> Result<MemoryPoke, String> {
    let address = resolve(
        Width::Address,
        literal::parse_literal(key),
        &format!("{key:?}"),
    )
    .map_err(|problem| format!("address key `{key}`: {problem}"))?;
    let words = words
        .0
        .iter()
        .map(|scalar| scalar.resolve(Width::Word))
        .collect::<Result<_, _>>()
        .map_err(|problem| format!("key `{key}`: {problem}"))?;
    Ok(MemoryPoke { address, words })
}

fn parse_dump(text: &str) -> Result<DumpRange, String> {
    let malformed = || {
        format!(
            "dump entry {text:?} must be `start..end` with both ends address literals, start <= end"
        )
    };
    let (start, end) = text.split_once("..").ok_or_else(malformed)?;
    let end_address = |end: &str| {
        resolve(
            Width::Address,
            literal::parse_literal(end),
            &format!("{end:?}"),
        )
    };
    let start = end_address(start).map_err(|problem| format!("dump entry {text:?}: {problem}"))?;
    let end = end_address(end).map_err(|problem| format!("dump entry {text:?}: {problem}"))?;
    if start > end {
        return Err(malformed());
    }
    Ok(DumpRange { start, end })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_ok(text: &str) -> RunSpec {
        parse(text, "run.toml", ConsoleInput::Stdin).unwrap()
    }

    fn parse_err(text: &str) -> String {
        parse(text, "run.toml", ConsoleInput::Stdin).unwrap_err().0
    }

    #[test]
    fn minimal_file_gets_defaults() {
        let spec = parse_ok("images = [\"a.asm\"]\n");
        assert_eq!(spec, {
            let mut bare = RunSpec::bare_source(PathBuf::from("a.asm"));
            bare.input = ConsoleInput::Stdin;
            bare
        });
        assert_eq!(spec.mem_latency.get(), 4);
        assert_eq!(spec.cycle_cap, 10_000_000);
    }

    #[test]
    fn unspecified_input_is_the_callers_choice() {
        let spec = parse("images = [\"a.asm\"]\n", "-", ConsoleInput::Empty).unwrap();
        assert_eq!(spec.input, ConsoleInput::Empty);
        let spec = parse(
            "images = [\"a.asm\"]\ninput = \"hi\"\n",
            "-",
            ConsoleInput::Empty,
        )
        .unwrap();
        assert_eq!(spec.input, ConsoleInput::Bytes(b"hi".to_vec()));
    }

    #[test]
    fn pokes_parse_in_every_literal_form_and_negative_words_wrap() {
        let spec = parse_ok(
            "images = [\"a.asm\"]\n[registers]\nr1 = 1\nPSR = \"x8002\"\nR0 = -1\n[memory]\nx4000 = [1, \"#-2\"]\n\"#10\" = \"xFFFF\"\n",
        );
        let mut registers = spec.pokes.registers;
        registers.sort_by_key(|(_, word)| *word);
        assert_eq!(
            registers,
            vec![
                (Register::R1, 1),
                (Register::Psr, 0x8002),
                (Register::R0, 0xFFFF)
            ]
        );
        let mut memory = spec.pokes.memory;
        memory.sort_by_key(|poke| poke.address);
        assert_eq!(
            memory,
            vec![
                MemoryPoke {
                    address: 10,
                    words: vec![0xFFFF]
                },
                MemoryPoke {
                    address: 0x4000,
                    words: vec![1, 0xFFFE]
                },
            ]
        );
    }

    #[test]
    fn triggers_and_dumps_parse() {
        let spec = parse_ok(
            "images = [\"a.asm\"]\ndump = [\"x3000..x3002\", \"#5..#5\"]\n[[at]]\npc = \"x3001\"\n[at.registers]\nR2 = 9\n[[at]]\npc = 3\n",
        );
        assert_eq!(
            spec.dumps,
            vec![
                DumpRange {
                    start: 0x3000,
                    end: 0x3002
                },
                DumpRange { start: 5, end: 5 }
            ]
        );
        assert_eq!(spec.triggers.len(), 2);
        assert_eq!(spec.triggers[0].pc, 0x3001);
        assert_eq!(spec.triggers[0].pokes.registers, vec![(Register::R2, 9)]);
        assert_eq!(spec.triggers[1].pc, 3);
        assert_eq!(spec.triggers[1].pokes, Pokes::default());
    }

    #[test]
    fn unknown_keys_are_named() {
        assert!(parse_err("images = [\"a\"]\nzzz = 1\n").contains("zzz"));
        assert!(parse_err("images = [\"a\"]\n[registers]\nR9 = 1\n").contains("R9"));
        assert!(parse_err("images = [\"a\"]\n[[at]]\npc = 1\nzzz = 1\n").contains("zzz"));
        assert!(
            parse_err("images = [\"a\"]\n[[at]]\npc = 1\n[at.registers]\nQ = 1\n").contains("`Q`")
        );
    }

    #[test]
    fn registers_given_twice_in_different_case_are_rejected() {
        let message = parse_err("images = [\"a\"]\n[registers]\nR1 = 1\nr1 = 2\n");
        assert!(
            message.contains("r1") && message.contains("R1"),
            "{message}"
        );
    }

    #[test]
    fn value_errors_name_key_and_value() {
        let message = parse_err("images = [\"a\"]\n[registers]\nR0 = \"xZZ\"\n");
        assert!(
            message.contains("R0") && message.contains("\"xZZ\""),
            "{message}"
        );
        let message = parse_err("images = [\"a\"]\n[[at]]\npc = 65536\n");
        assert!(
            message.contains("pc") && message.contains("65536"),
            "{message}"
        );
        let message = parse_err("images = [\"a\"]\n[memory]\nx3000 = [0, 65536]\n");
        assert!(
            message.contains("x3000") && message.contains("65536"),
            "{message}"
        );
    }

    fn overlap_message(memory: &str) -> String {
        parse_err(&format!("images = [\"a\"]\n[memory]\n{memory}"))
    }

    #[test]
    fn overlapping_memory_entries_are_rejected_naming_keys_and_address() {
        let message = overlap_message("x4000 = [1, 2, 3]\nx4001 = 7\n");
        assert!(
            message.contains("x4000") && message.contains("x4001"),
            "{message}"
        );
        let message = overlap_message("x4000 = 1\n\"#16384\" = 2\n");
        assert!(
            message.contains("#16384") && message.contains("x4000"),
            "{message}"
        );
        let message = overlap_message("xFFFF = [1, 2]\nx0000 = 9\n");
        assert!(
            message.contains("x0000") && message.contains("xFFFF"),
            "{message}"
        );
        let message = overlap_message("x0000 = [1, 2]\nxFFFF = [5, 6, 7]\n");
        assert!(message.contains("both write x0000"), "{message}");
    }

    #[test]
    fn adjacent_empty_and_trigger_memory_entries_do_not_overlap() {
        let spec = parse_ok(
            "images = [\"a\"]\n[memory]\nx4000 = [1, 2]\nx4002 = 3\nxFFFF = []\nx4001 = []\n[[at]]\npc = 1\n[at.memory]\nx4000 = 9\n",
        );
        assert_eq!(spec.pokes.memory.len(), 4);
        assert_eq!(spec.triggers[0].pokes.memory.len(), 1);
    }

    #[test]
    fn overlap_inside_a_trigger_table_is_rejected() {
        let message =
            parse_err("images = [\"a\"]\n[[at]]\npc = 1\n[at.memory]\nx10 = [1, 2]\nx11 = 3\n");
        assert!(
            message.contains("[[at]] entry 1") && message.contains("x0011"),
            "{message}"
        );
    }

    #[test]
    fn scalar_settings_are_validated() {
        assert!(parse_err("images = [\"a\"]\nmem_latency = 0\n").contains("mem_latency"));
        assert!(parse_err("images = [\"a\"]\ncycle_cap = 0\n").contains("cycle_cap"));
        assert!(parse_err("images = []\n").contains("images"));
        assert!(
            parse_err("images = [\"a\"]\ninput = \"a\"\ninput_file = \"b\"\n")
                .contains("input_file")
        );
    }

    #[test]
    fn dump_ranges_are_validated() {
        for bad in [
            "x3000",
            "x3001..x3000",
            "x3000..x10000",
            "..x3000",
            "x3000..",
        ] {
            let message = parse_err(&format!("images = [\"a\"]\ndump = [\"{bad}\"]\n"));
            assert!(message.contains(bad), "{message}");
        }
    }
}
