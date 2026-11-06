# The bundled OS is our own cycle-optimized OS

The bundled OS is written for this project to minimize cycles; it is not `lc3os.asm`. For GETC, OUT, PUTS, IN, PUTSP and HALT it must produce the same program output and R0 as the reference OS and preserve R1-R6; R7 and condition codes after a TRAP are unspecified, IN may omit its prompt, and the halt message may be shortened but must appear. An unknown trap fails the run.

## Considered Options

Bundling `lc3os.asm` verbatim was rejected: it is GPL and cycle-heavy (it polls device status the simulator always reports ready). Using the bundled OS for comparisons with `lc3sim` was rejected: `lc3sim` runs the reference OS, and a comparison needs the same OS (ADR 0001).

## Consequences

TRAP routines are counted, so a cycle count with the bundled OS differs from one with the reference OS. A user reproduces reference-OS counts by naming the reference OS in the run file. The run result names the OS used (bundled, or path and sha256).
