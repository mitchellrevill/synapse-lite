# Explicit Studio device playback

The native Studio toolbar now has **APPLY SAVED** and **STOP DEVICE**. Apply uses the
selected saved archive entry; unsaved edits must be saved first. It prepares an
authenticated physical frame session and runs it on the existing cooperative
lighting worker. Successful apply also starts the independent local canvas preview.
Later draft edits affect preview, not the applied device snapshot. Stop requests
packet cancellation immediately through a cloned control before queuing its worker
acknowledgement. It remains available while another Studio file/apply job is busy.

Current physical admission is exactly one connected Ornata V2, interface2, existing
Driver Mode3 and profile0/region0 CustomFrame8, edition0/default saved geometry with
an authenticated supported source layout. Existing Static/Spectrum/Fire composition
layers are supported. A matching catalogue name or artwork cannot enable output.
No automatic mode acquisition, input subscription, driver installation or normal-mode
reset is performed. A conflicting/unknown owner, missing retained brightness policy,
read-only run, absent worker, changed connection or invalid asset/composition refuses
apply or blocks further output. The manifest is pinned by the existing preparation
loader, not accepted from the composition's declaration.

## Shared worker and handoff

There is one existing lighting worker, no per-frame spawn and no extra parsed profile
Store. Studio reuses the validated uniform-worker cache to obtain brightness/ownership
and idle/display policy. Fresh policy/owner sampling precedes each frame. Compilation
uses the immutable archive separately from that device profile. Per-frame playback
does not reload the full Store through the brightness bridge: positive LIVE reconcile
runs after the first accepted frame and after a suppression/owner pause resumes.

Only the selected target yields its Quick frame writers. This covers the uniform/Wave
candidate loop, periodic Static/per-key refresh, Static brightness preparation and
profile activation's lighting projection. Other devices continue their own Quick
programs. Profile activation still applies its non-lighting settings. Saved Quick
intent is not overwritten. Explicit conflicting lighting actions, including PID-alias
requests and global sync, are refused before mutation while Studio holds the target.
Automatic brightness/idle/display policy continues. Interactive brightness uses a
separate [Studio brightness path](STUDIO-BRIGHTNESS.md), preserving the composition
and saved Quick program without a stop/restart.

The existing worker's batch admission encloses the complete synchronous Studio
transfer. Ordinary worker quiesce closes admission and drains an admitted batch;
timeout cancellation and explicit Studio Stop are checked between packets. A syscall
already entered cannot be interrupted. Completion is consumed before releasing the
target. A partial/error completion latches the Studio session without automatic retry
or Quick fallback; explicit stop/reapply is required. Source mode/effect observations
are not an interprocess ownership lease, firmware ACK or optical verification.

After stop/drain, old Quick sessions/prepared-Static records are invalidated so the
retained declared Quick path can prepare afresh. Opted Spectrum resumes on the same
worker; Static resumes on the next policy/frame refresh. This is declared-policy
handoff, not restoration of arbitrary old RGB or animation phase. Unsupported or
unopted Quick branches still report their ordinary limits. New handle generations,
worker replacement/stop and lighting shutdown invalidate Studio. Reconnect never
silently replays the old compiled plan.

## Native feedback and validation

The device worker sends the UI a control handle, current global output state and a
small status-cache reader. Opening/reopening Studio does not acquire the device
mutex on the UI thread or claim a new output owner. Cached status reads never access
HID or refresh producer time. Old observations are marked old; a stopped signal
cannot leave cached Studio feedback claiming a live worker. Fault transitions publish
immediately. Cached counts remain transport observations, including after worker stop;
reading them does not assert that a pending batch drained. Editor close/navigation
does not stop global device playback; tray shutdown follows its existing join policy.

Validation uses protocol simulators with physical discovery disabled. The actual
stock export and pinned manifest exercise production preparation with mock identity.
Checks cover selective two-keyboard playback, all retained Quick writer paths,
Static and opted Spectrum resume, profile-byte preservation, suppression and owner
conflict, frame-before-positive-LIVE resume, failure latching, mode/read-only/worker
refusal, connection and worker replacement, status liveness and normal scheduling.
The native HWND test clicks Apply and Stop, observes accepted shared-worker frames,
verifies Quick resume, refuses an unsaved edited composition and preserves original
export/profile bytes. Existing native actual-export, layer-authoring and failed-save/
exit-recovery checks also pass. An owned-window render was inspected; it is not a
desktop screenshot and omits child edit controls. No physical or optical output
claim follows from these checks. See `validation/studio-worker.json`.

## TODOs and current limits

- Validate actual keyboard playback, optical RGB/timing, stop/failure recovery and
  real unplug/sleep/wake. Driver mode acquisition remains tied to input coordination.
- Physically verify live brightness editing, including zero/off/on during playback
  and suppression, without changing saved brightness, mode or input state.
- Persist the global Studio selection and its saved archive reference, with verified
  launch/reconnect behavior. Current explicit in-memory output survives editor close
  but is not automatically restored on application restart or reconnect.
- Add explicit target selection/multi-device compositions and the remaining effects;
  extend families/transports only from recovered capability contracts.
- Complete chassis/key legends, stock palette and widget behavior, canvas appearance
  and responsive placement of toolbar controls at small window sizes. The inspected
  Studio render still differs visibly from stock; pixel-perfect acceptance is open.
- Finish full profiles/macros/input coverage, network checks and three complete-build
  runs under100,000,000 bytes for both working set and private memory. No partial
  renderer, mock-window or historical memory measurement closes that requirement.

If an exact behavior stalls, retain the evidence and next validation step as a TODO
and continue another useful feature, as requested. These limits remain part of the
full implementation goal; this checkpoint does not narrow its acceptance criteria.
