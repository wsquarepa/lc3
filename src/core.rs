use crate::{Failure, IllegalKind, Image, Pokes, Register, Registers, Trigger};
use std::num::NonZeroU32;

pub(crate) const MEMORY_WORDS: usize = 1 << 16;
const KBSR: u16 = 0xfe00;
const KBDR: u16 = 0xfe02;
const DSR: u16 = 0xfe04;
const DDR: u16 = 0xfe06;
const MCR: u16 = 0xfffe;
const MCR_RUNNING: u16 = 0x8000;
const HALT_VECTOR: u16 = 0x0025;
const CC_NEGATIVE: u16 = 0x0004;
const CC_ZERO: u16 = 0x0002;
const CC_POSITIVE: u16 = 0x0001;
const CC_MASK: u16 = CC_NEGATIVE | CC_ZERO | CC_POSITIVE;

enum Instruction {
    Branch,
    Add,
    Load,
    Store,
    JumpSubroutine,
    And,
    LoadBase,
    StoreBase,
    Not,
    LoadIndirect,
    StoreIndirect,
    Jump,
    LoadAddress,
    Trap,
    Illegal(IllegalKind),
}

impl Instruction {
    fn decode(word: u16) -> Self {
        match word >> 12 {
            0x0 => Self::Branch,
            0x1 if word & 0x20 != 0 || word & 0x18 == 0 => Self::Add,
            0x2 => Self::Load,
            0x3 => Self::Store,
            0x4 if word & 0x0800 != 0 || word & 0xfe3f == 0x4000 => Self::JumpSubroutine,
            0x5 if word & 0x20 != 0 || word & 0x18 == 0 => Self::And,
            0x6 => Self::LoadBase,
            0x7 => Self::StoreBase,
            0x8 => Self::Illegal(IllegalKind::Rti),
            0x9 if word & 0xf03f == 0x903f => Self::Not,
            0xa => Self::LoadIndirect,
            0xb => Self::StoreIndirect,
            0xc if word & 0xfe3f == 0xc000 => Self::Jump,
            0xd => Self::Illegal(IllegalKind::Reserved),
            0xe => Self::LoadAddress,
            0xf if word & 0xff00 == 0xf000 => Self::Trap,
            _ => Self::Illegal(IllegalKind::MalformedEncoding),
        }
    }

    fn cost(&self, mem_latency: NonZeroU32, branch_taken: bool) -> u64 {
        let n = u64::from(mem_latency.get());
        match self {
            Self::Branch => 4 + n + u64::from(branch_taken),
            Self::Add | Self::And | Self::Not | Self::Jump | Self::LoadAddress => 4 + n,
            Self::JumpSubroutine => 5 + n,
            Self::Load | Self::Store | Self::LoadBase | Self::StoreBase | Self::Trap => 5 + 2 * n,
            Self::LoadIndirect | Self::StoreIndirect => 6 + 3 * n,
            Self::Illegal(_) => 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct State {
    pub(crate) registers: Registers,
    pub(crate) memory: Box<[u16]>,
    pub(crate) program_output: Vec<u8>,
    pub(crate) halt_output: Vec<u8>,
    halted: bool,
    halt_started: bool,
    input_position: usize,
    bundled_os: bool,
}

impl State {
    pub(crate) fn zeroed(bundled_os: bool) -> Self {
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
            bundled_os,
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

    /// Returns state with memory then register pokes applied directly, without
    /// device effects or cycles; memory addresses wrap and MCR writes are ignored.
    pub(crate) fn apply_pokes(mut self, pokes: &Pokes) -> Self {
        for poke in &pokes.memory {
            let mut address = poke.address;
            for &word in &poke.words {
                if address != MCR {
                    self.memory[usize::from(address)] = word;
                }
                address = address.wrapping_add(1);
            }
        }
        for (register, value) in &pokes.registers {
            match (register.index(), register) {
                (Some(index), _) => self.registers.r[index] = *value,
                (None, Register::Pc) => self.registers.pc = *value,
                (None, _) => self.registers.psr = *value,
            }
        }
        self
    }

    // Inlining read, store and set_register trades code size for 18.6 -> 14.7 us
    // per case with boxed state (release, Ryzen AI 9 365, core 0).
    #[inline(always)]
    fn read(
        mut self: Box<Self>,
        address: u16,
        input: &[u8],
        pc: u16,
    ) -> Result<(Box<Self>, u16), Failure> {
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

    #[inline(always)]
    fn store(mut self: Box<Self>, address: u16, value: u16) -> Box<Self> {
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

    /// Executes with fixed fetch-address owners and optional once-per-PC triggers.
    /// Each owner must be below `owner_count`. Returns halted
    /// state, total cycles and owner buckets, or a cap, decode or input failure.
    /// Trigger checks compile out when `WITH_TRIGGERS` is false; otherwise a
    /// 64 KiB pending table gives constant-time checks between trigger batches.
    /// Captures the halt-routine address from x0025 before any triggers or fetches.
    pub(crate) fn execute<const WITH_TRIGGERS: bool>(
        self,
        mem_latency: NonZeroU32,
        cycle_cap: u64,
        input: &[u8],
        triggers: &[Trigger],
        owners: &[u16; MEMORY_WORDS],
        owner_count: usize,
    ) -> Result<(Self, u64, Vec<u64>), Failure> {
        // One small allocation replaces repeated 96-byte State moves through
        // read/step results: 96.6 -> 14.7 us/case (release, Ryzen AI 9 365, core 0).
        let mut state = Box::new(self);
        let halt_address = state.memory[usize::from(HALT_VECTOR)];
        let mut pending = if WITH_TRIGGERS {
            vec![false; MEMORY_WORDS]
        } else {
            Vec::new()
        };
        if WITH_TRIGGERS {
            for trigger in triggers {
                pending[usize::from(trigger.pc)] = true;
            }
        }
        let mut buckets = vec![0; owner_count];
        let mut cycles = 0;
        while !state.halted {
            if WITH_TRIGGERS {
                while pending[usize::from(state.registers.pc)] {
                    let pc = state.registers.pc;
                    pending[usize::from(pc)] = false;
                    for trigger in triggers.iter().filter(|trigger| trigger.pc == pc) {
                        state = Box::new((*state).apply_pokes(&trigger.pokes));
                    }
                }
            }
            if cycle_cap == 0 {
                return Err(Failure::CycleCapReached);
            }
            let pc = state.registers.pc;
            if pc == halt_address {
                state.halt_started = true;
            }
            let (next, cost) = state.step(mem_latency, cycle_cap - cycles, input)?;
            state = next;
            cycles += cost;
            buckets[usize::from(owners[usize::from(pc)])] += cost;
        }
        Ok((*state, cycles, buckets))
    }

    // Keep instruction dispatch inside execution: outlining it adds per-instruction
    // state transfers across a call boundary, measurable even without triggers.
    #[inline(always)]
    fn step(
        mut self: Box<Self>,
        mem_latency: NonZeroU32,
        budget: u64,
        input: &[u8],
    ) -> Result<(Box<Self>, u64), Failure> {
        let pc = self.registers.pc;
        let (state, word) = self.read(pc, input, pc)?;
        self = state;
        let instruction = Instruction::decode(word);
        let branch_taken = word >> 9 & self.registers.psr & CC_MASK != 0;
        let cost = instruction.cost(mem_latency, branch_taken);
        if cost >= budget {
            return Err(Failure::CycleCapReached);
        }
        let mut next_pc = pc.wrapping_add(1);
        let register = usize::from((word >> 9) & 7);
        let base = usize::from((word >> 6) & 7);
        match instruction {
            Instruction::Branch => {
                if branch_taken {
                    next_pc = next_pc.wrapping_add(sign_extend(word, 9));
                }
            }
            Instruction::Add | Instruction::And => {
                let left = self.registers.r[base];
                let right = if word & 0x20 != 0 {
                    sign_extend(word, 5)
                } else {
                    self.registers.r[usize::from(word & 7)]
                };
                let value = if matches!(instruction, Instruction::Add) {
                    left.wrapping_add(right)
                } else {
                    left & right
                };
                self = self.set_register(register, value);
            }
            Instruction::JumpSubroutine => {
                let target = if word & 0x0800 != 0 {
                    next_pc.wrapping_add(sign_extend(word, 11))
                } else {
                    self.registers.r[base]
                };
                self.registers.r[7] = next_pc;
                next_pc = target;
            }
            Instruction::Load | Instruction::LoadBase => {
                let address = if matches!(instruction, Instruction::Load) {
                    next_pc.wrapping_add(sign_extend(word, 9))
                } else {
                    self.registers.r[base].wrapping_add(sign_extend(word, 6))
                };
                let (state, value) = self.read(address, input, pc)?;
                self = state.set_register(register, value);
            }
            Instruction::Store | Instruction::StoreBase => {
                let address = if matches!(instruction, Instruction::Store) {
                    next_pc.wrapping_add(sign_extend(word, 9))
                } else {
                    self.registers.r[base].wrapping_add(sign_extend(word, 6))
                };
                let value = self.registers.r[register];
                self = self.store(address, value);
            }
            Instruction::Not => {
                let value = !self.registers.r[base];
                self = self.set_register(register, value);
            }
            Instruction::LoadIndirect => {
                let pointer = next_pc.wrapping_add(sign_extend(word, 9));
                let (state, address) = self.read(pointer, input, pc)?;
                let (state, value) = state.read(address, input, pc)?;
                self = state.set_register(register, value);
            }
            Instruction::StoreIndirect => {
                let pointer = next_pc.wrapping_add(sign_extend(word, 9));
                let (state, address) = self.read(pointer, input, pc)?;
                let value = state.registers.r[register];
                self = state.store(address, value);
            }
            Instruction::Jump => next_pc = self.registers.r[base],
            Instruction::LoadAddress => {
                let value = next_pc.wrapping_add(sign_extend(word, 9));
                self = self.set_register(register, value);
            }
            Instruction::Trap => {
                self.halt_started |= word == 0xf025;
                self.registers.r[7] = next_pc;
                let (state, address) = self.read(word & 0xff, input, pc)?;
                if state.bundled_os && address == MCR {
                    return Err(Failure::UnknownTrap { vector: word as u8 });
                }
                self = state;
                next_pc = address;
            }
            Instruction::Illegal(kind) => {
                return Err(Failure::IllegalInstruction { pc, word, kind });
            }
        }
        self.registers.pc = next_pc;
        Ok((self, cost))
    }

    #[inline(always)]
    fn set_register(mut self: Box<Self>, register: usize, value: u16) -> Box<Self> {
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
