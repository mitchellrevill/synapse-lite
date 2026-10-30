# Studio viewport navigation

Navigation changes only transient Canvas state; composition bytes, history, LED
membership and device profiles remain unchanged. It issues no lighting commands.

- Minus/plus zoom about the visible canvas center; FIT ALL resets pan and fits all
  admitted placements. The center excludes the 50-pixel bottom control strip.
- Numeric zoom accepts 5â€“400 percent, with an optional percent sign; SET % or Enter
  applies it. 50%, 100% and 200% presets appear when the viewport has room.
- FIT SELECTED fits complete placements containing selected LEDs. With no selected
  placements it leaves the view unchanged. This follows the recovered device-fit
  concept; it does not crop a device to the selected LED bounds.
- Ctrl+wheel keeps the world point beneath the pointer fixed, including fractional
  wheel detents and the zoom limits. Wheel input is limited to the canvas rectangle.
- Wheel scrolls vertically; Shift+wheel scrolls horizontally. Space+left drag pans
  with mouse capture, including release outside the canvas. Capture loss/cancellation
  restores the pan origin without changing the LED selection.
- NAMES and Ctrl+L toggle placement labels. Names come from productName or the local
  preset index; unidentified placements retain their explicit placement identity.
- Ctrl+F sets 100%; Ctrl+Shift+F fits all, matching the recovered shortcut handler.
  Studio shortcuts respect focus in every native edit field.

## Source and visual evidence

The historical inventory is supplemented by a fresh inert analysis of the shipped
EditorCanvas.21953e06.chunk.js (91,019 bytes, SHA256
190a2ef92615861142a1585b6cbffa578d2a716069f8d430cb6bd63ca103e5c6).
REA parent ev_28791fd52dd66ac0d0dc276c64bd4b1553af1e08caa53cfe8dea6774463cdcb7;
summary ev_9c18223bd71ed9b4d89a27cdda2e62c45dd3886d7f9155b282f0e049efcb1ab8.
One file parsed with zero parse failures. Dynamic bindings and 6,021 semantic
unknowns remain; static observations do not establish live stock execution.

Observed handlers expose selected/all/specific-device fit, numeric zoom, the
Ctrl+F/Ctrl+Shift+F shortcuts, Ctrl+L and the documented pan/wheel gestures.
The archived stock-chroma-studio-0.jpg from 27 October shows compact bottom-right
zoom controls at 160%, and an icon-based label toggle. Lite retains its existing
native shell and uses labeled buttons, a numeric field and a selected-device fit
button. It does not yet reproduce the stock icon dropdown or specific-device fit
submenu. This is functional navigation work, not a pixel-parity claim.

## Validation

The focused Rust tests cover anchored zoom, bounds/invalid numbers, all versus
selected-device fit, empty selection, pan/scroll transforms, quarter-turn inverses,
selection preservation, and source SVG hit testing after physical-pixel rounding
at display scales 1, 1.25, 1.5, 2 and 2.5 and zoom 5, 50, 100, 200 and 400 percent.

The bounded native simulator test also exercises Space-drag/release/cancel,
Ctrl-wheel anchoring after screen-to-client conversion, Shift-wheel, numeric zoom,
labels, fit selected and existing selection/save/undo/exit recovery. Its synthetic
keyboard state belongs only to the test UI thread and is restored by a guard; it
injects no global input and uses no real devices. Physical multi-monitor DPI
transitions and live hardware are outside this validation.

Final checks: 324 regular tray tests passed (57 opt-in tests ignored), six focused
canvas tests passed (two private pack tests ignored), and the bounded native test
passed with normal desktop access at 125% scaling. The isolated jobs=1 Rust test
build succeeded. Screenshot review moved the selection/zoom status below Add
Device, shortened the name-toggle caption, and included the numeric child field
in the offscreen capture. See validation/studio-viewport.json and
validation/images/studio-viewport.png. No release package or whole-workspace
build was required for this feature; hardware tests were not enabled.
