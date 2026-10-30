# Temporary Identify checkpoint

Implemented on isolated branch `feat/identify-bounded`, based on `7b99930`.
Canonical checkout was not changed.

## Behavior

`lighting_identify::Identify<T>` is an independently testable lifecycle with a
nonrenewable maximum five-second deadline, device or LED identity, cancellation,
opaque prior-intent retention and explicit restoration acknowledgement. Timeout,
cancellation and caller-reported output/ownership failure enter Restoring; another
Identify cannot overwrite the prior intent. Failed restoration leaves the snapshot
available for retry. No profile serialization or persistence is involved.

This is a passive state machine, not a hardware worker. A future worker must poll
before each frame, cancel on disconnect/shutdown/owner loss, retain the object
until restoration completes and reject stale/replacement identities. Drop cannot
perform asynchronous restoration. The five-second maximum is a conservative Lite
policy, not a recovered stock duration. Boolean admission arguments are caller
obligations, not an ownership lease.

Studio canvas right-click on device geometry exposes disabled Identify Device;
clicking an admitted LED also exposes disabled Identify LED. The menu and status
explain that ownership/restoration are unverified. These entries send no job,
write no packet, and do not edit the composition or selection. Native menu styling
is interim; no pixel-perfect or screenshot-comparison acceptance is claimed.

## Evidence and supported protocol boundary

Recovered cached stock `EditorCanvas.21953e06.chunk.js`, SHA256
`190a2ef92615861142a1585b6cbffa578d2a716069f8d430cb6bd63ca103e5c6`,
contains the context-menu Identify button when contextMenuProps.id exists. Its
identifyDevice method resolves that ID and passes productId, instanceId and
deviceId to m.J1. Recovered `main.88ac681e.js`, SHA256
`109b483f129044e1c7c17b9fcae0e30fa775885667976807cf2c94bd1c696d19`,
constructs `{action:"identifyDevice",payload:e}` and sends it via
setMemoryStorageItem. These are static renderer observations, not execution or a
USB command definition. No per-LED Identify contract was established by these
bounded searches. The LED entry is a disabled requested extension.

REA analyzed the isolated cached EditorCanvas tree without executing JavaScript:
parent `ev_f8ec1c065f7e29e33fc8e257ee7ed86da7d803c8dac9f20ac6fdc155f524da56`,
summary `ev_d9573f079a4e884517b9dc06f96ac18c8dfb6e40ccbad8b7361f464ba7bfcfbc`.
One source file parsed, zero parse failures; semantic coverage remains unknown
with 6021 semantic unknowns. These limits do not prove native dispatch behavior.

Existing HOST-STATIC.md establishes supported Ornata frame output but explicitly
states RGB buffers/previous animation phase cannot be read back and process checks
are not an ownership lease. Studio output admission is exact Ornata-specific,
not catalogue-wide support. Neither existing transport establishes exclusive
ownership or exact intent restoration for a temporary override. No device writes,
stock process stops, driver actions or optical tests were performed.

## Validation and remaining TODO

- Five focused core tests pass: exact raw bytes, boundary timing, repeated ticks,
  refusal, cancellation, retained failed restoration and reuse after acknowledgement.
- `cargo check -p secbird-tray --jobs 1` passed (isolated target directory); existing warnings remain.
- TODO: recover native handler/protocol for identifyDevice, including duration,
  sequence and cancellation; prove a per-LED target mapping before enabling it.
- TODO: coordinate output ownership and snapshot complete Quick/Studio intent,
  brightness, suppression policy and animation state; integrate the lifecycle into
  the existing worker with partial-write, disconnect, shutdown and restore failure
  tests. Do not use mode/brightness readback as proof of full restoration.
- TODO: capture isolated native menu and stock menu screenshots and compare layout;
  no live stock UI parity or hardware support is claimed in this checkpoint.
