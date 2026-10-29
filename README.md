# Synapse Lite

A Rust fork of [Secretarybird](https://github.com/infinityInfinite/secbird), based on **v0.5.0-beta1** (`e649877a77f9a4e8573dda55b525a0a258ae9b0e`).

The project keeps Secretarybird's direct device controls and rebuilds the editor around the installed Synapse interface. Layout, typography, assets and profile formats are recovered from actual application artifacts with REA. Screenshots are not the implementation baseline.

Development is in progress. Pixel-for-pixel parity, complete Synapse profile compatibility, complete driver replacement and the final combined memory budget have **not** been established.

## Intended behavior

- Synapse's familiar device controls and navigation, without advertising, telemetry, accounts, offers or automatic network checks.
- A native Rust editor with a combined application working set and private committed memory below 100,000,000 bytes.
- Secretarybird profile compatibility without losing unknown fields.
- Synapse profile import/export based on verified formats, with explicit reporting when a setting cannot be translated.
- Lighting, sensitivity, mappings, macros, profiles and reconnect behavior supported by the attached hardware.

The implementation currently targets an Ornata V2 and DeathAdder V3 HyperSpeed while retaining the upstream device database. Listed device capabilities are not proof of hardware verification.

## Build

On Windows, install stable Rust with the MSVC toolchain, then run:

```powershell
cargo test --workspace --locked
cargo build --release --locked
```

The fork uses its own configuration directory, `%APPDATA%\SynapseLite`. Set `SYNAPSE_LITE_CONFIG_DIR` to choose a test or portable location. It does not edit the original Secretarybird or Synapse profile databases.

The existing CLI remains `secbird.exe`. Profile interchange commands include:

```powershell
secbird import profile.synapse4
secbird export secretarybird-backup.json
secbird -d 00C4 export-synapse4 profile.synapse4
```

Synapse 4 export retains opaque imported settings and supports source-verified default profiles for the installed keyboard and mouse models. Unsupported settings are reported or cause export to fail. Device-wide Power intent is retained in native backups and explicitly reported when a profile-only export cannot represent it. Synapse 3 ZIP profiles are not yet implemented.

Both [actual installed profile payloads](docs/ACTUAL-PROFILES.md) round-trip unchanged through the CLI with independently verified hashes. Live import acceptance in Synapse remains unverified.

## Evidence and remaining work

See [the pinned baseline](docs/BASELINE.json), [acceptance criteria](docs/ACCEPTANCE.md), and the [original upstream documentation](docs/UPSTREAM-README.md).

The source-derived mouse [Performance](docs/PERFORMANCE.md) and [Power](docs/POWER.md) controls are connected to shared device actions. Their reports distinguish source reconstruction, simulator checks, physical setting/readback/restoration checks and memory observations from the remaining visual and interaction gates.

The keyboard [Lighting editor](docs/LIGHTING.md) includes recovered brightness/policy controls, palettes and the color editor. The tray runs idle/display brightness policies. A guarded [host Static path](docs/HOST-STATIC.md) and [Spectrum/fixed Breathing worker](docs/UNIFORM-RUNTIME.md) support the keyboard's existing Driver Mode. A later [control-sampling checkpoint](docs/UNIFORM-CONTROL-CHECKPOINTS.md) improved bounded measured throughput from about11fps to14.27/14.56fps with readable controls unchanged; cadence improvement, other host effects and pixel parity remain unfinished.

An independent [Rust kernel forwarding candidate](docs/KERNEL-BUILD.md) compiled and linked against generated WDK bindings. Its source and pinned Microsoft build dependencies are included separately under experimental/. It has never been loaded or attached to hardware and is not a replacement for the installed drivers.

The native [profile Import/Export modal](docs/PROFILE-MODAL.md) provides worker-backed previews, profile/macro selection, inactive imports, partial corrupt-record warnings and explicit full native backups. Source geometry and converted icon pixels are verified separately from whole-UI parity; the simulated HWND tests do not establish real Synapse re-import acceptance.

Six [active Lighting/color-editor measurements](docs/ACTIVE-LIGHTING-RUNTIME.md), including the actual fixed-Breathing worker, peaked at26,034,176 working-set bytes and11,468,800 sampled private bytes. Those scoped runs pass the100MB threshold; full-build/workflow acceptance is still open. Pure [source macro planning](docs/SOURCE-MACROS.md) and an [explicit keyboard packet bridge](docs/KEYBOARD-SYMBOLIC-BRIDGE.md) preserve recovered formats without sending input; macro execution remains unfinished.

Raw proprietary application bundles, recovered fonts/images and private user profiles are local research material and are not committed to this repository. Local visual assets are provisioned separately into an ignored directory.

The [Customize assignment popup](docs/CUSTOMIZE-ASSIGNMENT.md) now supports source-derived drafts, drawers, Mouse Function/Turbo editing and discard flows. Assignment writes remain disabled where protocol identity is unresolved. [Bounded native storage](docs/NATIVE-STORAGE.md) rejects predictable save failures before device mutation. Six [large-profile export-preview measurements](docs/LARGE-PROFILE-RUNTIME.md), each presenting 1,000 synthetic profiles, peaked at 51,798,016 working-set bytes and 34,291,712 sampled private bytes; they are query-only workload evidence. The guarded [host Wave action and worker](docs/HOST-WAVE.md) now combine source math, authenticated spatial mapping and a fresh layout identity GET. Simulated full-cycle, handoff and failure tests pass. A [two-frame physical diagnostic](docs/PHYSICAL-WAVE.md) accepted 14/14 packets with unchanged readable controls; a subsequent [six-run native Wave workload](docs/ACTIVE-WAVE-RUNTIME.md) passed in both directions at 25,632,768 working-set / 10,932,224 sampled private bytes. Optical output, stock cadence and final-build resource acceptance remain unverified.

Studio now has explicit [saved-composition device playback](docs/STUDIO-RUNTIME.md)
through the existing lighting worker, with selective Quick handoff and a stop control
that cancels packet admission before waiting on the device mutex. Static/Spectrum/Fire
on one authenticated Ornata in existing Driver Mode3 are admitted; mode acquisition,
remaining effects/devices, physical/optical verification and full RAM acceptance remain
unfinished. Saving or local canvas preview alone never starts device output.
The Lighting [brightness slider and switch](docs/STUDIO-BRIGHTNESS.md) now adjust
Studio through LIVE brightness without selecting Quick effects or restarting the
composition. The first accepted Studio frame also releases a previous LIVE0 using
the configured intent; hardware/optical checks remain open.
Studio [Wave](docs/STUDIO-WAVE.md) plays continuous cardinal regions with custom
RGB/None gradients, speed, width and transparent pauses through Local Preview and
the existing guarded Apply Saved output. [Add Wave and its native controls](docs/STUDIO-WAVE-AUTHORING.md)
now assign selected LEDs with source defaults, four directions, five source
presets and atomic speed/width/pause editing, save and undo/redo. Custom-stop
authoring, narrow widths, rotational/split/finite/input-triggered variants and
physical/visual checks remain TODOs.

A Rust [Windows audio endpoint backend](docs/AUDIO-ENDPOINTS.md) now reads playback
device names, IDs, default roles, volume and mute on explicit request. It opens no
audio streams. A separate opt-in [playback capture/analysis backend](docs/AUDIO-CAPTURE.md)
now has bounded PCM decoding, RMS/peak/FFT, stop and reopen. Synthetic numerical
references pass; the live endpoint was idle, so non-silent verification, source
Audio Meter scaling/rendering, device-selection UI and lighting output remain open.

Studio [Breathing](docs/STUDIO-BREATHING.md) now supports deterministic continuous
playback, two-colour regional editing, source speed presets, None/transparency,
preview and saved device output. Random, finite and key-triggered variants remain
explicitly unsupported; imported data is preserved.

## Attribution

Secretarybird is by [infinityInfinite and contributors](https://github.com/infinityInfinite/secbird). Its MIT/Apache-2.0 license files and [NOTICE](NOTICE) are preserved. Razer device protocol facts also draw on [OpenRazer](https://github.com/openrazer/openrazer). This is an independent project.
