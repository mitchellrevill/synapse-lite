# Active Lighting and color-editor memory checkpoint

Six release runs passed with the native tray/editor, actual fixed one-color Breathing worker and observed descendants. Each run lasted15seconds; three presented the Lighting view and three its color editor. This is an active-workload checkpoint, not final acceptance of the complete replacement.

Source commit: `ef214ca`. Executable SHA256: `ED6DAC78826435BF8EE131ABAD340F31AC9527E785107C7C2B53FC9B248B25A3`. [Sanitized sampled evidence](validation/active-lighting-native-memory.json) includes native presentation/readiness, worker shutdown, memory samples and readable controls. [Probe policy](ACTIVE-LIGHTING-PROBE.md) explains the admitted lighting-only scope and extra configuration-validation cost.

| View | Run | Peak working set, bytes | Sampled peak private bytes | Peak paged memory, bytes |
|---|---:|---:|---:|---:|
| Lighting | 1 | 23,838,720 | 9,732,096 | 9,732,096 |
| Lighting | 2 | 23,871,488 | 9,732,096 | 9,740,288 |
| Lighting | 3 | 23,793,664 | 9,707,520 | 9,707,520 |
| Color editor | 1 | 25,812,992 | 11,329,536 | 11,427,840 |
| Color editor | 2 | 26,034,176 | 11,362,304 | 11,620,352 |
| Color editor | 3 | 25,976,832 | 11,468,800 | 11,468,800 |

All three metrics stayed below100,000,000bytes in every run. One process was observed, with no TCP connection observed by the sampler. Working-set high-water and sampled private bytes describe different memory properties; neither file size nor private bytes alone replaces the combined criteria. Descendants were discovered once per second, so very short-lived helpers and UDP/DNS traffic are outside the observation scope. Sampling and inspection tools are external to the application measurement.

Each native frame report confirmed actual presentation, visibility, three source fonts, source icons/artwork and a recent producer-bound healthy worker observation. The final observations contained209–212 accepted frames, followed by successful bounded worker joins, zero restoration failures and fresh post-join cached error checks. Their last observation ages were399–671ms. All color-editor runs used the actual Breathing projection; no presentation-only Static substitution occurred. These counters establish transport/program progress; they do not measure optical RGB or certify source wall-clock cadence.

GET-only before/after reports matched all recorded control fields: mode3 with a one-byte declaration and unknown parameter; profile0 CustomFrame8/flags0/rate0/count0; profile1 Spectrum3/flags0/rate0/count0; both saved/live brightness255. An absent mode parameter remains unknown, not proven unchanged. No input/mapping, mode or saved-effect write was part of this workload. Stock user processes went11→0→15 (14 engine processes immediately after restart), with services/drivers left running.

The admitted configuration was copied to a fresh directory for each run, remained byte-identical, and already contained native opt-in. Probe-only guards disabled generic profile/input/power/foreground apply paths. Their fresh bounded Store reads and publication checks are included in these measurements. Stock services and installed vendor kernel-driver memory are excluded; the separately compiled unsigned Rust kernel candidate was not loaded. This scope differs from the broader stock app/service baseline and is not a complete system-memory comparison.

The earlier `5EBB761B...C018FE0` active attempt was rejected: native observations starved behind Engine and shutdown produced an unspecific cached error. No RAM result was accepted. [The repair](UNIFORM-SHUTDOWN-STATUS.md) documents the uncertainty and deterministic regressions; this successful checkpoint uses the new executable digest above.

The final-build gate still requires complete supported workflows and broader representative loads, including large profile/macro documents, other effects, wireless/hotplug/sleep and replacement input-driver behavior. Native pixel identity and real Synapse profile re-import acceptance remain unresolved. Earlier idle/read-only-view reports retain their own executable hashes and narrower scopes.
