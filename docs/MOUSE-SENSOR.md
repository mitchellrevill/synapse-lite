# Calibration and Advanced implementation and validation

28 October2026. Implements the user's priority change from tab refinement to
mouse features. Application tabs were not refined in this unit.

## Implemented behavior

- Calibration: Smart Tracking Low1mm/High2mm, source welcome dismissal saved
  locally, explicit read/retry, device-wide intent outside named profiles.
  State and lift registers are captured before changes; failed writes or saves
  restore the exact checkpoint and verify it. Non-UI modes/distance3 are retained.
- Advanced: enable/disable Dynamic Sensitivity, Classic/Natural/Jump/Custom,
  source preset seeds, custom template selection, X/Y chart zoom, existing-point
  dragging, source PCHIP interpolation and the exact250-sample firmware packing.
  Pointer drafts write only on release. All curve bytes are captured before a
  custom write; chunk headers and independent read-back are validated.
- Rotation: -44..44 degrees, switch retaining disabled angle, drag-on-release,
  verified firmware routing. Extended firmware1.10.1.0+ uses hardware0B14/0B94;
  older PID00C5 uses the already installed signed vendor filter. Old-firmware
  wired00C4 is refused until its separate filter handler is verified. No cursor
  hooks, synthetic movement, replacement driver or vendor DLL initialization.
- Rotation ownership persists across profile switches; save failure restores
  the prewrite value. Normal shutdown restores the device-lifetime baseline.
  Rescan restores before closing handles and reacquires prior saved intent.
  Changes from another owner are preserved. Stock Synapse blocks Rotation SET.
- Startup/device arrival/resume replays explicitly saved device-wide Calibration
  through a separate connection path. A named/foreground profile switch does
  not replay it. Matching registers need no SET; failed replay restores the
  original raw checkpoint. Stock-owner and measurement restrictions are honored.
- Rotation replay resolves the same nonempty protocol serial across00C4/00C5
  store aliases, without silently saving a read-only store migration. Tests also
  keep two identical model names bound to their distinct physical device keys.
- Dynamic Sensitivity and Rotation are per-profile; both round-trip through
  Synapse4 exports with opaque members preserved. Float-roundtrip parsing fixes
  checksum instability for custom sample arrays. Calibration exports warn that
  its device-wide preference belongs in the native JSON backup.
- Native actions pin device/profile identity, use the serialized device worker,
  and honor read-only admission. No per-frame HID polling or UI-thread SET.

## Evidence and validation

Source196 UI: main.bbc961f9.js SHA256
`f99b1f88ca9496a77cab258d05814a312b876873cb5c586841c28a732db78c42`;
CSS main.116d273e.css SHA256
`cb6c673798b7e9bc844829d41cea64b5a0ab2d389f8863604df3bdcccc68138c`.
REA parent `ev_9345fd412a081136ab9b01eb197d5bb79344fee6ad4673f1432ed8a36be02b62`.
Middleware parent
`ev_bfbfa82f382fec19dd5f8e4373998fef6772903a27584a9acbac1dfd84de407c`.
Static JavaScript coverage is partial; no claim of complete decompilation.

Source geometry and exact strings are retained in workspace
`work/calibration-source` and `work/advanced-layout`; interpolation replay lives
in `work/advanced-curves`. Repository golden fixtures:
[sensor-curve-fixtures.json](validation/sensor-curve-fixtures.json).
Filter ABI provenance: [sensor-rotation-source.json](validation/sensor-rotation-source.json).

Native HWND simulator tests exercise Low/High, the saved welcome preference,
read-only rejection, named-profile isolation, mode changes, custom point drag,
Rotation drag, no SET before release, and exact Rotation restoration on stop.
Core failure tests cover command/save failures, hidden curve restoration,
stale identity, numeric firmware gates, external-owner conflicts and opaque
profile/export preservation. Run native ignored tests alone on an interactive
desktop; they disable physical discovery and use isolated stores.

Read-only physical observations: PID00C5/interface0/firmware1.2.0.0 answered
Calibration GETs; exact Windows container association found its vendor-filter
endpoint and two GETs returned0degrees. A later PID00C4/interface0 connection
reported1.10.1.0 and answered Calibration plus Dynamic state/mode GETs. These
are different connection observations, not evidence of a firmware upgrade.
The first reversible write attempt refused before SET while stock Synapse was
running. After it exited, the isolated physical harness on wired00C4 with
firmware1.10.1.0 verified Low1mm, High2mm, a complete250-sample custom curve and
Rotation+5/-5degrees by independent GETs. Each stage restored and verified its
exact original checkpoint, including all original curve bytes. No personal
profile file or Synapse database was modified, and no driver was installed.
The physical report is [mouse-sensor-physical.json](validation/mouse-sensor-physical.json).
This proves register transactions and restoration, not measured optical lift
height, movement quality, or real unplug/sleep recovery. Receiver00C5 was absent
at the later identity probe, so real cross-connection serial matching and old
firmware filter SET/restore remain unverified.

## Remaining acceptance

This is functional native coverage, not pixel-for-pixel acceptance. The template
dropdown, live selected-mouse speed fill, Ctrl+wheel zoom and Rotation tool link
are now implemented; see [Advanced feedback](ADVANCED-FEEDBACK.md). Dragging marks
a curve as None to preserve its source save/reload semantics. Chart labels,
switches, help/tooltips, disabled fading and exact font baselines still need
comparison. Receiver/filter SET/restore, stock
re-import acceptance, optical tracking, unplug/replug and sleep/wake remain to
verify. Connection/profile replay is covered by four isolated engine tests;
physical reconnect behavior is a separate acceptance item.

The complete inventory, Studio, profile migration and three full-build resource
runs remain part of the active goal. A small executable, unit tests or one idle
process measurement do not establish the100MB completion criterion.
