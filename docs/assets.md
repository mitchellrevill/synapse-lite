# Assets and branding

## Project logo

`assets/logo-source.png` contains the Synapse Lite project mark. The original artwork was produced with OpenAI image generation; app and web exports are built from this source file.

Rebuild derived assets with:

```powershell
./tools/build-brand-assets.ps1
```

This creates the PNG sizes, transparent multi-size app/tray ICOs, embedded native BGRA mark, site favicon and README banner. The native `host-synapse` icon always uses the embedded mark, including Synapse tabs and shared Synapse icons. The legacy editor uses the same PNG. Keep binary resource filenames stable for existing CLI/install tooling.

## Device and control assets

Device artwork, fonts and source-derived control icons are not project branding. They remain separately provisioned local assets; they are not included in this public source tree. The native app reads `SYNAPSE_LITE_ASSETS`, or `assets` beside the executable. Missing packs can result in missing device artwork or controls.

The `provision-*.py` and `provision-*.cjs` tools take explicit local inputs and output paths. Run each tool without arguments to inspect its usage. Python image tools require Pillow; SVG tools take a path to a Sharp module. They do not require those dependencies at runtime.

For public metadata acquisition and offline packs:

```text
python tools/collect-device-manifests.py CATALOG OUTPUT
python tools/provision-device-dashboard.py CAPTURE CATALOG NEW_ARTWORK
python tools/audit-device-assets.py CATALOG CAPTURE NEW_ARTWORK STUDIO_PACK NEW_REPORT
```

The collector is offline by default; `--fetch` explicitly enables bounded fixed-origin downloads. Keep generated packs outside tracked source. The audit verifies identities, paths, sizes and hashes; it does not grant redistribution rights or certify hardware compatibility.

Pixel sidecars contain the eight-byte `SLBGRA1\0` magic, little-endian dimensions and top-down premultiplied BGRA. Native loading validates dimensions, length and alpha before allocation. Do not replace authenticated layout manifests with generic device photographs.
