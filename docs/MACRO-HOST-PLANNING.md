# Saved macro host-action compilation

The existing `source-macro plan PROFILE_DOCUMENT BINDING_JSON NEW_OUTPUT`
workflow now compiles Text, Run command, Launch program and Launch website rows
alongside keyboard, mouse and delay rows. It writes a local plan; it does not
execute commands, open a browser, change the clipboard or send input. Imported
macros remain inert and source/profile bytes remain unchanged.

Text becomes native `clipboard`/`id:text`, preserving Unicode and existing line
endings, followed by 500 ms. Commands become native `launch`, followed by 50 ms.
The recovered case-sensitive substring `taskmgr` special case is preserved in
the plan without rewriting the authored value. Executable program names retain
their full native command line and `pathToCheck`; document paths use the source
`cmd /c` wrapper and hidden flag. Websites use the exact source substring-prefix
rule and `cmd /c start` wrapper, hidden flag and 120 ms delay. These are source
compatibility projections, not a recommendation to execute unreviewed shell text.

Empty placeholders produce no native event, matching the converter. Null launch
selection produces no event. Invalid active fields, NUL, one-line command/launch
violations, overlong values, unknown native fields, duplicate JSON keys, bad
source/cache hashes and mismatched cached projections refuse the entire plan.
The existing byte/node/depth/event/delay budgets still apply; wrapper expansion
also counts against the native command-line bound. Text's project limit remains
250 UTF16 units. Unknown archival metadata is retained, not interpreted as a
native action option. Per-native-event source-row provenance remains exact.

Native host payloads now own strings, so source `Event` is Clone rather than Copy.
The input packet encoders still accept only their own input classes. The mixed
input scheduler rejects a host action during full admission, before emitting any
earlier input. Plans declare `HostLaunchExecution` and/or
`ClipboardAndPasteRecovery` gaps. No sender, child process or clipboard helper is
implicitly enabled by successful compilation.

## Evidence

Renderer converter and seven independently replayed vectors:
`tests/fixtures/macro-host-source.json`; renderer main.3f4b9604.js SHA256
fd20eeea9ac259a741c5deec4cc23f7e293cb99fa8ed658816af50bbd894941d.
See [authoring evidence](MACRO-HOST-ACTIONS.md) for retained extraction provenance.

Current installed mapping_engine.dll, AppEngine4.0.896, SHA256
c64e539437b50da2b1f2e4155d8cbb0fb1c3e1ebaad894caf509bad97e9857f9:

| Procedure | REA Ghidra Evidence | Observation |
| --- | --- | --- |
| 0x18010c0e3 | ev_3c36468a545aeb45e0319c59a5f55d902afd72aa16726a69dba3eca3b9d928b1 | Launch parser reads pathToCheck/startHidden |
| 0x180088c5c | ev_8bf0f185f4acda558a23a4ba2ad9628e503760e91d27b6fc0d5107e6acf97b44 | Device-thread launch calls helper and records failure |
| 0x18011b008 | ev_469629a6bdfd3507e8ac154ce2f55b308b6fe1df683e85468d1bcde26a94d16b | Helper calls base launch and has a fallback on failure |
| 0x1801ac6e0 | ev_d378b0af40f48b19eaa99bedf8659ea816dd8ad02dbff0b61f16f3e194f2d83a | Base path calls CreateProcessW / conditional CreateProcessAsUserW |

Static decompilation establishes these branches, not live timing, clipboard/paste
semantics or child-process completion. Historical4.0.827 addresses are not reused.
Fallback0x18011b736, path-check semantics, start-hidden option layout, current-user
versus elevated token policy and dispatch callback completion need further native
analysis before a Rust executor can reproduce the full contract.

## Remaining work

Verification: 578 core, nine CLI and 308 serial native-app tests pass; six core
and 48 hardware/opt-in native tests remain ignored. All 12 optimized host-action
checks pass and both release binaries build. This includes six new core checks
and one saved-file CLI check. See [validation and artifact hashes](validation/macro-host-planning.json).
No normal app, imported command, clipboard or input action was run.

Implement explicit host-action execution, completion-driven sequencing, cancel
before dispatch, process-handle cleanup, clipboard ownership and paste/recovery.
Do not kill a launched user's application on macro Stop merely to claim rollback.
Verify only owned benign process fixtures before user-command playback. Mixed
live input still requires actual driver routing, physical-held arbitration and
exclusive ownership. No full macro execution, pixel-perfect UI, catalogue hardware
coverage or complete-build sub100 MB acceptance is claimed by this compiler unit.
