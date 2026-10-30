# Shared native Studio gradient editor

The existing shared Spectrum property module now also supplies a colour strip,
selectable/draggable stop handles, HEX, RGB and HSV entry, and the eight default
source picker colour tiles to Spectrum, Wave, Audio Meter, Starlight and fixed
Breathing. Existing effect presets, add/delete limits and None semantics remain
in their effect-specific core paths. No effect algorithms were replaced.

Choose a stop with Previous/Next or its strip handle. Position is integer percent.
HEX accepts six digits with an optional #. RGB accepts comma-separated integers
0..255. HSV accepts hue 0..360 and saturation/value 0..100, with decimals.
Switching representations changes view state only. HSV display uses four decimal
places; committed edits round to RGB bytes. None remains an omitted Color field.
Invalid entry refuses without a document/history change. Colour tiles directly
change the selected stop, keeping its position.

Drag a handle to move its entire raw record. Release commits one undo entry;
losing capture or cancellation discards the transient move. Clicking an unchanged
handle does not change the document. Stable reordering keeps the moved/inserted
stop selected, including at duplicate percentages. Breathing retains its stricter
ordered-stop admission. Move changes only Stop, preserving Color and unknown
stop leaves. The existing regional transaction protects unselected memberships,
unknown profile data, mixed selections, save checkpoints and Undo/Redo. Mixed
values are shown explicitly; choose an effect preset to deliberately replace
them before editing a specific stop. Random Starlight, unsupported gradients,
read-only mode and empty selections retain their refusal guards.

## Evidence and limits

Source picker606 SHA256
`04b31567b57a5cf0b37bb4c2d7f81d1efa868aba4a33498e121ecbc1452e0c5e`
contains array x near decoded character365: None, red, orange(255,128,0), yellow,
green, cyan, blue and magenta. It exposes hex and RGB fields and converts HSV for
its colour plane. Numeric HSV entry is a useful native extension, not a claim
that stock has equivalent HSV text controls. Existing effect-palette fixtures
continue to verify Spectrum's five recovered palettes independently.

REA parent `ev_a02d7b1bbe545689660e69523e909cdc13bcf8082950ea71889fd8db8ed13895`
was reused against the same two-file source directory (30119 bytes, zero parse
failures, 2429 unresolved semantic relationships). This is inert source analysis,
not runtime observation. See STUDIO-WAVE-GRADIENT.md and STUDIO-SPECTRUM.md for
the previously recovered stop and regional contracts.

The retained stock screenshot `stock-studio-color-gradient-editor-0.jpg` was
inspected. It shows a compact gradient colour bar above properties at a different
viewport/state. Native now has the strip/handles and explicit colour modes, but
still uses an inline panel rather than the stock popup. Palette geometry,
colour plane/eyedropper, remembered custom picker colours and panel scrolling
remain visual/workflow TODOs. This change does not claim pixel parity, stock
re-import acceptance, physical lighting or complete-build memory acceptance.

## Validation

Focused core preservation/history tests, colour conversion tests and the native
Wave interaction regression cover this change. The native check uses an isolated
simulated engine, explicit source export/assets and a private config directory;
it verifies saved blue HSV/drag records, cancellation and read-only history,
then restores the original Wave edit for existing save/preview checks. It must
leave the source file and device profiles byte-identical and issue zero device
requests. Painter captures omit native child EDIT controls; native tests check
those fields separately. See validation/studio-gradient-editor.json for results.

Validation on 30 October 2026: 44 regional core checks and eight Breathing core
checks passed, as did the eight-test core gradient filter (overlapping those
sets). Four regular tray gradient checks and the independent Spectrum palette
fixture passed. All four explicit simulated native authoring tests passed.
The initial restricted-desktop attempt could not create its window; the retry
with desktop access passed. Visual inspection caught and corrected doubled
RGB/BGR conversion in the new paint calls and status/mode overlap. No stock app,
other checkout, physical device discovery or hardware transport was used.

The ordinary native debug build passed with one build job and an isolated target
directory. Three final swatch/strip pixel samples verified red and blue are not
swapped. Existing unrelated unused-code and incremental-cache warnings remain.
