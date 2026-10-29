# Brightness during Studio playback

The existing Lighting brightness slider and switch now work while Studio owns the
keyboard. They retain the selected saved composition and its animation generation;
there is no Quick frame projection, effect selection or Studio restart. Brightness
intent is persisted in the selected device profile, as with ordinary Lighting edits.
The physical saved-brightness register is not written; the host policy bridge uses
LIVE brightness. Source profile brightness enable/value semantics are unchanged.

The route requires the exact selected device and active/effective profile, retained
source brightness settings, an admitted Studio target and the running writable
worker. Unknown/conflicting ownership, stop-in-progress, wrong mode/effect, stale
identity, missing settings and invalid values refuse the request. The brightness
action cannot replace idle/display policies or unrelated extension settings.
Those continue through their separate policy controls.

## Commit before output

The engine loads one fresh local Store candidate, validates the exact profile,
updates only brightness intent/projection and clears obsolete hardware-bootstrap
provenance. Aggregate limits and source owner/policy admission are checked. A
read-only mode/CustomFrame preflight follows; it never renders Quick RGB. Only a
successful atomic profile save invalidates the worker's profile cache and requests
the configured LIVE value. A failed save leaves the old file, prepared Studio state,
hardware brightness and animation untouched. No second full Store is retained.

Positive LIVE updates wait for an accepted Studio frame. Turning brightness off
can darken immediately without submitting a frame; it retains the slider value and
freezes animation phase. Re-enabling resumes with the next frame and then raises
LIVE brightness. Display/idle/away suppression still wins over a newly saved slider
value. The composition revision/generation is unchanged across these actions.

The new checks also found an activation gap in the previous checkpoint: accepted
Studio RGB could leave a previous LIVE0 unchanged, because passive policy observation
does not impose a configured brightness. Explicit Studio apply/resume now requests
configured LIVE reconciliation after its accepted frame. This preserves the existing
policy's passive-observation behavior elsewhere instead of globally raising lights.

After a partial Studio output failure, an off/zero brightness intent can darken the
held frame, but positive brightness is refused until explicit stop/reapply. It does
not retry the failed frame or silently resume Quick. Mode acquisition, normal-mode
reset, input routing, driver changes and reconnect replay are unchanged.

## Evidence and remaining work

Eight focused simulator checks cover 37 percent LIVE mapping with saved brightness/
effect/input controls unchanged; switch off/on retaining61; display suppression;
independence from unsupported saved Quick frame intent; identity/value/policy/owner
refusal; a read-only destination save failure; darkening a faulted partial frame
without retry; and worker/mode/read-only/stop refusal. The original actual-export/
pinned-manifest preparation test additionally checks LIVE0→configured brightness
after its first accepted mock frame. The existing native Studio workflows exercise
the same dispatcher/worker integration without touching physical hardware.

These are protocol simulations and code checks, not optical or physical brightness
verification. See `validation/studio-brightness.json` for the completed run counts.
Actual hardware recovery, remaining effects/device APIs, complete profile migration,
stock visual parity, network observations and the three complete-build100MB runs
remain part of the active goal. There is no UI layout change in this checkpoint.
