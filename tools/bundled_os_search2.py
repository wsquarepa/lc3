#!/usr/bin/env python3
"""Second, independently written lower-bound search for GETC, OUT, IN and HALT.

It shares no code with tools/bundled_os_search.py and tracks a wider abstract
machine; its minima are compared with that tool's in docs/bundled-os-proof.md
("Verification"). Output: docs/bundled-os-search2-output.txt.

Usage: python3 tools/bundled_os_search2.py [--vector] [--regs N] [--only ROUTINE]
  --vector  allow a spill of the return address into the trap vector table
            (STI, cost I) and a return by TRAP (cost M) instead of RET (A).
  --regs N  number of tracked registers (2: R0, R7; 3: plus R1 whose entry
            value must be restored; 4: plus R2).

Abstract values (cls, b): b is the value's low byte when it is one of the
routine's printed characters or x00, else None.
  U            useless (entry value with no required use)
  T:<name>     token: RET (return address), C (input byte), X (OUT's R0),
               K1, K2 (entry values of scratch registers)
  Z            x0000
  MP, MN       other values in the MCR base window [xFFDE, x001E]: MP has
               bit 15 clear (x0001-x001E), MN set (xFFDE-xFFFF)
  LO           a value in [x0101, x30FF] (LEA, JSR or TRAP result) that is
               not in the pre-range
  PRE:<w>:<nb> a value in [x01D9, x021C] whose NOT is a base in window(s)
               w (D, K or DK) with low byte nb
  WIN:<w>      a base in the DDR window [xFDE7, xFE26] (D), the KBDR window
               [xFDE3, xFE22] (K) or both (DK)
Over-approximations (each only lowers the minimum): any ALU immediate op may
set the low byte to any value of interest while keeping the class; a load may
produce any WIN or Z directly; NOT of a WIN with an unknown low byte may give a
PRE whose own low byte is the linefeed.
Omitted (justified by lemma L7 in docs/bundled-os-proof.md): ADD/AND of two registers,
NOT of a LO value, values with high byte outside [x01, x30] and the windows.
"""

import argparse
import heapq
from dataclasses import dataclass

HALT_MESSAGE = "\n--- halted ---\n"
WINDOWS = ("D", "K", "DK")


@dataclass(frozen=True)
class Costs:
    a: int
    b: int
    m: int
    i: int


def costs_at(n: int) -> Costs:
    return Costs(a=4 + n, b=5 + n, m=5 + 2 * n, i=6 + 3 * n)


@dataclass(frozen=True)
class Routine:
    name: str
    registers: tuple
    entry: tuple
    events: tuple
    final: tuple
    finish: str


def low_bytes(routine):
    chars = sorted({ord(e[1]) for e in routine.events if e[0] == "char"})
    return tuple(chars) + (0x00, None)


def is_token(v):
    return v[0].startswith("T:")


def in_ddr(v):
    return v[0] in ("WIN:D", "WIN:DK")


def in_kbdr(v):
    return v[0] in ("WIN:K", "WIN:DK")


def in_mcr(v):
    return v[0] in ("Z", "MP", "MN")


def alu_targets(value, bytes_):
    """One ALU-immediate instruction on `value`: every class it may yield."""
    cls = value[0]
    out = {("Z", 0x00), ("MP", None)}
    if cls in ("Z", "MP", "MN"):
        return out | {("MN", None)}
    if cls == "U" or is_token(value):
        return out
    if cls == "LO":
        return out | {("LO", b) for b in bytes_ if b is not None}
    if cls.startswith("PRE:"):
        # Any pre-range value with any NOT target; own low byte unknown.
        return out | {(f"PRE:{w}:{nbs}", None) for w in WINDOWS for nbs in nb_names(bytes_)}
    # WIN: any window value with any low byte.
    return out | {("WIN:" + w, b) for w in WINDOWS for b in bytes_}


def not_targets(value, bytes_):
    cls = value[0]
    if cls == "Z":
        return {("MN", None)}
    if cls == "MP":
        return {("MN", None)}
    if cls == "MN":
        return {("MP", None), ("Z", 0x00)}
    if cls.startswith("PRE:"):
        _, w, nb = cls.split(":")
        return {("WIN:" + w, None if nb == "-" else int(nb, 16))}
    if cls.startswith("WIN:"):
        w = cls.split(":")[1]
        nb = "-" if value[1] is None else f"{value[1]:02x}"
        own = {None}
        if value[1] is None:
            own.add(0x0A)
        return {(f"PRE:{w}:{nb}", o) for o in own}
    return set()


def nb_names(bytes_):
    return ["-" if b is None else f"{b:02x}" for b in bytes_]


def lea_targets(bytes_):
    out = {("LO", b) for b in bytes_ if b is not None}
    for w in WINDOWS:
        for nb in bytes_:
            nbs = "-" if nb is None else f"{nb:02x}"
            # Own low byte: x0A only for x020A (NOT = xFDF5, nb None); else None.
            out.add((f"PRE:{w}:{nbs}", None))
            if nb is None:
                out.add((f"PRE:{w}:{nbs}", 0x0A))
    return out


def load_targets(bytes_):
    out = {("Z", 0x00)}
    for w in WINDOWS:
        out |= {("WIN:" + w, b) for b in bytes_}
    return out


def matches(value, char):
    b = ord(char)
    return value[1] == b and (value[0] == "LO" or value[0].startswith(("WIN:", "PRE:")) or value[0] in ("MP", "MN", "Z"))


def successors(routine, state, c, bytes_, vector):
    event, regs, memory = state
    names = routine.registers
    has_d = any(in_ddr(v) for v in regs)
    has_k = any(in_kbdr(v) for v in regs)
    for r, value in enumerate(regs):
        def put(v, r=r):
            return regs[:r] + (v,) + regs[r + 1:]

        for t in lea_targets(bytes_):
            yield c.a, (event, put(t), memory), f"{names[r]} <- LEA {t}"
        for t in load_targets(bytes_):
            yield c.m, (event, put(t), memory), f"{names[r]} <- LD {t}"
        for s, src in enumerate(regs):
            for t in alu_targets(src, bytes_):
                yield c.a, (event, put(t), memory), f"{names[r]} <- ALU {names[s]} {t}"
            for t in not_targets(src, bytes_):
                yield c.a, (event, put(t), memory), f"{names[r]} <- NOT {names[s]} {t}"
            if s != r:
                yield c.a, (event, put(src), memory), f"{names[r]} <- copy {names[s]}"
        for token in memory:
            if token.startswith("vec:"):
                continue
            yield c.m, (event, put((token, None)), memory), f"{names[r]} <- reload {token}"
        if is_token(value):
            yield c.m, (event, regs, memory | {value[0]}), f"spill {names[r]} ({value[0]})"
            if vector and value[0] == "T:RET":
                yield c.i, (event, regs, memory | {"vec:RET"}), f"STI {names[r]} -> vector table"
    if event < len(routine.events):
        kind = routine.events[event]
        if kind[0] == "read":
            cost = c.m if has_k else c.i
            for r, _ in enumerate(regs):
                yield cost, (event + 1, regs[:r] + (("T:C", None),) + regs[r + 1:], memory), f"{names[r]} <- KBDR"
        else:
            cost = c.m if has_d else c.i
            for r, value in enumerate(regs):
                ok = value[0] == "T:" + kind[1] if kind[0] == "token" else matches(value, kind[1])
                if ok:
                    yield cost, (event + 1, regs, memory), f"DDR <- {names[r]}"
                    break


def finish_cost(routine, state, c, vector):
    event, regs, memory = state
    if event != len(routine.events):
        return None
    for have, want in zip(regs, routine.final):
        if want is not None and have[0] != want:
            return None
    if routine.finish == "ret":
        if regs[1][0] == "T:RET":
            return c.a
        if vector and "vec:RET" in memory:
            return c.m
        return None
    # HALT: R0 = Z is required by the contract; a store through any MCR-window
    # base with data whose bit 15 is clear (STR, M), or STI with such data (I).
    if regs[0][0] != "Z":
        return None
    if any(in_mcr(v) for v in regs):
        return c.m
    return c.i


def minimum(routine, c, vector):
    bytes_ = low_bytes(routine)
    start = (0, routine.entry, frozenset())
    best = {start: c.m}
    heap = [(c.m, 0, start)]
    previous = {start: None}
    counter = 0
    while heap:
        cost, _, state = heapq.heappop(heap)
        if cost > best[state]:
            continue
        fin = finish_cost(routine, state, c, vector)
        if fin is not None:
            steps = []
            node = state
            while previous[node] is not None:
                node, step = previous[node]
                steps.append(step)
            steps.reverse()
            return cost + fin, ["TRAP"] + steps + [routine.finish.upper()], len(best)
        for step_cost, nxt, description in successors(routine, state, c, bytes_, vector):
            total = cost + step_cost
            if total < best.get(nxt, total + 1):
                best[nxt] = total
                previous[nxt] = (state, description)
                counter += 1
                heapq.heappush(heap, (total, counter, nxt))
    raise RuntimeError(routine.name)


def routines(nregs):
    names = ("R0", "R7", "R1", "R2")[:nregs]
    scratch = tuple((f"T:K{k}", None) for k in range(1, nregs - 1))
    scratch_final = tuple(f"T:K{k}" for k in range(1, nregs - 1))
    u = ("U", None)
    ret = ("T:RET", None)
    return (
        Routine("GETC", names, (u, ret) + scratch, (("read",),), ("T:C", None) + scratch_final, "ret"),
        Routine("OUT", names, (("T:X", None), ret) + scratch, (("token", "X"),), ("T:X", None) + scratch_final, "ret"),
        Routine("IN", names, (u, ret) + scratch, (("read",), ("token", "C"), ("char", "\n")), ("T:C", None) + scratch_final, "ret"),
        Routine("HALT", names, (u, u) + scratch, tuple(("char", ch) for ch in HALT_MESSAGE), ("Z", None) + scratch_final, "halt"),
    )


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--vector", action="store_true")
    parser.add_argument("--regs", type=int, default=2)
    parser.add_argument("--only", default=None)
    args = parser.parse_args()
    for n in (1, 4):
        c = costs_at(n)
        for routine in routines(args.regs):
            if args.only and routine.name != args.only:
                continue
            cost, steps, visited = minimum(routine, c, args.vector)
            print(f"N={n} {routine.name}: minimum {cost} (regs={args.regs}, vector={args.vector}, states={visited})")
            for step in steps:
                print(f"    {step}")


if __name__ == "__main__":
    main()
