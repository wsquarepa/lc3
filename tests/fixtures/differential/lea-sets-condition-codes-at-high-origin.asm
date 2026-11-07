; Loaded at x8000 so LEA produces a negative result; the branch result lands in R0.
        .ORIG x8000
        AND  R0, R0, #0
        LEA  R1, HERE
        BRn  WASNEG
        ADD  R0, R0, #1
WASNEG  ADD  R0, R0, #2
HERE    LEA  R2, HERE
        BRp  POS
        ADD  R0, R0, #4
POS     ADD  R0, R0, #8
        AND  R3, R3, #0
        STI  R3, MCRP
MCRP    .FILL xFFFE
        .END
