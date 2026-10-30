# Local device information

The native device toolbar has a Device info button. Opening it first queues a
read-only snapshot on the existing serialized device worker. Reopening displays
the session cache immediately. In the native dialog Yes refreshes, No copies the
full text to the Windows clipboard, and Cancel closes it. The explanatory text
names these actions. Snapshot time and the process-local connection generation
are included. No private HID path is emitted. Serials are copied only by request.

Product name, VID/PID, control interface, connection route, discovery serial and
catalog capability declarations are local data. The declarations explicitly do
not claim hardware verification. Unknown routes remain unknown.

Live identity is admitted only on Ornata025D/interface2, DeathAdder00C4/00C5 /
interface0 and Basilisk0099/interface3. This reuses the existing SERIAL GET00:82
and two-component FIRMWARE GET00:81, not the extended firmware fallback that
pads unknown build components with zero. Identity mismatch/empty/error suppresses
the firmware read. Other families receive no additional device requests.
Sources: core commands.rs, docs/DEVICE-API-MATRIX.md, docs/ORNATA-IDENTITY.md and
existing scoped mouse sensor firmware/identity code. No vendor downloads,
firmware writes, settings writes or profile changes are added.

Limitations / TODO:
- Cache is explicitly a historical snapshot. A reopened physical connection does
  not automatically update it; Refresh reads the currently enumerated handle.
  The existing global rescan is required to acquire a replacement handle.
- A failed GET cannot distinguish sleep, unplug and exclusive ownership; its
  error is displayed without asserting an exact physical cause.
- Firmware only exposes the two established components. Build/internal version
  remains unknown. Other families need independent adapter evidence.
- This is an accessible Windows message dialog with standard Yes/No/Cancel
  controls, not a stock Synapse visual clone. Stock screenshot comparison and
  live native interaction/hardware tests remain TODO. No pixel parity claim.
- No stock runtime was stopped and no intrusive hardware test was performed.

Validation: focused simulated identity admission, GET-only snapshot, unknown
family zero-query, changed serial suppression and disconnected error tests;
Windows tray cargo check (results recorded with the implementation commit).

Results: all three focused core tests passed (579 filtered out); Windows tray cargo check passed with existing unused/dead-code warnings. git diff --check passed. Live screenshot and hardware validation were not performed.
