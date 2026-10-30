# Device API expansion evidence and boundaries

29 October 2026; base e6c21c3. Assets, presets, DeviceManifest geometry and
dashboard IDs are not API support. No new physical device was accessed.

| Product / path | Protocol evidence | Rust baseline | This expansion / limit |
|---|---|---|---|
| Ornata V2 025D, USB interface 2 | Selected 605 SDK and prior scoped physical registers | Separate identity, lighting/frame and indicator adapters | Unchanged; does not establish other keyboard families |
| DeathAdder V3 HyperSpeed 00C4/00C5 | Selected 196 SDK and prior scoped physical registers | Power, sensor and lighting adapters | Unchanged; no implied permission to copy sensor commands |
| Basilisk V3 0099, USB interface 3 | 153 DeviceInfo: PID153, claimInterface3; own 763 SDK | DPI/polling/lighting, no wheel API | Add wheel mode, acceleration, Smart Reel; slot 1 only while hardware slot 1 is active |
| Basilisk V3 Pro 00AA cable / 00AB receiver, interface 0 | 170 DeviceInfo: PID170, dongle171, BLE172, claimInterface0; own 7846 SDK; OpenRazer explicit model cases | DPI/polling/lighting, no wheel API | Same three explicitly corroborated controls; BLE00AC and dock forwarding excluded |
| Viper Ultimate 007A/007B, interface 0 | 122 DeviceInfo: PID122/dongle123, rzDevice25, claimInterface0 | Generic legacy power methods | Investigated; newer low-power UI schema and exact model firmware requirements remain unproved; no new writes enabled |
| Mouse Dock Pro 00A4, interface 0 | 164 DeviceInfo and Linker SDK; dock transaction IDs E0..FE | Generic receiver/lighting paths, fixed FF dock assumption | Not enabled: generic charging helper exists but no selected model caller proves semantics; fixed FF is outside the inspected Linker sequence |
| Nommo Chroma0517 / Pro0518 | Own SDK wrappers call RzNative_0517/0518 DLL volume/mute, container identity | Catalog and generic HID lighting | Native DLL boundary unresolved; no guessed HID audio commands |
| BLE, Protocol30/40, serial, IoT, OLED, firmware maintenance | Distinct shipped adapters and native API boundaries | No general implementation | Refuse transport substitution; no flash/pairing/driver installation |

## Source anchors

Public shipped files were fetched inertly from
`https://apps.razer.com/synapse/products/{id}/mw/` into this chat's private
`work/model-evidence`. No bootstrap/vendor runtime was executed.

- 153 `main.13212a9fcf9f63d3e397.js`: DeviceInfo around character226593;
  `763.a34404889c2c905e8256.js`: wheel tuples around782000, setters/getters
  around887086..891300, task slot selection1842440 returns slot1 and mapped
  hardware slot when different. This implementation deliberately refuses the
  multi-slot case instead of partially updating it.
- 170 `main.6703881a7216b3250128.js`: DeviceInfo around232100;
  `7846.8a75e5fe80ffbb4883e1.js`: independently checked tuples746500..747400
  and Smart Reel setter855292. Interface0 includes cable and receiver, never BLE.
- 164 `main.74812b00da458b9eb800.js`: DeviceInfo147236 and Linker feature
  selection near end. Cached `rzDevice25Linker.aef7d6b63b7e973349f7.js`:
  `_getTransactionId` uses E0 OR counter0..30. Matching SET/CLEAR or transaction
  headers are not ownership proof.
- Cached 196 `6120.c553d3491acbb7a4bf09.js`: general SDK and named Nommo
  wrappers are context only, not authority to enable every PID.
- Local OpenRazer reference `work/protocol/razerchromacommon.c` lines1552..1638
  records 02:14/16/17, storage1, binary values and packet examples;
  `razermouse_driver.c` lines2902..3042 and6718..6795 explicitly bind wheel
  controls to Basilisk V3 / Pro models and transaction1F.

REA cached SDK parent Evidence
`ev_ed26b5d1227f5d620b75f5d47262d6e0f3014fa871ac38707e29269b8acd6647`,
summary `ev_04e7e22cfcc4679fd69abcb9dc6fb644c6a420e7127c5ce85a4332af18aff6fd`.
Model analysis summary
`ev_d9bc4029b242a7b6c2ebf7dfd656bbf41c8318eee5a2b6c00f05161fc51052c6`.
Pro SDK parent Evidence
`ev_3db29350e7c664e2a8f52590054f22f611741f1e8ad91b1b6446d56306a781b8`,
summary `ev_de98677e3a5643f659204f4fd4ed00cd2635e9bc62c41d72937c01f9d68446ec`.
Viper boundary parent Evidence
`ev_84d8358ee21b3f59bb7ac182e2ec01d5c1dc6848da0139789e3233a7666d866c`,
summary `ev_6cf54bca9e2ec9226bd0657c5ae65cd0e2d3e3c4204ce18947fd016cfe13f01c`.
All inspected JS parses; application/semantic coverage remains partial, with
heap/resource limits and unresolved imports. These are static observations,
not runtime validation. Subsequent 170 SDK and122 definitions are individually
hashed in [the source ledger](validation/device-api-sources.json); no claim of
whole-application completeness.

## Command contract and acceptance gates

Wheel mode:02:14/94, two bytes `[slot, mode]`, tactile0/free-spin1.
Acceleration:02:16/96, two bytes `[slot, enabled]`.
Smart Reel:02:17/97, **80 bytes** in both selected shipped SDKs, first two
`[slot, enabled]` and zero padding. OpenRazer uses two; that discrepancy is
deliberately not hidden. Getter minimum is two meaningful bytes. Unknown values,
wrong selector, nonzero unknown trailing data, short/oversized responses and
unsupported status refuse mutation. Mode2/micro-tactile is not enabled merely
because a newer shared SDK declares it.

Model + exact USB interface + transaction1F + active onboard slot1 are required.
No minimum firmware is declared by these selected source definitions; successful
strict GETs determine availability for each control on the attached firmware.
No capability is inferred solely from a firmware version or catalog flag.
Writes capture original raw value, send once, independently read back, and on
failure restore that captured value and verify it. Failed recovery is explicit.
No automatic retry of a SET, profile store save, active slot switch, background
replay or polling worker is introduced. Recovery does not claim ownership over
other clients; concurrent stock controllers must be isolated for physical tests.

| Firmware/capability boundary | Admission |
|---|---|
| 0099 wheel mode, acceleration, Smart Reel | Selected153 definitions do not declare a wheel firmware minimum. Each GET must pass the strict register decoder on the connected firmware. No all-version claim. |
| 00AA/00AB wheel controls | Selected170 definitions independently declare these tuples. USB collection0 and successful per-control GET are required. High-polling firmware eligibility is a separate capability and is not granted here. |
| 00AC Bluetooth / 00A4 dock forwarding | No admitted wheel route, regardless of firmware version or paired product name. |
| 025D / 00C4/00C5 existing controls | Retain their existing specialized firmware/identity gates; this module cannot override them. |
| Updater/latestFirmwareInfo fields | Maintenance metadata, never a capability minimum and never authority to flash. |

## Remaining acceptance work

Source-supported and simulated coverage must remain separate from physical
coverage (none for these new APIs). TODO: on owned Basilisk V3 and Pro hardware,
record firmware, exact HID collection/report descriptors, active-slot reads,
all three baseline registers; perform user-authorized one-control round trips
with raw restoration, mechanical/scroll-event observations and reconnect.
Test USB cable and receiver separately. Do not capture personal input unattended.
If active slot differs from1, next diagnostic is read-only register/slot mapping
evidence before implementing atomic multi-slot changes. No all-device parity,
driver replacement, optical fidelity or complete-build memory claim follows.

## UI and simulation

The native Advanced page exposes the three controls only on admitted USB paths;
other products retain their existing page. The browser editor has a Scroll wheel
section. An unsupported individual GET disables that individual control with its
reason. Slot changes disable the controls without switching slots. This small
native panel uses existing typography/buttons; it is not a source-faithful
Basilisk Customize layout. TODO: normalize owned product153/170 captures and
port the wheel card geometry, wording, dependency rules and profile-slot policy.
No approved installed-product/shared-tab styling was changed.

`SECBIRD_SIM=basilisk-v3` and `SECBIRD_SIM=basilisk-v3-pro` provide explicitly
configured register fixtures through the real packet codec. They are synthetic
devices, not evidence that physical hardware accepted these commands. Generic
mouse simulators do not acquire the new registers automatically.
