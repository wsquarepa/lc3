# Mid-run pokes are PC triggers

A case may carry triggers: sets of register and memory pokes applied the first time execution reaches a given PC, before that instruction executes, each firing at most once and costing no cycles. Some operating systems overwrite state at a known PC during a run, for example a pseudo-random generator that seeds itself from keypress timing; a poke applied at start is overwritten, so a deterministic run must poke once that PC is reached.

## Considered Options

Modifying such an OS to read its state from a fixed address was rejected: expected outputs come from `lc3sim` with the unmodified OS, so cycle counts would measure a different OS and reproduction would depend on editing it exactly right.

## Consequences

A run without triggers uses the plain execution loop; with triggers it runs to the next trigger PC, applies its pokes and continues, so the per-instruction cost stays a single comparison at most.
