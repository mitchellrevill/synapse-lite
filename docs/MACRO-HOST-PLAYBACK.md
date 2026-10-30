# Manual host-action macro playback

On Macro, save or discard the selected draft, then choose **PLAY ONCE**.
Saved commands, programs, websites and fixed delays now execute in source order.
**STOP** wakes a pending delay and prevents subsequent actions/retries. A program
already accepted by Windows keeps running. This explicit editor action performs
one pass and does not rewrite playback options or enable device-bound hotkeys.

The entire definition must be admitted before any action runs. Text, keyboard,
mouse, motion, random-delay, loop and nested-reference playback remain unavailable
in this checkpoint; a macro containing unsupported events refuses as a whole.
This keeps a preceding program launch from running before a later unsupported
event is discovered. Empty definitions produce a notice. Import and selection
never trigger execution. Read-only mode, an open form, an unsaved draft or another
active run prevent admission.

Filesystem and Windows errors can still occur during execution. An earlier
accepted launch is not rolled back; a later failure reports that accepted count.

## Saved data and execution

The worker reloads the bounded native Store, checks the selected GUID/revision,
and uses the existing source/cache/hash compiler. It verifies every native launch
request before starting. It does not save the Store, repair caches, modify a device
binding, acquire an input route or hold the device engine mutex. Source GUI row
indexes remain attached to compiled steps. Events remain compact under the
existing512-event,256KiB native-JSON and one-hour explicit-delay bounds.

The [native launch backend](NATIVE-HOST-LAUNCH.md) returns OS acceptance; playback
does not wait for a launched program to exit. Source50/120ms launch delays and
explicit delay rows start after the preceding operation returns. A condition
variable wakes delays on Stop without polling. A race after acceptance reports
that accepted launch; it does not pretend it was canceled or terminate the app.
Clipboard/paste and mixed input still require their separate recovery contracts.

## Lifetime and UI

One process-wide registry retains the actual JoinHandle. Stop leaves admission
closed until `is_finished` proves the thread ended and its result is joined.
There is no completion message containing an HWND or a State pointer. The editor
observes bounded progress at40ms intervals; an unchanged snapshot does not repaint
the whole window. A worker panic reports uncertainty rather than a false zero
launch result. Lock contention remains pending rather than losing a session.

While playback runs, draft edits and selection changes are disabled. Leaving
the page or closing the window requests Stop and continues the requested action
after drain. Once Close is queued, later navigation cannot replace it. Before
Close is requested, navigation can still change the destination while stopping.
Unexpected window destruction also requests Stop; the global registry
retains the orphan's handle and can reap it on the next start. It does not detach
an uninterruptible OS call or admit an overlapping run. A blocked OS launch can
therefore keep playback in Stopping; it does not block the device worker or
claim that recovery finished. Cross-process macro/input arbitration is not
established by this process-local registry.

## Verification and visual comparison

Ten worker checks cover acceptance-driven delays, late unsupported-event refusal,
source preservation, Stop during waits/in-flight calls, stale revisions, read-only
mode, failures, orphan reaping, panic uncertainty, pre-start Stop and contention.
Nine CLI and318 serial native-app checks pass;49 opt-in app checks remain ignored
in ordinary validation. All ten optimized worker checks and the separate optimized
native workflow pass. Both release binaries build.

The native workflow clicks the actual Play/Stop buttons in an isolated simulated
editor, executes only the explicitly built owned fixture, verifies one accepted
launch/two native steps, refuses read-only/dirty/mixed/stale requests, interrupts
a long delay, closes after drain, and reuses admission after close. No physical
discovery occurs and the simulator records zero device commands. Playback leaves
source/profile bytes unchanged; the test's controlled external rename is a
separate setup mutation used to verify revision refusal. See
[validation and artifact hashes](validation/macro-host-playback.json).

The debug and optimized playing captures were inspected against the inventory's
stock macro-editor capture. Play and Stop fit without overlapping the existing
toolbar. The macro page remains an interim native layout: stock action sidebar,
record toolbar, menus/popovers and the Macro module icon still differ. Child edit
controls are verified through HWND interactions; the painter capture does not
composite their text. The reference is the historical4.0.827 inventory capture,
not a newly matched-DPI capture of installed4.0.896. Pixel-perfect macro-page
acceptance is still open. Approved tab styling was not revisited.

## Remaining scope

Finish clipboard ownership/paste/recovery, actual keyboard/mouse routing and
physical-held arbitration, software/device bindings and Hypershift, other playback
modes, nested/loop/movement/random events, and recording coordination. Independent
feature threads retain their assignments; this checkpoint does not claim their
work is merged. Complete catalogue API/hardware coverage, network/privacy checks
and all three representative complete-build working-set/private-memory runs below
100,000,000 bytes remain required by the active implementation goal.
