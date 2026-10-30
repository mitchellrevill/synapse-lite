# Bounded strict CI cleanup

Based on `9603b7c85fa86febb324e385c0724c9b5410b2a4`, the latest committed consolidation head when the isolated worktree was created. Root later reported `9372cb1` with documentation and test-only viewport corrections; those later changes were not copied into this fixed baseline.

The cleanup fixes mechanical Clippy findings without changing source geometry or enabling hardware operations. It boxes the assignment job payload, preserves the same error report without unwrapping a known error, uses equivalent ranges/ceil division/default construction, moves test modules after production items, and keeps assertion-only APIs in test builds. Five float-literal edits retain the same f32 bits. No visible text changed.

The source-matching drawing helpers retain their signatures. Each necessary `too_many_arguments` allowance is attached to one drawing/FFI wrapper with an explicit rationale, as authorized by root. CI flags and workflow files are unchanged. Two never-constructed disabled Chroma command tags were removed; unavailable UI text and all active features remain.

Strict release Clippy passes for core and CLI across all targets. Workspace strict Clippy is **not green**: the app test target has 11 remaining findings, all in Studio files. A separate production check enumerates the remaining Studio diagnostics. Two retained audio API contracts have specifically authorized dead-code allowances and accurate TODO comments. The exact diagnostics, commands, hashes and release test results are in `validation/strict-ci-cleanup.json`.

Studio files are unchanged in this commit. Small mechanical and permission-restoration patches are supplied in the originating task outputs for root to review after the Wheel/Gradient fixes. `OutputSession::apply_saved` argument grouping and the unused Studio backend accessors remain Studio-owner decisions. Audio endpoint selection and bounded meter readback were retained with individual, root-authorized dead-code allowances and TODO explanations; no integrated runtime consumer is claimed.

Release workspace tests pass: 764 core, 369 app, 12 CLI and one documentation test. The declared suites retain 9 core and 74 app ignored tests; the native/font fixtures were not rerun by this bounded cleanup. Assignment composition and the local worker/no-delivery test pass.

One inherited stalled Dynamic Sensitivity test remains explicitly excluded from the release test command. This is not an unqualified complete-suite or final-product acceptance claim. Work used an owned Cargo target and TEMP/TMP, one build job and serial tests. No push, root edits, new chat, physical discovery/input/device SET, clipboard, driver/service operation or stopping another Lite instance occurred.
