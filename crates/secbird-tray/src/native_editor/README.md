# Native editor implementation boundary

This module draws with Win32 GDI and calls the existing device dispatcher directly on one worker. It starts no browser and opens no network listener. It uses layout facts recovered from installed AppEngine4.0.827 and locally cached Synapse product605/196 source. The source includes no copied vendor JavaScript or CSS.

Implemented native controls cover device selection, profile creation/switching, DPI axes, polling rate, one lighting zone's brightness/static color/spectrum/breathing, sleep timeout, firmware-supported button assignment reads/writes and restore-all. Calibration/Advanced tabs are shown as unavailable. Keyboard geometry/hit regions, all effects, per-key painting, macro editing, profile import/export UI, full source DOM/state parity, accessibility and exact Chromium text rendering remain incomplete. This is a partial source-based port, not verified pixel-for-pixel parity.

`open(Arc<Mutex<Engine>>)` runs a single editor on a dedicated UI thread. `run_standalone(engine, Some(duration))` exits the window after a smoke interval. The source-view smoke setting is accepted only for standalone timeout mode or `--runtime-smoke`: `SYNAPSE_LITE_SMOKE_VIEW=dashboard`, `mouse-performance`, or `keyboard-lighting`. It selects an actually detected matching device after the read-only snapshot and sends no mutation requests.

Runtime assets are loaded from `SYNAPSE_LITE_ASSETS` or an `assets` directory next to the executable. Fonts are loaded with `FR_PRIVATE` and removed on teardown. `provision_assets.py --evidence <local-recovered-evidence> --output <ignored-assets-directory>` copies the three reconstructed source fonts and converts the original mouse/keyboard model AVIF artwork to BMP against the source background color. It records hashes in provenance.json. Pillow is an authoring dependency only. Generated vendor assets must remain excluded from public source commits.

The source recovery specification and machine-readable CSS rules/font metrics are separate research deliverables. Native smoke memory results establish only the measured workload and duration; they do not establish complete functional or visual acceptance.
