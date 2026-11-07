mod fixture;

use fixture::{Expected, Fixture, OsSource, parse};
use lc3::{Format, Image, load};
use std::collections::HashSet;
use std::fs;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

fn manifest_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn committed_fixtures() -> PathBuf {
    manifest_dir().join("tests/fixtures/differential")
}

fn reference_os_path() -> PathBuf {
    manifest_dir().join("reference/lc3tools/lc3os.obj")
}

fn local_fixtures() -> PathBuf {
    manifest_dir().join("reference/fixtures")
}

fn skip(what: &str, path: &Path) {
    writeln!(
        std::io::stderr(),
        "skipping: {what} not found at {}",
        path.display()
    )
    .expect("failed to write the skip message to stderr");
}

/// Reads `path`, or writes a visible skip message and returns `None` when it is absent.
fn read_or_skip(path: &Path, what: &str) -> Option<Vec<u8>> {
    match fs::read(path) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == ErrorKind::NotFound => {
            skip(what, path);
            None
        }
        Err(error) => panic!("cannot read {what} at {}: {error}", path.display()),
    }
}

fn load_obj(path: &Path, bytes: &[u8]) -> Image {
    load(Format::Obj, bytes)
        .unwrap_or_else(|diagnostics| {
            panic!("{} is not a valid .obj: {diagnostics:?}", path.display())
        })
        .image
}

fn case_files(directory: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("cannot list {}: {error}", directory.display()))
        .map(|entry| entry.expect("cannot read a directory entry").path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "case")
        })
        .collect();
    files.sort();
    files
}

fn read_fixture(path: &Path) -> Fixture {
    let text = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    parse(path, &text).unwrap_or_else(|error| panic!("{error}"))
}

/// How many fixtures a directory run compared and how many it skipped.
#[derive(Debug, PartialEq, Eq)]
struct Tally {
    compared: usize,
    skipped: usize,
}

/// Compares every fixture in `directory` selected by `select`, panicking with every
/// mismatch found. A fixture whose OS file is absent is skipped with a message.
fn compare_directory(directory: &Path, select: fn(&Expected) -> bool) -> Tally {
    let mut tally = Tally {
        compared: 0,
        skipped: 0,
    };
    let mut failures = Vec::new();
    let mut absent: HashSet<PathBuf> = HashSet::new();
    for path in case_files(directory) {
        let fixture = read_fixture(&path);
        if !select(&fixture.expected) {
            continue;
        }
        let (os_path, what) = match &fixture.os {
            OsSource::Reference => (reference_os_path(), "reference OS"),
            OsSource::File(file) => (directory.join(file), "fixture OS"),
        };
        if absent.contains(&os_path) {
            tally.skipped += 1;
            continue;
        }
        let Some(os_bytes) = read_or_skip(&os_path, what) else {
            absent.insert(os_path);
            tally.skipped += 1;
            continue;
        };
        let image_path = directory.join(&fixture.image);
        let image_bytes = fs::read(&image_path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", image_path.display()));
        let found = fixture::mismatches(
            &fixture,
            &load_obj(&os_path, &os_bytes),
            &load_obj(&image_path, &image_bytes),
        );
        failures.extend(
            found
                .into_iter()
                .map(|line| format!("{}: {line}", path.display())),
        );
        tally.compared += 1;
    }
    assert!(
        failures.is_empty(),
        "differences from lc3sim:\n{}",
        failures.join("\n")
    );
    tally
}

fn is_halted(expected: &Expected) -> bool {
    matches!(expected, Expected::Halted(_))
}

fn is_illegal(expected: &Expected) -> bool {
    matches!(expected, Expected::IllegalInstruction { .. })
}

fn is_exhausted(expected: &Expected) -> bool {
    matches!(expected, Expected::InputExhausted { .. })
}

fn assert_committed_group(select: fn(&Expected) -> bool) {
    let tally = compare_directory(&committed_fixtures(), select);
    assert!(
        tally.compared + tally.skipped > 0,
        "no committed fixtures matched the group"
    );
}

#[test]
fn halting_programs_match_lc3sim_registers_memory_and_output() {
    assert_committed_group(is_halted);
}

#[test]
fn illegal_instructions_stop_where_lc3sim_stops() {
    assert_committed_group(is_illegal);
}

#[test]
fn input_exhaustion_stops_where_lc3sim_stops() {
    assert_committed_group(is_exhausted);
}

#[test]
fn local_fixtures_match_lc3sim() {
    let directory = local_fixtures();
    if !directory.is_dir() {
        skip("local differential fixtures folder", &directory);
        return;
    }
    let tally = compare_directory(&directory, |_| true);
    assert!(
        tally.compared + tally.skipped > 0,
        "the local fixtures folder {} holds no cases",
        directory.display()
    );
}

#[test]
fn committed_sources_load_to_the_reference_images() {
    let directory = committed_fixtures();
    let mut checked = 0;
    for path in case_files(&directory) {
        let fixture = read_fixture(&path);
        let source = directory.join(fixture.image.with_extension(""));
        let (extension, format) = [
            ("asm", Format::Asm),
            ("bin", Format::Bin),
            ("hex", Format::Hex),
        ]
        .into_iter()
        .find(|(extension, _)| source.with_extension(extension).is_file())
        .unwrap_or_else(|| panic!("no source for {}", path.display()));
        let source = source.with_extension(extension);
        let text = fs::read(&source)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", source.display()));
        let loaded = load(format, &text).unwrap_or_else(|diagnostics| {
            panic!("{} does not load: {diagnostics:?}", source.display())
        });
        let object_path = directory.join(&fixture.image);
        let object = load_obj(&object_path, &fs::read(&object_path).unwrap());
        assert_eq!(
            loaded.image,
            object,
            "{} differs from lc3as",
            source.display()
        );
        checked += 1;
    }
    assert!(checked > 0, "no committed fixtures found");
}

#[test]
fn parser_reads_every_directive_and_rejects_malformed_files() {
    let text = "# comment\nos reference\nimage a.obj\ninput 0A41\npoke 2F80 1234\n\
                trigger 1234 4000=BEEF 4001=0001\nhalted\nregisters 0 1 2 3 4 5 6 7 8\n\
                output 4142\nmemory 3000 0001 0002\n";
    let fixture = parse(Path::new("x.case"), text).unwrap();
    assert_eq!(fixture.os, OsSource::Reference);
    assert_eq!(fixture.input, b"\nA");
    assert_eq!(fixture.pokes, [(0x2f80, 0x1234)]);
    assert_eq!(
        fixture.trigger,
        Some((0x1234, vec![(0x4000, 0xbeef), (0x4001, 1)]))
    );
    let Expected::Halted(state) = fixture.expected else {
        panic!("expected a halted fixture");
    };
    assert_eq!(state.registers, [0, 1, 2, 3, 4, 5, 6, 7]);
    assert_eq!(state.pc, 8);
    assert_eq!(state.program_output, b"AB");
    assert_eq!(state.memory_changes, [(0x3000, vec![1, 2])]);

    for bad in [
        "os reference\nimage a.obj\n",
        "os reference\nimage a.obj\nhalted\nillegal 3001 8000\n",
        "os reference\nimage a.obj\nhalted\nregisters 1 2 3\noutput\n",
        "os reference\nimage a.obj\nillegal 3001 xyz\n",
        "os reference\nimage a.obj\ninput abc\nillegal 3001 8000\n",
        "os reference\nimage a.obj\nfrobnicate\nillegal 3001 8000\n",
    ] {
        assert!(
            parse(Path::new("bad.case"), bad).is_err(),
            "accepted {bad:?}"
        );
    }
}
