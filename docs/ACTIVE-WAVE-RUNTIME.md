# Physical Wave with native Lighting: resource checkpoint

Six fifteen-second release runs exercised the ordinary Wave worker and visible native Lighting view: three source Left and three source Right. All passed authenticated readiness, at least a complete 25-frame cycle plus wrap, unchanged configuration bytes and graceful worker shutdown. Reported pre-stop counts were 202–208 accepted frames per run; each therefore covered at least eight complete cycles. These are accepted USB frame counts, not optical observations or stock timing certification.

The 2,252,800-byte executable SHA256 is `FFF861C130B931D993E7EACDFB0E502B738BD5AA9A8952E6BC694D2450B8DDBA`. Runtime sources match `3fe483a`; the exact build used staged tree `683bfaa3e92acab473403c0c894b807a2cfc0436`. A measurement-script-only ordering fix followed that build; its separately pinned executed SHA256 is `4771DD18BE2A70B61537E7C2BCB31A4ACC5CDE842FA21B00F7753D4F79A5E471`. No runtime Rust source changed in that correction.

| Direction | Run | Reported accepted frames | Peak working set | Sampled peak private | Peak paged memory |
| --- | ---: | ---: | ---: | ---: | ---: |
| Left | 1 | 203 | 25,632,768 | 10,477,568 | 10,477,568 |
| Left | 2 | 202 | 24,809,472 | 10,461,184 | 10,461,184 |
| Left | 3 | 204 | 24,748,032 | 10,436,608 | 10,477,568 |
| Right | 1 | 208 | 25,366,528 | 10,932,224 | 10,977,280 |
| Right | 2 | 204 | 24,829,952 | 10,461,184 | 10,498,048 |
| Right | 3 | 204 | 24,895,488 | 10,854,400 | 10,862,592 |

All memory metrics are bytes and below 100,000,000. The sampler observed one replacement process and no TCP connections. Working set uses process-tree high water; private committed bytes are sampled and may miss brief peaks. Shared pages can be counted per process, and short-lived descendants or UDP/DNS traffic are not covered. The measurement excludes installed stock services and kernel drivers.

Every native report confirmed the requested view was matched, presented and visible with three source fonts loaded. Both native and worker reports identified source4/native2 and authenticated preparation from a fresh layout6/edition0 identity observation and the pinned 2,895,966-byte manifest. The worker's observed metadata is cached evidence with a conservative age, not a fresh identity/control query at draw time or an ownership lease. Pre-stop publication ages were 547–682 ms. Clean join and post-stop fault checks passed in every run.

GET-only snapshots before and after the six runs matched: mode3, live profile0 CustomFrame8 flags0/rate0/count0, saved profile1 effect3, and live/saved raw brightness255. Mode declares only one byte, so its optional parameter remains unknown. Preflight performs no SET; runtime brightness reconciliation may reapply the already configured LIVE value after a completed frame. No saved brightness, mode acquisition, generic profile/input/power replay or configuration write was requested by this probe policy.

Stock Razer user processes were 15 before isolation, zero at two stable observations and 15 after hidden restart. Services and drivers were unchanged. Stock restart is process recovery, not proof of restoring previous RGB pixels, animation phase or input state.

One earlier attempt failed in the measurement script before launching the app because report validation was placed in setup. It produced no worker, samples or Wave frames, is excluded, and restarted stock. The corrected script validates reports after the bounded run. [Sanitized evidence](validation/active-wave-native-memory.json) records this exclusion, per-run measurements, control snapshots, exact identities and limits.

This checkpoint does not include the later natural-text rendering pilot or a replacement kernel driver. Broader effects/workflows, large documents while streaming, UI edits, suppression/resume, hotplug, sleep/wake, abrupt termination, optical placement and final-build memory acceptance remain open. See [probe policy](ACTIVE-WAVE-PROBE.md), [host Wave](HOST-WAVE.md) and the earlier [two-frame diagnostic](PHYSICAL-WAVE.md).
