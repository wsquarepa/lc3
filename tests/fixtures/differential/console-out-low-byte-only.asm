        .ORIG x3000
        LD   R0, WIDE
        OUT
        LD   R0, NEWLINE
        OUT
        LD   R0, HIGHONLY
        OUT
        LD   R0, TILDE
        OUT
        HALT
WIDE    .FILL x1241
NEWLINE .FILL x000A
HIGHONLY .FILL xFF00
TILDE   .FILL x017E
        .END
