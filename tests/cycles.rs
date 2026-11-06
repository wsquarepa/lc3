mod common;

use common::halted;

fn halt_cost(n: u64) -> u64 {
    (5 + 2 * n) + (6 + 3 * n)
}

#[test]
fn register_operations_lea_and_jmp_ret_cost_4_plus_n() {
    for latency in [1, 4] {
        let n = u64::from(latency);
        for word in [0x1021, 0x1000, 0x5020, 0x5000, 0x903f, 0xe000] {
            assert_eq!(
                halted(0x3000, &[word, 0xf025], latency).cycles,
                (4 + n) + halt_cost(n)
            );
        }
        for words in [[0xe201, 0xc040, 0xf025], [0xee01, 0xc1c0, 0xf025]] {
            assert_eq!(
                halted(0x3000, &words, latency).cycles,
                (4 + n) + (4 + n) + halt_cost(n)
            );
        }
    }
}

#[test]
fn branches_cost_4_plus_n_untaken_and_5_plus_n_taken() {
    for latency in [1, 4] {
        let n = u64::from(latency);
        for word in [0x0000, 0x0200] {
            assert_eq!(
                halted(0x3000, &[word, 0xf025], latency).cycles,
                (4 + n) + halt_cost(n)
            );
        }
        assert_eq!(
            halted(0x3000, &[0x0400, 0xf025], latency).cycles,
            (5 + n) + halt_cost(n)
        );
    }
}

#[test]
fn jsr_and_jsrr_cost_5_plus_n() {
    for latency in [1, 4] {
        let n = u64::from(latency);
        assert_eq!(
            halted(0x3000, &[0x4800, 0xf025], latency).cycles,
            (5 + n) + halt_cost(n)
        );
        assert_eq!(
            halted(0x3000, &[0xee02, 0x41c0, 0xd000, 0xf025], latency).cycles,
            (4 + n) + (5 + n) + halt_cost(n)
        );
    }
}

#[test]
fn ld_and_ldr_cost_5_plus_2n() {
    for latency in [1, 4] {
        let n = u64::from(latency);
        let ld = halted(0x3000, &[0x2001, 0xf025, 0x8000], latency);
        assert_eq!(ld.registers.r[0], 0x8000);
        assert_eq!(ld.cycles, (5 + 2 * n) + halt_cost(n));
        let ldr = halted(0x3000, &[0xe202, 0x6040, 0xf025, 0x1234], latency);
        assert_eq!(ldr.registers.r[0], 0x1234);
        assert_eq!(ldr.cycles, (4 + n) + (5 + 2 * n) + halt_cost(n));
    }
}

#[test]
fn st_and_str_cost_5_plus_2n() {
    for latency in [1, 4] {
        let n = u64::from(latency);
        let st = halted(0x3000, &[0x3001, 0xf025, 0x1234], latency);
        assert_eq!(st.memory[0x3002], 0);
        assert_eq!(st.cycles, (5 + 2 * n) + halt_cost(n));
        let str = halted(0x3000, &[0xe202, 0x7040, 0xf025, 0x1234], latency);
        assert_eq!(str.memory[0x3003], 0);
        assert_eq!(str.cycles, (4 + n) + (5 + 2 * n) + halt_cost(n));
    }
}

#[test]
fn trap_costs_5_plus_2n_before_the_halt_routine() {
    for latency in [1, 4] {
        let n = u64::from(latency);
        assert_eq!(
            halted(0x3000, &[0xf025], latency).cycles,
            (5 + 2 * n) + (6 + 3 * n)
        );
    }
}

#[test]
fn ldi_and_sti_cost_6_plus_3n() {
    for latency in [1, 4] {
        let n = u64::from(latency);
        let ldi = halted(0x3000, &[0xa001, 0xf025, 0x3003, 0x1234], latency);
        assert_eq!(ldi.registers.r[0], 0x1234);
        assert_eq!(ldi.cycles, (6 + 3 * n) + halt_cost(n));
        let sti = halted(0x3000, &[0xb001, 0xf025, 0x3003, 0x1234], latency);
        assert_eq!(sti.memory[0x3003], 0);
        assert_eq!(sti.cycles, (6 + 3 * n) + halt_cost(n));
    }
}
