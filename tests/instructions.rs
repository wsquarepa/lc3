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
        (vec![0x1038, 0xf025], 0, 0xfff8, 0x0004),
        (vec![0x2002, 0x1221, 0xf025, 0xffff], 1, 0, 0x0002),
        (
            vec![0x2003, 0x2203, 0x1401, 0xf025, 0xffff, 2],
            2,
            1,
            0x0001,
        ),
        (vec![0x2002, 0x5230, 0xf025, 0x1234], 1, 0x1230, 0x0001),
        (vec![0x5020, 0xf025], 0, 0, 0x0002),
        (vec![0x2002, 0x5038, 0xf025, 0xffff], 0, 0xfff8, 0x0004),
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
fn malformed_add_fails_at_the_offending_pc_without_cycle_cost() {
    for word in [0x1018, 0x1008, 0x1010] {
        assert_illegal_word(word, IllegalKind::MalformedEncoding);
    }
}

#[test]
fn malformed_and_fails_at_the_offending_pc_without_cycle_cost() {
    for word in [0x5018, 0x5008, 0x5010] {
        assert_illegal_word(word, IllegalKind::MalformedEncoding);
    }
}

#[test]
fn malformed_jmp_fails_at_the_offending_pc_without_cycle_cost() {
    for word in [
        0xc1c1, 0xc201, 0xc200, 0xc400, 0xc800, 0xc001, 0xc002, 0xc004, 0xc008, 0xc010, 0xc020,
    ] {
        assert_illegal_word(word, IllegalKind::MalformedEncoding);
    }
}

#[test]
fn malformed_jsrr_fails_at_the_offending_pc_without_cycle_cost() {
    for word in [
        0x4201, 0x4041, 0x4200, 0x4400, 0x4001, 0x4002, 0x4004, 0x4008, 0x4010, 0x4020,
    ] {
        assert_illegal_word(word, IllegalKind::MalformedEncoding);
    }
}

#[test]
fn malformed_not_fails_at_the_offending_pc_without_cycle_cost() {
    for word in [0x903e, 0x903d, 0x903b, 0x9037, 0x902f, 0x901f, 0x9000] {
        assert_illegal_word(word, IllegalKind::MalformedEncoding);
    }
}

#[test]
fn malformed_trap_fails_at_the_offending_pc_without_cycle_cost() {
    for word in [0xf100, 0xf125, 0xf200, 0xf400, 0xf800, 0xff25] {
        assert_illegal_word(word, IllegalKind::MalformedEncoding);
    }
}

#[test]
fn rti_fails_at_the_offending_pc_without_cycle_cost() {
    for word in [0x8000, 0x8001, 0x8fff] {
        assert_illegal_word(word, IllegalKind::Rti);
    }
}

#[test]
fn reserved_opcode_fails_at_the_offending_pc_without_cycle_cost() {
    for word in [0xd000, 0xd001, 0xdfff] {
        assert_illegal_word(word, IllegalKind::Reserved);
    }
}

#[test]
fn ret_and_jsr_with_low_offset_bits_remain_legal() {
    let ret = halted(0x3000, &[0xee01, 0xc1c0, 0xf025], 4);
    assert_eq!(ret.registers.r[7], 0x3003);

    let mut words = [0xd000; 0x43];
    words[0] = 0x4840;
    words[0x41] = 0x11e0;
    words[0x42] = 0xf025;
    let jsr = halted(0x3000, &words, 4);
    assert_eq!(jsr.registers.r[0], 0x3001);
    assert_eq!(jsr.registers.r[7], 0x3043);
}

#[test]
fn trap_xff_remains_legal_and_uses_the_last_vector() {
    let prepared = prepare(
        &os(),
        &[
            image(0x3000, &[0xf0ff, 0xd000]),
            image(0x00ff, &[0x3100]),
            image(0x3100, &[0xba00, 0xfffe]),
        ],
        NonZeroU32::new(4).unwrap(),
    )
    .unwrap();
    let Outcome::Halted(halted) = run(&prepared, &Case::default()) else {
        panic!("expected TRAP xFF to halt through its vector");
    };
    assert_eq!(halted.registers.r[7], 0x3001);
    assert_eq!(halted.registers.pc, 0x3101);
    assert_eq!(halted.cycles, 31);
}

#[test]
fn legal_encoding_boundaries_reach_the_cycle_cap() {
    for word in [
        0x1000, 0x1fc7, 0x1020, 0x1038, 0x103f, 0x5000, 0x5fc7, 0x5020, 0x5038, 0x503f, 0x4800,
        0x4840, 0x4bff, 0x4fff, 0x903f, 0x9fff, 0xf000, 0xf0ff,
    ] {
        assert_legal_word(word);
    }
    for base in 0..8 {
        assert_legal_word(0xc000 | base << 6);
        assert_legal_word(0x4000 | base << 6);
    }
}

#[test]
fn unconstrained_opcodes_accept_arbitrary_operand_bits() {
    for opcode in [0x0, 0x2, 0x3, 0x6, 0x7, 0xa, 0xb, 0xe] {
        for operands in [0x000, 0x01f, 0x020, 0x038, 0x1ff, 0x3ff, 0x7ff, 0xfff] {
            assert_legal_word(opcode << 12 | operands);
        }
    }
}

fn assert_legal_word(word: u16) {
    let prepared = prepare(
        &os(),
        &[image(0x3000, &[word])],
        NonZeroU32::new(4).unwrap(),
    )
    .unwrap();
    assert_eq!(
        run(
            &prepared,
            &Case {
                cycle_cap: 1,
                ..Case::default()
            },
        ),
        Outcome::Failed(Failure::CycleCapReached),
        "word {word:#06x}"
    );
}

fn assert_illegal_word(word: u16, kind: IllegalKind) {
    for (latency, cap) in [(1, 6), (4, 9)] {
        let prepared = prepare(
            &os(),
            &[image(0x3000, &[0x1021, word])],
            NonZeroU32::new(latency).unwrap(),
        )
        .unwrap();
        assert_eq!(
            run(
                &prepared,
                &Case {
                    cycle_cap: cap,
                    ..Case::default()
                },
            ),
            Outcome::Failed(Failure::IllegalInstruction {
                pc: 0x3001,
                word,
                kind: kind.clone(),
            }),
            "word {word:#06x}, latency {latency}"
        );
    }
}
