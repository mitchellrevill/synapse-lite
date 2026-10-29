# Local macro library and definition drafts

Open **Macro** from the dashboard. The library combines retained Synapse4 macro
definitions with locally created definitions. Create, import, duplicate and
delete are explicit library operations. Names and event edits remain local until
Save; Discard abandons them. Leaving or closing a dirty editor is refused with a
Save/Discard prompt in the page. Undo/Redo retains up to20 snapshots and4MiB of
serialized record history, including after a successful save.

The initial library checkpoint authors fixed delays in milliseconds, up to one hour.
Manual keyboard/mouse events are now available; see MACRO-INPUT-AUTHORING.md.
Existing
event rows can be moved or removed, except for the source actionBar row. Other
source event types remain visible and retained. Random/structured delays and
definitions carrying an appEngine event cache refuse event edits until their
editing/cache-invalidation contract is verified. No recording, playback, keyboard
interception or device writes are activated by this page.

## Compatibility and persistence

The versioned local library lives in Store.extra and retains source encoded
records. Rename leaves the payload and source hash untouched. Event changes
preserve raw unmodified root leaves and event rows, including numeric tokens,
then recalculate the existing source stable-JSON MD5. MD5 is compatibility data,
not authentication. Duplicate assigns a fresh random UUID while retaining other
payload fields. Source macro hashes and existing store budgets are checked.

Import accepts definitions from a Synapse4-shaped profile export; it does not
reimport its profiles. Referenced missing definitions are attached to the
retained source envelope. Edits propagate to retained records of the same GUID,
so existing profile export can carry the revised referenced definition. Profile
settings and active selection are preserved. Referenced definitions cannot be
deleted; conservative exact-GUID references in retained metadata and other
definitions also prevent deletion. Conflicting GUID definitions refuse rather
than selecting a winner.

Save reads the current store and checks the selected source record revision
before replacing that definition. A detected external change preserves the
draft and refuses the save. This is an optimistic revision check, not a claim of
cross-process atomic compare-and-swap against uncooperative writers. Read-only
mode refuses mutations before saving. The library caps definitions at256, event
rows at512 per definition, combined encoded payloads at1MiB, and uses the
existing complete-store2MiB budget.

## Source evidence and visual comparison

The recovered macro renderer main.3f4b9604.js hashes to
fd20eeea9ac259a741c5deec4cc23f7e293cb99fa8ed658816af50bbd894941d.
Focused module13139 exposes the delay constructor
`{Type:0,Number:Number(seconds.toFixed(3)).toString(),selected:false}` and a
source macro containing GUID, macroEvents/actionBar and macroPlaybackOption.
Creation follows the existing exported definition projection; unrelated native
appEngine caches are not synthesized.

REA analyzed seven focused literal extracts:7,815bytes,2,737 AST nodes, zero
parse failures and587 semantic unknowns. Retained evidence:
ev_66d7b015c4e2ac27f16ee68fcdee4536bda647103c605063420847db62c821d7,
parent ev_d0e612800782a0718e4b534f2242a2e5f22bd8ba15ccce09a7c78d4559f9565c.
This is static source evidence, not proof of every vendor runtime behaviour.

The native painter capture was reviewed against stock-macro-editor-0.jpg from
the inventory. Both use a selector, action sidebar and event timeline. The new
page remains an interim functional editor: sidebar contents, dimensions, action
menus, duration/header controls and module icon differ. Painter captures omit
child edits; the native test checks their actual HWNDs separately. Pixel-perfect
acceptance remains open. The approved shared tab geometry was not redesigned.

## Verification and next work

Seven core checks cover raw-data preservation, retained profile export, missing
definition recovery, duplicate/delete references, stale-save conflicts, schema
refusal, invalid hashes, structured delays and native-cache protection. The
native HWND check uses an isolated simulated device/configuration and exercises
create, field edits, Save/Discard, blocked Home/Close, Undo/Redo after saving,
invalid/read-only refusal, duplicate/delete, external conflict and definition
import without importing profiles. No physical discovery or device SET is used.
See validation/macro-library.json for the completed build/test evidence.

The regular workspace passes494 core,306 tray,4 CLI and1 documentation checks
(6 core/47 tray intentionally ignored). The optimized workspace builds. The
broader optimized native suite passes23 of25 checks with private font access,
including the macro workflow and all12 Studio checks. Two checks remain failing:
Quick Lighting focused close reports changed file/transport count, and the Wave
dropdown check unwraps a missing geometry value. Their cause is unresolved; no
claim of a fully passing native suite or baseline-only failure is made. A first
restricted run also failed private-font loading/overlay checks; those two pass
with font access, without weakening assertions.

TODO diagnostics: isolate the two remaining checks, capture the exact file diff
and GET/SET trace around focused close, identify which dropdown geometry is
missing, and compare with the preceding commit under the same assets/DPI. Keep
the original assertions and preserve the failed evidence. Per the user's
instruction, continue independent features after recording these blockers.

TODO: recording; rebinding existing keyboard/mouse rows; nested macros and loop
editing; remaining source events and playback settings; key-binding workflow;
stock macro-only import/export contract; source cache invalidation; exact menus,
widgets and visual matching. Live activation additionally depends on proved
selector ownership, routing, cancellation, physical held-key arbitration and
recovery. Do not repeatedly retry that blocked driver route; continue independent
effects/device work. Full API coverage, all retained functions, privacy checks and
three complete-build memory measurements remain unfinished.
