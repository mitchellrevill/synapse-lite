# Static region editing

Select a Static layer and LEDs, enter a six-digit RGB hex colour, then choose
**Apply to Selection**. **Erase Selection** removes those LED memberships.
Both operations support Undo/Redo and Save/Discard/Cancel. Mixed and unassigned
selections differ; `000000` assigns black rather than erasing. These are local
file edits, with no lighting output. The RGB field/swatch is a temporary control,
not the stock colour picker or a claim of pixel parity.

## Preservation

The edit partitions selected LEDs out of existing regions, retains their other
parameters, changes colour in selected partitions, prunes empty containers and
merges compatible regions. Membership uses placement id and logical row/column,
validated against explicit `leds`/`ledRef` through the shared inventory reader.
Both forms must agree if supplied together; current `leds` metadata wins.
Existing raw LED metadata is retained; new assignments copy that inventory and
available root deviceId/instanceId/
productId. Missing identities are not invented. Unselected/offline memberships,
unknown fields and numeric spellings survive. New defaults set only colour.

Merging requires identical full non-device region metadata and, for memberships,
identical non-LED device metadata. This is stricter than merging by colour alone
so unknown data cannot disappear. Selected overlaps, conflicting Static IDs,
invalid colours, ambiguous inventories and excessive partitions refuse atomically.
The core also supports generic erase; its native button currently appears only
for Static. Other archive profiles and envelope fields retain their existing
raw-preservation contract.

Source: cached Studio main SHA256
`109b483f129044e1c7c17b9fcae0e30fa775885667976807cf2c94bd1c696d19`,
module9286 around303180–305900 for partition/merge and302190 for erase;
module2904 around57805 for Static packed-RGB default65280. Offsets are normalized
characters, static evidence rather than live/native execution. See
[file](STUDIO-FILES.md) and [canvas](STUDIO-CANVAS.md) evidence boundaries.

## State, limits and verification

Region-only edits affect cached dirty state through exact borrowed-leaf
comparisons. Unknown item types remain opaque. Older save acknowledgements keep
newer edits dirty; Discard restores the exact acknowledged document.

History retains at most20 operations and8MiB of changed-leaf payload across both
directions. Large regions evict older entries earlier; a single edit exceeding
that reversible-payload budget refuses without mutation. Regional history
currently stores before/after region-array leaves. A structurally shared region
tree remains necessary if full20-step history is required for the largest
documents. The8MiB document and512 regional partition bounds remain explicit;
none of these bounds proves the complete application's100MB target.

394 core,3 CLI,1 doc and233 serial tray tests pass. New core regressions cover
partition/merge, metadata/literal preservation, missing colour versus black,
erase/no-op, atomic failures, large bounded history and exact save/undo/discard.
A600-operation differential test checks an independent two-device LED-state
reference through mixed edits and both history directions.
The native simulator test covers RGB entry, invalid/read-only attempts,
Apply/Erase/Undo/Redo and saved memberships alongside existing selection/file/
exit recovery. It submits no device SETs and does not alter device profiles.
The regional-edit fixture is synthetic. The separate
[actual stock roundtrip](STUDIO-ACTUAL.md) preserves the real composition;
committed stock re-import and equivalent lighting remain unverified.

Remaining scope includes the stock colour picker, layer creation/management,
other effect parameters and gradients, retained algorithms/compositing, complete
tools, preview/output/ownership/recovery, actual file reconciliation, pixel and
optical acceptance, and complete-build memory runs. The full goal remains active.
