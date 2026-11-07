#!/usr/bin/env python3
"""Exhaustive lower-bound search for the bundled OS's straight-line routines.

Usage: python3 tools/bundled_os_search.py
Output (stdout): for GETC, OUT, IN and HALT at memory latency 1 and 4, the
minimum cost the abstract machine below admits and one sequence achieving it.
Raises AssertionError if a minimum differs from the bundled OS's measured
cost, so a clean run is the evidence cited in docs/bundled-os-proof.md.

The abstract machine is the model justified by the lemmas in that document
(sections "General lemmas" and "Search model"). Every executed path of a
contract-respecting routine maps onto a sequence of these abstract steps whose
summed cost is at most the path's cost, so the minimum here is a lower bound. The lemmas, not this file,
carry the argument that the mapping exists.

State: (index of the next required device event, contents of the tracked
registers, set of tokens held in memory). Register contents are one of:
  "U"            a value that is useless to the routine,
  a token        "RET", "C", "X", "K1", "K2": a value that must survive or be
                 output and is not a constant (the return address, the input
                 byte, OUT's character, the entry values of R1-R6),
  "Z"            the constant x0000,
  "O"            another known constant with no direct use,
  ("ch", c)      a constant with low byte c, outside the device windows,
  ("base", c)    a constant inside the device windows (KBDR or DDR within
                 LDR/STR offset reach), with low byte c, or None when c is not
                 a character the routine prints,
  ("pre", c)     a constant in [x01D9, x0218], whose complement is a base
                 with low byte c (None as above).
Steps and costs (A = 4+N, B = 5+N, M = 5+2N, I = 6+3N):
  write a constant Z, O, ("ch", c) or ("pre", c) into any
  register                                                       A
  write ("base", c) into any register: A if some register holds
  a base, or holds ("pre", c), or (for c = None) holds the
  linefeed constant (x020A is the one printed character in the
  pre range);
  otherwise M (a load)                                           A or M
  copy one register into another                                 A
  store a token register to memory                               M
  reload a token from memory into any register                   M
  KBDR read into any register (makes it hold "C"): M if some
  register holds a base, otherwise I                             M or I
  DDR store of a register holding the required token or a value
  with the required low byte: M with a base, otherwise I         M or I
  finish: RET (A) needs the final registers; HALT's MCR store
  (M, STR R0,R0,#-2) needs R0 = Z.
"""

import heapq
from dataclasses import dataclass

BASE_LOW_BYTES = frozenset(range(0xE3, 0x100)) | frozenset(range(0x00, 0x27))
HALT_MESSAGE = "\n--- halted ---\n"


@dataclass(frozen=True)
class Costs:
    a: int
    b: int
    m: int
    i: int


def costs_at(n: int) -> Costs:
    """Per-instruction costs at memory latency n (n >= 1)."""
    return Costs(a=4 + n, b=5 + n, m=5 + 2 * n, i=6 + 3 * n)


@dataclass(frozen=True)
class Routine:
    """One TRAP routine's required behavior in the abstract machine.

    registers: names of the tracked registers; "R0" and "R7" first.
    entry: their contents on entry.
    events: device events in order: ("read",), ("token", t) or ("char", c).
    final: required final contents per register name (None = unconstrained).
    finish: "ret" or "halt".
    """

    name: str
    registers: tuple
    entry: tuple
    events: tuple
    final: tuple
    finish: str


def has_base(regs) -> bool:
    return any(isinstance(c, tuple) and c[0] == "base" for c in regs)


def low_byte_matches(content, char: str) -> bool:
    return isinstance(content, tuple) and content[0] in ("ch", "base") and content[1] == char


def base_cost(target, regs, c: Costs) -> int:
    """Cost of one instruction writing the base class `target`.

    Lemma L4 (window constants) in docs/bundled-os-proof.md: a window value
    comes from a load (M), from NOT of a pre-range value, or from ADD on an
    existing base (over-approximated here as reaching any base).
    """
    char = target[1]
    derivable = has_base(regs) or ("pre", char) in regs or (char is None and ("ch", "\n") in regs)
    return c.a if derivable else c.m


def constant_targets(routine: Routine):
    """Constant classes worth writing: printed characters and bases."""
    chars = sorted({e[1] for e in routine.events if e[0] == "char"})
    base_chars = [c for c in chars if ord(c) in BASE_LOW_BYTES] + [None]
    targets = ["Z", "O"] + [("ch", c) for c in chars] + [("pre", c) for c in base_chars]
    bases = [("base", c) for c in base_chars]
    return targets, bases


def successors(routine: Routine, state, c: Costs):
    """Yield (cost, next_state, description) for every abstract step."""
    event, regs, memory = state
    names = routine.registers
    targets, bases = constant_targets(routine)
    base = has_base(regs)
    for r, _ in enumerate(regs):
        def put(content, r=r):
            return regs[:r] + (content,) + regs[r + 1 :]

        for t in targets:
            yield c.a, (event, put(t), memory), f"{names[r]} <- {t}"
        for t in bases:
            yield base_cost(t, regs, c), (event, put(t), memory), f"{names[r]} <- {t}"
        for s, content in enumerate(regs):
            if s != r:
                yield c.a, (event, put(content), memory), f"{names[r]} <- copy {names[s]}"
        for token in memory:
            yield c.m, (event, put(token), memory), f"{names[r]} <- reload {token}"
        if isinstance(regs[r], str) and regs[r] not in ("U", "Z", "O"):
            yield c.m, (event, regs, memory | {regs[r]}), f"spill {names[r]} ({regs[r]})"
    if event < len(routine.events):
        kind = routine.events[event]
        device = c.m if base else c.i
        if kind[0] == "read":
            for r, _ in enumerate(regs):
                nxt = regs[:r] + ("C",) + regs[r + 1 :]
                yield device, (event + 1, nxt, memory), f"{names[r]} <- KBDR"
        else:
            for r, content in enumerate(regs):
                ok = content == kind[1] if kind[0] == "token" else low_byte_matches(content, kind[1])
                if ok:
                    yield device, (event + 1, regs, memory), f"DDR <- {names[r]}"
                    break


def finished(routine: Routine, state) -> bool:
    event, regs, _ = state
    if event != len(routine.events):
        return False
    return all(want is None or have == want for have, want in zip(regs, routine.final))


def minimum(routine: Routine, c: Costs):
    """Dijkstra from entry to finish; returns (cost, steps) of a cheapest path."""
    start = (0, routine.entry, frozenset())
    finish_cost = c.a if routine.finish == "ret" else c.m
    trap = c.m
    best = {start: trap}
    heap = [(trap, 0, start)]
    previous = {start: None}
    counter = 0
    while heap:
        cost, _, state = heapq.heappop(heap)
        if cost > best[state]:
            continue
        if finished(routine, state):
            steps = []
            node = state
            while previous[node] is not None:
                node, step = previous[node]
                steps.append(step)
            steps.reverse()
            return cost + finish_cost, ["TRAP"] + steps + [routine.finish.upper()]
        for step_cost, nxt, description in successors(routine, state, c):
            total = cost + step_cost
            if total < best.get(nxt, total + 1):
                best[nxt] = total
                previous[nxt] = (state, description)
                counter += 1
                heapq.heappush(heap, (total, counter, nxt))
    raise RuntimeError(f"{routine.name}: no sequence satisfies the contract")


ROUTINES = (
    Routine("GETC", ("R0", "R7", "R1"), ("U", "RET", "K1"), (("read",),),
            ("C", "RET", "K1"), "ret"),
    Routine("OUT", ("R0", "R7", "R1"), ("X", "RET", "K1"), (("token", "X"),),
            ("X", "RET", "K1"), "ret"),
    Routine("IN", ("R0", "R7", "R1", "R2"), ("U", "RET", "K1", "K2"),
            (("read",), ("token", "C"), ("char", "\n")),
            ("C", "RET", "K1", "K2"), "ret"),
    Routine("HALT", ("R0", "R7", "R1"), ("U", "U", "K1"),
            tuple(("char", ch) for ch in HALT_MESSAGE),
            ("Z", None, "K1"), "halt"),
)


def measured(name: str, c: Costs) -> int:
    """The bundled OS's cost for each routine, as asserted by tests/bundled_os.rs."""
    return {
        "GETC": c.m + c.i + c.a,
        "OUT": c.m + c.i + c.a,
        "IN": 7 * c.m + c.a,
        "HALT": 19 * c.m + 11 * c.a,
    }[name]


def main() -> None:
    for n in (1, 4):
        c = costs_at(n)
        for routine in ROUTINES:
            cost, steps = minimum(routine, c)
            print(f"N={n} {routine.name}: minimum {cost}, bundled OS {measured(routine.name, c)}")
            for step in steps:
                print(f"    {step!r}")
            assert cost == measured(routine.name, c), routine.name


if __name__ == "__main__":
    main()
