# Studio Reactive authoring

Layer Actions now includes Add Reactive. Select LEDs, choose Slow/Normal/Fast,
enter an RGB colour or None, and turn Random Colour on/off. Changes use the
existing region partition, Undo/Redo, local Save and Discard workflow. Random
Colour preserves the saved colour and disables its editing. Changing a property
does not change unrelated properties, unselected memberships or device profiles.

Reactive key-triggered preview and saved-device output are not implemented.
Active Reactive compositions continue to refuse those routes explicitly. No
input observer, interception, generated input or device write starts by editing.
This checkpoint implements source-compatible settings, not live Reactive lighting.

The later [event and pulse replay boundary](STUDIO-REACTIVE-REPLAY.md) adds exact
metadata mapping, a sample-driven diagnostic renderer and an owned simulated
Windows message path. Ordinary preview/output remains closed pending verified
native animation and input delivery. Diagnostic samples are not a vendor fade.

## Source contract

Studio Reactive is name reactive, effect3; firmware quick effect IDs are separate.
The shipped editor offers colour, Random Colour and durations2000/1500/500ms.
Newly assigned memberships use colour65280, randomColor:false, duration1500,
playbackStart:onPress, playbackEnd:after and cycles1, plus inherited layer metadata.
Existing unknown/unlisted duration values are retained and do not select a preset.
Editing Random Colour writes a boolean; None omits color rather than writing null.

Scalar edits preserve opaque raw parameters, including numeric spellings and
inherited playback settings. Newly assigned memberships are built only from the
composition's explicit LED inventory; conflicting layer IDs, stale LEDs, ambiguous
membership, invalid values and random-enabled colour edits refuse atomically.
Mixed selections are reported. Clearing an inherited layer colour is refused
pending source verification, rather than removing the shared layer value and
changing unselected LEDs. That edge case is a TODO; other controls remain usable.

## Evidence and verification

Installed cached Studio main.88ac681e.js SHA256
109b483f129044e1c7c17b9fcae0e30fa775885667976807cf2c94bd1c696d19 supplies
module6257 constants and the effect-defaults factory. Chunk42.0132a52b.chunk.js
contains custom-reactive, its colour picker, random disable rule and duration
index conversion. The public fixture was independently extracted from literal AST
expressions, not constructed from Rust expectations. Full source hashes and REA
identifiers are in validation/studio-reactive-authoring.json. Three unexecuted
source files/projections were inspected:23931bytes,6745ASTnodes, zero parse
failures and929semantic unknowns. Static evidence does not prove frontend execution.

Three core checks cover source defaults, region split/raw preservation, mixed
selection, random guarding, None, history, invalid values/identities/inventory,
and inherited-clear refusal. The actual exported composition passes a native
simulated-window flow: add Reactive, select a source LED, save RGB112233 and
Fast500ms with source trigger defaults, test random-disabled edits, Undo/Redo,
reject invalid colour, save None, verify read-only refusal and explicit preview
refusal. Original composition and profile files remain byte-for-byte unchanged;
the simulated command log contains zero device requests. No physical discovery
or input generation occurs.

The native production-painter capture was reviewed; controls and status do not
overlap. Native child edit controls are omitted from that parent capture; their
existence and entered values are verified by saved output. The existing stock
Studio capture and shipped reactive component structure guide layout, but no
matched-state Reactive capture or pixel equality is claimed. Stock colour picker,
checkbox, duration slider/animation, exact spacing and full state comparison are
TODOs. Approved host tabs were not changed.

Workspace468core/295tray/4CLI/1doc checks pass; six core and41tray checks remain
intentionally ignored. The native Reactive check passes separately.

## Remaining work

Implement the recovered key event route, fade/retrigger/random-colour behaviour,
preview and guarded saved output. Verify real input/output, stop/failure recovery
and sleep/reconnect. Playback controls beyond the source defaults and the inherited
clear edge case remain TODOs. Continue independent work when those stall. Full
profile/device/input/privacy/visual and three complete-build memory requirements
remain open; authoring checks do not satisfy them.

The optimized workspace build and all eight native Studio checks pass, including
Reactive and the existing save-failure/exit-recovery flow, on simulated devices.
