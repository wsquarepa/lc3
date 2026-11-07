use lc3::{
    BUNDLED_OS_SOURCE, Case, Failure, Format, IllegalKind, Image, MemoryPoke, Outcome, Pokes,
    Trigger, bundled_os, load, prepare, run,
};
use std::num::NonZeroU32;

const BUNDLED_OS_OBJ: &[u8] = include_bytes!("fixtures/bundled_os.obj");

#[test]
fn bundled_os_matches_lc3as_and_assembles_without_warnings() {
    let fixture = load(Format::Obj, BUNDLED_OS_OBJ).unwrap().image;
    let assembled = load(Format::Asm, BUNDLED_OS_SOURCE.as_bytes()).unwrap();
    assert!(assembled.warnings.is_empty(), "{:?}", assembled.warnings);
    assert_eq!(bundled_os(), &fixture);
    assert_eq!(assembled.image, fixture);
}

#[test]
fn bundled_os_returns_the_same_reference() {
    assert!(std::ptr::eq(bundled_os(), bundled_os()));
}

#[test]
fn unassigned_traps_under_the_bundled_os_fail_with_the_executed_vector() {
    for vector in [0x00, 0x1f, 0x26, 0xff] {
        let program = Image::new(0x3000, vec![0xf000 | u16::from(vector)]);
        let prepared = prepare(bundled_os(), &[program], NonZeroU32::MIN).unwrap();
        assert_eq!(
            run(&prepared, &Case::default()),
            Outcome::Failed(Failure::UnknownTrap { vector })
        );
    }
}

#[test]
fn a_separately_loaded_bundled_image_enables_unknown_traps() {
    let os = load(Format::Obj, BUNDLED_OS_OBJ).unwrap().image;
    let program = Image::new(0x3000, vec![0xf026]);
    let prepared = prepare(&os, &[program], NonZeroU32::MIN).unwrap();
    assert_eq!(
        run(&prepared, &Case::default()),
        Outcome::Failed(Failure::UnknownTrap { vector: 0x26 })
    );
}

#[test]
fn the_cycle_cap_takes_precedence_when_an_unknown_trap_reaches_it() {
    for (n, cap) in [(1, 12), (4, 21)] {
        for vector in [0x00, 0x1f, 0x26, 0xff] {
            let program = Image::new(0x3000, vec![0x1021, 0xf000 | u16::from(vector)]);
            let prepared = prepare(bundled_os(), &[program], NonZeroU32::new(n).unwrap()).unwrap();
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
            assert_eq!(
                run(
                    &prepared,
                    &Case {
                        cycle_cap: cap + 1,
                        ..Case::default()
                    }
                ),
                Outcome::Failed(Failure::UnknownTrap { vector })
            );
        }
    }
}

#[test]
fn unassigned_traps_under_a_different_os_keep_the_rti_failure() {
    let mut words = bundled_os().words().to_vec();
    *words.last_mut().unwrap() ^= 1;
    let os = Image::new(bundled_os().origin(), words);
    for vector in [0x00, 0x1f, 0x26, 0xff] {
        let program = Image::new(0x3000, vec![0xf000 | vector]);
        let prepared = prepare(&os, &[program], NonZeroU32::MIN).unwrap();
        assert_eq!(
            run(&prepared, &Case::default()),
            Outcome::Failed(Failure::IllegalInstruction {
                pc: 0xfffe,
                word: 0x8000,
                kind: IllegalKind::Rti,
            })
        );
    }
}

#[test]
fn poking_an_assigned_vector_to_the_sentinel_makes_it_unknown() {
    let program = Image::new(0x3000, vec![0xf020]);
    let prepared = prepare(bundled_os(), &[program], NonZeroU32::MIN).unwrap();
    for triggers in [
        Vec::new(),
        vec![Trigger {
            pc: 0x3000,
            pokes: Pokes::default(),
        }],
    ] {
        let case = Case {
            pokes: Pokes {
                memory: vec![MemoryPoke {
                    address: 0x0020,
                    words: vec![0xfffe],
                }],
                ..Pokes::default()
            },
            triggers,
            ..Case::default()
        };
        assert_eq!(
            run(&prepared, &case),
            Outcome::Failed(Failure::UnknownTrap { vector: 0x20 })
        );
    }
}
