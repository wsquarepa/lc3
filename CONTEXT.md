# LC-3 Toolchain

A fast LC-3 simulator, assembler and converter that report the FSM cycle count of a run. It deliberately diverges from Lumetta's lc3tools wherever that does not change a program's architectural result.

## Language

### Programs and loading

**Source**:
A file holding LC-3 program words in one of four formats: `.asm`, `.bin`, `.hex` or `.obj`.
_Avoid_: input file, program file

**Image**:
The words a source places in memory, starting at its origin.
_Avoid_: object, binary, segment

**Origin**:
The address an image's first word loads at.
_Avoid_: load address, start address

**Word count**:
The number of words an image places in memory, `.BLKW` space included and the origin word excluded.
_Avoid_: program size, length

### The machine

**OS**:
The image that occupies system space and implements the TRAP routines for a run; always supplied to a run, never assumed.
_Avoid_: kernel, firmware

**Bundled OS**:
This project's own cycle-optimized OS, offered as the default when no OS is named.
_Avoid_: default OS, stock OS

**Reference OS**:
`lc3os.obj` as installed with the lc3tools release that `docs/lc3sim-reference.md` measures.
_Avoid_: lc3os, stock OS

**Boot**:
Execution of the OS from its entry point until it first halts, before any user image is loaded; never counted.
_Avoid_: startup, init

**Boot snapshot**:
The machine state at the end of boot, from which every run starts.
_Avoid_: initial state, reset state

**Memory latency**:
The number of cycles each memory-access FSM state costs in total.
_Avoid_: wait states, memory delay

### Runs

**Run**:
One execution of images over a boot snapshot, from the first image's origin until an outcome.
_Avoid_: simulation, execution, test

**Case**:
The per-run inputs: pokes, triggers, console input and cycle cap.
_Avoid_: test case, scenario

**Run file**:
A TOML document naming a run's OS, images, memory latency and case.
_Avoid_: config, case file, script

**Poke**:
A value written to a register or memory location by the case rather than by the program; costs no cycles.
_Avoid_: set, override, injection

**Trigger**:
A set of pokes applied the first time execution reaches a given PC.
_Avoid_: breakpoint, hook

**Cycle cap**:
The cycle count at which a run is stopped and fails.
_Avoid_: timeout, limit

**Cycle**:
One unit of FSM time: one per non-memory state, memory latency per memory-access state.
_Avoid_: tick, step

**Attribution**:
Assignment of each instruction's cycles to the image that originally loaded the address it was fetched from, or to no image.
_Avoid_: profile, breakdown

### Outcomes

**Outcome**:
The result of a run: exactly one of halted or failed.
_Avoid_: result, status, verdict

**Halted**:
The outcome of a run stopped by a write to the machine control register.
_Avoid_: success, passed, finished

**Failed**:
The outcome of a run stopped for any other reason, carrying that reason.
_Avoid_: error, crashed, rejected

**Program output**:
The console bytes written before the run first executes `TRAP x25`.
_Avoid_: stdout, console output

**Halt output**:
The console bytes written from the first `TRAP x25` onward.
_Avoid_: halt message, banner

### Compatibility

**Must-match tier**:
The architectural result (final registers, memory and program output) of a program that halts under `lc3sim` with `option device off`, which this project must reproduce given the same OS.
_Avoid_: compatibility, parity
