mod common;

use common::{halted, image, os};
use lc3::{
    Attribution, BootFailure, Case, Failure, Format, IllegalKind, MemoryPoke, Outcome, Pokes,
    PrepareError, Register, Trigger, WordCounts, load, prepare, run,
};
use std::fs;
use std::io::{ErrorKind, Write};
use std::num::NonZeroU32;
use std::path::Path;

#[test]
fn obj_runs_through_trap_halt_with_boot_cycles_excluded() {
    let halted = halted(0x3000, &[0xf025], 4);
    assert_eq!(halted.registers.r, [0, 0, 0, 0, 0, 0, 0, 0x3001]);
    assert_eq!(halted.registers.pc, 0x0211);
    assert_eq!(halted.registers.psr, 0x0002);
    assert_eq!(halted.memory.len(), 65536);
    assert_eq!(halted.memory[0x3000], 0xf025);
    assert_eq!(halted.memory[0xfffe], 0x8000);
    assert_eq!(halted.memory[0x1000], 0);
    assert_eq!(halted.cycles, (5 + 2 * 4) + (6 + 3 * 4));
    assert_eq!(halted.mem_latency, NonZeroU32::new(4).unwrap());
    assert_eq!(
        halted.attribution,
        Attribution {
            images: vec![13],
            os: 18,
            unattributed: 0,
        }
    );
    assert_eq!(
        halted.word_counts,
        WordCounts {
            images: vec![1],
            os: 493,
        }
    );
    assert_eq!(
        halted.attribution.images.iter().sum::<u64>()
            + halted.attribution.os
            + halted.attribution.unattributed,
        halted.cycles
    );
}

#[test]
fn zero_cycle_cap_stops_before_even_an_illegal_word() {
    let prepared = prepare(&os(), &[image(0x3000, &[0x8000])], latency()).unwrap();
    assert_eq!(
        run(
            &prepared,
            &Case {
                cycle_cap: 0,
                ..Case::default()
            }
        ),
        Outcome::Failed(Failure::CycleCapReached)
    );
}

#[test]
fn preparation_rejects_missing_images_before_boot() {
    for os in [os(), image(0x0200, &[0xd000])] {
        assert_eq!(prepare(&os, &[], latency()), Err(PrepareError::NoImages));
    }
}

#[test]
fn boot_reports_illegal_words_including_malformed_encodings() {
    for (word, kind) in [
        (0x8001, IllegalKind::Rti),
        (0xd123, IllegalKind::Reserved),
        (0x1018, IllegalKind::MalformedEncoding),
        (0x5018, IllegalKind::MalformedEncoding),
        (0xc1c1, IllegalKind::MalformedEncoding),
        (0x4201, IllegalKind::MalformedEncoding),
        (0x903e, IllegalKind::MalformedEncoding),
        (0xf100, IllegalKind::MalformedEncoding),
    ] {
        assert_eq!(
            prepare(
                &image(0x0200, &[word]),
                &[image(0x3000, &[0xf025])],
                latency()
            ),
            Err(PrepareError::Boot(BootFailure {
                reason: Failure::IllegalInstruction {
                    pc: 0x0200,
                    word,
                    kind,
                }
            }))
        );
    }
}

#[test]
fn boot_reports_the_fixed_cycle_cap() {
    assert_eq!(
        prepare(
            &image(0x0200, &[0x0fff]),
            &[image(0x3000, &[0xf025])],
            latency()
        ),
        Err(PrepareError::Boot(BootFailure {
            reason: Failure::CycleCapReached
        }))
    );
}

#[test]
fn illegal_instructions_report_the_offending_pc_and_word_without_cycle_cost() {
    for (word, kind) in [(0x8abc, IllegalKind::Rti), (0xddef, IllegalKind::Reserved)] {
        let prepared = prepare(&os(), &[image(0x3000, &[0x1021, word])], latency()).unwrap();
        assert_eq!(
            run(
                &prepared,
                &Case {
                    cycle_cap: 9,
                    ..Case::default()
                }
            ),
            Outcome::Failed(Failure::IllegalInstruction {
                pc: 0x3001,
                word,
                kind,
            })
        );
    }
}

#[test]
fn boot_snapshot_carries_executed_registers_memory_and_condition_codes() {
    let mut words = os().words().to_vec();
    let boot = [0x0401, 0xd000, 0x1267, 0x321c, 0xee00, 0xba00, 0xfffe];
    words[0x0200 - 0x0025..0x0207 - 0x0025].copy_from_slice(&boot);
    let prepared = prepare(
        &image(0x0025, &words),
        &[image(0x3000, &[0x15e0, 0xf025])],
        latency(),
    )
    .unwrap();
    let Outcome::Halted(halted) = run(&prepared, &Case::default()) else {
        panic!("expected halted");
    };
    assert_eq!(halted.registers.r, [0, 7, 0x0205, 0, 0, 0, 0, 0x3002]);
    assert_eq!(halted.registers.psr, 0x0001);
    assert_eq!(halted.memory[0x0220], 7);
    assert_eq!(halted.cycles, (4 + 4) + (5 + 2 * 4) + (6 + 3 * 4));
}

#[test]
fn images_overlay_the_boot_snapshot_in_order_and_use_the_first_origin() {
    let prepared = prepare(
        &os(),
        &[
            image(0x3000, &[0xd000, 0x1234]),
            image(0x3000, &[0xf025]),
            image(0x0025, &[0x3200]),
            image(0x3200, &[0xba00, 0xfffe]),
            image(0x3001, &[0xbeef]),
        ],
        latency(),
    )
    .unwrap();
    let Outcome::Halted(halted) = run(&prepared, &Case::default()) else {
        panic!("expected halted");
    };
    assert_eq!(halted.registers.pc, 0x3201);
    assert_eq!(halted.registers.r[7], 0x3001);
    assert_eq!(halted.memory[0x3001], 0xbeef);
    assert_eq!(
        halted.attribution,
        Attribution {
            images: vec![0, 13, 0, 18, 0],
            os: 0,
            unattributed: 0,
        }
    );
    assert_eq!(
        halted.word_counts,
        WordCounts {
            images: vec![2, 1, 1, 2, 1],
            os: 493,
        }
    );
    assert_eq!(halted.cycles, 31);
}

#[test]
fn a_subroutine_in_the_second_image_receives_its_instruction_cycles() {
    let prepared = prepare(
        &os(),
        &[
            image(0x3000, &[0x48ff, 0xf025]),
            image(0x3100, &[0x1021, 0xc1c0]),
        ],
        latency(),
    )
    .unwrap();
    let Outcome::Halted(result) = run(&prepared, &Case::default()) else {
        panic!("expected halted after returning from the second image");
    };
    assert_eq!(result.registers.r[0], 1);
    assert_eq!(
        result.attribution,
        Attribution {
            images: vec![22, 16],
            os: 18,
            unattributed: 0,
        }
    );
    assert_eq!(result.cycles, 56);
}

#[test]
fn an_image_overriding_the_os_halt_routine_owns_its_fetch_addresses() {
    let prepared = prepare(
        &os(),
        &[
            image(0x3000, &[0xf025]),
            image(0x0210, &[0x1261, 0xba00, 0xfffe]),
        ],
        latency(),
    )
    .unwrap();
    let Outcome::Halted(result) = run(&prepared, &Case::default()) else {
        panic!("expected halted in the replacement routine");
    };
    assert_eq!(result.registers.r[1], 1);
    assert_eq!(
        result.attribution,
        Attribution {
            images: vec![13, 26],
            os: 0,
            unattributed: 0,
        }
    );
    assert_eq!(result.cycles, 39);
}

#[test]
fn self_modifying_code_keeps_its_loading_image_owner() {
    let prepared = prepare(
        &os(),
        &[image(0x3000, &[0x2003, 0x3000, 0xd000, 0xf025, 0x1261])],
        latency(),
    )
    .unwrap();
    let Outcome::Halted(result) = run(&prepared, &Case::default()) else {
        panic!("expected halted after executing the stored ADD");
    };
    assert_eq!(result.memory[0x3002], 0x1261);
    assert_eq!(result.registers.r[1], 1);
    assert_eq!(
        result.attribution,
        Attribution {
            images: vec![47],
            os: 18,
            unattributed: 0,
        }
    );
    assert_eq!(result.cycles, 65);
}

#[test]
fn jumping_to_poked_unloaded_memory_attributes_the_halting_instruction_to_no_image() {
    let prepared = prepare(
        &os(),
        &[image(0x3000, &[0x2001, 0xc000, 0x4000])],
        latency(),
    )
    .unwrap();
    let case = Case {
        pokes: Pokes {
            memory: vec![MemoryPoke {
                address: 0x4000,
                words: vec![0x1261, 0xba00, 0xfffe],
            }],
            ..Pokes::default()
        },
        ..Case::default()
    };
    let Outcome::Halted(result) = run(&prepared, &case) else {
        panic!("expected halted in poked memory");
    };
    assert_eq!(result.registers.r[1], 1);
    assert_eq!(
        result.attribution,
        Attribution {
            images: vec![21],
            os: 0,
            unattributed: 26,
        }
    );
    assert_eq!(result.cycles, 47);
    assert_eq!(result.word_counts.images, vec![3]);
}

#[test]
fn word_counts_include_reserved_space_and_empty_images_despite_overlaps() {
    let source = b".ORIG x3000\nHALT\n.BLKW #4\n.END\n";
    let assembled = load(Format::Asm, source).unwrap().image;
    let prepared = prepare(
        &os(),
        &[assembled, image(0x3001, &[0xbeef]), image(0x4000, &[])],
        latency(),
    )
    .unwrap();
    let Outcome::Halted(result) = run(&prepared, &Case::default()) else {
        panic!("expected halted with reserved space");
    };
    assert_eq!(
        result.word_counts,
        WordCounts {
            images: vec![5, 1, 0],
            os: 493,
        }
    );
    assert_eq!(
        result.attribution,
        Attribution {
            images: vec![13, 0, 0],
            os: 18,
            unattributed: 0,
        }
    );
    assert_eq!(&result.memory[0x3001..0x3005], &[0xbeef, 0, 0, 0]);
    assert_eq!(result.cycles, 31);
}

#[test]
fn ownership_wraps_with_image_addresses_and_later_images_override_it() {
    let prepared = prepare(
        &os(),
        &[image(0xfffd, &[0, 0, 0x1021, 0xd000]), image(0, &[0xf025])],
        latency(),
    )
    .unwrap();
    let case = Case {
        pokes: Pokes {
            registers: vec![(Register::Pc, 0xffff)],
            ..Pokes::default()
        },
        ..Case::default()
    };
    let Outcome::Halted(result) = run(&prepared, &case) else {
        panic!("expected halted after wrapping the PC");
    };
    assert_eq!(result.memory[0xfffe], 0x8000);
    assert_eq!(
        result.attribution,
        Attribution {
            images: vec![8, 13],
            os: 18,
            unattributed: 0,
        }
    );
    assert_eq!(result.word_counts.images, vec![4, 1]);
    assert_eq!(result.cycles, 39);
}

#[test]
fn owner_capacity_accepts_65534_images_and_rejects_more_before_boot() {
    let mut images = vec![image(0x3000, &[]); 65_534];
    images[65_533] = image(0x3000, &[0xf025]);
    let prepared = prepare(&os(), &images, latency()).unwrap();
    let Outcome::Halted(result) = run(&prepared, &Case::default()) else {
        panic!("expected halted at the largest supported image index");
    };
    assert_eq!(result.attribution.images.len(), 65_534);
    assert_eq!(result.attribution.images[65_533], 13);
    assert_eq!(result.attribution.images.iter().sum::<u64>(), 13);
    assert_eq!(result.attribution.os, 18);
    assert_eq!(result.attribution.unattributed, 0);
    assert_eq!(result.word_counts.images.len(), 65_534);
    assert_eq!(result.word_counts.images[65_533], 1);
    assert_eq!(result.word_counts.images.iter().sum::<usize>(), 1);
    assert_eq!(result.cycles, 31);

    for count in [65_535, 65_536] {
        images.push(image(0x3000, &[]));
        for os in [os(), image(0x0200, &[0xd000])] {
            assert_eq!(
                prepare(&os, &images, latency()),
                Err(PrepareError::TooManyImages { count, max: 65_534 })
            );
        }
    }
}

#[test]
fn unreachable_triggers_preserve_illegal_input_output_and_halting_outcomes() {
    let mut words = os().words().to_vec();
    words.resize(0x0217 - 0x0025, 0);
    words[0x0210 - 0x0025..]
        .copy_from_slice(&[0x2003, 0xb003, 0xba03, 0xd000, 0x0148, 0xfe06, 0xfffe]);
    let os = image(0x0025, &words);
    for (origin, words, input, cycle_cap) in [
        (0x3000, vec![0x1021, 0x8abc], vec![], 9),
        (0x3000, vec![0x1021, 0xddef], vec![], 9),
        (0x3000, vec![0x1021, 0x1018], vec![], 9),
        (0x3000, vec![0x8000], vec![], 0),
        (0x3000, vec![0xa001, 0xf025, 0xfe02], vec![], 1000),
        (0x3000, vec![0xa001, 0xf025, 0xfe02], vec![0xff], 1000),
        (0xfe02, vec![0xd000], vec![], 1000),
        (0xfe02, vec![0xd000], vec![], 0),
        (
            0x3000,
            vec![0x2003, 0xb003, 0xf025, 0xd000, 0x0141, 0xfe06],
            vec![],
            1000,
        ),
        (0x3000, vec![0xb000, 0xfffe], vec![], 1000),
        (0x3000, vec![0xb000, 0xfffe], vec![], 18),
        (0x3000, vec![0xb000, 0xfffe], vec![], 19),
        (0x3000, vec![0xf025], vec![], 1000),
    ] {
        let prepared = prepare(&os, &[image(origin, &words)], latency()).unwrap();
        let case = Case {
            input,
            cycle_cap,
            ..Case::default()
        };
        let plain = run(&prepared, &case);
        let with_trigger = run(
            &prepared,
            &Case {
                triggers: vec![Trigger {
                    pc: 0xdead,
                    pokes: Pokes::default(),
                }],
                ..case
            },
        );
        assert_eq!(with_trigger, plain, "origin={origin:#06x}, words={words:?}");
        if let Outcome::Halted(result) = plain {
            assert_eq!(
                result.attribution.images.iter().sum::<u64>()
                    + result.attribution.os
                    + result.attribution.unattributed,
                result.cycles
            );
        }
    }
}

#[test]
fn cap_rejects_equal_or_crossing_totals_including_the_halting_instruction() {
    assert_eq!(Case::default().cycle_cap, 10_000_000);
    for n in [1, 4] {
        let prepared = prepare(
            &os(),
            &[image(0x3000, &[0xb000, 0xfffe])],
            NonZeroU32::new(n).unwrap(),
        )
        .unwrap();
        let cost = 6 + 3 * u64::from(n);
        for cap in [0, 1, cost - 1, cost] {
            assert_eq!(
                run(
                    &prepared,
                    &Case {
                        cycle_cap: cap,
                        ..Case::default()
                    }
                ),
                Outcome::Failed(Failure::CycleCapReached)
            );
        }
        for cap in [cost + 1, u64::MAX] {
            let Outcome::Halted(halted) = run(
                &prepared,
                &Case {
                    cycle_cap: cap,
                    ..Case::default()
                },
            ) else {
                panic!("expected halted with cap {cap}");
            };
            assert_eq!(halted.cycles, cost);
            assert_eq!(halted.registers.pc, 0x3001);
            assert_eq!(halted.memory[0xfffe], 0x8000);
        }
    }
}

#[test]
fn cycle_cap_applies_to_the_running_total() {
    let prepared = prepare(&os(), &[image(0x3000, &[0x1021, 0xf025])], latency()).unwrap();
    let total = (4 + 4) + (5 + 2 * 4) + (6 + 3 * 4);
    assert_eq!(
        run(
            &prepared,
            &Case {
                cycle_cap: total,
                ..Case::default()
            }
        ),
        Outcome::Failed(Failure::CycleCapReached)
    );
    let Outcome::Halted(halted) = run(
        &prepared,
        &Case {
            cycle_cap: total + 1,
            ..Case::default()
        },
    ) else {
        panic!("expected halted");
    };
    assert_eq!(halted.cycles, total);
}

#[test]
fn a_case_is_identical_alone_and_after_other_cases() {
    let prepared = prepare(
        &os(),
        &[image(0x3000, &[0x1021, 0x3001, 0xf025, 0x7777])],
        latency(),
    )
    .unwrap();
    let before = prepared.clone();
    let case = Case::default();
    let alone = run(&prepared, &case);
    assert!(matches!(alone, Outcome::Halted(_)));
    assert_eq!(
        run(
            &prepared,
            &Case {
                cycle_cap: 10,
                ..Case::default()
            }
        ),
        Outcome::Failed(Failure::CycleCapReached)
    );
    assert_eq!(run(&prepared, &case), alone);
    assert_eq!(
        run(
            &prepared,
            &Case {
                cycle_cap: 0,
                ..Case::default()
            }
        ),
        Outcome::Failed(Failure::CycleCapReached)
    );
    assert_eq!(run(&prepared, &case), alone);
    assert_eq!(prepared, before);
}

#[test]
fn prepared_is_shareable_and_runs_identically_from_several_threads() {
    let prepared = prepare(
        &os(),
        &[image(0x3000, &[0x1021, 0x3001, 0xf025, 0x7777])],
        latency(),
    )
    .unwrap();
    let case = Case::default();
    let expected = run(&prepared, &case);
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..4)
            .map(|_| scope.spawn(|| run(&prepared, &case)))
            .collect();
        for handle in handles {
            assert_eq!(handle.join().unwrap(), expected);
        }
    });
}

fn latency() -> NonZeroU32 {
    NonZeroU32::new(4).unwrap()
}

#[test]
fn register_pokes_select_the_first_instruction_and_preserve_raw_psr_at_no_cycle_cost() {
    let prepared = prepare(
        &os(),
        &[image(0x3000, &[0xd000, 0xb000, 0xfffe])],
        latency(),
    )
    .unwrap();
    let case = Case {
        pokes: Pokes {
            registers: vec![
                (Register::R0, 0x1111),
                (Register::R1, 0xffff),
                (Register::R1, 0x2222),
                (Register::R2, 0x3333),
                (Register::R3, 0x4444),
                (Register::R4, 0x5555),
                (Register::R5, 0x6666),
                (Register::R6, 0x7777),
                (Register::R7, 0x8888),
                (Register::Pc, 0x3001),
                (Register::Psr, 0xabcd),
            ],
            ..Pokes::default()
        },
        ..Case::default()
    };
    let Outcome::Halted(result) = run(&prepared, &case) else {
        panic!("expected halted after register pokes");
    };
    assert_eq!(
        result.registers.r,
        [
            0x1111, 0x2222, 0x3333, 0x4444, 0x5555, 0x6666, 0x7777, 0x8888
        ]
    );
    assert_eq!(result.registers.pc, 0x3002);
    assert_eq!(result.registers.psr, 0xabcd);
    assert_eq!(result.cycles, 18);
    assert_eq!(Case::default().pokes, Pokes::default());
    assert!(Pokes::default().registers.is_empty());
    assert!(Pokes::default().memory.is_empty());
    assert!(Case::default().triggers.is_empty());
}

#[test]
fn memory_pokes_overlay_images_wrap_and_bypass_devices_at_no_cycle_cost() {
    let prepared = prepare(&os(), &[image(0x3000, &[0xd000, 0xfffe])], latency()).unwrap();
    let case = Case {
        pokes: Pokes {
            memory: vec![
                MemoryPoke {
                    address: 0x3000,
                    words: vec![0xb000],
                },
                MemoryPoke {
                    address: 0x4000,
                    words: vec![1, 2, 3],
                },
                MemoryPoke {
                    address: 0x4001,
                    words: vec![0xbeef],
                },
                MemoryPoke {
                    address: 0xfffd,
                    words: vec![0x1111, 0, 0x3333, 0x4444],
                },
                MemoryPoke {
                    address: 0xfe00,
                    words: vec![0x1234, 0, 0xabcd, 0, 0x5678, 0, 0x0041],
                },
                MemoryPoke {
                    address: 0x3000,
                    words: vec![],
                },
            ],
            ..Pokes::default()
        },
        ..Case::default()
    };
    let Outcome::Halted(result) = run(&prepared, &case) else {
        panic!("expected halted after memory pokes");
    };
    assert_eq!(&result.memory[0x4000..0x4003], &[1, 0xbeef, 3]);
    assert_eq!(&result.memory[0xfffd..], &[0x1111, 0x8000, 0x3333]);
    assert_eq!(result.memory[0], 0x4444);
    assert_eq!(
        &result.memory[0xfe00..0xfe07],
        &[0x1234, 0, 0xabcd, 0, 0x5678, 0, 0x0041]
    );
    assert!(result.program_output.is_empty());
    assert!(result.halt_output.is_empty());
    assert_eq!(result.registers.pc, 0x3001);
    assert_eq!(result.cycles, 18);
}

#[test]
fn trigger_at_origin_follows_case_pokes_and_fires_once_across_loop_visits() {
    let prepared = prepare(
        &os(),
        &[image(0x3000, &[0x1021, 0x127f, 0x03fd, 0xb400, 0xfffe])],
        latency(),
    )
    .unwrap();
    let case = Case {
        pokes: Pokes {
            registers: vec![(Register::R0, 100), (Register::R1, 3)],
            ..Pokes::default()
        },
        triggers: vec![Trigger {
            pc: 0x3000,
            pokes: Pokes {
                registers: vec![(Register::R0, 10)],
                ..Pokes::default()
            },
        }],
        ..Case::default()
    };
    let Outcome::Halted(result) = run(&prepared, &case) else {
        panic!("expected halted after the loop");
    };
    assert_eq!(result.registers.r[0], 13);
    assert_eq!(result.registers.r[1], 0);
    assert_eq!(result.cycles, 92);
}

#[test]
fn triggers_at_one_pc_keep_list_order_before_following_a_poked_pc() {
    let prepared = prepare(
        &os(),
        &[
            image(0x3000, &[0xd000]),
            image(0x3100, &[0x1021, 0xb400, 0xfffe]),
        ],
        latency(),
    )
    .unwrap();
    let case = Case {
        triggers: vec![
            Trigger {
                pc: 0x3100,
                pokes: Pokes {
                    registers: vec![(Register::R0, 30)],
                    memory: vec![MemoryPoke {
                        address: 0x4001,
                        words: vec![3],
                    }],
                },
            },
            Trigger {
                pc: 0x3000,
                pokes: Pokes {
                    registers: vec![(Register::R0, 10), (Register::Pc, 0x3100)],
                    memory: vec![MemoryPoke {
                        address: 0x4000,
                        words: vec![1],
                    }],
                },
            },
            Trigger {
                pc: 0x3000,
                pokes: Pokes {
                    registers: vec![(Register::R0, 20)],
                    memory: vec![MemoryPoke {
                        address: 0x4000,
                        words: vec![2],
                    }],
                },
            },
        ],
        ..Case::default()
    };
    let Outcome::Halted(result) = run(&prepared, &case) else {
        panic!("expected halted at the poked PC");
    };
    assert_eq!(result.registers.r[0], 31);
    assert_eq!(&result.memory[0x4000..0x4002], &[2, 3]);
    assert_eq!(result.cycles, 26);
}

#[test]
fn triggers_at_different_pcs_fire_in_execution_order_and_leave_runs_independent() {
    let prepared = prepare(
        &os(),
        &[image(0x3000, &[0x1021, 0x1021, 0xb400, 0xfffe])],
        latency(),
    )
    .unwrap();
    let case = Case {
        triggers: vec![
            Trigger {
                pc: 0x3001,
                pokes: Pokes {
                    registers: vec![(Register::R0, 20)],
                    ..Pokes::default()
                },
            },
            Trigger {
                pc: 0x3000,
                pokes: Pokes {
                    registers: vec![(Register::R0, 10)],
                    ..Pokes::default()
                },
            },
        ],
        ..Case::default()
    };
    let prepared_before = prepared.clone();
    let case_before = case.clone();
    let alone = run(&prepared, &case);
    let Outcome::Halted(result) = &alone else {
        panic!("expected halted after both triggers");
    };
    assert_eq!(result.registers.r[0], 21);
    assert_eq!(result.cycles, 34);
    assert_eq!(run(&prepared, &case), alone);
    assert_eq!(
        run(
            &prepared,
            &Case {
                cycle_cap: 9,
                ..case.clone()
            }
        ),
        Outcome::Failed(Failure::CycleCapReached)
    );
    assert_eq!(run(&prepared, &case), alone);
    assert_eq!(prepared, prepared_before);
    assert_eq!(case, case_before);
}

#[test]
fn triggers_change_the_next_instruction_before_its_cycle_cap_check() {
    let prepared = prepare(
        &os(),
        &[image(0x3000, &[0x1021, 0xd000, 0xfffe])],
        latency(),
    )
    .unwrap();
    let case = Case {
        cycle_cap: 9,
        triggers: vec![Trigger {
            pc: 0x3001,
            pokes: Pokes {
                memory: vec![MemoryPoke {
                    address: 0x3001,
                    words: vec![0xb000],
                }],
                ..Pokes::default()
            },
        }],
        ..Case::default()
    };
    assert_eq!(
        run(&prepared, &case),
        Outcome::Failed(Failure::CycleCapReached)
    );
    let Outcome::Halted(result) = run(
        &prepared,
        &Case {
            cycle_cap: 27,
            ..case
        },
    ) else {
        panic!("expected halted after the triggered instruction replacement");
    };
    assert_eq!(result.cycles, 26);
}

#[test]
fn boot_never_applies_case_pokes_or_consumes_triggers_at_boot_pcs() {
    let mut words = vec![0; 0x21];
    words[..3].copy_from_slice(&[0x221f, 0xba00, 0xfffe]);
    words[0x20] = 5;
    let prepared = prepare(
        &image(0x0200, &words),
        &[image(0x3000, &[0xb000, 0xfffe])],
        latency(),
    )
    .unwrap();
    let mut case = Case {
        pokes: Pokes {
            memory: vec![MemoryPoke {
                address: 0x0220,
                words: vec![9],
            }],
            ..Pokes::default()
        },
        triggers: vec![Trigger {
            pc: 0x0200,
            pokes: Pokes {
                memory: vec![MemoryPoke {
                    address: 0x0220,
                    words: vec![99],
                }],
                ..Pokes::default()
            },
        }],
        ..Case::default()
    };
    let Outcome::Halted(result) = run(&prepared, &case) else {
        panic!("expected halted with the trigger unreached");
    };
    assert_eq!(result.registers.r[1], 5);
    assert_eq!(result.memory[0x0220], 9);
    assert_eq!(result.cycles, 18);
    case.pokes.registers.push((Register::Pc, 0x0200));
    let Outcome::Halted(result) = run(&prepared, &case) else {
        panic!("expected halted after revisiting the boot PC");
    };
    assert_eq!(result.registers.r[1], 99);
    assert_eq!(result.memory[0x0220], 99);
    assert_eq!(result.cycles, 31);
}

#[test]
fn pc_trigger_cycles_terminate_after_each_batch_fires_once() {
    let prepared = prepare(&os(), &[image(0x3000, &[0xb000, 0xfffe])], latency()).unwrap();
    let case = Case {
        triggers: vec![
            Trigger {
                pc: 0x3100,
                pokes: Pokes {
                    registers: vec![(Register::Pc, 0x3000)],
                    ..Pokes::default()
                },
            },
            Trigger {
                pc: 0x3000,
                pokes: Pokes {
                    registers: vec![(Register::Pc, 0x3100)],
                    ..Pokes::default()
                },
            },
        ],
        ..Case::default()
    };
    assert_eq!(run(&prepared, &case), run(&prepared, &Case::default()));
}

#[test]
fn poked_psr_uses_low_condition_bits_and_keyboard_pokes_do_not_consume_input() {
    let prepared = prepare(
        &os(),
        &[image(
            0x3000,
            &[0x0201, 0xd000, 0xa002, 0xb202, 0xd000, 0xfe02, 0xfffe],
        )],
        latency(),
    )
    .unwrap();
    let case = Case {
        input: vec![0x80],
        pokes: Pokes {
            registers: vec![(Register::Psr, 0x8001)],
            memory: vec![MemoryPoke {
                address: 0xfe02,
                words: vec![0xabcd],
            }],
        },
        ..Case::default()
    };
    let Outcome::Halted(result) = run(&prepared, &case) else {
        panic!("expected halted after a branch and keyboard read");
    };
    assert_eq!(result.registers.r[0], 0x80);
    assert_eq!(result.memory[0xfe02], 0x80);
    assert_eq!(result.registers.psr, 0x8001);
    assert_eq!(result.cycles, 45);
}

#[test]
fn memory_poke_arrays_can_wrap_more_than_once() {
    let prepared = prepare(&os(), &[image(0x3000, &[0xd000])], latency()).unwrap();
    let mut words = vec![0; 65_538];
    words[0] = 1;
    words[65_536] = 2;
    words[65_537] = 3;
    let case = Case {
        pokes: Pokes {
            memory: vec![
                MemoryPoke {
                    address: 0x2000,
                    words,
                },
                MemoryPoke {
                    address: 0x3000,
                    words: vec![0xb000, 0xfffe],
                },
            ],
            ..Pokes::default()
        },
        ..Case::default()
    };
    let Outcome::Halted(result) = run(&prepared, &case) else {
        panic!("expected halted after wrapping memory pokes");
    };
    assert_eq!(&result.memory[0x2000..0x2002], &[2, 3]);
    assert_eq!(result.memory[0xfffe], 0x8000);
    assert_eq!(result.cycles, 18);
}

#[test]
fn reference_os_start_state_puts_getc_and_out_match_measured_behavior() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("reference/lc3tools/lc3os.obj");
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            writeln!(
                std::io::stderr(),
                "skipping: reference OS not found at {}",
                path.display()
            )
            .expect("failed to write the reference OS skip message to stderr");
            return;
        }
        Err(error) => panic!("cannot read reference OS at {}: {error}", path.display()),
    };
    let os = load(Format::Obj, &bytes).unwrap().image;
    for (words, input, program_output, halt_output) in [
        (vec![0xb000, 0xfffe], vec![], &b""[..], &b""[..]),
        (
            vec![0xe002, 0xf022, 0xf025, 0x0048, 0x0069, 0],
            vec![],
            &b"Hi"[..],
            &b"\n\n--- halting the LC-3 ---\n\n"[..],
        ),
        (
            vec![0xf020, 0xf021, 0xf025],
            vec![0xff],
            &b"\xff"[..],
            &b"\n\n--- halting the LC-3 ---\n\n"[..],
        ),
    ] {
        let direct_halt = words[0] == 0xb000;
        let prepared = prepare(&os, &[image(0x3000, &words)], NonZeroU32::new(4).unwrap()).unwrap();
        let Outcome::Halted(result) = run(
            &prepared,
            &Case {
                input,
                ..Case::default()
            },
        ) else {
            panic!("expected halted under the reference OS");
        };
        assert_eq!(result.program_output, program_output);
        assert_eq!(result.halt_output, halt_output);
        if direct_halt {
            assert_eq!(result.registers.r, [0, 0x7fff, 0, 0, 0, 0, 0, 0x0490]);
            assert_eq!(result.registers.psr, 0x0002);
            assert_eq!(
                result.registers.pc.wrapping_sub(1),
                0x3000,
                "direct MCR halt advances the start PC by one"
            );
        }
    }
}

#[test]
fn register_index_numbers_general_registers_and_excludes_pc_and_psr() {
    let general = [
        Register::R0,
        Register::R1,
        Register::R2,
        Register::R3,
        Register::R4,
        Register::R5,
        Register::R6,
        Register::R7,
    ];
    for (expected, register) in general.iter().enumerate() {
        assert_eq!(register.index(), Some(expected));
    }
    assert_eq!(Register::Pc.index(), None);
    assert_eq!(Register::Psr.index(), None);
}
