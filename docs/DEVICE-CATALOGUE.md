# Catalogue coverage and device identity

`secbird catalog inspect FILE` reads a local copy of Razer's public catalogue before
opening HID or the profile store. It does not download resources or enable device
controls. The parser retains product IDs separately from dongle, Bluetooth, wired,
Xbox, PlayStation-mode and monitor-port IDs. Supplemental duplicate product records
merge; incompatible explicit Chroma declarations fail. Missing Chroma declarations
mean unknown. Input is limited to 2 MiB, 4096 records and 64 aliases per role per record.

Source: `https://apps.razer.com/synapse/dashboard/AvailableDevices.json`, retrieved
2026-10-28T23:26:46Z,10963bytes, SHA256
`f087e20349b803a11fa8299bfcb2e2c28c9e875c21aaa9242279655a22bc62e9`.
This is a changing metadata source, not a functional compatibility guarantee.

The inspected snapshot has 334 primary products and 481 distinct primary/connection
IDs. The inherited protocol database has 288 USB IDs across seven kinds; 186 current
catalogue primary IDs appear in it and 148 do not. Legacy entries absent from this
catalogue remain in the database. No new protocol declarations are inferred here.

Two connection IDs are ambiguous: 2308 maps to 117/574 and 2593 maps to 2594/2595.
Resolution returns every distinct matching product in stable order, even when an ID
also appears as a primary ID. Firmware fallback now refuses an ambiguous match
instead of choosing the first product's updater. A product-specific direct manifest
continues to take precedence. No firmware is installed or flashed.

This deliberately differs from the inspected stock lighting resolver, which uses
the first primary/dongle/Bluetooth match. Asset acquisition must preserve both the
requested ID in the manifest filename and the resolved product directory; arbitrary
resolution could select the wrong layout as well as the wrong firmware link.

## Portable metadata and artwork collection

`tools/collect-device-manifests.py CATALOG OUTPUT` generates an offline plan.
Add `--fetch` to collect the product UI/MW manifests through fixed-origin HTTPS.
Then use `--stage lighting` or `--stage dashboard`, again with `--fetch` when
ready. These stages only use editions explicitly listed by captured MW manifests.
Chroma manifests retain all raw LED configuration and SVG keys; the collector
does not execute stock getters that prune the maps to one selected layout.

Dashboard artwork uses the source-defined category-default role (keyboard layout 1,
otherwise layout 0), trying AVIF 3x then 1x. It does not prove that an edition/layout
pair exists physically or that every regional/sidepad/wrist image is present.
Missing editions, manifests and images become report TODOs; no IDs are guessed.
AVIF files are captured as inert bytes, not loaded into the app by this tool.

Each run has four workers, a 12-second request timeout, an 8 MiB resource limit and
a 256 MiB download budget. HTTP 429/503 stop further queued network work; HTTP 404
advances to an explicit fallback when available. Other failures are recorded
without a retry loop. Only the public asset origin is allowed through redirects.
JSON and AVIF container admission prevents saving HTML error responses as assets.

Captures retain local SHA256, byte count, source URL path, timestamp and available
version/header metadata. Different bytes never silently overwrite a captured
revision: start a new snapshot directory for a fresh revision. `--resume` reuses
validated files without network freshness claims or consuming the new download
budget, so collection can continue beyond one batch. Previous reports are retained
with unique history names before the latest report is replaced.

Native payloads, installer actions and remote application code are not fetched or
executed. Supply a private output directory outside the repository; artwork is
not automatically redistributed or eagerly loaded into runtime memory. These
acquisition reports are separate from protocol and hardware compatibility evidence.

Validation: `python -m unittest discover -s tools -p test_collect_device_manifests.py`.
In restricted Windows environments set TEMP/TMP to an allowed workspace directory.

### Captured coverage, 28 October 2026

The first snapshot and resumed bounded batch produced the following locally verified
resources. Every available file was independently reread and matched against its
recorded size and SHA256; all 653 dashboard images were fully decoded with Pillow.

| Resource role | Available files | Distinct primary IDs | Missing routes | Other TODOs |
|---|---:|---:|---:|---:|
| UI/MW manifests | 666 | 333 | 2 | 0 |
| Chroma edition manifests | 585 | 277 | 175 | 3 oversized resources |
| Category-default dashboard artwork | 653 | 298 | 110 | 0 |

Product 680 has no available UI/MW manifest, so lighting and dashboard planning
each retain one additional product TODO. Chroma resources for 653/edition0,
653/edition128 and 659/edition129 exceeded the 8 MiB per-resource budget and were
skipped. Raw captured lighting maps contain 2193 LED configuration keys and 2075
SVG keys; these are map entries, not distinct models or verified usable layouts.
All available lighting documents with a productId agree with the requested primary
ID. Primary-only collection does not yet cover every secondary connection filename.

The acquired files occupy roughly 319 MB on disk. They are private acquisition
outputs, not loaded runtime data, shipped assets or a measured application RAM result.
The independent summary and capture-report hashes are in
`docs/validation/device-asset-acquisition.json`. The seven collector tests cover
identity validation, supplementary records, explicit editions, category-default
selection, AVIF admission, revision preservation, resume and rate-limit behavior.

## Remaining integration work

- Acquire source-declared device, edition and layout manifests, artwork and LED SVGs.
  Acquired dashboard artwork and lighting maps now cover many products, but existing
  native UI integration principally covers DeathAdder 196 and Ornata 605. This
  change does not enable catalogue-wide controls or complete customization artwork.
- Record per-feature evidence separately: catalogue declaration, artwork available,
  implemented API, simulation result and hardware verification.
- Add product selection for shared receivers when runtime device identity supplies
  enough evidence. Never use the first catalogue match as a substitute.
- Extend the current feature-report transport for the recovered input/event,
  Bluetooth, serial, Wi-Fi, IoT, audio and LampArray families as applicable.
- Continue Chroma effects, tools, hardware output and full profile compatibility.

Unresolved details become explicit TODOs with their evidence and next check; work
continues on independent features. Full functional and visual parity and the
complete-build100MB acceptance gate remain open.

The subsequent Studio preset pack now supports977 exact layouts across117products
for offline LED selection and regional editing in imported compositions. See
STUDIO-PRESETS.md. This does not enable catalogue-wide physical controls/output
or replace the separate evidence requirements for each API.
