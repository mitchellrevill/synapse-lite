# Targeted rendering corrections — 27 October 2026

The broader implementation goal remains paused. This patch addresses the user's specific artwork, icon and inactive-frame complaints only.

- Preserve source artwork alpha in a bounded, top-down premultiplied BGRA sidecar. Legacy BMP assets remain supported. The loader rejects malformed dimensions, lengths and non-premultiplied channels.
- Keep close/restore glyphs around 13 logical pixels, centered inside their existing hit areas. Minimize/maximize SVGs already contain a 48×32 transparent cell.
- Reprovision available local shell SVGs at 3x density, including Refresh. Transparent images now use Windows GDI+ high-quality bicubic filtering at the actual output size. AlphaBlend overrides HALFTONE with COLORONCOLOR, so the previous alpha path could still look jagged. See [Microsoft's AlphaBlend remarks](https://learn.microsoft.com/en-us/windows/win32/api/wingdi/nf-wingdi-alphablend).
- Suppress default non-client painting on the full-client custom window and its inactive transition. Request no DWM border on Windows 11; keep the sizing style, hit regions and maximize/restore calculations. See [DWM window attributes](https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/ne-dwmapi-dwmwindowattribute).

Validation: offline release build passes; the GDI+ fractional-scale pixel test passes and verifies intermediate alpha edges over a dark background; all three existing frame geometry tests pass. All 21 generated alpha assets validate, and the three product artworks match premultiplied source pixels. A five-second read-only dashboard smoke with two simulated devices reports presented/matched, no failure, and lighting worker disabled. No physical device operation was used as a validator for this rendering patch.

The process-tree memory script could not access CIM in the sandbox, so it produced no valid resource acceptance result. No final-build RAM claim follows from this patch. A live active/inactive screenshot comparison remains outstanding, as does full pixel identity. Fonts/text, dotted background, tab styling, BMP-only assets, features and telemetry acceptance are outside this patch. The earlier natural-text prototype remains unintegrated and uncommitted.

Local vendor artwork/fonts and generated images remain excluded from Git. A local preview must include reprovisioned assets; swapping only the executable into an old assets directory will retain the old low-resolution images.
