# Installed mouse onboard performance settings

This feature adds an explicit, capability-gated Device196/197 workflow for reading
slot1, saving a named profile's DPI/stages and polling intent, and restoring raw
originals. It does not establish complete onboard-profile or all-device support.
Native entry: the selected mouse's profile actions menu → **Onboard memory**.

## Source contract and audit

The sanitized static source ledger is
[onboard-196-source-contract.json](validation/onboard-196-source-contract.json).
It records original artifact SHA256 digests, byte offsets, extraction provenance,
REA IDs and coverage limits. Vendor code and private account/device data are not
included. No source JavaScript, hardware writes or firmware operations were run.

REA evidence:
- `ev_a45c8695c9ad755f2f3a66b6894bafe3ba12bfc00796cf0bac075cab49352fb5`
  retains static/module facts; some semantic graphs exceeded the result budget.
- `ev_75665eeb8c255bf53b356c7cdd9ae7ef1b5b9cc18855ae8636391fea91fcbe56`
  covers the supplementary polling constants and actual OBM engine without that
  resource warning. Both are static evidence, not observations of firmware.

| Setting | Source contract | Implementation |
| --- | --- | --- |
| Slot count | DeviceInfo advertises1 | Only explicit slot1; no active-slot fallback |
| Active DPI | GET04:85 / SET04:05,7 bytes | Raw X/Y/Z backup and readback |
| Stages | GET04:86 size80; SET04:06 size3+7*n | 1–5 rows; GET IDs1-based, SET IDs0-based |
| Polling | GET00:8E / SET00:0E, `[slot,period]` | Product choices125/500/1000; raw original codes retained |
| Brightness/macros | Advertised false | No controls, allocations or writes |
| Buttons | Source advertises mapping groups, but installed acceptance unproved | Existing generic Save to mouse refuses196/197 |
| Profile identity | Source skips first-array metadata read and ID1 creation | Host name/serial retained; no synthetic firmware GUID/name/owner |
| Keyboard slots | Unverified | No inferred keyboard capability |

The previous `OnboardSave` journal only retained textual button actions. It did
not retain active DPI, stage rows/Z bytes, profile polling, firmware metadata or
full opaque mapping payloads. It remains separately preserved for legacy recovery.
The new `onboard_settings` journal never replaces or clears that record.

Source DeviceInfo's DPI maximum is20000, while the repository database says26000.
New onboard intent uses the source100–20000/step50 gate; raw originals outside it
are preserved. Source authored Z values are zero; restoration retains original Z.
Unused stage-tail bytes are retained in the journal but are not synthesized into
variable-length SET packets or treated as readback acceptance criteria.

## Recovery and persistence

One device-wide, versioned journal retains the physical serial, slot, named
profile, complete raw originals, desired payloads, opaque future fields and phase.
All deterministic pending/completed documents must satisfy the existing2MiB /
32768-token store limits before SET. The pending journal is synced to disk before
any device mutation. An acknowledged SET is insufficient: mandatory GETs must
match all applicable bytes. Every partial-write/readback/final-save failure keeps
all original registers. An unfinished save requires restore before a new save.
Polling-only saves skip unchanged DPI/stage registers, preserving an independent
active DPI value. Restore is idempotent and removes the record only after verified readback and a
successful final file save. It restores stages before independent active DPI so
a changed active-DPI register is recoverable; save follows source DPI-then-stages.

There is no automatic retry/rescan of these mutation requests. Explicit readback
reports unavailable fields/errors, rather than pretending fallback values are
saved. Reconnect196↔197 uses the existing same-nonempty-serial migration. A record
from a different serial/schema/slot cannot replay. Profile imports preserve local
recovery and retain conflicting imported records as opaque evidence. Native
backups roundtrip the journal; Synapse profile intent/unknown fields stay intact.
Opening/reading the dialog neither saves the store nor sends protocol SETs.
Read-only worker/handle gates reject writes before journal creation.

## Acceptance limits and next diagnostics

- **Physical acceptance TODO:** with the owner present, first read slot1 and
  retain an independent raw backup; save a synthetic profile; read back; verify
  unplug/reconnect and operation without Lite; restore originals and compare
  every applicable byte. Test each transport separately and record firmware.
  No real onboard writes were performed by this task.
- **Automatic profile replay TODO:** existing generic DPI/performance/polling
  actions and `Profile::apply_settings` predate this explicit workflow and can
  write saved settings without this new journal. Integrate their first mutation
  with a durable store-aware capture/recovery boundary before claiming a global
  original-state guarantee. Do not put implicit filesystem I/O in Device methods.
  The original baseline here means before the first explicit onboard save.
- **Live/stored separation TODO:** the existing simulator shares DPI state;
  source-derived mock vectors exercise independent raw banks, but neither proves
  physical persistent/live semantics. Confirm profile polling versus live polling
  before altering Performance controls or replay routes.
- **Visual TODO:** no normalized source onboard-popup captures were recovered.
  The native modal uses existing native typography/panels, adds no shared tab and
  makes no pixel-parity claim. Capture source dialog empty/sync/error/readback
  states and compare geometry before a source-faithful popup replacement.
- Slot metadata editing, installed-family mapping acceptance, full profile
  synchronization, hardware profile-switch events and reset semantics remain
  separate source/runtime work. No other product's slot format is used.
- Full application memory target and complete all-device compatibility are not
  established by component tests or this PR.

## Validation

The focused test transport consumes the saved synthetic source-contract vector
`onboard-196-registers.json`; it validates explicit selectors, packet lengths and
stage-ID asymmetry, including nonzero original Z bytes. It injects disconnects at
each save/restore SET, ignored SETs, malformed slot readback and final-file-save
failure. Additional checks cover repeated saves, reconnect serial migration,
wrong identity/profile, query-only handles, resource refusal, I/O refusal, source
profile import and opaque-field retention. These are protocol simulations, not
captured hardware packets.

The ignored native HWND regression runs one bounded window with an explicitly
simulated mouse and isolated store. It operates the actual menu, read/save/restore
hit targets and worker, and checks read-only refusal plus unchanged named profiles.
Run it alone with `cargo test -p secbird-tray native_onboard_reads_saves_restores_and_refuses_readonly_writes -- --ignored --test-threads=1`.

Validation results are recorded in [onboard-196-validation.json](validation/onboard-196-validation.json):14 final focused core tests and the simulated native HWND workflow passed. The sequential core doctest also passed. Broader unit suites passed 4 CLI / 522 core / 306 tray tests before the final polling-only adjustment, with affected paths rerun afterwards.
