# Studio Wave authoring checkpoint

Individual custom-stop editing is now available; see
[Wave gradients](STUDIO-WAVE-GRADIENT.md). Stock picker/drag geometry and the remaining Wave variants are still open.

Layer Actions now includes **Add Wave**. Select LEDs, choose Up/Right/Down/Left,
one of five recovered gradient presets, or edit speed, width and pause and choose
Apply Motion. Preview, save, erase membership and undo/redo use the existing Studio
workflow. This is an interim native property panel, not a pixel-perfect completion.

New root layers remain empty until an explicit property operation assigns LEDs.
New memberships seed the recovered Studio defaults: speed15, width100, pause0,
angle90, splitfalse, continuous start/end, cycles-1 and the seven-stop rainbow.
Layer-level parameters override defaults; existing regional fields override the
layer. Existing imported memberships do not acquire guessed defaults during a
scalar edit. Untouched fields and numeric spellings remain raw, including gradient
stop metadata. Selection projections include layer inheritance and report mixed
values rather than silently choosing one region. Presets explicitly replace only
the selected memberships' gradient and mark colorStopsCustom false.

Motion validates speed0..50, width100..400 and pause0..60000ms before committing
any of the three values. It is one undo transaction. Cardinal directions are
0/90/180/270. Empty selections do not assign LEDs; invalid inventory, overlapping
selected memberships, unknown presets and unsupported modes refuse without draft
changes. Rotational (including counterclockwisefalse), split, finite and input-
triggered modes remain preserved and are not silently converted to continuous.

The five palettes come from historical Studio module6257.k in
main.88ac681e.js SHA256
`109b483f129044e1c7c17b9fcae0e30fa775885667976807cf2c94bd1c696d19`.
They are distinct from Spectrum's five presets. The test fixture was independently
extracted from the cached source literal and checks every stop/colour. Source
build timing and native-renderer limits are described in [Wave](STUDIO-WAVE.md).
No additional native binary analysis or vendor-code execution was needed for this
regional editor operation.

Core checks cover preset data, layer inheritance, mixed selection, partitioning,
unassigned defaults, partner membership and opaque numeric preservation, atomic
motion history and invalid/unverified refusals. The isolated real-window workflow
loads the private stock export, adds Wave, selects a source LED, chooses Left and
solid-green preset5, edits motion to0/125/79, previews green, saves, undoes/redoes,
rejects width99 atomically and verifies read-only direction is inert. It makes no
simulated device requests and leaves original export/device profile bytes unchanged.

The app-owned render was inspected: controls fit the right panel and the selected
LED previews green. It omits native child EDIT painting and is not a desktop
screenshot. A Computer Use capture located the isolated window but returned
**Computer Use app approval timed out**; no desktop screenshot was obtained.
The held window completed its test and closed normally. This is separate from a
sandbox auto-review rejection. Existing Studio canvas/chassis/key labels and stock
palette/widget layout still visibly differ; exact visual acceptance remains open.

Deferred next checks: source stock Wave panel capture and widget measurements;
stock gradient popup/drag handles; narrow-width unsigned index instructions; arbitrary
angles and split/rotational/finite/input-triggered modes; actual optical output and
stock import of newly authored Wave. Proceed with independent effects/device work
instead of repeatedly waiting on those variants. The full implementation goal and
complete-build memory/network/profile/input/device gates remain active.
