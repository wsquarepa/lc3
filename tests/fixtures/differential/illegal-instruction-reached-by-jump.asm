        .ORIG x3000
        LEA  R2, TARGET
        ADD  R2, R2, #1
        JMP  R2
        HALT
TARGET  .FILL x0000
        .FILL xD123
        HALT
        .END
