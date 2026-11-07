        .ORIG x3000
        LEA  R0, TEXT
        PUTS
        LEA  R0, EMPTY
        PUTS
        LEA  R0, TEXT2
        PUTS
        HALT
TEXT    .STRINGZ "tab\there\nsecond line \"quoted\" back\\slash\n"
EMPTY   .STRINGZ ""
TEXT2   .STRINGZ "end"
        .END
