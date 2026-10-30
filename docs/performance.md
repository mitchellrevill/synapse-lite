# Performance measurement

The target is below **100,000,000 bytes** for both total working set and private committed memory across representative runs. Do not advertise an early idle result as the current full-build footprint.

- **Working set:** physical pages currently resident for a process, including shared pages.
- **Private memory:** committed memory private to that process; it need not all be resident.
- **CPU:** report the sampling interval and whether percentages are per-core or normalized across the machine.

Use `tools/Measure-Runtime.ps1` for bounded app page-presentation measurements, `tools/measure-process-baseline.ps1` for existing-process sampling, and `tools/Measure-StockRuntime.ps1` for the comparison application. Read their parameters before running. These are explicit tools, not automatic background collectors.

Record the source commit, binary hash, asset pack, configuration, requested page/workload, duration, sample interval, child processes and missing counters. Keep raw measurements in private output directories. Count descendants and compare the same workloads before and after changes. A protected process with unavailable counters is missing data, not zero memory.

Page-presentation smoke runs are query-only and do not establish active Studio output, recording/playback load, leak freedom or full UI latency. Final acceptance needs repeated active workloads, recovery and longer sessions; it remains open. No runtime policy, allocation strategy or CLI behavior is changed by the repository cleanup.
