# Launch assignment workflow

Customize now provides Program/Website selection, the stock .exe file picker,
read-only selected program path, editable website and a Lite arguments extension.
Save changes only the selected normal/Hypershift source mapping in the local
profile store. Cancel discards the draft. Save/import never execute the target.
The existing host-launch APIs and Macro playback are unchanged. Device-trigger
routing remains gated/unverified; saving an assignment does not enable it.

Stock evidence: recovered product605 MapLaunch.e978e3ea.chunk.js, SHA256
4c03a79826f14e48ed9f951fd66c8b48c001261600649eda52bf2153ba6486bb.
assignDefaultValues selects program by the presence of path, otherwise url;
openDialog calls showFileOpenDialog("Open", false, '[".exe"]');
getMappingData emits path or url; saveMapping wraps that in launchGroup.
Stock has no arguments field. Lite keeps unknown row/group/profile fields and
other inputs/layers. Source data is inspected as text, never executed.

Saving validates the full original profile and active profile against the latest
store while holding the existing worker lock. Ambiguous duplicate input/layer
records and missing source mappings refuse Save. Local persistence uses the
existing bounded atomic Store save. Imported profiles remain inert.

TODO: source-matched Launch form geometry/fonts and stock screenshot comparison;
verified device-trigger dispatch, reconnect/recovery and complete-build memory
acceptance. The additional arguments field and local-only status are intentional
Lite extensions. URL prefix normalization currently accepts nonempty strings and
adds https:// when no scheme separator is present; exact stock validator recovery
remains open.

Validation: 325 ordinary tray tests passed serially (58 ignored), including native and Synapse4 backup round-trip vectors. The opt-in native Save test passed using only a simulated keyboard and temporary store, with zero SET commands. Snapshot GET reads are permitted. The Windows file-replacement tests require ordinary host access; the restricted-run failures were resolved by rerunning with that access.

Both release workspace binaries build with cargo build --release --workspace -j 1. No physical input, process launch, device write, stock-service change or clipboard access was used for validation. The test fixture target is nonexistent.

Final targeted verification: nine assignment model tests pass, one native Launch Save test passes, and both existing native assignment popup regressions pass serially. The shared test-only capture helper now renders native EDIT child fields. Native capture geometry is 1600x900 at 125% DPI (1280x720 CSS geometry); no matched stock Launch screenshot is available, so visual parity is not claimed.
