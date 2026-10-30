# Performance baseline — 30 October 2026

No application refactor is included. The experimental catalogue index was reverted before the final measurements. Only measurement tooling changes.

Normal build: `33e46faceb5ff8843cbd278cc228be2e7c9d04b4`. Binary SHA-256: `005f1a6ecaacfdfab27af9af1d5d519251f9bcad24ee7e3d7c1699f322d71cd6`.

## Normal desktop observation

31 observations over approximately 30 seconds: median working set 84.13 MB; median private memory 61.47 MB; mean CPU 0.044% of machine capacity (16 logical processors). This was the existing desktop session, with no scripted interactions. It is not a controlled active workload.

## Repeated page presentation

Three fresh processes per scenario, eight-second intervals, same packaged release binary/assets, isolated fresh configuration, real connected Ornata, intrinsic query-only policy. Existing normal Lite was gracefully closed and restored. Descendants were enumerated; one app process was observed per run. Filesystem caches were not cleared: these are warm-cache process starts, not cold boot measurements.

| Scenario | Startup median (range), ms | Peak working set, MB | Peak sampled private, MB | Mean warm CPU, machine % |
| --- | ---: | ---: | ---: | ---: |
| chroma | 759 (758–788) | 78.92 | 60.49 | 0.084 |
| dashboard | 767 (752–795) | 78.84 | 61.99 | 0.000 |
| keyboard-lighting | 750 (748–789) | 76.70 | 58.43 | 0.000 |

Startup is process creation to the file recording the first confirmed, visible requested frame. It includes discovery/initialization; it is not click-to-response latency. Memory uses decimal MB; working set is the OS process high-water mark, private memory is sampled at approximately 200 ms, and shared pages may be counted in each descendant. CPU uses deltas after two seconds of warmup.

## Catalogue microbenchmark

334 products; 481 connection IDs; 609 queries per pass including misses. Nine samples, 500 passes each. Median lookup: 1019 ns/query. Median parse and drop: 117.6 microseconds/catalogue. Release profile, metadata only, no device discovery or protocol calls. This is not a UI latency or full device-support benchmark.

## Evaluation and limits

The measured read-only pages fit below 100,000,000 bytes for both memory metrics. The normal observation has roughly 16 MB of working-set headroom, but this does not establish the complete-build memory goal. These short samples cannot establish leak freedom or long-session stability.

The catalogue operation is approximately one microsecond. That does not establish it as a meaningful app bottleneck, so optimizing it is not justified by this baseline alone. Preserve this baseline while restructuring for maintainability, and compare the same binary profile, assets, configurations and workloads after each coherent refactor.

Not covered: active Studio composition preview and device output, macro recording/playback, Linked Games under a real catalogue, input remapping load, long sessions, cold-cache startup, UI interaction latency and the absent mouse Performance page. Query-only runs intentionally disable generic profile/policy application and mutation workers. No full feature-load or RAM acceptance is claimed.

The first stock observation was incomplete: protected Razer service start/CPU counters returned null, and that collector rejected their rows. Only one service had complete counters. This is not evidence that the other services were absent. Normal Lite had no counter errors. Do not use that partial service total as a stock comparison; the full stock application/workload comparison remains unmeasured. Any separate stock-services sample covers its recorded existing service state only. The exploratory indexed benchmark and stale build-cache run are retained under excluded/ and are not baseline evidence.

Raw measurements and hashes are in summary.json and the individual runtime.json files. The current normal-profile integrity check is recorded separately. No new application features or production refactor are part of this checkpoint.
