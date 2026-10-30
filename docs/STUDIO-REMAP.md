# Studio explicit layout remapping

The native Studio REMAP action selects one placement in the open composition, then an explicitly chosen placement in another `.ChromaEffects` file. The target file supplies its declared product/edition/layout and semantic LED inventory; no product-name heuristics or hardware discovery select a target. This is an offline import reconciliation step, available after opening an imported composition.

A preview reports matched assignment count and missing semantic identities. Cancel leaves the draft unchanged. Any missing LED blocks commit: there is no all-target-LED fallback. Successful commit is one reversible history entry; normal Save, archive export/reopen, Undo, Redo and Discard apply. Existing opaque placement/region/LED/profile values remain raw, including large numbers and original numeric spelling. Replaced placement inventories and their opaque LED metadata are also retained verbatim in the profile-level `synapseLiteLayoutRemaps` provenance array. Repeated remaps append bounded records; a conflicting non-array provenance field refuses without mutation. Only declared identity/layout/extent/inventory and matched LED identity fields are replaced. Placement position/orientation stay unchanged.

Duplicate placement IDs, colliding target IDs, duplicate semantic target identities, duplicate semantic source identities within an assignment, category mismatches, missing semantic metadata, invalid target declarations and stale previews refuse. Coordinate-only preset packs cannot supply semantic remapping and refuse honestly. Target choice does not authenticate hardware or authorize output.

## Source basis and intentional differences

Recovered `ModalImport.b61742fd.chunk.js`, SHA256 `d6d00e7c8dfa505cea636c564cc9f5a6938494022bc3adca784e41383e590b4d`, source offsets 1850–4370 (see STUDIO-FILES.md and inventory chroma-inventory.md S3), matches keyboard/system `inputType` + `inputData`, other devices `regionId` + `regionIndex`. Its no-match fallback broadens to all target LEDs. Lite deliberately blocks missing matches instead, retains imported composition IDs, and requires explicit placement selection instead of product-name guessing. See STUDIO-FILES.md for retained REA evidence IDs and coverage limits; this work reuses that evidence and directly inspects the hash-matched recovered source rather than repeating native analysis.

The native file/menu/message-box presentation is interim. No stock modal visual parity is claimed. Target choices currently require an exported composition containing semantic LED metadata; catalogue preset acquisition and the composition library remain separate workstreams. Legacy ZIP/XML conversion and hardware delivery remain unverified. This change does not establish complete-build memory acceptance.
## Validation

- Serial Studio core regression suite:100 passed before the provenance amendment; five final focused remapping tests pass, including exact raw inventory provenance and non-keyboard/identity collision cases.
- Native application suite:322 passed,58 opt-in checks ignored. No physical opt-in tests ran.
- Owned native remap workflow: native source menu cancellation, preview-without-mutation and explicit commit through a fixture hook using the production Draft API, asynchronous Save, Undo/Redo, source-file preservation and zero simulated device requests pass. The target-file picker and OK confirmation still need a manual end-to-end check; the automated commit uses the fixture hook.
- Workspace release builds with `cargo build --workspace --release -j 1`, in this task's separate target directory. Existing unrelated warnings remain.
- Screenshots are1600×900, same owned window geometry before/after. Inspected after-frame confirms REMAP is visible and geometry diagnostics are honest. The synthetic placement has no artwork; source fonts/assets are absent in this fixture. No matched stock modal comparison or pixel parity is claimed. Next visual check: provide the normal asset pack, choose a semantic stock export target, and capture the actual preview dialog at matched stock DPI/geometry.

The canonical checkout and other tasks' checkouts were never edited. No input capture/injection, device writes, stock process changes, or user clipboard access occurred.

Final native rerun after the provenance amendment also passed (one explicit ignored workflow,2.54s); final release build passed. Screenshots in the task outputs were refreshed from that final owned window.
