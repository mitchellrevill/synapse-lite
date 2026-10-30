# Repeatable stock runtime sampling

`tools/Measure-StockRuntime.ps1` passively samples visible Razer/Synapse/GameManager processes plus the process IDs of matching services inventoried at startup. It does not change processes, service configuration or device state. An empty process set is a measurement failure. Use an execution context that can enumerate the stock processes.

```powershell
tools/Measure-StockRuntime.ps1 -Seconds 60 -Output target/stock-runtime.json
```

The [historical 30-second desktop sample](validation/stock-runtime-desktop.json) averaged1,704,211,138 bytes of summed working set and939,385,256 private committed bytes across29 samples. Peak sampled values were1,705,050,112 and940,527,616 bytes. The measured portion of CPU averaged4.89% of one core on a16-logical-processor system. Protected/unreadable CPU counters remain missing rather than zero. No workload was automated; this is not a measurement of all Synapse workflows.

Working sets can count shared resident pages more than once. Private bytes are private committed memory, not private resident memory. Process peaks are sampled; kernel-driver memory is excluded. Matching stock services were running. The inventory and per-sample process counts are retained in the public result without process IDs or local paths.

Replacement runtime reports currently exclude installed stock services and kernel drivers. They must not be presented as the same scope or a complete system-memory saving. Final acceptance still requires representative replacement workflows, sustained host effects, declared service/driver state and broader stock workload baselines.

## Current installed build: 4.0.896.29

The [30 October passive census](validation/stock-runtime-4.0.896-ambient.json)
contains three observation windows of 21 samples each, spanning 24.44–25.55 seconds
between first and last readings. These share the existing stock process lifetime;
they are not three app launches or the final replacement acceptance runs.

| Window | Peak summed working set (bytes) | Peak private committed bytes | Mean CPU, one core |
| --- | ---: | ---: | ---: |
| 1 | 942,141,440 | 964,415,488 | 4.59% |
| 2 | 941,854,720 | 963,899,392 | 5.05% |
| 3 | 934,088,704 | 956,071,936 | 4.99% |

Each window observed 26 processes with a stable birth-identity roster. In addition
to the original two Razer directories, explicit roots admit Chroma SDK (10
processes), Experience Service (2), and the per-user LampArray helper (1).
The service census, including process identities, was unchanged before and after.
No apps were launched or closed, no input/settings were scripted, and desktop and
analysis activity was uncontrolled. Three unrelated processes had unreadable image
paths and remain unknown. This is a scoped baseline, not proof of all installed
components or a direct saving relative to the historical sample.

`tools/measure-stock-razer.ps1` now accepts explicit `-AdditionalRoots` and an
optional `-CounterPythonExe`. The latter invokes the developer-only, already
installed psutil backend when CIM/.NET cannot read protected stock service paths
or counters. It does not add Python or psutil to the replacement's runtime.

```powershell
$extra = @(
    'C:/Program Files (x86)/Razer Chroma SDK',
    'C:/Program Files (x86)/RazerExperienceService',
    "$env:LOCALAPPDATA/Razer/RazerAppEngine/User Data/Apps/Common/LampArray"
)
./tools/measure-stock-razer.ps1 -AppEngineExe 'C:/Program Files/Razer/RazerAppEngine/app-4.0.896/RazerAppEngine.exe' -OutputPath target/stock-new.json -Workload 'Describe actual app/service/UI state here' -SampleCount 21 -IntervalSeconds 1 -AdditionalRoots $extra -CounterPythonExe 'C:/path/to/python.exe'
```

Use a host execution context that can see the stock processes. Restricted process
visibility is not evidence that Synapse stopped. Inspect actual installed roots
before supplying them; whole drives, relative paths and wildcard roots refuse.
Names alone do not admit an executable. A named stock candidate outside the scope,
unreadable counters, stale/reused identity, empty census or missing target aborts
without a report. CPU intervals use a monotonic clock. Reports are exclusive new
files and pin the selected executable's SHA256 before/after capture.

The first CIM attempt refused a protected SDK service. The native retry produced
two windows before its collector terminated without a third report; a new third
capture began only after that process was confirmed absent. Raw local records
retain identities/paths; the published summary omits them and hashes every report.
All 34 PowerShell scope/refusal checks and six Python counter/identity checks pass.
Full workload coverage, kernel allocation accounting, complete Rust engine/driver
migration and the three representative sub-100 MB replacement runs remain open.
