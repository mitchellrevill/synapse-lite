# Mixed keyboard and mouse macro scheduling

The macro scheduler now compiles and advances one balanced keyboard/mouse pass
through the existing source packet planners. It tracks mouse button ownership,
serializes both input classes, carries the destination on every returned command,
and handles completion, stop and recovery through one timeline. The app's live
macro activation remains unfinished; this module has no native input sender.

## Destinations and completion

`Scheduler::new` retains keyboard-only admission. `Scheduler::with_contexts`
accepts an explicit `Contexts { keyboard, mouse }`. Each used class needs its own
native context. A missing mouse route never borrows the keyboard route, and an
omitted source UnitId remains native zero, requiring routing UnitId zero.
The returned `Command::identity()` identifies that command's destination;
`packet()` retains the keyboard/mouse packet kind and source-derived fields.
The former scheduler-wide identity accessor is replaced by per-command routing
and `contexts()`, so mixed output cannot imply a single shared destination.

Each command receives a scheduler ticket across both ledgers, including its run
Owner and a checked serial. A caller must use a fresh Owner per run, serialize
access, mark native API entry with `dispatch_started`, then report terminal
completion. Polling returns a command only once. Acceptance is not Delivered.
Stale, duplicate, wrong-phase and different-run callbacks cannot advance output.
The Owner and routing token describe local state; they do not grant driver access
or prove physical ownership.

## Ordering and recovery

Five source mouse button pairs and the four wheel axis/sign cases use
`mouse_symbolic_bridge` unchanged. The complete source pass is validated before
admission. Repeated makes, orphan releases, held-at-end inputs, repetition and
conditional E1 packet chains still refuse. Explicit down/up/down/up clicks stay
distinct. A source mouse event breaks keyboard adjacency; mouse events gain no
invented 10ms wait. Directly adjacent keyboard events retain the existing 10ms
gap. Explicit delays, including zero, retain their source position. Waits anchor
to terminal completion and do not emit catch-up bursts after a late poll.

`mouse_output_ledger` records UnitId, down flag, metadata and owners for each held
button. Cleanup changes only the five evidenced down-to-up flag mappings; it
does not transform metadata a second time. Stop cancels undispatched downs and
wheel impulses, skips future source actions/waits, and retains dispatched work
until completion. A late or uncertain down can still require an owned release.

Wheels are one-shot impulses. They acquire no held claim and receive no inverse
wheel or replay as recovery. Uncertain wheel completion stops the source timeline
and remains observable after earlier held keys/buttons have been released.

Each input class has a private ledger, with at most one command outstanding
globally. A refused cleanup release is latched for that individual key/button,
without consuming a native dispatch attempt or retrying it automatically. Other
eligible releases continue, both within that class and in the other class.
Uncertain releases retain the three-dispatch-attempt budget. After eligible
cleanup is exhausted, `RecoveryUnconfirmed` is reported and subsequent stopped
state remains unconfirmed while any possible hold survives. One failed class
cannot prevent cleanup of the other class. Cleanup order is a project policy,
not a claim of stock cancellation parity.

Clock regression stops the timeline but still records an already-dispatched
terminal completion. Checked duration overflow also stops and permits owned
cleanup. Neither stop nor dropping this Rust value performs I/O or proves crash
recovery. The caller must retain the state and native resources through drain or
an explicitly reported recovery failure.

## Evidence and validation

Retained static analysis is bound to mapping_engine.dll SHA256
`6eabdfdedf797e042738b630d827c06f7a45dbe560ebaec96c66698f88f3320a`:

- Mouse packet fields and constructor defaults: parser10BFED,
  constructor10D8A8, setters10E18C/10E2F8 and writers112C60/134C80, as recorded in
  [MOUSE-SYMBOLIC-BRIDGE.md](MOUSE-SYMBOLIC-BRIDGE.md). The 14 existing packet
  vectors are static transcriptions, not vendor runtime captures.
- Release conversion10E110 maps 1→2, 4→8, 16→32, 64→128 and 256→512:
  `ev_b51f99b667bcb45687c6639e879dbbe1208fd76becee13804d590965fd9c847b`.
- Down/up predicates10DFD0/10E030 exclude wheels:
  `ev_1ba38a71573b6acbad91118a696d9fde75635934fd32ec29b386f0dd60c07822`
  and `ev_c09cf247fecb53623bd31b457c5d6975ab4b486dd73acd98775ae25818b887d3`.
- Dispatch12BE6E's mouse-kind27 branch has no keyboard adjacency wait;
  `ev_bed07cbcc6184aaf6145e517884bf89970b7a48a8c5e1bb89b73117e82d41034`.
  Mouse ready dispatch130CF8 calls void1345AA without propagating actual native
  submission/completion: `ev_e298b47973ed9bb7317761bf37489f5c33ba6c736c55b453968b9a078aa65e7d`.

The new checks exercise mixed routes/tickets, all 14 fixed packet vectors,
normal ordering and timing, multiple simultaneous holds, stop before/after
dispatch, late/uncertain output, no wheel replay, per-entry refusal, release
exhaustion without stranding the other class, clock faults, context admission,
different-run callbacks and bounded storage. The existing keyboard scheduler
checks remain. See [validation/macro-mixed-output.json](validation/macro-mixed-output.json)
for final counts, build results and evidence hashes.

At most 512 source steps are retained without expanding repetitions; the tested
step-storage ceiling is 32,768 bytes. The mouse ledger admits at most 64 button
entries and 256 owner claims, with one wheel slot. These are module bounds,
not measured process-memory acceptance.

## Required integration

The next native adapter still needs enforceable driver selector ownership,
validated per-device/class routing, terminal completion/cancellation semantics,
foreground acceptance, physical-held-input arbitration and recovery through
disable/profile switch/close/crash/unplug/sleep. Normal profiles, input drivers
and live activation are unchanged here. Source modifier expansion and the native
mouse held-ledger fast path remain unresolved; repeated makes are refused.
Recording, the remaining macro modes/events and full UI integration remain open.
No screenshots are claimed for this backend change. Full device API, profile,
visual/privacy and all three complete-build memory acceptance runs remain open.
