        .ORIG x3000
        LD   R2, COUNT
POLL    LDI  R1, KBSR
        BRzp POLL
        LDI  R0, KBDR
        ST   R0, LAST
        ADD  R2, R2, #-1
        BRp  POLL
        HALT
COUNT   .FILL #2
LAST    .FILL x0000
KBSR    .FILL xFE00
KBDR    .FILL xFE02
        .END
