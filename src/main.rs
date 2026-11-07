//! The `lc3` command: runs a run file or a bare source and reports the outcome.
//!
//! Usage: `lc3 <path> [--json]`, `lc3 - [--json]` or `lc3 --help`; `--help`
//! documents the run-file schema. Exit codes:
//!
//! - 0: halted
//! - 1: usage or run-file error (bad arguments, unknown key, malformed value,
//!   unreadable file, `input` with `input_file`, no images, too many images)
//! - 2: a source had an error diagnostic
//! - 3: boot failure
//! - 10: cycle cap reached
//! - 11: illegal instruction
//! - 12: input exhausted
//! - 13: unknown trap
//!
//! `--json` prints one compact JSON object and a newline to stdout.

mod cli;

use std::ffi::OsString;
use std::process::ExitCode;

fn main() -> ExitCode {
    let arguments: Vec<OsString> = std::env::args_os().skip(1).collect();
    cli::main(&arguments)
}
