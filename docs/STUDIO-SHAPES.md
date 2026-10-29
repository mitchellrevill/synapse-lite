# Studio source basic shapes

The private preset pack now contains1268 layouts across177 products, expanding
from977 layouts/117products. All previous977 layouts remain available. New shape
admission adds291 layouts and60 products; source availability remains separate
from device API/physical verification. The pack totals15538730bytes on disk.

The provisioner now handles circle, ellipse, rounded rectangle, polygon and filled
polyline selection elements as well as supported paths and plain rectangles.
Source coordinates, explicit LED identities, manifest/SVG hashes and edition/layout
matching remain in use. Circles/ellipses and rounded rectangles are stored as typed
numeric primitives. Native rendering records Ellipse/RoundRect in GDI path brackets;
click hit tests use ellipse equations and clamped corner radii. Polygon/polyline
vertices become straight path segments, with the source's closing behaviour.

Rectangle radius defaults and half-width/half-height clamping follow the
[SVG basic-shape rules](https://www.w3.org/TR/SVG/shapes.html).
Native path construction uses the documented
[GDI path operations](https://learn.microsoft.com/en-us/windows/win32/gdi/path-creation).
The parser refuses conflicting path/primitive definitions, invalid/nonfinite
values, negative radii, non-rendering ellipses and excessive coordinates. Literal
numeric attributes are supported. Selection uses geometric interiors; source CSS
stroke-only/pointer-event parity remains a TODO. Relative units/CSS geometry, transforms, nested
SVG viewports, untraced path commands and ambiguous source definitions remain
TODOs. Line-only polylines are refused pending stroke-aware hit testing.

Every generated layout passed the native parser with its exact product/edition/
layout and LED count. The corpus contains70454 path entries,9697 rounded rectangles
and295 ellipses across its regional/edition variants. Largest estimated owned
layout capacity is383506bytes. Existing on-demand loading, shared geometry,
32-placement,4MiB unique-source and8MiB estimated-owned-capacity limits remain.
None of these figures proves the complete-build100MB requirement.

## Verification

Six Python tests cover literal paths/rectangles, curved-shape attributes and SVG
radius defaults, polygon/polyline closure, unsupported styles/transforms, hash
admission and conflicting definitions. Three new regular native tests check
analytic shape boundaries, transformed exact bounds, invalid/conflicting input and
GDI fill regions with rounded corners excluded. The full workspace passes468core,
303tray,4CLI and1doc check; six core and45tray checks remain intentionally ignored.

The optimized full1268-layout corpus check and eleven native Studio checks pass.
The new source620/edition0/layout1 keyboard check loads106 selectable source LEDs,
selects its circular multimedia region, saves Static RGBABCDEF and verifies
Undo/Redo, unchanged source/profile bytes and zero device requests. It uses
simulated hardware and leaves unsupported preview/output inert. The native capture
was reviewed; its circular region and selection outline are visible. Chassis,
legends and matched-state stock pixel comparison remain unfinished.

The extra Customize regression first encountered a restricted font-loading
limitation: the keyboard check loaded zero private font resources and refused its
font-baseline assertion; the mouse check passed. Both checks then passed with
normal font access using simulated devices and temporary profiles. No product font
or approved tab change was made for that environment-specific failure.

The private index retains908 source/layout TODO records, including missing exact
SVGs, transforms, untraced path commands, identity conflicts and invalid source
resources. These are not908 distinct devices. Continue independent work when a
case stalls; do not infer unsupported geometry or enable device output merely
because a preset is available. See STUDIO-PRESETS.md and
validation/studio-basic-shapes.json. Full device APIs, effects, input/macros,
profile/source migration, privacy, visual and three complete-build memory gates
remain open.
