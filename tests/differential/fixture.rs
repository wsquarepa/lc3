use lc3::{Case, Failure, Halted, Image, MemoryPoke, Outcome, Pokes, Trigger, prepare, run};
use std::fmt;
use std::num::NonZeroU32;
use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};

const DEVICE_REGISTERS: RangeInclusive<usize> = 0xfe00..=0xfe08;
const MEMORY_WORDS: usize = 0x10000;
const MEM_LATENCY: NonZeroU32 = NonZeroU32::MIN;
/// Large enough for the longest fixture, the longest local fixture.
const CYCLE_CAP: u64 = 1_000_000_000;

/// Where a fixture's OS comes from.
#[derive(Debug, PartialEq, Eq)]
pub enum OsSource {
    /// `lc3os.obj` from the local reference folder.
    Reference,
    /// A file named relative to the fixture's directory.
    File(PathBuf),
}

/// The state `lc3sim` ended in.
#[derive(Debug, PartialEq, Eq)]
pub enum Expected {
    Halted(HaltedState),
    IllegalInstruction { pc: u16, word: u16 },
    InputExhausted { pc: u16 },
}

/// Final registers (without PSR), trimmed program output and memory as differences
/// from the loaded OS image overlaid by the program image.
#[derive(Debug, PartialEq, Eq)]
pub struct HaltedState {
    pub registers: [u16; 8],
    pub pc: u16,
    pub program_output: Vec<u8>,
    pub memory_changes: Vec<(u16, Vec<u16>)>,
}

/// One program run captured from `lc3sim`.
#[derive(Debug, PartialEq, Eq)]
pub struct Fixture {
    pub os: OsSource,
    pub image: PathBuf,
    pub input: Vec<u8>,
    pub pokes: Vec<(u16, u16)>,
    pub trigger: Option<(u16, Vec<(u16, u16)>)>,
    pub expected: Expected,
}

/// A malformed fixture file, naming the file, the line and what was wrong.
#[derive(Debug)]
pub struct FixtureError {
    pub path: PathBuf,
    pub line: usize,
    pub message: String,
}

impl fmt::Display for FixtureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}: {}", self.path.display(), self.line, self.message)
    }
}

impl std::error::Error for FixtureError {}

fn hex_word(token: &str) -> Result<u16, String> {
    u16::from_str_radix(token, 16).map_err(|error| format!("bad hex word {token:?}: {error}"))
}

fn hex_bytes(token: &str) -> Result<Vec<u8>, String> {
    if !token.len().is_multiple_of(2) {
        return Err(format!("hex bytes {token:?} have an odd length"));
    }
    (0..token.len())
        .step_by(2)
        .map(|at| {
            u8::from_str_radix(&token[at..at + 2], 16)
                .map_err(|error| format!("bad hex bytes {token:?}: {error}"))
        })
        .collect()
}

fn hex_words(tokens: &[&str]) -> Result<Vec<u16>, String> {
    tokens.iter().map(|token| hex_word(token)).collect()
}

fn address_word_pair(token: &str) -> Result<(u16, u16), String> {
    let (address, word) = token
        .split_once('=')
        .ok_or_else(|| format!("expected <address>=<word>, found {token:?}"))?;
    Ok((hex_word(address)?, hex_word(word)?))
}

#[derive(Default)]
struct Draft {
    os: Option<OsSource>,
    image: Option<PathBuf>,
    input: Vec<u8>,
    pokes: Vec<(u16, u16)>,
    trigger: Option<(u16, Vec<(u16, u16)>)>,
    halted: bool,
    failure: Option<Expected>,
    registers: Option<([u16; 8], u16)>,
    output: Option<Vec<u8>>,
    memory_changes: Vec<(u16, Vec<u16>)>,
}

fn apply_line(draft: &mut Draft, line: &str) -> Result<(), String> {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    match tokens.as_slice() {
        ["os", "reference"] => draft.os = Some(OsSource::Reference),
        ["os", file] => draft.os = Some(OsSource::File(PathBuf::from(file))),
        ["image", file] => draft.image = Some(PathBuf::from(file)),
        ["input", hex] => draft.input = hex_bytes(hex)?,
        ["poke", address, word] => draft.pokes.push((hex_word(address)?, hex_word(word)?)),
        ["trigger", pc, pokes @ ..] => {
            let pokes = pokes
                .iter()
                .map(|token| address_word_pair(token))
                .collect::<Result<_, _>>()?;
            draft.trigger = Some((hex_word(pc)?, pokes));
        }
        ["halted"] => draft.halted = true,
        ["illegal", pc, word] => {
            draft.failure = Some(Expected::IllegalInstruction {
                pc: hex_word(pc)?,
                word: hex_word(word)?,
            })
        }
        ["input-exhausted", pc] => {
            draft.failure = Some(Expected::InputExhausted { pc: hex_word(pc)? })
        }
        ["registers", values @ ..] => {
            let words = hex_words(values)?;
            let [r @ .., pc] = <[u16; 9]>::try_from(words)
                .map_err(|words| format!("registers needs 9 words, found {}", words.len()))?;
            draft.registers = Some((r, pc));
        }
        ["output"] => draft.output = Some(Vec::new()),
        ["output", hex] => draft.output = Some(hex_bytes(hex)?),
        ["memory", address, words @ ..] => draft
            .memory_changes
            .push((hex_word(address)?, hex_words(words)?)),
        _ => return Err(format!("unrecognized line {line:?}")),
    }
    Ok(())
}

fn finish(draft: Draft) -> Result<Fixture, String> {
    let os = draft.os.ok_or("missing os line")?;
    let image = draft.image.ok_or("missing image line")?;
    let expected = match (draft.halted, draft.failure) {
        (true, None) => {
            let (registers, pc) = draft.registers.ok_or("missing registers line")?;
            Expected::Halted(HaltedState {
                registers,
                pc,
                program_output: draft.output.ok_or("missing output line")?,
                memory_changes: draft.memory_changes,
            })
        }
        (false, Some(failure)) => failure,
        _ => return Err("expected exactly one of halted, illegal, input-exhausted".to_string()),
    };
    Ok(Fixture {
        os,
        image,
        input: draft.input,
        pokes: draft.pokes,
        trigger: draft.trigger,
        expected,
    })
}

/// Parses a `.case` file (format in `scripts/capture-differential-fixtures`).
/// `path` only labels errors. Blank lines and `#` comments are ignored; a malformed
/// or incomplete file returns a `FixtureError` naming the line.
pub fn parse(path: &Path, text: &str) -> Result<Fixture, FixtureError> {
    let mut draft = Draft::default();
    for (index, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        apply_line(&mut draft, line).map_err(|message| FixtureError {
            path: path.to_path_buf(),
            line: index + 1,
            message,
        })?;
    }
    finish(draft).map_err(|message| FixtureError {
        path: path.to_path_buf(),
        line: text.lines().count(),
        message,
    })
}

fn load_words(memory: &mut [u16], image: &Image) {
    for (offset, word) in image.words().iter().enumerate() {
        memory[(usize::from(image.origin()) + offset) % MEMORY_WORDS] = *word;
    }
}

fn expected_memory(state: &HaltedState, os: &Image, image: &Image) -> Vec<u16> {
    let mut memory = vec![0; MEMORY_WORDS];
    load_words(&mut memory, os);
    load_words(&mut memory, image);
    for (address, words) in &state.memory_changes {
        for (offset, word) in words.iter().enumerate() {
            memory[(usize::from(*address) + offset) % MEMORY_WORDS] = *word;
        }
    }
    memory
}

fn memory_mismatches(expected: &[u16], actual: &[u16]) -> Vec<String> {
    const SHOWN: usize = 8;
    let differing: Vec<usize> = (0..MEMORY_WORDS)
        .filter(|address| {
            !DEVICE_REGISTERS.contains(address) && expected[*address] != actual[*address]
        })
        .collect();
    let mut lines: Vec<String> = differing
        .iter()
        .take(SHOWN)
        .map(|address| {
            format!(
                "memory x{address:04X}: lc3sim {:04X}, ours {:04X}",
                expected[*address], actual[*address]
            )
        })
        .collect();
    if differing.len() > SHOWN {
        lines.push(format!(
            "{} more memory words differ",
            differing.len() - SHOWN
        ));
    }
    lines
}

fn halted_mismatches(
    state: &HaltedState,
    halted: &Halted,
    os: &Image,
    image: &Image,
) -> Vec<String> {
    let mut lines = Vec::new();
    if halted.registers.r != state.registers {
        lines.push(format!(
            "registers: lc3sim {:04X?}, ours {:04X?}",
            state.registers, halted.registers.r
        ));
    }
    if halted.registers.pc != state.pc {
        lines.push(format!(
            "pc: lc3sim x{:04X}, ours x{:04X}",
            state.pc, halted.registers.pc
        ));
    }
    let output = halted.program_output.trim_ascii();
    if output != state.program_output {
        lines.push(format!(
            "program output: lc3sim {:?}, ours {:?}",
            state.program_output.escape_ascii().to_string(),
            output.escape_ascii().to_string()
        ));
    }
    lines.extend(memory_mismatches(
        &expected_memory(state, os, image),
        &halted.memory,
    ));
    lines
}

fn case_of(fixture: &Fixture) -> Case {
    let memory = |pokes: &[(u16, u16)]| Pokes {
        registers: Vec::new(),
        memory: pokes
            .iter()
            .map(|(address, word)| MemoryPoke {
                address: *address,
                words: vec![*word],
            })
            .collect(),
    };
    Case {
        input: fixture.input.clone(),
        cycle_cap: CYCLE_CAP,
        pokes: memory(&fixture.pokes),
        triggers: fixture
            .trigger
            .iter()
            .map(|(pc, pokes)| Trigger {
                pc: *pc,
                pokes: memory(pokes),
            })
            .collect(),
    }
}

/// Runs the fixture's program over `os` with this project and returns each way the
/// result differs from `lc3sim`'s, empty when it matches. Compares R0-R7, PC, memory
/// outside the device registers and trimmed program output for halted fixtures, and
/// the failure reason and PC otherwise; PSR is excluded because the condition codes
/// sit in different bits.
pub fn mismatches(fixture: &Fixture, os: &Image, image: &Image) -> Vec<String> {
    let prepared = match prepare(os, std::slice::from_ref(image), MEM_LATENCY) {
        Ok(prepared) => prepared,
        Err(error) => return vec![format!("prepare failed: {error:?}")],
    };
    match (run(&prepared, &case_of(fixture)), &fixture.expected) {
        (Outcome::Halted(halted), Expected::Halted(state)) => {
            halted_mismatches(state, &halted, os, image)
        }
        (
            Outcome::Failed(Failure::IllegalInstruction { pc, word, .. }),
            Expected::IllegalInstruction {
                pc: expected_pc,
                word: expected_word,
            },
        ) if pc == *expected_pc && word == *expected_word => Vec::new(),
        (
            Outcome::Failed(Failure::InputExhausted { pc }),
            Expected::InputExhausted { pc: expected_pc },
        ) if pc == *expected_pc => Vec::new(),
        (outcome, expected) => vec![format!(
            "lc3sim ended {}, ours {}",
            describe_expected(expected),
            describe_outcome(&outcome)
        )],
    }
}

fn describe_expected(expected: &Expected) -> String {
    match expected {
        Expected::Halted(_) => "halted".to_string(),
        failure => format!("{failure:?}"),
    }
}

fn describe_outcome(outcome: &Outcome) -> String {
    match outcome {
        Outcome::Halted(_) => "halted".to_string(),
        Outcome::Failed(failure) => format!("{failure:?}"),
    }
}
