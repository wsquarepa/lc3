; Original fixture: addresses, PC-relative labels and label spellings.
.ORIG x3000
Start
Second: Third : ADD R0,R0,#0
BRnZp FoRwArD
JSR forward
LD R1,Data
LDI R2,dAtA
LEA R3,FORWARD
ST R4,data
STI R5,DATA
Backward:
BR start
Forward RET
JSR BACKWARD
.fill START
Data .FILL Forward
ADD: .fill aDd
R0: .fill xffff
Adjacent.FILL adjacent
.end
