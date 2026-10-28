# Stock Chroma resource baseline,28October2026

[Evidence](validation/stock-studio-baseline-2026-10-28.json) records12 samples
per phase, approximately2seconds apart. Scope includes Razer executables under
both Program Files roots; the UI workload is Chroma, not full Synapse hardware
configuration. Installed AppEngine4.0.896's executable hash and raw-trace hashes
are retained with the measurements.

| Workload | Peak summed working set, bytes | Peak private bytes | Mean machine CPU |
|---|---:|---:|---:|
| Startup, Updating screen observed | 2,458,288,128 | 1,337,597,952 | Not derived |
| Studio open on existing Fire/Spectrum composition, no editing | 2,139,512,832 | 1,238,245,376 | 5.11% |

The Studio WS peak covered12 AppEngine processes and one RzEngineMon. Machine
CPU is the sum of per-process CPU deltas divided by actual elapsed time and16
logical processors. First-sample CPU is omitted; new-process CPU can be included
when its start lies in the sampling interval. The raw Studio trace keeps per-PID
start time, counters and timestamps privately in the task workspace.

These are sampled peaks. Working-set sums may count shared pages more than once;
short-lived spikes, exited-process tail CPU and kernel allocations are not fully
measured. Other desktop applications remained open. No full-build replacement
comparison or three-run100MB acceptance is implied. AppEngine was gracefully
closed after the export/measurements and observed exited.

## Repeating a sample

Open the stock workload and let startup finish. Run
`tools/measure-stock-razer.ps1` with explicit `-AppEngineExe`, a new `-OutputPath`
and a descriptive `-Workload`. Defaults are12 samples at2second intervals;
`-SampleCount` and `-IntervalSeconds` can change those bounds. The tool only reads
process counters and writes a new report; it does not launch apps or alter
services, drivers, profiles or settings. Executable paths must be readable for
process inclusion, so use equivalent access when comparing runs. The measured
capture used elevated read access. Record any inaccessible process evidence or
scope changes instead of silently treating an incomplete group as equivalent.

The reusable tool retains the counter/sampling logic used here and adds explicit
target/workload arguments and exclusive report creation. Its parameterized
wrapper passed PowerShell syntax validation; the underlying sampling logic
produced both traces above. Complete replacement baselines still need the final
retained workload, all required helpers and three representative runs.
