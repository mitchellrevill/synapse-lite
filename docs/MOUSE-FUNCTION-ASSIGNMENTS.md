# Mouse Function local assignment editor

Mouse Function now saves desired assignments to the selected local profile. Cancel
discards the draft. Save in the dirty-close alert completes the pending transition
only after persistence succeeds. A failed save keeps the editor and draft open.
The worker compares the selected device, active profile and complete profile
snapshot before saving; read-only mode refuses writes.

The ten recovered base choices are Left Click, Right Click, Scroll Click, Double
Click, Scroll Up/Down, Mouse Button 4/5 and Scroll Left/Right. The existing source
popup geometry, category rail, dropdown and Turbo controls are reused. Primary
mouse Left Click remains protected. Wheel inputs and the DPI input cannot enable
Turbo. Turbo rates are 1–20, with source default 7. Ornata keys require a recovered
key identity; unsupported media/dial/controller forms remain gated. Repeat-scroll,
BossKey and scroll-mode-switch choices are not inferred from generic bundle strings:
they require the device capabilities in the shipped MapMouse component.

## Persistence and delivery boundary

Pending intent lives in `_synapseLiteSynapse4.pendingMouseAssignments`, beside the
unchanged original decoded source. It does not add native `buttons` replay entries,
activate a profile, install a handler, intercept input or send a device command.
The UI explicitly says that hardware delivery is unavailable. This is a completed
local authoring/persistence path, not physical remapping acceptance.

Native JSON save/reopen retains pending intent. Synapse 4 export overlays it into
source mapping records before hashing/encoding. Only authored fields change;
unknown mapping/group/Turbo leaves and unrelated layers/records survive. Replacing
a normal Hypershift mapping removes its paired secondary in the effective/exported
view; the original remains in the retained source. A later explicitly authored
secondary is retained. Duplicate, structured/two-tap, inter-device and malformed
matching records refuse instead of being flattened.
An unavailable imported mouse choice is displayed as unavailable and cannot be
saved through a Turbo-only edit. An explicit dropdown selection is required to
replace it; Cancel retains the original.

`Choice::pulse_plan` uses the existing bounded source planner. Buttons have balanced
down/up pulses, wheels are one event with the evidenced axis and signed 120-unit
delta, and double-click has two left pairs separated by the source converter's
10ms delay. Tests pass events to the existing mouse packet bridge. These are bounded
planning primitives; held input/Turbo dispatch and completion/cancellation still
need a proven runtime owner and routing. No driver routing was changed.

## Evidence

Recovered source files, used statically without executing vendor bootstrap:

| Artifact | SHA-256 | Relevant source |
|---|---|---|
| 196 MapMouse.4a6de97a.chunk.js | db7a4ad16941a0c2d6d70694b5566031d430fd9527890c57304408f77c08ee6b | choice capabilities; getMappingData character offset 4121; Turbo restrictions 4792 |
| 605 MapMouse.4d52f9eb.chunk.js | ec0854822b340537695b25b51e1877f69e5a392fbb596fadba24db1a2de04cc9 | same shared fields; getMappingData 4145; Turbo restrictions 4816 |
| 196 257.8f5c638b.chunk.js | 85dd5d60dc9a7b14b588d7a161f17515ec6052bef612aec389452321179ac553 | replacement of normal Hypershift removes paired secondary, around character 45114 |

Retained REA source inventory: parent
`ev_df300d5c0bdd26501a642441e78f64cf4ce1a16adaabb2da1855be302dc22acf`,
summary `ev_37cca753633b7f0d8a50089d497e9ff17bad7a0620fbd1204acc26df9e03a683`.
Semantic coverage remains partial. Source pulse fixture is
`tests/fixtures/source_macro_conversion.json`, including independent double-click
case 9. These establish source intent/packet planning, not successful live delivery.

## Remaining acceptance

Physical routing, held trigger/release/Turbo scheduling, terminal completion,
disable/profile-switch/unplug/sleep/crash recovery, stock live re-import, exact
hover/focus/help rendering and full-build memory acceptance remain open. The saved
stock Mouse Function screenshot is Ornata; exact same-device/state/DPI screenshot
parity cannot be inferred from a mouse fixture capture. No complete-build memory
claim follows from these local tests.
