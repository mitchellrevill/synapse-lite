# Source Studio transform projection

The offline provisioner now admits 1,396 exact layouts across 183 products from
the pinned 28 October capture, adding 128 layouts without losing any of the
previous 1,268. These are edition/regional layout identities, not 1,396 devices.
The generated layout payloads total 17,963,550 bytes. No private layouts or vendor
artwork are committed, and no installed asset pack is overwritten.

`tools/svg_affine.py` composes literal SVG translate, scale, rotate (including
explicit centers), and matrix attributes in SVG order, including ancestors.
Supported M/L/H/V/C/Z paths are normalized to absolute transformed coordinates.
Ellipses and rounded rectangles retain their analytic primitive representation
for axis-aligned scale/reflection and exact quarter turns. Curved primitives
under other rotations or shear refuse admission instead of becoming approximate
polygons. CSS transforms/geometry, nested viewports, unsupported path commands,
singular/excessive matrices, malformed number lists, unknown identities and
conflicting source definitions continue to produce explicit TODOs.

The native format and renderer are unchanged. Existing lazy loading, exact
product/edition/layout selection, deduplication, placement limits and geometry
budgets still apply. A 1,024-entry bounded transform parsing cache is used only
by the offline Python provisioner. Captured paths now also refuse resolved
filesystem escapes. Projection order and index identity order are deterministic.

## Reproduce with private inputs

Use an absent output directory for each build:

```text
python tools/provision-studio-presets.py REPORT SOURCE_ROOT NEW_PACK --report-sha256 CAPTURE_SHA256
python tools/prepare-studio-comparisons.py SOURCE_ROOT REPORT NEW_PACK PREVIOUS_INDEX NEW_CASE_DIRECTORY
node tools/verify-studio-projection.cjs PLAYWRIGHT_MODULE CHROMIUM_EXECUTABLE NEW_CASE_DIRECTORY/cases.json COMPARISON_REPORT
```

The comparison preparer chooses the first newly admitted layout for each product,
checks manifest and SVG hashes, and writes private cases. Limit each comparison
batch to 32 products. The renderer aborts network requests and reconstructs only
inert geometry; source scripts, URLs, CSS, chassis and legends are not loaded.

Run native admission against an explicitly selected private pack:

```text
SYNAPSE_LITE_STUDIO_PRESETS=NEW_PACK
cargo test -p secbird-tray --locked --offline all_recovered_studio_presets_parse_and_keep_exact_identity -- --ignored
```

On PowerShell set environment variables using `$env:NAME='value'`.

## Evidence and limits

The report SHA256 is
`72518b0b3fdf1812c221d795bbd08b1ee7afd4ffc4f869ad63ecaefd78774cc2`.
Repeated provision runs produced index SHA256
`8e3a5bbfd983a7777da287ead26eabb0d8f09755c05fc9e2eebd93e173cde9d1`.
All 1,396 layouts passed the native parser with exact identities and LED counts;
largest estimated owned geometry capacity remains 383,506 bytes. Seven native
source-layout tests also pass. Python tests cover composition order, relative
cubics, centered quarter turns, reflection, malformed/singular/excessive transforms,
CSS refusal and nested projection, alongside existing hash/conflict checks.

Chromium 151.0.7922.34 compared one layout for each of the 13 products with new
layouts. Maximum per-region bounding-box difference was 0.000064849853515625
source units. Only one of 13 rasters was exactly equal. Across the cases, 1,441
pixels differed among 1,653,638 nonempty source pixels; maximum alpha difference
was 53/255. The complete numerical comparisons are in
`validation/studio-transform-comparison.json`. These measurements verify close
geometric bounds and quantify remaining raster differences; they do not establish
native or stock pixel equality. TODO: isolate the differing rounded-rectangle
edges and compare Chromium curve tessellation before making a stronger claim.

The private index retains 780 source/layout TODO records: 291 missing exact SVGs,
201 unsupported path cases, 116 identity mismatches, 47 duplicate/unmapped LED
cases, 44 non-axis-aligned curved primitives, two stylesheet geometry cases and
other malformed/missing-source cases. These categories overlap products. Next
diagnostics are explicit path-command inventory and an affine native primitive
representation; do not discard curvature, CSS, legends or chassis and claim
complete source rendering. Physical DeviceInfo matching and device APIs remain
separate gates.

REA manifest inventories were retained as
`ev_3d583bf08811dc61d5d1ee6788e73a1b19ecc2a9fd0caa715dd59fbbe28c988e`
(product 605 MW) and
`ev_53f779aafa4c3802b750719f0afb77b5856d778c9a68a3bef6546a156a5e4fd5`
(418 captured fallback manifests). REA observed inert inventory bytes, not SVG
execution or physical behavior. Corpus counts and geometry observations above
come from the hash-verified local projection and explicit browser/native checks.
Transform composition follows the [SVG coordinate specification](https://www.w3.org/TR/SVG/coords.html).
