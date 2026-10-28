# Full-client native window frame

The native editor now uses its complete outer window as the client area. At 125% scale, a 1280×720 logical window has 1600×900 physical client pixels, replacing the previous 1584×884 client and 8px inset per side. This matches the installed frameless AppEngine's normal Windows 11 bounds: full window width, with the product view beginning 42 logical pixels below the host header.

`WM_NCCALCSIZE`, `WM_GETMINMAXINFO` and `WM_NCHITTEST` are handled before borrowing editor State. Creation and synchronous Windows sizing callbacks therefore do not depend on a currently available mutable State borrow. The existing resizable/maximizable window styles remain. DPI-specific Windows sizing-frame and padded-border metrics supply all eight resize hit zones; maximized windows disable those resize zones. The 42px host remains draggable wherever an actual control does not occupy it.

Maximum size and position come from the selected monitor's work area, with position relative to that monitor's origin. During maximize calculations, the proposed rectangle selects the monitor, rather than stale cached editor bounds. Oversized maximized client rectangles are clipped to its work area. Smaller restore proposals retain a full client even if the maximized style has not yet cleared. The existing 1000×650 logical minimum is preserved.

## Validation

The new bounded HWND regression uses the simulated keyboard transport. It validates actual window/client dimensions, cached layout reconciliation, the scaled host hit-test boundary, all eight resize hit targets, the close-button target, and the real maximize/restore button action path. Restore must preserve both normal dimensions and position. No simulated command or profile write may result from these frame operations.

Observed at 125% on Windows 11 build 26200:

| Mode | Outer and client pixels | Client inset |
| --- | ---: | ---: |
| Normal |1600×900 |0,0 |
| Minimum |1250×813 |0,0 |
| Narrow |1325×875 |0,0 |
| Maximized work area |2560×1380 |0,0 |
| Restored normal |1600×900 |0,0 |

The functional header boundary is 53 physical pixels, the rounded 42×1.25 source offset. Three pure tests cover all resize zones at negative screen coordinates, monitor-relative maximum bounds, and restore/oversize/disjoint rectangle handling. All seven simulated native regressions pass, including the existing Lighting capture/draft, Power, profile modal, overlay and assignment flows. [Sanitized evidence](validation/full-client-frame.json) records the measured bounds and source references.

This validates window geometry and the tested interactions on the current desktop. Physical multi-monitor moves, DPI transitions, auto-hide taskbars and Windows 10's separate source product-view inset remain unverified. GDI raster parity and the remaining UI content are separate work. No physical devices, stock UI, screenshots or process controls were used; no new RAM claim applies to this change.

## Interactive move/resize correction

Caption dragging and border resizing enter a nested message loop inside `DefWindowProcW`. The editor previously retained its mutable state borrow across that call, preventing nested paint, resize, timer and DPI messages from accessing the editor until the loop ended. The default dispatch and cursor fallback now release the borrow before forwarding to Windows. The DPI handler also releases it before applying the suggested monitor bounds, allowing synchronous resize callbacks to reconcile the layout.

The frame regression now posts real `SC_MOVE` and `SC_SIZE` commands and verifies Windows reports an active move/resize loop. While that loop remains active, it changes position and size three times, checks the cached client dimensions, repaints and confirms interactive hit regions were rebuilt. It also delivers 144-DPI, 192-DPI and original-DPI suggested-bounds notifications inside each loop. The original code fails with editor state unavailable during `SC_MOVE`; the corrected code passes, including the existing maximize/restore and unchanged simulated device/profile assertions. Suggested DPI notifications are simulated; physically dragging across distinct monitors remains unverified.
