mod common;

use common::{halted, image, os};
use lc3::{Case, Failure, IllegalKind, Outcome, prepare, run};
use std::num::NonZeroU32;

#[test]
fn arithmetic_and_logic_use_wrapping_words_and_signed_immediates() {
    for (words, register, value, psr) in [
        (vec![0x1020, 0xf025], 0, 0, 0x0002),
        (vec![0x102f, 0xf025], 0, 15, 0x0001),
        (vec![0x1030, 0xf025], 0, 0xfff0, 0x0004),
        (vec![0x2002, 0x1221, 0xf025, 0xffff], 1, 0, 0x0002),
        (
            vec![0x2003, 0x2203, 0x1401, 0xf025, 0xffff, 2],
            2,
            1,
            0x0001,
        ),
        (vec![0x2002, 0x5230, 0xf025, 0x1234], 1, 0x1230, 0x0001),
        (
            vec![0x2003, 0x2403, 0x5202, 0xf025, 0x1234, 0x00ff],
            1,
            0x0034,
            0x0001,
        ),
        (vec![0x903f, 0xf025], 0, 0xffff, 0x0004),
        (vec![0x903f, 0x903f, 0xf025], 0, 0, 0x0002),
    ] {
        let halted = halted(0x3000, &words, 4);
        assert_eq!(halted.registers.r[register], value);
        assert_eq!(halted.registers.psr, psr);
    }
}

#[test]
fn lea_sets_positive_negative_and_zero_condition_codes() {
    for (origin, value, psr) in [(0x3000, 0x3001, 1), (0x8000, 0x8001, 4), (0xffff, 0, 2)] {
        let halted = halted(origin, &[0xe000, 0xf025], 4);
        assert_eq!(halted.registers.r[0], value);
        assert_eq!(halted.registers.psr, psr);
    }
}

#[test]
fn branch_masks_use_patt_and_patel_condition_bits_and_zero_never_branches() {
    for (set_flags, cc) in [(0x103f, 4), (0x1020, 2), (0x1021, 1)] {
        for mask in [0, 1, 2, 4, 7] {
            let branch = (mask << 9) | 2;
            let halted = halted(
                0x3000,
                &[set_flags, branch, 0x14a1, 0x0e01, 0x14a2, 0xf025],
                4,
            );
            assert_eq!(halted.registers.r[2], if mask & cc != 0 { 2 } else { 1 });
        }
    }
}

#[test]
fn pc_relative_loads_and_stores_sign_extend_negative_offsets() {
    let ld = halted(0x3000, &[0x0e01, 0xbeef, 0x21fe, 0xf025], 4);
    assert_eq!(ld.registers.r[0], 0xbeef);
    assert_eq!(ld.registers.psr, 4);
    let st = halted(0x3000, &[0x1027, 0x31fe, 0xf025], 4);
    assert_eq!(st.memory[0x3000], 7);
    assert_eq!(st.registers.psr, 1);
}

#[test]
fn base_relative_loads_and_stores_sign_extend_negative_offsets() {
    let halted = halted(
        0x3000,
        &[0xe205, 0x607f, 0x707e, 0xf025, 0x5555, 0xabcd, 0],
        4,
    );
    assert_eq!(halted.registers.r[0], 0xabcd);
    assert_eq!(halted.memory[0x3004], 0xabcd);
    assert_eq!(halted.registers.psr, 4);
}

#[test]
fn indirect_loads_and_stores_sign_extend_negative_offsets() {
    let ldi = halted(0x3000, &[0x0e02, 0x3002, 0xbeef, 0xa1fd, 0xf025], 4);
    assert_eq!(ldi.registers.r[0], 0xbeef);
    assert_eq!(ldi.registers.psr, 4);
    let sti = halted(0x3000, &[0x0e02, 0x3002, 0xbeef, 0xb1fd, 0xf025], 4);
    assert_eq!(sti.memory[0x3002], 0);
    assert_eq!(sti.registers.psr, 2);
}

#[test]
fn jsr_sign_extends_its_offset_and_saves_the_incremented_pc() {
    let halted = halted(0x3000, &[0x0e02, 0x11e0, 0xf025, 0x4ffd], 4);
    assert_eq!(halted.registers.r[0], 0x3004);
}

#[test]
fn jsrr_reads_r7_before_writing_the_return_pc() {
    let halted = halted(0x3000, &[0xee02, 0x41c0, 0xd000, 0x11e0, 0xf025], 4);
    assert_eq!(halted.registers.r[0], 0x3002);
}

#[test]
fn trap_uses_the_vector_table_saves_pc_and_preserves_psr() {
    let prepared = prepare(
        &os(),
        &[
            image(0x3000, &[0x903f, 0xf040, 0xd000]),
            image(0x0040, &[0x3100]),
            image(0x3100, &[0x3e01, 0xf025, 0]),
        ],
        NonZeroU32::new(4).unwrap(),
    )
    .unwrap();
    let Outcome::Halted(halted) = run(&prepared, &Case::default()) else {
        panic!("expected halted");
    };
    assert_eq!(halted.memory[0x3102], 0x3002);
    assert_eq!(halted.registers.r[7], 0x3102);
    assert_eq!(halted.registers.psr, 4);
}

#[test]
fn pc_and_image_addresses_wrap_modulo_x10000() {
    let halted = halted(0xffff, &[0x1021, 0xf025], 4);
    assert_eq!(halted.registers.r[0], 1);
    assert_eq!(halted.registers.r[7], 1);
    assert_eq!(halted.memory[0xffff], 0x1021);
    assert_eq!(halted.memory[0], 0xf025);
}

#[test]
fn base_relative_addresses_wrap_modulo_x10000() {
    let prepared = prepare(
        &os(),
        &[
            image(0x3000, &[0x2203, 0x6041, 0xf025, 0, 0xffff]),
            image(0, &[0xabcd]),
        ],
        NonZeroU32::new(4).unwrap(),
    )
    .unwrap();
    let Outcome::Halted(halted) = run(&prepared, &Case::default()) else {
        panic!("expected halted");
    };
    assert_eq!(halted.registers.r[0], 0xabcd);
    assert_eq!(halted.registers.psr, 4);
}

#[test]
fn fetch_observes_self_modifying_code() {
    let halted = halted(0x3000, &[0x2002, 0x3000, 0xd000, 0xf025], 4);
    assert_eq!(halted.memory[0x3002], 0xf025);
    assert_eq!(halted.registers.r[7], 0x3003);
}

#[test]
fn mcr_reads_x8000_and_ignores_writes_with_bit_15_set() {
    let prepared = prepare(
        &os(),
        &[
            image(0x3000, &[0xa003, 0xb002, 0xa201, 0xf025, 0xfffe]),
            image(0xfffe, &[0x1234]),
        ],
        NonZeroU32::new(4).unwrap(),
    )
    .unwrap();
    let Outcome::Halted(halted) = run(&prepared, &Case::default()) else {
        panic!("expected halted");
    };
    assert_eq!(halted.registers.r[0], 0x8000);
    assert_eq!(halted.registers.r[1], 0x8000);
    assert_eq!(halted.memory[0xfffe], 0x8000);
}

#[test]
fn mcr_write_with_bit_15_clear_halts_after_the_complete_instruction() {
    let halted = halted(0x3000, &[0x2002, 0xb002, 0xd000, 0x7fff, 0xfffe], 4);
    assert_eq!(halted.registers.r[0], 0x7fff);
    assert_eq!(halted.registers.pc, 0x3002);
    assert_eq!(halted.memory[0xfffe], 0x8000);
    assert_eq!(halted.cycles, (5 + 2 * 4) + (6 + 3 * 4));
}

#[test]
fn fetch_from_mcr_always_sees_x8000() {
    let prepared = prepare(
        &os(),
        &[image(0xfffe, &[0xf025])],
        NonZeroU32::new(4).unwrap(),
    )
    .unwrap();
    assert_eq!(
        run(&prepared, &Case::default()),
        Outcome::Failed(Failure::IllegalInstruction {
            pc: 0xfffe,
            word: 0x8000,
            kind: IllegalKind::Rti,
        })
    );
}

#[test]
fn other_addresses_in_the_device_region_are_ordinary_memory() {
    for address in [0xfe01, 0xfe08, 0xffff] {
        let prepared = prepare(
            &os(),
            &[
                image(0x3000, &[0xa003, 0x1021, 0xb001, 0xf025, address]),
                image(address, &[0x1234]),
            ],
            NonZeroU32::new(4).unwrap(),
        )
        .unwrap();
        let Outcome::Halted(halted) = run(&prepared, &Case::default()) else {
            panic!("expected halted at address {address:#06x}");
        };
        assert_eq!(halted.memory[usize::from(address)], 0x1235);
    }
}

#[test]
fn malformed_encodings_are_not_rejected_in_this_ticket() {
    for word in [0x1018, 0x5018, 0x9000, 0xf125] {
        halted(0x3000, &[word, 0xf025], 4);
    }
    halted(0x3000, &[0xe201, 0xc07f, 0xf025], 4);
}
