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

The keyboard [Lighting editor](docs/LIGHTING.md) includes recovered brightness/policy controls, palettes and the color editor. The tray runs idle/display brightness policies. Firmware effects have a guarded subset; the installed keyboard's Driver Mode currently prevents their application. Host animations and pixel parity remain unfinished.

Raw proprietary application bundles, recovered fonts/images and private user profiles are local research material and are not committed to this repository. Local visual assets are provisioned separately into an ignored directory.

## Attribution

Secretarybird is by [infinityInfinite and contributors](https://github.com/infinityInfinite/secbird). Its MIT/Apache-2.0 license files and [NOTICE](NOTICE) are preserved. Razer device protocol facts also draw on [OpenRazer](https://github.com/openrazer/openrazer). This is an independent project.
