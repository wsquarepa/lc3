# lc3sim reference behavior

Measured facts about an installed lc3tools, the authority for the must-match tier (ADR 0001). Measured 2026-11-06 on that install by running `lc3sim -s <script>` and `lc3as` in a scratch directory. Source reading used the public mirror https://github.com/RadixSeven/lc3tools (CHANGE_LOG says 0.12; the installed README says "the 0.5 release"; no binary carries a version string). Every behavior below was measured on the installed binary unless marked "source only".

## Reference OS

- `lc3os.obj` sha256 `aed2b105801f8d5e203ac2f5fcfb5ab4e6ac91ea78300e77b80c02fd43a22760`; `lc3os.sym` sha256 `8f6f9f69ed1d6565891cbd5c28fa63e5fc3a74a03d1c2d5b6a1e555a38ff4ab2`.
- `lc3os.asm` is not installed. The mirror's `lc3os.asm`, assembled by the installed `lc3as`, is byte-identical to the installed `.obj` and `.sym`, so the mirror source is the reference OS source.
- License of lc3tools: GPL v2.

## Boot and start state

- At startup `lc3sim` zeroes registers and memory, sets PSR=x0400, loads `lc3os.obj`, and runs it from x0200 until the OS halts by writing the machine control register. It prints a banner, the halt message and a register dump.
- Loading a program only sets PC to its origin.
- State at the first instruction of a program loaded at x3000: PC=x3000, IR=xB1AE, PSR=x0400 (condition code Z), R1=x7FFF, R7=x0490, R0 and R2-R6 = x0000.
- Unloaded memory is x0000, including xFE00-xFE08. x0100-x0103 hold x0498 x0499 x049A x049A (the reference OS's interrupt vectors). xFFFE (machine control register) reads x8000.
- After a HALT through the reference OS the registers are again the boot values (PC=x0494, PSR=x0400 ZERO, R1=x7FFF, R7=x0490, others 0): the HALT routine leaves them so.

## Instruction semantics

- `lc3sim`'s PSR keeps the condition code in bits 11:9 (N x0800, Z x0400, P x0200), not in bits 2:0 as in Patt and Patel; bit 15 and the priority field are never examined, and `register PSR <v>` stores arbitrary bits.

- LEA sets condition codes (result x3006 gives P, x8002 gives N).
- RTI (x8000), opcode 1101 (xD000), ADD or AND with bits [4:3] set and bit 5 clear (x1018, x5018), and TRAP with nonzero bits [11:8] (xF100) all behave identically: stdout `Illegal instruction at x<ADDR>!`, execution stops with PC on the offending word, IR set to it, PSR and R7 unchanged, exit status 0. There are no privilege modes, exception vectors or access-control checks.
- PC wraps modulo x10000. A stray jump into zeroed memory can reach xFFFE and stop there on RTI.
- `lc3sim` has no timeout: a looping program hangs the process.

## Devices and console

- `option device off` prints `Will not randomize device interactions.` and makes device status deterministic. Without it, keyboard and display readiness use `random()` seeded from `time(NULL)` (source only), so the OUT/PUTS wait loops spin a varying number of times.
- A display data write is dropped unless display status was read ready first (source only).
- Writing the machine control register with bit 15 clear halts after that instruction.
- With `option stdin off` (prints `Will not use stdin for LC-3 console input during script execution.`), GETC and IN consume the script lines that follow `continue`; leftover characters on a consumed line are then executed as a script command.
- When input is exhausted: stdout `LC-3 read past end of input stream.` and the process exits with status 3, with no halt message and no register dump.
- The reference OS's HALT writes `\n\n--- halting the LC-3 ---\n\n` after whatever the program printed, then `lc3sim` dumps registers (the R0-R7 line has a trailing space).
- Script `reset` is refused; `file` reloads only the program's own words; registers and other memory carry over between reloads.

## lc3as

- Built with `flex -i`: mnemonics and directives are case-insensitive (`halt`, `add`, `.fill` work). Label identity is case-insensitive.
- A word that is not a mnemonic or directive becomes a label silently (`NOP` emits nothing).
- Literals: `x`/`X` hex (negative allowed) and decimal with optional `#`; a bare `3000` is decimal.
- Values x8000-xFFFF are reinterpreted as negative for every field, so `ADD R0,R0,#16` assembles as imm5 = -16 with no message; `#-17` is an error.
- A second `.ORIG` prints `multiple .ORIG directives found` once per pass, is ignored, exits 0, and code continues contiguously.
- A missing `.END` is an error; text after `.END` is a warning.
- `.STRINGZ` allows embedded newlines and escapes `\a \b \e \f \n \r \t \v \\ \"`; any other `\c` stores `c`, so `\0` stores x30.
- On error, partial `.obj` and `.sym` files remain; a pass-1 failure leaves a zero-byte `.obj`.

## lc3convert (source only)

- Binary mode: one word per line, spaces ignored, `;` comments. A short line is an error but still written; a line over 16 digits is written from its first 16; any other character drops the line.
- Hex mode (`-b16`): `[xX]?-?hex+`, one value per line.
- In both modes the first emitted word is the origin.
