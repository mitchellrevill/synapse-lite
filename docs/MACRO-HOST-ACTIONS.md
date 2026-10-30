# Macro text, command and launch authoring

Open Macro's Input menu and choose Text function, Run command, Launch program or
Launch website. The action editor's Apply changes only the local macro draft.
Save on the macro page persists the definition. Cancel abandons the action form;
Home/window close wait for Apply or Cancel so the form is not silently lost.
Select an existing action and choose Edit Action to revise it.

Text supports multiline Unicode and the source textarea's 250 UTF-16-unit bound.
Windows CRLF input is normalized to LF for the edited Text value. Command and
launch fields have an explicit 32,767-unit project bound, reject NUL/line breaks
and are never truncated silently. Action fields use a complete bounded Unicode
read rather than the ordinary page-field limit. Retained values that cannot be
displayed safely are refused before opening; empty source placeholders may open.
The active value must be nonempty when applying. A launch
row retains both program and website strings when changing RadioIndex. The
program picker uses an ordinary owned file dialog without changing the working
directory; it does not open or run the selected file.

Nothing here executes commands, opens websites, starts programs, changes the
clipboard or generates input. Imported actions stay inert. The separate
[saved host-action compiler](MACRO-HOST-PLANNING.md) now projects these rows into
bounded native events and source delays. Recording, live playback, host execution
and recovery remain unfinished in this checkpoint.

## Source and preservation

The retained source rows use Type4/Text, Type5/TextCommand and
Type10/RadioIndex/Content0/Content1 with selected:false. The recovered text editor
has maxLength250; launch form updates include both inactive/active values. Edits
patch known row fields while retaining raw unknown leaves, IDs, phase and other
events. Existing action kinds cannot be converted into another kind by an edit;
program/website are the two choices of the same Type10 row. Existing appEngine
cache guards, source hashes, macro identity, store budgets and save conflict
checks remain enforced. Read-only mode refuses Apply but permits Cancel.

The recovered converter records additional native behaviour: text becomes a
clipboard event followed by 500ms; command becomes a launch followed by 50ms and
has a taskmgr special case; program/document and website conversions choose
different launch fields/delays. Authoring retains the original form values and
does not run those projections. In particular, it does not rewrite a command
merely because its text contains taskmgr. Safe native execution of these data,
including quoting, clipboard ownership and cancellation, is a separate TODO.

Source main.3f4b9604.js hashes to
fd20eeea9ac259a741c5deec4cc23f7e293cb99fa8ed658816af50bbd894941d;
the inspected 8190.4302afe2.chunk.js hashes to
b445768b9d9b2761323900e4437b111492e234a94f72e179b6cbcbcda6966403.
REA inspected five focused inert extracts: 10,304 bytes, 3,499 AST nodes, zero parse
failures and 612 semantic unknowns. Evidence:
ev_bd890dd233a7676ee1e739a864da1ad335668745410d3845a9ee70eee8c294ef;
parent ev_6858d12a81379ce2044a4ee077b8745463b76a018bb137e04ea19d076a05271d.
Seven fixture cases replay only the literal templates/pure converter; no vendor
bootstrap/native code, clipboard, process or browser action executes.

## Verification and remaining work

Core checks cover the independent template fixtures, UTF-16/emoji limits, CRLF,
invalid/control values, inactive launch fields, raw numeric metadata, action-kind
and native-cache refusal, local draft edits, profile/active preservation and
source export/reopen. The native workflow covers form open/apply/cancel, invalid
text, Home/close protection, Unicode readback, command preservation, program/
website switches, read-only Apply/Cancel, exact saved rows and a command with
an emoji crossing the former 4,096-unit reader boundary through Save/reopen. See
validation/macro-host-actions.json for completed evidence and build results.

The forms are an interim native presentation, not completed pixel-perfect stock
popovers. Emoji picker, source anchoring, exact widgets/focus/keyboard shortcuts,
live clipboard/process execution and recovery remain TODOs. Human file-picker
selection is not established by the simulated workflow. Child edit controls are
verified through HWND values; painter captures omit them. Broader source/device
API coverage, privacy audit and all three complete-build memory runs remain open.
The independent threads own loops, Audio Meter, lighting regressions and the ten
other assigned features; this checkpoint does not claim their completion.
