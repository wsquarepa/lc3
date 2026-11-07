use crate::core::{MEMORY_WORDS, State};
use crate::{Image, bundled_os};
use std::num::NonZeroU32;

const BOOT_CYCLE_CAP: u64 = 10_000_000;
const BOOT_MEM_LATENCY: NonZeroU32 = NonZeroU32::MIN;
const MAX_IMAGES: usize = u16::MAX as usize - 1;

/// An immutable boot snapshot with images loaded, ready for independent runs.
/// The snapshot records whether the OS origin and words match the bundled OS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prepared {
    snapshot: State,
    mem_latency: NonZeroU32,
    // A u16 fetch index is always in bounds: 14.7 -> 14.4 us/case on core 0
    // of a Ryzen AI 9 365 in release mode, with the same 128 KiB owner table.
    owners: Box<[u16; MEMORY_WORDS]>,
    word_counts: WordCounts,
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

impl Register {
    /// Returns the position of a general-purpose register in `Registers::r`
    /// (0 for R0 up to 7 for R7), or `None` for `Pc` and `Psr`.
    pub fn index(&self) -> Option<usize> {
        match self {
            Self::R0 => Some(0),
            Self::R1 => Some(1),
            Self::R2 => Some(2),
            Self::R3 => Some(3),
            Self::R4 => Some(4),
            Self::R5 => Some(5),
            Self::R6 => Some(6),
            Self::R7 => Some(7),
            Self::Pc | Self::Psr => None,
        }
    }
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

/// The result of one run: it halted, or it failed for exactly one reason.
/// Returned by [`run`]; a failed boot is reported by [`prepare`] instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Halted(Halted),
    Failed(Failure),
}

/// Instruction cycles assigned by fetch address ownership as loaded at preparation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attribution {
    /// Cycles fetched from addresses owned by each image, in loading order.
    pub images: Vec<u64>,
    /// Cycles fetched from addresses owned by the OS after image overlays.
    pub os: u64,
    /// Cycles fetched from addresses loaded by neither the OS nor any image.
    pub unattributed: u64,
}

/// Source word counts, including reserved space and independent of load overlap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WordCounts {
    /// Each image's word count, in loading order and excluding its origin word.
    pub images: Vec<usize>,
    /// The OS word count, excluding its origin word.
    pub os: usize,
}

/// The final state of a run stopped by a write to the machine control register
/// that clears its clock-enable bit (bit 15).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Halted {
    /// Register file as it stood after the halting instruction completed.
    pub registers: Registers,
    /// All 65,536 backing words, with the machine control register reading x8000.
    /// A KBDR read stores its zero-extended input byte; pokes may store any word.
    /// Console device writes and reads of KBSR, DSR and DDR leave their backing
    /// words unchanged; case and trigger pokes write them directly.
    pub memory: Box<[u16]>,
    /// Console bytes written before the first executed TRAP x25 (word xF025)
    /// or first instruction fetch at the halt-routine address, whichever comes
    /// first. The address is the word at x0025 after case pokes, before triggers
    /// and the first instruction, and remains fixed for the run.
    pub program_output: Vec<u8>,
    /// Console bytes written from the first executed TRAP x25 (word xF025)
    /// or first instruction fetch at the halt-routine address onward, whichever
    /// comes first. The address is the word at x0025 after case pokes, before
    /// triggers and the first instruction, and remains fixed for the run.
    /// A direct JSR into that address also starts halt output there. Empty when
    /// the run writes the machine control register without either event.
    pub halt_output: Vec<u8>,
    /// Total FSM cycles of the run, the halting instruction included and boot excluded.
    pub cycles: u64,
    /// Full instruction costs by fetch address owner, summing to `cycles`.
    pub attribution: Attribution,
    /// Source word counts for all images and the OS, independent of overlap.
    pub word_counts: WordCounts,
    /// The memory latency the run was prepared with.
    pub mem_latency: NonZeroU32,
}

/// The LC-3 register file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registers {
    /// General-purpose registers R0 to R7, indexed by register number.
    pub r: [u16; 8],
    /// Address from which the next instruction is fetched.
    pub pc: u16,
    /// Processor status. Only the condition codes are maintained by execution:
    /// N is bit 2, Z is bit 1 and P is bit 0, exactly one of them set after an
    /// instruction that writes a register; the other bits are never changed by
    /// execution and pokes may store any value.
    pub psr: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// KBDR was read after all console input bytes were consumed.
    InputExhausted {
        /// Address of the reading instruction, including instruction fetch.
        pc: u16,
    },
    /// The next instruction's cost would have made the run's total reach or
    /// exceed `Case::cycle_cap`; that instruction is not executed or counted.
    CycleCapReached,
    /// A TRAP under the bundled OS read xFFFE from its vector-table entry.
    /// The failing TRAP's cost is checked against the cap but is not counted.
    UnknownTrap {
        /// Low byte of the executing TRAP instruction.
        vector: u8,
    },
    /// The word fetched at `pc` cannot execute; no cycles are charged for it.
    IllegalInstruction {
        /// Address the word was fetched from.
        pc: u16,
        /// The fetched word.
        word: u16,
        /// Why the word cannot execute.
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

/// A failure of the OS's boot, carried by [`PrepareError::Boot`]. No run was started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootFailure {
    /// Why boot stopped: the boot cycle cap, an illegal word, or a KBDR read
    /// (boot has no console input).
    pub reason: Failure,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrepareError {
    /// Boot stopped because of the failure carried by `BootFailure`.
    Boot(BootFailure),
    /// No images were supplied to provide the run's origin; boot was not attempted.
    NoImages,
    /// Image count exceeds the u16 owner table's capacity; boot was not attempted.
    TooManyImages {
        /// Number of supplied images.
        count: usize,
        /// Largest supported image count, reserving owners for OS and unattributed.
        max: usize,
    },
}

/// Boots `os` from x0200, then loads nonempty `images` in order and selects the
/// first origin. Image addresses wrap modulo x10000. Returns a shareable prepared
/// machine, `PrepareError::NoImages` before boot for empty `images`, or
/// `PrepareError::Boot` for a boot cap, an illegal word, or a KBDR read during boot.
/// More than 65,534 images returns `PrepareError::TooManyImages` before boot.
/// Boot uses empty console input and a fixed 10,000,000-cycle cap measured at
/// memory latency 1 whatever `mem_latency` is, so the cap bounds the same amount
/// of boot work at every latency; it contributes no run cycles. Console bytes
/// written during boot are discarded.
/// Ownership is fixed by load order, including MCR and wrapping addresses; later
/// images override earlier images and the OS. Boot writes do not change ownership.
pub fn prepare(
    os: &Image,
    images: &[Image],
    mem_latency: NonZeroU32,
) -> Result<Prepared, PrepareError> {
    let first = images.first().ok_or(PrepareError::NoImages)?;
    if images.len() > MAX_IMAGES {
        return Err(PrepareError::TooManyImages {
            count: images.len(),
            max: MAX_IMAGES,
        });
    }
    let os_owner = images.len() as u16;
    let owners = load_ownership(Box::new([os_owner + 1; MEMORY_WORDS]), os, os_owner);
    let owners = images
        .iter()
        .zip(0u16..)
        .fold(owners, |owners, (image, owner)| {
            load_ownership(owners, image, owner)
        });
    let word_counts = WordCounts {
        images: images.iter().map(Image::word_count).collect(),
        os: os.word_count(),
    };
    let (boot_snapshot, _, _) = State::zeroed(os == bundled_os())
        .load_image(os)
        .start_at(0x0200)
        .execute::<false>(
            BOOT_MEM_LATENCY,
            BOOT_CYCLE_CAP,
            &[],
            &[],
            &owners,
            images.len() + 2,
        )
        .map_err(|reason| PrepareError::Boot(BootFailure { reason }))?;
    let snapshot = images
        .iter()
        .fold(boot_snapshot, State::load_image)
        .start_at(first.origin());
    Ok(Prepared {
        snapshot,
        mem_latency,
        owners,
        word_counts,
    })
}

/// Assigns an image's wrapping load addresses to `owner`, returning the owner table.
fn load_ownership(
    mut owners: Box<[u16; MEMORY_WORDS]>,
    image: &Image,
    owner: u16,
) -> Box<[u16; MEMORY_WORDS]> {
    let mut address = image.origin();
    for _ in image.words() {
        owners[usize::from(address)] = owner;
        address = address.wrapping_add(1);
    }
    owners
}

/// Runs `case` on a private copy of `prepared`, returning final state on halt or
/// `Outcome::Failed` with `CycleCapReached`, `IllegalInstruction`, `InputExhausted`,
/// or `UnknownTrap`. KBDR reads consume `case.input` bytes, including instruction
/// fetches from KBDR; an exhausted read reports the fetching/reading instruction's
/// unincremented PC. Counts include the halting instruction and exclude boot.
/// Each successful KBDR read stores its zero-extended byte in backing memory.
/// DDR reads return x0000, including instruction fetches.
/// On halt, console bytes split at the first executed word xF025 or first fetch
/// at the halt-routine address, whichever comes first, whether in an image or
/// the OS. The address is the word at x0025 after case pokes and before triggers
/// or the first instruction, and remains fixed for the run.
/// Illegal words fail before execution at their unincremented PC with zero cycle
/// cost; a zero cycle cap fails before fetching any word.
/// Case pokes apply before triggers, after loading; neither applies during boot.
/// Triggers fire once before fetch and cycle-cap checks, including at the origin.
/// Ownership and source word counts remain fixed across pokes and program stores.
/// Runs without triggers omit trigger checks from the shared execution loop.
/// Only an OS matching the bundled image's origin and words enables UnknownTrap:
/// after checking the TRAP's cost against the cap, a vector-table entry of xFFFE
/// fails without loading PC or counting that TRAP's cycles.
pub fn run(prepared: &Prepared, case: &Case) -> Outcome {
    let state = prepared.snapshot.clone().apply_pokes(&case.pokes);
    let image_count = prepared.word_counts.images.len();
    let result = if case.triggers.is_empty() {
        state.execute::<false>(
            prepared.mem_latency,
            case.cycle_cap,
            &case.input,
            &case.triggers,
            &prepared.owners,
            image_count + 2,
        )
    } else {
        state.execute::<true>(
            prepared.mem_latency,
            case.cycle_cap,
            &case.input,
            &case.triggers,
            &prepared.owners,
            image_count + 2,
        )
    };
    match result {
        Ok((state, cycles, mut buckets)) => {
            let os = buckets[image_count];
            let unattributed = buckets[image_count + 1];
            buckets.truncate(image_count);
            Outcome::Halted(Halted {
                registers: state.registers,
                memory: state.memory,
                program_output: state.program_output,
                halt_output: state.halt_output,
                cycles,
                attribution: Attribution {
                    images: buckets,
                    os,
                    unattributed,
                },
                word_counts: prepared.word_counts.clone(),
                mem_latency: prepared.mem_latency,
            })
        }
        Err(reason) => Outcome::Failed(reason),
    }
}
