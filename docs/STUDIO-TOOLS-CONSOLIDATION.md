# Studio tools consolidation, 30 October 2026

This integrates seven existing, completed local patches over the explicitly
requested `41b6c85` snapshot. It does not merge root/main or later root work.
The isolated worktree is `C:/Users/mitch/Documents/Codex/synapse-integrate-studio-tools`,
branch `integration/studio-tools-2026-10-30`. The canonical checkout is untouched.

Included commits: gradient `f6d66ce`, layer organization `f165fc2`, composition
library `c88dd30`, LED property clipboard `79171f8`, viewport `8277747`, quick
selection `9c03c00`, layout remapping `0d940b3`. Their feature documentation and
source-preservation tests remain in this tree. They form one verified local
consolidation commit rather than seven partially resolved integration commits.

## Semantic conflict resolution

- Quick selections and Remap originally both posted dialog4, already used by
  composition Delete. Library operations retain4..7; Quick uses8 and Remap9.
  The actual dispatch function rejects unknown kinds and has a regression check.
- Viewport navigation and remap fixtures both used Studio test operation10.
  Named constants now assign navigation10 and remap11, including their callers.
- The pointer entry retains property menus, gradient handles, and layer dragging
  with its wparam. Library popovers consume their click sequence before canvas
  hit testing, so device LEDs underneath cannot swallow Duplicate/Delete clicks.
- Opening a property context menu, library operation, quick selection or remap
  cancels transient captures/gestures; completed gradient/layer transactions and
  viewport navigation retain their own existing history/cancellation behavior.
- Clipboard native fixture coordinates use the new viewport center. The library
  regression now starts with actual canvas geometry, opens/cancels Quick, then
  exercises library actions with a populated canvas rather than only empty views.
- Studio startup admission is bounded separately at15 seconds for cold source
  asset loading. All existing action-response checks remain at four seconds.

## Validation

Serial focused Studio core:117 passed,0 failed. Serial focused tray:69 passed,
0 failed,24 existing optional checks ignored. Nine explicit native tests then
passed serially with simulated hardware: composition library (including Quick
cancel/populated canvas), layer organization, property clipboard, remap cancel/
commit/save/history, viewport and save-failure recovery, Wave gradient, Breathing,
Audio Meter, and Starlight. These authoring tests preserve source/profile bytes
where asserted and issue zero device requests. Native source discovery is not
physical hardware discovery.

A first cold-start attempt exceeded the old four-second startup wait. A separate
restricted-temp run failed with Access Denied; final checks used workspace-owned
TEMP/TMP with desktop access. Neither error was hidden or counted as a pass.
The final native production-painter gradient and library menu captures were
visually inspected. They show the combined header/tool controls; child edit
fields are verified through their native test paths. Exact stock styling remains
unproven and was not expanded into another UI redesign.

The one-job release workspace build passed. Final binary hashes are recorded
in `validation/studio-tools-consolidation.json`. Existing unused-code warnings
and filesystem incremental-cache hard-link warnings remain.

## Root merge and remaining work

Root reported later lazy-assets `6342e04` and bounded Identify `5b13094` changes
also touch `studio_canvas.rs` and `studio_view.rs`. They were deliberately not
merged into this branch. Root should preserve those features when resolving the
final overlap with this Studio branch. Native routing also touches
`native_editor.rs`; all seven features' native tests and fixture commands must
remain present after that final merge.

Existing per-feature limits remain: stock popup/icon/layout parity, remembered
custom picker colours and narrow-panel scrolling; interactive stock committed
re-import, physical/optical lighting and complete-build memory acceptance. No
stock app, other Lite process, driver, firmware or physical hardware test was
stopped or changed. No push or new chat was created.
