# Add devices to Studio compositions

Studio's **Add Device** control opens the packaged preset catalogue, grouped by
product with explicit edition/layout choices. Product names come from the same
hash-verified local manifests used by the projection tool. The current pack has
1,268 layouts across 177 products. The updated index adds names without changing
any projected geometry digest. It remains a partial source corpus, not support
for every Razer product or its device APIs.

Choose a preset after opening a composition. Its exact selection geometry loads
on demand and is placed to the right of the existing devices. Additions are local
until Save and participate in Undo/Redo, saved dirty-state tracking and reopen.
Cancelling the chooser makes no edit. Existing devices, layers and opaque JSON
leaves are retained as raw data; unrelated numeric tokens are not normalized.
The 32-placement, document/history and canvas geometry limits still apply.

These placements carry `editorPreset: true` and an explicit selectable LED
inventory. They have no physical device binding. The canvas and core preview
admission refuse their playback; native Apply also checks canvas admission before
queuing a device operation. Adding a preset stops local preview. Adding geometry
does not enable physical discovery, firmware commands or hardware support.

The index and selected file have bounded reads. Duplicate identities, unsafe file
names, invalid metadata, wrong file sizes/hashes and geometry identity conflicts
are refused. Only the selected preset's geometry loads; the catalogue does not
decode every product image. A complete candidate canvas is checked before the
live reversible edit is committed. Read-only/restricted editor states refuse it.

## Verification and remaining work

The deterministic checks cover placement history/save/discard, duplicate/invalid
and excessive placements, index ambiguity/path traversal/tampering, opaque numeric
preservation, offline preview refusal and preservation of canvas selection during
unrelated layer history. The isolated native check opens and cancels the actual
Windows preset menu, then exercises the shared addition handler for product100/
edition0/layout0 and product620/edition0/layout1. It verifies 108 selectable LEDs,
save/reopen, Undo/Redo, read-only refusal, unchanged original composition and
device profile files, and zero simulated device requests. Its rendered canvas
was reviewed. Menu item selection by an actual user remains unverified.

The chooser currently uses a Windows popup menu. Stock-matched presentation,
search, friendly edition/locale labels, chassis/legend artwork, device moving/
rotation/removal and preset-to-physical-device binding remain TODOs. Imported
layouts with missing or invalid geometry require reconciliation before addition.
Source-matched physical sizing and stock re-import of newly added offline
placements are not proven. Remaining effects, broader device APIs and the full
visual, input/recovery, privacy and three-run memory acceptance gates remain open.
Do not stall independent feature work on these exact-presentation gaps.
