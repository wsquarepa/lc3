        .ORIG x3000
        LD   R0, MARK
        OUT
        TRAP x30
        LD   R0, MARK
        OUT
        HALT
MARK    .FILL x0021
        .END
