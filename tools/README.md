# Maintained tools

The CLI stays `secbird`; these are development and asset utilities, not additional app services. Outputs should go to ignored or external directories.

| Group | Entry points |
| --- | --- |
| Project branding | `build-brand-assets.ps1` converts the chosen logo into app/tray/native/web assets and the README banner. |
| Runtime measurements | `Measure-Runtime.ps1`, `measure-process-baseline.ps1`. |
| Stock comparison | `Measure-StockRuntime.ps1`, `measure-stock-razer.ps1`, `stock-runtime-counters.ps1`, `stock-process-snapshot.py`; their tests cover counter admission. |
| Catalogue and packs | `collect-device-manifests.py`, `audit-device-assets.py`, `provision-device-dashboard.py`, `provision-studio-presets.py`. |
| Local UI assets | Remaining `provision-*.cjs` / `provision-*.py` commands for control icons, geometry and artwork. |
| Source references | `derive_fire_parameters.py`, `derive-studio-wheel-reference.py`, `reference-breathing-random.py`, `verify-studio-projection.cjs`, `verify-studio-property-defaults.cjs`. |
| Device facts | `restate_openrazer.py` regenerates factual database declarations from an explicitly selected upstream checkout. |
| Shared SVG math | `svg_affine.py`, used by the pack preparation and validation tools. |

Python tests use synthetic temporary inputs:

```powershell
python -m unittest discover -s tools -p "test*.py"
node --test tools/test-provision-window-assets.cjs
./tools/Test-StockRuntimeCounters.ps1
```

Read the selected tool's arguments before use. Python image tools require Pillow; Node SVG tools take a separately installed Sharp module. A tool missing its private local inputs is not a reason to download or execute an application bundle. See [asset setup](../docs/assets.md) and [measurements](../docs/performance.md).
