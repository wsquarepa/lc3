# Performance

Measured on 2026-11-06 with an AMD Ryzen AI 9 365 w/ Radeon 880M,
Linux x86_64, pinned to logical core 0, frequency boost enabled, and
rustc 1.98.0 (88d9e12ae 2026-08-18), default Cargo release profile.

```sh
taskset -c 0 cargo run --release --example throughput
```

The example prepares one machine with `reference/lc3tools/lc3os.obj` and an
original eight-character transform. Each case executes GETC, 63 iterations of
arithmetic and memory work per character, OUT, PUTS, and TRAP x25. It runs
3,000 distinct inputs per mode after 100 warmups per mode, alternating mode
order. Every result must halt with the expected program output, halt output,
and 23,767 cycles. Each case executes 4,080 instructions, including OS routines.

The first pair of the final interleaved baseline/candidate comparison produced
these per-case times in microseconds:

| Version | Mode | Median | p90 | p99 | Max |
| --- | --- | ---: | ---: | ---: | ---: |
| Before tuning | No triggers | 111.739 | 114.625 | 127.598 | 1114.393 |
| After tuning | No triggers | 14.337 | 14.387 | 16.632 | 33.323 |
| Before tuning | One origin trigger | 113.062 | 116.107 | 134.311 | 582.159 |
| After tuning | One origin trigger | 15.579 | 15.660 | 18.044 | 45.505 |

All three final candidate invocations had medians below 16 microseconds in
both modes, meeting the 50 microsecond target. Timing includes snapshot cloning
and result allocation; preparation, input construction, validation, and result
destruction are excluded. Percentiles use nearest rank; median averages the two
middle samples. The core was pinned but not isolated, and maximum times vary.

Private execution passes an owned state box through inlined helpers, avoiding
repeated state copies while preserving pure ownership passing. A fixed-size
owner array removes the fetch-address bounds check. The stack trigger bitset
was slower in interleaved measurements, so the existing pending table remains.
