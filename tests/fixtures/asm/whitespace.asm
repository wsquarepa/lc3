; Original fixture: tabs, CRLF, colons and EOF after .END.
	.OrIg	#12288	; origin

	Long_Label_17 : Alias:	; label-only line
	aDd	r0	, r1 ,	r2	; register commas
	ld	r3,	long_label_17
	.FiLl	ALIAS
	getC
	oUt
	pUtS
	iN
	putsP
	hAlT
	.StRiNgZ	"tab	;quoted comment"	; real comment
	.eNd