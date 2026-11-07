//! The `--help` text and the one-line usage hint.

pub const HINT: &str = "usage: lc3 <run.toml | source | -> [--json]   (see lc3 --help)";

pub const HELP: &str = r##"usage: lc3 <path> [--json]
       lc3 - [--json]
       lc3 --help

  <path>   a run file ending in .toml, or a bare source ending in .asm, .bin,
           .hex or .obj (extensions are lowercase and choose the format)
  -        read a run file from stdin
  --json   write one compact JSON object plus a newline to stdout instead of
           the console bytes and a stderr summary; may precede or follow <path>

A bare source runs with the bundled OS, memory latency 4, cycle cap 10000000,
no pokes or triggers, and console input read from stdin.

Run file (TOML; unknown keys are errors):

  os          = "os.obj"          optional; the bundled OS if absent
  images      = ["main.asm"]      required, non-empty; PC starts at the first origin
  mem_latency = 4                 optional integer >= 1
  cycle_cap   = 10000000          optional integer >= 1
  input       = "text"            optional console input (UTF-8 bytes), or
  input_file  = "keys.bin"        optional console input (raw bytes), never both
  dump        = ["x3000..x300F"]  optional inclusive ranges, start <= end

  [registers]                     R0-R7, PC, PSR, case-insensitive
  R0 = "x0005"
  [memory]                        address = word, or address = [words] written upward
  x4000 = [1, "#-2", 3]
  [[at]]                          triggers fire the first time PC reaches pc
  pc = "x3002"
  [at.registers]                  same shapes as the top-level tables
  R3 = 7
  [at.memory]
  x4010 = 9

  Words and addresses are TOML integers or literal strings: x/X hex, # decimal
  or bare decimal, each with an optional minus directly before the digits (x-10,
  #-12, -12; not -x10). Words take -32768..65535
  (negative is two's complement); addresses and pc take 0..65535.
  All relative paths resolve against the current working directory.

Console input: input or input_file when given; otherwise raw stdin bytes read
to end of file before the run. With `lc3 -` the run file occupies stdin, so
console input comes only from input or input_file and is otherwise empty.

Output: text mode writes the program output then the halt output to stdout and
a summary to stderr. JSON mode keys: outcome, failure, registers, dumps,
program_output, halt_output, cycles, word_counts, mem_latency, os. Console
strings map each byte to the code point of the same value (U+0000..U+00FF).
Source diagnostics go to stderr as <path>:<line>: <severity>: <message>.

Exit codes:
  0  halted
  1  usage or run-file error (bad arguments, unknown key, malformed value,
     unreadable file, input with input_file, no images, too many images)
  2  source error: a source had an error diagnostic
  3  boot failure
  10 cycle cap reached
  11 illegal instruction
  12 input exhausted
  13 unknown trap
"##;
