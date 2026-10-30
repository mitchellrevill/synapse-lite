# Studio workspace repair

This checkpoint restores the persistent thirteen-effect palette and source effect
icons in the layer list. It uses the existing local draft operations, history and
visibility controls. It adds no device protocol or effect renderer.

The recovered `EditorCanvas.9130aa00.chunk.css` establishes 76 × 72 CSS pixel
effect cells, 30 pixel palette masks, 10 pixel labels, 40 pixel layer rows and
20 pixel layer masks. The native editor now uses those dimensions and the
recovered masks, with gray and green tints. Layer scrolling and drop targets end
above the palette; short windows scroll the palette independently. The eye fades
visually while its target stays available, as in the source surface.

`tools/provision-studio-chrome.cjs` takes a local recovered `static/media`
directory, output asset directory and installed Sharp module path. It creates
46 premultiplied BGRA masks, with per-file source and output hashes in
`studio-chrome-provenance.json`. Vector masks render at threefold density before
resizing. It performs no network access or vendor JavaScript execution. These
private vendor assets are not embedded in the Rust binary or committed here.

The native regression enters Studio through the real Lighting button, creates a
private composition, adds an Ornata layout, clicks every palette tile and checks
its effect identity and undo. It also checks the eye toggle and undo, and records
native captures. The device is simulated and the configuration isolated. This is
UI interaction evidence, not physical device or complete pixel-parity proof.

The comparison reference is the saved 27 October stock Studio capture. The
desktop integration failure prevents a fresh stock capture. Reference and native
windows differ in viewport and DPI, so no numerical pixel-match score is claimed.

Still open: compact source toolbar and contextual commands, canvas chassis and
key legends, property controls, bottom-right viewport controls, full state/menu
comparison, and any effect-runtime gaps. Palette presence does not certify live
device playback for all thirteen effect types. Full pixel parity remains open.
