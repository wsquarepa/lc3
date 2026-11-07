use std::sync::OnceLock;

mod assembler;
mod convert;
mod core;
mod literal_text;
mod machine;
mod source;

pub use machine::{
    Attribution, BootFailure, Case, Failure, Halted, IllegalKind, MemoryPoke, Outcome, Pokes,
    PrepareError, Prepared, Register, Registers, Trigger, WordCounts, prepare, run,
};
pub use source::{Diagnostic, Format, Image, Loaded, Severity, load};

/// UTF-8 assembly source for this project's cycle-optimized bundled OS.
pub const BUNDLED_OS_SOURCE: &str = include_str!("bundled_os.asm");

/// Returns the bundled OS image, assembled once per process and shared thereafter.
/// Panics with the first diagnostic if the embedded source cannot assemble;
/// diagnostic-free assembly is a tested invariant of the crate's own source.
pub fn bundled_os() -> &'static Image {
    static IMAGE: OnceLock<Image> = OnceLock::new();
    IMAGE.get_or_init(|| {
        load(Format::Asm, BUNDLED_OS_SOURCE.as_bytes())
            .map_err(|diagnostics| diagnostics.into_iter().next())
            .expect("bundled OS source must assemble; first diagnostic")
            .image
    })
}
