# Local Studio composition management

The Studio header has a composition selector and a separate ellipsis menu for
Add, Import, Rename, Duplicate, Export and Delete. This is a native local file
workflow; it does not modify the vendor store, migrate legacy profiles or invoke
device output. The existing JSON-v1 interchange codec remains the file boundary.

Add creates a composition with an empty device list and a Spectrum Cycling layer.
Add and Duplicate generate twelve-character lowercase hexadecimal composition IDs.
Duplicate changes only the composition ID and title: layer/group/device identities,
LED metadata, unknown fields and raw numeric literals remain intact. Imported IDs
are retained. Names require 1–32 UTF16 units, nonblank text and no control characters.
Generated duplicate names are bounded and avoid existing titles.

Delete requires a native confirmation with No as the default. Deleting the last
composition produces a valid empty library, which can be saved and reopened.
The session bookmark accepts a null composition identity only for an empty library;
existing string-valued bookmarks remain compatible. Unknown bookmark fields remain
preserved by the existing session writer.

Import reads a file on the existing file worker, then presents a checked native
selection menu. Done commits the chosen subset in source order; Escape cancels.
Select all/none are available and an empty selection cannot be committed. Import
appends the selected raw profiles and retains disjoint unknown envelope fields.
Duplicate composition IDs, ambiguous headers, conflicting envelope fields and the
existing codec budgets cause an atomic refusal. This deliberately preserves IDs
and foreign-device metadata rather than performing stock's device remapping.

Export uses the same selection menu, initially selecting the active composition.
It exports the current draft snapshot and retains its dirty state. The native save
dialog confirms overwriting an existing export. Exporting over the open library's
path is refused; Save is the action for updating that file. Normal paths are compared
after canonicalization and without case sensitivity on Windows.

Composition changes remain drafts until Save succeeds. An existing library saves
to its current path; a new library asks for a path. Creation, duplication, deletion,
import, composition switching, file opening and closing use the existing unsaved
Yes/No/Cancel flow. Yes waits for successful persistence before proceeding, No
restores the saved library/draft, and Cancel leaves the draft open. A failed save
retains the dirty library and cancels its continuation. Library changes establish
a new per-composition draft session; layer history is scoped to that session.
Composition operations are recovered through the saved-library discard boundary,
not the layer Undo/Redo buttons. Renaming without changing the text is inert.

## Source and visual evidence

The retained `stock-studio-composition-menu-0.jpg` shows the separate ellipsis
menu with Add, Import, Link Games, Rename, Duplicate, Export and Delete. Link Games
is outside this feature. The native menu follows that separation and retained action
order; its buttons, spacing, selection menus and confirmation dialogs are native
presentations, not a claim of pixel parity. The retained screenshot is 2048×1104;
new screenshots come only from this checkout's isolated simulated window.

Cached Studio main SHA256
`109b483f129044e1c7c17b9fcae0e30fa775885667976807cf2c94bd1c696d19`
and the existing [file evidence](STUDIO-FILES.md) establish the v1 container and
source-shaped Spectrum default. This feature reuses those contracts. A fresh REA
analysis of copied, unchanged ModalImport/ModalExport artifacts parsed both files
with no parse failures: parent
`ev_b46a06938891cb6662635a2cec7fa93558d8a5cc3070ef8f23f4d41d897332fc`,
summary `ev_4e16171b3bbcfe81d5e932e479c7c3f3e9b890882fa98e45a1073bee74d31dc5`.
The subset has 11,294 text bytes, 6 module roles and 939 semantic unknowns.
It is static artifact evidence, not execution of the stock importer/exporter or
proof of complete graph coverage. Original ModalImport/Export hashes are retained
in STUDIO-FILES.md. No stock runtime was launched or stopped by this feature.

## Validation and limits

Focused validation commands (one build job and one isolated target directory):

```powershell
$env:TEMP = '<checkout-private-temp>'
$env:TMP = $env:TEMP
cargo test -p secbird-core studio:: --jobs 1 --target-dir <isolated-target>
cargo test -p secbird-tray studio --jobs 1 --target-dir <isolated-target> -- --test-threads=1
cargo test -p secbird-tray native_studio_composition_library_create_rename_duplicate_delete_and_dirty_guards --jobs 1 --target-dir <isolated-target> -- --ignored --test-threads=1 --nocapture
```

On 30 October 2026, 99 core Studio tests passed; 61 tray Studio tests passed
and 21 optional fixture/native checks were ignored. The opt-in composition-library
native test passed separately. The first tray run used the restricted default
temporary directory and failed file replacement with access denied, including
unchanged existing tests; using a private writable temporary directory resolved
all 23 failures. No production persistence workaround was added for that host
test-environment restriction. Test builds passed; no optimized release build or
complete-workspace test run is claimed.

Core tests cover raw preservation, creation/admission, duplication/rename/delete,
selected import, ID/envelope conflicts, invalid names and ambiguous root metadata.
Native module tests cover discard of new and saved libraries, deletion of the last
composition, empty-library persistence, read-only worker refusal, failed saves and
invalid exports leaving existing bytes intact. The bookmark regression covers an
empty-library restart and refusal to silently select a composition added externally.
The opt-in native test exercises actual header/menu hit routing, the rename field,
Yes/No/Cancel transitions, delete confirmation, failed/successful asynchronous saves,
stable saved IDs, raw numeric preservation and zero simulated device requests.

Interactive import/export selection menus still need stock/runtime comparison and
manual usability coverage. Stock committed re-import, physical lighting, full visual
parity and complete-build three-run memory acceptance remain unverified. The file
workflow inherits the existing atomic writer; concurrent external modification
detection and hard-link identity checks are not added by this feature.
