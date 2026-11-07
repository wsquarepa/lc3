        .ORIG x3000
        IN
        ST   R0, FIRST
        IN
        ST   R0, SECOND
        HALT
FIRST   .FILL x0000
SECOND  .FILL x0000
        .END
