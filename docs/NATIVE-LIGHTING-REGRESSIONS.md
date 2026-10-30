# Optimized native Lighting regression investigation

This checkpoint resolves the two failures retained in MACRO-LIBRARY.md. Both
are native test infrastructure defects; no production Lighting behavior or
approved shared tab layout changes. Initial diagnosis and repeated checks use
committed main `3d75ea7`; the feature is subsequently rebased onto `e6c21c3` to
preserve the completed nested-macro work. Results for both bases are retained.
Native checks use owned HWNDs, an isolated synthetic profile store and simulated
transport. They do not discover physical devices or change normal profiles.

## Focused close: finish Save before measuring Close

The unchanged optimized binary reproduces the reported combined
"Focused close persisted draft or dispatched hardware" assertion. Instrumented
runs capture before/after profile documents and the transport tail. In the
captured failure the profile documents have no changed values and the UI still
reports `pending: 1`, `status: Working…` immediately before close. The tail is:

| Command | Meaning |
| --- | --- |
| `00:84` | GET device mode |
| `05:84` | GET active profile slot |
| `0F:02` | SET matrix effect |
| `0F:82` | GET matrix effect/readback |

Saving a custom slot queues `/api/source-custom-colors`, then
`/api/source-effect`. The old test waits only until the first operation's slot
is visible on disk. Its snapshot can therefore precede completion of the
already accepted effect operation. Close correctly drains that work; counting
it as an operation dispatched by Close is a harness race. The generic keyboard
simulator refuses the region-zero effect readback with NotSupported, so the
effect can additionally produce acknowledged failure feedback. This checkpoint
does not relax that refusal or claim successful physical effect application.

The test now waits for zero active and queued operations and completed failure
acknowledgement before reopening the colour editor and recording the close
baseline. Posting a dialog acknowledgement alone is not treated as completion.
It enters a distinct unsaved `fedcba` draft over the saved `#a1b2c3` slot, focuses
the real HEX child and injects a deferred background failure. Close must destroy
both editor and owned failure dialog, preserve the exact store bytes, and add
zero transport commands (GET or SET). Existing opaque-slot, other-profile,
brightness release/cancel and failure-deduplication assertions remain.

## Wave: a message collision consumed the geometry query

The first dropdown query returns JSON null. The missing rectangle is the
initial `trigger`, specifically its `x` coordinate; the layout routine was
never called. `LIGHTING_DROPDOWN_TEST` and `mouse_feedback::NOTIFY` both used
`WM_APP + 109`. The earlier mouse-feedback handler consumes the query and
returns before the test handler can fill its output parameter. Retrying the
unchanged ID still times out, which rules out a transient borrow as the cause
of this missing initial geometry.

The dropdown test now uses `WM_APP + 110`. An ordinary, non-ignored unit check
requires distinct IDs for all current private messages routed through this
editor HWND, including module notifications and test ports. It would reject
the original collision. Test geometry access reports the named rectangle and
field on failure, and unavailable snapshots are bounded retries rather than
implicitly valid geometry.

The original HWND workflow still requires selected-row scroll 122, thumb drag
to zero, close/reopen restoration, actual Wave selection, direction requests
`[2, 1, 2]`, retained opaque setting data, and upward opening after narrow-view
scrolling. It still requires byte-identical store contents and zero transport
commands while using the test-only effect-command sink.

## Input delivery and evidence

The initial corrected optimized suite passes all 25 native interaction checks, including
the source-font/overlay and all 12 Studio checks. No original check is removed,
ignored anew, or weakened.

Each corrected check also passes six separate optimized runs. The ordinary
workspace passes 503 core, 307 tray, 4 CLI and 1 documentation checks with
`--test-threads=1` (6 core and 47 tray checks remain opt-in). An earlier parallel
workspace run failed the unchanged dot-background resource test because its
process-wide GDI counts changed from 5 to 7 while other tests ran. The assertion
is retained and passes serially; its concurrency isolation is a separate TODO.
The same-assets unchanged baseline passes22/25: both target checks and an
unchanged Reactive layer-add check fail in that run. The full corrected25/25
run and these failed runs are retained separately.

After rebasing onto `e6c21c3`, both target checks, the font/overlay checks and all
12 Studio checks pass again. That complete native run is24/25: the unmodified
Advanced check times out at "Sensitivity mode". The same timeout occurred in
the earlier incomplete-setup run. Its cause remains unresolved; it is not
silently retried or classified as a product regression. TODO: capture Advanced
mode, pending/queued state and simulated transport around each mode click,
then compare its cross-thread input with acknowledged UI-thread delivery while
retaining every mode expectation. This checkpoint does not claim a wholly
passing native suite on the final integration base.

The final integration workspace run passes509 core,307 tray,4 CLI and1
documentation checks serially (821 total; existing6 core/47 tray opt-in checks
remain ignored). The message-ID invariant is included in the307 tray checks.

Cross-thread sent test messages may reenter Win32 calls while the UI's RefCell
is borrowed. The two affected tests now dispatch their input through a test-only
UI-thread port that refuses a whole request while borrowed, with a bounded retry
before any input is delivered. Click down/up/paint is one accepted batch. These
ports are compiled only for tests. The other native tests' click helper remains
unchanged. This avoids hiding actual dropped input behind timing sleeps.

Raw failed logs and synthetic diagnostic documents are retained in this chat's
private work directory; sanitized command traces, source/binary/asset hashes,
results and limitations are in
[validation/native-lighting-regressions.json](validation/native-lighting-regressions.json).
The original macro-library validation file remains historical evidence.

Baseline and fixed runs use the same private source fonts/LUTs and 120-DPI
(scale 1.25) desktop. Font, overlay and rendering assertions are retained.
An initial broad run lacked the explicit stock Studio fixture and is recorded
as a setup failure, not silently counted as a passing suite.

Reproduction, with private assets and the stock fixture provisioned explicitly:

```powershell
$env:SYNAPSE_LITE_ASSETS = '<checkout>/target/native-assets'
$env:SYNAPSE_LITE_STOCK_STUDIO_FILE = '<private>/stock-2026-10-28.ChromaEffects'
cargo test -p secbird-tray --release --locked --offline native_editor::interaction_tests:: -- --ignored --test-threads=1 --nocapture
cargo test -p secbird-tray --release --locked --offline editor_private_messages_have_distinct_ids
```

Physical optical output, full device coverage, source pixel parity, and the
three complete-build memory acceptance runs remain outside this checkpoint.
