# Bundled OS: design and optimality proof

This document fixes the bundled OS's placement, gives every TRAP routine's exact cycle count, and proves each count optimal or bounds it. Source: `src/bundled_os.asm`. Tests: `tests/bundled_os.rs`. Tools: `tools/bundled_os_search.py` (lower-bound search; output in `docs/bundled-os-search-output.txt`).

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
| PUTS, L >= 1 | see "PUTS" | 142 at L=1, 1270 at L=32, 7000 at L=200 | 79, 709, 3913 | worst-case gap to the per-length lower bound 42 over L ≤ 100 (24 at N=1); best in every class analyzed, not proven against all designs |
| PUTSP | see "PUTSP" | 259 + per word | 142 + per word | best known; proven lower bound, gap documented |

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

**Constraint used in every bound below.** The map matches the assignment the controller ruled on, so the contract includes this: the OS places code and data only in x0000-x00FF (the trap vector table) and x0200-x2FFF. Nothing goes in x0100-x01FF or xFE00-xFFFF, and the image does not wrap. Consequences:

- **Image.** One `.ORIG x0200` image, x0200-x2F76. The trap vector table is written by boot, which is uncounted (ADR 0004), so the image never covers x0100-x01FF.
- **Vector table.** It is entirely data fixed by the contract: 250 entries hold the UnknownTrap sentinel and six hold routine addresses. Executing any of its words is either illegal (xFFFE is a malformed TRAP, bits 11:8 = 1111) or one of six fixed BR/ADD/LD words that do nothing a routine could not do elsewhere. So all code is in x0200-x2FFF.
- **Device reach.** From any PC in x0200-x2FFF, LD/ST/LEA reach [PC-255, PC+256] ⊂ [x0101, x30FF]. No device register is PC-relative reachable, and no LEA or JSR result lies near xFE00.

**Word budget of x0200-x2FFF (11776 words).**

| Range | Words | Use |
|---|---|---|
| x0200 | 1 | `BRnzp BOOT` (boot entry) |
| x0201 + 256n + t, n = 0..31, t = 0..7 | 256 | PUTSP high-byte table (rows x0201-x2108) |
| x0209-x0291 | 137 | boot, GETC, OUT, IN, PUTSP, shared constants and spill slots |
| x0292-x0300 | 111 | free (end of gap 0) |
| x0320, x032D, x0368 | 3 | HALT's LEA address labels (contents unused) |
| x0369-x0386 | 30 | HALT and its base constant |
| rest of gap 1 | 215 | free |
| gaps after rows 2-30 | 7192 | free (29 gaps of 248 words) |
| x2109-x223C | 308 | PUTS (prologue, 31-word prefix, exits, slots and constants, switch, 64-word loop, epilogue) |
| x223D-x2FFF | 3523 | free (above the image) |

The image is x0200-x223C (8253 words). That is 732 words of code and data, 3 address labels, and 11041 free words: 7518 inside the image and 3523 above it.

## General lemmas

**L1 (instruction floor).** Every instruction costs at least A. The only instructions that change PC other than by +1 are BR (A not taken, B taken), JMP/RET (A), JSR/JSRR (B), and TRAP (M). Every memory read or write instruction costs at least M.

**L2 (worst case over paths).** A routine's cost is its worst case over entry registers and console input. Unconditional jumps and branches can be deleted from any executed path without changing its data effects, which only lowers cost, so a lower bound may consider the straight-line sequence of data instructions on one path. That holds as long as the bound does not let the path exploit coincidences in entry values. For that, take entry registers "generic": no entry value, and nothing computed from entry values alone by the path's finitely many instructions, equals any constant the routine needs (a printed character's low byte, a device-window value, zero). Such a choice exists because each coincidence excludes finitely many values of a 16-bit register. On the generic path, values derived only from entry registers are useless except as copies of themselves. Branching on data instead (to learn a value and then rebuild it with constants) needs at least 8 conditional branches on some path to rebuild a byte, since 256 cases need a binary decision tree of depth 8. That costs 8A > 2M (32 + 8N > 10 + 4N), more than the memory round trip it would replace. This is the case for IN's input byte below, and it holds at every N.

**L3 (device access).** Because of the layout constraint, a KBDR read, DDR write or MCR write is either LDI/STI through a pointer cell (I) or LDR/STR off a base register (M plus the cost of creating the base). The base window for DDR is [xFDE7, xFE26] (xFE06 within offset6 -32..31), for KBDR [xFDE3, xFE22], and for MCR [xFFDE, x001E]. The last includes x0000: `STR R0,R0,#-2` with R0 = 0 writes 0 to xFFFE.

**L4 (window constants).** A register value in the DDR or KBDR window can only come from (a) a load (at least M), (b) NOT of a value in [x01D9, x0218], or (c) an ALU op whose operand already lies in or near the window. Proof: on the generic path, constants come from LEA, JSR, TRAP (values in [x0101, x3000], L3's reach), AND #0 (x0000), loads, or ALU ops on earlier constants. One ALU op on such a constant reaches the window only by NOT of [x01D9, x0218]. ADD/AND with immediates or with two such constants stay within [x0000, x61FE] or below their operands. Among values in [x01D9, x0218], the only one whose low byte is a character a routine prints is x020A, a linefeed (low byte x0A); its complement xFDF5 is a base whose low byte, xF5, is not printed. So creating a base costs a load (M), or an LEA plus NOT (2A > M), or one NOT of an x020A linefeed constant that was needed anyway (A, but the result's low byte is not a printed character).

**L5 (carries go up).** For ADD, AND and NOT, bit i of the result depends only on bits 0..i of the operands. So the low byte of any ALU result depends only on the operands' low bytes, and a value's high byte can reach a register's low byte only through control flow (branches), a load from a data-dependent address, or a data-dependent jump.

**L6 (round trips).** If at some instant the values that must survive (tokens such as R7's return address, an input byte, and the entry values of R1-R6) outnumber the registers free to hold them, the excess must go to memory and come back: a store and a load, 2M each. An ALU "packing" of two tokens into one register cannot replace this. By L5, final low bytes after the instant are functions of the low bytes at the instant (absent loads of stored values and absent branches). One low byte fewer than needed cannot carry them, and branching to rebuild a byte costs at least 8A > 2M (L2). Executing a stored word as an instruction to restore a value costs a store (M), plus the executed instruction (A), plus building a word with an opcode around the value (A): at least M + 2A > 2M. Using one of R1-R6 as scratch costs at least a copy and a restore (2A) or a store and a load (2M).

## Search model

`tools/bundled_os_search.py` runs Dijkstra over an abstract machine. Its states are: the device events done so far; the class of each tracked register (R0, R7, and one or two of R1-R6 as needed); and the tokens currently stored in memory. Classes: useless; a token; zero; another constant; a constant with a printed low byte; a pre-range constant (L4 (b)); and a window base with a printed or other low byte. The steps and their costs are listed in the tool's docstring.

Each step is one instruction at the cheapest cost L1 allows for its effect. Base creation follows L4. Device events follow L3. Tokens are atomic by L6, and token spills and reloads are M each. Branches are absent by L2. Every class transition a real instruction can make is a step, or is dominated by one. The model over-approximates in two ways: any ADD from a base may reach any base, and every linefeed constant counts as pre-range. Both only lower the minimum, so the minimum is a lower bound.

Tracking only some of R1-R6 is complete because each additional scratch register costs at least 2A (L6), and the minimum's non-forced cost leaves no room for one more:

- GETC and OUT: slack 5 (N=4) or 2 (N=1), against 2A for one extra register. The search still tracks one.
- IN: tracks two. A third costs at least 6A, more than the 3M of non-forced cost at the minimum.
- HALT: tracks one. Two extra registers add at least 4A to the 10 forced writes (14A), more than the minimum's M + 11A at both latencies.

The search output (`docs/bundled-os-search-output.txt`, regenerated with `python3 tools/bundled_os_search.py`, about a minute) finds, at both latencies, minima equal to the bundled OS's tested costs: 39/21, 39/21, 99/54, 335/188. It asserts this equality.

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

The search confirms 99 and 54.

## HALT: 19M + 11A (335 / 188)

R7 = xFE0A is the DDR base and prints both linefeeds. R0 carries the other characters, through three LEAs of address labels chosen for their low bytes and ADD steps between neighbors. Then `AND R0,R0,#0; STR R0,R0,#-2` halts with R0 = x0000, as the reference OS leaves R0 (its HALT computes x8000 AND x7FFF and stores it). R1-R6 are untouched.

Lower bound. Forced: TRAP (M), 16 DDR stores and one MCR store (at least M each, L3), and R0 = 0 at the halting store. The register writes form a two-register caching problem over the request sequence `\n - - - sp h a l t e d sp - - - \n`, plus a final zero in R0, plus at least one base if stores are to cost M rather than I. Nine distinct characters plus zero give at least 10 writes. The search finds the exact minimum, M + 11A for the writes (one load for the base-and-linefeed, eleven register writes), so 19M + 11A. Cheaper-looking options exist but lose: deriving a base by NOT of a linefeed constant (L4) costs A but yields a base that prints nothing, and STI stores cost I each.

## PUTS

### Objective

This is the user's third decision on PUTS. Minimize the worst case over L of gap(L) = T(L) - LB(L) at N=4, where LB is the per-length lower bound below; ties go to the smaller gap at short lengths (L ≤ 100). It replaces the earlier objectives ("marginal cost first", then "the 64-word loop for realistic lengths").

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
LB(L) = 4M + 2A + L(2M + A) + min(L(N + 1), 3M) + [L ≥ 32](2M + A*ceil((L - 31) / 64))
```

so LB(0) = 4M + 2A. A design whose empty-string check falls through (`ST R7; LDR R7,R0,#0; BRnp NONEMPTY; LD R7; RET`) meets it. An earlier revision of this section stated LB(0) = 4M + A + B, which no design is bound by. The terms:

- TRAP (M) and RET (A).
- L + 1 loads of string words (LDR, M) and L + 1 word-dependent instructions (at least A each; P2 step 1).
- L DDR stores, each STR off a base (M) or STI (I = M + N + 1).
- The word needs a register while R7 holds the return address and R0 the string start, so one value makes a round trip (2M, L6).
- Using a base costs its creation (M, L4) and another register's round trip (2M), so stores cost at least min(L(N+1), 3M) more than all-STR.
- 33 or more words cannot be reached from R0 with offsets 0..31, so they need a pointer value per further 64 words (A each) and a second preserved value (2M).

LB holds numerically against every design analyzed, at every L up to 5000. It charges no loop closure (back edges), which is why every finite-loop design's gap grows slowly with L.

### Gap of the shipped design

```
gap(0)  = 1
gap(L)  = 3M + 1 - min(L(N + 1), 3M)                                1 ≤ L ≤ 30
gap(31) = 0
gap(L)  = 3M + 1 + floor((L - 32) / 64) + [64 does not divide L - 31]  L ≥ 32
```

floor((L + 32)/64) = ceil((L - 31)/64) cancels the ADD term exactly. The gap for L ≥ 32 is the stride register's save, restore and load (3M), the taken branch over the slot block (1), and the back edges LB does not charge.

| L | 0 | 1 | 2 | 5 | 7 | 8 | 10 | 30 | 31 | 32 | 64 | 95 | 96 | 100 | 200 | 1000 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| N=4 | 1 | 35 | 30 | 15 | 5 | 1 | 1 | 1 | 0 | 41 | 41 | 40 | 42 | 42 | 43 | 56 |
| N=1 | 1 | 20 | 18 | 12 | 8 | 6 | 2 | 1 | 0 | 23 | 23 | 22 | 24 | 24 | 25 | 38 |

Worst case: **42 over L ≤ 100 at N=4** (first reached at L = 96; 24 at N=1), and 56 (38) over L ≤ 1000. Over every possible length (L ≤ 65535) the back edges accumulate to 1064 (1046).

The empty string's gap of 1 comes from the tie ruling, which makes the empty string pay the taken entry branch. The ruling still holds under this objective. The alternative (`BRnp` to the non-empty path) would give gap(0) = 0 but add the taken cycle to every L ≥ 1, raising the worst case to 43 (25 at N=1) and every short-length gap by one.

### P2. Slope

These steps are unchanged in substance from the analysis of the plain 64-word design.

1. **Per character, at least 2M + A.** Each word needs an LDR (M), a DDR store (at least M), and an instruction whose behavior depends on that word before its byte is stored (at least A).
2. **Per 64 characters, at least one more A.** Each LDR base value covers at most 64 words (offset6), and each new base value is a register write.
3. **The shipped loop's slope** is 2M + A + (A + 1)/64: 34.1406 (N=4), 19.0938 (N=1), within 1/64 cycle per character of the floor 2M + A + A/64.
4. **Single-branch loops.** Among loops closed by one taken conditional branch, a 64-word body is optimal: the body must fit the branch's 256-word reach, so U ≤ 84, and (A·ceil(U/64) + 1)/U is least at U = 64.

### P3. The switch point is best within the hybrid classes (N=4 and N=1)

Classes analyzed, with costs derived from the cycle table:

- **H(j, k).** j words by STI with nothing set up, then words j..k-1 off the unmoved R0 with a base, then the 64-word stride loop (0 ≤ j ≤ k ≤ 31).
- **H(0, 0)** is the plain 64-word design (commit 7bf3090).
- **The maximal unroll** (commit 82b3334).
- **The ADD-#15 design** (commit 8bc2c16).
- **STI with ADD-#15 blocks.**
- **STI with offsets only** (L ≤ 31).

Worst-case gap over L ≤ 100 at N=4 (N=1):

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

- LB(L) holds for every fitting design at every L ≥ 0 (P1; corrected at L = 0, where it is 4M + 2A).
- The shipped design's exact gap at every L is the closed form above.
- Its loop slope is within 1/64 of the universal floor and optimal among single-branch loops (P2).
- Within the classes in P3, no design has a smaller worst-case gap over L ≤ 100 or L ≤ 1000, at either latency. The exceptions are the idealized H(0, 31), which this layout cannot realize, and the N=1 tie noted in P3.

**Not proven:**

- That no fitting design outside those classes has a smaller worst-case gap. Untried shapes include a different prefix register allocation, a loop that keeps the string start recoverable without a spill, or a closure structure LB does not charge.
- That LB is tight: it omits loop closure, and its short-length STI branch is met only by the STI design.

### Superseded designs

| Design | Commits | Why it was replaced |
|---|---|---|
| ADD-#15 unroll | 8bc2c16 | superseded by the "keep unrolling" ruling |
| plain 64-word loop with stride register | 37406b5, 7bf3090 | superseded by this hybrid: worst gap 100 at L = 1 |
| maximal unroll, 1216 words per iteration with a register back edge, slope 2M + A + 20A/1216 (34.13158 / 19.08224) | 7a8a35c, 82b3334 | rejected by the user because it is dearer for every L below 4865 (N=4) and 2369 (N=1) |

The maximal unroll is slope-optimal among all designs that fit with PUTSP's table. That argument, from the run structure and branch reach, is in the history of this document at commit ca139d7.

## PUTSP

### Design (best known)

- **Low byte.** For each word W: `LDR` (its sign is W[15]), then `AND R5,R7,R2` (low-byte NUL test) and `BRz`, then `STR` of W to print its low byte.
- **High byte.** A branch tree on W[15:13] (`ADD R5,R7,R7` and `ADD R5,R5,R5` put W[14] and W[13] in the sign). In each of the 8 leaves: `AND R7,R7,R3` (x1F00), `ADD R7,R7,R4` (table base), `LDR R7,R7,#t` gives h from the table, then `STR`. Only leaf 000 needs `BRz` for h = 0.
- **Loop.** One pointer `ADD` per word. Leaf 011 (h = x60-x7F, lower-case ASCII) falls through into the next word, and the other leaves branch back.
- **Table.** HIGH_TABLE + (W & x1F00) + W[15:13] holds h; rows sit at x0201 + 256n.

Fixed cost: 18M + 2A + B (259 / 142), covering the saves of R0-R5 and R7, three constant loads, an LEA for the table base, the entry branch, the restores and RET.

Per word (both bytes printed), by t = W[15:13]: t=0: 4M + 11A + B; t=1 and t=4: 4M + 9A + 2B; t=2, 5 and 6: 4M + 8A + 3B; t=3: 4M + 9A + B; t=7: 4M + 7A + 4B. At N=4 that is 149, 142, 143, 133, 142, 143, 143, 144.

Terminators:

| Terminating word | Cost |
|---|---|
| Low NUL, W[15] = 0 | M + 2A + B |
| Low NUL, W[15] = 1 | M + A + 2B |
| h = 0 after printing the low byte | 3M + 9A + B |

### Proven lower bound (not tight)

Per word printing both bytes, with h ≥ x20: at least **4M + 8A** plus A/64 amortized (116 / 68). With 0 < h < x20: at least 4M + 9A.

1. One load of W (M) and two DDR stores (at least M each, L3).
2. **Table lookup costs at least 2A + M.** By L5, h reaches a printed low byte only through branches or data-dependent addresses. A table address derived from W carries h-information only in bits 8-15 (L5), so a table that distinguishes 2^k values of h spans at least 2^k pages. The allowed region has 46 pages (x02-x2F), and the vector-table page is fixed data. So k ≤ 5, achieved only by bits 8-12 (32 pages); any other bit set spans more than 46 pages. Building the address needs at least two ALU ops: one to remove W's other bits (AND), and one to move pages x00-x01 into the region, since no single op does both. That is 2A, then the table load (M).
3. **At least 4A of branches and 2A of CC-setting writes.** The bits W[15:13] (8 cases) and the low-byte NUL decision (2 cases) must be resolved by branches: 16 outcomes need at least 4 two-way branches on the worst path. Each CC-setting write yields at most 3 outcomes (N, Z, P). The load of W supplies one 2-way setting, because Z only arises for W = 0. 2·3^s ≥ 16 needs s ≥ 2 more writes, each at least A and each W-dependent. They are distinct from the table-address ops, whose CC carries only bits 8-12.
4. The pointer advance costs at least A/64 per word (PUTS, P2 step 2).
5. For 0 < h < x20 (t = 0), "h = 0?" is one more decision: at least one more A.

The best class of the shipped design (t = 3) costs 4M + 9A + B = 4M + 10A + 1, so the gap is 2A + 1 (17 at N=4). It comes from three sources: the per-word pointer ADD, one CC-setting write more than the bound (the low-byte test is separate from the tree), and the taken bit-14 branch. Other classes add their rejoin branch (B) and taken tree branches.

### Design space explored, and why it stopped

| Design | Per word (N=4) | Why not chosen |
|---|---|---|
| Reference-style shift loop (8 rotate steps per high byte) | more than 280 | slowest |
| Full 8-level branch tree, LEA leaves, no table (about 1300 words) | about 204 | slower than the tree plus table |
| 3-level tree plus 32-row table (shipped) | 133-149 | |
| 2-level tree plus 64-row table | | does not fit the region (64 pages > 46) |
| 4-level tree plus 16-row table | about 16 more per word | slower; the space it frees is not needed |

Known improvements, not implemented:

1. **Unroll PUTSP across the free table gaps.** This replaces the per-word pointer ADD with LDR offsets, saving about A per word (6 to 8 at N=4) except at block boundaries. There are about 60 words per word-position. With R6 free, it can use a register back edge, or a 4-position loop inside one 248-word gap with a direct back edge (saving 3A/4 per word, less on the fall-through class). It was not implemented because the rejoin branches must stay within branch reach across table rows, and a second generated section would be needed; the remaining gain is at most A per word.
2. **Fold the low-byte NUL test into the tree's CC settings.** The counting bound allows one fewer write. No concrete encoding was found that keeps the NUL decision exact on every path.

## UnknownTrap mechanism

- **Sentinel.** Every unassigned trap vector (x00-x1F and x26-xFF) holds **xFFFE**, written by boot. xFFFE is the machine control register. It can never be a routine address in this layout (the OS uses only x0200-x2FFF), and fetching it reads x8000 (RTI). So under a machine without the rule, a jump there fails with `IllegalInstruction { pc: xFFFE, word: x8000, kind: Rti }` rather than running anything.
- **Rule.** It applies only when the run's OS is the bundled OS; `Prepared` records whether the OS image equals the bundled image, origin and words. When the machine executes a TRAP:
  1. The ordinary cycle-cap check for the TRAP's 5+2N cost runs first, so `CycleCapReached` takes precedence.
  2. The machine reads the vector-table word M[ZEXT(IR[7:0])] (state 28).
  3. If that word equals xFFFE, the run fails with `UnknownTrap { vector: IR[7:0] }` instead of loading PC. The vector is exact because it comes from the executing TRAP word.
- **Why gated.** UnknownTrap is raised only by the bundled OS. A user who pokes xFFFE into a vector under another OS keeps that OS's behavior (the RTI failure above). Under the bundled OS, the same poke reports UnknownTrap, which is the intended meaning.
- **Test.** `tests/bundled_os.rs` checks the table contents after boot: 250 entries of xFFFE, six distinct routine addresses in x0200-x2FFF, x0100-x01FF untouched.

## Known issues

- Commit dbf1c2d does not pass the gate on its own: its tests expect the 64-word PUTS, while its fixture is still the maximal unroll. A failed `git add` split one change across dbf1c2d and 7bf3090, and history is not rewritten. It is a bisect hazard only; 7bf3090 and every later commit pass.
- Commit dbf1c2d's subject ("resolve the reference os from the manifest dir") does not describe its content: that change landed only in 89ce635.

## Reproducing

```
scripts/capture-bundled-os-fixture <path to native lc3as>   # assembles with no diagnostics
cargo test --test bundled_os                                 # contract and exact cycles at N=1 and N=4
python3 tools/bundled_os_search.py                           # lower-bound search; asserts minima equal the tested costs
```
