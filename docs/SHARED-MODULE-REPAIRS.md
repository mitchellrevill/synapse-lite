# Shared module repairs

This checkpoint repairs the existing Macro, Lighting/Studio, Linked Games and
shared icon surfaces. It adds no new device protocols or lighting effects.

## Changes

- The normal Macro page uses the recovered 600px editor and 250px action menu,
  source SVGs, a bordered library selector, contextual file actions and a rounded
  navigation pill. AI template/promotional content remains excluded.
- Parent-painted library menus no longer compete with child EDIT windows.
  Switching a changed macro offers Save, Discard and Cancel. Save uses the existing
  revision guard and switches only after the worker succeeds.
- The old detailed screen has been removed. Delay values, loop repeats and
  random ranges edit within event rows; Rename edits the selector; event
  commands use a contextual menu. Key Binds and recording-options parity remain
  unfinished.
- Lighting / Advanced Effects opens the existing Studio through the ordinary
  parent action dispatcher and its transition guards. Opening Studio does not
  itself apply a lighting effect.
- Linked Games reads the active Game Manager 3 account's detected and manually
  retained game catalogues with native Rust/CNG AES-CBC decoding. It does not run
  the vendor service, scan other accounts, launch games or modify vendor files.
- Source games merge by normalized executable path. Existing local names, IDs,
  profile links and unknown metadata win. Viewing does not save; an explicit
  edit persists the merged catalogue. Removed source games remain hidden.
- Missing or unrecognized stock catalogue data leaves local games available
  with a notice. Explicit private configurations do not read normal account data.
- Local game ICO and PNG-in-ICO artwork replaces generic placeholders when available. Optional
  locally provisioned covers are bounded to 32 decoded images in memory. The
  application never fetches covers in the background.
- Re-entrant child rendering keeps ordinary edit fields dark without aliasing
  the borrowed editor state. Studio empty-state text describes the existing
  layer tools instead of claiming they are unavailable.
- Macro, Chroma and Settings use explicit source assets. Settings no longer uses
  a Unicode gear or text button; Device Information uses the source book icon.
  Source SVG hashes and 3x raster preparation remain in the provisioning tool.

## Artwork download gate

`tools/provision-game-covers.cjs` is an explicit, separate build-time importer.
It accepts only bounded image downloads from the official static cover directory,
with no redirects, credentials, query strings or application telemetry. Automatic
approval review initially rejected the import because individual URLs disclose
which games are in the catalogue. The user explicitly approved those 62 requests.
60 covers were imported; two requests returned HTTP 403 and use local icons.
The application itself remains offline for artwork rendering.
Personal catalogues, icon files, covers and recovered vendor assets are not added
to this repository.

## Verification boundaries

The native surface check exercises visible pointer hit targets, real inline EDIT
notifications, worker persistence and captured rendering with private simulated
devices. Separate command regressions dispatch existing Macro actions directly
through the current window and verify history, recording, revision conflicts and
XML behavior; they are not visual parity checks. No test renders the removed
legacy Macro screen. A separate installed-catalogue check reads current account metadata without saves.
The game view check uses a read-only copy of installed metadata in a private store.
These checks do not establish physical input delivery or fresh stock-window pixel
parity. The desktop-control helper currently fails before initialization, so the
stock reference captures remain explicitly historical (27 October).

Full visual parity, recording options, Key Binds,
the two unavailable covers, all device/API acceptance and the complete-build memory target
remain open. No full-goal completion is claimed by this checkpoint.
