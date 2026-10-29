# Studio physical frames and output sessions

`host_studio` connects the existing Studio compositor to the authenticated Ornata
logical-to-physical map and the existing seven-packet frame format. This is a
backend component used by the explicit native [Studio output controls](STUDIO-RUNTIME.md).
Normal startup and local canvas preview do not start Studio device output.
Applying a saved composition now uses the shared worker under the admission limits
in that report; hardware/optical validation remains unfinished.

`PreparedStudio` compiles one immutable composition snapshot and retains its
originating draft identity. Current admission is the compositor's existing single,
unrotated Ornata V2 placement, edition 0, supported source layout, default geometry
and Static/Spectrum/Fire layers (later checkpoints add deterministic continuous
[Breathing](STUDIO-BREATHING.md) and imported cardinal [Wave](STUDIO-WAVE.md)).
The production constructor requires the manifest
loader's opaque `AuthenticatedLayout`; declaring a hash in a composition cannot
substitute for source authentication.

The composition's product/edition/layout must match the authenticated selection.
Its logical and physical dimensions, ordered physical projection and mapping entry
count must agree too. Output retains the authenticated mapping instead of trusting
the composition's declared table. Holes map black. One current frame yields six
physical 6x22 row packets followed by the profile 0/region 0 CustomFrame8 commit.
Reading a frame repeatedly never consumes animation phase; advancement is explicit.
The nominal interval is 40 ms, not a measured USB cadence or stock-phase claim.

The preparation child checks Ornata 025D/interface 2/physical 6x22 before any request,
then performs a fresh GET 00:86 and loads the exact selected source manifest. It
binds the result to the selected device key/path and records conservative monotonic
observation timestamps, declared identity size, source hash and byte length. It
performs no mode, brightness, input, macro, effect or profile mutation. Target
key/path matching is not an ownership lease, connection-generation proof or fresh
hardware-state admission.

## Physical-preparation checkpoint verification (`bbc3996`)

Seven scoped tests pass in debug and optimized builds, including the opt-in actual
export/private-manifest test with a mock device. The regular serial tray suite has
245 passing tests and 31 opt-in exclusions. No physical device was used here.

- All 500 Fire/Spectrum physical frames match an independent scalar reconstruction
  and ordered source projection: RGB MD5 `b0c46374f3ff3a91d3bf64f197e30345`.
  The existing logical reference remains `15c72331781274566ba1d29ebfc5b0d1`.
- The frames traverse the existing streaming adapter into the protocol simulator:
  3,500 OUT packets, exact simulated RGB matrix contents, unchanged mode, effects,
  brightness, gaming bits and macro state. Accepted mock OUTs are not firmware ACKs
  or optical evidence.
- Repeated frame reads, snapshot/session separation, incorrect physical maps,
  dimensions, identities, missing/untrusted assets and extra placements are covered.
  Preparation performs a fresh identity GET on every attempt and never guesses an
  edition or layout.
- Cancellation, wrong mode and failures at row/commit boundaries produce no hidden
  retry or phase advancement. A future live controller must latch failures and own
  cancellation/completion; preparation itself is not that controller.

See `docs/validation/studio-physical-preparation.json` and the synthetic numeric
fixture `crates/secbird-tray/tests/fixtures/studio-physical-reference.json`.
The reference generator is private local analysis code and invokes neither Rust
nor vendor programs. Pinned Ornata source manifest SHA256:
`3595e11f1d8778180d1dd6edd2463c8b3b8e446542c5020dc1fa038c67937a93`.

## Source lifecycle and remaining integration

Cached lighting-engine main.a383ea9e.js SHA256
`c82c7074e5da8abcffbff95f23a6b962c1e14b04586715b9d4b7a6f11fe9e6d2`
uses separate saved Advanced and draft Preview engines. The per-user global
synapseChromaStudioUsers enabled/currentProfile selection is separate from device
Quick effects. Applying Studio must not overwrite saved device Quick settings or
silently apply an unsaved editor draft.

Source handoff disables eligible Quick engines before enabling Advanced, and
re-enables Quick after disabling Advanced. Unsupported devices can keep their
Quick path. Saved product/instance/device identities are resolved against live
device metadata before output. Canvas preview is not hardware output authority.
The source frame callback's Promise.all is not awaited inside its named lock, and
destroy/exit syntax does not prove callback drain or exclusive ownership. Shutdown
uses declared Quick/default policy, not restoration of an arbitrary prior RGB frame.
These source behaviors are documented in private work/studio-output/OWNER-CONTRACT.md.

The session controller below implements applied revisions, generations,
completion/drain and partial-transfer fault handling. The subsequent
[worker/native checkpoint](STUDIO-RUNTIME.md) connects policy/owner sampling,
connection admission, selective Quick/Studio handoff and explicit apply/stop.
Mode acquisition must coordinate with the input path; copying a normal-mode reset
on exit would not prove input preservation. Wider devices, remaining effects,
physical/optical verification and the complete-build 100 MB gate remain open.
This component does not narrow the full implementation goal.

## Explicit applied-session controller, 29 October 2026 (`caa10bc`)

`host_studio::session::OutputSession` owns one authenticated, compiled composition.
Its production apply method reads an entry from an immutable `Archive`, creates
its own snapshot and performs fresh preparation. It does not accept the editor's
mutable draft. The applied composition ID and SHA256 of its compiled JSON snapshot
are reported separately from the device's Quick profile. Snapshot serialization
preserves raw values but can normalize whitespace; this hash identifies the
compiled revision, not the original interchange file's bytes.

An opaque apply intent belongs to one control instance and carries a monotonic
request sequence and stop epoch. Newer pending requests supersede older requests;
failed compilation does not cancel the last successfully applied scene. Stop
invalidates pending requests and current output admission without acquiring the
engine mutex. Explicit later apply can start a fresh generation. Counter exhaustion
fails closed instead of wrapping. Applying while a frame is pending is refused.

Each frame ticket retains the issuing control identity as well as generation,
sequence, frame index and start time. Numerically identical tickets from another
session are refused. A ticket admits at most one transfer and one completion.
Duplicate/stale calls cannot output or consume a newer pending frame. The caller's
connection epoch, exact device key/path and physical scope are checked before IO;
an unchanged key/path after reconnect cannot substitute for a new epoch.

The caller supplies fresh policy/owner availability. Suppression and owner blocking
freeze phase; late polls issue one frame without catch-up bursts. An admitted frame
uses the existing stream adapter's fresh mode/control checks, deadline and per-packet
cancellation. Only seven accepted/attempted packets with no error advance phase.
Any other completion, backwards completion clock or failed phase advance latches a
fault and holds the selected target against Quick until explicit recovery. An OUT
acceptance remains neither a firmware ACK nor optical evidence.

Stop retains that target while its pending batch drains. A syscall already entered
can finish after the stop request; no method claims to interrupt it. The owner must
consume completion and observe `Stopped` before releasing the target to its declared
Quick policy. No normal-mode reset, saved Quick edit, brightness write, macro/input
mutation or arbitrary prior-RGB restoration is performed by the session. It spawns
no worker or per-frame task. Existing worker batch admission must enclose transfer.

Verification: 21 focused debug and optimized tests pass, including both opt-in
actual-export/pinned-manifest checks with mock identity/output. The new session
checks include a genuinely blocked mock transport syscall: stop returns while the
owner is occupied, that row finishes, no next OUT is admitted, and completion drains.
The saved-export session accepts 25 frames/175 mock OUTs with source file and readable
Quick/input controls unchanged. All 258 regular serial tray tests pass; 32 explicit
opt-in tests remain excluded from that regular run. No physical output or GUI change
was enabled by this checkpoint. See `validation/studio-session.json`.

### Integration checklist at the session-only checkpoint

The subsequent [worker/native checkpoint](STUDIO-RUNTIME.md) implements selective
Quick inhibition, explicit native apply/stop, cache reuse, owner/policy sampling and
handle/worker invalidation. This list records the earlier session-only boundary;
remaining limits and TODOs are maintained in that newer report.

- Connect the coordinator to the existing single worker and native explicit apply/
  stop actions. Reuse the validated profile cache; do not add another full store.
- Inhibit *all* Quick frame writers only for the held target, including periodic
  Static refresh, profile apply and brightness's frame-preparation paths. Continue
  brightness policy and other devices. Test declared Quick resume after drain.
- Supply fresh owner/policy admission, worker batch lifetime, monotonic connection
  epochs and lock-independent stop control. Invalidate on actual handle replacement
  and worker replacement; do not automatically replay a stale plan.
- Keep global saved Studio selection separate from per-device Quick profiles.
  Launch/reconnect replay and persistence need their own verified implementation.
- Driver Mode acquisition needs input coordination. Existing mode3/CustomFrame8
  admission does not establish that a mode0 device can be safely taken over.
- Remaining effects, multi-device output, broader transports, physical/optical
  checks, layout parity and complete-build RAM acceptance remain separate tasks.

If an exact branch stalls, retain its evidence and next check here or in its feature
report, then continue another independent feature as the user requested. This policy
does not turn an unfinished function into a passing acceptance criterion.
