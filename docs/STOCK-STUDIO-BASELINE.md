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

## Capture guards, 29 October 2026

New captures use schema version 2. Each sample requires the selected executable
to be present, a nonempty process roster, readable counters and matching process
creation identities. A known stock process name with an unreadable executable
path or a path outside the documented roots aborts the capture for scope review.
An exited, replaced or unreadable process aborts the run rather than contributing
zero. The selected executable hash must also remain unchanged. Reports are created
only after every sample succeeds, using exclusive creation; failures do not write
a partial or zero-memory baseline. Process counter handles are disposed each read.

`tools/Test-StockRuntimeCounters.ps1` passes 16 deterministic checks, including
the actual report writer's refusal to create a file for an empty inventory.
These fixtures do not measure stock or the replacement. No matching stock process
was visible during the latest read-only process-name check, so a fresh stock
capture remains TODO: open a representative stock workload, establish readable
scope including any components outside these roots, and then run the capture.
Do not repeat empty samples or infer zero usage. The historical measurements
above are retained unchanged. Full replacement three-run acceptance remains open.
