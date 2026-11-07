#!/usr/bin/env python3
"""Generate the PUTS section of src/bundled_os.asm.

Usage: python3 tools/bundled_os_puts.py
Output (stdout): the lines between the "BEGIN GENERATED PUTS" and
"END GENERATED PUTS" markers of src/bundled_os.asm, markers excluded.
scripts/capture-bundled-os-fixture regenerates the section and fails if the
source differs.

Design (docs/bundled-os-proof.md, "PUTS"): one loop iteration prints
BLOCKS * 64 words. R0 is the pointer; each 64-word window is read with LDR
offsets -32..31 and advanced by one ADD of the stride R2 = 64 (the first
window of a call starts at offset 0). The iteration closes with JMP R3, R3
holding the body's address, because the body is longer than a branch can
span. An exit is the taken BRz of the check that loads the zero word; when
the epilogue is out of reach it lands on another check's BRz, which is taken
again because a branch leaves the Z condition code set. Each check hops in
the cheaper direction: back to PUTS_EXIT, or forward to PUTS_STUB, which
jumps to PUTS_EXIT through R7 (free once the zero word is in it).
"""

BLOCKS = 19
WORDS = BLOCKS * 64
BRANCH_REACH_BACK = 255
BRANCH_REACH_FORWARD = 256
PREFIX = (
    "PUTS_R0\t\t.BLKW 1",
    "PUTS_R1\t\t.BLKW 1",
    "PUTS_R2\t\t.BLKW 1",
    "PUTS_R3\t\t.BLKW 1",
    "PUTS_R7\t\t.BLKW 1",
    "PUTS_BASE\t.FILL xFE0A",
    "PUTS_STRIDE\t.FILL #64",
    "PUTS_BODY_PTR\t.FILL PUTS_BODY",
    "PUTS_EXIT",
    "\tLD R3,PUTS_R3",
    "\tLD R2,PUTS_R2",
    "\tLD R1,PUTS_R1",
    "\tLD R0,PUTS_R0",
    "PUTS_EMPTY",
    "\tLD R7,PUTS_R7",
    "\tRET",
    "TRAP_PUTS",
    "\tST R7,PUTS_R7",
    "\tLDR R7,R0,#0",
    "\tBRz PUTS_EMPTY\t\t; an empty string pays the taken branch",
    "\tST R0,PUTS_R0",
    "\tST R1,PUTS_R1",
    "\tST R2,PUTS_R2",
    "\tST R3,PUTS_R3",
    "\tLD R1,PUTS_BASE",
    "\tLD R2,PUTS_STRIDE",
    "\tLD R3,PUTS_BODY_PTR",
)
EXIT_OFFSET = -16
"""Address of PUTS_EXIT relative to PUTS_BODY: 6 epilogue + 10 prologue words."""


def adds_through(word: int) -> int:
    """Stride ADDs executed before loading word `word` (word >= 1)."""
    return (word + 32) // 64


def check_offset(word: int) -> int:
    """Address of the BRz that tests word `word` (1..WORDS), relative to PUTS_BODY."""
    return 3 * word - 1 + adds_through(word)


def stub_offset() -> int:
    return check_offset(WORDS) + 2


def exit_routes():
    """Map each word index 1..WORDS to (target label, hops after the first).

    Backward chains end at PUTS_EXIT; forward chains end at PUTS_STUB. A check
    goes backward when its backward hops (B each) cost no more than its
    forward hops plus the stub (LD R7 then JMP R7, M + A): at N = 4 that is
    9h_back <= 9h_forward + 21 and at N = 1 6h_back <= 6h_forward + 12, both
    h_back <= h_forward + 2. The split is one threshold, so every chain stays
    in one direction.
    """
    checks = {word: check_offset(word) for word in range(1, WORDS + 1)}
    by_address = sorted((address, word) for word, address in checks.items())
    back = {}
    for address, word in by_address:
        if EXIT_OFFSET >= address - BRANCH_REACH_BACK:
            back[word] = ("PUTS_EXIT", 0)
        else:
            target = min(w for a, w in by_address if a >= address - BRANCH_REACH_BACK)
            back[word] = (f"PUTS_K{target}", back[target][1] + 1)
    forward = {}
    for address, word in reversed(by_address):
        if stub_offset() <= address + BRANCH_REACH_FORWARD:
            forward[word] = ("PUTS_STUB", 0)
        else:
            target = max(w for a, w in by_address if a <= address + BRANCH_REACH_FORWARD)
            forward[word] = (f"PUTS_K{target}", forward[target][1] + 1)
    return {
        word: back[word] if back[word][1] <= forward[word][1] + 2 else forward[word]
        for word in checks
    }


def body_lines(routes) -> list:
    targets = {label for label, _ in routes.values()}
    lines = ["PUTS_BODY", "\tSTR R7,R1,#-4\t\t; R7 = word 0, known non-zero"]
    for word in range(1, WORDS + 1):
        if word % 64 == 32:
            lines.append("\tADD R0,R0,R2\t\t; next 64-word window")
        offset = (word + 32) % 64 - 32
        lines.append(f"\tLDR R7,R0,#{offset}")
        label = f"PUTS_K{word}"
        prefix = label if label in targets else ""
        lines.append(f"{prefix}\tBRz {routes[word][0]}")
        if word < WORDS:
            lines.append("\tSTR R7,R1,#-4")
    lines.append("\tJMP R3\t\t\t; next iteration; word 0 of it is already in R7")
    lines.extend(("PUTS_STUB", "\tLD R7,PUTS_EXIT_PTR", "\tJMP R7", "PUTS_EXIT_PTR\t.FILL PUTS_EXIT"))
    return lines


def main() -> None:
    routes = exit_routes()
    print("\n".join(PREFIX + tuple(body_lines(routes))))


if __name__ == "__main__":
    main()
