# Saved GUI macro definitions to native plans

`profile_compat::macro_library::planning::prepare` connects saved source GUI
macro records to `source_macro::SymbolicPlan`. Its output can be admitted by
the separate mixed keyboard/mouse scheduler when that scheduler's requirements
are satisfied. Preparation reads an explicit Store and native binding JSON;
it does not change profiles, repair caches, discover devices or send input.

## Local file commands

Use a native backup or supported Synapse 4 export. Listing shows macro names,
GUIDs, row counts, references and saved-record revisions:

```text
secbird source-macro list PROFILE_DOCUMENT
secbird source-macro plan PROFILE_DOCUMENT BINDING_JSON NEW_OUTPUT
```

The binding file supplies the native macroGroup options explicitly:

```json
{"guid":"GUID_FROM_LIST","macroPlaybackOption":"Once"}
```

`NTimes` also needs a positive integer `repeatCount`. The planner retains compact
repetition; the current scheduler still refuses repeated execution. Sequence,
held/toggle, queue and phased execution remain unresolved. This binding is the
native three-field format, not an arbitrary entire profile mapping object.

The plan report contains native events, their zero-based source GUI row indexes,
explicit delay totals, repetition, cache provenance and unresolved runtime gaps.
It always reports live playback unavailable. A successful projection does not
mean the native scheduler can admit every event or that hardware is compatible.
The CLI reads only the specified inputs and does not install a Store. It routes these commands
before HID initialization/discovery; `-d` is refused. Inputs are regular files
with bounded reads, and an existing output or alias of an input is never replaced.
All validation completes before creating the new report.

## Source projection and preservation

[Host-action compilation](MACRO-HOST-PLANNING.md) now adds Text, Run command,
Launch program and Launch website, with source delays and exact cache comparison.
The input scheduler rejects these host payloads; this is compilation, not execution.

The bridge uses the recovered default UI-key-code converter, not HID usages or
a guessed physical layout. It retains the order of all 150 source lookup entries,
including entries without an input ID. It implements category/default lookup,
the special modifier codes, and the evidenced stored-scan-code/flag fallback.
A null stored scan code does not invent a fallback. Explicit key states retain
their native flags. A null key state follows the source's ordinary flag0/1 pair;
it does not infer an extended prefix from the key's display name.

Five mouse buttons, double-click and four wheel functions follow the source
converter. Double-click expands to down/up, delay10ms, down/up. A null button
state retains the source up/down order. The scheduler can refuse that unbalanced
sequence; this bridge never rewrites it into a different action. Known source
placeholders and complete-action release no-ops are omitted intentionally, with
row mapping retained for emitted events. Unknown event types are never silently
skipped. Empty definitions do not yield a playable plan.

JSON integer and floating-point spellings retain JavaScript numeric equality
for GUI codes/states and complete cache comparison. Strings are not silently
coerced into key/button codes. The retained payload is not normalized or rewritten.

Fixed delays use the source's JavaScript Math.round behavior, including negative
half rounding to zero and the near-half case where floor(value+0.5) is wrong.
Canonical decimal strings/numbers, empty values and null follow the supported
source cases. Boolean/hex coercions and random/structured
delay execution remain unsupported. Phased rows and motion, host actions, loops,
nested macros, profile/multimedia/lighting events are explicit remaining work.

The selected saved record must pass its source hash and unique-key checks,
including duplicates nested in opaque fields. Streaming uniqueness validation
keeps no second semantic tree; ordinary serde Value parsing preserves the numeric
semantics used by the existing source hash code. Profile references, activation,
opaque metadata and exact encoded payload strings are not rewritten. Preparation
returns the saved-record revision; a live actor must recheck freshness before
admission and handle changes while a run is active.

If `appEngine` is present, its hash must verify and its entire events array must
equal the GUI projection under complete canonical source-value comparison.
The known optional mouseMoveType must be `none`;
unknown cache fields/settings refuse. An absent cache permits generated planning.
A malformed, stale, cache-only or incompatible cache never silently wins or
falls back to another program. Successful cache comparison does not authorize
editing/invalidation: existing authoring guards remain. Hashes establish format
consistency, not physical input ownership or delivery.

Bounds remain explicit: profile input2MiB, binding4KiB/256 nodes/depth8, decoded
definition1MiB/32,768 nodes/depth64, the existing whole-Store/library budgets,
512 GUI rows and512 expanded native events. Native parsing keeps its existing
byte/node/repetition/delay limits. A larger expansion refuses as a whole. These
are component bounds, not complete-build memory acceptance.

## Evidence and verification

The source parent is main.3f4b9604.js, SHA256
`fd20eeea9ac259a741c5deec4cc23f7e293cb99fa8ed658816af50bbd894941d`.
Module13139's pure f/m helpers and module629's literal lookup data are covered
by retained REA summary
`ev_368f033770d7aaf127eb74322e35ac3865eaf53a0136ff7e35db3eac13a56a2a`,
parent `ev_7b6e25e07a28ce08411cfcb137c2ed7ee33f1c073b954c0ed1006d0325f36587`
(nine focused files, 30,687 bytes, 7,900 AST nodes, zero parse failures,
639 semantic unknowns). No vendor bootstrap/native code or host action executes
in the controlled converter replay.

The new fixture accounts for501 source cases: lookup entries, legacy category/
default lookups, special modifiers, scan fallbacks, null states, mouse functions,
placeholders and delay rounding. Source results with unresolved/null scan codes
are explicitly refused. All125 authored keyboard cases also match their existing
independent conversion vectors. An editor-save → preparation → routed scheduler
check covers a keyboard/mouse chord, wheel, delay and double-click. Additional
checks cover caches, duplicate keys, hashes, conflicts, unsupported rows/modes,
bounded expansion, revision snapshots, exact source re-export and file refusal.

The built optimized CLI was exercised on a synthetic Synapse export with an
independently generated source cache/hash. All11 native events, source-row
mapping and260ms explicit delay matched the recovered converter; input documents
and source profile records stayed unchanged. This is file/format evidence, not
physical playback. See validation/macro-definition-planning.json for counts,
source/build/evidence hashes and remaining work.

The native adapter, enforceable selector ownership, completion/cancellation,
physical held-input arbitration/recovery, full binding translation, remaining
event/mode support, UI activation, device APIs/profile acceptance and final
visual/privacy/three-run memory gates remain required. Approved tabs are unchanged.
