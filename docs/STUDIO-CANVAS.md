# Studio canvas geometry and selection

The native Studio file editor now draws Ornata LED outlines from the shipped
Studio SVG rather than reusing the differently scaled Customize geometry.
Single click replaces selection, Ctrl-click toggles it, and drag selects LED
bounds. Canceling a drag restores the prior selection. Zoom/Fit and navigation
do not dirty a composition. Selection is transient;
[Static property controls](STUDIO-REGIONS.md) can now assign/erase its region
memberships. These local operations do not submit lighting frames.

## Data and source boundary

`tools/provision-studio-layouts.py MANIFEST PRIVATE_ASSET_DIRECTORY` hashes the
whole local manifest against SHA256
`3595e11f1d8778180d1dd6edd2463c8b3b8e446542c5020dc1fa038c67937a93`,
then projects literal `.selection` paths and row/column identities. No vendor
JavaScript runs. Generated paths/assets remain private and outside Git. Sixteen
exact layouts exist:1–8,10–12,15–18,20. Layout9 has a matrix but no matching SVG;
no fallback is inferred. Five matrix entries have no selection path in most
layouts; layout12 has six. No shapes are fabricated for them.

Placement admission currently requires explicit `id`, `productId:605`,
`editionId:0`, `layout`, finite x/y/width/height, quarter-turn `orientation`
(optional zero), and `ledRef` row/column membership. At most32 placements and512
LED references per placement are accepted. Unknown or incomplete placements
are retained in the file and reported unavailable for the canvas. This narrow
admission is not proof that every actual stock export uses these identity fields;
live device reconciliation and actual user composition acceptance remain open.
No historical UK identity or operating-system locale supplies missing fields.

Identical layouts share parsed geometry. Native paths use the same coordinate
transform as hit testing, including quarter turns and Windows display scaling.
The provisioner checks source identity; runtime reads the private projection
and checks its declared identity. It does not reauthenticate the original
manifest on every paint. The ordinary local asset directory is trusted.

Source Studio `EditorCanvas.21953e06.chunk.js`, SHA256
`190a2ef92615861142a1585b6cbffa578d2a716069f8d430cb6bd63ca103e5c6`,
uses device x/y plus width/height/orientation, with rotation origin0,0 and
quarter-turn margins (around character1872–5300). Selection is keyed by
device identity and data-row/data-col (around27098). The canvas background and
selection strokes are source-derived; Move-mode grid is not shown in this
Select-only checkpoint. REA refreshed the unchanged cached tree: parent
`ev_4d318d05da1dda4a8d9034c2a8ef8da573575b62afb734df5ecb4d09ec54d934`,
summary `ev_eea295d06680c79794a99e4a1513601666a0dd317b424c5ac587e5f2b734c83a`.
Forty JavaScript files parsed without parse failures; application/semantic
coverage is partial and resource-limited. These are static observations, not
proof of the current live vendor view or every interaction modifier.

## Verification and remaining work

Pure tests verify quarter-turn coordinate inverses and source-path hits, and
refusal to guess a missing layout. An opt-in private-asset test parses every
exact projection without substituted paths. The native interaction test loads
a synthetic explicit UK placement, independently targets Esc/F1 coordinates,
checks click/marquee/cancel/zoom, and reruns file/save/exit preservation with no
device SETs. The production paint output was captured and inspected.

Device chassis/artwork, character paths, antialiasing/hover fidelity, full
toolbars, pan/move/pen/bucket, selection groups, effect-region assignment and
properties, actual stock reconciliation, preview/output, optical verification
and the complete-build100MB target remain unfinished. Canvas fit/zoom and
Ctrl-toggle behavior are an explicit partial native interaction, not demonstrated
stock pixel/interaction parity.
