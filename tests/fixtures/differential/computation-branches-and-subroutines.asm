        .ORIG x3000
        AND  R0, R0, #0
        AND  R1, R1, #0
        ADD  R2, R0, #10
SUM     ADD  R1, R1, R2
        ADD  R2, R2, #-1
        BRp  SUM
        BRz  ZERO
        ADD  R0, R0, #1
ZERO    ADD  R0, R0, #2
        ADD  R3, R1, #-15
        BRn  NEG
        BRz  EQ
        ADD  R0, R0, #4
        BRnzp JOIN
NEG     ADD  R0, R0, #8
EQ      ADD  R0, R0, #15
JOIN    LEA  R4, DOUBLE
        JSRR R4
        JSR  TRIPLE
        LEA  R5, AFTER
        JMP  R5
        ADD  R0, R0, #1
AFTER   ST   R0, OUT0
        ST   R1, OUT1
        AND  R2, R2, #0
        STI  R2, MCRP
DOUBLE  ADD  R1, R1, R1
        RET
TRIPLE  ST   R7, SAVE7
        ADD  R2, R1, R1
        ADD  R1, R1, R2
        LD   R7, SAVE7
        RET
OUT0    .FILL x0000
OUT1    .FILL x0000
SAVE7   .FILL x0000
MCRP    .FILL xFFFE
        .END
