# Reopen the last Studio composition

The native editor remembers the opened composition file and selected profile.
After restarting the app, opening Chroma Studio reloads that file and selects the
same profile by its stable source ID. A reordered archive therefore does not
silently select a different composition. Existing editor controls and tabs are
unchanged. Opening Studio again in the same session retains its current document.

The bookmark is editor state only. Restore does not activate device playback,
start Local Preview, capture input, change driver mode or modify device profiles.
Automatic Studio output across launch/reconnect remains separate unfinished work.

## Persistence and failure behaviour

Successful manual Open, profile selection and composition Save enqueue a bookmark
write on the existing file worker. The record is studio-editor.json in the separate
SynapseLite configuration directory. It contains version1, absolute path and stable
profile_id; unknown native bookmark fields survive updates. No composition data,
LED frames or key events are duplicated there. Paths and profile IDs remain local.

The256KiB record budget,32767UTF-16 path limit,512-byte profile-ID limit and existing
8MiB archive budget are enforced. Relative paths and Windows device namespaces are
refused; restoring requires a regular file. Missing/duplicate IDs refuse instead
of falling back to a numeric index. A missing file, unreadable/corrupt archive or
unsupported bookmark retains the reference and reports an error while Open File
remains available. Malformed/unsupported bookmark records are not overwritten by
later automatic saves; explicit bookmark repair/reset remains a TODO.

Restore runs once when Studio is first opened in an app session, rather than
repeatedly reopening a failing path. Read-only presentations may restore; neither
they nor the worker's measurement read-only mode can write bookmarks. The worker
also refuses composition-file saves in measurement read-only mode. The device lock
is released before file work. Writes use the existing create-new temporary file,
flush and atomic-replacement helper. A bookmark failure is reported separately
from the composition file; it does not roll back a successfully saved composition.
Epochs reject stale restore/remember responses. Existing unsaved-edit gates remain.
External paths can still fail or stall in filesystem I/O; no file deadline, network
route or physical-device acceptance is established by this unit.

## Verification

Three focused tests cover exact identity after reorder; missing/duplicate identity;
read-only refusal without creating a file; corrupt/unsupported/excessive bookmark
retention; unknown fields; failed metadata replacement; relative/device-path
refusal. A native two-window check opens a two-profile synthetic composition,
selects Second, closes, reorders the source archive and opens a fresh window.
Studio restores Second at its new index0. Restore leaves bookmark bytes unchanged,
keeps the source archive and native device profiles byte-for-byte unchanged, and
starts neither preview nor device requests. Hardware is simulated and physical
discovery disabled. The captured restored title was inspected; this test fixture
has no authenticated device canvas. Existing actual-export Studio checks cover
normal geometry and editing separately; no new visual-parity claim is made.

Workspace468core/298tray/4CLI/1doc checks pass; six core and42tray checks remain
intentionally ignored. See validation/studio-editor-session.json.

Full source library migration, automatic output/reconnect policy, all effects,
macros/Snap Tap/input recovery, broad device APIs, stock UI fidelity, privacy and
three complete-build runs below100,000,000bytes remain open. The installed vendor
selector ownership/cleanup limitation remains recorded in KEYBOARD-OUTPUT-RECOVERY.md;
this editor feature does not resolve or bypass it.

The optimized build, three optimized session checks and nine optimized native
Studio checks pass. Restoration only reopens editor data; automatic saved-output
startup/reconnect acceptance remains open.
