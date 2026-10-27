# Acceptance criteria

The project is complete only when all requirements below are demonstrated against the current build. A passing parser test, a device database entry, or a successful idle-memory check is insufficient for broader claims.

| Requirement | Required evidence | Current state |
| --- | --- | --- |
| Fork the requested beta baseline | GitHub fork relationship and exact upstream tag/commit | Fork created; baseline pinned to v0.5.0-beta1 |
| Rust implementation using Secretarybird functionality | Buildable fork, source review and functional paths | Fork release builds; native editor and shared device dispatcher integrated |
| Pixel-for-pixel Synapse interface without bloat | REA source/component/cascade recovery, exact fonts/assets, all required screen states, rendered validation | Source Performance/Power/Lighting controls, palettes/color editor and keyboard geometry integrated; native resize/capture tests pass; full native UI parity unproven |
| No ads, telemetry, account requirement or bundled offers | Runtime entry-point and dependency/source review plus network observations | Automatic network timer and update popups removed; no TCP observed in seven measured views; longer verification pending |
| Cross-compatible Secretarybird profiles | Native format round trips, unknown-field preservation, typed edits and conflict-safe imports | Expanded codec, aggregate-budget, identity, rename and connection-mode tests pass; earlier CLI round trip through the actual upstream beta passed; upstream still drops unknown fields when saving |
| Cross-compatible Synapse profiles | Verified format codec, translated settings, opaque-field preservation, re-import into real Synapse | Source-verified Synapse 4 codec translates DPI stages including hidden/disabled/independent intent, sleep, unconditional game mode and supported mappings; real-app round trip pending; macros, linked apps and Synapse 3 unresolved |
| Installed Ornata V2 functionality | Actual lighting/effect/mapping/macro/profile/restore tests per advertised feature | Saved/live brightness and exact raw restoration passed; policy runtime passes simulator checks; physical mode GET reports Driver Mode3 and blocks firmware effects; actual host effects, display/idle workflows, mappings/macros and full recovery remain pending |
| Installed DeathAdder V3 HyperSpeed functionality | Actual wired/wireless sensitivity, polling, mappings/macros/profile/restore and reconnect tests | Fork wired DPI/polling/sleep-timeout/low-power-threshold/stage, hidden/disabled projection and activation checks with exact raw restoration passed; wireless/input/sleep-wake workflows pending |
| Driver/API coverage | Driver roles, control ABI evidence, implemented behavior and safe input/recovery verification | HID and custom control ABI partially recovered; read-only RZCONTROL queries pass; client-close selector lifetime identified; no Rust kernel driver replacement |
| Stock RAM and CPU baselines | Repeatable sampled stock process/service inventory, metric definitions and workload | One 60-second desktop-state baseline saved locally; broader workload baseline pending |
| Combined memory below 100,000,000 bytes | Three representative runs of the final build including editor, tray and every replacement helper; working set and private bytes; declared stock-service state | Three current Lighting runs peak at 23,392,256 working-set / 8,577,024 sampled private bytes at125% DPI; earlier Performance/Power checkpoints also below100 MB; host-effect/color-editor workloads and final-build acceptance pending |
| Preserve input and recovery | Failed-command handling, safe restore, profile backup/import, hotplug, sleep/wake and process-exit tests | Controlled error recovery and captured-register restoration passed; hotplug, abrupt termination, full input and sleep/wake recovery remain unverified |

The UI reference deliberately excludes advertising/promotional/account/telemetry content requested for removal. Required device-control screens must not be replaced with static mockups. Missing functionality must remain visible as incomplete rather than being hidden by narrower acceptance criteria.

Private hardware evidence and actual profile documents remain outside the public source repository. Synthetic fixtures are identified as synthetic.
