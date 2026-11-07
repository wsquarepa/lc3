use crate::Image;
use crate::core::State;
use std::num::NonZeroU32;

const BOOT_CYCLE_CAP: u64 = 10_000_000;

/// An immutable boot snapshot with images loaded, ready for independent runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prepared {
    snapshot: State,
    mem_latency: NonZeroU32,
}

/// A register accepted by a case or trigger poke; pokes do not update other registers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Register {
    /// General-purpose register R0.
    R0,
    /// General-purpose register R1.
    R1,
    /// General-purpose register R2.
    R2,
    /// General-purpose register R3.
    R3,
    /// General-purpose register R4.
    R4,
    /// General-purpose register R5.
    R5,
    /// General-purpose register R6.
    R6,
    /// General-purpose register R7.
    R7,
    /// Address from which the next instruction is fetched.
    Pc,
    /// Raw processor status, with condition codes in bits 2:0; stored verbatim.
    Psr,
}

/// Backing words written upward from an address, wrapping modulo x10000.
/// Device addresses have no side effects. A poke of xFFFE has no effect because
/// the machine control register always reads x8000. An empty word list does nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryPoke {
    /// Address receiving the first word, if any.
    pub address: u16,
    /// Words written in order, including repeated wraps for lists over 65,536 words.
    pub words: Vec<u16>,
}

/// Direct state writes at no cycle cost: memory first, then registers, each in
/// list order so later pokes overwrite earlier ones. Both lists default to empty.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Pokes {
    /// Register values stored verbatim without updating condition codes.
    pub registers: Vec<(Register, u16)>,
    /// Backing memory writes that bypass device semantics, including console I/O.
    pub memory: Vec<MemoryPoke>,
}

/// Pokes applied once per run before the first fetch at `pc`, at no cycle cost.
/// Triggers at the same PC fire in list order, even when one changes PC; triggers
/// at the resulting PC fire before its fetch. Triggers never apply during boot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trigger {
    /// Instruction address that causes this trigger to fire, before cycle-cap checks.
    pub pc: u16,
    /// State writes applied when this trigger fires.
    pub pokes: Pokes,
}

/// Per-run pokes, triggers, console input bytes and cycle cap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Case {
    /// Console bytes consumed in order by KBDR reads; defaults to empty.
    pub input: Vec<u8>,
    /// A run fails before executing an instruction whose cost would make the
    /// running total reach or exceed this cap, including a halting instruction.
    /// Zero permits no instructions to execute.
    pub cycle_cap: u64,
    /// Pokes applied once after loading and selecting the first image's origin,
    /// before any triggers or instructions. Defaults to empty; never applies at boot.
    pub pokes: Pokes,
    /// Triggers checked before instruction fetch; defaults to empty. Fired state
    /// belongs to each run, so reusing a case produces independent outcomes.
    pub triggers: Vec<Trigger>,
}

impl Default for Case {
    fn default() -> Self {
        Self {
            input: Vec::new(),
            cycle_cap: 10_000_000,
            pokes: Pokes::default(),
            triggers: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Halted(Halted),
    Failed(Failure),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Halted {
    pub registers: Registers,
    /// All 65,536 backing words, with the machine control register reading x8000.
    /// A KBDR read stores its zero-extended input byte; pokes may store any word.
    /// Console device writes and reads of KBSR, DSR and DDR leave their backing
    /// words unchanged; case and trigger pokes write them directly.
    pub memory: Box<[u16]>,
    /// Console bytes written before the run first executes the word xF025.
    pub program_output: Vec<u8>,
    /// Console bytes written from the first executed TRAP x25 onward; empty
    /// when the run writes the machine control register without executing xF025.
    pub halt_output: Vec<u8>,
    pub cycles: u64,
    pub mem_latency: NonZeroU32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registers {
    pub r: [u16; 8],
    pub pc: u16,
    pub psr: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// KBDR was read after all console input bytes were consumed.
    InputExhausted {
        /// Address of the reading instruction, including instruction fetch.
        pc: u16,
    },
    CycleCapReached,
    IllegalInstruction {
        pc: u16,
        word: u16,
        kind: IllegalKind,
    },
}

/// Why a fetched word cannot execute; returned by run and boot failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IllegalKind {
    /// Any opcode x8 word; RTI is unsupported without privilege modes.
    Rti,
    /// Any opcode xD word, which is reserved by the instruction set.
    Reserved,
    /// Invalid fixed bits in an ADD, AND, JMP, JSRR, NOT, or TRAP encoding.
    MalformedEncoding,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootFailure {
    pub reason: Failure,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrepareError {
    /// Boot stopped because of the failure carried by `BootFailure`.
    Boot(BootFailure),
    /// No images were supplied to provide the run's origin; boot was not attempted.
    NoImages,
}

/// Boots `os` from x0200, then loads nonempty `images` in order and selects the
/// first origin. Image addresses wrap modulo x10000. Returns a shareable prepared
/// machine, `PrepareError::NoImages` before boot for empty `images`, or
/// `PrepareError::Boot` for a boot cap, an illegal word, or a KBDR read during boot.
/// Boot uses empty console input and a fixed 10,000,000-cycle cap, and contributes
/// no run cycles. Console bytes written during boot are discarded.
pub fn prepare(
    os: &Image,
    images: &[Image],
    mem_latency: NonZeroU32,
) -> Result<Prepared, PrepareError> {
    let first = images.first().ok_or(PrepareError::NoImages)?;
    let (boot_snapshot, _) = State::zeroed()
        .load_image(os)
        .start_at(0x0200)
        .execute(mem_latency, BOOT_CYCLE_CAP, &[])
        .map_err(|reason| PrepareError::Boot(BootFailure { reason }))?;
    let snapshot = images
        .iter()
        .fold(boot_snapshot, State::load_image)
        .start_at(first.origin());
    Ok(Prepared {
        snapshot,
        mem_latency,
    })
}

/// Runs `case` on a private copy of `prepared`, returning final state on halt or
/// `Outcome::Failed` with `CycleCapReached`, `IllegalInstruction`, or
/// `InputExhausted`. KBDR reads consume `case.input` bytes, including instruction
/// fetches from KBDR; an exhausted read reports the fetching/reading instruction's
/// unincremented PC. Counts include the halting instruction and exclude boot.
/// Each successful KBDR read stores its zero-extended byte in backing memory.
/// DDR reads return x0000, including instruction fetches.
/// On halt, console bytes split into program output and halt output at the first
/// executed word xF025, whether fetched from an image or the OS.
/// Illegal words fail before execution at their unincremented PC with zero cycle
/// cost; a zero cycle cap fails before fetching any word.
/// Case pokes apply before triggers, after loading; neither applies during boot.
/// Triggers fire once before fetch and cycle-cap checks, including at the origin.
/// Runs without triggers use the plain execution loop.
pub fn run(prepared: &Prepared, case: &Case) -> Outcome {
    let state = prepared.snapshot.clone().apply_pokes(&case.pokes);
    let result = if case.triggers.is_empty() {
        state.execute(prepared.mem_latency, case.cycle_cap, &case.input)
    } else {
        state.execute_with_triggers(
            prepared.mem_latency,
            case.cycle_cap,
            &case.input,
            &case.triggers,
        )
    };
    match result {
        Ok((state, cycles)) => Outcome::Halted(Halted {
            registers: state.registers,
            memory: state.memory,
            program_output: state.program_output,
            halt_output: state.halt_output,
            cycles,
            mem_latency: prepared.mem_latency,
        }),
        Err(reason) => Outcome::Failed(reason),
    }
}
