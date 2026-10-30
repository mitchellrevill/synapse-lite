# Native Fire and pixel reconstruction vectors

`native_fire_vectors.json`, `native_pixel_palette.json` and
`native_pixel_blend.json` are deterministic, independently written Python
reference results derived from the installed DLL's static instructions. They
are not observations from loading or executing vendor code. Digest comparisons
use MD5 solely as compact byte-equality checks for these non-adversarial test
vectors, not as a security primitive.

The reference target is the statically inspected lighting engine DLL, SHA256
`cd9fc2a61ff920b9b73e1f5e27632020e2f8586f443e426a875ad3b042b714fa`.
Private captures and development transcripts are not shipped. These reference
fixtures describe instruction-derived behavior, not vendor execution or optical
parity. See the maintained [Studio status](../../../../docs/studio.md) for the
current output boundary.

Fire hashes cover every frame/pixel/control byte in each complete500-frame
cycle. Nine cases include both region branch thresholds, cropping, scaling,
tiling, offsets, None/fallback and explicit CRT streams. Blending vectors use
explicit IEEE f32 roundtrips after each native multiply/divide and signed
truncation. Palette vectors preserve the native one-slot final-segment quirk.

The existing native-spectrum-static-derived.rgb remains a separate earlier
full943-frame ordinary-RGB fixture and also validates the new pixel palette.

`native_fire_parameters.json` extends Fire's intrinsic grayscale reference to
variable Rate/Seed, using the same table carrier SHA256. Its reproducible
generator is `tools/derive_fire_parameters.py`, accepting the recovered table
JSON and a caller-selected output path. Ten cases hash all161 intensity bytes
for each of4588 complete frames, with explicit initial/final CRT states. These
are static reconstruction vectors, not observations of vendor execution. The
existing nine mapped four-channel cycles continue to test palette/mapping math.
