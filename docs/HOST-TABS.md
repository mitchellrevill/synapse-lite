# Shared host tab surface

28 October 2026. This implements the visual surface, overflow navigation and
device-view close/reopen/session portion of inventory stage1. Drag reordering,
module pages and final visual acceptance remain incomplete.

The native header now uses one `host_tabs` component for Home and every device,
replacing the fixed178/214px blocks. It uses title/icon-based92..242px widths,
34px height at y8 in the42px host,5px rounded upper corners,1px selected green
gradient edge, source hover gradient, Roboto12 weight300,20px product icons,
clipped labels and the source45px title fade. Transport suffixes stay in the
device view instead of the host tab. The Synapse, mouse and keyboard icons are
the recovered favicons, rasterized with alpha at3x density, not screenshots.

The main tab is pinned and cannot close. A clipped device viewport retains the source reserve
for dragging and144px window controls. Overflow arrows move to the preceding
tab edge or next obscured tab's right edge plus6px; wheel movement scrolls
horizontally. Selection and resize reveal the active tab. Hit areas are clipped
with the viewport. Disabled arrows do not dispatch actions. Scrolling does not
cancel an assignment draft; device selection retains the existing dirty-draft
transition gate. Every device page receives the same hover/pressed/leave
routing before its own pointer handler. This path queues no device operation.

## Evidence and reproduction

Original extracted host sources are private local evidence outside the repo:

| Source | SHA256 |
|---|---|
| `work/app-analysis/electron/components/Tab/TabUI.js` | `c94c67c427d8c417a787c7ec735412e8dd9bb34600050849d83b0838bc55d90b` |
| `work/synapse-renderer/electron_index.css` | `30e429ff6289bd694e8c519a73ef42f7cb181e11c8687d44fe59893ecf95792c` |
| `work/app-analysis/electron/components/Tab/common.js` | `7262ee43326e3960b0159547909282434421aba1cb000c01a6a96fda068f0532` |

REA parent `ev_f5d0ae84cd9569f21e46bed18474e9ab4cae411fb4a69b84889765bc0e344ec7`,
TabUI module view `ev_5568d6dba4505af6f9e7a58df4e6a95de67894b7368d8f73ce9e3fa968dc8ac9`.
Seven JavaScript files parsed without parse errors. The semantic graph is
partial with6,060 unknowns. Source rules are static observations, not proof of
every runtime path or product-specific close eligibility.

Useful TabUI byte locations: title overflow7698, active reveal20823,
drag switch~22200/move22887/down23644, viewport24530. Common.js3390..4570
contains the stock keyboard shortcuts. CSS defines the geometry and gradients.

`tools/provision-host-assets.cjs` verifies eleven exact local SVG source hashes
before producing private icons through `provision-window-assets.cjs`.
Its arguments are the recovered `apps.razer.com` cache, asset output directory
and installed Sharp module path. It also expects sibling `icons/raw` arrow
extractions from the same evidence tree. It performs no downloads or vendor
JavaScript execution. Keep generated assets outside public source commits.

Validation with the repository's offline dependency cache:

```
cargo test -p secbird-tray --offline host_tabs::tests -- --test-threads=1
cargo test -p secbird-tray --offline native_assignment_mouse_labels -- --ignored --test-threads=1
cargo test -p secbird-tray --offline native_full_client_bounds -- --ignored --test-threads=1
cargo build -p secbird-tray --release --offline
```

Native tests use `SYNAPSE_LITE_ASSETS` pointing at provisioned private assets,
isolated temporary profiles and simulated transport with physical discovery
disabled. They run through a real HWND and Win32 messages. This is not physical
input injection or real-device acceptance. The mouse test now navigates the
shared host hit map, checks hover/pressed/leave state, returns to Customize,
checks its existing assignment workflow, and verifies zero added simulated
commands and unchanged profile bytes. The frame test covers resize, caption
and control hit zones, maximize and restore. Three unit checks cover names,
overflow geometry and real GDI+ selected/hover pixels at125% scaling.

## Remaining acceptance

- Full pixel equality is unverified. Titles now use authenticated natural
  Roboto12/300, the measured source baseline, fractional painting/clips and
  rounded slot positions. See the third-role evidence in `TEXT-METRICS.md`.
  The gradient
  start is extended by one physical pixel to avoid GDI+ wrapping its first
  antialiased row; normalized browser comparisons must quantify the residual.
- Smooth scroll/200ms tab animation, tooltip and drag reordering remain open.
  Source arrow default/hover/pressed/disabled art is now provisioned and routed.
- Device tabs now represent open views. Ordinary close/reopen is implemented
  for the installed device families, gated by the current assignment draft.
  Module-specific close/force-focus policy must be integrated when those
  modules exist; no general close-policy parity for every stock workflow is claimed.
- Multi-device overflow is covered by layout tests, not a multi-device native
  runtime fixture. Native navigation uses one simulated mouse.
- This adds no profile migration, Calibration/Advanced, macro or Studio feature.
  Inactive frame appearance, other controls/text and complete-build memory,
  privacy, compatibility and hardware acceptance remain inventory work.

## Device view lifecycle and persistence

Closing a device tab now closes only the view. Its dashboard card, physical
device and profiles remain available. A discovery refresh does not reopen a
user-closed tab; clicking its dashboard card explicitly reopens it. A temporary
disconnect does not count as user closure. Closing the active view selects its
right neighbor, then its left neighbor if last, otherwise Home. Closing an
inactive view preserves selection and the current draft. Switching between
open views retains each view's selected device section; closing a view resets
that in-memory section cache. Clicking the already-active tab is a no-op.

The active hovered tab reveals the recovered24px close control, including its
hover/pressed artwork. Middle click also closes a device view. Ctrl1..9 select
position (Home=1); CtrlTab/CtrlShiftTab cycle; CtrlW requests active close;
CtrlShiftT reopens the latest eligible closed view; CtrlShiftPageUp/PageDown
move an active device tab within its current visible order. Home remains pinned.
The message-loop shortcut handler also works while a native text field owns
focus, but leaves the custom profile import/export modal alone.

The `Session` module persists order and user-closed IDs in the fork's
`tabs.json`, alongside but separate from profile data. Writes use the existing
flushed temporary-file replacement helper; file reads are bounded to128KiB,
lists to128 entries and each ID to512 bytes. Unknown JSON fields survive writes.
Unsupported-version, malformed or oversized existing files are retained and
are not overwritten; save failures use the existing visible error path.
Read-only measurement modes skip persistence. The close history is session
memory, not a copied stock localStorage file. Identity currently follows Lite
device keys; wired/wireless identity unification remains a compatibility task.

Source recovery corrected the initial assumption that connected views must
stay permanently open. Host `TabStore.js` defaults `manualCloseViews=[]`; its
SHA is `6fdba648e551a66cd906428de87bac7d9dc7b3fee0c868f22c8af7c1a03b4ab2`.
`TabManager.js` SHA `52e68cd707de5504d0ee1ef053a2a796b2b0b846e2a004870f73100c0f5dd36f`
handles close requests around byte3301 and intercepts manual-close/force-focus
views. Bounded196/605 scans found wrapper definitions, not direct activation;
that negative scan is not proof against computed or runtime activation.

Dashboard `App.70b4a4e6.chunk.js` module28648 around bytes3830..8400 implements
`synapse-tab-orders` and `synapse-closed-tab`, normalizing device window names,
separating automatic removal from user close, and allowing explicit userOpen
to bypass closed state. SHA
`b6cd72ceea9ad59cadd024c4d8cee836b6816389008e4cee9d0ea3c4f2a267cc`.
Device module39709 around byte68700 uses autoOpen for discovery and userOpen
for card activation. These are inert shipped-source observations, not live
vendor import, account or hardware operations.

Six model/renderer tests pass. Native mouse tests cover actual close hit targets,
middle close, retained Performance section, closed-state persistence, card reopen
and shortcut mapper close/reopen. Native keyboard tests prove dirty close is
deferred, dismiss preserves the draft, and explicit discard closes only the
view. They retain exact profile bytes and add zero simulated transport commands.
The keyboard regression uses process-private fonts outside the restricted runner.
Shortcuts are exercised with simulated modifier facts through the real mapper
and action path, not OS keyboard injection. Reordering/reload are model/file
tests; multi-view native dragging and OS chord acceptance remain unverified.
See `validation/host-tab-lifecycle.json` for the recorded results.
