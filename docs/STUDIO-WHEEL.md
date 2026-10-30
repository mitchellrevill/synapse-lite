# Studio Wheel authoring and continuous playback

Wheel13 now has native authoring, composition persistence, deterministic local
preview and the existing explicit Apply Saved compositor path. This is a bounded
Wheel checkpoint, not complete Synapse parity, pixel-fidelity acceptance or a
complete-build RAM acceptance result.

Select Wheel in Layer Actions, select LEDs, then edit speed or the gradient.
The speed field accepts0..360; dragging the speed track commits one undo entry on
release and cancellation discards the transient drag. Five source presets and
up to seven editable RGB24/None gradient stops reuse the existing gradient panel.
Center and direction are layer-wide, while speed and color stops are regional.
Select Center Point places the center without replacing LED selection; numeric
editor coordinates0..10000 are also available. A cross marks the center. Save,
undo/redo and reopening preserve composition data, including raw unknown numbers.
Center-only changes now correctly mark a Wheel draft dirty.

## Source contract

Recovered historical Studio `main.88ac681e.js` module6257 establishes speed
minimum0, maximum360, default60, the shared Wheel/Wave five gradient presets,
continuous `playbackStart:random`, `playbackEnd:never`, `cycles:-1`, and the
default editor center5000/5000. Module1981.DO maps editor coordinates to native
coordinates with `round(x/40)` and `round(y/40)-2`. Layer construction already
stored the resulting center125/123 and clockwise rotation. Recovered
`729.b16ed779.chunk.js` module8379 establishes the Wheel color/properties panel,
speed control, center picker and clockwise/counterclockwise controls. Its speed
and gradient edits target regional params; center and rotation target params2.

The installed lighting backend's recovered `native-effect-params.js` maps
Wheel13 to Mode1024 clockwise or2048 counterclockwise, ColorStops, Speed, Cycles,
CenterRow and CenterCol. There is no separately evidenced initial-angle/rotation
offset property in Wheel authoring, so none was invented.

Native evidence is from `RzLightingEngineApi_v4.0.55.0.dll`, SHA256
`cd9fc2a61ff920b9b73e1f5e27632020e2f8586f443e426a875ad3b042b714fa`.
Wheel initializer0x180066b40 and renderer0x180066d60 were recovered with the
installed Ghidra12.1.4 in an isolated no-analysis import. Dumpbin instructions
closed the decompiler's omitted column argument to angle helper0x180066f20.
This is static reconstruction, not execution of the vendor renderer. REA's
connected provider lacked its Ghidra configuration; the first CLI recovery
failed and a full configured recovery timed out before the smaller import
succeeded. Successful direct Ghidra output is not labelled REA Evidence.

The renderer uses a1024-entry four-channel palette. At25fps its period is
`trunc_f32(360000/speed)/40`; speed0 freezes the phase. Spatial indices use
`trunc(1024*(atan2(row+originRow-centerRow,col+originCol-centerCol)+pi)/(2*pi))`.
The row argument passes through f32 before double atan2; the column is an int
converted to double. Clockwise reverses the temporal offset. Native centers
outside0..255 fall back to0. Absent stops use native red/green/blue/red anchors
at0/33/66/100. Authoring seeds the distinct seven-stop Studio rainbow instead.
Missing endpoint anchors duplicate the first/last color. Malformed gradients
are refused rather than guessing the native invalid-stop branch.

## Verification

- Eight independent Python scalar cases each compare200 frames, across both
  directions and speeds0/1/60/360. A masked Wheel-over-Static composition also
  matches200 independently derived frames and preserves snapshot/random state.
- Tests cover cardinal phases, center/origin offsets, period, speed0 freeze,
  wrong-size non-advancement, bounded storage, regional inheritance/partitioning,
  mixed values, RGB24/None gradients, raw unknown JSON, undo/redo and refusals.
- The isolated native HWND test exercises center-only dirty tracking, save,
  speed drag with one commit, center picking without membership changes, Add
  Wheel, gradient editing, animated preview, read-only and invalid-value gates.
  It asserts zero simulator transport requests, unchanged input and unchanged
  private profile-store bytes. No physical discovery or output is used.
- Core suite:520 passed,6 ignored; core doctest:1 passed. Tray suite:306 passed,
  48 ignored. The Wheel HWND test runs separately with `--ignored`.
- Optimized `secbird-tray` build passes. These checks use jobs1 and private Cargo
  target/temp paths. Compiler warnings from the published base remain.

`tools/derive-studio-wheel-reference.py` reproduces the scalar fixture without
loading vendor code. `validation/studio-wheel.json` records provenance and gaps.
The task's `outputs/wheel-validation` folder holds native properties, gradient
and preview screenshots plus an offline controlled replay of the original Wheel
property JSX/CSS. Redux, translation, tooltip and utility seams are substituted;
visible slider/toggle widgets and their icons come from recovered source. The
comparison is a source reference, not a live stock-app capture or a zero-diff
pixel acceptance result.

## Remaining gaps and admission boundaries

Native property layout, gradient navigation and numeric center controls differ
from the recovered source panel; exact pixel fidelity remains open. The source
snapshot is the July2025 Studio build and does not prove today's active UI.
Imported fractional editor center coordinates are retained without mutation but
are not yet authorable by the bounded integer center controls.

Local composition preview retains the pre-existing gate: one exact, unrotated,
default-layout Ornata V2 placement with a saved explicit inventory and source
geometry. Foreign devices, editor-only presets and unproven layouts remain
refused. Other effects' admission gates are unchanged. No universal hardware
compatibility, optical comparison, stock re-import or physical Wheel stream is
claimed. Finite cycles and input-triggered playback remain explicitly refused;
native input ownership/terminal completion is not relaxed.

The palette plus161-cell phase plan uses about4.4KiB per Ornata Wheel generator.
That excludes the application and other components and does not establish the
under100MB total-RAM goal. Whole-build memory/network acceptance remains open.

The isolated clone retained canonical base7b99930. The feature branch was based
on verified published main6149f7a instead, to exclude unpublished integrations.
No canonical files, shared profiles, running Lite/Synapse instances, drivers or
hardware streams were changed by this checkpoint.
