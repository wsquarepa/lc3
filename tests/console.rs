mod common;

use common::{halted, image, os};
use lc3::{BootFailure, Case, Failure, IllegalKind, Outcome, PrepareError, prepare, run};
use std::num::NonZeroU32;

#[test]
fn status_reads_are_always_ready_and_device_stores_preserve_backing_words() {
    for address in [0xfe00, 0xfe04] {
        let prepared = prepare(
            &os(),
            &[
                image(0x3000, &[0xa004, 0xb203, 0xa402, 0xf025, 0, address]),
                image(address, &[0x1234]),
            ],
            NonZeroU32::new(4).unwrap(),
        )
        .unwrap();
        let Outcome::Halted(result) = run(&prepared, &Case::default()) else {
            panic!("expected halted");
        };
        assert_eq!(result.registers.r[0], 0x8000);
        assert_eq!(result.registers.r[2], 0x8000);
        assert_eq!(result.memory[usize::from(address)], 0x1234);
    }
    let result = halted(0x3000, &[0xb001, 0xf025, 0xfe02], 4);
    assert_eq!(result.memory[0xfe02], 0);
}

#[test]
fn keyboard_reads_consume_zero_extended_bytes_and_report_the_reading_pc() {
    let prepared = prepare(
        &os(),
        &[image(
            0x3000,
            &[0xa005, 0xa204, 0xa403, 0xf025, 0, 0, 0xfe02],
        )],
        NonZeroU32::new(4).unwrap(),
    )
    .unwrap();
    assert!(Case::default().input.is_empty());
    assert_eq!(
        run(&prepared, &Case::default()),
        Outcome::Failed(Failure::InputExhausted { pc: 0x3000 })
    );
    assert_eq!(
        run(
            &prepared,
            &Case {
                input: vec![0x80, 0xff],
                ..Case::default()
            }
        ),
        Outcome::Failed(Failure::InputExhausted { pc: 0x3002 })
    );
    let case = Case {
        input: vec![0x80, 0xff, 0],
        ..Case::default()
    };
    let alone = run(&prepared, &case);
    let Outcome::Halted(result) = &alone else {
        panic!("expected halted");
    };
    assert_eq!(&result.registers.r[..3], &[0x80, 0xff, 0]);
    assert_eq!(
        run(&prepared, &Case::default()),
        Outcome::Failed(Failure::InputExhausted { pc: 0x3000 })
    );
    assert_eq!(run(&prepared, &case), alone);
}

#[test]
fn independent_runs_preserve_raw_console_bytes_and_leave_prepared_unchanged() {
    let prepared = prepare(
        &os(),
        &[image(
            0x3000,
            &[
                0xa008, 0xb008, 0xa006, 0xb006, 0xa004, 0xb004, 0xba04, 0xd000, 0xf025, 0xfe02,
                0xfe06, 0xfffe,
            ],
        )],
        NonZeroU32::new(4).unwrap(),
    )
    .unwrap();
    let before = prepared.clone();
    for input in [vec![0, 0x80, 0xff], b"abc".to_vec(), vec![0, 0x80, 0xff]] {
        let case = Case {
            input: input.clone(),
            ..Case::default()
        };
        let Outcome::Halted(result) = run(&prepared, &case) else {
            panic!("expected halted");
        };
        assert_eq!(result.program_output, input);
        assert_eq!(result.memory[0xfe02], u16::from(input[2]));
        assert!(result.halt_output.is_empty());
        assert_eq!(case.input, input);
    }
    assert_eq!(prepared, before);
}

#[test]
fn display_writes_split_at_the_first_executed_halt_trap_and_use_the_low_byte() {
    let mut words = os().words().to_vec();
    words.resize(0x0217 - 0x0025, 0);
    words[0x0210 - 0x0025..]
        .copy_from_slice(&[0x2003, 0xb003, 0xba03, 0xd000, 0x0148, 0xfe06, 0xfffe]);
    let prepared = prepare(
        &image(0x0025, &words),
        &[
            image(0x3000, &[0x2003, 0xb003, 0xf025, 0xd000, 0x0141, 0xfe06]),
            image(0xfe06, &[0x1234]),
        ],
        NonZeroU32::new(4).unwrap(),
    )
    .unwrap();
    let Outcome::Halted(result) = run(&prepared, &Case::default()) else {
        panic!("expected halted");
    };
    assert_eq!(result.program_output, b"A");
    assert_eq!(result.halt_output, b"H");
    assert_eq!(result.memory[0xfe06], 0x1234);
}

#[test]
fn display_reads_return_zero_without_changing_backing_words() {
    let prepared = prepare(
        &os(),
        &[
            image(0x3000, &[0xa001, 0xf025, 0xfe06]),
            image(0xfe06, &[0xabcd]),
        ],
        NonZeroU32::new(4).unwrap(),
    )
    .unwrap();
    let Outcome::Halted(result) = run(&prepared, &Case::default()) else {
        panic!("expected halted");
    };
    assert_eq!(result.registers.r[0], 0);
    assert_eq!(result.memory[0xfe06], 0xabcd);
    assert!(result.program_output.is_empty());
    assert!(result.halt_output.is_empty());
}

#[test]
fn ld_ldr_and_ldi_consume_keyboard_bytes_with_normal_memory_costs() {
    for (origin, words, pc, costs) in [
        (0xfdf0, vec![0x2011, 0xf025], 0xfdf0, [23, 44]),
        (
            0x3000,
            vec![0x2202, 0x6040, 0xf025, 0xfe02],
            0x3001,
            [30, 57],
        ),
        (0x3000, vec![0xa001, 0xf025, 0xfe02], 0x3000, [25, 49]),
    ] {
        for (n, cycles) in [1, 4].into_iter().zip(costs) {
            let prepared =
                prepare(&os(), &[image(origin, &words)], NonZeroU32::new(n).unwrap()).unwrap();
            assert_eq!(
                run(&prepared, &Case::default()),
                Outcome::Failed(Failure::InputExhausted { pc })
            );
            let Outcome::Halted(result) = run(
                &prepared,
                &Case {
                    input: vec![0x80],
                    ..Case::default()
                },
            ) else {
                panic!("expected halted");
            };
            assert_eq!(result.registers.r[0], 0x80);
            assert_eq!(result.memory[0xfe02], 0x80);
            assert_eq!(result.cycles, cycles);
        }
    }
}

#[test]
fn st_str_and_sti_write_display_bytes_with_normal_memory_costs() {
    for (origin, words, byte, costs) in [
        (0xfdf0, vec![0x2002, 0x3014, 0xf025, 0x0141], b'A', [30, 57]),
        (
            0x3000,
            vec![0x2004, 0x2204, 0x7040, 0xf025, 0, 0x0142, 0xfe06],
            b'B',
            [37, 70],
        ),
        (
            0x3000,
            vec![0x2003, 0xb003, 0xf025, 0, 0x0143, 0xfe06],
            b'C',
            [32, 62],
        ),
    ] {
        for (n, cycles) in [1, 4].into_iter().zip(costs) {
            let result = halted(origin, &words, n);
            assert_eq!(result.program_output, [byte]);
            assert!(result.halt_output.is_empty());
            assert_eq!(result.memory[0xfe06], 0);
            assert_eq!(result.cycles, cycles);
        }
    }
}

#[test]
fn indirect_pointer_reads_observe_keyboard_and_status_devices() {
    for (offset, input, address) in [
        (0x11, vec![0x80], 0x0080),
        (0x0f, vec![], 0x8000),
        (0x13, vec![], 0x8000),
    ] {
        for opcode in [0xa000, 0xb000] {
            let prepared = prepare(
                &os(),
                &[
                    image(0xfdf0, &[opcode | offset, 0xf025]),
                    image(address, &[0xbeef]),
                ],
                NonZeroU32::new(4).unwrap(),
            )
            .unwrap();
            let Outcome::Halted(result) = run(
                &prepared,
                &Case {
                    input: input.clone(),
                    ..Case::default()
                },
            ) else {
                panic!("expected halted");
            };
            if opcode == 0xa000 {
                assert_eq!(result.registers.r[0], 0xbeef);
            } else {
                assert_eq!(result.memory[usize::from(address)], 0);
            }
            if offset == 0x11 {
                assert_eq!(result.memory[0xfe02], 0x80);
                assert_eq!(
                    run(&prepared, &Case::default()),
                    Outcome::Failed(Failure::InputExhausted { pc: 0xfdf0 })
                );
            }
        }
    }
}

#[test]
fn instruction_fetch_uses_keyboard_status_and_display_read_semantics() {
    let prepared = prepare(
        &os(),
        &[
            image(0xfe02, &[0xd000, 0x0e0c]),
            image(0xfe10, &[0x21f1, 0xf025]),
        ],
        NonZeroU32::new(4).unwrap(),
    )
    .unwrap();
    assert_eq!(
        run(&prepared, &Case::default()),
        Outcome::Failed(Failure::InputExhausted { pc: 0xfe02 })
    );
    assert_eq!(
        run(
            &prepared,
            &Case {
                input: vec![0],
                ..Case::default()
            }
        ),
        Outcome::Failed(Failure::InputExhausted { pc: 0xfe10 })
    );
    let Outcome::Halted(result) = run(
        &prepared,
        &Case {
            input: vec![0, 0xff],
            ..Case::default()
        },
    ) else {
        panic!("expected halted");
    };
    assert_eq!(result.registers.r[0], 0xff);
    assert_eq!(result.memory[0xfe02], 0xff);
    for address in [0xfe00, 0xfe04] {
        let prepared = prepare(
            &os(),
            &[image(address, &[0xf025])],
            NonZeroU32::new(4).unwrap(),
        )
        .unwrap();
        assert_eq!(
            run(&prepared, &Case::default()),
            Outcome::Failed(Failure::IllegalInstruction {
                pc: address,
                word: 0x8000,
                kind: IllegalKind::Rti
            })
        );
    }
    let result = halted(0xfe06, &[0xf025, 0xf025], 4);
    assert_eq!(result.registers.r[7], 0xfe08);
    assert_eq!(result.memory[0xfe06], 0xf025);
    assert!(result.program_output.is_empty());
    assert!(result.halt_output.is_empty());
}

#[test]
fn ignored_device_accesses_preserve_input_and_backing_words() {
    let prepared = prepare(
        &os(),
        &[
            image(
                0x3000,
                &[0xb005, 0xa204, 0xb003, 0xa403, 0xf025, 0, 0xfe02, 0xfe06],
            ),
            image(0xfe02, &[0x1234]),
            image(0xfe06, &[0xabcd]),
        ],
        NonZeroU32::new(4).unwrap(),
    )
    .unwrap();
    let Outcome::Halted(result) = run(
        &prepared,
        &Case {
            input: vec![0x80],
            ..Case::default()
        },
    ) else {
        panic!("expected halted");
    };
    assert_eq!(result.registers.r[1], 0x80);
    assert_eq!(result.registers.r[2], 0);
    assert_eq!(result.memory[0xfe02], 0x80);
    assert_eq!(result.memory[0xfe06], 0xabcd);
    assert!(result.program_output.is_empty());
    assert!(result.halt_output.is_empty());
}

#[test]
fn boot_has_no_console_input_and_discards_both_output_buffers() {
    assert_eq!(
        prepare(
            &image(0x0200, &[0xa001, 0xd000, 0xfe02]),
            &[image(0x3000, &[0xf025])],
            NonZeroU32::new(4).unwrap()
        ),
        Err(PrepareError::Boot(BootFailure {
            reason: Failure::InputExhausted { pc: 0x0200 }
        }))
    );
    let mut words = os().words().to_vec();
    words.resize(0x0216 - 0x0025, 0);
    words[0x0200 - 0x0025..0x0207 - 0x0025]
        .copy_from_slice(&[0x2003, 0xb003, 0xf025, 0xd000, 0x0042, 0xfe06, 0]);
    words[0x0210 - 0x0025..].copy_from_slice(&[0xb003, 0xba03, 0xd000, 0, 0xfe06, 0xfffe]);
    let prepared = prepare(
        &image(0x0025, &words),
        &[image(
            0x3000,
            &[0xa003, 0xb003, 0xba03, 0xd000, 0xfe02, 0xfe06, 0xfffe],
        )],
        NonZeroU32::new(4).unwrap(),
    )
    .unwrap();
    let Outcome::Halted(result) = run(
        &prepared,
        &Case {
            input: vec![b'P'],
            ..Case::default()
        },
    ) else {
        panic!("expected halted");
    };
    assert_eq!(result.program_output, b"P");
    assert!(result.halt_output.is_empty());
}

#[test]
fn halt_output_continues_across_returning_and_repeated_halt_traps() {
    let prepared = prepare(
        &os(),
        &[
            image(
                0x3000,
                &[
                    0x2006, 0xb006, 0xf025, 0x2005, 0xb003, 0xf025, 0xba03, 0x0041, 0xfe06, 0x0042,
                    0xfffe,
                ],
            ),
            image(0x0210, &[0x2002, 0xb002, 0xc1c0, 0x0048, 0xfe06]),
        ],
        NonZeroU32::new(4).unwrap(),
    )
    .unwrap();
    let Outcome::Halted(result) = run(&prepared, &Case::default()) else {
        panic!("expected halted");
    };
    assert_eq!(result.program_output, b"A");
    assert_eq!(result.halt_output, b"HBH");
}

#[test]
fn other_traps_start_halt_output_at_the_halt_entry_or_an_exact_halt_trap() {
    for word in [0xf040, 0xf024] {
        let prepared = prepare(
            &os(),
            &[
                image(0x3000, &[word]),
                image(0x0024, &[0x0210]),
                image(0x0040, &[0x0210]),
                image(
                    0x0210,
                    &[
                        0x2004, 0xb004, 0xba04, 0xd000, 0xf025, 0x0041, 0xfe06, 0xfffe,
                    ],
                ),
            ],
            NonZeroU32::new(4).unwrap(),
        )
        .unwrap();
        let Outcome::Halted(result) = run(&prepared, &Case::default()) else {
            panic!("expected halted");
        };
        assert!(result.program_output.is_empty());
        assert_eq!(result.halt_output, b"A");
    }
    let prepared = prepare(
        &os(),
        &[
            image(0x3000, &[0xf040]),
            image(0x0040, &[0x3100]),
            image(0x3100, &[0xf025]),
            image(0x0210, &[0x2002, 0xb002, 0xba02, 0x0048, 0xfe06, 0xfffe]),
        ],
        NonZeroU32::new(4).unwrap(),
    )
    .unwrap();
    let Outcome::Halted(result) = run(&prepared, &Case::default()) else {
        panic!("expected halted");
    };
    assert!(result.program_output.is_empty());
    assert_eq!(result.halt_output, b"H");
}
