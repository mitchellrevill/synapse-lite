# Studio layer draft model

`studio::document::Draft` now implements local layer-title and visibility edits,
separate saved/working content, discard and20 undo/redo steps. It has no device
access, file I/O or preview engine. The native Studio page now uses this model
for local file editing; canvas editing and effect playback remain unfinished.

## Implemented local behavior

- Open one raw composition from a Studio archive and enumerate root layers,
  one-level groups and their children by stable IDs. Empty layer lists are valid.
- Rename a layer/group using a nonblank title of at most32 UTF16 code units,
  matching the source editor's default length convention. Imported IDs need not
  be generated12-character IDs. Existing titles are retained when opened.
- Change visibility. Setting a group also sets every child's visibility to the
  new value, as in the source. One group operation is one history step, and Undo
  restores each child's original state. Missing visibility remains distinct from
  false and is removed again when undo restores its original absence.
- Undo/redo restores only the changed leaves. New edits drop redo. History caps
  at20 edits; saved and current documents share immutable storage until changed.
- Dirty tracking compares the currently editable layer metadata to its saved
  state. Returning manually or through Undo to saved values clears dirty, despite
  ancestor JSON whitespace changing. This projection must be extended before
  adding edit families beyond titles/visibility.
- `checkpoint` captures the exact document offered to persistence. After a
  successful write, `accept_saved(checkpoint)` establishes that snapshot as the
  local baseline. Newer working edits remain dirty; checkpoints from another
  draft session are refused. Preparing bytes alone is not a successful save.
  `discard` restores that exact
  saved document and clears both history directions. The model never silently
  saves on destruction or touches a vendor store.

`Archive::replace_profile_bytes` prepares a complete output container with one
replacement. The original archive, other raw profiles and unknown envelope data
remain intact. Preparation performs no file write and does not accept a draft.
It inherits the8MiB output limit and refuses invalid indices/non-object data.
The caller still owns actual file persistence, external-file-change detection,
unsaved prompts and profile-selection lifecycle.
After a successful write, the caller must also replace its archive baseline with
the prepared container, so later saves of other compositions retain earlier
saved changes. The draft checkpoint does not perform that archive/store commit.

## Preservation and admission

Only explicit `title`/`visible` values change. Device placements, regions, LED
identity metadata, effect-wide and regional parameters, unknown fields and large
numeric literals remain raw JSON. Modified ancestor objects/arrays are serialized
again; whitespace and escaping of their keys may differ. Ordinary numeric trees
or whole-document history snapshots are not used.

Editor admission is stricter than archive preservation: required root id/title,
devices/layers shape, layer id/title/type, unique nonempty layer IDs and boolean
visibility (when supplied) are checked. Nested groups and ambiguous/excessive
objects cannot be edited; the archive codec still preserves their original data.
Unknown item types can retain display metadata, but group visibility refuses an
unknown child atomically rather than guessing its behavior. These checks do not
certify any device, layout, LED assignment or effect runtime.

The model allows at most512 layer/group entries, layer IDs up to512 bytes, text values
up to4096 bytes on open and bounded object/array decoding. The parent8MiB/depth/
structural budgets still apply. History stores changed title/boolean leaves,
not20 copies of a potentially8MiB composition. These are design bounds, not a
measurement proving complete-build RAM acceptance.

## Source and validation

Cached Studio main SHA256
`109b483f129044e1c7c17b9fcae0e30fa775885667976807cf2c94bd1c696d19`:
module9286 around294278–305900 defines layer/group operations; visibility295800
overwrites child states. Saved profile versus working effectLayer state is at
136300–140900. History module5303 around157300 caps past at20;159300 compares
working devices/layers to saved state. Discard137700 reloads saved content and
resets history. Offsets are normalized character locations, not proof of live
runtime execution. See [file evidence](STUDIO-FILES.md) for artifact/REA limits.

The source's full Save path also normalizes effect parameters and device ledRef,
handles generated-media histories, remote persistence and preview restoration.
Those operations are not implemented by local `accept_saved`. Cross-save history
behavior here is the explicit local API contract, not a claim of full native
save parity. The native view keeps selection and expanded-group overrides outside
document history; it reads the source `opened` flag without rewriting it.

Nine new draft tests cover group cascade/recovery, rename/dirty/save/discard,
history limits, failed edits, empty/missing fields, archive replacement preserving
other profiles, and unsupported/nested data remaining intact. A1000-operation
differential test compares compact command history to a full-snapshot reference,
including redo invalidation and returning to saved values. Fixtures are source-
shaped synthetic data, not actual stock compositions. A separate asynchronous
save-acknowledgement regression verifies that newer edits remain dirty, foreign
checkpoints are inert and discard returns to the snapshot actually acknowledged.

382 core tests (six intentionally ignored), three CLI tests, one doc test and230
serial tray tests pass. An additional opt-in native Studio interaction test uses
an actual window/message loop and simulated device transport. See
[native Studio](STUDIO-NATIVE.md) for its scope and remaining gaps.

## Native layout source

Recovered cached EditorCanvas JS SHA256
`190a2ef92615861142a1585b6cbffa578d2a716069f8d430cb6bd63ca103e5c6`
and main CSS SHA256
`15c13290984d2223e77eda01f08b4a2e53a47d9eff6410c3eeaca4aea61cf500`
plus EditorCanvas CSS SHA256
`9163eac79b917bdbf3814ae84e0765515f21076ba5d6bf43fe4009fa19e83e9d`
establish a48px renderer header,250px left panel
(resizable to450px),250px right panel,40px toolbar/rows and25px child indentation.
Preview/zoom controls float lower-right; there is no reserved bottom footer.
Grid/axes render only in Move mode, with40px/480px cells and the fine grid hidden
at zoom<=0.5. Save is a right-panel bottom action; Discard belongs to the unsaved
modal rather than a permanent peer button. These are source layout facts to use,
not a rendered-UI verification.

The native view must still connect imports, profile/layer selection, these
commands, persistence and unsaved guards. Add/delete/duplicate/reorder/group
editing, LED selection/painting, gradients, effect properties, device placement,
canvas tools, preview/runtime and all physical/visual/complete-build memory
acceptance remain unfinished. Stock composition capture/import is still needed.
