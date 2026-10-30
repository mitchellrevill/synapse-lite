# DeathAdder V3 HyperSpeed connection lifecycle

This change completes a deterministic software boundary for the existing 00C4
wired / 00C5 receiver controls. Actual receiver operation, battery transitions,
sleep/wake and optical output remain unverified. It does not establish the
complete product or memory acceptance goal.

## Identity and cancellation

The existing profile store resolves 00C4 and 00C5 aliases only for an exact,
nonempty serial. A receiver with an empty/placeholder protocol serial is now
excluded from discovered mouse targets. Shared 00B3/00A4 receivers retain their
own identities; paired-product replies do not establish a DeathAdder serial.
No cached discovery result establishes that an unanswered endpoint is sleeping.

Each opened Device receives a process-local connection_id. Reopening the same
PID, serial and path produces a different token. Tokens are not saved to the
profile store. Power, Calibration, Dynamic Sensitivity and Rotation POSTs require
expected_connection matching the current device, in addition to their existing
device/profile checks. Clients obtain it from the device state response. The
native editor captures it before queuing; sensor actions use the token from the
sensor snapshot, and Power gestures keep the token from pointer-down. Old
requests fail before hardware or profile access. The retained browser client
also supplies the connection token.

These actions re-read the exact protocol serial before mutation. The core Power
setters perform their own serial preflight, and engine profile/reconnection
application also checks identity. This is a best-effort preflight, not atomic
ownership of firmware or vendor-filter state; other clients can race any
read/write sequence. The existing filter ownership and firmware gates remain
unchanged and must not be treated as an accepted replacement input driver.

## Power and reconnection

The Power page exposes independent battery and charging observations, including
unavailable values. The API power_status object includes transport, identity
status, battery_percent, charging and per-read errors. A charging-read failure
does not become false; tray low-battery notifications are suppressed while
charging is unknown. Battery percentage retains the existing protocol conversion.

The sleep and threshold controls continue to use the existing source contracts
in [POWER.md](POWER.md): exact raw seconds and threshold readback, device-wide
scope, and the effective polling-rate gate for threshold edits. A threshold is
configuration; it does not prove the current low-power state. sleeping and
low_power_active are null until an established runtime status contract is
available. No new low-power selector, firmware minimum, pairing command or flash
command is invented.

Before replacing handles, rescans restore owned Rotation using the existing
controller, invalidate Studio/Uniform handles and lighting acknowledgements, and
discard cached DPI, foreground-profile observations and pending relight retries.
Scoped mouse handles are returned for connection replay even if their key did
not change. The existing arrival/resume path then applies profile intent and
device-wide Calibration, subject to its existing firmware, owner and read-only
gates. Replay does not save/migrate the profile file. Power configuration remains
hardware-owned and is not silently overwritten by profile activation.

If the endpoint has already disappeared, exact hardware restoration cannot be
acknowledged. Existing restoration errors are reported; this change does not
promise that a disconnected device was restored. No restoration is redirected
to an unrelated serial or shared receiver. This mouse's database entries expose
no lighting zones, so no mouse LED protocol is added. Existing lighting session
cancellation and invalidation remain in use.

## Evidence and remaining diagnostics

This implementation uses repository contracts and the previously recorded
source provenance in POWER.md, MOUSE-SENSOR.md and ADVANCED-FEEDBACK.md.
It performs no physical sampling.
The recorded [connection identity check](validation/mouse-connection-identity.json)
found one responding wired collection and no eligible 00C5 collection. Its
two-serial comparison remains null. Wired register results cannot prove the
receiver route. The numeric firmware gates in sensor_rotation_control.rs are
retained; no firmware threshold for new commands is inferred.

A bounded static follow-up inspected the cached middleware with SHA-256
`df8334aa3021d129893918cecb3c1d14b3727f476c42981bac3fb02f6736b633`.
The full-file REA attempt exhausted its 256 MiB worker heap and is partial
(`ev_bc8ae7e2d688529b7c27faa70a648e7e12adc7b7b0b96cc21525c87e89f554ef`).
A focused inert excerpt parsed successfully with zero parse failures
(`ev_cd6ef7855ee4d57d731ec6287eff0955a90ed991e1a04431b3f62556ebf1e391`).
It retains the getter at character offsets 698191..698469 and its constants
at 701336..701394, adding only declaration prefixes/terminators. Ordinary
original-source reads join exports m9/pJ to re/ae: a generic
getLowPowerModeState helper sends one argument byte with GET07:95 and returns
data[0]. That command also shares a selector with a differently sized indicator
configuration helper. This is a static source observation, not evidence that
the DeathAdder's receiver/firmware supports it or that any raw value is a Boolean.
The focused excerpt leaves its framework dependencies unresolved. No model196
call path or firmware gate was established, so the implementation does not send
this generic selector. The next diagnostic is its product-specific call path
and raw-state enum, before any user-attended hardware validation.

Deterministic tests exercise unavailable I/O, placeholder and changed serials,
shared receivers, stale connection tokens, independently failing charging
queries, same-key reopen, 00C4-to-00C5 reconnect, calibration failure rollback,
Rotation restoration/reacquisition and byte-identical saved profiles. They use
simulated transports and isolated temporary files, never normal profiles or
physical HID handles.

The opt-in native Power and Calibration HWND tests passed with an empty asset
directory and simulated transport. A Power run loading the private device pack
timed out at its four-second initial-frame wait even though a valid Power frame
was subsequently written; the empty-assets run isolates functional routing from
asset loading. This is not visual acceptance.

The optional native Advanced test passed its enable/mode/curve steps but timed
out at Jump template dropdown selection. Its cause is unresolved; no claim is
made that it predates this change. Next diagnostic: capture the existing
RUNTIME_TEST_QUERY pending/error/snapshot-token and dropdown-highlight fields
immediately before/after the Up/Enter messages, then compare with committed main
under the same asset/load conditions. Do not blindly repeat the timeout or infer
that the later native Rotation steps passed. The deterministic engine Rotation
restoration/reacquisition tests are independent of that uncompleted UI workflow.

Physical TODOs requiring the user:

- Establish the actual 00C5 protocol collection/descriptors and exact protocol
  serial, then compare it with wired 00C4. If absent/ambiguous, record the changed
  setup before further sampling; do not repeat the previous unchanged probe.
- With the user controlling unplug/sleep/wake, check connection tokens, absence
  handling, unchanged active profiles, restored Calibration and gated Rotation
  on each proven transport. Verify old queued actions fail after reconnect.
- Observe battery/charging changes and actual sleep/low-power transitions using
  an established source GET contract. Next diagnostic for low-power status is
  a focused REA trace from the generic runtime getter to a model196 caller and
  raw-state enum, retaining its Evidence ID and declared payload before adding
  any selector. Threshold
  readback alone is insufficient.
- Compare the Power status row and existing cards with normalized source
  captures at supported widths/DPI; the added status row is functional UI,
  not a claim of source pixel parity. Perform optical checks only with the user.
- Validate complete-build memory across three representative runs separately.
  Pairing, firmware flashing, driver installation and personal input/audio
  capture are excluded from unattended tests.
