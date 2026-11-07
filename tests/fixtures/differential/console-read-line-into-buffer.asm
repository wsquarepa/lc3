        .ORIG x3000
        LEA  R3, BUF
        LD   R4, NEGNL
READ    GETC
        ADD  R5, R0, R4
        BRz  DONE
        STR  R0, R3, #0
        ADD  R3, R3, #1
        BRnzp READ
DONE    AND  R0, R0, #0
        STR  R0, R3, #0
        LEA  R0, BUF
        PUTS
        LD   R0, NL
        OUT
        HALT
NEGNL   .FILL xFFF6
NL      .FILL x000A
BUF     .BLKW 32
        .END
