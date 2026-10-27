# Cached Lighting status

An open Ornata Lighting view observes cached engine runtime status at most once per second. It uses the existing serialized worker, permits one outstanding observation, skips a busy engine, and pauses while hidden, minimized, closing, or user work is queued. An observation calls only `Engine::lighting_runtime_status`; it does not refresh hardware, parse the profile store, sample processes, or dispatch commands.

Replies update runtime feedback only. Device keys and both available profile bindings must still match the selected owner. A different cached profile produces a refresh-required notice; it does not replace the selected profile or discard local edits. Stale nested Static results are excluded. Manual Refresh remains necessary to obtain a new device/intent snapshot after ownership changes.

Background failures use the acknowledged native error dialog, an explicit port adaptation rather than a recovered Synapse toast. A bounded per-window history suppresses repeated faults independently of foreground-operation acknowledgements. Dialog presentation waits for lighting capture, focused editing, and profile modals to end. The processed dialog message repeats that check, covering a gesture started after the notification was queued. Closing can acknowledge an outstanding failure without waiting indefinitely for edit focus to leave.

Validation covers cadence/coalescing, owner mismatch, independent fault deduplication, cached reads against a simulated transport and an invalid store file, and retained dirty intent, color drafts, and opaque custom slots. The opt-in native Lighting test also sends real HWND messages during brightness capture and focused close. These tests provide no physical-device or pixel-equivalence proof.

The current validation run passed 128 regular tray tests (6 opt-in tests ignored after adding the physical uniform probe) and all 4 simulated native interaction tests in 4.07 seconds. Sanitized results: [lighting-status-native.json](validation/lighting-status-native.json).
