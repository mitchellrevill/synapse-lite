# Native Studio file editor

Open **Synapse → Chroma Studio** from the device dashboard, then **Open File**.
The native view can load a JSON-v1 `.ChromaEffects` archive, switch compositions,
select layers/groups, expand groups, rename and toggle visibility, undo/redo, and
save the edited archive. Group visibility updates children in one history step.
These operations edit local files only; they do not apply lighting to hardware.

Saving runs on the existing background worker before acquiring the device-engine
lock. A successful atomic write updates both the draft's saved checkpoint and the
whole archive baseline. Saving another composition therefore retains earlier
saved edits. Failed writes leave the prior file and working draft intact. The
Studio file limit is8MiB; device-profile exports retain their2MiB limit.

Home/device navigation keeps the Studio draft. Switching compositions, opening a
file, closing the editor and tray exit check unsaved changes, offering Save,
Discard or Cancel. Invalid pending names reveal Studio so they can be corrected
or canceled. The tray waits for editor destruction before completing an accepted
exit. No OS-shutdown/crash persistence guarantee is established by this flow.

The view uses the recovered Studio panel/row dimensions documented in
[STUDIO-DRAFT.md](STUDIO-DRAFT.md). This is a partial functional editor, not visual
or behavioral parity with stock Studio. Text actions currently stand in for some
source icons. Canvas, LED layout/selection, effect properties and algorithms,
gradients, add/delete/reorder/group operations, preview, device output and full
composition-management controls remain to implement. Actual stock file acceptance
and the user's saved composition have not been tested; current fixtures are
source-shaped synthetic JSON. Legacy ZIP/XML files remain unsupported.

## Verification

`native_studio_edits_save_failure_and_exit_recovery` is an opt-in real-window test
using only a simulated mouse and an isolated settings directory. It exercises
actual pointer hit testing, the native text field, async read/save workers,
composition switching, history, failed save, navigation, invalid hidden rename,
and tray-exit Cancel/Discard through a message-only window owned by the test.
It verifies unknown numeric literals and untouched compositions survive, earlier
saved composition edits survive later saves, the device-profile file is unchanged
and no device SET command was submitted. It also captures the application's own
production paint output for review; this is not a stock pixel comparison.

The file-picker selection and unsaved-modal button results are injected at the
test boundary; Windows' common file-dialog UI is not automated by this test.
Other paths use the same production dispatch, worker, writer and close guard.
The serial core/CLI/tray suites and optimized builds also pass. The full-goal
three-run100MB memory target remains unverified.
