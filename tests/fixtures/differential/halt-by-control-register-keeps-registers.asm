; Ends with a store to the machine control register, so no OS routine restores registers.
        .ORIG x3000
        LD   R0, VALA
        LD   R1, VALB
        ADD  R2, R0, R1
        ADD  R3, R2, #-1
        NOT  R4, R3
        AND  R5, R4, R2
        AND  R6, R5, #15
        ADD  R6, R6, #-16
        ADD  R7, R6, R6
        ST   R7, RESULT
        LD   R1, KEEP
        STI  R1, MCRP
        LD   R1, VALB
        AND  R1, R1, #0
        STI  R1, MCRP
        ADD  R1, R1, #9
VALA    .FILL x7FFF
VALB    .FILL x0001
KEEP    .FILL x8000
RESULT  .FILL x0000
MCRP    .FILL xFFFE
        .END
