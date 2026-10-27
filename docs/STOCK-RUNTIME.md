# Repeatable stock runtime sampling

`tools/Measure-StockRuntime.ps1` passively samples visible Razer/Synapse/GameManager processes plus the process IDs of matching services inventoried at startup. It does not change processes, service configuration or device state. An empty process set is a measurement failure. Use an execution context that can enumerate the stock processes.

```powershell
tools/Measure-StockRuntime.ps1 -Seconds 60 -Output target/stock-runtime.json
```

The [current 30-second desktop sample](validation/stock-runtime-desktop.json) averaged1,704,211,138 bytes of summed working set and939,385,256 private committed bytes across29 samples. Peak sampled values were1,705,050,112 and940,527,616 bytes. The measured portion of CPU averaged4.89% of one core on a16-logical-processor system. Protected/unreadable CPU counters remain missing rather than zero. No workload was automated; this is not a measurement of all Synapse workflows.

Working sets can count shared resident pages more than once. Private bytes are private committed memory, not private resident memory. Process peaks are sampled; kernel-driver memory is excluded. Matching stock services were running. The inventory and per-sample process counts are retained in the public result without process IDs or local paths.

Replacement runtime reports currently exclude installed stock services and kernel drivers. They must not be presented as the same scope or a complete system-memory saving. Final acceptance still requires representative replacement workflows, sustained host effects, declared service/driver state and broader stock workload baselines.
