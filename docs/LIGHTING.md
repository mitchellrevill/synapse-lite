# Source-derived Ornata Lighting editor

The native editor ports the installed product605 Lighting controls from recovered Synapse JavaScript/CSS, including responsive panel bounds, brightness and idle sliders, display/idle checkboxes, Quick/Advanced selection, twelve source effect choices, preset/custom palettes and the HSV/Hex/RGB color editor. The source boost stepper keeps its recovered decimal validation and quarter-step normalization. Screenshots are not the layout baseline.

Brightness and policy edits use exact device/profile ownership and the shared serialized worker. Policy edits retain brightness intent without issuing a saved brightness write. Adjacent pending edits coalesce only within the same device/profile/preset. Capture cancellation restores local state and prevents a later mouseup from committing. Unknown imported palette values remain opaque while newly authored colors require valid RGB data.

Firmware effect support is deliberately bounded; see the backend coverage below. The twelve recovered choices do not imply twelve working host effects. Screen/audio capture, the Rust animation engine, Chroma synchronization, advanced effects and the native eyedropper remain incomplete. Current installed Driver Mode blocks firmware-only application. No mode acquisition is implemented.

Local fonts, recovered icon artwork and inert CSS-canvas color tables are provisioned into ignored `target/native-assets`; proprietary bundles/assets are not embedded or committed. `native_editor/provision_lighting_assets.cjs` records provenance and creates the lighting assets from explicit local source inputs without executing vendor JavaScript.

This remains a partial native port. GDI rasterization, hover/transition details, all clipping/modal states and the extra35px native status footer have not been proved pixel-identical to Synapse. Source-derived layout tests and native-message checks do not replace that final gate. Physical brightness checks, host-runtime simulator checks and read-only mode evidence are documented separately in [LIGHTING-BRIGHTNESS.md](LIGHTING-BRIGHTNESS.md), [LIGHTING-RUNTIME.md](LIGHTING-RUNTIME.md) and [LIGHTING-MODE.md](LIGHTING-MODE.md).

## Retained profile contract

The brightness switch retains its enabled flag and numeric value; off projects zero and on restores the retained value. Slider zero disables brightness; positive values enable it. Every integer0..100 is accepted. Source names are brightness.isEnabled/value and switchOffLighting.isDisplayOn/isIdleEnabled/idleMinutes. isDisplayOn selects “When display is turned Off”; it is not a display-state reading. Brightness-off disables policy controls without deleting their flags. Zone.off still means effect-off.

Profile.lighting_settings preserves unknown members and remains separate from a device sleep timer. Synapse4 import/export keeps this profile intent and a legacy physical brightness projection. Older Secretarybird versions can discard the new intent when saving; old profiles without typed intent retain their prior behavior. Explicit brightness actions write/read back even unchanged intent to repair drift, while policy-only edits reject simultaneous brightness changes. Preflight failures restore legacy device-key resolution as well as profiles.

Tests cover all101 brightness values, retained flags, opaque data/deletion, budget/ownership/stale-profile refusals and persistence. Source provenance: product605 main.fcf07de2.js SHA256307e2b199037ed53a8fde90d76fc67a9a3766a9361fb152e7820d8bf05f19c65; focused REA parent ev_4226379063083f5c77706b1653aa8bdabb4b73bd8c6e21dd3ebe216f3aca2901. Static/source replay establishes contracts; it does not establish full runtime or pixel equivalence.

See [firmware subset coverage](LIGHTING-EFFECTS.md), [native interaction checks](validation/NATIVE-INTERACTION.md) and [the three-run memory checkpoint](validation/lighting-native-memory.json). This checkpoint peaked at23,392,256 bytes working set and8,577,024 sampled private bytes; complete host-effect/color-editor workloads and final-build acceptance remain pending.
