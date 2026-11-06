mod common;

use common::{halted, image, os};
use lc3::{BootFailure, Case, Failure, IllegalKind, Outcome, PrepareError, prepare, run};
use std::num::NonZeroU32;

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
}

#[test]
fn zero_cycle_cap_stops_before_even_an_illegal_word() {
    let prepared = prepare(&os(), &[image(0x3000, &[0x8000])], latency()).unwrap();
    assert_eq!(
        run(&prepared, &Case { cycle_cap: 0 }),
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
fn boot_reports_illegal_words_and_the_fixed_cycle_cap() {
    for (word, kind) in [(0x8001, IllegalKind::Rti), (0xd123, IllegalKind::Reserved)] {
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
            run(&prepared, &Case { cycle_cap: 9 }),
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
                run(&prepared, &Case { cycle_cap: cap }),
                Outcome::Failed(Failure::CycleCapReached)
            );
        }
        for cap in [cost + 1, u64::MAX] {
            let Outcome::Halted(halted) = run(&prepared, &Case { cycle_cap: cap }) else {
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
        run(&prepared, &Case { cycle_cap: total }),
        Outcome::Failed(Failure::CycleCapReached)
    );
    let Outcome::Halted(halted) = run(
        &prepared,
        &Case {
            cycle_cap: total + 1,
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
        run(&prepared, &Case { cycle_cap: 10 }),
        Outcome::Failed(Failure::CycleCapReached)
    );
    assert_eq!(run(&prepared, &case), alone);
    assert_eq!(
        run(&prepared, &Case { cycle_cap: 0 }),
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
