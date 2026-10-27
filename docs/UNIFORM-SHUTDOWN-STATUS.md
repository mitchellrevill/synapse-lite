# Uniform shutdown and status publication checkpoint

This checkpoint repairs two paths exposed by a failed active-lighting RAM probe: immediate stop cancellation could interrupt a frame, and one-shot 1Hz Engine try-lock observations could starve. The failed physical run retained 87 accepted frames and a 9,004ms-old pre-stop sample, followed by an unspecific post-stop error. Its private log lacked the exact fault; therefore its actual failure is not retrospectively identified. No RAM acceptance follows from that run.

## Graceful worker stop

A stop request now closes local batch admission atomically and wakes the worker. An already-admitted batch may finish its seven OUTs and full postflight, using the existing 500ms cooperative batch deadline. No new batch may be admitted after quiesce. Replacing the worker signal starts a fresh playback generation, discarding any unsubmitted pending ticket while retaining device-bound faults. No phase continuity across replacement actors is claimed. The CAS/RAII handshake is only local scheduling; it is not a device ownership lease, optical proof or restore token.

`Worker.stop` joins outside Engine for up to its supplied interval (the tray uses 2s). Only timeout requests immediate cooperative cancellation. It cannot interrupt a blocked HID syscall. Timeout does not acquire Engine for cleanup and does not claim clean stop. Panic/poison exits also remain failures; a second stop call cannot erase an unexpected exit. The join interval is not a global process-shutdown deadline: unrelated native jobs could hold Engine separately.

A successful admitted frame during quiesce does not initiate a new LIVE brightness resume. Frame errors retain actual accepted/attempted counts and the existing device-bound fault latch. Direct cancellation or timeout can still stop between packets and leave a partial frame. No write retry or old-program repaint is introduced.

Bounded diagnostics retain local admission/completion/abandonment counts, in-flight state, quiesce/cancel flags, timeouts and the latest known packet outcome. Private local shutdown logs capture actual cached uniform errors, brightness-policy errors and signal metrics before session cleanup. Public RAM metadata remains sanitized; generic categories do not replace that private diagnostic detail.

## Presentation cache

The Engine publishes its already-cached status from an existing worker-owned Engine guard. Healthy publication is at most 1Hz, with fault transitions and final ticks published immediately. It adds no GET, Store parse, polling process or new Engine-lock acquisition. Observation time is captured before status JSON construction.

Native/App readers copy the requested device and producer timestamp through a separate bounded cache. The small lock only copies an Arc and timestamp; JSON copying and live-signal sampling happen outside it. Readers never wait for Engine or restamp consumption as a fresh observation. One latest value is retained, with no frame/status history.

Each publication binds its producing signal/generation. A replacement actor invalidates the old publication and must publish its own value; a new live signal cannot revive an old healthy snapshot. A retained small signal tombstone owns no thread. It lets readers detect stop/panic even if Engine is poisoned or its session cache was cleared. An unexpected exit supplies a generic liveness error only when no actual cached error already exists. Genuine partial counts/errors remain intact. Thread-alive, quiesce and accepting-work state are distinct; quiesce is not proof the thread has joined.

## Readiness meaning

Presentation/RAM readiness means a native view received a recent (at most2,500ms old) healthy producer observation, with a valid control checkpoint at that observation and enough accepted frames. It is not a current 500ms hardware-admission lease. Reports expose control age at the producer, snapshot age, conservative control age now and separately sampled live-worker state. `current_controls_verified` remains false without a new query. Unknown mode parameters stay unknown. Fresh post-join Engine errors remain authoritative before cache cleanup.

The worker's real hardware checks, 250ms control-refresh target, 500ms first-OUT admission maximum, exact frame packets and per-packet deadline/cancellation remain unchanged. The 8ms park minimum and timer implementation are also unchanged. This checkpoint supplies no new hardware-rate or memory result. Intrinsic read-only smoke enforcement remains a separate pending unit.

## Verification

Simulator tests exercise stop while a real fake-transport row call is blocked: normal quiesce completes exactly 7/7 once; genuine row failure remains 2 accepted/3 attempted. Explicit immediate cancellation reproduces a 3/3 partial-cancel fault, supporting a possible failure mechanism without identifying the old physical error. A blocked call keeps Engine locked across the actual 2s stop budget; stop returns without acquiring it, and release subsequently preserves the 3/3 fault.

Additional tests cover CAS admission races, no re-entry after quiesce, panic/poison/timeout, publication throttling/error transitions, device binding/producer timestamp, held-Engine cache reads, actual worker panic after a healthy publication, retained partial errors and replacement-actor invalidation. Tray suite:155 passed, 6 ignored. Tests use synthetic stores/spy transports only; no physical calls, stock-process changes or release rebuild were performed.
