# Editing existing macro input rows

Select a keyboard or mouse row, choose another input, then **Update Pair** or
**Update Event**. Rebinding keeps the original direction and row count. The
gesture chooser continues to control newly added inputs; it does not convert an
existing pair into a single transition. Keyboard keys update to other keyboard
keys, mouse buttons to other buttons, and complete mouse actions to other
complete actions. To change between these kinds, add the desired new events.

Selecting either member of a proved pair updates both. The delete button says
**Delete Pair** and removes both as one reversible operation. Intervening delays
and unrelated events stay in their original positions. Save/Discard and bounded
Undo/Redo use the existing local draft workflow. No recording, playback, input
interception, device discovery or device SET is enabled.

## Identity and retained data

A pair needs exactly two rows with the same safe integer ID, the same identified
input, and ordered down/up states. A standalone explicit transition or complete
mouse action can be edited individually. Known legacy keyboard rows may omit
ScanCode and category when the remaining descriptor is unambiguous; rebind adds
the explicit source metadata. Aliased Enter/modifier keys require enough source
metadata to select one key. Ambiguous identities, three or more matching rows,
different inputs sharing an ID, reversed/mismatched flags, unverified/null states,
and duplicate known row fields refuse group editing/deletion. The native editor
does not silently fall back to deleting one member of an ambiguous input group.

Rebind patches only the selected group's known input fields. It preserves raw
IDs, selected/isPairing flags, phase fields, unknown root/nested leaves, other
event rows, macro identity, playback metadata and profile references. Numeric
tokens in opaque leaves stay raw. Keyboard edits synchronize the known ScanCode
and existing lowercase scancode alias; the explicit-zero prefix rule from
MACRO-INPUT-AUTHORING.md is retained. appEngine cache guards, source hashes,
complete-store budgets and detected external-save conflicts remain enforced.
This is definition editing, not proof of safe execution or atomic ownership of
the profile file against concurrent writers.

The source keyboard-row component dispatches paired key changes through a
shared event identity and opposite transition flags. This checkpoint uses that
identity relationship but verifies the group before changing it. Existing REA
evidence is recorded in MACRO-INPUT-AUTHORING.md:
ev_368f033770d7aaf127eb74322e35ac3865eaf53a0136ff7e35db3eac13a56a2a,
parent ev_7b6e25e07a28ce08411cfcb137c2ed7ee33f1c073b954c0ed1006d0325f36587.
No new vendor execution or complete runtime equivalence is claimed.

## Verification and remaining work

Five added core checks rebind an existing pair to every125 independently
replayed source key fixture; verify raw nested/root leaves and an intervening
delay; edit legacy missing-scan rows and all five mouse buttons/complete actions;
refuse ambiguous IDs/states/duplicate fields; and preserve profile records,
active selection and source-export references. Existing input/cache/budget
checks also run.

The isolated native HWND check now changes Right Ctrl down/up to Right Shift,
changes both mouse-button rows, preserves pair IDs, deletes a pair and restores
it with Undo, saves the exact resulting rows, and refuses read-only rebinds.
It retains dirty-close, conflicting-save, import and no-device-write assertions.
Menu selection is supplied through the existing test-only message; chooser
open/cancel uses the real menu. Human menu selection, physical input and stock
acceptance are not claimed. See validation/macro-pair-editing.json.

TODO: null-state/Sequence-style row editing; incomplete/ambiguous source key
descriptors; direction/kind conversion; paired block reordering; remaining macro
event types, recording, binding and playback settings; macro-only export/cache
invalidation; native routing, ownership, physical-key arbitration and recovery.
Windows chooser/sidebar presentation still differs from stock. Exact visual
acceptance and the two previous broader native failures remain open. Approved
shared tab geometry was not revisited. Continue independent inventory work when
an exact behaviour stalls. Full device APIs, privacy and three complete-build
memory measurements remain unfinished.
