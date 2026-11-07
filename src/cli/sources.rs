//! Reading and loading the OS and image files named by a run, with diagnostics.

use super::exit::UsageError;
use super::run_file::RunSpec;
use super::sha256::sha256_hex;
use lc3::{Diagnostic, Format, Image, Severity, bundled_os, load};
use serde::Serialize;
use std::fs;
use std::io::Write;
use std::path::Path;

pub const SOURCE_EXTENSIONS: &str = ".asm, .bin, .hex, .obj";

/// Which OS a run used, as reported in output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OsIdentity {
    Bundled,
    /// `path` is as given; `sha256` is the lowercase hex digest of the file's bytes.
    File {
        path: String,
        sha256: String,
    },
}

#[derive(Debug)]
pub struct LoadedSources {
    pub os: Image,
    pub os_identity: OsIdentity,
    pub images: Vec<Image>,
}

/// Reports whether `path` ends in `suffix` (a dot plus the extension, in
/// lowercase), ignoring ASCII case.
pub fn has_suffix(path: &Path, suffix: &str) -> bool {
    let name = path.as_os_str().as_encoded_bytes();
    name.len() >= suffix.len()
        && name[name.len() - suffix.len()..].eq_ignore_ascii_case(suffix.as_bytes())
}

/// Chooses the source format from the extension at the end of `path`, ignoring ASCII case.
pub fn format_of(path: &Path) -> Option<Format> {
    [
        (".asm", Format::Asm),
        (".bin", Format::Bin),
        (".hex", Format::Hex),
        (".obj", Format::Obj),
    ]
    .into_iter()
    .find(|(suffix, _)| has_suffix(path, suffix))
    .map(|(_, format)| format)
}

/// Formats as `<path>:<line>: <severity>: <message>`, omitting `:<line>` without a line.
pub fn diagnostic_line(path: &Path, diagnostic: &Diagnostic) -> String {
    let line = diagnostic
        .line
        .map_or_else(String::new, |line| format!(":{line}"));
    let severity = match diagnostic.severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
    };
    format!(
        "{}{line}: {severity}: {}",
        path.display(),
        diagnostic.message
    )
}

/// Loads the OS, then every image in order, writing all diagnostics of every
/// source to `diagnostics`. Returns `None` when any source had an error, and
/// an error when a file is unreadable or has an unsupported extension.
pub fn load_all(
    spec: &RunSpec,
    diagnostics: &mut dyn Write,
) -> Result<Option<LoadedSources>, UsageError> {
    let (os, os_identity) = match &spec.os {
        None => (Some(bundled_os().clone()), OsIdentity::Bundled),
        Some(path) => {
            let format = require_format("os", path)?;
            let bytes = read_source("os", path)?;
            let identity = OsIdentity::File {
                path: path.to_string_lossy().into_owned(),
                sha256: sha256_hex(&bytes),
            };
            (load_source(format, path, &bytes, diagnostics)?, identity)
        }
    };
    let images = spec
        .images
        .iter()
        .enumerate()
        .map(|(index, path)| {
            let key = format!("images[{index}]");
            let format = require_format(&key, path)?;
            let bytes = read_source(&key, path)?;
            load_source(format, path, &bytes, diagnostics)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(os
        .zip(images.into_iter().collect::<Option<Vec<_>>>())
        .map(|(os, images)| LoadedSources {
            os,
            os_identity,
            images,
        }))
}

fn read_source(key: &str, path: &Path) -> Result<Vec<u8>, UsageError> {
    fs::read(path).map_err(|error| {
        UsageError(format!(
            "cannot read {} (from {key}): {error}",
            path.display()
        ))
    })
}

fn require_format(key: &str, path: &Path) -> Result<Format, UsageError> {
    format_of(path).ok_or_else(|| {
        UsageError(format!(
            "{} (from {key}): unsupported extension; expected one of {SOURCE_EXTENSIONS}",
            path.display()
        ))
    })
}

fn load_source(
    format: Format,
    path: &Path,
    bytes: &[u8],
    diagnostics: &mut dyn Write,
) -> Result<Option<Image>, UsageError> {
    let (image, reported) = match load(format, bytes) {
        Ok(loaded) => (Some(loaded.image), loaded.warnings),
        Err(reported) => (None, reported),
    };
    for diagnostic in &reported {
        writeln!(diagnostics, "{}", diagnostic_line(path, diagnostic))
            .map_err(|error| UsageError(format!("cannot write diagnostics to stderr: {error}")))?;
    }
    Ok(image)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn format_follows_the_suffix_ignoring_case() {
        assert_eq!(format_of(Path::new("a.asm")), Some(Format::Asm));
        assert_eq!(format_of(Path::new("d/a.bin")), Some(Format::Bin));
        assert_eq!(format_of(Path::new("a.hex")), Some(Format::Hex));
        assert_eq!(format_of(Path::new("a.obj")), Some(Format::Obj));
        assert_eq!(format_of(Path::new(".asm")), Some(Format::Asm));
        assert_eq!(format_of(Path::new("a.ASM")), Some(Format::Asm));
        assert_eq!(format_of(Path::new("a.ObJ")), Some(Format::Obj));
        assert_eq!(format_of(Path::new("a.asm.bak")), None);
        assert_eq!(format_of(Path::new("a")), None);
    }

    #[test]
    fn diagnostics_carry_file_line_and_severity() {
        let path = PathBuf::from("dir/x.asm");
        let with_line = Diagnostic {
            severity: Severity::Error,
            line: Some(7),
            message: "bad".to_owned(),
        };
        let without_line = Diagnostic {
            severity: Severity::Warning,
            line: None,
            message: "odd".to_owned(),
        };
        assert_eq!(
            diagnostic_line(&path, &with_line),
            "dir/x.asm:7: error: bad"
        );
        assert_eq!(
            diagnostic_line(&path, &without_line),
            "dir/x.asm: warning: odd"
        );
    }
}
