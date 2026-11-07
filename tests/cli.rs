//! Process-level tests of the `lc3` binary. Every test runs in its own temporary
//! directory, which is also the working directory of the child process.

use serde_json::{Value, json};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

const HALT_ASM: &str = ".ORIG x3000\nSTI R0,PTR\nPTR .FILL xFFFE\n.END\n";
const ECHO_ASM: &str = ".ORIG x3000\nGETC\nOUT\nHALT\n.END\n";
const TINY_OS_ASM: &str = ".ORIG x0200\nSTI R0,PTR\nPTR .FILL xFFFE\n.END\n";
const TINY_OS_SHA256: &str = "209ee3f8453f2154b4b9a8adf2d23f89530a3d3b9caf8106bdfec487d51ddc76";
const HALT_MESSAGE: &[u8] = b"\n--- halted ---\n";
const REGISTER_NAMES: [&str; 10] = ["R0", "R1", "R2", "R3", "R4", "R5", "R6", "R7", "PC", "PSR"];

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct Sandbox {
    root: PathBuf,
}

impl Sandbox {
    fn new() -> Self {
        let id = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!("lc3-cli-{}-{id}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        Self { root }
    }

    fn write(&self, relative: &str, bytes: impl AsRef<[u8]>) {
        let path = self.root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }

    fn lc3(&self, args: &[&str], stdin: &[u8]) -> Output {
        let mut child = Command::new(env!("CARGO_BIN_EXE_lc3"))
            .current_dir(&self.root)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        // The child may exit without reading stdin, which closes the pipe early.
        let _ = child.stdin.take().unwrap().write_all(stdin);
        child.wait_with_output().unwrap()
    }

    fn json(&self, args: &[&str], stdin: &[u8]) -> (Output, Value) {
        let output = self.lc3(args, stdin);
        let value = serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
            panic!(
                "stdout is not exactly one JSON object ({error}): {}\nstderr: {}",
                String::from_utf8_lossy(&output.stdout),
                stderr(&output)
            )
        });
        (output, value)
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn code(output: &Output) -> i32 {
    output
        .status
        .code()
        .expect("process was killed by a signal")
}

fn assert_usage_error(output: &Output, expected: &[&str]) {
    let message = stderr(output);
    assert_eq!(code(output), 1, "{message}");
    assert!(output.stdout.is_empty(), "stdout: {:?}", output.stdout);
    for fragment in expected {
        assert!(
            message.contains(fragment),
            "missing {fragment:?} in {message}"
        );
    }
}

fn assert_run_file_error(run_file: &str, expected: &[&str]) {
    let sandbox = Sandbox::new();
    sandbox.write("halt.asm", HALT_ASM);
    sandbox.write("run.toml", run_file);
    let output = sandbox.lc3(&["run.toml"], b"");
    assert_usage_error(&output, expected);
}

const FULL_PROGRAM: &str = "\
.ORIG x3000
LD R1, VAL
ADD R2, R0, R1
ADD R4, R3, #1
LDI R5, DATAPTR
STI R0, MCRPTR
VAL .FILL #0
DATAPTR .FILL x4000
MCRPTR .FILL xFFFE
.END
";

const FULL_RUN_FILE: &str = r##"
os = "os/tiny.asm"
images = ["main.asm", "data.hex"]
mem_latency = 2
cycle_cap = 5000
input = "unused"
dump = ["x4000..x4002", "12293..#12293"]

[registers]
r0 = 5
PC = "x3000"
psr = "x0002"

[memory]
x3005 = "#-3"
x4010 = [1, "#2", "x3", -1]
x5000 = "xBEEF"

[[at]]
pc = "x3002"
[at.registers]
R3 = 7
[at.memory]
x4000 = [10, "x14", 30]

[[at]]
pc = 12291
[at.registers]
R6 = "#-1"
[at.memory]
x4002 = 99
"##;

fn full_sandbox() -> Sandbox {
    let sandbox = Sandbox::new();
    sandbox.write("os/tiny.asm", TINY_OS_ASM);
    sandbox.write("main.asm", FULL_PROGRAM);
    sandbox.write("data.hex", "5000\n00AA\n00BB\n");
    sandbox.write("full.toml", FULL_RUN_FILE);
    sandbox
}

fn assert_json_shape(value: &Value) {
    let object = value.as_object().unwrap();
    let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "cycles",
            "dumps",
            "failure",
            "halt_output",
            "mem_latency",
            "os",
            "outcome",
            "program_output",
            "registers",
            "word_counts"
        ]
    );
}

#[test]
fn bare_assembly_reads_stdin_and_reports_on_stderr() {
    let sandbox = Sandbox::new();
    sandbox.write("echo.asm", ECHO_ASM);
    let output = sandbox.lc3(&["echo.asm"], b"A");
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    assert_eq!(output.stdout, [b"A".as_slice(), HALT_MESSAGE].concat());
    let summary = stderr(&output);
    for expected in [
        "halted",
        "cycles: ",
        "echo.asm: ",
        "OS: ",
        "unattributed: 0",
        "word counts:",
        "memory latency: 4",
        "os: bundled",
        "R0=x",
        "R7=x",
        "PC=x",
        "PSR=x",
    ] {
        assert!(
            summary.contains(expected),
            "missing {expected:?} in {summary}"
        );
    }
}

#[test]
fn every_source_extension_runs_with_json_before_or_after_the_path() {
    let sandbox = Sandbox::new();
    sandbox.write("halt.asm", HALT_ASM);
    sandbox.write(
        "halt.bin",
        "0011000000000000\n1011000000000000\n1111111111111110\n",
    );
    sandbox.write("halt.hex", "3000\nB000\nFFFE\n");
    sandbox.write("halt.obj", [0x30, 0x00, 0xB0, 0x00, 0xFF, 0xFE]);
    for path in ["halt.asm", "halt.bin", "halt.hex", "halt.obj"] {
        for args in [["--json", path], [path, "--json"]] {
            let (output, value) = sandbox.json(&args, b"");
            assert_eq!(code(&output), 0, "{path}: {}", stderr(&output));
            assert!(output.stderr.is_empty(), "{path}: {}", stderr(&output));
            assert_eq!(value["outcome"], "halted", "{path}");
            assert_eq!(value["registers"]["PC"], 0x3001, "{path}");
            assert_eq!(value["word_counts"]["images"], json!([2]), "{path}");
        }
    }
}

#[test]
fn source_and_run_file_extensions_match_case_insensitively() {
    let sandbox = Sandbox::new();
    sandbox.write("HALT.ASM", HALT_ASM);
    sandbox.write("Halt.Hex", "3000\nB000\nFFFE\n");
    sandbox.write("run.TOML", "images = [\"HALT.ASM\"]\n");
    for path in ["HALT.ASM", "Halt.Hex", "run.TOML"] {
        let (output, value) = sandbox.json(&[path, "--json"], b"");
        assert_eq!(code(&output), 0, "{path}: {}", stderr(&output));
        assert_eq!(value["outcome"], "halted", "{path}");
    }
}

#[test]
fn rejected_extensions_name_the_accepted_ones() {
    let sandbox = Sandbox::new();
    for path in ["prog.txt", "prog.AS", "prog", "prog.asm.bak"] {
        sandbox.write(path, HALT_ASM);
        let output = sandbox.lc3(&[path], b"");
        assert_usage_error(&output, &[path, ".asm", ".bin", ".hex", ".obj", ".toml"]);
    }
}

#[test]
fn argument_errors_are_usage_errors_with_a_hint() {
    let sandbox = Sandbox::new();
    sandbox.write("a.asm", HALT_ASM);
    let cases: [(&[&str], &str); 5] = [
        (&[], "usage: lc3"),
        (&["a.asm", "b.asm"], "b.asm"),
        (&["--wat", "a.asm"], "--wat"),
        (&["--json"], "usage: lc3"),
        (&["--help", "a.asm"], "usage: lc3"),
    ];
    for (args, fragment) in cases {
        let output = sandbox.lc3(args, b"");
        assert_usage_error(&output, &["usage: lc3", fragment]);
    }
}

#[test]
fn help_prints_usage_schema_and_exit_codes_to_stdout() {
    let sandbox = Sandbox::new();
    let output = sandbox.lc3(&["--help"], b"");
    assert_eq!(code(&output), 0);
    assert!(output.stderr.is_empty());
    let help = String::from_utf8(output.stdout).unwrap();
    for expected in [
        "usage: lc3",
        "images",
        "mem_latency",
        "cycle_cap",
        "input_file",
        "[registers]",
        "[memory]",
        "dump",
        "[[at]]",
        "0  halted",
        "1  usage or run-file error",
        "2  source error",
        "3  boot failure",
        "10 cycle cap reached",
        "11 illegal instruction",
        "12 input exhausted",
        "13 unknown trap",
    ] {
        assert!(help.contains(expected), "missing {expected:?} in {help}");
    }
}

#[test]
fn run_file_exercising_every_key_reports_in_json() {
    let sandbox = full_sandbox();
    let (output, value) = sandbox.json(&["full.toml", "--json"], b"stdin is ignored");
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    assert_json_shape(&value);
    assert_eq!(value["outcome"], "halted");
    assert_eq!(value["failure"], Value::Null);
    let registers = &value["registers"];
    assert_eq!(registers.as_object().unwrap().len(), REGISTER_NAMES.len());
    assert_eq!(registers["R0"], 5);
    assert_eq!(registers["R1"], 0xFFFD);
    assert_eq!(registers["R2"], 2);
    assert_eq!(registers["R3"], 7);
    assert_eq!(registers["R4"], 8);
    assert_eq!(registers["R5"], 10);
    assert_eq!(registers["R6"], 0xFFFF);
    assert_eq!(registers["PC"], 0x3005);
    assert_eq!(registers["PSR"], 0x0001);
    assert_eq!(
        value["dumps"],
        json!([
            {"start": 0x4000, "end": 0x4002, "words": [10, 20, 99]},
            {"start": 0x3005, "end": 0x3005, "words": [0xFFFD]},
        ])
    );
    assert_eq!(value["program_output"], "");
    assert_eq!(value["halt_output"], "");
    let cycles = &value["cycles"];
    let total = cycles["total"].as_u64().unwrap();
    assert!(total > 0);
    assert_eq!(cycles["images"], json!([total, 0]));
    assert_eq!(cycles["os"], 0);
    assert_eq!(cycles["unattributed"], 0);
    assert_eq!(value["word_counts"], json!({"images": [8, 2], "os": 2}));
    assert_eq!(value["mem_latency"], 2);
    assert_eq!(
        value["os"],
        json!({"kind": "file", "path": "os/tiny.asm", "sha256": TINY_OS_SHA256})
    );
}

#[test]
fn run_file_text_summary_lists_attribution_registers_and_dumps() {
    let sandbox = full_sandbox();
    let output = sandbox.lc3(&["full.toml"], b"");
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    assert!(output.stdout.is_empty());
    let summary = stderr(&output);
    for expected in [
        "main.asm: ",
        "data.hex: 0",
        "OS: 0",
        "unattributed: 0",
        "main.asm: 8",
        "data.hex: 2",
        "OS: 2",
        "memory latency: 2",
        &format!("os: os/tiny.asm (sha256 {TINY_OS_SHA256})"),
        "R3=x0007",
        "R6=xFFFF",
        "PC=x3005",
        "PSR=x0001",
        "dump x4000..x4002:",
        "x4000: x000A",
        "x4001: x0014",
        "x4002: x0063",
        "dump x3005..x3005:",
        "x3005: xFFFD",
    ] {
        assert!(
            summary.contains(expected),
            "missing {expected:?} in {summary}"
        );
    }
}

#[test]
fn bundled_os_identity_is_reported_for_bare_sources_and_run_files() {
    let sandbox = Sandbox::new();
    sandbox.write("halt.asm", HALT_ASM);
    sandbox.write("run.toml", "images = [\"halt.asm\"]\n");
    for path in ["halt.asm", "run.toml"] {
        let (_, value) = sandbox.json(&[path, "--json"], b"");
        assert_eq!(value["os"], json!({"kind": "bundled"}), "{path}");
        assert_eq!(value["mem_latency"], 4, "{path}");
        assert!(value["word_counts"]["os"].as_u64().unwrap() > 0, "{path}");
    }
}

#[test]
fn run_file_paths_resolve_against_the_working_directory() {
    let sandbox = Sandbox::new();
    sandbox.write("prog.asm", HALT_ASM);
    sandbox.write("sub/run.toml", "images = [\"prog.asm\"]\n");
    let (output, value) = sandbox.json(&["sub/run.toml", "--json"], b"");
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    assert_eq!(value["outcome"], "halted");

    let elsewhere = Sandbox::new();
    elsewhere.write("sub/prog.asm", HALT_ASM);
    elsewhere.write("sub/run.toml", "images = [\"prog.asm\"]\n");
    let output = elsewhere.lc3(&["sub/run.toml"], b"");
    assert_usage_error(&output, &["prog.asm"]);
}

#[test]
fn stdin_run_file_takes_console_input_only_from_the_run_file() {
    let sandbox = Sandbox::new();
    sandbox.write("echo.asm", ECHO_ASM);
    sandbox.write("keys.bin", [0x00, 0xFF, b'Q']);

    let output = sandbox.lc3(&["-"], b"images = [\"echo.asm\"]\ninput = \"Z\"\n");
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    assert_eq!(output.stdout, [b"Z".as_slice(), HALT_MESSAGE].concat());

    let output = sandbox.lc3(
        &["-"],
        b"images = [\"echo.asm\"]\ninput_file = \"keys.bin\"\n",
    );
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    assert_eq!(output.stdout, [b"\0".as_slice(), HALT_MESSAGE].concat());

    let (output, value) = sandbox.json(&["--json", "-"], b"images = [\"echo.asm\"]\n");
    assert_eq!(code(&output), 12, "{}", stderr(&output));
    assert_eq!(value["failure"]["reason"], "input_exhausted");
}

#[test]
fn path_run_file_falls_back_to_stdin_unless_it_gives_input() {
    let sandbox = Sandbox::new();
    sandbox.write("echo.asm", ECHO_ASM);
    sandbox.write("bare.toml", "images = [\"echo.asm\"]\n");
    sandbox.write("given.toml", "images = [\"echo.asm\"]\ninput = \"G\"\n");
    sandbox.write(
        "file.toml",
        "images = [\"echo.asm\"]\ninput_file = \"echo.asm\"\n",
    );

    let output = sandbox.lc3(&["bare.toml"], b"S");
    assert_eq!(output.stdout, [b"S".as_slice(), HALT_MESSAGE].concat());
    let output = sandbox.lc3(&["given.toml"], b"S");
    assert_eq!(output.stdout, [b"G".as_slice(), HALT_MESSAGE].concat());
    let output = sandbox.lc3(&["file.toml"], b"S");
    assert_eq!(output.stdout, [b".".as_slice(), HALT_MESSAGE].concat());
}

#[test]
fn stdin_is_read_as_raw_bytes_including_invalid_utf8() {
    let sandbox = Sandbox::new();
    sandbox.write(
        "two.asm",
        ".ORIG x3000\nGETC\nADD R1,R0,#0\nGETC\nSTI R0,PTR\nPTR .FILL xFFFE\n.END\n",
    );
    let (output, value) = sandbox.json(&["two.asm", "--json"], &[0xFF, 0x80, 0xC3]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    assert_eq!(value["registers"]["R1"], 0xFF);
    assert_eq!(value["registers"]["R0"], 0x80);
}

#[test]
fn console_bytes_are_raw_in_text_mode_and_latin1_in_json() {
    let sandbox = Sandbox::new();
    sandbox.write(
        "out.asm",
        ".ORIG x3000\nLD R0,CH\nOUT\nHALT\nCH .FILL xE9\n.END\n",
    );
    let output = sandbox.lc3(&["out.asm"], b"");
    assert_eq!(output.stdout, [b"\xE9".as_slice(), HALT_MESSAGE].concat());

    let (_, value) = sandbox.json(&["out.asm", "--json"], b"");
    assert_eq!(value["program_output"], "\u{e9}");
    assert_eq!(value["halt_output"], "\n--- halted ---\n");
}

#[test]
fn json_failures_carry_a_reason_fields_and_null_state() {
    let sandbox = Sandbox::new();
    sandbox.write("echo.asm", ECHO_ASM);
    sandbox.write("halt.asm", HALT_ASM);
    sandbox.write("cap.toml", "images = [\"halt.asm\"]\ncycle_cap = 3\n");
    sandbox.write("reserved.hex", "3000\nD000\n");
    sandbox.write("rti.hex", "3000\n8000\n");
    sandbox.write("malformed.hex", "3000\n1010\n");
    sandbox.write("trap.hex", "3000\nF030\n");
    sandbox.write("boot-os.hex", "0200\nD000\n");
    sandbox.write(
        "boot.toml",
        "os = \"boot-os.hex\"\nimages = [\"halt.asm\"]\n",
    );

    let cases: [(&str, i32, Value); 7] = [
        ("cap.toml", 10, json!({"reason": "cycle_cap_reached"})),
        (
            "reserved.hex",
            11,
            json!({"reason": "illegal_instruction", "pc": 0x3000, "word": 0xD000, "kind": "reserved"}),
        ),
        (
            "rti.hex",
            11,
            json!({"reason": "illegal_instruction", "pc": 0x3000, "word": 0x8000, "kind": "rti"}),
        ),
        (
            "malformed.hex",
            11,
            json!({"reason": "illegal_instruction", "pc": 0x3000, "word": 0x1010, "kind": "malformed_encoding"}),
        ),
        (
            "trap.hex",
            13,
            json!({"reason": "unknown_trap", "vector": 0x30}),
        ),
        (
            "boot.toml",
            3,
            json!({
                "reason": "boot",
                "failure": {"reason": "illegal_instruction", "pc": 0x0200, "word": 0xD000, "kind": "reserved"}
            }),
        ),
        ("echo.asm", 12, Value::Null),
    ];
    for (path, exit, failure) in cases {
        let (output, value) = sandbox.json(&[path, "--json"], b"");
        assert_eq!(code(&output), exit, "{path}: {}", stderr(&output));
        assert_json_shape(&value);
        assert_eq!(value["outcome"], "failed", "{path}");
        assert_eq!(value["registers"], Value::Null, "{path}");
        assert_eq!(value["cycles"], Value::Null, "{path}");
        assert_eq!(value["dumps"], json!([]), "{path}");
        assert_eq!(value["program_output"], "", "{path}");
        assert_eq!(value["halt_output"], "", "{path}");
        assert_eq!(value["mem_latency"], 4, "{path}");
        assert!(value["word_counts"]["images"].is_array(), "{path}");
        assert!(value["word_counts"]["os"].as_u64().unwrap() > 0, "{path}");
        if failure.is_null() {
            assert_eq!(value["failure"]["reason"], "input_exhausted", "{path}");
            assert!(value["failure"]["pc"].is_u64(), "{path}");
        } else {
            assert_eq!(value["failure"], failure, "{path}");
        }
    }
}

#[test]
fn text_failures_explain_the_reason_in_lc3_hex_and_exit_distinctly() {
    let sandbox = Sandbox::new();
    sandbox.write("echo.asm", ECHO_ASM);
    sandbox.write("halt.asm", HALT_ASM);
    sandbox.write("cap.toml", "images = [\"halt.asm\"]\ncycle_cap = 3\n");
    sandbox.write("reserved.hex", "3000\nD000\n");
    sandbox.write("trap.hex", "3000\nF030\n");
    sandbox.write("boot-os.hex", "0200\nD000\n");
    sandbox.write(
        "boot.toml",
        "os = \"boot-os.hex\"\nimages = [\"halt.asm\"]\n",
    );

    let cases: [(&str, i32, &str); 5] = [
        ("cap.toml", 10, "failed: cycle cap reached"),
        (
            "reserved.hex",
            11,
            "failed: illegal instruction xD000 at x3000 (reserved opcode)",
        ),
        ("trap.hex", 13, "failed: unknown trap x30"),
        ("echo.asm", 12, "failed: input exhausted at x"),
        (
            "boot.toml",
            3,
            "failed: boot: illegal instruction xD000 at x0200 (reserved opcode)",
        ),
    ];
    for (path, exit, message) in cases {
        let output = sandbox.lc3(&[path], b"");
        let summary = stderr(&output);
        assert_eq!(code(&output), exit, "{path}: {summary}");
        assert!(output.stdout.is_empty(), "{path}");
        assert!(
            summary.contains(message),
            "{path}: missing {message:?} in {summary}"
        );
        assert!(summary.contains("memory latency: 4"), "{path}: {summary}");
        assert!(summary.contains("os: "), "{path}: {summary}");
    }
}

#[test]
fn halted_run_exits_zero() {
    let sandbox = Sandbox::new();
    sandbox.write("halt.asm", HALT_ASM);
    assert_eq!(code(&sandbox.lc3(&["halt.asm"], b"")), 0);
}

#[test]
fn unknown_keys_are_rejected_by_name() {
    assert_run_file_error(
        "images = [\"halt.asm\"]\nbogus_key = 1\n",
        &["run.toml", "bogus_key"],
    );
    assert_run_file_error(
        "images = [\"halt.asm\"]\n[registers]\nR9 = 1\n",
        &["run.toml", "R9", "[registers]"],
    );
    assert_run_file_error(
        "images = [\"halt.asm\"]\n[registers]\nbogus = 1\n",
        &["run.toml", "bogus"],
    );
    assert_run_file_error(
        "images = [\"halt.asm\"]\n[[at]]\npc = 1\nbogus_at = 2\n",
        &["run.toml", "bogus_at"],
    );
    assert_run_file_error(
        "images = [\"halt.asm\"]\n[[at]]\npc = 1\n[at.registers]\nR8 = 2\n",
        &["run.toml", "R8", "[[at]]"],
    );
}

#[test]
fn malformed_run_files_are_run_file_errors() {
    assert_run_file_error("= nope\n", &["run.toml"]);
    assert_run_file_error("mem_latency = 4\n", &["run.toml", "images"]);
    assert_run_file_error("images = []\n", &["run.toml", "images", "non-empty"]);
    assert_run_file_error("images = [\"missing.asm\"]\n", &["missing.asm"]);
    assert_run_file_error("images = [\"halt.txt\"]\n", &["halt.txt", ".asm", ".obj"]);
    assert_run_file_error(
        "images = [\"halt.asm\"]\nos = \"missing.obj\"\n",
        &["missing.obj"],
    );
    assert_run_file_error(
        "images = [\"halt.asm\"]\ninput_file = \"missing.bin\"\n",
        &["missing.bin"],
    );
    assert_run_file_error(
        "images = [\"halt.asm\"]\ninput = \"a\"\ninput_file = \"halt.asm\"\n",
        &["input", "input_file"],
    );
    assert_run_file_error(
        "images = [\"halt.asm\"]\nmem_latency = 0\n",
        &["mem_latency", "0"],
    );
    assert_run_file_error(
        "images = [\"halt.asm\"]\nmem_latency = 5000000000\n",
        &["mem_latency", "5000000000"],
    );
    assert_run_file_error(
        "images = [\"halt.asm\"]\ncycle_cap = -1\n",
        &["cycle_cap", "-1"],
    );
    assert_run_file_error(
        "images = [\"halt.asm\"]\nmem_latency = \"4\"\n",
        &["mem_latency"],
    );
}

#[test]
fn bad_values_name_the_key_and_the_value() {
    let cases: [(&str, &[&str]); 11] = [
        ("[registers]\nR0 = 65536\n", &["R0", "65536"]),
        ("[registers]\nR0 = -32769\n", &["R0", "-32769"]),
        ("[registers]\nR1 = \"xZZ\"\n", &["R1", "xZZ"]),
        ("[registers]\nR1 = \"x10000\"\n", &["R1", "x10000"]),
        ("[registers]\nR1 = 1\nr1 = 2\n", &["r1", "R1"]),
        ("[registers]\nR1 = true\n", &["R1"]),
        ("[memory]\nzzz = 1\n", &["zzz"]),
        ("[memory]\n\"x10000\" = 1\n", &["x10000"]),
        ("[memory]\nx3000 = [1, \"#70000\"]\n", &["x3000", "#70000"]),
        ("[[at]]\npc = 70000\n", &["pc", "70000"]),
        ("[[at]]\nmemory = {}\n", &["pc"]),
    ];
    for (body, expected) in cases {
        assert_run_file_error(&format!("images = [\"halt.asm\"]\n{body}"), expected);
    }
}

#[test]
fn dump_ranges_must_be_inclusive_address_pairs() {
    for dump in [
        "x3010..x3000",
        "x3000",
        "x3000..",
        "x3000..x10000",
        "x3000..#-1",
        "a..b",
    ] {
        assert_run_file_error(
            &format!("images = [\"halt.asm\"]\ndump = [\"{dump}\"]\n"),
            &["dump", dump],
        );
    }
}

#[test]
fn memory_and_register_literal_forms_all_parse() {
    let sandbox = Sandbox::new();
    sandbox.write("halt.asm", HALT_ASM);
    sandbox.write(
        "run.toml",
        r##"
images = ["halt.asm"]
dump = ["x5000..x5006"]

[memory]
x5000 = "XFF"
x5001 = "#-1"
x5002 = "-1"
x5003 = "12"
x5004 = "x-1"
x5005 = -32768
x5006 = 65535
"##,
    );
    let (output, value) = sandbox.json(&["run.toml", "--json"], b"");
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    assert_eq!(
        value["dumps"][0]["words"],
        json!([0xFF, 0xFFFF, 0xFFFF, 12, 0xFFFF, 0x8000, 0xFFFF])
    );
}

#[test]
fn a_minus_before_a_literal_prefix_is_rejected_like_the_assembler_does() {
    for literal in ["-x1", "-#1"] {
        assert_run_file_error(
            &format!("images = [\"halt.asm\"]\n[memory]\nx5000 = \"{literal}\"\n"),
            &["run.toml", literal],
        );
    }
}

fn assert_memory_overlap_rejected(memory: &str, expected: &[&str]) {
    assert_run_file_error(
        &format!("images = [\"halt.asm\"]\n[memory]\n{memory}"),
        expected,
    );
}

#[test]
fn overlapping_memory_entries_in_one_table_are_rejected() {
    assert_memory_overlap_rejected(
        "x4000 = [1, 2, 3]\nx4001 = 7\n",
        &["x4000", "x4001", "x4001"],
    );
    assert_memory_overlap_rejected(
        "x4001 = 7\nx4000 = [1, 2, 3]\n",
        &["x4000", "x4001", "x4001"],
    );
    assert_memory_overlap_rejected("x4000 = 1\n\"#16384\" = 2\n", &["x4000", "#16384", "x4000"]);
    assert_memory_overlap_rejected(
        "xFFFE = [1, 2, 3]\nx0000 = 9\n",
        &["xFFFE", "x0000", "x0000"],
    );
    assert_run_file_error(
        "images = [\"halt.asm\"]\n[[at]]\npc = 1\n[at.memory]\nx10 = [1, 2]\nx11 = 3\n",
        &["[[at]] entry 1", "x10", "x11", "x0011"],
    );
}

#[test]
fn non_overlapping_memory_is_independent_of_key_order_and_top_level_may_overlap_triggers() {
    let sandbox = Sandbox::new();
    sandbox.write("halt.asm", HALT_ASM);
    let dump = "dump = [\"x4000..x4003\"]\n";
    let forward = "[memory]\nx4000 = [1, 2]\nx4002 = [3, 4]\n";
    let reverse = "[memory]\nx4002 = [3, 4]\nx4000 = [1, 2]\n";
    for memory in [forward, reverse] {
        sandbox.write(
            "run.toml",
            format!("images = [\"halt.asm\"]\n{dump}{memory}"),
        );
        let (output, value) = sandbox.json(&["run.toml", "--json"], b"");
        assert_eq!(code(&output), 0, "{}", stderr(&output));
        assert_eq!(value["dumps"][0]["words"], json!([1, 2, 3, 4]));
    }
    sandbox.write(
        "run.toml",
        format!(
            "images = [\"halt.asm\"]\n{dump}{forward}[[at]]\npc = 12288\n[at.memory]\nx4000 = 9\n"
        ),
    );
    let (output, value) = sandbox.json(&["run.toml", "--json"], b"");
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    assert_eq!(value["dumps"][0]["words"], json!([9, 2, 3, 4]));
}

#[test]
fn source_errors_exit_two_with_file_and_line_before_any_run() {
    let sandbox = Sandbox::new();
    sandbox.write("bad.asm", ".ORIG x3000\nADD R0,R0\n.END\n");
    let output = sandbox.lc3(&["bad.asm"], b"");
    assert_eq!(code(&output), 2);
    assert!(output.stdout.is_empty());
    assert!(
        stderr(&output).contains("bad.asm:2: error: "),
        "{}",
        stderr(&output)
    );

    sandbox.write("bad.obj", [0x30, 0x00, 0x01]);
    let output = sandbox.lc3(&["bad.obj"], b"");
    assert_eq!(code(&output), 2);
    let message = stderr(&output);
    assert!(message.contains("bad.obj: error: "), "{message}");
    assert!(!message.contains("bad.obj:0"), "{message}");

    sandbox.write("good.asm", HALT_ASM);
    sandbox.write("run.toml", "images = [\"good.asm\", \"bad.asm\"]\n");
    let output = sandbox.lc3(&["run.toml", "--json"], b"");
    assert_eq!(code(&output), 2);
    assert!(output.stdout.is_empty());
    assert!(stderr(&output).contains("bad.asm:2: error: "));
}

#[test]
fn source_warnings_are_printed_and_the_run_proceeds() {
    let sandbox = Sandbox::new();
    sandbox.write("warn.asm", format!("{HALT_ASM}trailing\n"));
    let output = sandbox.lc3(&["warn.asm"], b"");
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    assert!(
        stderr(&output).contains("warn.asm:5: warning: "),
        "{}",
        stderr(&output)
    );

    let (output, value) = sandbox.json(&["warn.asm", "--json"], b"");
    assert_eq!(code(&output), 0);
    assert_eq!(value["outcome"], "halted");
    assert!(stderr(&output).contains("warn.asm:5: warning: "));
}

#[test]
fn json_mode_keeps_stdout_to_one_object() {
    let sandbox = Sandbox::new();
    sandbox.write("echo.asm", ECHO_ASM);
    let output = sandbox.lc3(&["echo.asm", "--json"], b"A");
    let text = String::from_utf8(output.stdout).unwrap();
    assert_eq!(text.lines().count(), 1, "{text}");
    assert!(text.ends_with('\n'));
    let value: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(value["program_output"], "A");
}
