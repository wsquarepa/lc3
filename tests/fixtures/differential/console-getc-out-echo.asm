        .ORIG x3000
        LD   R2, COUNT
LOOP    GETC
        OUT
        ADD  R2, R2, #-1
        BRp  LOOP
        HALT
COUNT   .FILL #5
        .END
