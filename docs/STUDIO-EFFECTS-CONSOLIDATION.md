# Studio effects consolidation

This local consolidation combines completed bounded effect units on integration
base `41b6c85`. It preserves that base's Audio, Ambient, Starlight and Reactive
work. It does not enable a missing renderer, physical output or input backend.

| Source unit | Commit | Preserved scope |
| --- | --- | --- |
| Wheel | `5913bc0` | Regional speed/gradient, layer center/direction, continuous renderer and native panel |
| Breathing | `5cb97ab` | Random/fixed and finite playback authoring/rendering; input modes remain gated |
| Spectrum | `bb53b88` | Duration/stop gestures and inspector scrolling |
| Tidal | `c5495d1` | Native authoring, layer geometry and bounded descriptor; radial runtime remains refused |
| Fire | `02267db` | Bounded native API parameters and rebuild semantics; Studio codec retains its fixed source parameters |
| Ripple | `025931f` | Core draft authoring only; native panel and renderer remain TODO |

The commits are combined in one consolidation checkpoint rather than importing
their independent published-base histories. Original evidence and documentation
remain in their feature files. This branch is intended for local root integration
and is not pushed.

## Semantic resolutions

- Audio retains edit IDs2120/2121. Wheel uses2130/2131/2132 and Tidal uses
  2140/2141/2142/2143. Existing generic, Wave, Starlight, Reactive and Ambient
  fields retain their ranges. Field handlers require an open Studio view and
  their owning active effect; stale messages cannot route through another
  inspector's handler.
- Spectrum's scrolled inspector is used for Spectrum only. Shared gradient
  editing still dispatches to Wheel, Wave, Breathing, Starlight and Audio APIs,
  preserving their own limits/defaults and their own properties panels.
- The renderer retains Wheel's saved global origin and Breathing's explicit
  clock constructor. The synthetic-Ambient diagnostic uses the same internal
  builder with its explicit diagnostic flag; normal preview/output still refuse
  Ambient capture. Fire rebuild retains the source random-state sequence.
- Layer-param dirty tracking compares all layer params, preserving both Wheel
  center edits and Tidal's layer-only geometry/direction edits, including escaped
  JSON effect names. Unknown JSON remains raw.
- Wheel and Tidal center picking/markers coexist and are selected by effect
  identity. Existing exact-device/layout, input-ownership, terminal-completion
  and hardware-output gates are unchanged.
- Spectrum cancellation releases its own captured pointer when WM_CANCELMODE
  occurs. Wheel retains its corresponding single-commit/cancellation behavior.
- Native Add Wheel/Tidal tests find the actual painted menu hit rather than
  assuming the effect still occupies the twelfth menu row.

## Integration checks

New core tests compose Wheel, random finite Breathing, Spectrum and Fire with an
explicit clock. They check deterministic frames, origin handling, Fire CRT-state
ownership, and draft/snapshot immutability. They also check hidden versus visible
Tidal/Ripple refusal, Breathing input gating, normal Ambient refusal and the
explicit synthetic-Ambient diagnostic.

A new isolated HWND test switches between Wheel, Tidal, Audio, Spectrum,
Breathing, Fire and Ambient; checks field ownership and stale-message inertness;
saves distinct authored values; and asserts unchanged source/profile-store bytes
and zero simulated transport requests. It captures the integrated panels.
Existing feature-native tests remain separate, serial and opt-in.

Final serial suites pass:692 core tests with9 ignored, one core doc test, and336
tray tests with64 ignored. The optimized full workspace build passes for both
the CLI and tray executable. The ignored native checks are run separately only
for the isolated simulated cases; ignored hardware/input acceptance tests are
not implicitly treated as passes.

Exact commands, final counts, branch/commit, logs and screenshots are recorded in
the existing Wheel task's `outputs/effects-consolidation` handoff. All builds use
jobs1, a task-owned Cargo target/temp directory and read-only shared assets. No
running Lite/Synapse instance, shared profile, driver or physical device stream
is changed.

## Remaining work

This is not full effect parity or pixel/RAM acceptance. Tidal's radial renderer
and Ripple's panel/propagation/input runtime remain absent. Breathing's input
modes and Wheel's finite/input modes retain their documented refusals. Original
per-feature visual, fractional-center, malformed-gradient, native-oracle,
physical/optical and stock re-import gaps remain open. Source snapshots and
synthetic checks do not establish universal device compatibility or total
application RAM below100MB. Root still owns merging this local checkpoint into
the evolving integration branch.
