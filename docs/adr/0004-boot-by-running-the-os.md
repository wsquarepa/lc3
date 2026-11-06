# Boot by running the OS, not by synthesizing a start state

Every run starts from a boot snapshot taken by actually executing the supplied OS from x0200 until its first machine-control-register halt, then loading the images over that state and setting PC to the first image's origin. `lc3sim` does exactly this, and the registers the user program sees (R1=x7FFF, R7=x0490 with the reference OS) are residue of that boot code, so any OS, including a custom one, reproduces `lc3sim`'s start state by construction. A hard-coded start state would be correct only for the reference OS.

## Consequences

The boot snapshot depends only on the OS, so it is computed once and restored per run; boot cycles are never counted. A boot that does not halt within a fixed internal cap fails the run with a boot reason.
