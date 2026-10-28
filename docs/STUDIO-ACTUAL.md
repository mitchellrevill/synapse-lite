# Actual stock composition compatibility

28October2026: stock Chroma's normal Export flow produced the user's saved
**Chroma Effect1**. The private27897-byte JSON-v1 file has SHA256
`7e87b7ed6836e8d9f81686ff426624c4a7be0953d7af17e1f61dcbdb245f9649`.
It contains one Ornata placement (product605, edition0, layout6,960×360),109
LED records, and Fire/Spectrum Cycling layers. Raw profiles, device identifiers
and configuration are private and are not committed or bundled in app packages.

## Compatibility fix

This actual export carries the inventory in **`leds`**, without `ledRef`. The
earlier synthetic/save-derived admission required `ledRef`, so it could not show
this real composition. Core region editing and native canvas admission now share
one bounded inventory reader. Either form works. If both occur, coordinate sets
must agree; current `leds` order/metadata takes precedence. Conflicts require
explicit reconciliation and refuse without changing the file. Null/non-array,
duplicate or excessive inventories are rejected. Raw LED metadata is borrowed,
not converted through numeric objects or rewritten into a different schema.

## Observed validation

- An isolated actual-window test loaded the private export through the production
  Studio worker, admitted one device/all109 selectable paths and both layers,
  and hit the independently known Esc position without dirtying the document.
- Native Save wrote a new private file. Its raw profile **and whole-file bytes**
  match the stock export in this observation. Whole-file identity is not a
  universal guarantee for arbitrary envelope formatting or edited documents.
- Stock's Import preview opened the Lite-produced file, listed Chroma Effect1
  with Import available, and displayed no unsupported-device/layout warning.
  The preview was canceled; the final Import action was not clicked. Committed
  stock re-import/persistence and effect replay therefore remain unverified.
- Source export and isolated device-profile store stayed unchanged, with no
  device SETs from the Lite test. Native RGB/erase/history/recovery tests also
  pass using the current exported `leds` shape. Separate core tests retain
  saved-reference support and reject conflicts atomically.

394 core,3 CLI,1 doc and233 serial tray tests pass; actual-file and regional
native tests pass separately with simulated transport. See
[actual file evidence](validation/studio-actual-file.json) and
[native evidence](validation/native-studio-actual-file.json).

The export was copied to the local task's private migration output for the user
to open in Lite. It was not installed into a vendor account/store, automatically
activated, or included in the public repository. Fire/Spectrum properties,
compositing and lighting playback remain unfinished; successful parsing and file
preservation do not prove equivalent lighting or full migration acceptance.

## Current stock launch provenance

The installed AppEngine is now4.0.896, replacing the previously inspected4.0.827.
Its archived `electron/main.js` is61862 bytes, SHA256
`8b88a8f24f8864c263343b0d33ac04c68455f278335a16e5f8f81b4ea7e2999c`.
Bounded static extraction verified `--url-params=apps=chroma-app`; the Studio
tab is inside that application, not a supported top-level `chroma-studio` launch
target. `--exit-application` provided graceful cleanup after export/baselines;
AppEngine processes were observed exited. The existing Lite editor was untouched.

REA entry-extraction parent
`ev_29ced43fa5460a06a971c25e908df4732959d44a2b7c8d0aeb964bf1e67425d1`,
summary `ev_04f472d16476cad892ecd85e9323420edcdf9f31e33d1d66596eff56a850a01b`:
one JS file parsed without failures. Only package metadata and this entry file
were selected; dependencies/full application behaviour remain unexamined by this
focused analysis. No vendor JS was executed by REA.

## Stock resource sample

The separate [stock baseline](STOCK-STUDIO-BASELINE.md) records startup and
Studio samples, executable/raw-trace hashes, process scope and limitations.
It is not full Synapse workload or replacement memory acceptance.
