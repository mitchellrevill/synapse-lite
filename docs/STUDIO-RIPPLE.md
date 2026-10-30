# Studio Ripple regional authoring API

This checkpoint implements the pure Rust draft API for Studio Ripple (effect 1).
It is a bounded authoring unit, not completion of Studio Ripple. The native panel,
wave propagation, event-triggered preview and saved physical output remain open.
No UI, Reactive implementation, event observer, input ownership gate or device
transport was changed. There is no changed UI to screenshot in this checkpoint.

## Implemented contract

`Draft::set_ripple_property` edits Speed or Width; `set_ripple_motion` edits both
in one Undo transaction. Newly authored speed is 1..50; width is 100/200/300/400
percent, following the source step of 100. Imported out-of-range or non-step values
remain raw when another property is edited. New memberships start with speed 10,
width 300, the seven-stop rainbow, colorStopsCustom false, playbackStart onPress,
playbackEnd after and cycles 1. Layer parameters override these defaults; region
parameters override the layer. Existing memberships do not acquire missing defaults
during scalar edits.

`set_ripple_preset` selects one of the five source patterns. They share module6257's
general pattern table with Wave; they are not Spectrum's patterns. `edit_ripple_stop`
supports Set/Insert/Delete through the existing raw-preserving gradient editor,
with a seven-stop insertion limit and at least two stops after Delete. None omits
the Color field. Editing inherited gradients copies the layer's raw stop records,
including unknown metadata. Preset replacement explicitly replaces that gradient.

`set_ripple_playback` atomically writes a typed `RipplePlayback` tuple. Starts are
random, onPress and onSelectedKeys; ends are never, after and toggle. After requires
1..100 cycles; other ends require -1. Random+toggle is not an offered source state
and is rejected. This API represents the resulting source settings; it does not
implement the panel's remembered repeat-count state or execute triggers. The source
panel resets Start=random to Never/-1 and an input start to After with its remembered
count (initially 1). End changes use After's remembered count, otherwise -1.

Selection getters report Empty, Mixed or Uniform. Missing imported scalar/playback
fields stay missing; unsupported future playback strings produce a read error without
rewriting the file. Authoring a different property can still preserve those strings.
No arbitrary missing renderer parameter is silently defaulted for execution.

Edits use the existing inventory and regional partition machinery, preserving
unselected memberships, unknown JSON values and numeric spellings. Invalid values,
foreign LEDs, overlapping selected memberships and conflicting effect identities
refuse before committing. Empty motion selections do not change the draft. Archives
can export and reopen the edited composition while preserving other profiles and
raw envelope values.

## Source evidence

Observed static contracts come from the installed cached Studio renderer:

- `main.88ac681e.js`, SHA256
  `109b483f129044e1c7c17b9fcae0e30fa775885667976807cf2c94bd1c696d19`:
  module6257 constants/patterns and the effect-defaults factory.
- `255.e317cf93.chunk.js`: module5305's `custom-ripple` contains gradient,
  speed, width and shared playback controls. Module5805 establishes Start/End
  transitions, count bounds and exclusion of random+toggle.
- The extractor `tools/extract-ripple-contract.py` reads inert literals and asserts
  the specific factory/control wiring before generating the independent fixture
  `crates/secbird-core/tests/fixtures/studio-ripple-contract.json`. It never executes
  the renderer. The fixture records the three input hashes and every pattern stop.

REA summary `ev_af625da5c7e3e9440a4cafef9bef7afd3bce6478faf8a4ade4472d81d63268de`
links to retained parent
`ev_36d880b858f18e00cc52b16a25436f762fdbecca11b1a2bb8a73bc4587f824fd`.
It inspected three source files (50,885 bytes, 10,677 AST nodes), zero parse failures,
and 1,397 semantic unknowns. The source set also included the English label chunk;
static analysis is not proof of renderer execution or current live UI equality.
The July 2025 source build is recorded historical shipped evidence.

## Remaining implementation and acceptance

1. Recover the native Ripple initializer, coordinate/radius math, speed/width
   mapping, palette direction, overlap/retrigger, random start, repeat and toggle
   semantics. No saved Ripple native analysis was found in the scoped evidence
   directory. REA reports Ghidra unavailable because GHIDRA_INSTALL_DIR is unset.
   Do not derive Ripple's radial renderer from Wave's linear formulas without
   evidence. Ordinary Preview and physical output still refuse active Ripple.
2. Build the native panel over these APIs; preserve the source's count memory and
   mode-switch behavior. Verify code plus matched-state screenshots against source
   controls. This checkpoint introduces no UI or pixel-parity claim.
3. Connect the existing exact-device event-observation abstraction after input
   admission and recovery are proved. Synthetic tests are suitable; no input
   registration, injection, driver or hardware stream was started here.
4. Verify real trigger origin, cross-device coordinates, composition order,
   stop/failure/unplug/reconnect and optical output. These remain explicit live
   device acceptance tasks. Catalogue presence must not admit device writes.
5. Complete the overall profile/device/network/visual work and three complete-build
   RAM measurements below 100,000,000 bytes. Core authoring tests cannot establish
   those acceptance gates.

Validation results and the precise integration base are recorded in
`validation/studio-ripple-authoring.json`. This change is intentionally based on
published main `6149f7a`, excluding unrelated unpublished canonical integrations.

Nine Ripple tests pass in the optimized test build. The full core run passes
523 tests, with six existing ignored tests, plus one passing doc test. The Ripple
checks exercise all 63 mode/count combinations, every authored speed and width
boundary, independent defaults/pattern data, partition/history/raw preservation,
None and inherited stops, invalid identities/inventory, gradient limits, archive
round-trip and continued ordinary-preview refusal. Initial system-temp access
failures were resolved by setting TEMP/TMP to task-owned directories. No full-app
optimized build, live hardware test or RAM acceptance is implied by these results.
