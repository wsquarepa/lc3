# Memory latency is the total cycle cost of each memory-access state

Cycles follow the Patt and Patel 2nd edition LC-3 FSM (Figure C.2). Every non-memory state costs one cycle and every memory-access state costs N cycles in total, where N is the memory latency, configurable per run with a default of 4 and applied to memory-mapped device accesses too. N counts the whole state, not extra wait cycles on top of it, so N=1 reproduces the one-cycle-per-state table and N=4 makes fetch cost 7 cycles. Results report the N used, because counts are comparable only at equal N.
