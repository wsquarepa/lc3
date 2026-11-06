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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Case {
    /// A run fails before executing an instruction whose cost would make the
    /// running total reach or exceed this cap, including a halting instruction.
    /// Zero permits no instructions to execute.
    pub cycle_cap: u64,
}

impl Default for Case {
    fn default() -> Self {
        Self {
            cycle_cap: 10_000_000,
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
    /// All 65,536 words, with the machine control register reading x8000.
    pub memory: Box<[u16]>,
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
    CycleCapReached,
    IllegalInstruction {
        pc: u16,
        word: u16,
        kind: IllegalKind,
    },
    /// Preparation requires at least one image to supply the run's origin.
    MissingImages,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IllegalKind {
    Rti,
    Reserved,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootFailure {
    pub reason: Failure,
}

/// Boots `os` from x0200, then loads nonempty `images` in order and selects the
/// first origin. Image addresses wrap modulo x10000. Returns a shareable prepared
/// machine, or `BootFailure` for missing images, a boot cap, or an illegal word.
/// Boot uses a fixed 10,000,000-cycle cap and contributes no run cycles.
pub fn prepare(
    os: &Image,
    images: &[Image],
    mem_latency: NonZeroU32,
) -> Result<Prepared, BootFailure> {
    let first = images.first().ok_or(BootFailure {
        reason: Failure::MissingImages,
    })?;
    let (boot_snapshot, _) = State::zeroed()
        .load_image(os)
        .start_at(0x0200)
        .execute(mem_latency, BOOT_CYCLE_CAP)
        .map_err(|reason| BootFailure { reason })?;
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
/// `Outcome::Failed` with `CycleCapReached` or `IllegalInstruction`. Counts
/// include the halting instruction and exclude boot.
pub fn run(prepared: &Prepared, case: &Case) -> Outcome {
    match prepared
        .snapshot
        .clone()
        .execute(prepared.mem_latency, case.cycle_cap)
    {
        Ok((state, cycles)) => Outcome::Halted(Halted {
            registers: state.registers,
            memory: state.memory,
            cycles,
            mem_latency: prepared.mem_latency,
        }),
        Err(reason) => Outcome::Failed(reason),
    }
}
