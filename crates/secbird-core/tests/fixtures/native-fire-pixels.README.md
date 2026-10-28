# Native Fire and pixel reconstruction vectors

`native_fire_vectors.json`, `native_pixel_palette.json` and
`native_pixel_blend.json` are deterministic, independently written Python
reference results derived from the installed DLL's static instructions. They
are not observations from loading or executing vendor code. Digest comparisons
use MD5 solely as compact byte-equality checks for these non-adversarial test
vectors, not as a security primitive.

Target SHA256 and REA evidence are recorded in docs/FIRE-LIGHTING.md and
docs/LIGHTING-PIXELS.md. The private reference generator is
work/studio-render-contract/derive_vectors.py. It decodes the independently
recovered cooling/mask data, generates grayscale keyframes, applies four-channel
interpolation, maps regions and hashes all output bytes. It contains no vendor
implementation text and does not import the Rust implementation.

Fire hashes cover every frame/pixel/control byte in each complete500-frame
cycle. Nine cases include both region branch thresholds, cropping, scaling,
tiling, offsets, None/fallback and explicit CRT streams. Blending vectors use
explicit IEEE f32 roundtrips after each native multiply/divide and signed
truncation. Palette vectors preserve the native one-slot final-segment quirk.

The existing native-spectrum-static-derived.rgb remains a separate earlier
full943-frame ordinary-RGB fixture and also validates the new pixel palette.
