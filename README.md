# lc3

> [!WARNING]
> This project is mostly to-be-used by a grading workflow.  
> I do not guarantee maintenance or support.

A "fast"[^1] LC-3 simulator and assembler that reports the FSM cycle count of every run. It loads `.asm`, `.bin`, `.hex` and `.obj` sources, matches `lc3sim`'s final registers, memory and program output for any program that halts there, and ships a cycle-optimized OS of its own.[^2]


## Install

```sh
cargo install --git https://github.com/wsquarepa/lc3
```

This builds the `lc3` binary with the pinned Rust toolchain (`rust-toolchain.toml`) and installs it to `~/.cargo/bin`.

## Quickstart

1. Write a program, for example `hello.asm`:

   ```asm
           .ORIG x3000
           LEA R0, MSG
           PUTS
           HALT
   MSG     .STRINGZ "hello\n"
           .END
   ```

2. Run `lc3 hello.asm < /dev/null`. The program output goes to stdout, and a summary with the cycle count goes to stderr.

A bare source runs with the bundled OS, memory latency 4, a 10,000,000-cycle cap, and console input read from stdin. Stdin is read to end of file before the run starts, so a bare `lc3 hello.asm` in a terminal waits for Ctrl-D. Use a run file for anything else.

## Common Commands

```sh
# Run a source with the defaults and no console input
lc3 main.asm < /dev/null

# Feed console input from stdin
printf 'abc\n' | lc3 main.asm

# Run a TOML run file (OS, images, latency, pokes, triggers, dumps)
lc3 run.toml

# Read the run file from stdin
lc3 - < run.toml

# One JSON object instead of console bytes and a summary
lc3 run.toml --json

# Full run-file reference, output format and exit codes
lc3 --help
```

A run file names everything a run needs:

```toml
os          = "lc3os.obj"   # optional; the bundled OS if absent
images      = ["main.asm"]
mem_latency = 4             # how many cycles should r/w take?
input       = "abc\n"
dump        = ["x4000..x400F"]

[registers]
R0 = "x0005"

[[at]]                      # pokes applied the first time PC reaches x3002
pc = "x3002"
[at.memory]
x4010 = 9
```

## Library

The same machine is a library crate. Prepare once, then run any number of cases against the shared boot snapshot:

```rust
use std::num::NonZeroU32;

use lc3::{Case, Format, Outcome, bundled_os, load, prepare, run};

fn main() {
    let source = std::fs::read("main.asm").expect("main.asm is readable");
    let image = load(Format::Asm, &source).expect("main.asm assembles").image;
    let mem_latency = NonZeroU32::new(4).unwrap();
    let machine = prepare(bundled_os(), &[image], mem_latency).expect("the OS boots");

    for input in [&b"abc\n"[..], b"xyz\n"] {
        let case = Case { input: input.to_vec(), ..Case::default() };
        match run(&machine, &case) {
            Outcome::Halted(halted) => println!("{} cycles", halted.cycles),
            Outcome::Failed(failure) => eprintln!("failed: {failure:?}"),
        }
    }
}
```

## Documentation

- [Domain glossary](CONTEXT.md): the terms used throughout the code and docs (image, case, trigger, outcome, must-match tier).
- [Decisions](docs/adr/): why the project diverges from lc3tools where it does, how boot, console output and triggers work.
- [lc3sim reference](docs/lc3sim-reference.md): measured `lc3sim` and `lc3as` behavior that the must-match tier follows.
- [FSM state sequences](docs/fsm-state-sequences.md): the cycle model, verified against Patt and Patel's Figure C.2.
- [Bundled OS](docs/bundled-os-proof.md): the cycle cost of each TRAP routine and the lower bounds behind it.
- [Performance](docs/throughput.md): measured throughput and how to reproduce it.

## Develop

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

The differential tests compare runs against fixtures captured from `lc3sim` (`scripts/capture-differential-fixtures`). Tests that need a local lc3tools install under `reference/` print a skip message when it is absent.

[^1]: ...well, at least it's faster than `lc3tools`.
[^2]: Okay, well, at least I think it's optimal. Definitely more optimal than `lc3sim`'s default OS though.
