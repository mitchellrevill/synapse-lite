# Local Customize category consolidation

This local integration starts at `41b6c8519a165bcf399336c83420f6d2aa693b89`.
It combines these completed units, with their original commits retained as merge
parents:

| Unit | Commit |
|---|---|
| Keyboard | `4aefd7c68610ca583de7fa68b7ce8b8186c585e7` |
| Mouse | `0c810a50e7b55b7ed35c2430ad75fd1dcb03048f` |
| Multimedia | `c196b00bcf1bc07aec38093cc03e63caa11020c6` |
| Windows Shortcuts | `e725e29bf7f3924bdc241a3e11e904684f187d36` |
| Launch | `545659f2b5986cbbdcfc6c2eb2d2c32bc2fd0a6b` |
| Default / Disable | `851b633f00cec340f7014c10d8144ad2fc98514f` |
| Sensitivity inert drafts | `7b5906a51075a246d142901bed64f405583f4a9c` |
| Profile Switch reference validation only | `459132cc2225e118b3a4a678ad87173ef01023bb` |

Profile Switch's native form, save and delivery remain unimplemented. Sensitivity
is included only as its completed inert-draft unit. Approved tabs are unchanged.

## Shared behavior

Keyboard, Mouse, Multimedia, Windows, Launch and Default/Disable use one
worker-side local commit boundary. It checks
read-only mode, the connected device and PID, active profile and complete original
profile before building a candidate. Validation and the existing bounded store
save run before the UI follows a pending close/navigation action. Errors unlock
the form and retain its draft. Save, Cancel, Undo and the dirty-close alert share
this lifecycle. Completion explicitly says hardware delivery is unavailable.

Dropdown labels, counts, selection and scroll range dispatch by category. Keyboard
retains its manual/group chooser. Mouse retains its established top-of-list reopen
behavior. Windows retains scrolling to its selected command. Launch retains its
native edit notifications and owned file-picker seam; Save never executes it.

Mouse and Default/Disable store overlays, while the other units author decoded
source records. `profile_compat::assignment_state` composes these views. Before
changing a physical input's category, it materializes that input's effective
records, consumes only its prior overlays and retains all other inputs' overlays.
Both layers are composed together so previously accepted source Hypershift pair
removal cannot be undone. Any removed paired source record is retained privately
in `retiredAssignmentRecords`. Unknown root/group leaves and profile identity
survive; malformed or ambiguous ownership, layers and structured contexts refuse.

Only the authored layers lose stale native button projections. An opposite-layer
projection remains unless an earlier source Hypershift replacement explicitly
cleared that pair. `localAssignmentInputs` prevents the legacy export projection
from deleting materialized local records. Validated Default/Disable overlays also
flow through source export; native mapping-cache rewriting still refuses. Native
backup and source-codec round trips are tested, not live stock import acceptance.

The existing category API seams use the same overlay preparation. The common
native save path uses the engine's bound store rather than an unrelated default
location. No assignment runtime, recorder, interception, injection, profile
application, device setter, driver/service action or live delivery is enabled.

Sensitivity keeps its dedicated **Save Draft** endpoint and stores no decoded
sensitivity assignment or executable projection. Its field IDs are 1610/1611,
separate from Launch's 1601-1603. Its test diagnostics use operation 7, separate
from Launch's operations 4/5 and Keyboard's render operation 6. The same successful
save lifecycle closes the draft and follows pending navigation. Changing into
Sensitivity retires only same-layer Mouse/Default overlays; changing to another
category consumes only that input/layer's Sensitivity draft. Complete retired
values remain in private history. Saving a Sensitivity draft leaves decoded source
records and executable button entries unchanged. Unknown draft schemas refuse.

## Integration checks

The final ordinary workspace run passes 680 core, 351 tray, 11 CLI and one doc
test. The previously documented
`dynamic_sensitivity::tests::presets_custom_chunk_readback_rollback_and_profile_scope`
remains explicitly excluded; this is not an unqualified complete-suite pass.
Focused composition tests cycle Mouse → Keyboard → Multimedia → Windows → Launch
→ Disable → Default and back, checking source export/import and opaque/layer
preservation. Core checks also retain retired Hypershift records and reject
malformed local metadata without changing the original candidate.

The final release workspace build also passes.

Eight serial native fixtures pass: Keyboard, Mouse, Multimedia, Windows, Launch,
the shared source Cancel/Undo/Default/Disable regression, mouse label/drawer
hit targets, and Sensitivity clutch save/reopen/filter controls. They use simulated devices and temporary stores, and assert the
applicable no-SET/no-write/read-only/conflict conditions. Fixture startup and
execution are bounded. No fixture timed out in the final run.

The first native pass exposed two integration regressions: selected-row scrolling
changed Mouse's row positions, and the shared completion text lacked the explicit
"unavailable" delivery status. Both were fixed and all affected fixtures rerun.
Earlier compile issues included a test message-number collision, a non-Copy macro
event API, a misplaced test boundary and a removed Launch notification handler;
the final compiled checks include their corrections. Evidence logs are retained
in the assignment task's outputs. See `validation/customize-category-integration.json`
for exact build hashes and command results.

Reproduce with a private asset pack in `SYNAPSE_LITE_ASSETS`, an owned TEMP/TMP and
an independent `CARGO_TARGET_DIR`. Cargo uses `-j 1`; all tests use
`--test-threads=1`. The eight opt-in native fixture names are in the validation
manifest. No normal store, user clipboard, physical input/device, other Lite
instance, canonical checkout or another feature checkout was modified.

## Remaining acceptance

Hardware readback/delivery, ownership/release/reconnect recovery and exact visual
parity remain the original category TODOs. Profile Switch is a read-only reference
API only; Sensitivity remains an inert draft with device delivery unavailable. The inherited excluded
Dynamic Sensitivity test needs independent resolution. Final combined privacy,
functionality and complete-build memory acceptance below 100 MB remain open.
This integration is local only: no push or main merge is performed.
