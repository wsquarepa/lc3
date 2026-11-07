#!/usr/bin/env python3
"""Regenerate src/bundled_os.asm with PUTSP unrolled eight words per iteration.

Usage: python3 tools/bundled_os_generate.py src/bundled_os.asm

Input: the current source. Its comment header (everything before `.ORIG`),
HALT (from "; Address constants for HALT's LEAs" through the halting STR) and
PUTS (from "; PUTS: print the low byte" through `.END`) are copied verbatim;
everything else (table rows, boot, GETC, OUT, IN, PUTSP) is emitted here.
Output: the same file, rewritten in place. Running the tool on its own output
reproduces it byte for byte, which `git diff` after a run checks.

PUTSP's eight copies differ only in their LDR offset, their labels and the
copy they fall into, so the copies are generated rather than hand-written.
The placement in gaps 27-29 is forced by branch reach: each copy's side
blocks (47 words) must lie within 256 words of the copy's chain sites and of
the next copy's entry, and the exit stubs read the spill slots off R3 = x1F00
(offset6 reaches x1EE0-x1F1F), so the slots are pinned at x1EE0.

Raises AssertionError when a gap overflows or a block is out of reach.
"""

import sys

UNROLL = 8
GAP_WORDS = 0xF8
ROW_STRIDE = 0x100
FIRST_GAP = 0x0209
CHAIN_GAP = 28
SLOTS = 0x1EE0
SIDE_GAPS = {27: (0, 2), 29: (4, UNROLL)}
MNEMONICS = {
    "ADD", "AND", "NOT", "LD", "LDR", "LDI", "LEA", "ST", "STR", "STI", "BR", "BRN", "BRZ",
    "BRP", "BRNZ", "BRNP", "BRZP", "BRNZP", "JMP", "RET", "JSR", "JSRR", "TRAP", "RTI",
    "GETC", "OUT", "PUTS", "IN", "PUTSP", "HALT",
}


def words(block):
    """Number of words an asm block occupies: one per instruction or .FILL, n per .BLKW."""
    count = 0
    for line in block.split("\n"):
        text = line.split(";")[0].strip()
        if not text:
            continue
        tokens = text.split()
        if not (tokens[0].startswith(".") or tokens[0].upper() in MNEMONICS):
            tokens = tokens[1:]
        if not tokens:
            continue
        if tokens[0].upper() == ".BLKW":
            size = tokens[1]
            count += int(size[1:], 16) if size.startswith("x") else int(size.lstrip("#"))
        else:
            count += 1
    return count


def gap_start(n):
    return FIRST_GAP + ROW_STRIDE * n


def row(n):
    return "\n".join(f"\t.FILL x{t * 0x20 + n:02X}" for t in range(8))


GAP0 = """
; Boot (uncounted) starts at x0200, which branches here past the first
; table row. It fills the trap vector table with UNKNOWN_TRAP, points the six
; assigned vectors at their routines, and halts. R0 is zero on entry. The
; table is never written again: UnknownTrap relies on every unassigned vector
; holding the sentinel for the whole run.
BOOT	LD R1,UNKNOWN_TRAP
	LD R2,VECTOR_COUNT
BOOT_FILL
	STR R1,R0,#0
	ADD R0,R0,#1
	ADD R2,R2,#-1
	BRp BOOT_FILL
	LD R0,FIRST_VECTOR
	LEA R1,TRAP_GETC
	STR R1,R0,#0
	LEA R1,TRAP_OUT
	STR R1,R0,#1
	LD R1,PUTS_VECTOR	; TRAP_PUTS is beyond LEA reach of boot
	STR R1,R0,#2
	LEA R1,TRAP_IN
	STR R1,R0,#3
	LD R1,PUTSP_VECTOR	; TRAP_PUTSP is beyond LEA reach of boot
	STR R1,R0,#4
	LD R1,HALT_VECTOR	; TRAP_HALT is beyond LEA reach of boot
	STR R1,R0,#5
	AND R0,R0,#0
	STR R0,R0,#-2		; x0000 - 2 = xFFFE: MCR <- 0 halts boot

; GETC: R0 <- next input byte. LDI is cheaper than building a base register
; for a single device access.
TRAP_GETC
	LDI R0,KBDR_PTR
	RET

; OUT: print R0[7:0]; R0 unchanged.
TRAP_OUT
	STI R0,DDR_PTR
	RET

; IN: R0 <- next input byte, then echo it and a linefeed (the reference
; prompt is omitted). R7 is spilled so that it can hold DDR_NL, which serves
; as the device base for all three accesses and as the linefeed itself.
TRAP_IN
	ST R7,IN_R7
	LD R7,DDR_NL
	LDR R0,R7,#-8		; KBDR
	STR R0,R7,#-4		; echo
	STR R7,R7,#-4		; '\\n' from the base's low byte
	LD R7,IN_R7
	RET

DDR_NL		.FILL xFE0A
KBDR_PTR	.FILL xFE02
DDR_PTR		.FILL xFE06
UNKNOWN_TRAP	.FILL xFFFE
VECTOR_COUNT	.FILL #256
FIRST_VECTOR	.FILL x0020
PUTS_VECTOR	.FILL TRAP_PUTS
PUTSP_VECTOR	.FILL TRAP_PUTSP
HALT_VECTOR	.FILL TRAP_HALT
IN_R7		.BLKW 1
"""

PUTSP_ENTRY = """
; PUTSP: print each word's low byte, then its high byte, stopping at the first
; NUL byte in either position; R0 restored.
;
; The high byte h cannot reach a register's low byte by arithmetic (carries
; only propagate upward), so it is found by a 3-level branch tree on W[15:13]
; followed by a table lookup on W[12:8]: HIGH_TABLE + (W & x1F00) + W[15:13]
; holds h, with x0000 for h = 0.
;
; The word code is unrolled eight times over LDR offsets 0-7 (WORD_0..WORD_7),
; so the pointer advances once per eight words (NEXT_BLOCK). Each copy's
; all-fallthrough path is class 011 (h = x60-x7F, lower-case ASCII): its
; branches are not taken and its leaf falls into the next copy. The other
; seven leaves of copy k sit in side blocks and branch to WORD_{k+1}.
;
; Registers: R0 = pointer, R7 = word W, R1 = DDR_NL, R2 = x00FF,
; R3 = x1F00 (also the base the exit stubs restore from), R4 = HIGH_TABLE,
; R5 = shifted copy of W or the low-byte test. Leaf names give W[15:13].
TRAP_PUTSP
	ST R7,SAVE_R7
	ST R0,SAVE_R0
	ST R1,SAVE_R1
	ST R2,SAVE_R2
	ST R3,SAVE_R3
	ST R4,SAVE_R4
	ST R5,SAVE_R5
	LD R1,PUTSP_DDR_NL
	LD R2,LOW_MASK
	LD R3,HIGH_MASK
	LD R4,TABLE_BASE
"""

PUTSP_CONSTANTS = """PUTSP_DDR_NL	.FILL xFE0A
LOW_MASK	.FILL x00FF
HIGH_MASK	.FILL x1F00
TABLE_BASE	.FILL HIGH_TABLE"""

PUTSP_SLOTS = """; Spill slots at x1EE0, within offset6 of R3 = x1F00 for the exit stubs.
SAVE_R0		.BLKW 1
SAVE_R1		.BLKW 1
SAVE_R2		.BLKW 1
SAVE_R3		.BLKW 1
SAVE_R4		.BLKW 1
SAVE_R5		.BLKW 1
SAVE_R7		.BLKW 1"""


def next_label(k):
    return f"WORD_{k + 1}" if k + 1 < UNROLL else "NEXT_BLOCK"


def leaf(k, t, nxt, exit_label, fall=False):
    """Leaf for class t of copy k; falls through into the next copy when `fall`."""
    out = [f"L{t:03b}_{k}", "\tAND R7,R7,R3", "\tADD R7,R7,R4", f"\tLDR R7,R7,#{t}"]
    if t == 0:
        out.append(f"\tBRz {exit_label}\t\t; high byte NUL: only possible when W[15:13] = 0")
    out.append("\tSTR R7,R1,#-4")
    if not fall:
        out.append(f"\tBRnzp {nxt}")
    return "\n".join(out)


def chain_copy(k, exit_label):
    return "\n".join([
        f"WORD_{k}",
        f"\tLDR R7,R0,#{k}\t\t; sign = W[15]",
        f"\tBRn HI_{k}",
        "\tAND R5,R7,R2",
        f"\tBRz {exit_label}\t\t; low byte NUL",
        "\tSTR R7,R1,#-4",
        "\tADD R5,R7,R7\t\t; sign = W[14]",
        f"\tBRzp L00_{k}",
        "\tADD R5,R5,R5\t\t; sign = W[13]",
        f"\tBRzp L010_{k}",
        leaf(k, 3, next_label(k), exit_label, fall=True),
    ])


def side(k, exit_label):
    """All non-fallthrough code of copy k: 47 words."""
    nxt = next_label(k)
    return "\n".join([
        f"L00_{k}",
        "\tADD R5,R5,R5\t\t; sign = W[13]",
        f"\tBRzp L000_{k}",
        leaf(k, 1, nxt, exit_label),
        leaf(k, 0, nxt, exit_label),
        leaf(k, 2, nxt, exit_label),
        f"HI_{k}",
        "\tAND R5,R7,R2",
        f"\tBRz {exit_label}\t\t; low byte NUL",
        "\tSTR R7,R1,#-4",
        "\tADD R5,R7,R7\t\t; sign = W[14]",
        f"\tBRn L11_{k}",
        "\tADD R5,R5,R5\t\t; sign = W[13]",
        f"\tBRn L101_{k}",
        leaf(k, 4, nxt, exit_label),
        leaf(k, 5, nxt, exit_label),
        f"L11_{k}",
        "\tADD R5,R5,R5\t\t; sign = W[13]",
        f"\tBRn L111_{k}",
        leaf(k, 6, nxt, exit_label),
        leaf(k, 7, nxt, exit_label),
    ])


def stub(name):
    """Exit: restore R0-R5 and R7 from the slots off R3, then return."""
    return "\n".join([
        name,
        "\tLDR R5,R3,#-27", "\tLDR R4,R3,#-28", "\tLDR R2,R3,#-30", "\tLDR R1,R3,#-31",
        "\tLDR R0,R3,#-32", "\tLDR R7,R3,#-26", "\tLDR R3,R3,#-29", "\tRET",
    ])


def emit_gap(n, blocks, pre_pad=0, pinned=None):
    """Emit gap n: optional leading pad, the blocks (pinned ones at their address), the pad to the next row, and that row."""
    out = []
    addr = gap_start(n)
    if pre_pad:
        out.append(f"\t.BLKW #{pre_pad}")
        addr += pre_pad
    for block in blocks:
        if pinned and block in pinned:
            target = pinned[block]
            assert target >= addr, f"gap {n}: block cannot start at x{target:04X} (cursor x{addr:04X})"
            if target > addr:
                out.append(f"\t.BLKW #{target - addr}")
                addr = target
        out.append(block)
        addr += words(block)
    end = gap_start(n) + GAP_WORDS
    assert addr <= end, f"gap {n} overflows by {addr - end}"
    if addr < end:
        out.append(f"\t.BLKW #{end - addr}")
    out.append(row(n + 1))
    return "\n".join(out)


def verbatim_pieces(lines):
    """Header (before .ORIG), HALT block and PUTS block of the current source."""
    i_orig = next(i for i, l in enumerate(lines) if l.strip().startswith(".ORIG"))
    i_halt = next(i for i, l in enumerate(lines) if l.startswith("; Address constants for HALT"))
    i_halt_end = next(i for i, l in enumerate(lines) if "MCR <- 0 halts" in l and i > i_halt)
    i_puts = next(i for i, l in enumerate(lines) if l.startswith("; PUTS: print the low byte"))
    return (
        "\n".join(lines[:i_orig]).rstrip("\n"),
        "\n".join(lines[i_halt:i_halt_end + 1]),
        "\n".join(lines[i_puts:]),
    )


def putsp_gaps():
    """Text of gaps 27, 28 and 29 (each followed by its row), with reach checks."""
    chain = "\n".join(chain_copy(k, "PUTSP_EXIT_A") for k in range(UNROLL))
    next_block = f"NEXT_BLOCK\n\tADD R0,R0,#{UNROLL}\n\tBRnzp WORD_0"
    sides = {27: "PUTSP_EXIT_C", 28: "PUTSP_EXIT_A", 29: "PUTSP_EXIT_B"}
    g27_blocks = [side(k, sides[27]) for k in range(*SIDE_GAPS[27])] + [stub(sides[27])]
    g28_blocks = [side(2, sides[28]), side(3, sides[28]), PUTSP_ENTRY, chain, next_block,
                  PUTSP_CONSTANTS, PUTSP_SLOTS, stub(sides[28])]
    g29_blocks = [side(k, sides[29]) for k in range(*SIDE_GAPS[29])] + [stub(sides[29])]

    word0 = gap_start(CHAIN_GAP) + words(side(2, sides[28])) * 2 + words(PUTSP_ENTRY)
    word1 = word0 + words(chain_copy(0, sides[28]))
    g27_pre = GAP_WORDS - sum(words(b) for b in g27_blocks)
    copy0 = gap_start(27) + g27_pre
    assert copy0 >= word1 - 256, f"copy 0 back edge out of reach: x{copy0:04X} vs WORD_1 x{word1:04X}"
    return (
        emit_gap(27, g27_blocks, pre_pad=g27_pre),
        emit_gap(28, g28_blocks, pinned={PUTSP_SLOTS: SLOTS}),
        emit_gap(29, g29_blocks),
    )


def generate(source):
    lines = source.split("\n")
    header, halt_block, puts_block = verbatim_pieces(lines)
    g27, g28, g29 = putsp_gaps()
    body = [
        header,
        "\n\t.ORIG x0200\n\tBRnzp BOOT\t\t; x0200 is the boot entry; the table row starts at x0201\n",
        "; HIGH_TABLE: row n (n = W[12:8]) starts at HIGH_TABLE + n * x100; entry t\n"
        "; (t = W[15:13]) holds h = t * x20 + n, which is x0000 only for h = 0.\nHIGH_TABLE",
        row(0),
        GAP0,
        f"\t.BLKW #{GAP_WORDS - words(GAP0)}",
        row(1),
        halt_block,
        f"\t.BLKW #{GAP_WORDS - words(halt_block)}",
        row(2),
    ]
    for n in range(2, 27):
        body.append("\t.BLKW xF8")
        body.append(row(n + 1))
    body.append("\n; PUTSP side blocks for copies 0 and 1 (gap 27).")
    body.append(g27)
    body.append("\n; PUTSP: copies 2 and 3, entry, main chain, constants, spill slots (gap 28).")
    body.append(g28)
    body.append("\n; PUTSP side blocks for copies 4 to 7 (gap 29).")
    body.append(g29)
    body.append("\t.BLKW xF8")
    body.append(row(31))
    body.append("")
    body.append(puts_block)
    return "\n".join(body)


def main(argv):
    if len(argv) != 2:
        raise SystemExit("usage: bundled_os_generate.py <src/bundled_os.asm>")
    path = argv[1]
    with open(path, encoding="utf-8") as f:
        source = f.read()
    with open(path, "w", encoding="utf-8") as f:
        f.write(generate(source))


if __name__ == "__main__":
    main(sys.argv)
