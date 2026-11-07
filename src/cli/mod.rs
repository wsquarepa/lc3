//! Process-level orchestration: reads files and stdin, runs the library and
//! writes output. Everything with real logic lives in the private submodules.

mod arguments;
mod exit;
mod help;
mod literal;
#[path = "../literal_text.rs"]
mod literal_text;
mod render;
mod run_file;
mod sha256;
mod sources;

use arguments::{Entry, Invocation};
use exit::{Exit, Finish, UsageError};
use lc3::{Case, PrepareError, WordCounts, prepare, run};
use render::Report;
use run_file::{ConsoleInput, RunSpec};
use std::ffi::OsString;
use std::fs;
use std::io::{self, Read, Write};
use std::process::ExitCode;

const STDIN_NAME: &str = "<stdin>";

/// Runs the command for `arguments` (after the program name) and returns its exit code.
pub fn main(arguments: &[OsString]) -> ExitCode {
    match execute(arguments) {
        Ok(exit) => exit.into(),
        Err(error) => {
            // A closed stderr leaves no channel on which to report anything further.
            let _ = writeln!(io::stderr().lock(), "lc3: {error}");
            Exit::Usage.into()
        }
    }
}

fn execute(arguments: &[OsString]) -> Result<Exit, UsageError> {
    match arguments::parse(arguments)? {
        Invocation::Help => {
            write_stdout(help::HELP.as_bytes())?;
            Ok(Exit::Halted)
        }
        Invocation::Run { entry, json } => run_entry(entry, json),
    }
}

fn run_entry(entry: Entry, json: bool) -> Result<Exit, UsageError> {
    let spec = read_spec(entry)?;
    let Some(sources) = sources::load_all(&spec, &mut io::stderr().lock())? else {
        return Ok(Exit::Source);
    };
    let input = read_console_input(&spec.input)?;
    let word_counts = WordCounts {
        images: sources.images.iter().map(lc3::Image::word_count).collect(),
        os: sources.os.word_count(),
    };
    let finish = match prepare(&sources.os, &sources.images, spec.mem_latency) {
        Ok(prepared) => Finish::Ran(run(
            &prepared,
            &Case {
                input,
                cycle_cap: spec.cycle_cap,
                pokes: spec.pokes,
                triggers: spec.triggers,
            },
        )),
        Err(PrepareError::Boot(boot)) => Finish::BootFailed(boot.reason),
        Err(PrepareError::NoImages) => {
            return Err(UsageError(
                "images must name at least one source".to_owned(),
            ));
        }
        Err(PrepareError::TooManyImages { count, max }) => {
            return Err(UsageError(format!(
                "images lists {count} sources but at most {max} are supported"
            )));
        }
    };
    let image_paths: Vec<String> = spec
        .images
        .iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect();
    let report = Report {
        finish: &finish,
        image_paths: &image_paths,
        word_counts: &word_counts,
        mem_latency: spec.mem_latency.get(),
        os: &sources.os_identity,
        dumps: &spec.dumps,
    };
    emit(&report, json)?;
    Ok(finish.exit())
}

fn emit(report: &Report, json: bool) -> Result<(), UsageError> {
    if json {
        let document = render::json(report)
            .map_err(|error| UsageError(format!("cannot serialize the JSON report: {error}")))?;
        write_stdout(document.as_bytes())
    } else {
        write_stdout(&render::console_bytes(report.finish))?;
        io::stderr()
            .lock()
            .write_all(render::text_summary(report).as_bytes())
            .map_err(|error| UsageError(format!("cannot write the summary to stderr: {error}")))
    }
}

fn write_stdout(bytes: &[u8]) -> Result<(), UsageError> {
    let mut stdout = io::stdout().lock();
    stdout
        .write_all(bytes)
        .and_then(|()| stdout.flush())
        .map_err(|error| UsageError(format!("cannot write to stdout: {error}")))
}

fn read_spec(entry: Entry) -> Result<RunSpec, UsageError> {
    match entry {
        Entry::BareSource(path) => Ok(RunSpec::bare_source(path)),
        Entry::RunFile(path) => {
            let bytes = fs::read(&path).map_err(|error| {
                UsageError(format!("cannot read run file {}: {error}", path.display()))
            })?;
            parse_run_file(bytes, &path.display().to_string(), ConsoleInput::Stdin)
        }
        Entry::RunFileOnStdin => parse_run_file(read_stdin()?, STDIN_NAME, ConsoleInput::Empty),
    }
}

fn parse_run_file(
    bytes: Vec<u8>,
    name: &str,
    unspecified_input: ConsoleInput,
) -> Result<RunSpec, UsageError> {
    let text = String::from_utf8(bytes)
        .map_err(|error| UsageError(format!("{name}: run file is not UTF-8: {error}")))?;
    run_file::parse(&text, name, unspecified_input)
}

fn read_console_input(input: &ConsoleInput) -> Result<Vec<u8>, UsageError> {
    match input {
        ConsoleInput::Bytes(bytes) => Ok(bytes.clone()),
        ConsoleInput::File(path) => fs::read(path).map_err(|error| {
            UsageError(format!(
                "cannot read {} (from input_file): {error}",
                path.display()
            ))
        }),
        ConsoleInput::Stdin => read_stdin(),
        ConsoleInput::Empty => Ok(Vec::new()),
    }
}

fn read_stdin() -> Result<Vec<u8>, UsageError> {
    let mut bytes = Vec::new();
    io::stdin()
        .lock()
        .read_to_end(&mut bytes)
        .map_err(|error| UsageError(format!("cannot read stdin: {error}")))?;
    Ok(bytes)
}
