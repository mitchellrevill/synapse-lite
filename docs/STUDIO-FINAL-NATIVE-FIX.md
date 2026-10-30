# Final Studio native regressions

Base: immutable `ec38ed4f84b2b80912dc501b731f6a0e63a3de54`.
Branch: `fix/studio-final-native-regressions`.
Worktree: `C:/Users/mitch/Documents/Codex/synapse-fix-studio-final`.

The viewport/save-recovery failure was restricted-context setup: the unchanged
input implementation passes in a host simulated window. No production viewport
or keyboard semantics were changed.

Spectrum's added drag/scroll fixture still used the older vertical canvas center
for two Esc clicks. Both use the current `(118+height)/2` center now, matching the
existing canvas transform and other fixtures. LED identity, read-only, source
preservation, exact save/history, drag cancellation and scrolling assertions stay.

Wave's invalid-input checks exposed an asynchronous UI error bug. A successful
preview reply installed its plan and unconditionally reset `failed`/`status`,
even when an invalid HSV or motion edit had produced a newer error while the
compile was pending. A combined host run reproduced this at Invalid HSV, although
Wave passed alone. Background preview progress/completion now preserves that
error. Valid frames are still installed; document/history and snapshot admission
are untouched. An explicit preview toggle can acknowledge the previous error,
so a deliberate successful retry is not stuck showing an old failure.

The actual plan-acceptance path has a regression using a real compiled Preview
and exact draft/history checks. It failed before the guard with `background
success erased the validation error`, then passed after the fix. The existing
pending generation/mode lifecycle regression also passes. No assertion or effect
validation range was relaxed. Failure messages now include current diagnostics.

Validation: one-job native test-package build in its own target; two focused
preview lifecycle unit tests passed; all nine native Studio-module tests passed
serially in one host process (18.08s), including all three requested cases. The
pre-fix combined sequence had eight passes and a Wave invalid-HSV failure. Tests
used explicit private assets/export, owned TEMP/TMP and simulated devices. Existing
source/profile preservation and simulated-output guards passed. The saved-output
workflow intentionally exercises mock output, not physical hardware.

No root/canonical checkout, Wheel/Tidal module or moving integration head was
edited. No push/new chat/release rebuild. Root is already building release and
owns the final combined run after merging this and the separate Wheel/Tidal fix.
Existing visual, physical/optical and complete memory acceptance remain open.
See `validation/studio-final-native-fix.json` and the supplied test logs.
