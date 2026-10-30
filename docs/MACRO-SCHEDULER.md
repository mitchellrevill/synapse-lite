# Completion-driven macro scheduler

The current scheduler also supports balanced mixed keyboard/mouse passes, with
explicit per-class contexts, a destination on each command, mouse button
ownership and one-shot wheel completion. See
[MACRO-MIXED-OUTPUT.md](MACRO-MIXED-OUTPUT.md) for the current interface and
verification. The keyboard-only constructor remains. Refused cleanup now
continues other eligible releases before reporting unconfirmed recovery,
rather than stranding the whole class. The specific failure is reported once
before the terminal unconfirmed stopped state.

## Original keyboard-only checkpoint

The text and counts below describe the initial checkpoint; the linked mixed
output document supersedes its mouse admission and refusal limitations.

`secbird_core::macro_scheduler` joins the existing source macro plan, native
keyboard packet planner and held-output ledger into a bounded execution state
machine. It has no input sender, driver handle, scheduling thread, clock reader,
file access or Drop-time I/O. The app does not activate live macro playback from
this checkpoint. A native adapter and hardware acceptance remain required.

The current admission scope is one balanced keyboard pass with at most512 source
steps. Source events retain native scan-code/prefix fields. Every native context
requires an explicit routing identity, modifier mask and optional source UnitId;
there is no Default. Omitted source UnitId remains the recovered native0, and
must match routing. A nonzero routing value is never inferred into source fields.
Metadata conversion and conditional packet detection reuse the existing bridge.

Mouse events, repetition, repeated makes, orphan releases, keys held at the end
and conditional E1/Pause packet chains refuse before execution. They are required
future work, not silently skipped or converted into a different macro. Concurrent
macro/physical-key ownership arbitration is not established by the private ledger
of this single-run scheduler. No selector-owner token or physical-device permission
is inferred from a local owner value.

## Execution and recovery

The caller supplies a nondecreasing monotonic timestamp at serialized entry.
Polling returns one new command, a pending state, a deadline or a terminal state.
It does not return the same command on repeated polls. The adapter must mark
dispatch immediately before its native API call, then report terminal completion
separately. Submission acceptance is not Delivered. Stale/duplicate tickets and
wrong-phase callbacks cannot advance the source cursor.

Explicit delays start after the preceding completed operation. The recovered10ms
gap between directly consecutive keyboard events is preserved; explicit delay0
also breaks that adjacency. Deadlines are checked for overflow, and delayed polls
do not backdate output completion or emit a burst of future key commands. The
source cursor is a prepared program position, not a count of completed delays.
This is a scheduler policy tested with a simulated clock. Actual native completion
meaning, source timing parity and foreground acceptance are still unverified.

Stop cancels undispatched downs and skips future waits/source events. Dispatched
downs retain their pending state, including a late delivered completion, followed
by owned releases. Uncertain delivery latches stop and uses the existing ledger's
three dispatched-release-attempt budget. Exhaustion never reports recovery
confirmed. A cleanup command refused before native dispatch retains held state
and reports unconfirmed recovery rather than automatically retrying forever.
Historical uncertainty remains observable after a confirmed release.

A regressed completion timestamp stops the scheduler but still records terminal
delivery so an owned release can be prepared. An adapter must retain this state
and its native resources until completion/drain or an explicitly reported recovery
failure; discarding a scheduler does not send key-ups or prove crash recovery.

## Evidence and open integration

The existing parser and bridge evidence remains in SOURCE-MACROS.md,
KEYBOARD-SYMBOLIC-BRIDGE.md and KEYBOARD-OUTPUT-RECOVERY.md. In particular, native
dispatch12BE6E evidence ev_bed07cbcc6184aaf6145e517884bf89970b7a48a8c5e1bb89b73117e82d41034
establishes immediate-event keyboard adjacency, including delay0 replacing the
previous event. No new vendor execution or hardware observation is claimed here.
The existing symbolic plan's runtime gaps remain unchanged.

Twelve scheduler checks cover completion-relative delays, zero-delay adjacency,
one-command handoff, stale/duplicate phases, stop before/after dispatch, late
completion, uncertain/failed recovery, clock regression/overflow, exact routed
E0 chord cleanup, explicit UnitId/metadata handling and atomic unsupported input
refusal. The512-step fixture retains at most32768 bytes of step storage; this
excludes the ledger/allocator/runtime and is not process-memory acceptance.

The next integration requires a serialized app actor, proved native output
completion and cancellation, enforceable selector ownership and physical tests
for held keys across disable/profile switch/close/crash/unplug/sleep. Then add
mouse, conditional packets, repetition boundaries, concurrent ownership and the
remaining playback modes. Full macro authoring/activation and the complete
device/profile/Studio/privacy/visual/three-run memory gates remain open.

The read-only VM prerequisite recheck on29October observed10683760640 available
host RAM bytes against its unchanged10737418240-byte requirement. No guest was
started or disk created. Continue independent work rather than lowering the gate
or closing unrelated apps. This observation does not establish VM or driver load.
