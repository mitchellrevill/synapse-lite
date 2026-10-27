# Acceptance criteria

The project is complete only when all requirements below are demonstrated against the current build. A passing parser test, a device database entry, or a successful idle-memory check is insufficient for broader claims.

| Requirement | Required evidence | Current state |
| --- | --- | --- |
| Fork the requested beta baseline | GitHub fork relationship and exact upstream tag/commit | Fork created; baseline pinned to v0.5.0-beta1 |
| Rust implementation using Secretarybird functionality | Buildable fork, source review and functional paths | Fork release builds; native editor and shared device dispatcher integrated |
| Pixel-for-pixel Synapse interface without bloat | REA source/component/cascade recovery, exact fonts/assets, all required screen states, rendered validation | Source rules and fonts recovered; full native UI parity unproven |
| No ads, telemetry, account requirement or bundled offers | Runtime entry-point and dependency/source review plus network observations | Automatic network timer and update popups removed; no TCP observed in three measured views; longer verification pending |
| Cross-compatible Secretarybird profiles | Native format round trips, unknown-field preservation, typed edits and conflict-safe imports | Eleven compatibility tests and CLI round trip through the actual upstream beta passed; upstream still drops unknown fields when saving |
| Cross-compatible Synapse profiles | Verified format codec, translated settings, opaque-field preservation, re-import into real Synapse | Synapse 4 codec in development; real-app round trip pending; Synapse 3 unresolved |
| Installed Ornata V2 functionality | Actual lighting/effect/mapping/macro/profile/restore tests per advertised feature | Earlier independent client verified brightness and firmware only |
| Installed DeathAdder V3 HyperSpeed functionality | Actual wired/wireless sensitivity, polling, mappings/macros/profile/restore and reconnect tests | Earlier clients read wired status; full fork validation pending |
| Driver/API coverage | Driver roles, control ABI evidence, implemented behavior and safe input/recovery verification | HID and custom control ABI partially recovered; no Rust kernel driver replacement |
| Stock RAM and CPU baselines | Repeatable sampled stock process/service inventory, metric definitions and workload | One 60-second desktop-state baseline saved locally; broader workload baseline pending |
| Combined memory below 100,000,000 bytes | Three representative runs of the final build including editor, tray and every replacement helper; working set and private bytes; declared stock-service state | Three actual rendered views passed below 100 MB (max working set 22,241,280 bytes); complete workflow coverage and final-build acceptance pending |
| Preserve input and recovery | Failed-command handling, safe restore, profile backup/import, hotplug, sleep/wake and process-exit tests | Incomplete |

The UI reference deliberately excludes advertising/promotional/account/telemetry content requested for removal. Required device-control screens must not be replaced with static mockups. Missing functionality must remain visible as incomplete rather than being hidden by narrower acceptance criteria.

Private hardware evidence and actual profile documents remain outside the public source repository. Synthetic fixtures are identified as synthetic.
