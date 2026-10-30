# Native editor responsibilities

The editor's background-job implementation lives in
`crates/secbird-tray/src/native_editor/device_worker.rs`. The main
`native_editor.rs` owns window state, input dispatch, rendering and consuming
worker replies. It imports the worker's existing request/reply types and entry
points directly; there is no forwarding layer or new runtime abstraction.

The worker module owns:

- `Job`, `Reply` and `WorkerReply`, the internal channel protocol.
- `enqueue_setting`, including the existing 16-job limit and adjacent-update
  coalescing rules. Coalescing never crosses an ordered operation, device,
  expected profile or source-effect identity.
- `worker`, which processes requests serially using the existing Engine mutex
  and delegates feature work to the existing feature modules.
- The three queue regressions, now next to the policy they exercise.

Keep the existing lock scopes when changing this code. Cached lighting status
uses a reader cloned once before job processing. Composition file work drops
the Engine lock first. Profile/device mutation remains in the same serialized
worker path as before. Error strings, reply ordering and shutdown behaviour are
part of the existing contract and were preserved in this extraction.

Bitmap allocation, format validation and native resource cleanup remain in
`native_editor/bitmap.rs`. Feature-specific views and their draft lifecycles
remain in their existing modules.

This refactor removes 435 lines of worker code and queue tests from the main
window module and expands the moved code into normal readable formatting. It
does not change algorithms, allocation policy, threads, caches, assets, device
commands, persisted formats or user-visible behaviour. It is a focused
maintainability checkpoint, not a claim that the whole repository has been
refactored.

Verification compares the moved implementation against its source: operations
and string literals are preserved after visibility/formatting normalization;
three existing Settings expression closures now have braces. The regular app
suite exercises the queue rules and local-assignment worker path. The existing
native Macro, Lighting/Studio and Linked Games checks exercise the same worker
through private windows and simulated devices. The final validation artifact
records their results and the existing read-only presentation checks.

Run the ordinary suite with:

```powershell
cargo test -p secbird-tray --release --offline -- --test-threads=1
```

Native ignored checks additionally need Windows desktop access and the private
asset directory in `SYNAPSE_LITE_ASSETS`. They do not certify physical hardware
behaviour or full stock-app parity.
