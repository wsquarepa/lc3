//! Command-line argument parsing; performs no I/O.

use super::exit::UsageError;
use super::help::HINT;
use super::sources::{SOURCE_EXTENSIONS, format_of, has_suffix};
use std::ffi::OsString;
use std::path::PathBuf;

const RUN_FILE_SUFFIX: &str = ".toml";
const STDIN_PATH: &str = "-";
const HELP_FLAG: &str = "--help";
const JSON_FLAG: &str = "--json";

/// What the one path argument names, decided by its spelling alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entry {
    /// A run file read from stdin.
    RunFileOnStdin,
    /// A path ending in `.toml`.
    RunFile(PathBuf),
    /// A path with one of the four source extensions, run with the default case.
    BareSource(PathBuf),
}

/// A parsed command line: the only two things `lc3` can be asked to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Invocation {
    /// `--help` given alone.
    Help,
    /// Run `entry`; `json` selects the JSON report over console bytes plus a stderr summary.
    Run { entry: Entry, json: bool },
}

/// Parses the arguments after the program name. `--help` is only valid alone.
/// Errors: unknown flags, repeated `--json`, a missing or second path, and a
/// path whose extension is neither `.toml` nor one of the four source extensions.
pub fn parse(arguments: &[OsString]) -> Result<Invocation, UsageError> {
    if arguments == [OsString::from(HELP_FLAG)] {
        return Ok(Invocation::Help);
    }
    let mut path: Option<&OsString> = None;
    let mut json = false;
    for argument in arguments {
        if argument == JSON_FLAG {
            if json {
                return Err(usage(format!("{JSON_FLAG} given twice")));
            }
            json = true;
        } else if argument != STDIN_PATH && argument.as_encoded_bytes().starts_with(b"-") {
            return Err(usage(format!("unknown flag {argument:?}")));
        } else if let Some(first) = path {
            return Err(usage(format!(
                "unexpected second path {argument:?} after {first:?}; give exactly one"
            )));
        } else {
            path = Some(argument);
        }
    }
    let path = path.ok_or_else(|| usage("missing path".to_owned()))?;
    Ok(Invocation::Run {
        entry: classify(PathBuf::from(path))?,
        json,
    })
}

fn classify(path: PathBuf) -> Result<Entry, UsageError> {
    if path.as_os_str() == STDIN_PATH {
        Ok(Entry::RunFileOnStdin)
    } else if has_suffix(&path, RUN_FILE_SUFFIX) {
        Ok(Entry::RunFile(path))
    } else if format_of(&path).is_some() {
        Ok(Entry::BareSource(path))
    } else {
        Err(usage(format!(
            "{}: unsupported extension; expected one of {SOURCE_EXTENSIONS} (sources) or .toml (run file)",
            path.display()
        )))
    }
}

fn usage(problem: String) -> UsageError {
    UsageError(format!("{problem}\n{HINT}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    fn run(entry: Entry, json: bool) -> Result<Invocation, UsageError> {
        Ok(Invocation::Run { entry, json })
    }

    #[test]
    fn json_may_precede_or_follow_the_path() {
        let expected = run(Entry::BareSource("a.asm".into()), true);
        assert_eq!(parse(&args(&["--json", "a.asm"])), expected);
        assert_eq!(parse(&args(&["a.asm", "--json"])), expected);
        assert_eq!(
            parse(&args(&["a.asm"])),
            run(Entry::BareSource("a.asm".into()), false)
        );
    }

    #[test]
    fn paths_are_classified_by_extension() {
        for name in ["r.toml", "r.TOML"] {
            assert_eq!(
                parse(&args(&[name])),
                run(Entry::RunFile(name.into()), false)
            );
        }
        assert_eq!(parse(&args(&["-"])), run(Entry::RunFileOnStdin, false));
        assert_eq!(
            parse(&args(&["--json", "-"])),
            run(Entry::RunFileOnStdin, true)
        );
        for name in ["a.bin", "a.hex", "a.obj", "a.ASM", "a.Hex"] {
            assert_eq!(
                parse(&args(&[name])),
                run(Entry::BareSource(name.into()), false)
            );
        }
    }

    #[test]
    fn help_is_valid_only_alone() {
        assert_eq!(parse(&args(&["--help"])), Ok(Invocation::Help));
        assert!(parse(&args(&["--help", "a.asm"])).is_err());
    }

    #[test]
    fn bad_arguments_are_usage_errors_with_the_hint() {
        for bad in [
            vec![],
            vec!["--json"],
            vec!["a.asm", "b.asm"],
            vec!["--wat"],
            vec!["--json", "--json", "a.asm"],
            vec!["a.txt"],
        ] {
            let error = parse(&args(&bad)).unwrap_err();
            assert!(error.0.contains(HINT), "{bad:?}: {error}");
        }
        let message = parse(&args(&["a.txt"])).unwrap_err().0;
        assert!(message.contains(SOURCE_EXTENSIONS) && message.contains(".toml"));
    }
}
