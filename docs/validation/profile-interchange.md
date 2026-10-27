# Expanded profile interchange checkpoint

Baseline: Secretarybird v0.5.0-beta1, unchanged upstream ancestry.

The initial cb82cca checkpoint passed 137 tests (127 core, 9 native tray, 1 integration); 5 existing core tests remain ignored. Release builds pass offline with the locked dependency graph. Synthetic tests cover foreign canonical hashes, DPI stages/independent axes, sleep, unconditional game mode, supported mouse/keyboard mappings, unknown-field preservation, native-created profile export, duplicate GUID renewal, unchanged identity on rename, aggregate document limits, and explicit rejection of unsupported exports. Additional regressions verify that rename preserves the on-board restore backup and that wired/wireless profile migration requires the same nonempty serial. Import refuses ambiguous matching connected units.

The native editor exposes Add, Duplicate, Rename, Import and Export. Add and Duplicate autoactivate the result. File input/output is capped at 2 MiB; export writes and flushes a temporary sibling before replacement. A regression test verifies successful replacement and preservation of the old file when an oversized export is refused. Common dialogs run outside the window-state borrow, and file writes finish outside the shared engine lock. Compatibility warnings remain available in a scrollable Help control.

The source observations come from the installed Synapse product605/196 UI and middleware. Focused retained REA evidence: ev_ff6dc972bf71433ce286388abede063d82564f9046937bc6c9813cc51e0efb96 and ev_95834af4e4e6514a194a837857248e20e745bf363d55cad95da8d9d1a1454942. Source-specific literals and path tables are kept in local ignored runtime assets; the public format fixtures are synthetic.

Limits: native Windows file dialogs adapt the functionality and do not reproduce the original Synapse modal. Interactive native file-picker/inline-rename flows still need end-to-end validation. No real Synapse re-import has yet been demonstrated. Macro execution/installation, linked applications, conditional game mode and Synapse 3 migration remain unsupported. The original Secretarybird beta does not preserve unknown extensions when it saves. Profile translation tests do not establish successful hardware application or pixel parity.

Four native geometry tests additionally cover SVG meet transforms, curved/concave hit regions, input-path validation and shared drawing/hit bounds. Recovered UK keyboard placement was corroborated by inert DOM/CSS replay without vendor JavaScript or screenshots. This establishes the local coordinate relationship, not full rendered parity.

The newer typed Performance state now translates and retains hidden/disabled rows and independent-axis intent. See ../PERFORMANCE.md for the updated contract, physical projection checks and current limitations; the test count above identifies the earlier checkpoint.
