; Stores R0-R7 before executing anything else, so the memory image records the start state.
        .ORIG x3000
        ST   R0, S0
        ST   R1, S1
        ST   R2, S2
        ST   R3, S3
        ST   R4, S4
        ST   R5, S5
        ST   R6, S6
        ST   R7, S7
        HALT
S0      .FILL xDEAD
S1      .FILL xDEAD
S2      .FILL xDEAD
S3      .FILL xDEAD
S4      .FILL xDEAD
S5      .FILL xDEAD
S6      .FILL xDEAD
S7      .FILL xDEAD
        .END
