# Keyboard output ownership and completion

28 October 2026. This checkpoint implements a pure bounded output ledger, not
live interception or input submission. The physical receive comparison remains
recorded in [Ornata input source](ORNATA-INPUT-SOURCE.md).

## Implemented behavior

`keyboard_output_ledger` coalesces claims for the same key by this runtime's
logical owners. Releasing one owner preserves the others. A release uses the
complete admitted down template, changing only the break flag. Releases precede
new downs and only one command may be outstanding. Capacity is bounded to 256
keys and 1,024 owner claims; unknown flags and conflicting metadata are refused.

The adapter must mark dispatch immediately before entering its send API and
report terminal completion separately. Submission, cancellation and completion
are not interchangeable. Stop rejects new claims, invalidates an undispatched
down and retains dispatched work until completion. A late down completion still
requires a release. Releasing the last owner before dispatch supersedes that
prepared down. Duplicate dispatches and stale completions cannot advance state.

Uncertain completion latches stop and retains possible held state. Release
attempts are bounded to three; exhaustion reports recovery unconfirmed, never
drained. An exhausted key does not prevent release attempts for other keys.
A confirmed release can drain the ledger while historical uncertainty
remains observable. Refusal before dispatch needs no speculative release.

An adapter must serialize ledger changes and dispatch, check every returned
result, dispatch a command at most once, retain native resources until terminal
completion, and separately establish what successful completion guarantees.
There is no Drop-time I/O, automatic global key-up or claim of crash recovery.
Owner tokens are local bookkeeping, not driver ownership or physical identity.

## Installed selector limitation

Static inspection of installed RzCommon.sys 1.0.0.76 confirms:

- SET helper `0x7630` clears matching descriptors before adding the replacement;
  a failed add may already have removed a previous matching selector.
- Keyboard CLEAR helper `0x622C` ignores the 260-byte selector tail and compares
  ordinary UnitId values as a class, rather than exact per-device identifiers.
- ExtraInformation participates in matching and cannot be repurposed as an
  arbitrary owner token. A label in the retained tail does not isolate cleanup.
- Enumeration is not an atomic snapshot. A full-record check followed by CLEAR
  cannot prevent another writer from replacing the entry between those calls.
- Windows error 31 alone is not proof of an empty selector table. The current
  read-only probe returned zero records with that error and stays incomplete.

Binary SHA256:
`64b72ef9ce409bb17d31096c119b68c4a6a02ab35b7535d763157a4e1a587e59`.
The retained REA selector-contract report and disassembly are private workspace
evidence; no selector writes or generated input were used to establish this.

The ledger solves logical output tracking only. Enforced selector ownership,
physical routing, native send/completion behavior and foreground acceptance are
still separate work. A local mutex or empty process list does not establish
exclusive access against uncooperative vendor clients. Live Snap Tap remains
disabled pending that work and recovery verification across profile changes,
disable, close, crash, unplug and sleep. The complete implementation and memory
goal is unchanged.

## Verification

Ten focused ledger tests pass, covering coalesced ownership, exact release
metadata, release-before-down ordering, stale/duplicate phases, late completion
after stop, prepared-command cancellation, refusal without phantom release,
bounded uncertain recovery without starving other releases, and capacity.
The full core suite passes: 362 tests,
six intentionally ignored tests, and one doc test. These are simulated state
tests; they do not prove native output delivery or physical release.
