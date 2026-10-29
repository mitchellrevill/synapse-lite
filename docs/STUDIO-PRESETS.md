# Broader Studio device presets

The native Studio canvas can now load exact product/edition/layout projections
beyond Ornata. The current private pack contains1268 layouts across177 products
from the acquired source catalogue. These are source preset availability, not
verified hardware support, and do not cover every current Razer product. The
source-basic-shape extension is documented in STUDIO-SHAPES.md.

Imported compositions can display their matching preset, select its explicit LED
memberships and edit compatible regional data through existing controls. A native
Basilisk100/edition0/layout0 check selects a real source scroll-wheel LED, saves
Static RGBABCDEF, and verifies Undo/Redo without device requests. Offline editing
preserves source composition and native device profile bytes. Adding arbitrary
devices from a preset chooser is not yet implemented; the placement must already
exist in the opened composition with explicit identity, dimensions and LED inventory.

## Acquisition and projection

Run tools/provision-studio-presets.py REPORT SOURCE_ROOT NEW_OUTPUT
--report-sha256 EXPECTED_SHA against a local lighting capture. It verifies the
capture report and each manifest's recorded length/SHA256 before projecting data.
The currently used report SHA256 is
72518b0b3fdf1812c221d795bbd08b1ee7afd4ffc4f869ad63ecaefd78774cc2.
No network, vendor JavaScript or native DLL runs. The new output directory must
not exist; it does not replace an installed pack. Generated assets remain private
build data, outside Git. The public tool and synthetic tests are included.

Projection keeps literal supported selection paths and their row/column identity.
Plain SVG rectangles and polygon/polyline vertices preserve their straight edges;
circles/ellipses and rounded rectangles use native numeric primitives. Transforms,
untraced path commands and missing exact SVGs remain TODOs. A layout is never substituted from another product, edition or locale.
Two conflicting source definitions remove the candidate rather than selecting an
arbitrary winner. Matrix membership, explicit viewBox and optional configuration
identity fields must agree. Existing source hashes remain in every projection.

The pack index records908 per-layout/source TODOs, including repeated missing
layouts in edition manifests. These are not908 distinct devices. The1268 generated
files total15538730bytes on disk. The complete index and per-resource TODOs remain
with the private pack; a redacted summary is in validation/studio-basic-shapes.json.

Source EditorCanvas.21953e06.chunk.js SHA256
190a2ef92615861142a1585b6cbffa578d2a716069f8d430cb6bd63ca103e5c6 supplies
the existing generic placement/selection model. A focused REA inspection retains
one91019-byte unexecuted chunk,28168ASTnodes, zero parse failures and6021semantic
unknowns. This static evidence does not prove current live frontend execution.

## Runtime admission and memory

The canvas resolves only studio-presets/studio-PRODUCT-EDITION-LAYOUT.json, with
legacy root-path support for prior local projections. A present wrong-identity
pack file refuses; it cannot fall back to another projection. Product, edition
and layout must match the composition exactly. Existing bounds of32 placements,
512 LED references, finite geometry and quarter-turn orientation remain in place.
Only source paths present in the composition's explicit LED inventory are selectable.

Geometry loads on demand, not by loading the1268-layout index. Equal identities
share one parsed Arc across placements. A canvas admits at most4MiB of unique
serialized geometry and8MiB of estimated owned vector/string capacity. This
estimate excludes allocator/GDI overhead and the rest of the app; parsing peak
and the full-process100MB gate are not established by these local bounds. Native
corpus validation found a largest owned geometry capacity of383506bytes; all1268
files parsed with exact IDs. The whole pack is never retained as parsed geometry.

Local Preview and Apply Saved keep the existing single unrotated Ornata backend
admission. Other presets and multi-device canvases support offline authoring only;
visual availability does not enable an unimplemented device API or output route.

## Verification and remaining work

Six Python checks cover literal paths/rectangles, unsupported shapes/transforms,
missing SVG/matrix identity, report/manifest hashes and conflicting definitions.
Two regular native tests cover foreign selection, wrong-identity refusal, shared
geometry across32 placements and the placement limit. The opt-in corpus check
loads every generated layout. A native window check verifies Basilisk LED selection,
colour save, Undo/Redo, unchanged source/profiles, inert unsupported preview/output
and zero device requests with physical discovery disabled.

The captured native view was inspected. It draws LED selection paths, not the
complete device chassis. Chassis/legends/background, stock hover/fill/control
states, remaining shape types, transforms and matched-DPI full visual comparison
remain TODOs. No pixel-perfect or physical lighting claim follows. Existing
Ornata and actual-export Studio checks remain required regression coverage.

Workspace468core/303tray/4CLI/1doc checks pass; six core and45tray checks remain
intentionally ignored. Full APIs, profiles/macros/input, remaining effects,
source migration, privacy and three complete-build memory runs remain open.

The optimized workspace build, full native preset-corpus check and all eleven native
Studio checks pass on simulated hardware. These checks do not establish device
APIs or optical output for the broader preset pack.
