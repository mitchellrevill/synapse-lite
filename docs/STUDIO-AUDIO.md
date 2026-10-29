# Studio Audio Meter authoring

Layer Actions now includes **Add Audio Meter**. Select LEDs, edit Boost and Decay,
toggle Auto, and use Edit Gradient for the four recovered patterns or individual
percent/RGB stops. Auto preserves the manual Boost and disables editing it. None
omits a stop's Color. Mixed values remain explicit. Edits use the existing region
partition, Undo/Redo, Save, Discard and editor recovery infrastructure.

This checkpoint implements authoring. **Audio-reactive canvas preview and saved
device output remain unfinished.** Active Audio Meter regions still refuse those
routes through the existing compositor. Opening, editing, saving and reopening
never start audio capture. No raw audio is recorded. The existing explicitly
requested WASAPI diagnostic remains separate; see [AUDIO-CAPTURE.md](AUDIO-CAPTURE.md).

## Recovered contract

Studio uses name `audio`, effect ID12, separate from Quick/firmware IDs. The source
defaults for newly assigned memberships are Boost1, Auto false, Decay1 second,
colorStopsCustom false and the first Audio gradient. Boost accepts0.25–4 in
steps of0.25; Decay accepts0.1–2 seconds in steps of0.1. Auto disables only Boost;
gradient and Decay remain editable. Its four gradients are:

| Pattern | Percent / RGB stops |
| --- | --- |
| 1 | 0/00FF00, 51/FFFF00, 100/FF0000 |
| 2 | 0/FF0000, 26/FFF300, 42/36FF48, 55/62F1FF, 68/000CFF, 81/8000FF, 100/FF0000 |
| 3 | 0/0000FF, 51/FFFFFF, 100/FF0000 |
| 4 | 0/000000, 100/00FF00 |

The shared picker admits seven authored stops, refuses deletion below two and
retains imported data without truncation. Presets set colorStopsCustom false;
individual edits set it true. Inherited gradients materialize only for selected
regional editing. Stable sorting and the shared raw stop mutation preserve opaque
stop metadata. Unselected memberships, layer fields, unknown parameters, imported
values and numeric spellings survive unrelated edits. Invalid input, ambiguous
membership, stale inventory and conflicting effect IDs refuse atomically.

The engine wrapper converts these fields to Mode0, BoostLevel, AutoBoost0/1,
Decay in milliseconds, Auto0 and Flow (default0). Missing Color maps to the native
transparent sentinel0x64000000; missing0/100 percent endpoints are extended from
the first/last color. A controlled replay verified this descriptor conversion
with an inert native stub. It does **not** establish the proprietary FFT scaling,
AutoBoost normalization, attack/decay envelope or per-LED animation algorithm.

## Evidence and checks

Cached Studio `main.88ac681e.js`, module6257 and the effect-default factory establish
defaults, ranges and palettes. `412.f789b489.chunk.js` supplies `custom-audio`,
gradient controls, mixed state and the Auto disable rule. The public numeric
fixture was extracted independently from literal AST expressions. Cached engine
`main.a383ea9e.js`, recovered module2578, supplies the descriptor conversion.
Hashes and REA references are in [validation/studio-audio-authoring.json](validation/studio-audio-authoring.json).

REA parsed the selected Audio UI chunk as inert text: one file,13556 bytes,
4675 AST nodes, zero parse failures and1097 semantic unknowns. This covers the
selected chunk, not the whole application or live frontend behavior. The native
DLL was identified by hash, but the connected provider lacked its Ghidra setting.
An isolated CLI process reused the existing installed Ghidra and private snapshot;
both queued native queries timed out after240000ms each. No successful native algorithm
evidence was returned and no vendor DLL was executed.

Four core regressions cover independently extracted defaults/presets, raw regional
preservation, Auto guarding only Boost, mixed selections, inherited gradients,
None, history, source bounds and atomic refusal. The native simulated-window test
loads the actual stock export, adds Audio Meter, selects a source LED, edits and
saves Boost2.25/Decay0.3 and a seven-stop custom gradient, saves None, verifies
Undo/Redo, Auto disablement, invalid values and read-only refusal, and checks that
unsupported preview refuses. Original composition and device-profile bytes remain
unchanged. Its command log contains zero device requests with physical discovery
disabled. It does not capture audio or prove physical output.

The offline workspace suite passes513 core,306 tray, four CLI and one doc check;
six core and48 tray opt-in checks are excluded from that ordinary run. The
optimized workspace build passes. Optimized Audio Meter native authoring passes.
The optimized Studio group passes11 of13 checks; existing Breathing None-save
and preset-reopen checks time out in that group and each passes in a separate
optimized process. The earlier debug group also had two timing failures, retained
in the validation record. A synchronous repaint after Audio's load completion
now establishes its paint-owned hit regions before test clicks. Whole-group GUI
timing stability remains a follow-up; individual passes do not erase group failures.

The production-painter property and gradient captures were visually inspected:
controls and status do not overlap. Parent captures omit native child edit fields;
the test verifies their existence and saved input values separately. The panels
remain interim: stock sliders/steppers, checkbox/popup, gradient strip, color picker,
drag handles and matched-state visual comparison remain TODOs. Approved shared
host tabs were not redesigned.

## Next checks

Repair the isolated native provider's startup/bridge timeout, then locate the
Audio effect12 constructor, initialization, frame and audio input routines in
the pinned RzLightingEngineApi DLL. Recover FFT/bin/scale, AutoBoost, gradient
sampling, transparent behavior, time step and decay from those routines before
connecting existing PCM analysis to a renderer. Keep source evidence separate
from any chosen local algorithm and deterministic fixture results.

After that, implement explicit capture selection/start/stop and fault feedback,
fresh-frame expiry, default-change/unplug cleanup, draft/saved snapshot separation
and shared-worker output admission. Capture must never start merely from editing
or reopening. Verify deterministic audio fixtures and isolated simulated output,
then non-silent physical playback and optical behavior when the user is available.
Idle endpoint reads and synthetic tones are not non-silent hardware proof.

The retained full product/device/profile/input/privacy/visual requirements and
three complete-build runs below100,000,000 bytes for both working set and private
memory remain open. This authoring checkpoint does not close the Audio Meter
runtime goal or narrow the overall product goal.
