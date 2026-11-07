//! Process exit codes, the error type for exit code 1, and the finished-run value.

use lc3::{Failure, Outcome};
use std::fmt;
use std::process::ExitCode;

/// Distinct exit codes of the `lc3` command; documented in `--help`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Exit {
    Halted = 0,
    Usage = 1,
    Source = 2,
    Boot = 3,
    CycleCapReached = 10,
    IllegalInstruction = 11,
    InputExhausted = 12,
    UnknownTrap = 13,
}

impl From<Exit> for ExitCode {
    fn from(exit: Exit) -> Self {
        ExitCode::from(exit as u8)
    }
}

/// A bad argument, a malformed run file, an unreadable or unwritable file, or an
/// invalid image count. Always exits with [`Exit::Usage`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageError(pub String);

impl fmt::Display for UsageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// How a prepared run ended: a library outcome, or a failure while booting the OS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Finish {
    Ran(Outcome),
    BootFailed(Failure),
}

impl Finish {
    /// The process exit code for this finish: 0 for a halt, 3 for any boot
    /// failure, and a code per run failure reason (10 to 13).
    pub fn exit(&self) -> Exit {
        match self {
            Finish::Ran(Outcome::Halted(_)) => Exit::Halted,
            Finish::Ran(Outcome::Failed(failure)) => failure_exit(failure),
            Finish::BootFailed(_) => Exit::Boot,
        }
    }
}

fn failure_exit(failure: &Failure) -> Exit {
    match failure {
        Failure::CycleCapReached => Exit::CycleCapReached,
        Failure::IllegalInstruction { .. } => Exit::IllegalInstruction,
        Failure::InputExhausted { .. } => Exit::InputExhausted,
        Failure::UnknownTrap { .. } => Exit::UnknownTrap,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lc3::IllegalKind;

    #[test]
    fn every_failure_reason_has_its_own_code() {
        let failures = [
            (Failure::CycleCapReached, 10),
            (
                Failure::IllegalInstruction {
                    pc: 0,
                    word: 0,
                    kind: IllegalKind::Rti,
                },
                11,
            ),
            (Failure::InputExhausted { pc: 0 }, 12),
            (Failure::UnknownTrap { vector: 0 }, 13),
        ];
        for (failure, code) in failures {
            assert_eq!(
                Finish::Ran(Outcome::Failed(failure.clone())).exit() as u8,
                code
            );
            assert_eq!(Finish::BootFailed(failure).exit() as u8, 3);
        }
    }
}
