        .ORIG x3000
        LD   R0, WORD1
        LEA  R1, TABLE
        LDR  R2, R1, #2
        LDR  R3, R1, #-1
        LDI  R4, PTR
        ADD  R5, R0, R2
        STR  R5, R1, #5
        ST   R4, SAVED
        LEA  R6, BACK
        STI  R3, PTR2
        LDR  R7, R6, #0
        STR  R0, R1, #-2
        LD   R1, KEEP
        LDI  R1, MCRP2
        AND  R1, R1, #0
        STI  R1, MCRP
        ADD  R1, R1, #1
        .FILL x0000
BACK    .FILL x1234
WORD1   .FILL xFF00
        .FILL x0F0F
TABLE   .FILL x1111
        .FILL x2222
        .FILL x3333
        .FILL x4444
        .FILL x5555
        .FILL x6666
        .FILL x7777
PTR     .FILL TABLE
PTR2    .FILL SAVED
SAVED   .FILL x0000
KEEP    .FILL x8000
MCRP    .FILL xFFFE
MCRP2   .FILL MCRP
        .END
