//! Run with `cargo run --release --example trigger_overhead`.
//! Measures complete runs of a fixed nested loop, including snapshot cloning.

use lc3::{Case, Format, Image, Outcome, Pokes, Prepared, Trigger, load, prepare, run};
use std::hint::black_box;
use std::io;
use std::num::NonZeroU32;
use std::time::{Duration, Instant};

const INNER_ITERATIONS: u16 = 32_767;
const OUTER_ITERATIONS: u16 = 64;
const INSTRUCTIONS: u64 = 2 + OUTER_ITERATIONS as u64 * (2 * INNER_ITERATIONS as u64 + 3);
const EXPECTED_CYCLES: u64 = 15 + OUTER_ITERATIONS as u64 * (11 * INNER_ITERATIONS as u64 + 17);
const SAMPLES: usize = 9;
const RUNS_PER_SAMPLE: u32 = 4;

/// Loads in-memory words as an object image; reports loader diagnostics on failure.
fn image(origin: u16, words: &[u16]) -> io::Result<Image> {
    let bytes: Vec<u8> = std::iter::once(origin)
        .chain(words.iter().copied())
        .flat_map(u16::to_be_bytes)
        .collect();
    load(Format::Obj, &bytes)
        .map(|loaded| loaded.image)
        .map_err(|diagnostics| io::Error::other(format!("image at {origin:#06x}: {diagnostics:?}")))
}

/// Runs the fixed loop; reports a failed outcome or unexpected cycle count.
fn checked_run(prepared: &Prepared, case: &Case) -> io::Result<()> {
    match black_box(run(black_box(prepared), black_box(case))) {
        Outcome::Halted(halted) if halted.cycles == EXPECTED_CYCLES => Ok(()),
        Outcome::Halted(halted) => Err(io::Error::other(format!(
            "expected {EXPECTED_CYCLES} cycles, got {}",
            halted.cycles
        ))),
        Outcome::Failed(reason) => Err(io::Error::other(format!("loop failed: {reason:?}"))),
    }
}

/// Times repeated complete runs; propagates loop failures and cycle-count errors.
fn measure(prepared: &Prepared, case: &Case) -> io::Result<Duration> {
    let start = Instant::now();
    for _ in 0..RUNS_PER_SAMPLE {
        checked_run(prepared, case)?;
    }
    Ok(start.elapsed())
}

/// Prints median and range in ns per instruction for interleaved samples.
/// Returns errors for invalid images, failed preparation, or unexpected loop outcomes.
fn main() -> io::Result<()> {
    let os = image(0x0200, &[0xb400, 0xfffe])?;
    let program = image(
        0x3000,
        &[
            0x2208,
            0x2006,
            0x103f,
            0x03fe,
            0x127f,
            0x03fb,
            0xb403,
            0xd000,
            INNER_ITERATIONS,
            OUTER_ITERATIONS,
            0xfffe,
        ],
    )?;
    let prepared = prepare(&os, &[program], NonZeroU32::MIN)
        .map_err(|error| io::Error::other(format!("preparation failed: {error:?}")))?;
    let cases = [
        (
            "no triggers",
            Case {
                cycle_cap: u64::MAX,
                ..Case::default()
            },
        ),
        (
            "one unreachable trigger",
            Case {
                cycle_cap: u64::MAX,
                triggers: vec![Trigger {
                    pc: 0x4000,
                    pokes: Pokes::default(),
                }],
                ..Case::default()
            },
        ),
    ];
    for (_, case) in &cases {
        checked_run(&prepared, case)?;
    }
    let mut samples: Vec<_> = (0..cases.len())
        .map(|_| Vec::with_capacity(SAMPLES))
        .collect();
    for sample in 0..SAMPLES {
        for offset in 0..cases.len() {
            let index = (sample + offset) % cases.len();
            let elapsed = measure(&prepared, &cases[index].1)?;
            samples[index].push(
                elapsed.as_secs_f64() * 1e9 / (f64::from(RUNS_PER_SAMPLE) * INSTRUCTIONS as f64),
            );
        }
    }
    println!(
        "{INSTRUCTIONS} instructions/run; {SAMPLES} samples; {RUNS_PER_SAMPLE} runs/sample; latency=1"
    );
    for ((name, _), mut values) in cases.into_iter().zip(samples) {
        values.sort_by(f64::total_cmp);
        println!(
            "{name}: median {:.3} ns/instruction; range {:.3}..{:.3}",
            values[SAMPLES / 2],
            values[0],
            values[SAMPLES - 1]
        );
    }
    Ok(())
}
