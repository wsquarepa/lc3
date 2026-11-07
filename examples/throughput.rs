//! Run with `taskset -c 0 cargo run --release --example throughput`.

use lc3::{Case, Format, Halted, Image, Outcome, Pokes, Prepared, Trigger, load, prepare, run};
use std::fs;
use std::hint::black_box;
use std::io;
use std::num::NonZeroU32;
use std::path::Path;
use std::process::{Command, ExitCode};
use std::time::{Duration, Instant};

const CASES: usize = 3_000;
const WARMUP: usize = 100;
const CHARACTERS: usize = 8;
const ROUNDS: u16 = 63;
const SUFFIX: &[u8] = b" done\n";
const HALT_OUTPUT: &[u8] = b"\n\n--- halting the LC-3 ---\n\n";
const EXPECTED_CYCLES: u64 = 23_767;
// The reference OS executes GETC in 4 instructions, OUT in 6, PUTS in
// 10 + 11 per byte, and HALT in 6 plus its nested PUTS (ready devices).
const INSTRUCTIONS: usize = 5
    + CHARACTERS * (8 + 7 * ROUNDS as usize + 4 + 6)
    + 3
    + 10
    + 11 * SUFFIX.len()
    + 6
    + 10
    + 11 * HALT_OUTPUT.len();
const PROGRAM: &str = r#"
.ORIG x3000
AND R3,R3,#0
ADD R3,R3,#15
AND R4,R4,#0
ADD R4,R4,#8
LEA R5,SLOT
CHAR GETC
AND R1,R1,#0
LD R2,ROUNDS
WORK ADD R1,R1,R0
STR R1,R5,#0
LDR R1,R5,#0
ADD R1,R1,#1
AND R1,R1,R3
ADD R2,R2,#-1
BRp WORK
LD R0,ASCII
ADD R0,R0,R1
OUT
ADD R4,R4,#-1
BRp CHAR
LEA R0,SUFFIX
PUTS
HALT
ROUNDS .FILL #63
ASCII .FILL #65
SLOT .BLKW #1
SUFFIX .STRINGZ " done\n"
.END
"#;

fn image(format: Format, bytes: &[u8]) -> io::Result<Image> {
    load(format, bytes)
        .map(|loaded| loaded.image)
        .map_err(|diagnostics| io::Error::other(format!("benchmark image: {diagnostics:?}")))
}

fn case(index: usize, triggered: bool) -> Case {
    let mut digits = index;
    let input = (0..CHARACTERS)
        .map(|_| {
            let byte = b'A' + (digits % 26) as u8;
            digits /= 26;
            byte
        })
        .collect();
    Case {
        input,
        triggers: if triggered {
            vec![Trigger {
                pc: 0x3000,
                pokes: Pokes::default(),
            }]
        } else {
            Vec::new()
        },
        ..Case::default()
    }
}

fn expected_output(input: &[u8]) -> Vec<u8> {
    input
        .iter()
        .map(|byte| b'A' + ((u16::from(*byte) + 1) * ROUNDS % 16) as u8)
        .chain(SUFFIX.iter().copied())
        .collect()
}

fn checked_outcome(outcome: Outcome, case: &Case) -> io::Result<Halted> {
    match outcome {
        Outcome::Halted(halted)
            if halted.program_output == expected_output(&case.input)
                && halted.halt_output == HALT_OUTPUT
                && halted.cycles == EXPECTED_CYCLES =>
        {
            Ok(halted)
        }
        Outcome::Halted(halted) => Err(io::Error::other(format!(
            "input {:?}: expected output {:?}, halt {:?}, cycles {EXPECTED_CYCLES}; got {:?}, {:?}, {}",
            case.input,
            expected_output(&case.input),
            HALT_OUTPUT,
            halted.program_output,
            halted.halt_output,
            halted.cycles
        ))),
        Outcome::Failed(reason) => Err(io::Error::other(format!(
            "input {:?}: run failed: {reason:?}",
            case.input
        ))),
    }
}

fn conditions() -> io::Result<()> {
    let cpuinfo = fs::read_to_string("/proc/cpuinfo")?;
    let model = cpuinfo
        .lines()
        .find_map(|line| line.strip_prefix("model name\t: "))
        .ok_or_else(|| io::Error::other("/proc/cpuinfo has no CPU model"))?;
    let status = fs::read_to_string("/proc/self/status")?;
    let affinity = status
        .lines()
        .find_map(|line| line.strip_prefix("Cpus_allowed_list:\t"))
        .ok_or_else(|| io::Error::other("/proc/self/status has no CPU affinity"))?;
    let rustc = Command::new("rustc").arg("--version").output()?;
    if !rustc.status.success() {
        return Err(io::Error::other(format!(
            "rustc --version: status {}; stderr {}",
            rustc.status,
            String::from_utf8_lossy(&rustc.stderr)
        )));
    }
    println!("CPU: {model}; allowed cores: {affinity}");
    println!("rustc: {}", String::from_utf8_lossy(&rustc.stdout).trim());
    println!(
        "profile: {}; latency=1; cases/mode={CASES}; warmup/mode={WARMUP}",
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        }
    );
    println!("one Prepared; 8 GETC; 63 arithmetic/memory iterations/byte; OUT, PUTS, TRAP x25");
    println!(
        "timing: run including snapshot/result allocations; validation and result drop excluded"
    );
    Ok(())
}

fn measure(prepared: &Prepared) -> io::Result<()> {
    let cases: Vec<_> = (0..CASES)
        .map(|index| [case(index, false), case(index, true)])
        .collect();
    for pair in cases.iter().take(WARMUP) {
        for case in pair {
            checked_outcome(run(prepared, case), case)?;
        }
    }
    let mut samples = [Vec::with_capacity(CASES), Vec::with_capacity(CASES)];
    let mut cycles = [0u64; 2];
    for (index, pair) in cases.iter().enumerate() {
        for offset in 0..2 {
            let mode = (index + offset) % 2;
            let case = &pair[mode];
            let start = Instant::now();
            let outcome = black_box(run(black_box(prepared), black_box(case)));
            let elapsed = start.elapsed();
            let halted = checked_outcome(outcome, case)?;
            cycles[mode] += halted.cycles;
            samples[mode].push(elapsed);
        }
    }
    for (mode, mut times) in samples.into_iter().enumerate() {
        times.sort_unstable();
        let micros = |duration: Duration| duration.as_secs_f64() * 1e6;
        println!(
            "{}: instructions/case mean={:.3}; cycles/case mean={:.3}; median={:.3} us; p90={:.3} us; p99={:.3} us; max={:.3} us",
            ["no triggers", "one origin trigger"][mode],
            INSTRUCTIONS as f64,
            cycles[mode] as f64 / CASES as f64,
            micros(times[CASES / 2 - 1] + (times[CASES / 2] - times[CASES / 2 - 1]) / 2),
            micros(times[(CASES * 90).div_ceil(100) - 1]),
            micros(times[(CASES * 99).div_ceil(100) - 1]),
            micros(times[CASES - 1]),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn workload_matches_known_output_with_and_without_a_trigger() -> io::Result<()> {
        let inputs: HashSet<_> = (0..CASES).map(|index| case(index, false).input).collect();
        assert_eq!(inputs.len(), CASES);
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("reference/lc3tools/lc3os.obj");
        let os = image(Format::Obj, &fs::read(path)?)?;
        let program = image(Format::Asm, PROGRAM.as_bytes())?;
        let prepared = prepare(&os, &[program], NonZeroU32::MIN)
            .map_err(|error| io::Error::other(format!("test preparation: {error:?}")))?;
        for triggered in [false, true] {
            for (input, expected) in [
                (b"AAAAAAAA", b"OOOOOOOO done\n"),
                (b"ABCDEFGH", b"ONMLKJIH done\n"),
            ] {
                let case = Case {
                    input: input.to_vec(),
                    ..case(0, triggered)
                };
                let halted = checked_outcome(run(&prepared, &case), &case)?;
                assert_eq!(halted.program_output, expected);
            }
        }
        Ok(())
    }
}

fn main() -> io::Result<ExitCode> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("reference/lc3tools/lc3os.obj");
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            eprintln!("reference OS not found at {}", path.display());
            return Ok(ExitCode::from(2));
        }
        Err(error) => {
            return Err(io::Error::new(
                error.kind(),
                format!("cannot read reference OS at {}: {error}", path.display()),
            ));
        }
    };
    let os = image(Format::Obj, &bytes)?;
    let program = image(Format::Asm, PROGRAM.as_bytes())?;
    let prepared = prepare(&os, &[program], NonZeroU32::MIN)
        .map_err(|error| io::Error::other(format!("benchmark preparation: {error:?}")))?;
    conditions()?;
    measure(&prepared)?;
    Ok(ExitCode::SUCCESS)
}
