//! Pure rendering of a finished run as console bytes, a text summary or JSON.

use super::exit::Finish;
use super::run_file::DumpRange;
use super::sources::OsIdentity;
use lc3::{Attribution, Failure, Halted, IllegalKind, Outcome, WordCounts};
use serde::Serialize;

/// Everything about a run that output needs besides the [`Finish`] itself.
pub struct Report<'a> {
    pub finish: &'a Finish,
    /// Image paths as given, in loading order.
    pub image_paths: &'a [String],
    pub word_counts: &'a WordCounts,
    pub mem_latency: u32,
    pub os: &'a OsIdentity,
    pub dumps: &'a [DumpRange],
}

/// Program output followed by halt output; empty for a failed run.
pub fn console_bytes(finish: &Finish) -> Vec<u8> {
    halted_of(finish).map_or_else(Vec::new, |halted| {
        [
            halted.program_output.as_slice(),
            halted.halt_output.as_slice(),
        ]
        .concat()
    })
}

fn halted_of(finish: &Finish) -> Option<&Halted> {
    match finish {
        Finish::Ran(Outcome::Halted(halted)) => Some(halted),
        _ => None,
    }
}

/// Words of `range` from the halted memory image; the range was validated to
/// lie within the 65,536-word address space.
fn dump_words(halted: &Halted, range: DumpRange) -> Vec<u16> {
    halted.memory[usize::from(range.start)..=usize::from(range.end)].to_vec()
}

fn push_line(text: &mut String, line: String) {
    text.push_str(&line);
    text.push('\n');
}

fn describe_failure(failure: &Failure) -> String {
    match failure {
        Failure::CycleCapReached => "cycle cap reached".to_owned(),
        Failure::IllegalInstruction { pc, word, kind } => {
            let kind = match kind {
                IllegalKind::Rti => "RTI is unsupported",
                IllegalKind::Reserved => "reserved opcode",
                IllegalKind::MalformedEncoding => "malformed encoding",
            };
            format!("illegal instruction x{word:04X} at x{pc:04X} ({kind})")
        }
        Failure::InputExhausted { pc } => format!("input exhausted at x{pc:04X}"),
        Failure::UnknownTrap { vector } => format!("unknown trap x{vector:02X}"),
    }
}

fn describe_os(os: &OsIdentity) -> String {
    match os {
        OsIdentity::Bundled => "bundled".to_owned(),
        OsIdentity::File { path, sha256 } => format!("{path} (sha256 {sha256})"),
    }
}

/// The stderr summary of a run, ending in a newline.
pub fn text_summary(report: &Report) -> String {
    let mut text = String::new();
    match report.finish {
        Finish::Ran(Outcome::Halted(halted)) => text.push_str(&halted_head(report, halted)),
        Finish::Ran(Outcome::Failed(failure)) => {
            push_line(&mut text, format!("failed: {}", describe_failure(failure)));
        }
        Finish::BootFailed(failure) => {
            push_line(
                &mut text,
                format!("failed: boot: {}", describe_failure(failure)),
            );
        }
    }
    text.push_str("word counts:\n");
    for (path, count) in report.image_paths.iter().zip(&report.word_counts.images) {
        push_line(&mut text, format!("  {path}: {count}"));
    }
    push_line(&mut text, format!("  OS: {}", report.word_counts.os));
    push_line(&mut text, format!("memory latency: {}", report.mem_latency));
    push_line(&mut text, format!("os: {}", describe_os(report.os)));
    if let Some(halted) = halted_of(report.finish) {
        text.push_str(&halted_tail(report, halted));
    }
    text
}

fn halted_head(report: &Report, halted: &Halted) -> String {
    let Attribution {
        images,
        os,
        unattributed,
    } = &halted.attribution;
    let mut text = format!("halted\ncycles: {}\n", halted.cycles);
    for (path, cycles) in report.image_paths.iter().zip(images) {
        push_line(&mut text, format!("  {path}: {cycles}"));
    }
    push_line(&mut text, format!("  OS: {os}"));
    push_line(&mut text, format!("  unattributed: {unattributed}"));
    text
}

fn halted_tail(report: &Report, halted: &Halted) -> String {
    let mut text = String::new();
    for (index, value) in halted.registers.r.iter().enumerate() {
        push_line(&mut text, format!("R{index}=x{value:04X}"));
    }
    push_line(&mut text, format!("PC=x{:04X}", halted.registers.pc));
    push_line(&mut text, format!("PSR=x{:04X}", halted.registers.psr));
    for range in report.dumps {
        push_line(
            &mut text,
            format!("dump x{:04X}..x{:04X}:", range.start, range.end),
        );
        for (address, word) in (range.start..=range.end).zip(dump_words(halted, *range)) {
            push_line(&mut text, format!("  x{address:04X}: x{word:04X}"));
        }
    }
    text
}

#[derive(Serialize)]
struct RegistersJson {
    #[serde(rename = "R0")]
    r0: u16,
    #[serde(rename = "R1")]
    r1: u16,
    #[serde(rename = "R2")]
    r2: u16,
    #[serde(rename = "R3")]
    r3: u16,
    #[serde(rename = "R4")]
    r4: u16,
    #[serde(rename = "R5")]
    r5: u16,
    #[serde(rename = "R6")]
    r6: u16,
    #[serde(rename = "R7")]
    r7: u16,
    #[serde(rename = "PC")]
    pc: u16,
    #[serde(rename = "PSR")]
    psr: u16,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum KindJson {
    Rti,
    Reserved,
    MalformedEncoding,
}

#[derive(Serialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
enum FailureJson {
    CycleCapReached,
    IllegalInstruction { pc: u16, word: u16, kind: KindJson },
    InputExhausted { pc: u16 },
    UnknownTrap { vector: u8 },
    Boot { failure: Box<FailureJson> },
}

#[derive(Serialize)]
struct DumpJson {
    start: u16,
    end: u16,
    words: Vec<u16>,
}

#[derive(Serialize)]
struct CyclesJson<'a> {
    total: u64,
    images: &'a [u64],
    os: u64,
    unattributed: u64,
}

#[derive(Serialize)]
struct WordCountsJson<'a> {
    images: &'a [usize],
    os: usize,
}

#[derive(Serialize)]
struct ReportJson<'a> {
    outcome: &'static str,
    failure: Option<FailureJson>,
    registers: Option<RegistersJson>,
    dumps: Vec<DumpJson>,
    program_output: String,
    halt_output: String,
    cycles: Option<CyclesJson<'a>>,
    word_counts: WordCountsJson<'a>,
    mem_latency: u32,
    os: &'a OsIdentity,
}

fn failure_json(failure: &Failure) -> FailureJson {
    match failure {
        Failure::CycleCapReached => FailureJson::CycleCapReached,
        Failure::IllegalInstruction { pc, word, kind } => FailureJson::IllegalInstruction {
            pc: *pc,
            word: *word,
            kind: match kind {
                IllegalKind::Rti => KindJson::Rti,
                IllegalKind::Reserved => KindJson::Reserved,
                IllegalKind::MalformedEncoding => KindJson::MalformedEncoding,
            },
        },
        Failure::InputExhausted { pc } => FailureJson::InputExhausted { pc: *pc },
        Failure::UnknownTrap { vector } => FailureJson::UnknownTrap { vector: *vector },
    }
}

/// Maps each console byte to the Unicode scalar of the same value, losslessly.
fn console_string(bytes: &[u8]) -> String {
    bytes.iter().copied().map(char::from).collect()
}

fn registers_json(halted: &Halted) -> RegistersJson {
    let [r0, r1, r2, r3, r4, r5, r6, r7] = halted.registers.r;
    RegistersJson {
        r0,
        r1,
        r2,
        r3,
        r4,
        r5,
        r6,
        r7,
        pc: halted.registers.pc,
        psr: halted.registers.psr,
    }
}

/// One compact JSON object followed by a newline.
pub fn json(report: &Report) -> Result<String, serde_json::Error> {
    let halted = halted_of(report.finish);
    let failure = match report.finish {
        Finish::Ran(Outcome::Failed(failure)) => Some(failure_json(failure)),
        Finish::BootFailed(failure) => Some(FailureJson::Boot {
            failure: Box::new(failure_json(failure)),
        }),
        Finish::Ran(Outcome::Halted(_)) => None,
    };
    let document = ReportJson {
        outcome: if halted.is_some() { "halted" } else { "failed" },
        failure,
        registers: halted.map(registers_json),
        dumps: halted.map_or_else(Vec::new, |halted| {
            report
                .dumps
                .iter()
                .map(|range| DumpJson {
                    start: range.start,
                    end: range.end,
                    words: dump_words(halted, *range),
                })
                .collect()
        }),
        program_output: halted
            .map_or_else(String::new, |halted| console_string(&halted.program_output)),
        halt_output: halted.map_or_else(String::new, |halted| console_string(&halted.halt_output)),
        cycles: halted.map(|halted| CyclesJson {
            total: halted.cycles,
            images: &halted.attribution.images,
            os: halted.attribution.os,
            unattributed: halted.attribution.unattributed,
        }),
        word_counts: WordCountsJson {
            images: &report.word_counts.images,
            os: report.word_counts.os,
        },
        mem_latency: report.mem_latency,
        os: report.os,
    };
    serde_json::to_string(&document).map(|text| text + "\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn failed_report(finish: &Finish) -> String {
        let counts = WordCounts {
            images: vec![3],
            os: 9,
        };
        let paths = ["p.asm".to_owned()];
        text_summary(&Report {
            finish,
            image_paths: &paths,
            word_counts: &counts,
            mem_latency: 4,
            os: &OsIdentity::Bundled,
            dumps: &[],
        })
    }

    #[test]
    fn console_strings_are_lossless_latin1() {
        assert_eq!(
            console_string(&[0x00, 0x41, 0xE9, 0xFF]),
            "\u{0}A\u{e9}\u{ff}"
        );
    }

    #[test]
    fn failure_text_uses_lc3_hex() {
        let finish = Finish::Ran(Outcome::Failed(Failure::IllegalInstruction {
            pc: 0x3002,
            word: 0xD000,
            kind: IllegalKind::Reserved,
        }));
        assert!(
            failed_report(&finish)
                .starts_with("failed: illegal instruction xD000 at x3002 (reserved opcode)\n")
        );
        let boot = Finish::BootFailed(Failure::UnknownTrap { vector: 0x7 });
        assert!(failed_report(&boot).starts_with("failed: boot: unknown trap x07\n"));
    }

    #[test]
    fn failed_json_has_null_state_and_a_nested_boot_failure() {
        let counts = WordCounts {
            images: vec![3],
            os: 9,
        };
        let finish = Finish::BootFailed(Failure::CycleCapReached);
        let text = json(&Report {
            finish: &finish,
            image_paths: &[],
            word_counts: &counts,
            mem_latency: 4,
            os: &OsIdentity::Bundled,
            dumps: &[],
        })
        .unwrap();
        assert_eq!(
            text,
            "{\"outcome\":\"failed\",\"failure\":{\"reason\":\"boot\",\"failure\":{\"reason\":\"cycle_cap_reached\"}},\"registers\":null,\"dumps\":[],\"program_output\":\"\",\"halt_output\":\"\",\"cycles\":null,\"word_counts\":{\"images\":[3],\"os\":9},\"mem_latency\":4,\"os\":{\"kind\":\"bundled\"}}\n"
        );
    }
}
