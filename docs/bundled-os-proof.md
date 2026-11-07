# Bundled OS: design and optimality proof

This document fixes the bundled OS's placement, gives every TRAP routine's exact cycle count, and proves each count optimal or bounds it. Source: `src/bundled_os.asm` (PUTSP's unrolled copies are emitted by `tools/bundled_os_generate.py`). Tests: `tests/bundled_os.rs`. Tools: `tools/bundled_os_search.py` and `tools/bundled_os_search2.py` (two independently written lower-bound searches; outputs in `docs/bundled-os-search-output.txt` and `docs/bundled-os-search2-output.txt`). The "Verification" section at the end records how each claim was checked.

## Results

N is the memory latency. Per-instruction costs, from the verified FSM table (`docs/fsm-state-sequences.md`):

| Symbol | Instructions | Cycles | N=1 | N=4 |
|---|---|---|---|---|
| A | ADD, AND, NOT, LEA, JMP/RET, BR not taken | 4+N | 5 | 8 |
| B | BR taken, JSR, JSRR | 5+N | 6 | 9 |
| M | LD, LDR, ST, STR, TRAP | 5+2N | 7 | 13 |
| I | LDI, STI | 6+3N | 9 | 18 |

A routine's cost runs from the TRAP fetch through the instruction that returns, or that halts.

| Routine | Formula | N=4 | N=1 | Status |
|---|---|---|---|---|
| GETC | M + I + A | 39 | 21 | optimal (argument and search) |
| OUT | M + I + A | 39 | 21 | optimal (argument and search) |
| IN | 7M + A | 99 | 54 | optimal (argument and search) |
| HALT | 19M + 11A | 335 | 188 | optimal (search) |
| PUTS, L = 0 | 4M + A + B | 69 | 39 | |
| PUTS, L >= 1 | see "PUTS" | 142 at L=1, 1270 at L=32, 7000 at L=200 | 79, 709, 3913 | worst-case gap to the per-length lower bound 59 over 0 ≤ L ≤ 1000 at N=4 (38 at N=1); best among every design examined, not proven against all designs |
| PUTSP | see "PUTSP" | 255 + per word (124-143, plus 17 per eight words) | 138 + per word (73-86, plus 11 per eight words) | best known; one class meets the proven per-word lower bound up to the amortized block cost |

No proven optimum differs between N=1 and N=4. Each argument below holds at both latencies, and the search runs at both.

## Memory layout

**Source checked.** Patt and Patel, *Introduction to Computing Systems*, 2nd edition, Appendix A. This is the publisher's typeset appendix, the PDF the University of Verona course site serves at https://docs.univr.it/documenti/OccorrenzaIns/matdid/matdid105426.pdf (sha256 `aa93391e33b11d5f51caa35129cbf85967624637b1a2f009605e9fdc4200b5d6`). The PDF metadata reads `Title: app-a`, `Subject: TeX output 2003.06.30:1147`. Each page footer is the typesetting stamp "app-a", 2003/6/30, page number (for example page 521, #1). That is the same stamp series as the 2nd edition pages that `docs/fsm-state-sequences.md` authenticates; the reproduction on the author's own site, https://users.ece.utexas.edu/~patt/17f.306/Exams/OldExams/f09_final.pdf, carries Table A.3 on its page stamped "app-a", 2003/6/30, page 543, #23. The edition is therefore the 2nd.

**Figure A.1, "Memory map of the LC-3"** (book page 521), as extracted from the PDF:

```
x0000 - x00FF   Trap Vector Table
x0100 - x01FF   Interrupt Vector Table
x0200 - x2FFF   Operating system and Supervisor Stack
x3000 - xFDFF   Available for user programs
xFE00 - xFFFF   Device register addresses
```

Page 522: "Addresses xFE00 through xFFFF have been allocated to represent the addresses of I/O devices. See Figure A.1." Section A.4 (page 543): "The starting address of the interrupt vector table is x0100." Table A.3, "Device Register Assignments" (page 543): KBSR xFE00, KBDR xFE02, DSR xFE04, DDR xFE06, MCR xFFFE ("Bit [15] is the clock enable bit. When cleared, instruction processing stops.").

**Constraint used in every bound below.** The contract follows this map: the OS places code and data only in x0000-x00FF (the trap vector table) and x0200-x2FFF. Nothing goes in x0100-x01FF or xFE00-xFFFF, and the image does not wrap. Consequences:

- **Image.** One `.ORIG x0200` image, x0200-x223C. The trap vector table is written by boot, which is uncounted (ADR 0004), so the image never covers x0100-x01FF.
- **Vector table.** It is written once, by boot, and never again during a run: 250 entries hold the UnknownTrap sentinel and six hold routine addresses, and the UnknownTrap mechanism (below) requires every unassigned vector to hold the sentinel throughout the run. So no routine may use a vector as a spill slot, even though an `STI` into a vector followed by a `TRAP` through it would be a cheaper return than `LD` plus `RET` (see IN). Executing any of its words is either illegal (xFFFE is a malformed TRAP, bits 11:8 = 1111) or one of six fixed BR/ADD/LD words that do nothing a routine could not do elsewhere. So all code is in x0200-x2FFF.
- **Device reach.** From any PC in x0200-x2FFF, LD/ST/LEA reach [PC-255, PC+256] ⊂ [x0101, x30FF]. No device register is PC-relative reachable, and no LEA or JSR result lies near xFE00.

**Word budget of x0200-x2FFF (11776 words).**

| Range | Words | Use |
|---|---|---|
| x0200 | 1 | `BRnzp BOOT` (boot entry) |
| x0201 + 256n + t, n = 0..31, t = 0..7 | 256 | PUTSP high-byte table (rows x0201-x2108) |
| x0209-x0232 | 42 | boot, GETC, OUT, IN, their constants and IN's spill slot |
| x0233-x0300 | 206 | free (end of gap 0) |
| x0320, x032D, x0368 | 3 | HALT's LEA address labels (contents unused) |
| x0369-x0386 | 30 | HALT and its base constant |
| rest of gap 1 | 215 | free |
| gaps after rows 2-26 | 6200 | free (25 gaps of 248 words) |
| x1D9B-x1E00 | 102 | PUTSP side blocks of copies 0-1 and an exit stub (end of gap 27) |
| x1E09-x1EEE | 230 | PUTSP side blocks of copies 2-3, entry, the eight-copy chain, pointer advance, constants, spill slots (x1EE0), exit stub (gap 28) |
| x1F09-x1FCC | 196 | PUTSP side blocks of copies 4-7 and an exit stub (gap 29) |
| rest of gaps 27-29, gap 30 | 464 | free (146, 18, 52 and 248 words) |
| x2109-x223C | 308 | PUTS (prologue, 31-word prefix, exits, slots and constants, switch, 64-word loop, epilogue) |
| x223D-x2FFF | 3523 | free (above the image) |

The image is x0200-x223C (8253 words). That is 1165 words of code and data, 3 address labels, and 10608 free words: 7085 inside the image and 3523 above it.

## General lemmas

**L1 (instruction floor).** Every instruction costs at least A. The only instructions that change PC other than by +1 are BR (A not taken, B taken), JMP/RET (A), JSR/JSRR (B), and TRAP (M). Every memory read or write instruction costs at least M.

**L2 (worst case over paths).** A routine's cost is its worst case over entry registers and console input. Unconditional jumps and branches can be deleted from any executed path without changing its data effects, which only lowers cost, so a lower bound may consider the straight-line sequence of data instructions on one path. That holds as long as the bound does not let the path exploit coincidences in entry values. For that, take entry registers "generic": no entry value, and nothing computed from entry values alone by the path's finitely many instructions, equals any constant the routine needs (a printed character's low byte, a device-window value, zero). Such a choice exists because each coincidence excludes finitely many values of a 16-bit register. On the generic path, values derived only from entry registers are useless except as copies of themselves. Branching on data instead (to learn a value and then rebuild it with constants) needs at least 8 conditional branches on some path to rebuild a byte, since 256 cases need a binary decision tree of depth 8. That costs 8A > 2M (32 + 8N > 10 + 4N), more than the memory round trip it would replace. This is the case for IN's input byte below, and it holds at every N.

**L3 (device access).** Because of the layout constraint, a KBDR read, DDR write or MCR write is either LDI/STI through a pointer cell (I) or LDR/STR off a base register (M plus the cost of creating the base). The base window for DDR is [xFDE7, xFE26] (xFE06 within offset6 -32..31), for KBDR [xFDE3, xFE22], and for MCR [xFFDE, x001E]. The last includes x0000: `STR R0,R0,#-2` with R0 = 0 writes 0 to xFFFE.

**L4 (window constants).** A register value in the DDR or KBDR window can only come from (a) a load (at least M), (b) NOT of a value in [x01D9, x0218], or (c) an ALU op whose operand already lies in or near the window. Proof: on the generic path, constants come from LEA, JSR, TRAP (values in [x0101, x3000], L3's reach), AND #0 (x0000), loads, or ALU ops on earlier constants. One ALU op on such a constant reaches the window only by NOT of [x01D9, x0218]. ADD/AND with immediates or with two such constants stay within [x0000, x61FE] or below their operands. Among values in [x01D9, x0218], the only one whose low byte is a character a routine prints is x020A, a linefeed (low byte x0A); its complement xFDF5 is a base whose low byte, xF5, is not printed. So creating a base costs a load (M), or an LEA plus NOT (2A > M), or one NOT of an x020A linefeed constant that was needed anyway (A, but the result's low byte is not a printed character).

**L5 (carries go up).** For ADD, AND and NOT, bit i of the result depends only on bits 0..i of the operands. So the low byte of any ALU result depends only on the operands' low bytes, and a value's high byte can reach a register's low byte only through control flow (branches), a load from a data-dependent address, or a data-dependent jump.

**L6 (round trips).** If at some instant the values that must survive (tokens such as R7's return address, an input byte, and the entry values of R1-R6) outnumber the registers free to hold them, the excess must go to memory and come back: a store and a load, 2M each. An ALU "packing" of two tokens into one register cannot replace this. By L5, final low bytes after the instant are functions of the low bytes at the instant (absent loads of stored values and absent branches). One low byte fewer than needed cannot carry them, and branching to rebuild a byte costs at least 8A > 2M (L2). Executing a stored word as an instruction to restore a value costs a store (M), plus the executed instruction (A), plus building a word with an opcode around the value (A): at least M + 2A > 2M. Using one of R1-R6 as scratch costs at least a copy and a restore (2A) or a store and a load (2M). The spill slot is an ordinary data word: a spill of the return address by `STI` into a trap vector, returned through by `TRAP` (I + M, cheaper than ST + LD + RET by 3 at every N), is excluded by the vector-table rule in "Memory layout".

**L7 (register-register ALU ops add nothing).** An ADD or AND of two registers needs both operands present. By L4 a device-window base needs a NOT of a value in [x01D9, x0218] among its operands, since sums of two LEA-span values stay in [x0202, x61FE] and ANDs stay at or below x30FF, neither of which meets a window; so a register-register base costs at least NOT + ADD = 2A on top of operands that exist for other reasons, and 2A > M, so a load dominates. A non-window value with a chosen low byte costs one LEA, and zero costs one `AND #0`. Hence the searches may omit register-register ALU steps without raising any minimum.

## Search model

`tools/bundled_os_search.py` runs Dijkstra over an abstract machine. Its states are: the device events done so far; the class of each tracked register (R0, R7, and one or two of R1-R6 as needed); and the tokens currently stored in memory. Classes: useless; a token; zero; another constant; a constant with a printed low byte; a pre-range constant (L4 (b)); and a window base with a printed or other low byte. The steps and their costs are listed in the tool's docstring.

Each step is one instruction at the cheapest cost L1 allows for its effect. Base creation follows L4. Device events follow L3. Tokens are atomic by L6, and token spills and reloads are M each (a spill is a store to a data word, never to a trap vector, by the vector-table rule). Branches are absent by L2. Register-register ALU steps are absent by L7. Every class transition a real instruction can make is a step, or is dominated by one. The model over-approximates in two ways: any ADD from a base may reach any base, and every linefeed constant counts as pre-range. Both only lower the minimum, so the minimum is a lower bound.

Tracking only some of R1-R6 is complete because each additional scratch register costs at least 2A (L6), and the minimum's non-forced cost leaves no room for one more:

- GETC and OUT: slack 5 (N=4) or 2 (N=1), against 2A for one extra register. The search still tracks one.
- IN: tracks two. A third costs at least 6A, more than the 3M of non-forced cost at the minimum.
- HALT: tracks one. Two extra registers add at least 4A to the 10 forced writes (14A), more than the minimum's M + 11A at both latencies.

The search output (`docs/bundled-os-search-output.txt`, regenerated with `python3 tools/bundled_os_search.py`, about a minute) finds, at both latencies, minima equal to the bundled OS's tested costs: 39/21, 39/21, 99/54, 335/188. It asserts this equality.

A second search, `tools/bundled_os_search2.py`, was written independently of the first and shares no code with it. Its abstract machine is wider: it tracks zero and the MCR window exactly (split on bit 15, since the halting store needs data with bit 15 clear), NOT between the pre-range and the windows in both directions, KBDR-only, DDR-only and shared windows, LDI/STI paths without a base, copies, spills and reloads, an optional third tracked register whose entry value must be restored (`--regs 3`), and an optional spill into the trap vector table with a TRAP return (`--vector`, to measure what the vector-table rule costs). Its over-approximations (ALU immediates may set any low byte of interest; loads may produce any window value) only lower its minima. Its output (`docs/bundled-os-search2-output.txt`) reaches the same minima as the first search in every mode that respects the vector-table rule, and 96/51 for IN in vector mode (see IN). The three-register HALT space is about ten million states and is argued rather than searched (see HALT).

## GETC: M + I + A (39 / 21)

`LDI R0,KBDR_PTR; RET`.

Forced: the TRAP (M), exactly one KBDR read into R0 (a second read would consume input the reference OS leaves), and a return through R7 (at least A, L1). The read is LDI (I), or LDR off a base, which costs at least 2M once the base's load is counted (L4). Since I < 2M at every N (6 + 3N < 10 + 4N), the bound M + I + A is met.

## OUT: M + I + A (39 / 21)

`STI R0,DDR_PTR; RET`. The argument is GETC's with one DDR store.

## IN: 7M + A (99 / 54)

```
ST R7,SAVE_R7     LD R7,DDR_NL      LDR R0,R7,#-8    STR R0,R7,#-4
STR R7,R7,#-4     LD R7,SAVE_R7     RET
```

DDR_NL = xFE0A is a base for KBDR (offset -8) and DDR (offset -4), and its low byte is the linefeed.

Lower bound:

- Forced: TRAP (M), one KBDR read, two DDR stores (the byte, then x0A), RET (A).
- Some register must have low byte x0A at the second store. At that instant the input byte (for R0), the return address (for R7) and the linefeed value are all live. Two free registers cannot hold three values, so L6 forces a round trip (2M).
- If no device access uses a base, the cost is at least M + 3I + A + A (making the linefeed) + 2M = 3M + 3I + 2A = 109 / 58.
- With a base, the base costs at least M (L4). If the base is not also the linefeed, four values (byte, return address, linefeed, base) are live at the store, which needs a second round trip, or else the store uses STI. Both cost more.
- The cheapest case is a base that is also the linefeed (xFE0A, a load): M + 3M + A + M + 2M = 7M + A.

Both searches confirm 99 and 54.

The round trip is charged as a store and a load of a data word (2M) followed by RET (A). Writing the return address into an unassigned trap vector instead, `STI R7,VEC00_PTR` (I) with `TRAP x00` as the return (M), costs 6M + I = 96 / 51, three cycles less at every N, and gives the contract's output, R0 and R1-R6. It is excluded by the vector-table rule: after the first call that vector would hold a return address instead of the sentinel, so a later erroneous `TRAP x00` would jump there instead of failing with UnknownTrap. No assigned vector can be used either (overwriting x23 breaks the next IN call). Under the rule, 7M + A is the minimum; without it, 6M + I is (`bundled_os_search2.py --vector`).

## HALT: 19M + 11A (335 / 188)

R7 = xFE0A is the DDR base and prints both linefeeds. R0 carries the other characters, through three LEAs of address labels chosen for their low bytes and ADD steps between neighbors. Then `AND R0,R0,#0; STR R0,R0,#-2` halts with R0 = x0000, as the reference OS leaves R0 (its HALT computes x8000 AND x7FFF and stores it). R1-R6 are untouched.

Lower bound. Forced: TRAP (M), 16 DDR stores and one MCR store (at least M each, L3), and R0 = 0 at the halting store. The register writes form a two-register caching problem over the request sequence `\n - - - sp h a l t e d sp - - - \n`, plus a final zero in R0, plus at least one base if stores are to cost M rather than I. Nine distinct characters plus zero give at least 10 writes. The search finds the exact minimum, M + 11A for the writes (one load for the base-and-linefeed, eleven register writes), so 19M + 11A. Cheaper-looking options exist but lose: deriving a base by NOT of a linefeed constant (L4) costs A but yields a base that prints nothing, and STI stores cost I each.

Scratch registers. Both searches track R0, R7 and at most one of R1-R6; the three-working-register case is argued, not searched (its state space is about ten million states). A third register is usable only while its entry value is parked elsewhere. Parking it in R0 or R7 by a copy leaves two working registers, so nothing changes; parking it in memory costs a spill and a reload, 2M. With three working registers and the base in R7, the best write count for the request sequence is 10 instead of 11 (keep ' ' or '-' across "halted"), saving one A, and a third register cannot make the base cheaper (LEA + NOT = 2A > M regardless of register count). So every design that uses R1-R6 costs at least 19M + 11A + 2M - A > 19M + 11A at both latencies.

## PUTS

### Objective

The objective is a hybrid: minimize the worst case over 0 ≤ L ≤ 1000 of gap(L) = T(L) - LB(L) at N=4, where LB is the per-length lower bound below; ties go to the smaller gap at short lengths. Earlier objectives ("marginal cost first", then "the 64-word loop for realistic lengths") were replaced by it.

### Design (shipped): base prefix, then the 64-word stride loop

1. **Entry.** Spill R7 and load word 0; an empty string exits on this taken BRz.
2. **Prefix.** Spill R1 and load the DDR base xFE0A, then print words 0-30 at LDR offsets 1-31 from the unmoved R0. The check of word 31 is a taken `BRnp` over a block holding the prefix exit (`LD R1; LD R7; RET`) and all of PUTS's slots and constants. A 31-word string falls through into that exit.
3. **Switch.** Word 31 is non-zero: spill R0 and R2, load R2 = 64, and fall into the loop.
4. **Loop.** The 64-word stride loop, rotated to start at the store of word 31 + 64m. Its ADD comes before words 32 + 64m, it reads offsets -32..31, and the check of word 95 + 64m is the taken back edge. Exits go to `LD R2; LD R0; LD R1; LD R7; RET`.

Why the switch is at 31 and the block sits where it does:

- **Switch point.** 31 is the largest LDR offset, so the prefix cannot run longer. Switching earlier leaves a band of lengths that pay the stride setup without needing it (see P3).
- **Block placement.** The routine spans 308 words, but the prologue's spills and the epilogue's restores must reach the same slots (LD/ST reach ±256). So the slots go between the phases, and passing over them costs one taken branch for strings of 32 or more words.
- **Location.** PUTS is too long for a 248-word table gap, so it sits above the table.

### Exact cost

```
T(0)  = 4M + A + B
T(L)  = 7M + 2A + 1 + L(2M + A)                                    1 ≤ L ≤ 30
T(31) = 7M + 2A + 31(2M + A)
T(L)  = 12M + 2A + 1 + L(2M + A) + A*floor((L + 32) / 64)
        + floor((L - 32) / 64) + [64 does not divide L - 31]        L ≥ 32
```

| L | 0 | 1 | 31 | 32 | 64 | 200 |
|---|---|---|---|---|---|---|
| N=4 | 69 | 142 | 1161 | 1270 | 2358 | 7000 |
| N=1 | 39 | 79 | 648 | 709 | 1317 | 3913 |

`tests/bundled_os.rs` checks 19 lengths at both latencies: 0, 1, 2, 7, 8, 30, 31, 32, 33, 63, 64, 65, 94, 95, 96, 159, 160, 200, 500.

### P1. Per-length lower bound (all fitting designs)

For every L ≥ 0:

```
LB(L) = 4M + 2A + L(2M + A) + min(L(N + 1), 3M)
        + [L ≥ 32] min(2M + A*ceil((L - 31) / 64), 2A*ceil((L - 31) / 15))
```

so LB(0) = 4M + 2A. A design whose empty-string check falls through (`ST R7; LDR R7,R0,#0; BRnp NONEMPTY; LD R7; RET`) meets it. An earlier form of this bound stated LB(0) = 4M + A + B, which no design is bound by, and charged the last term as 2M + A·ceil((L - 31)/64) alone, which design X below beats (see "Verification"). The terms:

- TRAP (M) and RET (A).
- L + 1 loads of string words (LDR, M) and L + 1 word-dependent instructions (at least A each; P2 step 1).
- L DDR stores, each STR off a base (M) or STI (I = M + N + 1).
- The word needs a register while R7 holds the return address and R0 the string start, so one value makes a round trip (2M, L6).
- Using a base costs its creation (M, L4) and another register's round trip (2M), so stores cost at least min(L(N+1), 3M) more than all-STR.
- 33 or more words cannot be reached from R0 with offsets 0..31, so the pointer must move. Two routes exist. Either the moved pointer is restored by a load, which costs a pointer write per further 64 words (A each, one base value covers 64 words) and a second preserved value (2M); or the pointer is moved by immediate ADDs and restored by immediate ADDs, which costs no round trip but two writes per further 15 words (an imm5 reaches at most 15 further words with offsets 0..31 already in use, and each move must be undone), 2A·ceil((L - 31)/15). An ADD of -16 does not reduce the count below ceil((L - 31)/15) for the lengths where this route is cheaper. The bound takes the smaller route; at N=4 the immediate route is smaller for 32 ≤ L ≤ 61 (2A = 16 and 4A = 32 against 2M + A = 34), at N=1 for 32 ≤ L ≤ 46 (2A = 10 against 19).

LB holds numerically against every design analyzed, at every L up to 5000. It charges no loop closure (back edges), which is why every finite-loop design's gap grows slowly with L. It is also loose by up to 3M for L ≥ 62: a stride register's save, restore and load, which every 64-word loop pays because 64 is neither an imm5 nor LEA-reachable; no design found exploits either looseness.

### Gap of the shipped design

```
gap(0)  = 1
gap(L)  = 3M + 1 - min(L(N + 1), 3M)                                1 ≤ L ≤ 30
gap(31) = 0
gap(L)  = 3M + 1 + floor((L - 32) / 64) + [64 does not divide L - 31]
          + max(0, 2M + A*ceil((L - 31) / 64) - 2A*ceil((L - 31) / 15))      L ≥ 32
```

floor((L + 32)/64) = ceil((L - 31)/64) cancels the load-route ADD term exactly. The gap for L ≥ 32 is the stride register's save, restore and load (3M), the taken branch over the slot block (1), the back edges LB does not charge, and, where LB takes the immediate route, the difference between the two routes: 18 for 32 ≤ L ≤ 46 and 2 for 47 ≤ L ≤ 61 at N=4; 9 for 32 ≤ L ≤ 46 at N=1; zero elsewhere.

| L | 0 | 1 | 2 | 5 | 7 | 8 | 10 | 30 | 31 | 32 | 46 | 47 | 61 | 64 | 95 | 96 | 100 | 200 | 1000 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| N=4 | 1 | 35 | 30 | 15 | 5 | 1 | 1 | 1 | 0 | 59 | 59 | 43 | 43 | 41 | 40 | 42 | 42 | 43 | 56 |
| N=1 | 1 | 20 | 18 | 12 | 8 | 6 | 2 | 1 | 0 | 32 | 32 | 23 | 23 | 23 | 22 | 24 | 24 | 25 | 38 |

Worst case over 0 ≤ L ≤ 1000: **59 at N=4** (every L in 32..46) and **38 at N=1** (L = 992). Over L ≤ 100 it is 59 (N=4) and 32 (N=1). Over every possible length (L ≤ 65535) the back edges accumulate to 1064 (1046).

The empty string's gap of 1 comes from the tie-break, which makes the empty string pay the taken entry branch. The alternative (`BRnp` to the non-empty path) would give gap(0) = 0 but add the taken cycle to every L ≥ 1, raising the worst case to 60 (39 at N=1) and every short-length gap by one.

### P2. Slope

These steps are unchanged in substance from the analysis of the plain 64-word design.

1. **Per character, at least 2M + A.** Each word needs an LDR (M), a DDR store (at least M), and an instruction whose behavior depends on that word before its byte is stored (at least A).
2. **Per 64 characters, at least one more A.** Each LDR base value covers at most 64 words (offset6), and each new base value is a register write.
3. **The shipped loop's slope** is 2M + A + (A + 1)/64: 34.1406 (N=4), 19.0938 (N=1), within 1/64 cycle per character of the floor 2M + A + A/64.
4. **Single-branch loops.** Among loops closed by one taken conditional branch, a 64-word body is optimal: the body must fit the branch's 256-word reach, so U ≤ 84, and (A·ceil(U/64) + 1)/U is least at U = 64.

### P3. The switch point is best within the hybrid classes (N=4 and N=1)

Classes analyzed, with costs derived from the cycle table:

- **H(j, k).** j words by STI with nothing set up, then words j..k-1 off the unmoved R0 with a base, then the 64-word stride loop (0 ≤ j ≤ k ≤ 31).
- **H(0, 0)** is the plain 64-word design.
- **The maximal unroll.**
- **The ADD-#15 design.**
- **STI with ADD-#15 blocks.**
- **STI with offsets only** (L ≤ 31).

Worst-case gap over L ≤ 100 at N=4 (N=1), measured against the load route of LB alone (the form of the bound before the immediate route was added; the corrected bound raises every design's gap at 32 ≤ L ≤ 61 by the same route difference, so the ranking is unchanged):

| Design | Worst gap, L ≤ 100 |
|---|---|
| shipped | 42 (24) |
| H(0, 31) ignoring code reach | 41 (23) |
| H(j, k), every other (j, k) | at least 46 (25) when j > 0; at least 66 (36) when k < 31 |
| plain 64-word | 100 (55) |
| ADD-#15 | 61 (34) |
| maximal unroll | 134 (74) |
| STI with ADD-#15 blocks | 500 (206) |

Why each alternative loses:

- **k < 31.** Lengths k < L ≤ 31 pay the 5M stride-and-R0 setup that LB does not require below 32 words: gap 5M + 1 = 66 at N=4.
- **j > 0.** Every length beyond j pays I - M = N + 1 per STI word, raising the L ≥ 32 gaps.
- **The 1-cycle loss against the idealized H(0, 31)** is forced by reach in this class. Between the prologue's spills (R7, R1) and the epilogue's restores there are more than 256 words of straight-line code, so their slots can only sit inside it, which costs every longer string a taken branch over them. The alternatives cost more: restores through pointers cost 2(N + 1) more on long exits, and a jump back from the far end is out of branch reach or costs M + A.
- **N=1.** The best switch point is the same over L ≤ 100. Over L ≤ 1000 at N=1, switch points k < 31 tie with k = 31 at the growth-dominated maximum (37 idealized). The tie-break by short-length gaps picks k = 31, because k < 31 leaves a 36-cycle band.

With the literal objective over all L ≤ 65535, the maximal unroll's slower back-edge growth gives it the smaller worst case (570 against 1064). The plain 64-word design is 1 lower than the shipped one (1063), because of the jump-over cycle. That end of the range is set by loop closure, which no switch point changes, so the switch point is chosen on L ≤ 100 (and is the same on L ≤ 1000).

### What is and is not proven

**Proven:**

- LB(L) holds for every fitting design at every L ≥ 0 (P1, with the immediate route in its last term; the earlier load-only form was refuted by design X at 32 ≤ L ≤ 61).
- The shipped design's exact gap at every L is the closed form above; the formula was checked by measurement at every L in 0..1000 at both latencies.
- Its loop slope is within 1/64 of the universal floor and optimal among single-branch loops (P2).
- Within the classes in P3 and among the immediate-ADD block designs X, X2, Z1 and Z2 below, no design has a smaller worst-case gap over 0 ≤ L ≤ 1000, at either latency. The exceptions are the idealized H(0, 31), which this layout cannot realize, and the N=1 tie noted in P3.

**Not proven:**

- That no fitting design outside those classes has a smaller worst-case gap. Untried shapes include a different prefix register allocation, a loop that keeps the string start recoverable without a spill, or a closure structure LB does not charge.
- That LB is tight: it omits loop closure and the stride register's 3M, and its short-length STI branch is met only by the STI design.

### Superseded and rejected designs

| Design | Why it was not kept |
|---|---|
| ADD-#15 unroll | worst gap 61 (34) over L ≤ 100 |
| plain 64-word loop with stride register | worst gap 100 at L = 1 |
| maximal unroll, 1216 words per iteration with a register back edge, slope 2M + A + 20A/1216 (34.13158 / 19.08224) | dearer for every L below 4865 (N=4) and 2369 (N=1) |
| design X: shipped prefix, then one immediate-ADD block (ADD R0,R0,#15; words 32-46 at offsets 17-31; exits undo the ADD), then the stride loop with R0 + 15 spilled | faster than the shipped design for 32 ≤ L ≤ 61 at N=4 (1212 against 1270 at L = 32) but its loop windows start at 47 + 64m while LB charges one pointer write per window 32 + 64m, so it executes one ADD more than LB charges for 49 of every 64 lengths, plus the undo: worst gap 71 (47) at L = 943 |
| design X2: as X with a second block for words 47-61 | worst gap 87 (57) at L = 958 |
| design Z1: as X but R0 is spilled before the block, so the loop exit reloads instead of undoing | worst gap 63 (42) at L = 943; over L ≤ 100 alone it would win at N=4 (51 against 59), which the objective over 0..1000 does not reward |
| design Z2: as Z1 with two blocks | worst gap 71 (47) at L = 958 |

Worst gaps are over 0 ≤ L ≤ 1000 against the corrected LB, N=4 (N=1). Every immediate-ADD block loses at the long end for the same reason: realigning the loop windows (ADD #-15 before the loop) costs the same A on every L ≥ 47, and covering 32..95 by immediates before the loop costs 4A extra on every L ≥ 92. So within "prefix, immediates, stride loop" the switch at 31 is best for the maximum over 0..1000 at both latencies.

The maximal unroll is slope-optimal among all designs that fit with PUTSP's table; that argument, from the run structure and branch reach, is not reproduced here.

## PUTSP

### Design (best known)

- **Low byte.** For each word W: `LDR` (its sign is W[15]), then `AND R5,R7,R2` (low-byte NUL test) and `BRz`, then `STR` of W to print its low byte.
- **High byte.** A branch tree on W[15:13] (`ADD R5,R7,R7` and `ADD R5,R5,R5` put W[14] and W[13] in the sign). In each of the 8 leaves: `AND R7,R7,R3` (x1F00), `ADD R7,R7,R4` (table base), `LDR R7,R7,#t` gives h from the table, then `STR`. Only leaf 000 needs `BRz` for h = 0.
- **Unrolling.** The word code is copied eight times over `LDR R7,R0,#k` (k = 0..7); after copy 7, `ADD R0,R0,#8; BRnzp WORD_0` advances the pointer, so the pointer write and the back edge cost A + B per eight words instead of A per word.
- **Fall-through class.** Each copy's main chain (13 words: `LDR; BRn; AND; BRz; STR; ADD 2W; BRzp; ADD 4W; BRzp; AND; ADD; LDR #3; STR`) is the all-fallthrough path for class 011 (h = x60-x7F, lower-case ASCII): its branches are not taken and its leaf falls into the next copy. The branch directions are the reverse of the natural tree so that this class pays no taken branch and no back edge. The other seven leaves of copy k sit in a side block (47 words) and end with `BRnzp WORD_{k+1}`.
- **Table.** HIGH_TABLE + (W & x1F00) + W[15:13] holds h; rows sit at x0201 + 256n.
- **Placement.** Branch reach (256 words) limits the unroll: copy k's side block must lie within reach of its chain sites and of copy k + 1, and with 47 side words per copy about four copies fit on each side of a contiguous chain, so eight copies is the largest unroll this geometry admits (twelve and sixteen were checked and do not fit; splitting the chain costs a taken branch per split, which cancels the gain). The chain sits in gap 28 with copies 2-3's side blocks, the entry and the constants; copies 0-1's side blocks are in gap 27 and copies 4-7's in gap 29. The spill slots sit at x1EE0, within offset6 of the row mask R3 = x1F00, so the eight-word exit stub (seven `LDR` off R3, `RET`) can be replicated wherever a `BRz` needs one within reach (three stubs). The entry's `ST`s and `LD`s fall straight into copy 0; the table base is loaded (`LD`, M) rather than `LEA` because the table is out of LEA reach from gap 28.

Fixed cost: 19M + A (255 / 138), covering the TRAP, the saves of R0-R5 and R7, four constant loads, the seven restores and RET.

Per word (both bytes printed), by t = W[15:13]: t=0: 4M + 8A + 3B; t=1, 2 and 4: 4M + 8A + 2B; t=3: 4M + 9A; t=5 and 6: 4M + 7A + 3B; t=7: 4M + 6A + 4B. At N=4 that is 143, 134, 134, 124, 134, 135, 135, 136; at N=1 86, 80, 80, 73, 80, 81, 81, 82. Plus (A + B) per eight words, paid after words 7, 15, ... even when the word that follows terminates: 17 / 11, that is 2.125 / 1.375 per word amortized. The asymmetry between t=0 (143) and t=1 (134) is a choice of which leaf of the `L00` node falls through; swapping them gives 142 and 135.

Terminators:

| Terminating word | Cost |
|---|---|
| Low NUL, W[15] = 0 | M + 2A + B |
| Low NUL, W[15] = 1 | M + A + 2B |
| h = 0 after printing the low byte | 3M + 7A + 3B |

### Proven lower bound

Per word printing both bytes, with h ≥ x20: at least **4M + 9A** (124 / 73) plus A/64 amortized for the pointer and, in a loop, the back edge's share. With 0 < h < x20: at least 4M + 10A. Class 011 of the design meets 4M + 9A exactly, up to the amortized (A + B)/8 per word; the other classes pay their taken tree branches and their rejoin branch (section "Worst class" below).

1. One load of W (M) and two DDR stores (at least M each, L3).
2. **Table lookup costs at least 2A + M.** By L5, h reaches a printed low byte only through branches or data-dependent addresses. A table address derived from W carries h-information only in bits 8-15 (L5), so a table that distinguishes 2^k values of h spans at least 2^k pages. The allowed region has 46 pages (x02-x2F), and the vector-table page is fixed data. So k ≤ 5, achieved only by bits 8-12 (32 pages); any other bit set spans more than 46 pages. Building the address needs at least two ALU ops: one to remove W's other bits (AND), and one to move pages x00-x01 into the region, since no single op does both. That is 2A, then the table load (M).
   - Sharper: by L5 the address bits 15:13 can only be influenced by W[15:13], and an in-region address (x0200-x2FFF) has bits 15:14 = 00, so the address can carry at most W[13] of the top three bits. A table carrying W[13] plus k bits of W[12:8] spans 32 + 2^k pages: with k = 4 that is 48 pages, two more than the 46 available, whether the offset is added, subtracted or wrapped; with k = 3 (40 pages, fits) the tree still needs four levels. So a three-level tree needs all five of W[12:8] in the address, and no table saves a level.
3. **At least 4A of branches and 3A of CC-setting writes.** The bits W[15:13] (8 cases) and the low-byte NUL decision (2 cases) must be resolved by control transfers: 16 outcomes need at least 4 two-way branches on the worst path (a data-dependent JMP costs A like a not-taken branch and its target needs a write of its own). Three W-dependent writes are needed on every both-byte path, not the two that a count of 3-way outcomes (2·3^s ≥ 16) would allow:
   - NUL (W[7:0] = 0) can only be decided by a Z outcome whose zero set is exactly the NUL words of the current path set. `ADD` of two registers, `ADD` with an immediate and `NOT` have singleton zero sets (one W value), and N or P of an `ADD` is a threshold on W[14:0], which mixes NUL and non-NUL words in any interval longer than one. So the NUL write is an `AND` with a mask whose low byte is xFF, and its N outcome is W[15] AND mask[15].
   - Once W[15] is known (from the LDR's condition codes, or from that AND itself), the AND's N outcome is constant, so it decides NUL and nothing else; a mask with further unknown bits makes P ambiguous for both decisions.
   - The only three-way AND is one on a path where W[15] is still unknown (mask x80FF: N = W[15], Z = NUL, P = the rest). It replaces the free W[15] branch after the LDR; it never decides W[14] or W[13].
   - W[14] and W[13] are decided by the sign bits of distinct writes (2W, 4W, or AND masks whose result bit 15 is constant, hence two-way), or by a data-dependent JMP whose target needs its own write. Table-address writes (`AND x1F00`, `ADD base`) carry no W[15:13] or NUL information in their condition codes.
   So 2A for the address, 3A for the decisions and 4A for the transfers: 9A per word.
4. The pointer advance costs at least A/64 per word (PUTS, P2 step 2).
5. For 0 < h < x20 (t = 0), "h = 0?" is one more decision: at least one more A.

**Worst class.** In a binary tree with eight distinct leaves, each internal node has one taken edge, so the sum over leaves of taken tree branches is 12 and the worst leaf has three; every leaf but the one that falls through into the next word also pays a rejoin (B, or A via `JMP Rk` at the price of keeping Rk current, which costs A per word once the target changes per copy). Unrolled, the class costs are therefore 4M + 9A once, 4M + 8A + 2B three times, 4M + 7A + 3B three times and 4M + 6A + 4B once, plus A for t = 0; the design meets this distribution exactly (with the t = 0 leaf placed on the two-taken-branch path of the `L00` node). What remains unproven is a bound on the fixed cost and on the amortized block cost: the design pays (A + B)/8 per word for the pointer and the back edge where the bound charges A/64.

### Design space explored

| Design | Per word (N=4) | Why not chosen |
|---|---|---|
| Reference-style shift loop (8 rotate steps per high byte) | more than 280 | slowest |
| Full 8-level branch tree, LEA leaves, no table (about 1300 words) | about 204 | slower than the tree plus table |
| 3-level tree plus 32-row table, one pointer ADD per word, natural branch directions (the earlier design) | 133-149 plus the per-word ADD | superseded by the unrolled design: 124-143 plus 2.125 amortized, fixed 255 instead of 259 |
| 3-level tree plus 32-row table, unrolled eight times (shipped) | 124-143 | |
| 2-level tree plus 64-row table | | does not fit the region (64 pages > 46), and no table can carry W[13] with four more bits (48 pages > 46) |
| 4-level tree plus 16-row table | about 16 more per word | slower; the space it frees is not needed |

Rejected alternatives, with the reason each cannot beat the design:

- **Fold the low-byte NUL test into the tree's condition-code settings.** The outcome count 2·3^s ≥ 16 allows two writes, but step 3 shows the NUL decision needs an AND whose only free extra outcome is W[15], which the LDR already supplies; no encoding with two writes keeps the NUL decision exact on every path. The third write is forced.
- **Larger or reshaped tables.** A table address can carry at most W[13] of the top three bits and, with W[13], at most three of W[12:8] (40 pages), which trades one branch level for nothing. Tables keyed by shifted copies (2W, 4W) have strides 512 or 1024, so 32 rows do not fit. A per-leaf single-op table (address W + c, no AND, 8192 words, saving A for one class) is incompatible with the row table: its 255-word runs overlap the eight-word rows of every page it covers, and the other seven leaves still need their rows.
- **Computed jumps.** A JMP target obeys the same L5 constraint as a load address (it carries at most W[13] and some of W[12:8]) and needs a write of its own; JMP costs A like a not-taken branch, so it cannot beat the tree. A `JMP Rk` rejoin (A instead of B) needs Rk = the next copy's entry, which changes per copy, so keeping it current costs A per word.
- **Self-modifying code.** Any patch is a store (M > A); nothing per word is cheaper than the ADD it would replace, and the pointer ADD is already amortized over eight words.
- **A larger unroll.** Twelve and sixteen copies do not fit the reach geometry (47 side words per copy within 256 words of both the copy's chain sites and the next copy's entry allow about four copies on each side of a contiguous chain); splitting the chain costs B per split, which cancels the gain. Seven copies with the chain in gaps 0-1 would allow `LEA` for the table base (fixed 250) but cost 0.3 more per word, so eight copies with `LD` (fixed 255) was kept.
- **Fewer saved registers.** Seven live registers are needed (pointer, W, DDR base, NUL mask, row mask, table base, scratch) and no two can merge: the NUL mask must be x00FF-like on the W[15] = 1 path, and AND-after-ADD address forms hit page x00 for some row. This is a design argument, not a proven bound on the fixed cost.

## UnknownTrap mechanism

- **Sentinel.** Every unassigned trap vector (x00-x1F and x26-xFF) holds **xFFFE**, written by boot. xFFFE is the machine control register. It can never be a routine address in this layout (the OS uses only x0200-x2FFF), and fetching it reads x8000 (RTI). So under a machine without the rule, a jump there fails with `IllegalInstruction { pc: xFFFE, word: x8000, kind: Rti }` rather than running anything.
- **Rule.** It applies only when the run's OS is the bundled OS; `Prepared` records whether the OS image equals the bundled image, origin and words. When the machine executes a TRAP:
  1. The ordinary cycle-cap check for the TRAP's 5+2N cost runs first, so `CycleCapReached` takes precedence.
  2. The machine reads the vector-table word M[ZEXT(IR[7:0])] (state 28).
  3. If that word equals xFFFE, the run fails with `UnknownTrap { vector: IR[7:0] }` instead of loading PC. The vector is exact because it comes from the executing TRAP word.
- **Why gated.** UnknownTrap is raised only by the bundled OS. A user who pokes xFFFE into a vector under another OS keeps that OS's behavior (the RTI failure above). Under the bundled OS, the same poke reports UnknownTrap, which is the intended meaning.
- **Test.** `tests/bundled_os.rs` checks the table contents after boot: 250 entries of xFFFE, six distinct routine addresses in x0200-x2FFF, x0100-x01FF untouched.
- **Invariant.** The rule only works because no routine writes the table after boot; the IN section shows the three cycles that this rule costs.

## Reproducing

```
python3 tools/bundled_os_generate.py src/bundled_os.asm      # regenerates the source in place (a no-op on a current source)
scripts/capture-bundled-os-fixture <path to native lc3as>   # assembles with no diagnostics
cargo test --test bundled_os                                 # contract and exact cycles at N=1 and N=4
python3 tools/bundled_os_search.py                           # lower-bound search; asserts minima equal the tested costs
python3 tools/bundled_os_search2.py --regs 2                 # independent search (about a minute); --vector and --regs 3 as in its docstring
```

## Verification

Every claim above was re-derived independently of its original author, with the aim of breaking it: each lower-bound argument was either accepted with a fresh justification or refuted by a concrete design measured through the simulator at N=1 and N=4; each search was rerun and a second search was written from scratch; and the design space of every routine was attacked within the contract (output and R0 equal to the reference OS, R1-R6 preserved, the layout rule, and the PUTS objective). Verdicts are **accepted**, **refuted and corrected**, **improved**, or **open** with the remaining gap.

### GETC and OUT: accepted

M + I + A (39 / 21). TRAP (M), one device access into or out of R0 (I; an LDR or STR needs a base that costs at least A to create, and A + M > I at every N: 9 + 3N against 6 + 3N), and a return (A, nothing cheaper exists). Both searches find 21 / 39 in every mode, including with a third tracked register and with the vector-table spill allowed.

### IN: accepted under the vector-table rule, which is now stated

7M + A (99 / 54). The argument's hidden assumption was that the round trip of the return address goes through a data word. With an `STI` into an unassigned trap vector and a `TRAP` through it as the return, IN costs 6M + I (96 / 51), measured, with the contract's output, R0 and R1-R6; the second search in vector mode finds exactly that minimum. The design is excluded because UnknownTrap requires every unassigned vector to hold the sentinel for the whole run, which is now written into "Memory layout" as part of the contract, into L6, and into the search model's spill step. Under the rule the routine is optimal: both searches agree at 99 / 54, including with a third register.

### HALT: accepted

19M + 11A (335 / 188). Eighteen forced memory instructions (TRAP, sixteen DDR stores, the MCR store), one base load (LEA + NOT costs 2A > M and yields a base that prints nothing), and eleven register writes (ten character runs for one character register plus the required zero; the base covers the two linefeeds or the two spaces, never both). Both searches find 335 / 188 with two tracked registers; the three-register case is argued in the HALT section rather than searched (about ten million states), and every design that uses R1-R6 costs at least 2M - A more.

### Lemmas and searches: accepted, with L7 added

L1, L2, L3 and L5 were accepted by argument. L4's finite claims were checked by brute force over the LEA span: only NOT of x01D9-x0218 (DDR) and x01DD-x021C (KBDR) reaches a window in one instruction; ADD and AND of two LEA-span values stay in [x0202, x61FE] and at or below x30FF. L6 is correct for data-word spills and now excludes vector spills explicitly. L7 (register-register ALU ops add nothing) was added to justify an omission both searches make. The original search's output was reproduced byte for byte and its assertions pass; the second search reaches the same minima with a wider model (exact zero and MCR classes, NOT in both directions, separate device windows, LDI/STI paths, copies and spills, a restorable third register).

The search space was checked for excluded valid sequences: self-modifying code (the only useful data-dependent control transfer is TRAP through a vector word, which is the excluded IN design; patching instruction words cannot encode a 16-bit target or remove a forced device access), use of R7 (present in both searches), reuse of condition codes and branches (the four routines are straight-line by L2), data inside code and executing data as instructions (every window constant xFDE7-xFE26 and xFFDE-xFFFF decodes as a malformed TRAP and is illegal; executing a stored word to restore a register costs at least M + 2A > 2M), overlapping or shared routine tails (saves words, never cycles), and vector-table entries pointing into x0000-x00FF, outside the layout, or into another routine (illegal, out of layout, or no cheaper than entering normally).

### PUTS: closed form accepted; lower bound refuted and corrected; design kept; "best among all designs" open

- **Closed form: accepted.** The cost formula was checked against measurement at every L in 0..1000 at both latencies (2002 runs, no mismatch); output and R0 equal the reference OS and R1-R6 are preserved in every run.
- **Per-length lower bound: refuted and corrected.** The earlier bound charged every design with 33 or more words a round trip (2M) for the moved pointer, assuming it is restored by a load. A pointer moved by an immediate ADD can be restored by an immediate ADD. Design X (the shipped prefix; when word 31 is non-zero, `ADD R0,R0,#15` and words 32-46 at offsets 17-31, with every exit from that block undoing the ADD; when word 46 is non-zero, the stride loop with R0 + 15 spilled and undone on exit) passes the contract at every L in 0..1000 at both latencies and costs 1212 at L = 32 and 1687 at L = 46 at N=4 (678 and 943 at N=1), below the earlier bound's 1229 and 1705 (686 and 952). A two-block variant beats it at 47..61 at N=4 as well. The bound's last term is now the minimum of the load route and the immediate route, 2A per further 15 words; under the corrected bound design X costs at most one cycle above it at 32..46. The other terms of P1 (TRAP and RET, L + 1 loads and decisions, L stores, one round trip, the base term, A per 64 words) were accepted with their own justifications.
- **Gaps: recomputed.** Against the corrected bound the shipped design's worst gap over 0 ≤ L ≤ 1000 is 59 at N=4 (every L in 32..46) and 38 at N=1 (L = 992); the earlier figures 42 and 56 (24 and 38) were correct against the earlier bound. The objective's ranking of designs is unchanged: X, X2, Z1 and Z2 (worst gaps 71, 87, 63, 71 at N=4) all lose at the long end because an immediate block misaligns the loop windows with the bound's 64-word charge, and the P3 classes lose as before. Design X's faster range, 32 ≤ L ≤ 61 at N=4, is recorded under "Superseded and rejected designs".
- **No better design over 0..1000: open.** Not refuted; every design measured or argued (the P3 classes, the four immediate-ADD block designs, stride loops without a fourth register, state kept in R7, computed jumps and self-modifying code, shared epilogues, and a different placement of the slot block) has a larger worst-case gap or is dominated. The bound is known to be loose by up to 3M for L ≥ 62 and by the back edges, and no design exploits either.

### PUTSP: lower bound accepted and tightened; design improved; fixed and amortized costs open

- **Per-word lower bound: accepted and tightened** from 4M + 8A to 4M + 9A (124 / 73). Steps 1, 2, 4 and 5 were accepted, step 2 with a sharper page-count argument (an address can carry at most W[13] of the top three bits, and with it at most three of W[12:8], so no table saves a tree level). Step 3's count of two condition-code writes is not achievable: the NUL decision needs an AND whose low mask byte is xFF, that AND's only other free outcome is W[15], which the LDR already supplies, and W[14] and W[13] need the sign bits of two further writes. The claimed fold of the NUL test into the tree is therefore refuted, not merely unimplemented.
- **Design: improved.** The earlier design (one pointer ADD per word, natural branch directions, 259 / 142 fixed, 133-149 per word at N=4) was replaced by the eight-copy unroll: 255 / 138 fixed, 124-143 per word (73-86 at N=1), plus 17 / 11 per eight words. Class 011 meets the tightened bound exactly up to the amortized block cost; the earlier estimate that unrolling gains "at most A per word" was low, since the fall-through class also sheds its taken bit-14 branch (A + B). The contract was checked exhaustively for every position 0..16 in the eight-word block and every word value 0..xFFFF (17 × 65536 strings `[x6261 × p, W, x4142, x0000]`, no mismatch against the reference OS in output, R0 or R1-R6), by 20,000 random strings of length 0-199 with NULs in either byte at both latencies, and by the exact-cycle model in `tests/bundled_os.rs` at lengths 0, 1, 2, 5, 7, 8, 9, 16 and 17 for every class, at the 255 non-zero high bytes, and at the three terminators. GETC, OUT, PUTS, IN and HALT are unchanged in output, registers and cycles.
- **Open.** No lower bound is proven for the fixed cost (19M + A) or for the amortized pointer and back-edge cost ((A + B)/8 per word against the bound's A/64); the limit of eight copies is a consequence of this placement's branch reach, not a proven optimum over all placements.
