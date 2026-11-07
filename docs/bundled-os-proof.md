# Bundled OS: design and optimality proof

This document fixes the bundled OS's placement, gives every TRAP routine's exact cycle count, and proves each count optimal or bounds it. Source: `src/bundled_os.asm`. Tests: `tests/bundled_os.rs`. Tools: `tools/bundled_os_search.py` (lower-bound search; output in `docs/bundled-os-search-output.txt`) and `tools/bundled_os_puts.py` (generates the unrolled PUTS section).

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
| PUTS, L >= 1 | see "PUTS" | 207 + 34L + ... | 114 + 19L + ... | slope optimal among all designs that fit |
| PUTSP | see "PUTSP" | 259 + per word | 142 + per word | best known; proven lower bound, gap documented |

No optimum differs between N=1 and N=4. Each argument below holds at both latencies, and the search runs at both. The one choice that compares costs across N (PUTS exit routing) gives the same rule at N=1 and N=4.

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
| x2109-x2F76 | 3694 | PUTS (constants, epilogue, prologue, 1216-word unrolled body, exit stub) |
| x2F77-x2FFF | 137 | free |

That is 4118 words of code and data, 3 address labels, and 7655 free words. The free words are scattered: no free run is longer than 248 words except the top 137 and the end of gap 0. Why PUTS cannot profit from them is proved under "PUTS".

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

### Design

One loop iteration prints 1216 words (19 blocks of 64).

- **Registers.** R0 = pointer, R7 = current word, R1 = xFE0A (DDR base), R2 = 64 (stride), R3 = the body's address. LDR offsets -32..31 cover each 64-word window, and one `ADD R0,R0,R2` per window advances it; the first window of a call starts at offset 0. The iteration closes with `JMP R3`, since the 3667-word body is beyond any branch's reach.
- **Per character.** `STR R7,R1,#-4`, then `LDR` and `BRz` for the next word, so the check is not taken for non-zero words.
- **Exits.** The zero word's BRz is taken. When the epilogue is out of reach, it lands on another check's BRz, which is taken again (branches leave Z set). Chains run backward to `PUTS_EXIT` for words 1-708 (at most 8 hops), and forward to `PUTS_STUB` (`LD R7; JMP R7`, then the epilogue) for words 709-1216 (at most 5 hops). Each check takes whichever direction is cheaper. With B per hop and M + A for the stub, the rule is "backward when its hops are at most two more than the forward hops", and it is the same at N=1 and N=4.
- **Empty string.** It exits on the prologue's taken BRz, so every non-empty string gets the not-taken branch (the tie ruling).

### Exact cost

For L >= 1, with q = L mod 1216 (or 1216 when that is 0):

```
T(L) = 14M + 3A + L(2M + A) + A*floor((L + 32) / 64) + A*floor((L - 1) / 1216)
       + 1 + B*hops(q) + [forward route](M + A)
```

hops(q) and the route follow the chain rule above; `tests/bundled_os.rs` re-derives them independently. T(0) = 4M + A + B. Examples at N=4: T(0) = 69, T(1) = 241, T(64) = 2391, T(1216) = 41724, T(1217) = 41745. At N=1: 39, 133, 1337, 23325, 23337. The tests check 22 lengths from 0 to 2600 at both latencies, which crosses two iteration boundaries.

### Slope optimality among all designs that fit

The objective is the per-character marginal cost first and the fixed cost second. The shipped slope is 2M + A + A·20/1216: 34.13158 at N=4 and 19.08224 at N=1. That is optimal among all PUTS designs that fit the OS region, with PUTSP's table and the other routines as given. Proof:

1. **Per character, at least 2M + A.** Each word needs a load: LDR (M). LD has a fixed address, and LDI needs its pointer cell updated per word, costing at least I + M. Each word also needs a DDR store (at least M), and an instruction whose behavior depends on that word before its byte is stored (at least A). Without such an instruction, the zero and non-zero cases would store the same byte. Redirecting stores by data instead of branching also costs an ALU op per word.
2. **Per 64 characters, at least one more A.** An LDR addresses base + offset with the offset in [-32, 31], so each distinct base value covers at most 64 consecutive words. Each new base value is a register write (at least A, L1) distinct from the loads, stores and checks. Loaded words are data and cannot serve as bases. That gives at least A per 64 words, exactly A/64 per character only when the words per iteration are a multiple of 64.
3. **Closing the loop.**
   - **Branches only.** A program processing L words executes at least 3L instructions that advance PC by one, and the PC stays inside the region. Taken branches move PC back at most 255 words, so they must be at least (3L - Range)/256 in number, each costing at least 1 cycle more than a not-taken check. As L grows, that is at least 3/256 per character. The slope is then at least 2M + A + A/64 + 3/256: 34.1368 (N=4) and 19.0899 (N=1), above the shipped slope.
   - **A register jump** (JMP/JSRR/RET/TRAP, at least A) per iteration. A straight run of code is confined to one contiguous free region, because crossing a table row costs a taken transfer, or 8A to execute its NOP-valued words. The largest such region is x2109-x2FFF (3831 words), and an iteration whose runs lie in it handles U ≤ 3831/3 words. Its slope is at least 2M + A + A·ceil(U/64)/U + A/U. Adding runs in table gaps (at most 248 words) costs at least 1 cycle per run of at most 82 words, plus the stride ADDs: at least 1/82 + A/64 per character, above the shipped average. So the best single-run design has U = 64j words with 3U + j + 1 ≤ 3831 (the body plus its JMP), which gives j ≤ 19. j = 20 needs 3861 words.
   - **U not a multiple of 64.** For U in (1216, 1270], the slope is 2M + A + 21A/U ≥ 2M + A + 21A/1270 (34.13228 / 19.08268), worse than j = 19.
   - **JSR back edges** (reach 1023) cost B as extra instructions, at least 3B/1023 per character, which is worse again.
   - So j = 19 is the optimum. The shipped design meets it exactly: per iteration, 19 ADDs and one JMP, and nothing else beyond the per-character work.
4. **Floor without integrality.** Relaxing the integrality gives the floor 2M + A + A/64 + 3A/3831: 34.13126 (N=4) and 19.08204 (N=1). The 0.0003 cycles per character between that floor and the shipped slope comes entirely from rounding to whole blocks.

Fixed cost (second). Five loop registers hold the pointer, word, base, stride and body address. Together with the return address, the string start and the six entry values of R1-R6, that is 13 values for 8 registers. So at least five 16-bit values (return address, string start, and three of R1-R6) make a round trip: 10M (L6). The base costs M (L4). The stride (64) costs M: it is not LEA-reachable, and at least five ALU ops would build it. The body address costs A (LEA). With the TRAP, the first load, the first check and RET, that totals 14M + 3A, which the prologue and epilogue meet. The exit term is L-dependent. Greedy farthest-reach chaining minimizes the hop count in each direction, and each check takes the cheaper direction.

### Alternatives (for comparison; all at the same layout)

| Design | T(L), L >= 1 | Slope N=4 | Slope N=1 | Words |
|---|---|---|---|---|
| rolled, 1 word per iteration | 9M + 2A - 1 + L(2M + 2A + 1) | 43 | 25 | ~12 |
| 15 per block, one `ADD #15` | 9M + 2A + 1 + L(2M + A) + (A+1)floor(L/15) - 2[15 divides L] | 34.600 | 19.400 | ~55 |
| 64 per iteration, stride register, BR back edge | 12M + 2A + L(2M + A) + A floor((L+32)/64) + floor((L-1)/64) + [64 does not divide L] | 34.1406 | 19.0938 | ~205 |
| shipped: 1216 per iteration, JMP back edge | above | 34.13158 | 19.08224 | 3694 |

The 64-word design beats the 15-word one for every L above 104 (N=4) and 74 (N=1). The shipped design costs more fixed cycles (14M + 3A plus exit hops, against 12M + 2A). It is cheaper than the 64-word design from L = 4865 (N=4) and 2369 (N=1) onward, and dearer below those lengths, with the order alternating near them. This is the direct consequence of "marginal cost first" combined with the ruling to unroll as far as space allows. It is noted in the report.

A larger PUTS would need a smaller PUTSP table. A 16-row table (one more tree level in PUTSP, at least A + A per word there) would free about 3840 contiguous words and allow j = 39, a slope of 2M + A + 40A/2496: 0.0034 cycles per character better at N=4. Under the ruling, PUTS's budget is the region minus PUTSP's needs, so PUTSP's table stays.

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
4. The pointer advance costs at least A/64 per word (PUTS step 2).
5. For 0 < h < x20 (t = 0), "h = 0?" is one more decision: at least one more A.

The best class of the shipped design (t = 3) costs 4M + 9A + B = 4M + 10A + 1, so the gap is 2A + 1 (17 at N=4). It comes from three sources: the per-word pointer ADD, one CC-setting write more than the bound (the low-byte test is separate from the tree), and the taken bit-14 branch. Other classes add their rejoin branch (B) and taken tree branches.

### Design space explored, and why it stopped

| Design | Per word (N=4) | Why not chosen |
|---|---|---|
| Reference-style shift loop (8 rotate steps per high byte) | more than 280 | slowest |
| Full 8-level branch tree, LEA leaves, no table (about 1300 words) | about 204 | slower than the tree plus table |
| 3-level tree plus 32-row table (shipped) | 133-149 | |
| 2-level tree plus 64-row table | | does not fit the region (64 pages > 46) |
| 4-level tree plus 16-row table | about 16 more per word | frees space only PUTS could use, at 0.0034 cycles/char |

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

## Reproducing

```
scripts/capture-bundled-os-fixture <path to native lc3as>   # checks the generated PUTS section, assembles with no diagnostics
cargo test --test bundled_os                                 # contract and exact cycles at N=1 and N=4
python3 tools/bundled_os_search.py                           # lower-bound search; asserts minima equal the tested costs
```
