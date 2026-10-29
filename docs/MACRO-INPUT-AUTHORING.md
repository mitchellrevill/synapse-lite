# Manual keyboard and mouse macro events

In Macro, choose **Input**, select a keyboard key or mouse function, choose
Press and release/Down/Up, then **Add Input**. Press and release inserts explicit
down/up rows as one reversible draft edit. Double-click and wheel functions are
complete actions, so they insert one source row and disable the transition menu.
Save writes the draft through the existing revision-checked library workflow.
Opening/cancelling a chooser or changing the current choice does not write a
definition. No input recording, playback, device discovery or injection starts.

The selector includes125 source keys, grouped by source category, five mouse
buttons, double-click and four wheel directions. Four diagonal-arrow entries
have zero scan codes; KEY_YEN and KEY_RO lack numeric virtual-key descriptors.
Those six entries remain TODO rather than guessing their encoding. The source
table's names are not locale-aware character translation. Layout-aware captions
and those missing regional descriptors still need source evidence.

## Source format and compatibility

Keyboard rows retain source UI Makecode, category, explicit State, ScanCode and
source flag. Makecode is not a HID usage or necessarily a scan code. Left/right
modifier Makecodes use the recovered convertVKCode mapping, and native prefix
flags use the source getFlag helper, respecting an explicit outputFlag of zero.
This preserves Right Shift's ordinary54 make/break rather than interpreting
outputFlag0 as absent. Some source UI/recorder paths use outputFlag||flag instead;
the authoring path deliberately uses the explicit-zero helper contract.

The existing recovered converter consumes these descriptors with its recorded
scan-code fallback. Controlled pure-JavaScript replay checked250 emitted key
transitions against125 descriptors, with zero scan-code/flag mismatches. This
includes ordinary keys, E0 keys, Right Shift and the E1 Pause representation.
It does not establish that the unfinished native sender can safely dispatch
every representation; conditional E1/Pause execution remains blocked there.

Mouse function numbers match the source GUI format, including Right click1,
Scroll click2 and wheel6..9. Button presses use explicit down/up rows rather than
the source null-state conversion's unusual up/down ordering. Double-click uses
the source complete action row. This module stores definitions only and does
not synthesize or execute an appEngine cache.

IDs follow source max-existing-ID+1 within JavaScript's safe integer range.
Paired rows share an identity; an explicit Up reuses the latest unmatched
preceding authored Down of that input when available. Foreign/noninteger or
exhausted ID ranges refuse authoring. Existing unmodified rows and opaque root
leaves remain raw, including numeric tokens. Insertion cannot precede actionBar,
and a pair must fit the existing512-row limit atomically. Definitions carrying
appEngine caches still refuse event edits. The existing complete-store budget,
GUID/hash validation, conflict checks and profile preservation remain in force.

The main renderer hashes to
fd20eeea9ac259a741c5deec4cc23f7e293cb99fa8ed658816af50bbd894941d;
the inspected UI chunk8190.4302afe2.chunk.js hashes to
b445768b9d9b2761323900e4437b111492e234a94f72e179b6cbcbcda6966403.
REA analyzed nine focused inert source extracts:30,687bytes,7,900 AST nodes,
zero parse failures and639 semantic unknowns. Summary evidence:
ev_368f033770d7aaf127eb74322e35ac3865eaf53a0136ff7e35db3eac13a56a2a;
parent ev_7b6e25e07a28ce08411cfcb137c2ed7ee33f1c073b954c0ed1006d0325f36587.
The first extraction selected only the event-name template; it was corrected
to include the actual keyboard-row component before this analysis. Static
coverage and pure converter replay do not prove full vendor runtime behaviour.

## Verification and limits

Four core checks cover the125 independently replayed source fixtures, extended
flags, mouse pairs/complete actions, release identities, malformed/overflow IDs,
unsupported keys, raw leaves, cache protection, actionBar and row bounds. The
expanded isolated HWND check exercises chooser open/cancel, adds Right Ctrl
down/up, Right click down/up and Scroll up, verifies exact saved rows, history,
read-only refusal, dirty close and conflicting saves. Chooser selection is
injected through a test-only message; human menu selection is not claimed as
verified. No physical discovery, input generation or device SET is used.

One development check timed out on Duplicate after test-only read-only toggling.
The toggle changes state without repainting; disabled hit targets could remain
until a paint. The test now explicitly repaints after both toggle transitions,
retaining the original draft/disk assertions. The final optimized check and
workspace results are recorded in validation/macro-input-authoring.json.

The reviewed painter capture shows readable key transitions, mouse functions and
the added controls within the existing panel. Child edit HWNDs are checked
separately. Windows grouped menus and interim sidebar controls still differ from
stock: exact menu styling, widgets, layout and full visual acceptance are TODOs.
The shared approved tabs were not redesigned.

Existing input/pair rebinding and grouped deletion are now implemented; see
MACRO-PAIR-EDITING.md. Remaining: null-state/direction/kind conversion, paired
block reordering, recording, remaining events, binding and playback settings,
macro-only import/export and cache invalidation. Live playback additionally
requires proved driver routing/ownership, physical-key arbitration and recovery.
The two broader native failures recorded in MACRO-LIBRARY.md remain unresolved;
they were not blindly rerun for this independent feature. Full hardware/API,
privacy, visual and three complete-build memory acceptance remains open.

Paired loop editing is covered by MACRO-LOOPS.md; nested references and profile
dependency preservation are covered by MACRO-NESTED.md. Neither enables playback.
