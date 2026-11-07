        .ORIG x3000
        LEA  R0, PACKED
        PUTSP
        LEA  R0, ODD
        PUTSP
        LEA  R0, MIXED
        PUTSP
        HALT
PACKED  .FILL x6548
        .FILL x6C6C
        .FILL x206F
        .FILL x6F77
        .FILL x6C72
        .FILL x0064
        .FILL x0000
ODD     .FILL x4241
        .FILL x0043
        .FILL x0000
MIXED   .FILL x0A21
        .FILL x0000
        .END
