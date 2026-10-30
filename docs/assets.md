# Assets and branding

## Project logo

`assets/logo-source.png` is the selected original generated mark: three angular green signal paths around an open triangular centre. It is distinct project branding inspired by threefold composition, without the original snake artwork. The unused concepts are not shipped.

The source was created with the built-in image generator. Prompt: “Create a distinctive Synapse Lite emblem; strip the snakes and organic detail from the threefold inspiration and reinterpret it as three minimal interlocking angular signal paths around an open triangular core; bright green, transparent background, no text.”

Rebuild derived assets with:

```powershell
./tools/build-brand-assets.ps1
```

This creates the PNG sizes, transparent multi-size app/tray ICOs, embedded native BGRA mark, site favicon and README banner. The native `host-synapse` icon always uses the embedded mark, including Synapse tabs and shared Synapse icons. The legacy editor uses the same PNG. Keep binary resource filenames stable for existing CLI/install tooling.

## Device and control assets

Device artwork, fonts and source-derived control icons are not project branding. They remain separately provisioned local assets; they are not included in this public source tree. The native app reads `SYNAPSE_LITE_ASSETS`, or `assets` beside the executable. Missing packs can result in missing device artwork or controls.

The retained `provision-*.py` and `provision-*.cjs` tools take explicit local inputs and output paths. Run each tool without arguments to inspect its usage. Python image tools require Pillow; SVG tools take a path to a Sharp module. They do not require those dependencies at runtime.

For public metadata acquisition and offline packs:

```text
python tools/collect-device-manifests.py CATALOG OUTPUT
python tools/provision-device-dashboard.py CAPTURE CATALOG NEW_ARTWORK
python tools/audit-device-assets.py CATALOG CAPTURE NEW_ARTWORK STUDIO_PACK NEW_REPORT
```

The collector is offline by default; `--fetch` explicitly enables bounded fixed-origin downloads. Keep generated packs outside tracked source. The audit verifies identities, paths, sizes and hashes; it does not grant redistribution rights or certify hardware compatibility.

Pixel sidecars contain the eight-byte `SLBGRA1\0` magic, little-endian dimensions and top-down premultiplied BGRA. Native loading validates dimensions, length and alpha before allocation. Do not replace authenticated layout manifests with generic device photographs.
