# Intrinsic read-only runtime smoke checkpoint

At this checkpoint, `--runtime-smoke <1..600 seconds>` is intrinsically query-only for its entire process lifetime, including when an existing native Store contains DPI, polling, remaps, power settings, lighting policies or an opted animation. It is separate from normal runtime and the explicitly active `--runtime-lighting-probe` workload. Mixed entrypoint flags are rejected rather than selecting an interactive server or active path.

## Application policy

The read-only policy is installed before discovery. It starts neither host brightness policies nor the uniform worker and skips generic profile replay, foreground switching, reapply/retry/remap timers, firmware/update actions and mutating tray commands. GET discovery, snapshots, input listeners and native presentation remain available. Events cannot persist DPI/profile changes. Shutdown does not initiate lighting restoration for a runtime that never acquired a lease.

Engine guards independently refuse replay/policy/mapping paths and profile switches. Dispatch rejects mutation requests before body parsing, Store changes or hardware access; direct import wrappers also refuse. Native presentation refuses action/import/export-file/button/rescan jobs and hardware gestures, while permitting GET refresh, cached observations and a pure export-preview needed to render an existing smoke modal. Local presentation overrides remain local.

## Device capability and send-path audit

Every Engine handle is irreversibly marked query-only, including handles obtained by later discovery. Normal and active-probe handles retain their default write capability. The Device transport is private.

| Device send path | Enforcement |
|---|---|
| `transact`, `transact_slow`, `transact_as` | Refuse every protocol command whose ID has the GET bit clear, before transport |
| LED operations and public `get` | Route through the guarded transactions |
| Scoped mode/effect readback exceptions | Explicit permission check on fixed GET 00:84 / GET 0F:82; existing response validation unchanged |
| Private raw Ornata row/commit sender | Refuses all frame OUTs on a query-only handle before validation/transport |
| Initial handle discovery | Harmless serial GET before the Device exists; receiver identification uses GET-only queries |

Protocol GETs still use USB feature-report OUT query packets followed by reads. Zero protocol SETs does not mean zero USB OUT traffic, nor does it assert that a vendor GET has no incidental firmware behavior. This is an application device/configuration mutation boundary, not a filesystem sandbox: local logs and measurement evidence are still written. It neither stops stock processes nor installs/replaces drivers.

## Verification and evidence scope

An exhaustive spy test rejects IDs 0..127 across six classes through all three transaction entrypoints, plus every canonical row/commit packet, before transport. Ordinary handles still write and query-only handles still read. A whole-lifecycle keyboard+mouse test uses a populated private Store and exercises replay, foreground, away/display, frame/battery/laptop/remap callbacks, profile/DPI events, mutation routes, imports, direct setters, worker startup and shutdown. It observes zero protocol SETs and byte-for-byte unchanged Store data. Source-policy assertions and native interaction tests cover the presentation boundary. No hardware or release build was used for this checkpoint.

GET state reports actual Engine restriction and handle capabilities. Presented-frame evidence reports intrinsic enforcement only when those capabilities and the actual read-only smoke context agree. `Measure-Runtime.ps1` scopes new runs according to that reported evidence. Earlier reports remain tied to their original executable hashes and fresh no-action configurations; this checkpoint does not retrofit intrinsic protection or new memory results onto them. This workload excludes active rendering and full device workflows, so it does not establish final feature, optical or aggregate product RAM acceptance.

The independent indexed-tree checkpoint passed1 CLI test,253 core tests (5 ignored),158 tray tests (6 ignored), and1 doc test. All four simulated native HWND regressions passed; the overlay test initially required local source assets at its own checkpoint path and passed after those assets were provisioned. The measurement script passed PowerShell syntax parsing. [Verification](validation/read-only-runtime-smoke-tests.json) records this isolated scope, excluding the concurrent native-storage/transaction changes. No new physical or RAM result is claimed.
