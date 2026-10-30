# Studio layer authoring

Open a local composition, then choose **Layer Actions**. The native editor now
offers Add Static, Add Spectrum, Add Fire, Add Breathing, Add Wave, Add Starlight, Add Reactive,
New Group, Duplicate, Move Up/Down
and Delete. Ctrl+D duplicates and Delete removes the selected item when an edit
field is not focused. Escape dismisses the menu. All changes participate in the
existing local undo, redo, save, discard and unsaved-exit workflow.

Adding an effect prepends an empty root layer and clears LED selection. Select
LEDs and assign properties afterward; adding does not silently light the previous
selection. Newly added Static/Spectrum/Fire and [fixed Breathing](STUDIO-BREATHING.md) regions work with Local Preview.
Local Preview sends no hardware frames. [Starlight settings](STUDIO-STARLIGHT.md)
can be authored and saved; Starlight preview/output remain unimplemented and
refuse explicitly. [Reactive settings](STUDIO-REACTIVE.md) can also be authored
and saved; key-triggered Reactive preview/output remain unimplemented.

New groups are empty, closed root groups and do not wrap selected layers or
change the current effect. Copies prepend to the same sibling list. A group
copy receives fresh IDs for the group and every child, with new child groupId
references. Root copies stay root and child copies stay in their parent. Default
effect titles remain identical; custom titles receive a sibling-unique numeric
suffix. Unknown imported JSON and numeric spellings remain raw. The English
display translates recognized default title keys without rewriting file data.

Deleting a group removes its children together. Deleting its last child leaves
an empty group. Public deletion refuses to remove the final effect layer, matching
the stock UI guard rather than reproducing the source core's stale-selection edge.
Move Up/Down changes adjacent sibling order; groups move with their children.
Undo reconciles selection if the selected item no longer exists. Hidden groups
do not contribute visible children to local preview.

## Contracts and bounds

The pure core exposes source-shaped constructors for all 13 retained Studio IDs;
this does not implement every effect's properties or playback. The native Add menu
exposes the three effects with current editing and local preview support. Constructor
params are empty except Wheel/Tidal's recovered center-point defaults. Regions start
empty. No fabricated regional defaults, input hooks, media ownership or device APIs
are enabled by constructing a row.

New IDs have the recovered 12 lowercase hexadecimal character shape. The native
editor uses Windows random bytes instead of the cached source's Math.random-seeded
generator. Imported IDs are preserved and are not required to have this shape.
Duplicate/new IDs are checked before mutation. Missing items, conflicting groupId
back-references, nested groups and invalid operations are refused without changing
the draft or history.

Tree edits retain reversible root layer arrays; ordinary property edits retain
their existing leaves. History remains at most 20 steps and 8 MiB, with at most
512 nodes and an 8 MiB composition. Oversized structural edits fail before mutation;
full 20-step editing of the largest admitted trees still requires a more compact
representation. This is not the complete application's RAM acceptance result.

## Evidence

Source: cached Chroma Studio `main.88ac681e.js`, SHA256
`109b483f129044e1c7c17b9fcae0e30fa775885667976807cf2c94bd1c696d19`.
Constructor/add/group/copy/delete actions are around decoded offsets 294810,
294980, 295332, 297454 and 298005. ID generation is in module6074 around171757.
REA parent `ev_4d318d05da1dda4a8d9034c2a8ef8da573575b62afb734df5ecb4d09ec54d934`
was re-inspected through summary
`ev_eea295d06680c79794a99e4a1513601666a0dd317b424c5ac587e5f2b734c83a`:
40 parsed JS files, zero parse failures, partial application coverage. Literal
source inspection supplements that evidence; it is not a current-build equivalence
or vendor runtime claim. Detailed private notes are under work/studio-layer-authoring.

The native test adds Static to the actual exported composition, selects Esc,
assigns RGB123456 and verifies that the preview frame contains that RGB above
the existing Fire/Spectrum. It duplicates, moves, deletes, adds a group, undoes
six mixed structural/region edits, redoes and saves a locally readable file.
Read-only routing blocks edits. Original export and device profiles remain unchanged,
and simulated device calls added are zero. Existing actual-export playback and
failed-save/unsaved-exit recovery regressions also pass.

See `docs/validation/studio-layer-authoring.json`. Native owned-state rendering
was inspected alongside the stock gallery. It omits child Win32 edit controls and
is not a replacement for desktop screenshot verification.

## Layer organization, 30 October 2026

Drag a layer title above or below another row to reorder it or change its parent.
Drop on the middle half of a group row to prepend the layer to that group, including
a collapsed empty group. Drop beside a root row to move a child out of its group.
Groups move with their children and stay at the root. A green line marks the drop
position; Escape, capture loss and release outside the list cancel. Scroll the list
with the wheel to reach other rows. Ctrl+E and Rename Group edit the selected name.

Expand/collapse now edits the saved `opened` property and participates in undo/redo,
alongside rename, visibility and structural moves. Group visibility keeps its existing
source behavior of overwriting child visibility in one reversible step. A drop is one
structural history step; it updates only the moved layer's `groupId` when crossing
parents (an empty string at the root). Same-parent moves retain the imported reference
spelling/absence. Conflicting back references fail before mutation; nested groups are
refused. Unknown fields and raw number spellings remain intact.

Recovered source `main.88ac681e.js` (same SHA256 above), decoded UTF-16 offset 298657,
contains the before/after/add reducer and root/group restrictions. The REA analysis
parent for this pass is `ev_8802e345246ce84cc6e9f177adf39f06d49a57eca46060468a42f242a825b1c4`;
40 parsed JavaScript files, zero parse failures, partial application/semantic coverage.
Direct literal inspection supplies the reducer details; no vendor execution or
full runtime equivalence is claimed. Persisted undoable expansion is an explicit
requested editor behavior; stock expansion does not set its save-enabled flag.

## Remaining work

- The stock add-effect palette, icons, context menus and exact positioning. Current
  Layer Actions and text Show/Hide buttons remain interim UI. Drag auto-scroll at
  the viewport edge is not implemented; wheel scrolling is available.
- Confirm the stock localized default group-name template; current English naming
  is New group N. Imported/custom group titles are preserved.
- Chassis, key legends, canvas framing/zoom persistence and the complete stock
  property widgets. The retained screenshot has substantial differences here.
- Breathing and the remaining effect algorithms, including random, finite and
  input-trigger modes. Quick-effect math alone does not certify Studio behavior.
- Complete composition creation, stock re-import of authored trees, hardware
  output, wider device layouts and the three-run complete-build 100 MB gate.

These remain implementation TODOs; the full goal stays active.
