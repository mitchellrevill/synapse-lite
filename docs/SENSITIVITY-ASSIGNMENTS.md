# Sensitivity assignment forms and local drafts

Customize now provides the installed mouse's sensitivity choices: Clutch, Stage Up,
Stage Down, On-The-Fly, Cycle Up and Cycle Down. Clutch has linked/independent X-Y
numeric fields and sliders. The existing device specification supplies min/max/step;
missing or malformed capability data leaves the controls unavailable. Numeric blur
clamps integers without step rounding; sliders quantize to the device step. Blank
input restores the previous value. Visible desired stages are summarized for stage
actions, with navigation to Performance using the existing dirty-close guard.

The DPI-cycle button excludes Clutch and On-The-Fly. Hypershift excludes On-The-Fly.
Primary click and wheel rotations do not gain sensitivity assignments. These are
source input/layer filters, not new device support declarations. Keyboard sensitivity
remains unavailable pending recovery of its cross-device target/capability contract.

## Persistence and delivery boundary

**SAVE DRAFT stores local intent only. No assignment is applied to a device.** The
form and worker status say this explicitly. Installed-family mapping delivery remains
unverified. The new endpoint `/api/sensitivity-draft` checks the selected device,
active profile and complete persisted profile snapshot, validates the input/layer
and device bounds, then atomically saves a candidate store. A failed save does not
replace in-memory state. Read-only measurement refuses this endpoint.

Intent lives in the profile extension `_synapseLiteSensitivityDrafts`, keyed by
source input ID and layer. It is separate from `buttons` and retained Synapse source
`mappings`; neither executable settings nor opaque source records are rewritten.
Unknown draft schemas are refused rather than silently stripped. Reopening the
input selects its saved sensitivity draft. Ordinary profile apply ignores this
extension. Native backup retains it; delivery and stock export of these drafts
are deliberately not claimed.

## Shipped-source evidence

The retained local cache supplied these files; they were parsed as inert text:

| Product | Bundle | SHA-256 |
|---|---|---|
| 196 | `MapSensitivity.429bbfa9.chunk.js` | `7e164e6569622a26a0f5154a18c931d1ecfbeeee3545f566f4fa073fa0900b8b` |
| 605 | `MapSensitivity.264336b8.chunk.js` | `c58dda7c7cae0e1256f784b199fa341939ee3231195c30ee1f911691fe4a2b78` |

Source196's `buildDataSet`, `getMappingData`, `renderSensitivityBody`,
`renderSensitivityClutch`, `toggleXYEnabled`, `renderStages` and numeric
`handleBlur` establish the conditionals and fields. Its numeric component receives
min/max but no step prop; its slider receives `DeviceInfo.dpiStep`. The shared
dataset in `main.bbc961f9.js` fixes option order and identifies `GGY` as On-The-Fly.
REA parent evidence `ev_6c26eea31cac9b0c5b71a8a2dab0ce98e25ce31158725ecf056a80cca431f54a`
inventoried both focused bundles (23,127 text bytes, four modules, zero parse
failures). Static semantic coverage is partial/unknown (1,420 unknowns); this does
not prove execution, hardware delivery or pixel parity. No vendor bootstrap ran.

## Verification and remaining acceptance

Core tests cover source filters, capability bounds, linked-axis consistency,
atomic save/reopen, stale snapshot refusal and preservation of executable/source
and opaque fields. The form-model test covers 401 DPI without rounding, 99,999
clamping to 26,000, rejected nonnumeric input, blank restoration, pending-draft
reopen and normal/DPI-button/Hypershift option lists.

The opt-in `native_sensitivity_clutch_save_reopen_and_filters` test opens one owned
HWND using an isolated store and simulated transport. It drives actual rendered
hit targets and EDIT notifications for category selection, clutch numeric and X-Y
controls, local save, reopen and filtered dropdown choices. It captures the owned
window and verifies zero simulated SET commands, unchanged executable/source and
opaque fields, and no physical event paths. Run alone with `--ignored --test-threads=1`.

The checkpoint passes 575 core, 323 app, five CLI and one documentation test;
six core and 58 app opt-in tests are skipped in that ordinary run. Both release
binaries build with `cargo build -j 1 --release --workspace`. The final explicit
native run passes with 28 simulated reads and zero SETs, including Y-slider
movement/step/axis isolation. Two focused core tests also pass after adding
future-draft-schema preservation assertions. Captures are 1600x900 client pixels
at 125% DPI. Early reruns exposed stale test-frame readiness and a missing slider
diagnostic; the fixture now removes its own stale frame and retries busy native
diagnostic reads before using rendered hit targets.

Remaining TODOs, explicitly outside this bounded local-draft checkpoint:

- Recover/prove installed mouse layer mapping delivery/readback and clutch hold/
  release restoration before enabling assignment application. No input hook,
  injection, driver, service or physical device write is added here.
- Recover keyboard sensitivity target selection and cross-device capabilities.
- Integrate accepted mapping delivery with source export/re-import and lifecycle
  recovery; promote a saved draft only through an explicit verified operation.
- Match stock sensitivity screenshots at identical geometry and DPI. Native
  screenshots verify visible states, but the inventory has no matching Sensitivity
  capture; form spacing, slider thumb tags, hover/focus and dropdown typography
  are not pixel-parity accepted. Existing approved tabs are unchanged.
- Complete catalogue/hardware acceptance and three complete-build memory runs
  below 100,000,000 bytes. This feature's tests/build do not establish that goal.
