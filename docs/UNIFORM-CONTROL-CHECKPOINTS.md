# Uniform control sampling checkpoint 1

The 9b22bba baseline physically achieved 10.84 subsequent Spectrum fps and 10.89 Breathing fps, with the original worker policy. This implementation changes control sampling; it has simulator coverage only and no new achieved-rate claim. Core frame bytes, fresh mode admission, no-ACK OUT behavior, cancellation/deadline and no-retry semantics remain intact. The worker still parks at least 8 ms; no timer or resolution setting changed.

## Source authority versus local policy

Retained stock Ornata chunk 2890 SHA256 `1e49e375d27a457dadcf525f2032d66c4d3e7d9330eb34afca279d5633a1b3cd`: frame writer at UTF8 offset 1228, multi-transfer branch 1428, `controlTransferOuts` 1986, row `checkUSBTransferIn:false` 2466, flush 2907. It packs six row bodies and a CustomFrame 8 commit behind local `isCustomFrame`. Its fallback disables transfer-in and flushes a batch. There is no per-frame mode/effect GET in that writer. This supports distinguishing frame OUT from control observation; it does not establish our observation interval or exclusive ownership.

The native API accepting concatenated 90-byte bodies is not proof that a Windows HID feature report may carry 630 protocol bytes. Rust continues seven independent canonical 91-byte feature calls, with no concatenation or new proprietary driver dependency.

## Admission policy

- Explicit source-effect actions retain strict pre/post control reads through existing `io::submit`.
- A worker session does a complete control checkpoint initially and after suppression/resume, owner block/re-entry, effective profile/program change, reconnect or explicit recovery. Ordinary frames retain the core fresh mode GET. Full checkpoints additionally read CustomFrame before the batch and mode/CustomFrame afterward.
- A local 250 ms refresh target is measured conservatively from the checkpoint start, not its response completion. A separate 500 ms maximum age applies immediately before the first OUT, checked again after the fresh mode query. The 250 ms margin avoids predictably expiring midway through an admitted frame near the refresh target.
- That 500 ms age is an admission check only. Once admitted, it never cuts a partially written batch off solely because observation age crosses 500 ms. Normal cooperative cancellation and the caller's batch deadline remain checked before every packet. An overslow mode GET can therefore reject with zero OUTs. An overslow full postflight can fail after seven accepted OUTs, preserving those counts.
- Failed/mismatched checks invalidate the observation and use the existing device fault latch. There is no retry or automatic repaint of an older saved program. No fresh-check result is inferred from undeclared bytes or cached values.

The observation is not an ownership lease. Known-process/idle/policy gating stays fresh before eligible attempts. A mode change is checked every batch, but changes to CustomFrame between periodic checks may go unobserved and a normal commit reasserts CustomFrame 8. Even the previous four-GET policy could miss concurrent writes between queries. No mode SET, ownership acquisition or exclusion of WDL/third-party writers is claimed.

Suppression invalidates prepared/control state while retaining the playback index. LIVE resume waits for the next full-checkpoint frame; it neither resets phase nor brightens onto an unvalidated pending buffer. Existing Static behavior is unchanged. Raw-bootstrap brightness provenance continues to preserve observed raw bytes until explicit brightness edits.

## Diagnostics and next measurement

`uniform_host.control_observation` exposes refresh target, maximum first-packet age, actual last observation age, start-to-completion span, completed checkpoints and fresh-mode scope. The span includes the intervening frame, not just GET syscall time. A cached observation never makes `custom_frame_control_verified` true on an ordinary frame. Its `mode_before` is freshly read; `mode_after` and unknown parameters remain null where no corresponding query occurred.

`last_mode_get_us` and `last_seven_out_calls_us` come from readonly timing fields on the same core transfer invocation; failed OUT call time is included. They exclude other control queries/packing. `last_submit_us` and gate time remain separate. These durations do not claim optical verification.

`worker_timing` contains bounded atomic counters for ticks, lock skips, total/last/max tick duration, park calls, and distinct requested/actual park totals and last/max measurements. Early unpark legitimately makes actual wait shorter. Snapshot fields are independent diagnostic atomics, not a simultaneous trace. No per-frame history or unbounded sample allocation exists. Use these actual measurements before considering a new timer or fairness delay.

## Tests

Four pure/spy stream tests cover 250 ms refresh equality, 500 ms first-OUT equality, 501 ms zero-OUT refusal after fresh mode, no mid-batch TTL, overslow initial/postflight checks, wrong scope, cancellation, changed controls and partial counts. Controller coverage verifies full checks on resume/owner re-entry/program changes/reconnect, cached versus fresh status, source phase retention, no old-program retry and raw brightness preservation. Core independent packet/ordering tests remain unchanged; readonly timing assertions cover no-I/O and zero-OUT cases. Worker tests check diagnostic aggregation plus existing panic/poison/stop/timeout behavior.

Validation: core frame 9 passed; tray 134 passed, 6 ignored. No hardware or stock-process operations were performed for this checkpoint. Repeat the reviewed bounded physical probe later to measure achieved window fps, frame counts, checkpoint costs, park behavior and unchanged readable registers.
