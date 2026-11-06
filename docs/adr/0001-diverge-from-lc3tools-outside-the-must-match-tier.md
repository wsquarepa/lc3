# Diverge from lc3tools outside the must-match tier

This project is a faster replacement for lc3tools, not an emulation of it. Only the must-match tier is bound to `lc3sim`: final registers, memory and program output of any program that halts under `lc3sim` with `option device off`, given the same OS. A grader that checks correctness with `lc3sim` and scores performance with this project relies on this tier, since a disagreement there would score a different program than the one checked. Everything else (assembler syntax and strictness, diagnostics, the CLI, how anomalies are reported, and the bundled OS) is ours to change. Cycle counts are ours by definition, since `lc3sim` has none.

## Consequences

The authority for the must-match tier is an installed `lc3sim` binary, measured directly (`docs/lc3sim-reference.md`), not the public RadixSeven mirror, which is version 0.12 against an installed "0.5" release.
