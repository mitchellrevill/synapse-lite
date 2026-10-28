# Shared host tab surface

28 October 2026. This implements the visual surface and overflow navigation
portion of inventory stage1. It does not complete the tab workflow or stage1.

The native header now uses one `host_tabs` component for Home and every device,
replacing the fixed178/214px blocks. It uses title/icon-based92..242px widths,
34px height at y8 in the42px host,5px rounded upper corners,1px selected green
gradient edge, source hover gradient, Roboto12 weight300,20px product icons,
clipped labels and the source45px title fade. Transport suffixes stay in the
device view instead of the host tab. The Synapse, mouse and keyboard icons are
the recovered favicons, rasterized with alpha at3x density, not screenshots.

The main tab is pinned. A clipped device viewport retains the source reserve
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

`tools/provision-host-assets.cjs` verifies five exact local SVG source hashes
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
- Source arrow hover/pressed/disabled art, smooth scroll/200ms tab animation,
  tooltip, close eligibility/closed stack, drag reordering, session order and
  global keyboard shortcuts remain open. The current arrows use original
  default SVG artwork for every visual state; disabled actions are gated.
- Device tabs still represent connected devices, as in the previous Lite
  header. No new close/unpair/remove behavior is invented. Product196/605
  manual-close activation remains unresolved; the source supports both states.
- Multi-device overflow is covered by layout tests, not a multi-device native
  runtime fixture. Native navigation uses one simulated mouse.
- This adds no profile migration, Calibration/Advanced, macro or Studio feature.
  Inactive frame appearance, other controls/text and complete-build memory,
  privacy, compatibility and hardware acceptance remain inventory work.
