# Uniform control sampling checkpoint 1

The 9b22bba baseline physically achieved 10.84 subsequent Spectrum fps and 10.89 Breathing fps, with the original worker policy. The 3a3a45b control-sampling checkpoint reached14.27 and14.56fps respectively in a bounded follow-up. Core frame bytes, fresh mode admission, no-ACK OUT behavior, cancellation/deadline and no-retry semantics remain intact. The worker still requests waits of at least8ms; no timer or resolution setting changed. Neither checkpoint meets the nominal25fps target.

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

Validation: core226 plus doc1 passed; tray134 passed,6 ignored. The implementation agent performed no hardware operations. The root then ran the same reviewed ignored physical test with a freshly compiled executable, explicitly pinned by SHA256 rather than inferred from a directory timestamp.

## Bounded physical result

[Sanitized report](validation/uniform-control-checkpoint-physical.json), executable SHA256 `325CE14C99313A1596B23405D33BEC58711D7EEDA12183E02EFFEC4D65D33DD3`: Spectrum accepted30frames including the initial action over2.0326seconds (14.267 subsequentfps); fixed one-color Breathing accepted74 over5.0120seconds (14.565 subsequentfps). No fault was reported, every observed final batch accepted7/7, and the worker joined without shutdown errors. Readable mode, both effect controls and saved/live brightness matched the baseline. Stock user processes went15→0→15; services/drivers were unchanged. No tray/native window was involved, so this is not release RAM acceptance, optical output verification or input-recovery evidence.

The final sampled full checkpoints took75.08/74.18ms; their fresh mode GETs took about12ms and seven OUT calls about28ms. Final requested8ms parks actually lasted16.18/16.06ms. Counters span the worker lifetime, so Breathing's totals include the preceding Spectrum phase; they are not independent phase distributions. Requested and actual wait totals differ because wakeups may return early and Windows may oversleep. These measurements identify costs, not a certified cadence or proof that timer changes alone can attain25fps.

An accidental earlier rerun selected the old baseline executable (`35824F...596FB`) and was excluded from checkpoint performance results. Its hardware probe completed and stock processes restarted. The measurement wrapper now requires an explicit executable path and matching hash.

`tools/Test-UniformWorker.ps1` publishes the portable equivalent: explicit `-WriteUniformFrames`, `-TestExecutable` and `-ExpectedSha256`, a fresh native config, exact ignored-test filter, bounded stock-user isolation,60second child timeout and finally stock-engine restart. It runs2second Spectrum and5second fixed Breathing and never stops services/drivers. The portable adaptation was syntax-checked; the executed wrapper used the recorded local paths. This test requires existing Driver3/Custom8 and positive brightness and does not acquire that mode or restore an unknown optical phase.

## Prepared physical-frame boundary

The guarded submission implementation now also accepts an explicit `PreparedOrnataFrame`: `io::submit_prepared` for a full checkpoint and `StreamState::submit_frame` for the existing sampled-control stream. Uniform callers still construct exactly the same repeated-color frame and delegate to these helpers. Fresh mode GETs, CustomFrame checks, deadlines, per-packet cancellation, partial counts, postflight checks and invalidation retain their previous ordering. The new entry points choose no layout, authenticate no source artifact and create no profile or ownership permission.

Two additional simulator regressions submit independently colored physical cells and verify every delivered RGB triple. They cover full-checkpoint partial failure without retry and two streamed frames with distinct colors, cached controls and a fresh mode read per batch. All 171 ordinary tray tests passed, with eight opt-in tests ignored. No new physical submission, runtime Wave path, cadence or RAM claim accompanies this extraction. The earlier physical results above remain tied to their original executable.
