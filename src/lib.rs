mod core;
mod machine;
mod source;

pub use machine::{
    BootFailure, Case, Failure, Halted, IllegalKind, Outcome, Prepared, Registers, prepare, run,
};
pub use source::{Diagnostic, Format, Image, Loaded, Severity, load};
