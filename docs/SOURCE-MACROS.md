# Source macro parsing and symbolic planning

The separate [saved GUI definition bridge](MACRO-DEFINITION-PLANNING.md) now
projects supported editor records into this native planner, verifies matching
source caches, and provides local file inspection. This module's parser and
runtime gaps remain unresolved; no live input adapter is enabled. Optional native
keyboard fields now follow the [field contract](NATIVE-KEYBOARD-FIELDS.md).

**29 October addition:** a separate bounded scheduler now joins admitted one-pass
keyboard plans with packet planning and the output ledger under simulated
completion/clock control. See [MACRO-SCHEDULER.md](MACRO-SCHEDULER.md). It has no
live input adapter; the symbolic runtime gaps below remain unresolved for actual
execution, and no profile macro activation is enabled by that model.

`secbird_core::source_macro` is a pure Rust boundary for the shipped source's AppEngine event JSON and macroGroup playback metadata. It opens no files/devices, creates no scheduling thread, sends no input and does not construct RZCONTROL packets. The existing `macros` module is separate onboard HID-usage bytecode; their key/button namespaces must not be merged.

`SourceJson::parse(text, Limits)` retains the exact original JSON plus a bounded semantic Value. Unknown fields, lexical number spelling and duplicate keys survive through `original_json()`; the Value view has ordinary last-key semantics, so any duplicate key blocks planning. Existing profile_compat source storage is unchanged. Planning explicitly selects the root event array or `/appEngine/events`; it projects that complete array and does not claim to validate the outer macro cache/envelope, hash or GUID identity.

`PlaybackMetadata::parse(&macro_group)` borrows the source Value. It recognizes the observed Once, NTimes, ContinuousHeld, ContinuousToggle, Sequence, Queue, Phased and PhasedMacro names. Only Once and positive integer u32 NTimes produce a `SymbolicPlan`; other modes return UnresolvedPlayback. Once ignores repeatCount as the shipped converter does, retaining that inert field without interpreting it. Unknown metadata fields fail conservatively rather than being discarded. This initial supported domain deliberately rejects JavaScript coercion, malformed/zero/fractional counts and ambiguous values.

The planner accepts only these native source events:

| Event | Accepted fields | Interpretation |
|---|---|---|
|delay|type, ms|Integer u32 milliseconds; zero remains an actual event.|
|keyboard|type, scancode, flag; optional modifiers, unitId|Native u16 scan code retained; explicit flags0/1 ordinary down/up,2/3 E0 down/up,4/5 E1 down/up. Optional modifiers is u32 and unitId is u16; absence and explicit zero remain distinct. Mixed-prefix6/7, other flags and timer fields are unsupported. No prefix inference or HID/VK conversion.|
|mouse button|type, id|Single transition: left1/2, right4/8, middle16/32, button4 64/128, button5 256/512.|
|mouse wheel|type, id, data|Vertical1024 or horizontal2048; raw120→+120, raw65416→−120. Both raw bits and signed delta remain available.|

Every selected event is validated. Unknown types, mixed/extra action fields, combined button IDs, unsupported wheel magnitudes and malformed numeric fields fail the whole plan; no partial plan escapes and no action is silently skipped. Random delay, launch/clipboard, nested macros, loops and mouse motion remain unsupported. UI Type events are not converted here: the fixture inputs explain provenance, while the accepted outputs are the native event JSON.

Repetition is compact: the plan stores one base sequence plus repeat count, checked expanded-event count and checked total explicit-delay milliseconds. It never materializes every repetition. Native completion costs are excluded from that sum. `source_keyboard_gap_before_us` reports the evidenced10000microsecond rule for directly consecutive keyboard events within the base sequence. A zero-delay event breaks adjacency. Repetition-boundary timing remains unresolved.

Every plan reports NativeIdentityAndRouting, CompletionDrivenTiming and HeldOutputOwnershipAndCancellation gaps; repeated plans add RepeatBoundaryTiming. This is not a runnable macro, an input sender, a release/drain guarantee, or proof that stopping playback releases every key/button. [RZCONTROL packets](RZCONTROL-PACKETS.md) and [driver control](DRIVER-CONTROL.md) remain separate boundaries.

Default parser budgets are256KiB input,4096 structured nodes including object keys, depth32 and512 selected events. Default planning budgets are100000 expanded events and3600000ms of explicit delay. The node/depth guard runs during deserialization, before accepting additional nested values. Limits cannot exceed the hard byte/node/depth/event ceilings. A compile-time conservative retained-data accounting ceiling is2MiB; it is not a measured allocator/RSS guarantee. Repetition/delay budget exhaustion and arithmetic overflow return errors without expansion or clamping.

## Source evidence and validation

The macro renderer parent is main.3f4b9604.js, SHA256 `fd20eeea9ac259a741c5deec4cc23f7e293cb99fa8ed658816af50bbd894941d`. Module13139 occupies UTF8 bytes68792..90653; helperf converts null key State to flag0/1, m converts source button/wheel events and rounds delay seconds to integer milliseconds. Module42358 occupies303878..369990; S maps configured playback mode/count to down/up outputs. Controlled replay produced the ten conversion and eight playback cases retained in `crates/secbird-core/tests/fixtures/source_macro_conversion.json`. The fixture contains synthetic input/output data and source provenance, not vendor implementation text.

Native mapping_engine.dll SHA256 `6eabdfdedf797e042738b630d827c06f7a45dbe560ebaec96c66698f88f3320a`: event dispatcher ECEE3 (REA ev_4452f561ee33385b82db43dc86da9ed3ba7afdd22d6071d174ea06b97ce4649c), delay parser ED667 (ev_24cbff4f3a9547f7c33ca505d29fab001846e81419f1170130398a59d5e55ecd), delay executor12AB1E (ev_b3dcdd1329fe55aeba1bbce14b5259935d8da0e50af95b5279d24cc322304b0e) and dispatch12BE6E establish the different native type indices, u32 milliseconds, microsecond wait conversion and keyboard adjacency rule. The shipped key field builder retains native make/flag fields; Windows SDK ntddkbd.h separately defines KEY_MAKE0/KEY_BREAK1. Full identity/default UnitId/extended mapping and cancellation ownership remain unresolved.

Zero-delay adjacency was checked against the actual dispatcher branch, rather than inferred from elapsed time. Dispatch12BE6E evidence ev_bed07cbcc6184aaf6145e517884bf89970b7a48a8c5e1bb89b73117e82d41034 shows the delay branch calls12AB1E at12C1C7 and then unconditionally calls6566A at12C201 with the current delay object and executor+0x80. Helper6566A writes the incoming pointer into that slot at65689; it does not filter for output types. The delay executor tests ms at12AB36..12AB3C and skips only its wait when zero, returning to that assignment. The keyboard branch reads+0x80 at12C2AB, invokes previous/current type methods and compares at12C2DA. Only equal types take the 0x2710 (10,000 microsecond) wait setup/call at12C2E7/12C2EE. Consequently a dispatched delay-kind12, including delay0, replaces the prior event before the next keyboard-kind21 check. This proves immediate-event adjacency for these successful linear dispatch paths; cancellation, parse failure and repeat-boundary behavior remain unresolved.

Twelve focused parser tests cover source vectors, every supported transition/wheel encoding, strict flags/numbers, exact raw retention, duplicates, atomic unsupported-action failure, finite/unresolved playback, compact repetition, budgets and duration overflow. Full core validation passed238tests with5ignored after setting TEMP/TMP to a writable workspace directory. The first full run's15 disk-test failures were sandbox temporary-directory AccessDenied errors; no implementation change was needed to resolve them. No hardware or macro runtime was exercised.

The separate [keyboard symbolic bridge](KEYBOARD-SYMBOLIC-BRIDGE.md) can plan raw packets from explicitly supplied native keyboard fields and routing context. It does not make this macro plan executable or resolve held-output recovery.
