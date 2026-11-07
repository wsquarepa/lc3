use crate::{Failure, IllegalKind, Image, Registers};
use std::num::NonZeroU32;

const MEMORY_WORDS: usize = 1 << 16;
const KBSR: u16 = 0xfe00;
const KBDR: u16 = 0xfe02;
const DSR: u16 = 0xfe04;
const DDR: u16 = 0xfe06;
const MCR: u16 = 0xfffe;
const MCR_RUNNING: u16 = 0x8000;
const CC_NEGATIVE: u16 = 0x0004;
const CC_ZERO: u16 = 0x0002;
const CC_POSITIVE: u16 = 0x0001;
const CC_MASK: u16 = CC_NEGATIVE | CC_ZERO | CC_POSITIVE;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct State {
    pub(crate) registers: Registers,
    pub(crate) memory: Box<[u16]>,
    pub(crate) program_output: Vec<u8>,
    pub(crate) halt_output: Vec<u8>,
    halted: bool,
    halt_started: bool,
    input_position: usize,
}

impl State {
    pub(crate) fn zeroed() -> Self {
        let mut memory = vec![0; MEMORY_WORDS].into_boxed_slice();
        memory[usize::from(MCR)] = MCR_RUNNING;
        Self {
            registers: Registers {
                r: [0; 8],
                pc: 0,
                psr: CC_ZERO,
            },
            memory,
            program_output: Vec::new(),
            halt_output: Vec::new(),
            halted: false,
            halt_started: false,
            input_position: 0,
        }
    }

    pub(crate) fn load_image(mut self, image: &Image) -> Self {
        let mut address = image.origin();
        for &word in image.words() {
            if address != MCR {
                self.memory[usize::from(address)] = word;
            }
            address = address.wrapping_add(1);
        }
        self
    }

    pub(crate) fn start_at(mut self, pc: u16) -> Self {
        self.registers.pc = pc;
        self.halted = false;
        self.input_position = 0;
        self.program_output.clear();
        self.halt_output.clear();
        self.halt_started = false;
        self
    }

    fn read(mut self, address: u16, input: &[u8], pc: u16) -> Result<(Self, u16), Failure> {
        if address >= KBSR {
            match address {
                KBSR | DSR => return Ok((self, 0x8000)),
                DDR => return Ok((self, 0)),
                KBDR => {
                    let byte = input
                        .get(self.input_position)
                        .ok_or(Failure::InputExhausted { pc })?;
                    self.input_position += 1;
                    let value = u16::from(*byte);
                    self.memory[usize::from(KBDR)] = value;
                    return Ok((self, value));
                }
                _ => {}
            }
        }
        let value = self.memory[usize::from(address)];
        Ok((self, value))
    }

    fn store(mut self, address: u16, value: u16) -> Self {
        if address >= KBSR {
            match address {
                DDR => {
                    if self.halt_started {
                        self.halt_output.push(value as u8);
                    } else {
                        self.program_output.push(value as u8);
                    }
                    return self;
                }
                MCR if value & MCR_RUNNING == 0 => {
                    self.halted = true;
                    return self;
                }
                KBSR | KBDR | DSR | MCR => return self,
                _ => {}
            }
        }
        self.memory[usize::from(address)] = value;
        self
    }

    pub(crate) fn execute(
        mut self,
        mem_latency: NonZeroU32,
        cycle_cap: u64,
        input: &[u8],
    ) -> Result<(Self, u64), Failure> {
        if cycle_cap == 0 {
            return Err(Failure::CycleCapReached);
        }
        let mut cycles = 0;
        while !self.halted {
            let (next, cost) = self.step(mem_latency, cycle_cap - cycles, input)?;
            self = next;
            cycles += cost;
        }
        Ok((self, cycles))
    }

    fn step(
        mut self,
        mem_latency: NonZeroU32,
        budget: u64,
        input: &[u8],
    ) -> Result<(Self, u64), Failure> {
        let pc = self.registers.pc;
        let (state, word) = self.read(pc, input, pc)?;
        self = state;
        let opcode = word >> 12;
        let n = u64::from(mem_latency.get());
        let branch_taken = word >> 9 & self.registers.psr & CC_MASK != 0;
        let cost = match opcode {
            0x0 => 4 + n + u64::from(branch_taken),
            0x1 | 0x5 | 0x9 | 0xc | 0xe => 4 + n,
            0x4 => 5 + n,
            0x2 | 0x3 | 0x6 | 0x7 => 5 + 2 * n,
            0xa | 0xb => 6 + 3 * n,
            0xf => 5 + 2 * n,
            _ => {
                return Err(Failure::IllegalInstruction {
                    pc,
                    word,
                    kind: if opcode == 8 {
                        IllegalKind::Rti
                    } else {
                        IllegalKind::Reserved
                    },
                });
            }
        };
        if cost >= budget {
            return Err(Failure::CycleCapReached);
        }
        self.registers.pc = pc.wrapping_add(1);
        let register = usize::from((word >> 9) & 7);
        let base = usize::from((word >> 6) & 7);
        match opcode {
            0x0 => {
                if branch_taken {
                    self.registers.pc = self.registers.pc.wrapping_add(sign_extend(word, 9));
                }
            }
            0x1 | 0x5 => {
                let left = self.registers.r[base];
                let right = if word & 0x20 != 0 {
                    sign_extend(word, 5)
                } else {
                    self.registers.r[usize::from(word & 7)]
                };
                let value = if opcode == 1 {
                    left.wrapping_add(right)
                } else {
                    left & right
                };
                self = self.set_register(register, value);
            }
            0x4 => {
                let target = if word & 0x0800 != 0 {
                    self.registers.pc.wrapping_add(sign_extend(word, 11))
                } else {
                    self.registers.r[base]
                };
                self.registers.r[7] = self.registers.pc;
                self.registers.pc = target;
            }
            0x2 | 0x6 => {
                let address = if opcode == 2 {
                    self.registers.pc.wrapping_add(sign_extend(word, 9))
                } else {
                    self.registers.r[base].wrapping_add(sign_extend(word, 6))
                };
                let (state, value) = self.read(address, input, pc)?;
                self = state.set_register(register, value);
            }
            0x3 | 0x7 => {
                let address = if opcode == 3 {
                    self.registers.pc.wrapping_add(sign_extend(word, 9))
                } else {
                    self.registers.r[base].wrapping_add(sign_extend(word, 6))
                };
                let value = self.registers.r[register];
                self = self.store(address, value);
            }
            0x9 => {
                let value = !self.registers.r[base];
                self = self.set_register(register, value);
            }
            0xa => {
                let pointer = self.registers.pc.wrapping_add(sign_extend(word, 9));
                let (state, address) = self.read(pointer, input, pc)?;
                let (state, value) = state.read(address, input, pc)?;
                self = state.set_register(register, value);
            }
            0xb => {
                let pointer = self.registers.pc.wrapping_add(sign_extend(word, 9));
                let (state, address) = self.read(pointer, input, pc)?;
                let value = state.registers.r[register];
                self = state.store(address, value);
            }
            0xc => self.registers.pc = self.registers.r[base],
            0xe => {
                let value = self.registers.pc.wrapping_add(sign_extend(word, 9));
                self = self.set_register(register, value);
            }
            0xf => {
                self.halt_started |= word == 0xf025;
                self.registers.r[7] = self.registers.pc;
                let (state, address) = self.read(word & 0xff, input, pc)?;
                self = state;
                self.registers.pc = address;
            }
            _ => unreachable!(),
        }
        Ok((self, cost))
    }

    fn set_register(mut self, register: usize, value: u16) -> Self {
        self.registers.r[register] = value;
        self.registers.psr = (self.registers.psr & !CC_MASK)
            | if value == 0 {
                CC_ZERO
            } else if value & 0x8000 != 0 {
                CC_NEGATIVE
            } else {
                CC_POSITIVE
            };
        self
    }
}

fn sign_extend(word: u16, bits: u32) -> u16 {
    ((word << (16 - bits)) as i16 >> (16 - bits)) as u16
}
