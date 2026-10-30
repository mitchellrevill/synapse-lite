# Studio Spectrum regional authoring

Open a composition, select its Spectrum layer, then select LEDs. The native
editor now provides the three recovered duration choices, five source gradient
presets and individual stop editing. Previous/Next selects a stop; enter percent
and RGB, then Apply Stop or None. Add Stop inserts the entered values (a blank
colour inserts None); Delete removes the selected stop. These actions participate
in the same bounded Undo/Redo, Save and Discard history as other regional edits.
Changing layers clears LED selection, as the source does.

These controls author local files. [Local Preview](STUDIO-PREVIEW.md) now renders
the admitted Static/Spectrum/Fire composition on the canvas, without hardware
frames or physical-colour acceptance. This is not complete Studio parity. Duration
track, numbered preset buttons and percent/hex fields use native presentation.
The follow-up below adds direct stop gestures and panel scrolling; exact stock
gradient popup geometry, colour picker and remembered custom tile remain open. Captured production paint excludes native child EDIT controls;
the tests separately exercise those controls and verify their saved values.

## Stored semantics and preservation

- Duration is75480,37740 or18870 milliseconds, never the slider index0–2.
  Missing/unsupported imported durations project as37740 without changing the
  loaded archive. A gradient edit materializes that projection when duration
  is uniform; mixed durations retain their per-existing-region raw values.
- Gradient records use `Stop` integer percent and packed RGB24 `Color`. None
  omits Color, rather than writing null or erasing LED membership. Stable sorting
  retains duplicate positions; endpoints need not be0 and100.
- New replacement gradients require2–10 stops. Add refuses at10; Delete refuses
  at2. Imported gradients are not truncated to10: the bounded editor can inspect
  up to512 stops, modify/delete them, and preserve them through duration edits.
  Unsupported imported stop data remains raw; duration-only edits do not require
  a supported gradient projection.
- A preset explicitly writes `colorStopsCustom:false`. Stop edits explicitly
  write true, even if the result equals a preset. Untouched custom flags retain
  omission. Full preset replacement replaces the gradient deliberately; a stop
  edit preserves unknown fields and raw literals on the other stops and on the
  edited stop itself, changing only Stop/Color.
- Unassigned Select LEDs start with no gradient, false custom flag and37740.
  Mixed selections use the first encountered selected LED's normalized regional
  property state for newly assigned memberships. Existing overlaps retain their
  differing untouched fields separately. Selection ordering is explicit in the
  core API; native selection follows its current admitted-coordinate ordering.
  Exact stock ordering for every canvas tool remains unverified.

Layer.params, unselected/offline memberships, unrelated profiles, unknown region
fields and large numeric spellings retain the raw-preservation contract.
Ambiguous inventory/memberships, effect ID conflicts and invalid authored values
refuse atomically. File load/save alone never normalizes imported effect fields.

## Evidence

Cached Studio main `main.88ac681e.js` SHA256
`109b483f129044e1c7c17b9fcae0e30fa775885667976807cf2c94bd1c696d19`:
module6257 supplies the five palettes and durations; module2904 supplies selected
parameter normalization; selection handler ze around109300–111200 visits selected
LEDs and retains the first distinct parameter projection. Regional merger9286
around303100–305500 restores remaining mixed keys on existing regions and applies
the current projection to unassigned LEDs. Offsets are decoded characters.

Spectrum/gradient chunk820 SHA256
`6395219cfde8a06aa2b651073efaa6aace2e0eb3c36dcfd5ad90bd95e5fbd9a8`
and shared picker606 SHA256
`04b31567b57a5cf0b37bb4c2d7f81d1efa868aba4a33498e121ecbc1452e0c5e`
establish completed duration/gradient patch granularity, authoring bounds,
duplicates, movable endpoints and omitted None colours. Static source evidence
does not prove equality with the currently installed live frontend or native
effect execution. Spectrum4 here is a Studio API namespace, not a firmware ID.

Seven core regressions cover independent duration/gradient patches, mixed and
unassigned projections in either selection order, raw metadata, invalid/None
colours, duplicates, deletion limits, unsupported and twelve-stop imports,
history and atomic refusal. The independent source-data fixture verifies all
five native palettes, with stop counts10/7/6/5/3.

The isolated actual-export native test uses simulated devices: it checks
duration-only gradient preservation, preset/stop/None saves, add/delete history,
invalid percentage refusal, read-only enforcement and layer-switch selection.
Original export and device profiles remain unchanged and no device SET is sent.
See [native evidence](validation/native-studio-actual-file.json). Stock acceptance
of edited files, effect playback/compositing, optical output, full recovery and
complete-build memory acceptance remain unverified. The full goal stays active.

## Native control follow-up (2026-10-30)

The duration track now selects the source's three discrete values on release.
The stop track below the fields selects/drags existing stops and inserts the
entered colour at an empty track position. Crossing neighbours retains the
selected stop under stable sorting; insertion selects the inserted record.
Delete remains available down to the two-stop source minimum. Dragging commits
one edit on release; capture loss cancels the pending gesture. These changes
reuse the existing atomic regional operations and never start device output.

The Spectrum property body supports wheel scrolling, clips painted controls and
click targets, moves native edit fields, and shows a scroll thumb at short
heights. Shared gradient panels for other effects retain their existing layout.

Source: cached chunk 820 `j` uses a discrete duration slider with Q9 index mapping
and dispatches onAfterChange; the existing module6257 fixture supplies durations.
Cached chunks 820/606 and the earlier evidence above remain the basis for stop
limits, percent positions, stable sorting and None preservation. This follow-up
is a native interaction implementation, not a claim of pixel equivalence: stock
popup geometry, colour picker, remembered custom tile, exact insertion colour
sampling and drag-away deletion still need source/runtime visual acceptance.
Full application parity and total RAM acceptance remain open.

Verification: seven core Spectrum preservation regressions, three focused native
unit tests, and the opt-in Spectrum-only real-window regression passed. The
latter uses a private stock export with simulated transport, verifies read-only
admission, crossing/None persistence, one drag undo entry, click-without-edit,
cancellation, add/delete undo and unchanged input/store. Production captures
were visually inspected at normal size and the native minimum 650 CSS pixel
height with the property body scrolled to its end. The window enforces that
minimum; smaller unsupported sizes are not claimed. Child EDIT windows are
excluded by the existing capture function and verified through saved values.
The broader existing stock-file test timed out in its preview-hide stage; it is
not counted as passing this checkpoint. One focused run also timed out during
window presentation; the final repeated focused run passed in 3.30 seconds.
