//! The bundled OS TRAP contract (ADR 0005) and its exact cycle counts.
//!
//! Each routine is compared with the reference OS on program output and R0,
//! checked for R1-R6 preservation, and timed at memory latency 1 and 4. A
//! routine's cost runs from the TRAP fetch through the returning (or halting)
//! instruction; it is measured as the difference between a program that
//! executes the TRAP and the same program with the TRAP word replaced by
//! x0000 (a never-taken BR costing 4+N). The expected costs are the closed
//! forms derived in docs/bundled-os-proof.md.

use lc3::{Case, Format, Halted, Image, Outcome, load, prepare, run};
use std::fs;
use std::io::{ErrorKind, Write};
use std::num::NonZeroU32;

const BUNDLED_OS_OBJ: &[u8] = include_bytes!("fixtures/bundled_os.obj");
/// The git-excluded reference folder exists only in the main checkout, not in
/// worktrees, so its location is named once here.
const REFERENCE_OS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/reference/lc3tools/lc3os.obj");
const PRESERVED: [u16; 6] = [0x1234, 0x8765, 0xffff, 0x0000, 0x7fff, 0xa5a5];
const NOP: u16 = 0x0000;
const LEA_R0_DATA: u16 = 0xe000 | 10;
const LD_R0_DATA: u16 = 0x2000 | 10;
const IN_PROMPT: &[u8] = b"\nInput a character> ";

/// Per-instruction costs at one memory latency, from the verified FSM table.
#[derive(Clone, Copy)]
struct Costs {
    /// ADD, AND, NOT, LEA, JMP, BR not taken.
    a: u64,
    /// BR taken, JSR, JSRR.
    b: u64,
    /// LD, LDR, ST, STR, TRAP.
    m: u64,
    /// LDI, STI.
    i: u64,
}

impl Costs {
    fn at(n: u32) -> Self {
        let n = u64::from(n);
        Self {
            a: 4 + n,
            b: 5 + n,
            m: 5 + 2 * n,
            i: 6 + 3 * n,
        }
    }
}

fn image(origin: u16, words: &[u16]) -> Image {
    let bytes: Vec<u8> = std::iter::once(origin)
        .chain(words.iter().copied())
        .flat_map(u16::to_be_bytes)
        .collect();
    load(Format::Obj, &bytes).unwrap().image
}

fn bundled() -> Image {
    load(Format::Obj, BUNDLED_OS_OBJ).unwrap().image
}

/// Returns the reference OS, or `None` after printing the skip message when the
/// git-excluded reference folder is absent.
fn reference() -> Option<Image> {
    match fs::read(REFERENCE_OS) {
        Ok(bytes) => Some(load(Format::Obj, &bytes).unwrap().image),
        Err(error) if error.kind() == ErrorKind::NotFound => {
            writeln!(
                std::io::stderr(),
                "skipping: reference OS not found at {REFERENCE_OS}"
            )
            .expect("failed to write the reference OS skip message to stderr");
            None
        }
        Err(error) => panic!("cannot read reference OS at {REFERENCE_OS}: {error}"),
    }
}

/// A program at x3000: load R1-R6 with `PRESERVED`, run `r0_setup` (which may
/// point R0 at `data` or load its first word), execute `trap`, then halt by
/// writing zero to the machine control register. `data` starts at x3011.
fn program(r0_setup: u16, trap: u16, data: &[u16]) -> Vec<u16> {
    let mut words: Vec<u16> = (1..=6u16).map(|k| 0x2000 | (k << 9) | 10).collect();
    words.extend([r0_setup, trap, 0x5fe0, 0xbe00, 0xfffe]);
    words.extend(PRESERVED);
    words.extend_from_slice(data);
    words
}

fn halted(os: &Image, words: &[u16], input: &[u8], n: u32) -> Halted {
    let prepared = prepare(os, &[image(0x3000, words)], NonZeroU32::new(n).unwrap()).unwrap();
    match run(
        &prepared,
        &Case {
            input: input.to_vec(),
            ..Case::default()
        },
    ) {
        Outcome::Halted(halted) => halted,
        outcome => panic!("expected halted, got {outcome:?}"),
    }
}

/// Runs one TRAP under the bundled OS and returns the result with the
/// routine's cost, asserting that R1-R6 survive it.
fn trap_cost(r0_setup: u16, trap: u16, data: &[u16], input: &[u8], n: u32) -> (Halted, u64) {
    let os = bundled();
    let with = halted(&os, &program(r0_setup, trap, data), input, n);
    let without = halted(&os, &program(r0_setup, NOP, data), &[], n);
    assert_eq!(
        with.registers.r[1..7],
        PRESERVED,
        "R1-R6 after TRAP x{trap:04x}"
    );
    let cost = with.cycles - without.cycles + Costs::at(n).a;
    (with, cost)
}

/// Asserts that program output and R0 match the reference OS for one TRAP;
/// `reference_prefix` is output the reference prints and the bundled OS omits.
fn assert_matches_reference(
    ours: &Halted,
    r0_setup: u16,
    trap: u16,
    data: &[u16],
    input: &[u8],
    reference_prefix: &[u8],
) {
    let Some(os) = reference() else { return };
    let theirs = halted(&os, &program(r0_setup, trap, data), input, 4);
    let expected = theirs
        .program_output
        .strip_prefix(reference_prefix)
        .expect("reference output starts with the omitted prefix");
    assert_eq!(
        ours.program_output, expected,
        "program output of TRAP x{trap:04x}"
    );
    assert_eq!(
        ours.registers.r[0], theirs.registers.r[0],
        "R0 after TRAP x{trap:04x}"
    );
}

#[test]
fn bundled_os_sits_in_system_space_and_boot_fills_the_trap_vector_table() {
    let os = bundled();
    assert_eq!(os.origin(), 0x0200);
    assert!(usize::from(os.origin()) + os.words().len() <= 0x3000);
    let after_boot = halted(&os, &program(NOP, NOP, &[]), &[], 4);
    for vector in 0..0x100 {
        let entry = after_boot.memory[vector];
        if (0x20..=0x25).contains(&vector) {
            assert!(
                (0x0200..0x3000).contains(&entry),
                "vector x{vector:02x} -> x{entry:04x}"
            );
        } else {
            assert_eq!(entry, 0xfffe, "unassigned vector x{vector:02x}");
        }
    }
    let mut assigned = after_boot.memory[0x20..=0x25].to_vec();
    assigned.sort_unstable();
    assigned.dedup();
    assert_eq!(
        assigned.len(),
        6,
        "each assigned vector has its own routine"
    );
    assert!(
        after_boot.memory[0x0100..0x0200]
            .iter()
            .all(|&word| word == 0)
    );
    assert!(
        after_boot.memory[0xfe00..0xfffe]
            .iter()
            .all(|&word| word == 0)
    );
}

#[test]
fn getc_returns_the_input_byte_in_39_cycles_at_latency_4() {
    for byte in [b'q', 0x00, 0xff] {
        for n in [1, 4] {
            let c = Costs::at(n);
            let (ours, cost) = trap_cost(NOP, 0xf020, &[], &[byte], n);
            assert_eq!(ours.registers.r[0], u16::from(byte));
            assert_eq!(ours.program_output, b"");
            assert_eq!(cost, c.m + c.i + c.a, "GETC at N={n}");
            assert_matches_reference(&ours, NOP, 0xf020, &[], &[byte], b"");
        }
    }
    assert_eq!(Costs::at(4).m + Costs::at(4).i + Costs::at(4).a, 39);
    assert_eq!(Costs::at(1).m + Costs::at(1).i + Costs::at(1).a, 21);
}

#[test]
fn out_prints_the_low_byte_of_r0_in_39_cycles_at_latency_4() {
    for word in [0x1241, 0x0000, 0x80ff] {
        for n in [1, 4] {
            let c = Costs::at(n);
            let (ours, cost) = trap_cost(LD_R0_DATA, 0xf021, &[word], &[], n);
            assert_eq!(ours.program_output, [word as u8]);
            assert_eq!(ours.registers.r[0], word);
            assert_eq!(cost, c.m + c.i + c.a, "OUT at N={n}");
            assert_matches_reference(&ours, LD_R0_DATA, 0xf021, &[word], &[], b"");
        }
    }
}

#[test]
fn in_echoes_the_byte_and_a_linefeed_without_the_prompt_in_99_cycles() {
    for byte in [b'z', 0x00, 0xff] {
        for n in [1, 4] {
            let c = Costs::at(n);
            let (ours, cost) = trap_cost(NOP, 0xf023, &[], &[byte], n);
            assert_eq!(ours.program_output, [byte, b'\n']);
            assert_eq!(ours.registers.r[0], u16::from(byte));
            assert_eq!(cost, 7 * c.m + c.a, "IN at N={n}");
            assert_matches_reference(&ours, NOP, 0xf023, &[], &[byte], IN_PROMPT);
        }
    }
    assert_eq!(7 * Costs::at(4).m + Costs::at(4).a, 99);
    assert_eq!(7 * Costs::at(1).m + Costs::at(1).a, 54);
}

/// PUTS cost for a string of `len` non-zero words. Words 0-31 are read from
/// the unmoved R0 with only R7 and R1 spilled; the check of word 31 is a taken
/// BRnp over the exit block, so a 31-word string falls through to its exit.
/// Longer strings switch to the 64-word stride loop, whose ADDs fall before
/// words 32, 96, ... and whose back edges are the checks of words 95, 159, ....
fn puts_cost(len: u64, c: Costs) -> u64 {
    match len {
        0 => 4 * c.m + c.a + c.b,
        1..=30 => 7 * c.m + 2 * c.a + 1 + len * (2 * c.m + c.a),
        31 => 7 * c.m + 2 * c.a + len * (2 * c.m + c.a),
        _ => {
            12 * c.m
                + 2 * c.a
                + len * (2 * c.m + c.a)
                + 1
                + c.a * ((len + 32) / 64)
                + (len - 32) / 64
                + u64::from(!(len - 31).is_multiple_of(64))
        }
    }
}

#[test]
fn puts_prints_low_bytes_until_a_zero_word_at_34_cycles_per_character() {
    for len in [
        0, 1, 2, 7, 8, 30, 31, 32, 33, 63, 64, 65, 94, 95, 96, 159, 160, 200, 500,
    ] {
        let mut data: Vec<u16> = (0..len)
            .map(|i| (i as u16) << 8 | (0x41 + i as u16 % 26))
            .collect();
        data.extend([0x0000, 0x4242]);
        for n in [1, 4] {
            let (ours, cost) = trap_cost(LEA_R0_DATA, 0xf022, &data, &[], n);
            let expected: Vec<u8> = data[..len].iter().map(|&word| word as u8).collect();
            assert_eq!(ours.program_output, expected);
            assert_eq!(ours.registers.r[0], 0x3011);
            assert_eq!(
                cost,
                puts_cost(len as u64, Costs::at(n)),
                "PUTS L={len} N={n}"
            );
            assert_matches_reference(&ours, LEA_R0_DATA, 0xf022, &data, &[], b"");
        }
    }
    let c = Costs::at(4);
    assert_eq!(
        [0, 1, 31, 32, 64, 200].map(|len| puts_cost(len, c)),
        [69, 142, 1161, 1270, 2358, 7000]
    );
}

#[test]
fn puts_prints_a_nul_low_byte_from_a_non_zero_word() {
    let data = [0x0141, 0x0100, 0x0042, 0x0000];
    let (ours, cost) = trap_cost(LEA_R0_DATA, 0xf022, &data, &[], 4);
    assert_eq!(ours.program_output, b"A\0B");
    assert_eq!(cost, puts_cost(3, Costs::at(4)));
    assert_matches_reference(&ours, LEA_R0_DATA, 0xf022, &data, &[], b"");
}

/// Cost of one PUTSP word that prints both bytes, by its bits 15:13.
fn putsp_word_cost(class: u16, c: Costs) -> u64 {
    let (a, b) = match class {
        0 => (11, 1),
        1 | 4 => (9, 2),
        2 | 5 | 6 => (8, 3),
        3 => (9, 1),
        7 => (7, 4),
        _ => unreachable!("a word class is three bits"),
    };
    4 * c.m + a * c.a + b * c.b
}

/// Prologue and register restore of PUTSP, excluding the words.
fn putsp_fixed_cost(c: Costs) -> u64 {
    18 * c.m + 2 * c.a + c.b
}

#[test]
fn putsp_costs_per_word_class_as_a_function_of_length() {
    for class in 0..8u16 {
        for len in [0u16, 1, 2, 5] {
            let mut data: Vec<u16> = (0..len)
                .map(|i| class << 13 | (1 + i % 31) << 8 | (0x41 + i % 26))
                .collect();
            data.extend([0x0000, 0x4242]);
            for n in [1, 4] {
                let c = Costs::at(n);
                let (ours, cost) = trap_cost(LEA_R0_DATA, 0xf024, &data, &[], n);
                let expected: Vec<u8> = data[..usize::from(len)]
                    .iter()
                    .flat_map(|&word| [word as u8, (word >> 8) as u8])
                    .collect();
                assert_eq!(ours.program_output, expected);
                assert_eq!(ours.registers.r[0], 0x3011);
                let terminator = c.m + 2 * c.a + c.b;
                assert_eq!(
                    cost,
                    putsp_fixed_cost(c) + u64::from(len) * putsp_word_cost(class, c) + terminator,
                    "PUTSP class {class:03b} L={len} N={n}"
                );
                assert_matches_reference(&ours, LEA_R0_DATA, 0xf024, &data, &[], b"");
            }
        }
    }
    let c = Costs::at(4);
    assert_eq!(putsp_fixed_cost(c), 259);
    assert_eq!(
        (0..8)
            .map(|class| putsp_word_cost(class, c))
            .collect::<Vec<_>>(),
        [149, 142, 143, 133, 142, 143, 143, 144]
    );
}

#[test]
fn putsp_stops_at_a_nul_byte_in_either_half() {
    let c = Costs::at(4);
    let low_nul_sign_clear = c.m + 2 * c.a + c.b;
    let low_nul_sign_set = c.m + c.a + 2 * c.b;
    let high_nul = 3 * c.m + 9 * c.a + c.b;
    let first = 0x6261;
    for (tail, output, stop) in [
        (0x4100, &b"ab"[..], low_nul_sign_clear),
        (0x8100, &b"ab"[..], low_nul_sign_set),
        (0x0043, &b"abC"[..], high_nul),
    ] {
        let data = [first, tail, 0x6465, 0x0000];
        let (ours, cost) = trap_cost(LEA_R0_DATA, 0xf024, &data, &[], 4);
        assert_eq!(ours.program_output, output);
        assert_eq!(cost, putsp_fixed_cost(c) + putsp_word_cost(3, c) + stop);
        assert_matches_reference(&ours, LEA_R0_DATA, 0xf024, &data, &[], b"");
    }
}

#[test]
fn halt_prints_the_short_message_and_leaves_r0_zero_in_335_cycles() {
    let os = bundled();
    for n in [1, 4] {
        let c = Costs::at(n);
        let with = halted(&os, &program(NOP, 0xf025, &[]), &[], n);
        let without = halted(&os, &program(NOP, NOP, &[]), &[], n);
        let tail = c.a + c.a + c.i;
        assert_eq!(
            with.cycles - without.cycles + tail,
            19 * c.m + 11 * c.a,
            "HALT N={n}"
        );
        assert_eq!(with.program_output, b"");
        assert_eq!(with.halt_output, b"\n--- halted ---\n");
        assert_eq!(with.registers.r[0], 0);
        assert_eq!(with.registers.r[1..7], PRESERVED);
        if let Some(reference) = reference() {
            let theirs = halted(&reference, &program(NOP, 0xf025, &[]), &[], n);
            assert_eq!(theirs.program_output, b"");
            assert_eq!(with.registers.r[0], theirs.registers.r[0]);
        }
    }
    assert_eq!(19 * Costs::at(4).m + 11 * Costs::at(4).a, 335);
    assert_eq!(19 * Costs::at(1).m + 11 * Costs::at(1).a, 188);
}
