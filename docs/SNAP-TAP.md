# Ornata Snap Tap implementation

This checkpoint adds source-compatible profile intent and a bounded digital
conflict resolver. **Live Snap Tap is not implemented yet.** No input hooks,
notification subscriptions, key injection, firmware writes, driver replacements
or user-profile activation were performed for this unit.

## Product evidence

Ornata605/USB025D takes the host mapping-engine route. Shared middleware module
42731 tags participating ordinary make/break selectors with the pair identity,
preserves existing mappings and excludes ordinary Hypershift selectors. Its
caller compiles Gaming Mode, then Snap Tap before submitting host mappings.
The helper does not consume `pair.mode`. The generic firmware methods are gated
by other product capabilities absent from605; their presence in a shared SDK is
not evidence of an Ornata firmware backend.

Source UI limits configuration to four pairs, excludes Menu/left Windows/Fn and
special aliases DKM_F6/DKM_D2, and checks duplicate keys across the entire list.
Imported pairs with omitted mode default to the UI's LAST_INPUT choice. That
default is not proof of native arbitration details or analog-mode support.

Original shared middleware SHA256:
`9c4a687e1a4868cd9da4dec40024e5ce3403fe866907887b47995f493e23a3a8`.
Original product middleware SHA256:
`9cdec67b89a1f8ef54a386fc2ce2a0e2f93f50830e9da385a7be91074ba90b18`.
UI SHA256:
`307e2b199037ed53a8fde90d76fc67a9a3766a9361fb152e7820d8bf05f19c65`.
Unwrapped module42731 factory SHA256:
`6a7001903a2960bdb7cae584c4a7c269d8e4412a642a2f316818949c2e559d25`.
Working projections normalize line endings; their byte offsets must not be
silently substituted for original-artifact offsets.

REA focused factory parent:
`ev_145eca212245ec895ca0aaca13c00f2fc6f1b7e5c21220ac9a01e32c4ee1fbb6`.
It records964 AST nodes, zero parse errors and237 semantic unknowns. Earlier
whole-bundle parent `ev_274e73cc93190aaa7b72fc9beb0659f4025988789801bca99aa2328fbbc1426a`
exhausted semantic heap/result bounds. Its unmatched trace is not feature absence.
These are static observations; no vendor runtime was executed for this unit.

## Profile and resolver behavior

`snap_tap::Settings` and `Pair` preserve unknown members and stable pair IDs.
Import validates keys against the existing source mapping table and actual
Ornata key layout, rejects duplicate keys/IDs and unsupported modes, and keeps
unsupported source objects opaque with a notice. Exact untouched source shape
survives export, including omitted mode. Editing replaces the complete pair list
so removed pairs cannot reappear. Native backup retains typed intent and source
metadata. Sparse keyboard profiles no longer acquire an empty lighting zone and
empty quickEffects object merely through import/export.

The pure resolver reconstructs intended digital last-press behavior: pressing the
second key releases the first logical winner, releasing the winner restores a
still-held partner, repeats do not steal priority, and unrelated keys pass
through. At most four independent pairs and two edges per event are needed.
Reset returns only participating logical winner releases, and clears held state.
Unknown extensions remain serializable but cannot silently prepare a runtime.
This is a tested reconstruction, not native/physical arbitration acceptance.
Edges name logical HID inputs, not native scan codes or ready-to-send packets.
The adapter must retain their existing assignments, prefixes and Hypershift
boundaries when compiling output; default A/D packets cannot replace remaps.

The reducer performs no I/O. A logical edge does not prove output delivery or
ownership; initialization does not query physical held state. The live adapter
must establish an all-up baseline, exact physical-device/profile generation,
interception ownership, accepted output tracking and completion-aware recovery.
Profile application reports the missing host runtime explicitly, including for
historical native backups that contain only opaque source Snap Tap data. It
does not try an unrelated firmware command or silently claim replay succeeded.

## Remaining live work

Connect narrowly scoped025D selectors, asynchronous notifications, confirmed
native pair arbitration,32-byte output submission and completion tracking.
Preserve unrelated selectors and outputs owned by other clients. Cancel and drain
in-flight I/O before releasing buffers; rollback only owned selectors. The
observed filter close path leaves device-scoped selectors behind, so handle close
is not recovery. Disable/profile switch/exit/unplug/reconnect/sleep and crash
handling need physical validation, including keys held during transitions.
See [driver contract](DRIVER-CONTROL.md) and
[keyboard packet planning](KEYBOARD-SYMBOLIC-BRIDGE.md).

The source-style pair editor, Fn+left Shift shortcut, Gaming Mode shared
interactions and actual activation remain unfinished. The running a29eb3d preview
has no new Snap Tap control from this backend checkpoint. This does not satisfy
the full device/input workflow, profile, Studio, privacy or final memory gates.

## Validation

339 core tests passed, including seven Snap Tap tests and one profile regression.
All65536 eight-event traces of a pair preserve balanced logical edges and at most
one winner. Additional cases cover partner restoration, losing-key release,
multiple pairs, reset, unrelated input, repeats, excluded/unknown/duplicate keys,
invalid modes/IDs, disabled settings, unknown extensions, source/native round trip,
pair deletion and unsupported-data preservation. Simulated profile application
reports the missing runtime with zero transport commands, including historical
source-only backups whose saved bytes remain unchanged.

229 tray tests passed with one test thread. The initial parallel run encountered
an unrelated process-wide GDI-object count assertion in the existing dot-cache
test; the serial suite passed. This is not a new rendering or tab change.
The optimized CLI/tray build passed. No physical input acceptance or complete-build
memory acceptance is claimed.
