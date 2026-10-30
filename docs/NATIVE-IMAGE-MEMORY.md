# Native image memory repair

The host Macro SVG was rasterized at its intrinsic 1024 pixel size multiplied by
three. The resulting 3072 × 3072 BGRA image occupied 37,748,736 decoded bytes,
despite being displayed at about 20 CSS pixels. Chroma occupied another 2,359,296
decoded bytes at 768 × 768. Both were loaded when the native editor was created.

The asset provisioner now uses a display-role budget. Host icons retain up to
96 pixels on their longest edge (32 CSS pixels at 3x). The 60 CSS pixel game
placeholder has a separate 180 pixel budget. Small icons retain their existing
3x raster density. The SVG renderer's density is bounded before decoding, and a
final aspect-preserving resize enforces the output budget. Unrelated device
artwork retains its original resolution.

Exactly two runtime rasters change in the current package: Macro and Chroma,
both to 96 × 96. This removes 40,034,304 decoded pixel bytes. SVG sources, other
icon pixels and device artwork are unchanged. The same executable with the new
assets gave a first Dashboard peak of 42,356,736 working-set bytes and 19,976,192
sampled private bytes, compared with the baseline's approximately 78.8/60–62 MB.
Repeated final-build measurements are recorded separately; this first check is
not the complete-build memory acceptance result.

Bitmap ownership and loading now live in `native_editor/bitmap.rs`, outside the
large editor module. That module validates file identity, dimensions and exact
pixel length before allocating a native surface. It preserves premultiplied
alpha validation, BMP fallback and the existing format limits. The GDI+ view
borrows the DIB pixels and is destroyed before that backing allocation. No
profile or device behaviour changes in this refactor.

The CLI regression decodes actual SVG fixtures through Sharp and checks output
budgets, aspect ratios, alpha, premultiplied BGRA and full-size device artwork.
The Rust regression checks malformed and truncated pixel layouts before native
allocation. Native Macro checks use both asset packs, and Studio entry, effect
palette and visibility checks use the new pack. The ordinary app suite and
strict Clippy cover the bitmap extraction.

The native Macro captures use the same private fixture at 125% scaling. The
27 × 28 tab icon crop differs by at most 5/255 per channel, with mean absolute
channel difference 0.279; the 27 × 27 menu crop has maximum 5/255 and mean 0.301.
These are small rasterization differences, not a claim of full visual parity
with stock Synapse. The original/current captures are retained with the runtime
evidence.

Historical 10–11 MB numbers in `ACTIVE-LIGHTING-RUNTIME.md` describe sampled
private memory in earlier builds; their working sets were about 24–26 MB. The
new measurements report both metrics and keep the current baseline unchanged.
Active Studio effects, macro playback, long sessions and broad device coverage
remain outside this read-only page benchmark. The complete-build memory goal
and the broader codebase refactor remain unfinished.

To reproduce asset generation, run `provision-window-assets.cjs` against a local
asset directory with the installed Sharp module path. The provenance records
include source/output hashes, actual density and display-role maximum edge.
The generated BGRA files are private build artifacts, not embedded Rust data.

## Repeated final-build results

Same workloads and settings as the 30 October baseline; three repetitions per page. Decimal MB. Startup is warm-cache process creation to the first confirmed presented frame.

| Page | Peak working set, before → after MB | Peak private, before → after MB | Startup median, before → after ms |
| --- | ---: | ---: | ---: |
| chroma | 78.92 → 38.93 | 60.49 → 21.54 | 759 → 770 |
| dashboard | 78.84 → 38.84 | 61.99 → 19.88 | 767 → 770 |
| keyboard-lighting | 76.70 → 35.36 | 58.43 → 18.33 | 750 → 765 |

Working-set peaks fell about 50–54%; sampled private-memory peaks fell about 64–69%. Startup remains roughly 0.75–0.77 seconds; these runs do not establish a startup-speed improvement.

Two oversized icon rasters accounted for 40.03 MB of decoded pixels. Their SVG sources and all other generated icon pixels are unchanged. Bitmap validation and resource ownership were extracted into a dedicated native module.

376 regular app tests, strict Clippy, the real asset-generation CLI test, native Macro checks with both packs, and the native Lighting/Studio palette check pass. Profiles are unchanged. See the native-image-memory documentation for the source and pixel-comparison details.

The complete-build memory goal remains open: active Studio effects, macro playback, remapping load, long sessions and broad device coverage are not covered by these read-only runs.

Raw results, profile integrity, source/log hashes, screenshots and asset sizes are retained in this directory.
