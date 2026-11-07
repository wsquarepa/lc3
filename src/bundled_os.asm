; Bundled OS: cycle-optimized TRAP routines for this simulator.
;
; Contract (ADR 0005): GETC, OUT, PUTS, IN, PUTSP and HALT give the same
; program output and R0 as the reference OS and preserve R1-R6; R7 and the
; condition codes are unspecified on return. Devices always read ready, so no
; routine polls a status register. Cycle counts and optimality arguments are in
; docs/bundled-os-proof.md.
;
; Placement follows the Patt and Patel 2nd edition memory map (Figure A.1):
; code and data live only in x0200-x2FFF, and the trap vector table
; x0000-x00FF is written by boot. Nothing is placed in the interrupt vector
; table (x0100-x01FF) or the device page (xFE00-xFFFF).
;
; Device registers are out of PC-relative reach, so a routine reaches them
; either through a pointer (LDI/STI) or through a base register holding
; DDR_NL = xFE0A, from which KBDR is at offset -8 and DDR at -4. The low byte
; of xFE0A is a linefeed, so that base register can also be stored to DDR to
; print '\n'.

	.ORIG x0200

; Boot runs once from x0200 on an all-zero machine and is never counted.
; It fills the trap vector table with UNKNOWN_TRAP, points the six assigned
; vectors at their routines, and halts. R0 is zero on entry.
BOOT	LD R1,UNKNOWN_TRAP
	LD R2,VECTOR_COUNT
BOOT_FILL
	STR R1,R0,#0
	ADD R0,R0,#1
	ADD R2,R2,#-1
	BRp BOOT_FILL
	LD R0,FIRST_VECTOR
	LEA R1,TRAP_GETC
	STR R1,R0,#0
	LEA R1,TRAP_OUT
	STR R1,R0,#1
	LEA R1,TRAP_PUTS
	STR R1,R0,#2
	LEA R1,TRAP_IN
	STR R1,R0,#3
	LEA R1,TRAP_PUTSP
	STR R1,R0,#4
	LD R1,HALT_VECTOR	; TRAP_HALT is beyond LEA reach of boot
	STR R1,R0,#5
	AND R0,R0,#0
	STR R0,R0,#-2		; x0000 - 2 = xFFFE: MCR <- 0 halts boot

; GETC: R0 <- next input byte. LDI is cheaper than building a base register
; for a single device access.
TRAP_GETC
	LDI R0,KBDR_PTR
	RET

; OUT: print R0[7:0]; R0 unchanged.
TRAP_OUT
	STI R0,DDR_PTR
	RET

; IN: R0 <- next input byte, then echo it and a linefeed (the reference
; prompt is omitted). R7 is spilled so that it can hold DDR_NL, which serves
; as the device base for all three accesses and as the linefeed itself.
TRAP_IN
	ST R7,SAVE_R7
	LD R7,DDR_NL
	LDR R0,R7,#-8		; KBDR
	STR R0,R7,#-4		; echo
	STR R7,R7,#-4		; '\n' from the base's low byte
	LD R7,SAVE_R7
	RET


; PUTSP: print each word's low byte, then its high byte, stopping at the first
; NUL byte in either position; R0 restored.
;
; The high byte h cannot reach a register's low byte by arithmetic (carries
; only propagate upward), so it is found by a 3-level branch tree on W[15:13]
; followed by a table lookup on W[12:8]: HIGH_TABLE + (W & x1F00) + W[15:13]
; holds h, with x0000 for h = 0.
;
; Registers: R0 = pointer, R7 = word W, R1 = DDR_NL, R2 = x00FF,
; R3 = x1F00, R4 = HIGH_TABLE, R5 = shifted copy of W or the low-byte test.
; Leaf names give W[15:13]. Leaf 011 (h = x60-x7F, lower-case ASCII) falls
; through into the next word; every other leaf branches back.
TRAP_PUTSP
	ST R7,SAVE_R7
	ST R0,SAVE_R0
	ST R1,SAVE_R1
	ST R2,SAVE_R2
	ST R3,SAVE_R3
	ST R4,SAVE_R4
	ST R5,SAVE_R5
	LD R1,DDR_NL
	LD R2,LOW_MASK
	LD R3,HIGH_MASK
	LD R4,HIGH_TABLE_PTR
	BRnzp PUTSP_WORD
PUTSP_01
	ADD R5,R5,R5		; sign = W[13]
	BRzp PUTSP_010
PUTSP_011
	AND R7,R7,R3
	ADD R7,R7,R4
	LDR R7,R7,#3
	STR R7,R1,#-4
	ADD R0,R0,#1
PUTSP_WORD
	LDR R7,R0,#0		; sign = W[15]
	BRn PUTSP_1
	AND R5,R7,R2
	BRz PUTSP_EXIT		; low byte NUL
	STR R7,R1,#-4
	ADD R5,R7,R7		; sign = W[14]
	BRn PUTSP_01
	ADD R5,R5,R5		; sign = W[13]
	BRn PUTSP_001
PUTSP_000
	AND R7,R7,R3
	ADD R7,R7,R4
	LDR R7,R7,#0
	BRz PUTSP_EXIT		; high byte NUL: only possible when W[15:13] = 0
	STR R7,R1,#-4
	ADD R0,R0,#1
	BRnzp PUTSP_WORD
PUTSP_001
	AND R7,R7,R3
	ADD R7,R7,R4
	LDR R7,R7,#1
	STR R7,R1,#-4
	ADD R0,R0,#1
	BRnzp PUTSP_WORD
PUTSP_010
	AND R7,R7,R3
	ADD R7,R7,R4
	LDR R7,R7,#2
	STR R7,R1,#-4
	ADD R0,R0,#1
	BRnzp PUTSP_WORD
PUTSP_1
	AND R5,R7,R2
	BRz PUTSP_EXIT		; low byte NUL
	STR R7,R1,#-4
	ADD R5,R7,R7		; sign = W[14]
	BRn PUTSP_11
	ADD R5,R5,R5		; sign = W[13]
	BRn PUTSP_101
PUTSP_100
	AND R7,R7,R3
	ADD R7,R7,R4
	LDR R7,R7,#4
	STR R7,R1,#-4
	ADD R0,R0,#1
	BRnzp PUTSP_WORD
PUTSP_101
	AND R7,R7,R3
	ADD R7,R7,R4
	LDR R7,R7,#5
	STR R7,R1,#-4
	ADD R0,R0,#1
	BRnzp PUTSP_WORD
PUTSP_11
	ADD R5,R5,R5		; sign = W[13]
	BRn PUTSP_111
PUTSP_110
	AND R7,R7,R3
	ADD R7,R7,R4
	LDR R7,R7,#6
	STR R7,R1,#-4
	ADD R0,R0,#1
	BRnzp PUTSP_WORD
PUTSP_111
	AND R7,R7,R3
	ADD R7,R7,R4
	LDR R7,R7,#7
	STR R7,R1,#-4
	ADD R0,R0,#1
	BRnzp PUTSP_WORD
PUTSP_EXIT
	LD R5,SAVE_R5
	LD R4,SAVE_R4
	LD R3,SAVE_R3
	LD R2,SAVE_R2
	LD R1,SAVE_R1
	LD R0,SAVE_R0
	LD R7,SAVE_R7
	RET

; Constants and spill slots, placed between the routines so that every LD and
; ST that uses them is in PC-relative range. TRAP routines never call each
; other, so they share one set of spill slots.
DDR_NL		.FILL xFE0A
KBDR_PTR	.FILL xFE02
DDR_PTR		.FILL xFE06
LOW_MASK	.FILL x00FF
HIGH_MASK	.FILL x1F00
HIGH_TABLE_PTR	.FILL HIGH_TABLE
PUTS_STRIDE	.FILL #64
UNKNOWN_TRAP	.FILL xFFFE
VECTOR_COUNT	.FILL #256
FIRST_VECTOR	.FILL x0020
HALT_VECTOR	.FILL TRAP_HALT
SAVE_R0		.BLKW 1
SAVE_R1		.BLKW 1
SAVE_R2		.BLKW 1
SAVE_R3		.BLKW 1
SAVE_R4		.BLKW 1
SAVE_R5		.BLKW 1
SAVE_R7		.BLKW 1


; PUTS: print the low byte of each word from M[R0] up to the first x0000
; word; R0 restored. The loop body handles 64 words with one pointer ADD of
; the stride R2 = 64, using LDR offsets -32..31 around R0; 64 words is the
; most one direct back edge can span (193 words <= 256). Loop invariants:
; R7 = the word just loaded (already known non-zero), R1 = DDR_NL, and the
; next word is at R0 + its offset in the body. An empty string exits on the
; taken BRz so that every non-empty string gets the cheaper not-taken branch.
TRAP_PUTS
	ST R7,SAVE_R7
	LDR R7,R0,#0
	BRz PUTS_EMPTY
	ST R0,SAVE_R0
	ST R1,SAVE_R1
	ST R2,SAVE_R2
	LD R1,DDR_NL
	LD R2,PUTS_STRIDE
PUTS_LOOP
	STR R7,R1,#-4		; word at offset 0
	LDR R7,R0,#1
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#2
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#3
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#4
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#5
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#6
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#7
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#8
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#9
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#10
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#11
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#12
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#13
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#14
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#15
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#16
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#17
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#18
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#19
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#20
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#21
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#22
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#23
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#24
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#25
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#26
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#27
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#28
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#29
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#30
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#31
	BRz PUTS_END
	STR R7,R1,#-4
	ADD R0,R0,R2		; next 64-word window: offsets -32..31
	LDR R7,R0,#-32
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#-31
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#-30
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#-29
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#-28
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#-27
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#-26
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#-25
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#-24
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#-23
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#-22
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#-21
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#-20
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#-19
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#-18
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#-17
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#-16
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#-15
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#-14
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#-13
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#-12
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#-11
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#-10
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#-9
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#-8
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#-7
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#-6
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#-5
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#-4
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#-3
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#-2
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#-1
	BRz PUTS_END
	STR R7,R1,#-4
	LDR R7,R0,#0
	BRnp PUTS_LOOP		; the next iteration's first check is the back edge
PUTS_END
	LD R2,SAVE_R2
	LD R1,SAVE_R1
	LD R0,SAVE_R0
PUTS_EMPTY
	LD R7,SAVE_R7
	RET

; HALT: print "\n--- halted ---\n", then halt with R0 = x0000 (as the
; reference OS leaves it) and R1-R6 untouched. R7 = DDR_NL serves as the device
; base and as both linefeeds; R0 carries every other character. LEA targets
; are the CHAR_* addresses below, chosen for their low bytes; the other
; characters are ADD steps from the previous one. The padding before it puts
; HALT within LEA reach of those addresses, so HALT keeps its own copy of
; DDR_NL.
	.BLKW x8A
HALT_BASE	.FILL xFE0A
TRAP_HALT
	LD R7,HALT_BASE
	STR R7,R7,#-4		; '\n'
	LEA R0,CHAR_DASH
	STR R0,R7,#-4		; '-'
	STR R0,R7,#-4		; '-'
	STR R0,R7,#-4		; '-'
	ADD R0,R0,#-13
	STR R0,R7,#-4		; ' '
	LEA R0,CHAR_H
	STR R0,R7,#-4		; 'h'
	ADD R0,R0,#-7
	STR R0,R7,#-4		; 'a'
	ADD R0,R0,#11
	STR R0,R7,#-4		; 'l'
	ADD R0,R0,#8
	STR R0,R7,#-4		; 't'
	ADD R0,R0,#-15
	STR R0,R7,#-4		; 'e'
	ADD R0,R0,#-1
	STR R0,R7,#-4		; 'd'
	LEA R0,CHAR_SPACE
	STR R0,R7,#-4		; ' '
	ADD R0,R0,#13
	STR R0,R7,#-4		; '-'
	STR R0,R7,#-4		; '-'
	STR R0,R7,#-4		; '-'
	STR R7,R7,#-4		; '\n'
	AND R0,R0,#0
	STR R0,R0,#-2		; x0000 - 2 = xFFFE: MCR <- 0 halts


; HIGH_TABLE: row n (n = W[12:8]) starts at HIGH_TABLE + n * x100; entry t
; (t = W[15:13]) holds h = t * x20 + n. Rows sit at x0400-x2307, inside the
; OS region, and the gaps between them are unused.
HIGH_TABLE
	.FILL x00
	.FILL x20
	.FILL x40
	.FILL x60
	.FILL x80
	.FILL xA0
	.FILL xC0
	.FILL xE0
; Address constants for HALT's LEAs: only the low byte of each address
; matters (x20 ' ', x2D '-', x68 'h'). They sit in the gap after the first
; table row at x0420, x042D and x0468; tests/bundled_os.rs checks the message.
	.BLKW x18
CHAR_SPACE	.BLKW 1
	.BLKW x0C
CHAR_DASH	.BLKW 1
	.BLKW x3A
CHAR_H		.BLKW 1
	.BLKW x97
	.FILL x01
	.FILL x21
	.FILL x41
	.FILL x61
	.FILL x81
	.FILL xA1
	.FILL xC1
	.FILL xE1
	.BLKW xF8
	.FILL x02
	.FILL x22
	.FILL x42
	.FILL x62
	.FILL x82
	.FILL xA2
	.FILL xC2
	.FILL xE2
	.BLKW xF8
	.FILL x03
	.FILL x23
	.FILL x43
	.FILL x63
	.FILL x83
	.FILL xA3
	.FILL xC3
	.FILL xE3
	.BLKW xF8
	.FILL x04
	.FILL x24
	.FILL x44
	.FILL x64
	.FILL x84
	.FILL xA4
	.FILL xC4
	.FILL xE4
	.BLKW xF8
	.FILL x05
	.FILL x25
	.FILL x45
	.FILL x65
	.FILL x85
	.FILL xA5
	.FILL xC5
	.FILL xE5
	.BLKW xF8
	.FILL x06
	.FILL x26
	.FILL x46
	.FILL x66
	.FILL x86
	.FILL xA6
	.FILL xC6
	.FILL xE6
	.BLKW xF8
	.FILL x07
	.FILL x27
	.FILL x47
	.FILL x67
	.FILL x87
	.FILL xA7
	.FILL xC7
	.FILL xE7
	.BLKW xF8
	.FILL x08
	.FILL x28
	.FILL x48
	.FILL x68
	.FILL x88
	.FILL xA8
	.FILL xC8
	.FILL xE8
	.BLKW xF8
	.FILL x09
	.FILL x29
	.FILL x49
	.FILL x69
	.FILL x89
	.FILL xA9
	.FILL xC9
	.FILL xE9
	.BLKW xF8
	.FILL x0A
	.FILL x2A
	.FILL x4A
	.FILL x6A
	.FILL x8A
	.FILL xAA
	.FILL xCA
	.FILL xEA
	.BLKW xF8
	.FILL x0B
	.FILL x2B
	.FILL x4B
	.FILL x6B
	.FILL x8B
	.FILL xAB
	.FILL xCB
	.FILL xEB
	.BLKW xF8
	.FILL x0C
	.FILL x2C
	.FILL x4C
	.FILL x6C
	.FILL x8C
	.FILL xAC
	.FILL xCC
	.FILL xEC
	.BLKW xF8
	.FILL x0D
	.FILL x2D
	.FILL x4D
	.FILL x6D
	.FILL x8D
	.FILL xAD
	.FILL xCD
	.FILL xED
	.BLKW xF8
	.FILL x0E
	.FILL x2E
	.FILL x4E
	.FILL x6E
	.FILL x8E
	.FILL xAE
	.FILL xCE
	.FILL xEE
	.BLKW xF8
	.FILL x0F
	.FILL x2F
	.FILL x4F
	.FILL x6F
	.FILL x8F
	.FILL xAF
	.FILL xCF
	.FILL xEF
	.BLKW xF8
	.FILL x10
	.FILL x30
	.FILL x50
	.FILL x70
	.FILL x90
	.FILL xB0
	.FILL xD0
	.FILL xF0
	.BLKW xF8
	.FILL x11
	.FILL x31
	.FILL x51
	.FILL x71
	.FILL x91
	.FILL xB1
	.FILL xD1
	.FILL xF1
	.BLKW xF8
	.FILL x12
	.FILL x32
	.FILL x52
	.FILL x72
	.FILL x92
	.FILL xB2
	.FILL xD2
	.FILL xF2
	.BLKW xF8
	.FILL x13
	.FILL x33
	.FILL x53
	.FILL x73
	.FILL x93
	.FILL xB3
	.FILL xD3
	.FILL xF3
	.BLKW xF8
	.FILL x14
	.FILL x34
	.FILL x54
	.FILL x74
	.FILL x94
	.FILL xB4
	.FILL xD4
	.FILL xF4
	.BLKW xF8
	.FILL x15
	.FILL x35
	.FILL x55
	.FILL x75
	.FILL x95
	.FILL xB5
	.FILL xD5
	.FILL xF5
	.BLKW xF8
	.FILL x16
	.FILL x36
	.FILL x56
	.FILL x76
	.FILL x96
	.FILL xB6
	.FILL xD6
	.FILL xF6
	.BLKW xF8
	.FILL x17
	.FILL x37
	.FILL x57
	.FILL x77
	.FILL x97
	.FILL xB7
	.FILL xD7
	.FILL xF7
	.BLKW xF8
	.FILL x18
	.FILL x38
	.FILL x58
	.FILL x78
	.FILL x98
	.FILL xB8
	.FILL xD8
	.FILL xF8
	.BLKW xF8
	.FILL x19
	.FILL x39
	.FILL x59
	.FILL x79
	.FILL x99
	.FILL xB9
	.FILL xD9
	.FILL xF9
	.BLKW xF8
	.FILL x1A
	.FILL x3A
	.FILL x5A
	.FILL x7A
	.FILL x9A
	.FILL xBA
	.FILL xDA
	.FILL xFA
	.BLKW xF8
	.FILL x1B
	.FILL x3B
	.FILL x5B
	.FILL x7B
	.FILL x9B
	.FILL xBB
	.FILL xDB
	.FILL xFB
	.BLKW xF8
	.FILL x1C
	.FILL x3C
	.FILL x5C
	.FILL x7C
	.FILL x9C
	.FILL xBC
	.FILL xDC
	.FILL xFC
	.BLKW xF8
	.FILL x1D
	.FILL x3D
	.FILL x5D
	.FILL x7D
	.FILL x9D
	.FILL xBD
	.FILL xDD
	.FILL xFD
	.BLKW xF8
	.FILL x1E
	.FILL x3E
	.FILL x5E
	.FILL x7E
	.FILL x9E
	.FILL xBE
	.FILL xDE
	.FILL xFE
	.BLKW xF8
	.FILL x1F
	.FILL x3F
	.FILL x5F
	.FILL x7F
	.FILL x9F
	.FILL xBF
	.FILL xDF
	.FILL xFF
	.END
