# Catalogue coverage and device identity

`secbird catalog inspect FILE` reads a local copy of Razer's public catalogue before
opening HID or the profile store. It does not download resources or enable device
controls. The parser retains product IDs separately from dongle, Bluetooth, wired,
Xbox, PlayStation-mode and monitor-port IDs. Supplemental duplicate product records
merge; incompatible explicit Chroma declarations fail. Missing Chroma declarations
mean unknown. Input is limited to2MiB,4096 records and64 aliases per role per record.

Source: `https://apps.razer.com/synapse/dashboard/AvailableDevices.json`, retrieved
2026-10-28T23:26:46Z,10963bytes, SHA256
`f087e20349b803a11fa8299bfcb2e2c28c9e875c21aaa9242279655a22bc62e9`.
This is a changing metadata source, not a functional compatibility guarantee.

The inspected snapshot has334 primary products and481 distinct primary/connection
IDs. The inherited protocol database has288 USB IDs across seven kinds;186 current
catalogue primary IDs appear in it and148 do not. Legacy entries absent from this
catalogue remain in the database. No new protocol declarations are inferred here.

Two connection IDs are ambiguous:2308 maps to117/574 and2593 maps to2594/2595.
Resolution returns every distinct matching product in stable order, even when an ID
also appears as a primary ID. Firmware fallback now refuses an ambiguous match
instead of choosing the first product's updater. A product-specific direct manifest
continues to take precedence. No firmware is installed or flashed.

This deliberately differs from the inspected stock lighting resolver, which uses
the first primary/dongle/Bluetooth match. Asset acquisition must preserve both the
requested ID in the manifest filename and the resolved product directory; arbitrary
resolution could select the wrong layout as well as the wrong firmware link.

## Remaining work

- Acquire source-declared device, edition and layout manifests, artwork and LED SVGs.
  Existing custom assets principally cover DeathAdder196 and Ornata605; this change
  does not supply catalogue-wide images or controls.
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
