; Original fixture: intervening quotes prevent multi-line string matches.
.ORIG x3000
.STRINGZ "C:\\"
.STRINGZ "later string"
.STRINGZ "D:\\" ; quote " blocks extension
HALT ; later quote "
.END
