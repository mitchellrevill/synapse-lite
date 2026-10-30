# Development

Use stable Rust. Windows builds need the MSVC toolchain and Windows SDK. Linux builds target `secbird-core` and `secbird`; `secbird-tray` is Windows-only.

```powershell
cargo test --workspace --locked -j 1 -- --test-threads=1
cargo clippy --workspace --all-targets --locked -j 1 -- -D warnings
cargo build --release --locked
```

Run shared worker/session tests serially. Ordinary tests use local fixtures and simulated transports. Explicitly ignored tests may require a desktop, private assets, network or physical hardware; read each test's requirements before selecting it. Do not run every ignored test indiscriminately.

Asset tools require Python with Pillow for image decoding, and Node.js with a separately supplied Sharp installation for SVG rasterization. Tool usage is documented in [assets](assets.md) and [tools](../tools/README.md). These are development dependencies, not app runtime services.

```powershell
python -m unittest discover -s tools -p "test*.py"
node --test tools/test-provision-window-assets.cjs
```

## Isolated runs

Set `SYNAPSE_LITE_CONFIG_DIR` to a fresh local directory and `SYNAPSE_LITE_ASSETS` to a provisioned pack. `SECBIRD_SIM` enables the upstream simulated device catalogue; consult `secbird --help` for selectors. Simulation does not itself disable physical discovery. Use the dedicated native test harnesses when testing without hardware access.

The owned `host-launch-fixture` example supports opt-in launch tests. `catalog_benchmark` measures local catalogue parsing and lookup. Removed one-off hardware probes are not a supported test interface.

## Change boundaries

Keep device writes in shared engine/worker paths. Preserve connection identity, cancellation, owned-input cleanup and restoration. File inspection must not discover hardware or activate a profile. Never commit user profiles, serial numbers, source application bundles or generated vendor assets.

Source fixtures should be synthetic or clearly document how their reference values were derived. Keep regression tests with the behavior they exercise. Do not replace physical checks with simulator claims or remove a failing test just to obtain a green build.

The CI workflow runs Windows app/core/CLI checks and Linux core/CLI checks. A release build alone does not establish functional or visual parity.
