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
buttons, numbered preset buttons and percent/hex fields are interim controls;
the stock slider, gradient popup/drag handles, colour picker, remembered custom
tile and complete panel scrolling remain to implement. The current native test
checks the ordinary 1280×720 CSS viewport; smaller property-panel layouts remain
unverified. Captured production paint excludes native child EDIT controls;
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
