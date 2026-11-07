; Reads four bytes, including values above x7F and a NUL, storing and echoing each.
        .ORIG x3000
        LEA  R3, BUF
        LD   R2, COUNT
LOOP    GETC
        STR  R0, R3, #0
        OUT
        ADD  R3, R3, #1
        ADD  R2, R2, #-1
        BRp  LOOP
        HALT
COUNT   .FILL #4
BUF     .BLKW 4
        .END
