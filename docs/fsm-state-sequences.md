# LC-3 FSM state sequences: verification against Figure C.2

Scope: confirm the per-instruction state sequences and the set of memory-access states that the cycle model (ADR 0006) takes from Patt and Patel, *Introduction to Computing Systems: From Bits and Gates to C and Beyond*, 2nd edition (McGraw-Hill, 2004), Appendix C, Figure C.2, "A state machine for the LC-3".

Result: every state sequence and the memory-access set are confirmed. One arithmetic error in the cycle table was found and corrected (LDI and STI at N=4 are 18 cycles, not 15). The state sequences themselves are unchanged.

## Sources

Primary sources were obtained. Ranked by trust:

1. **Figure C.2 as printed on page 568 of the 2nd edition**, reproduced by the book's author on his university course site: Yale Patt, EE 306 final exam, 15 December 2009, University of Texas at Austin, https://users.ece.utexas.edu/~patt/17f.306/Exams/OldExams/f09_final.pdf (PDF page 12 is the book's page 568; page 14 is book page 570; PDF page 13 carries Figure C.7 on book page 577). The exam reproduces the book's own pages, footer stamps included.
   - Edition: 2nd edition. How this is known: (a) the page footers read `"app-c" - 2003/6/30 - page 568 - #4` and `"app-a" - 2003/6/30 - page 525`, the typesetting stamps of the 2004 edition; (b) the page header reads "568 appendix c The Microarchitecture of the LC-3", while the 3rd edition prints the same figure on page 702; (c) the publisher's 2nd edition errata page (source 2) addresses "Page 568, Figure C.2 (A state machine for the LC-3)", the same page number; (d) the instructor's course descriptors for the same course in 2008 and 2009 state "Mc-Graw Hill, 2004, 2nd edition. You will need the 2nd edition." (https://users.ece.utexas.edu/~patt/09f.306/descriptor.html, https://users.ece.utexas.edu/~patt/08f.306/descriptor.html).
   - Limitation: this is a reproduction of the page, not the physical book, which was not available. The reproduced figure already shows the errata corrections to states 4, 20 and 21 (source 2), so it is from a corrected printing or a corrected re-typeset page. The corrections do not alter the number of states on any path.
2. **Publisher's 2nd edition errata page** (McGraw-Hill, "Introduction to Computing Systems, 2/e", Errata Page): https://highered.mheducation.com/sites/0072467509/information_center_view0/errata_page.html. It is headed "2nd Edition Errata" and numbers errors by printing.
   - Entry "Page 568, Figure C.2 (A state machine for the LC-3). (1,2)": "Please correct states 4, 20 and 21 in the figure to read: State 4: [IR11]; State 20: PC <- BaseR, R7 <- PC; State 21: PC <- PC+off11, R7 <- PC."
   - Entry "Page 530, JSR/JSRR (1,2)": corrected operation is `TEMP = PC; if (bit[11] == 0) PC = BaseR; else PC = PC + SEXT(PCoffset11); R7 = TEMP;`.
   - No entry mentions states 32, 33 or 35, so those are as printed.
3. **3rd edition Appendix C**, used only to show the difference: https://users.ece.utexas.edu/~patt/19f.306/Handouts/LC3_State_Machine.pdf. Edition: 3rd. How known: header "702 appendix C", and the figure contains the 3rd edition's access-control-violation (ACV) states.

Not relied on: UT EE 360N Appendix C handouts (e.g. https://users.ece.utexas.edu/~patt/06s.382N/handouts/360n.appC.pdf). They describe the LC-3b, a different ISA with a different state machine (31 states, byte addressing, a five-cycle memory in its examples). They are not evidence for the LC-3 sequences below.

No secondary source (blog, forum, student notes, AI-generated summary) was used for any conclusion.

## 2nd edition versus 3rd edition

| Item | 2nd edition (this model) | 3rd edition (not modelled) |
|---|---|---|
| Fetch | 18, 33 (memory), 35 (IR <- MDR), 32 (decode) | 18, 33 (an [ACV] test state), 28 (memory), 30 (IR <- MDR), 32 |
| Address checks | none in Figure C.2 | every memory instruction passes through ACV test states (17, 19, 35 and others) |
| TRAP | 15, 28, 30 | leaves Figure C.2 at state 15 ("To 15, see figure C.7") |
| Memory-access state numbers | 33, 25, 24, 28, 29, 16 | 28 (fetch read), 25, 24, 29, 16 and others; 33 is an ACV test |

State numbers are reused with different meanings between editions (28 and 30 are the fetch memory read and IR load in the 3rd edition but the TRAP vector read and PC load in the 2nd). Any citation of a state number must name the edition.

Interrupt and exception handling exists in the 2nd edition too, in Figure C.7 (book page 577): state 18 has an [INT] test with an arc "To 49", state 32 sends RTI to state 8 and opcode 1101 to state 13. The simulator has no interrupts, privilege modes or RTI, so INT is never asserted and the arcs to 49, 8 and 13 are never taken. State 18 therefore always continues to 33 at no extra cost.

## Figure C.2, as read

Fetch and decode (common to all instructions):

| State | RTL | Memory wait loop |
|---|---|---|
| 18 | MAR <- PC; PC <- PC+1; [INT] | no |
| 33 | MDR <- M | yes, loops on not-R |
| 35 | IR <- MDR | no |
| 32 | BEN <- IR[11]&N + IR[10]&Z + IR[9]&P; dispatch on IR[15:12] | no |

Execute paths from state 32 (every path ends with an arc "To 18" unless noted):

| Instruction | States after 32 |
|---|---|
| ADD | 1 |
| AND | 5 |
| NOT | 9 |
| LEA | 14 (DR <- PC+off9, set CC) |
| JMP / RET | 12 (PC <- BaseR) |
| BR | 0 ([BEN] test); BEN=0 goes to 18; BEN=1 goes to 22 (PC <- PC+off9), then 18 |
| JSR | 4 ([IR11] test), then 21 (PC <- PC+off11, R7 <- PC) |
| JSRR | 4, then 20 (PC <- BaseR, R7 <- PC) |
| LD | 2 (MAR <- PC+off9), 25, 27 |
| LDR | 6 (MAR <- B+off6), 25, 27 |
| LDI | 10 (MAR <- PC+off9), 24, 26 (MAR <- MDR), 25, 27 |
| ST | 3 (MAR <- PC+off9), 23 (MDR <- SR), 16 |
| STR | 7 (MAR <- B+off6), 23, 16 |
| STI | 11 (MAR <- PC+off9), 29, 31 (MAR <- MDR), 23, 16 |
| TRAP | 15 (MAR <- ZEXT[IR[7:0]]), 28 (MDR <- M[MAR]; R7 <- PC), 30 (PC <- MDR) |

Memory states (the ones with a self-loop on not-R in the figure): 33, 28, 25, 24, 29, 16. Every other state has no wait loop. In particular 35, 32, 23, 26, 27, 30, 31 and the register-only states cost one cycle each.

## Answers to the specific questions

- **State 20 (JSRR) RTL.** Confirmed: `PC <- BaseR` and `R7 <- PC` in one state. Both transfers happen in the same clock cycle, so R7 receives the incremented PC and PC receives the old base register even when BaseR is R7. The printed figure shows this (state 20 box, corrected by the publisher's errata for printings 1 and 2). State 21 (JSR) is the same shape with `PC <- PC+off11`. JSR and JSRR therefore take two states after fetch (4 then 21 or 20).
- **State 32 is decode.** Confirmed: it computes BEN and dispatches on IR[15:12], and costs one cycle.
- **State 35 is the IR load.** Confirmed: `IR <- MDR`, one cycle, no wait loop. The wait loop is on state 33 only.
- **BR not taken.** Confirmed: BR goes to state 0, which tests BEN. With BEN=0 the arc goes directly back to 18. BEN=1 goes to 22 and then 18. So not taken costs one state after fetch (state 0) and taken costs two (0 and 22).
- **Memory-access states.** Confirmed as 33, 25, 24, 28, 29, 16. No additional state has a wait loop; none of the believed set lacks one.

## Cycle table check

With fetch = 18 (1) + 33 (N) + 35 (1) + 32 (1) = 3 + N.

| Instruction | After fetch | Total | N=1 | N=4 |
|---|---|---|---|---|
| ADD, AND, NOT, LEA, JMP/RET | 1 cycle | 4 + N | 5 | 8 |
| BR not taken (0) | 1 | 4 + N | 5 | 8 |
| BR taken (0, 22) | 2 | 5 + N | 6 | 9 |
| JSR (4, 21), JSRR (4, 20) | 2 | 5 + N | 6 | 9 |
| LD (2, 25, 27), LDR (6, 25, 27) | 1 + N + 1 | 5 + 2N | 7 | 13 |
| ST (3, 23, 16), STR (7, 23, 16) | 1 + 1 + N | 5 + 2N | 7 | 13 |
| TRAP (15, 28, 30) | 1 + N + 1 | 5 + 2N | 7 | 13 |
| LDI (10, 24, 26, 25, 27) | 1 + N + 1 + N + 1 | 6 + 3N | 9 | 18 |
| STI (11, 29, 31, 23, 16) | 1 + N + 1 + 1 + N | 6 + 3N | 9 | 18 |

LDI and STI each have two memory states after fetch plus the fetch state 33, so three memory states in all. After fetch each costs 3 + 2N (LDI: 1 + N + 1 + N + 1; STI: 1 + N + 1 + 1 + N), giving 6 + 3N in total.

## Corrections

One arithmetic error, in the cycle table only: the LDI/STI row listed 15 cycles at N=4. 6 + 3 x 4 = 18. The N=1 value (9) and the formula (6 + 3N) were right.

The cycle table's LDI/STI row now reads 18 in the "At N=4" column, and the sentence after the table states that the state sequences were verified against the published Figure C.2, with a pointer to this note.

No state sequence or memory-state classification changed.

## Effect on the per-row cycle tests

The per-row cycle tests assert the count at memory latency 1 and 4 for each row of the cycle table. The LDI row and the STI row assert 18 at N=4. The ADR 0006 sentence "N=4 makes fetch cost 7 cycles" is correct (3 + 4).
