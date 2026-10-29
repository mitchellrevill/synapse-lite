# Wave custom gradient editing

Select a Wave layer and LEDs, then choose **Edit Gradient**. The shared gradient
controls edit individual percent/RGB stops, clear a colour to None, insert and
delete stops, and select any of Wave's five recovered presets. Back to Motion
returns to direction, speed, width and pause. Enter applies the active stop field.
Editing participates in preview, Undo/Redo, Save and Discard.

Wave uses its own presets and never writes Spectrum duration defaults. Stop edits
mark `colorStopsCustom:true`; presets mark it false. None removes `Color` without
removing the stop or LED membership. Stable sorting retains duplicate positions.
Insertion refuses at ten stops and deletion refuses at two. Larger admitted
imported gradients are not truncated.

Only selected memberships change. Unselected memberships, layer parameters,
motion values, unknown fields and raw numeric spellings are retained. An inherited
layer gradient is materialized only for selected regional editing. Mixed gradients
require choosing a preset first. Invalid position, RGB, index and unsupported
Wave variants refuse atomically. Split, rotational, finite and input-triggered
Wave authoring remain TODOs.

## Source evidence

Cached Studio Wave chunk `285.2c6cd39f.chunk.js`, SHA256
`93283775e1c7fa30ba3fde49a92c6de6848d3939e2214a16b996b942b6276ca6`,
contains `custom-wave`, selector `S` and the shared gradient component. Around
decoded characters 3000–4100, completed changes dispatch only `colorStops` and
`colorStopsCustom` through the regional parameter action. Its gradient component
uses the same conversion and authoring contract as Spectrum. Shared picker chunk
`606.6175bd02.chunk.js` SHA256
`04b31567b57a5cf0b37bb4c2d7f81d1efa868aba4a33498e121ecbc1452e0c5e`
and the earlier [Spectrum evidence](STUDIO-SPECTRUM.md) establish None and stop
mutation bounds. Existing Wave playback interprets the same regional records.

Fresh REA summary `ev_b17ef7bc9c0e06f2799b42c98a9fb0b4d22f2d9662e5eb8e4f2f1279a03edf67`,
parent `ev_a02d7b1bbe545689660e69523e909cdc13bcf8082950ea71889fd8db8ed13895`:
two cached chunks, 30119 bytes, 10366 AST nodes, zero parse failures; 2429 semantic
unknowns remain. This is static evidence, not observation of the installed live
frontend or proof of complete application equivalence.

## Verification and remaining work

Three added core regressions check regional partitioning, inherited gradients,
None, stable duplicates, raw metadata, no Spectrum scalar writes, history,
limits and atomic refusal. Existing Spectrum regional tests pass after extracting
the common raw stop mutation. Workspace: 454 core, 291 tray, four CLI and one doc
test pass; six core and 38 tray opt-in tests remain ignored in that run.

The isolated native Wave test loads the actual stock composition, adds Wave,
edits both endpoint colours, checks animated RGB `[17,34,51]`, refuses 101 percent,
saves exact motion/gradient fields and verifies Undo/Redo and read-only refusal.
Source file and device-profile bytes remain unchanged, with zero simulated
device requests and physical discovery disabled. Six optimized native Studio
checks also pass, covering existing Spectrum/Fire authoring, Breathing, layer
operations, file-save recovery and simulated Apply/Stop alongside Wave.

The production-painter panel capture was visually inspected against the retained
stock gradient-editor reference. **The UI is interim, not pixel-perfect.**
Remaining differences include the stock gradient strip/popup, colour picker,
draggable handles, remembered custom tile and panel geometry/scrolling. Painter
captures omit native child edit controls; the test separately checks their
existence, input handling and saved values. The reference shows Spectrum at a
different viewport/state, so this comparison does not establish pixel equality.
Keep these visual TODOs and continue remaining effects and device/input features.

No new physical/optical lighting, all-family device coverage, complete privacy
or complete-build memory acceptance is claimed.
