# Source-defined Customize background

The native Customize scene now draws the recovered dot field before the device artwork, labels and controls. Both installed product renderers explicitly create `config-wrapper dot-bg` with a `dim-corner` child. Their wrapper has a 340 CSS pixel height and 770–1220 pixel width. The existing source-derived scene center, drawer offset and scroll coordinates also position the background; it is clipped below the device navigation.

The source uses two perpendicular 22px repeating gradients, each masking its first 20px with #222, over #5d5d5d. A transparent-to-#222 radial ellipse fades the field toward its corners. The native renderer computes the corresponding per-pixel coverage and fade, including fractional physical tile rounding observed at 125%. It caches one physical-size/phase raster, releases it before resizing and keeps GDI handles owned until teardown. The source mouse DPI-button artwork variant is also provisioned with alpha so hovering that input does not erase the field with an opaque rectangle.

The cache's maximum allocation is one 4881×1361 BGRA surface (26,572,164 bytes) at the supported upper scale bound; the usual 1220×340 view at 125% uses 2,592,500 pixel bytes. Small temporary horizontal coverage data is additional. These are local storage bounds, not a complete RAM measurement or final-build acceptance.

Validation:

- Independent network-blocked Edge rendering of the actual recovered CSS compared against Rust-generated pixels at widths 770/1000/1220 and scales 100/125/150/200%. No vendor JavaScript or personal browser profile was used. See [the comparison evidence](validation/customize-dot-background.json).
- Eleven fixtures have a maximum channel difference of one or two integer values. The 770px/125% fixture retains an eleven-value maximum difference on one edge column (425 pixels). Browser dithering/edge compositing and exact fractional placement remain unresolved. This is **not pixel-identical acceptance**.
- A real GDI cache test draws visible source-color pixels, repeatedly resizes and restores the raster, and returns the process's GDI object count to its initial value after dropping the cache.
- Native simulated mouse and keyboard Customize interaction regressions pass. The mouse test verifies the background survives opening its drawer; the existing profile/draft isolation and no-SET checks remain in place. No physical device operation is part of these validators.

The comparison validates the isolated field. Full stock/native scene comparisons, unusual fractional origins, remaining text, tab styling, device functions, profiles and Studio are separate unfinished inventory items. No overall visual or resource completion is claimed.
