# Studio property clipboard and context actions

The native Studio canvas now offers Copy, Paste, Reset Effect and Remove Effect
on right click. Ctrl+C/Ctrl+V use the same operations; Delete/Backspace remove the
selected LEDs' membership from the current effect. Delete with no LED selection
retains the existing layer-delete shortcut. Escape, outside click, focus loss and
cancel dismiss the property menu. An open menu blocks canvas wheel zoom.

## Behaviour and preservation

- Copy requires LEDs on exactly one device and uniform effective regional
  properties. Empty, mixed, ambiguous/overlapping, unknown-effect and conflicting
  effect-ID selections refuse without changing the document or previous clipboard.
- Paste accepts multiple destination devices in the same layer and draft session.
  Changing the selected layer or composition clears the UI clipboard. A clip from
  another draft/layer is also refused by the Rust API. Nothing is placed on or
  read from the operating system clipboard.
- Paste copies the regional properties and captured layer-wide `params`, including
  Wheel/Tidal center settings. Copied None colours clear the corresponding known
  fields. Destination opaque fields absent from the clip remain intact. Fields
  explicitly present in the clip replace the corresponding destination fields.
- Reset merges the recovered default fields for all 13 retained effects. It keeps
  opaque regional properties, regional/device/LED metadata and layer-wide params.
  Reset and context Remove require exactly one selected device. Remove edits only
  current-layer membership and clears the transient LED selection, preserving
  other effects and the composition's device inventory.
- Paste, Reset and Remove each make one bounded reversible history entry. Unknown
  large integers/exponents remain raw; unrelated profile fields are not re-encoded.
  Layer-wide params now participate in saved/dirty comparison as regions do.
- New target memberships use explicit composition LED inventory, never geometric
  guesses. These operations do not admit hardware output or effect runtimes.

Known property equality uses raw value spellings conservatively. Differently
encoded opaque numeric values can therefore refuse Copy as mixed even if a
floating point parser would round them to the same value. Existing unrecognised
regional fields survive reset and splitting; pasted copied values are explicit
user property replacements. The UI disables Copy for mixed properties instead of
opening the stock unable-to-copy modal; the shortcut reports a local status error.

## Recovered evidence

Private cache base: `work/synapse-renderer/cache/apps.razer.com/synapse/chroma-studio/static/`
in the original research workspace. Vendor programs are not committed.

| Artifact | SHA-256 | Relevant code |
|---|---|---|
| `js/main.88ac681e.js` | `109b483f129044e1c7c17b9fcae0e30fa775885667976807cf2c94bd1c696d19` | module9286 R/D/N/x; module2904 d; module6257 constants; effectLayer reducer M |
| `js/EditorCanvas.21953e06.chunk.js` | `190a2ef92615861142a1585b6cbffa578d2a716069f8d430cb6bd63ca103e5c6` | R context menu; canvasOnContextMenu |
| `css/EditorCanvas.9130aa00.chunk.css` | `9163eac79b917bdbf3814ae84e0765515f21076ba5d6bf43fe4009fa19e83e9d` | editor-dropdown flex/helper rules |

Observed statically: the select-mode context menu requires exactly one selected
device for Copy/Reset/Remove; Paste requires both copied parameter objects. Copy's
thunk additionally rejects `paramsMixed`. Reset merges `rH(effect,true)` into
regional params and retains params2; Remove filters matching LEDs in the current
layer's regions. Layer/profile selection clears the property clipboard. The source
Copy thunk alone lacks the menu's one-device guard; Lite enforces it in the core
API so keyboard and mouse routes agree.

REA parent `ev_79d228f14bc7bf94ac04249d45f91fff1c8d211450180b5749edd9d29456b83d`
analyzed the extracted effect-actions factory: 13,846 bytes, 5,375 AST nodes,
zero parse failures. This is static evidence with unresolved dynamic relationships,
not an observation of the vendor app running. Controlled replay of the hash-pinned
constants/default/action factories separately matches 26 default cases and 19
copy/reset cases. The retained JSON is data derived from those expressions.

```powershell
node tools/verify-studio-property-defaults.cjs PRIVATE_MAIN_JS
cargo test -p secbird-core --locked --offline studio::document -j 1 --target-dir OWN_TARGET
cargo test -p secbird-tray --locked --offline property_tools -j 1 --target-dir OWN_TARGET
# Explicit private export/assets, simulated hardware only; opens one owned window.
cargo test -p secbird-tray --locked --offline native_studio_property_clipboard -j 1 --target-dir OWN_TARGET -- --ignored --test-threads=1 --nocapture
```

The native test uses `SYNAPSE_LITE_STOCK_STUDIO_FILE` and
`SYNAPSE_LITE_ASSETS`, a new temporary profile directory and simulated transport.
It covers popup targeting, copy without history, paste, defaults, removal,
undo/redo, save, read-only refusal, unchanged source export/device profiles and
zero transport requests. Pure tests additionally cover mixed selections,
unassigned LEDs, cross-device paste, unknown numeric literals, history invalidation,
foreign clipboard refusal, all effect defaults, layer-wide dirty state and atomic
refusal. Validation results are recorded in `validation/studio-property-clipboard.json`.

## Visual comparison and remaining scope

A network-blocked inert DOM render of the recovered CSS and local Roboto font
measures the full source menu at 256.328125px wide, 27.03125px rows and 14px text.
The native four-action subset uses 256px width, 27px rows, a 9px divider gap,
#000 background, #ccc text, #454545 disabled text and #1a1a1a hover. Source/native
screenshots were inspected at their reported 100%/125% scales. The native owned
capture shows the four property actions over the source-path LED canvas.

This is a scoped native adaptation: Identify, Move, Refresh and move-mode device
rotation are other workflows. Full menu composition, fractional CSS metrics,
placement near native child edit controls, detailed hover/focus parity and full
stock screen pixel comparison remain TODO. No pixel-perfect or physical hardware
support claim is made. Existing effect runtime, selection algorithms and output
admission are outside this change.
