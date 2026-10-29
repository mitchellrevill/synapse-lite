# Nested macro references and profile interchange

In Macro, open **Input**, choose from **Macros**, then **Add Macro**. Selecting
an existing nested row exposes **Update Macro**. Both edits remain local until
Save and use the existing Undo/Redo, Discard and revision-conflict workflow.
The timeline displays the target name. References keep the target GUID rather
than copying its events. No recording, expansion, playback or input injection is
enabled by this feature.

The selector disables self-references, dependency cycles, missing/incomplete
chains and mismatched playback options. Saving rechecks new references against
the current library, so deleting a target or introducing a cycle after the draft
was prepared refuses the save. Referenced definitions cannot be deleted. Existing
bad imported chains remain retained and are reported; they are not silently
removed or repaired. Retargeting an incomplete reference to a verified definition
can resolve it. Inspection is bounded by256 definitions,512 rows per definition
and the existing payload/store budgets; it never executes referenced events.

## Source contract

The retained source template is
`{Type:7,MPIndex:null,guid:target,selected:false}`. The recovered pure converter
projects it to a native-format macro event with the target GUID and flag0. The
source selector follows GUID relationships, disables cycles and compares playback
options with strict equality. The implementation accepts matching scalar options
and absent/absent options; separate object/array options remain unsupported.
Retargeting preserves raw unknown fields and clears the existing legacy MPIndex
placeholder rather than keeping a stale list index.

The main renderer hashes to
fd20eeea9ac259a741c5deec4cc23f7e293cb99fa8ed658816af50bbd894941d;
the selector chunk hashes to
b445768b9d9b2761323900e4437b111492e234a94f72e179b6cbcbcda6966403.
REA inspected three focused inert extracts:5,469bytes,1,803 AST nodes, zero parse
failures and246 semantic unknowns. Evidence:
ev_51b27c3021b0fa118f5aadb5b940a01beb202923b5e5d2c5806d12ec63fd8e7c;
parent ev_a1291f1c39956af9f3d3bd8359d4a59fb487791a71c9d4d3561a089a2c854b12.
The fixture records controlled replay of only the literal template/pure converter,
without bundle bootstrap, native DLL execution or device access. This proves a
data projection, not working native nested playback.

## Profile compatibility

Retained source envelopes now carry the reachable GUI definition closure. A
profile referencing A, where A references B and B references C, exports all three
definitions without reimporting or replacing the profile. GUI selected export
also retains required dependencies; deselected unrelated definitions stay out.

Selected import resolves the dependency closure before cloning. All new GUIDs
are assigned first, then nested references and profile bindings are rewritten
consistently. A collision therefore cannot leave the imported parent pointing
at the destination's older child. Existing native caches are not guessed: nested
GUID changes requiring appEngine cache rewriting refuse and direct the user to
a full native backup. The full-backup path retains its established semantics.

Raw unchanged event/root leaves, profile settings, active selection, identities
and source hashes remain protected through ordinary library edits. Known cached
GUI event edits also refuse at final replacement. Dependency inspection covers
the recovered GUI event graph; cache-only/native-only dependency contracts and
cache invalidation remain TODOs. The optimistic save revision check does not
establish cross-process atomic compare-and-swap or exclusive file ownership.

## Verification and remaining work

Six new core checks cover source row projection, self/cycle/mode/missing-target
refusal, stale dependency checks at Save, raw retarget fields, reference deletion
guards, imported bad-chain diagnostics/recovery, transitive export/reopen, GUID
collision rewriting in selected import, selected export closure, and cache
refusal without changing the input object. The interchange fixture explicitly
binds its profile to the root macro so selected-profile coverage is meaningful.

The isolated native HWND check adds/saves/retargets nested rows, checks referenced
deletion protection and disabled cycle choices, then bypasses the menu to prove
that a cycle attempt changes neither draft nor file. Read-only refusal, profile
preservation and zero simulated device SETs are retained. Menu selection uses a
test-only message; real menu open/cancel is exercised. The optimized profile-modal
regression also passes. See validation/macro-nested.json for test/build evidence.

The reviewed painter capture shows the named nested row and Add/Update Macro
controls. It omits child edits, which the HWND workflow checks separately. Windows
popup/sidebar presentation remains interim; full stock visual acceptance and the
two previously recorded broader native failures are still open. Shared approved
tab geometry was not redesigned.

TODO: live nested execution and its cancellation/ownership/recovery contract;
recording; loop authoring; remaining event types, bindings and playback settings;
cache-only graphs and cache rewriting; macro-only export; source-matched menus.
Full device APIs, privacy and three representative complete-build memory runs
remain required. Continue independent inventory work when exact behaviour stalls.
