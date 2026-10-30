"""Provision local native UI assets from an operator-selected recovered cache.

Usage:
    python provision_assets.py --evidence <synapse-renderer-directory> --output <assets>

This reads local evidence only. It neither downloads nor executes vendor code,
installs fonts system-wide, nor copies any JavaScript/CSS. Pillow is needed to
decode the original product AVIFs. Generated fonts/artwork are private runtime
assets and must remain excluded from public source commits.
"""
import argparse
import hashlib
import json
import pathlib
import shutil
import struct

from PIL import Image


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def unique(directory, pattern):
    matches = list(directory.glob(pattern))
    if len(matches) != 1:
        raise ValueError(f"Expected exactly one {pattern} in {directory}; found {len(matches)}")
    return matches[0]


def save_alpha(image, target):
    """Write top-down premultiplied BGRA for Win32 AC_SRC_ALPHA."""
    pixels = bytearray(image.width * image.height * 4)
    for index, (red, green, blue, alpha) in enumerate(image.getdata()):
        offset = index * 4
        pixels[offset:offset + 4] = bytes((
            (blue * alpha + 127) // 255,
            (green * alpha + 127) // 255,
            (red * alpha + 127) // 255,
            alpha,
        ))
    target.write_bytes(b"SLBGRA1\0" + struct.pack("<II", image.width, image.height) + pixels)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--evidence", type=pathlib.Path, required=True)
    parser.add_argument("--output", type=pathlib.Path, required=True)
    parser.add_argument("--icons-from", type=pathlib.Path,
                        help="Optional directory of already rasterized, source-provenanced icon-*.bmp/SVG assets")
    args = parser.parse_args()
    evidence = args.evidence.resolve(strict=True)
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    products = evidence / "cache/apps.razer.com/synapse/products"
    manifest = {"format": "synapse-lite-local-assets-v1", "assets": [],
                "note": "Recovered product artwork and private fonts; no vendor program code."}
    for name in ("Roboto-Regular.ttf", "Roboto-Light.ttf", "RazerF5-Regular.ttf"):
        source = evidence / "fonts-sfnt" / name
        target = output / name
        shutil.copyfile(source, target)
        manifest["assets"].append({"file": name, "sha256": sha256(target),
                                   "source_sha256": sha256(source), "kind": "font"})
    # These exact names follow the observed 196_0/605_0 webpack asset contexts.
    # The keyboard's model art is not a claim about the user's physical key layout.
    for product, pattern, name in (
        ("196", "prd-3x.311c6ba5.avif*", "mouse.bmp"),
        ("196", "litat2m-profile-button-3x.e01e76b1.avif*", "icon-assignment-mouse-cycle.bmp"),
        ("605", "prd.ecc687ec.avif*", "keyboard.bmp"),
    ):
        source = unique(products / product / "ui/static/media", pattern)
        image = Image.open(source).convert("RGBA")
        background = Image.new("RGBA", image.size, (34, 34, 34, 255))
        background.alpha_composite(image)
        target = output / name
        background.convert("RGB").save(target, "BMP")
        manifest["assets"].append({"file": name, "sha256": sha256(target),
                                   "source_sha256": sha256(source), "source_name": source.name,
                                   "width": image.width, "height": image.height, "kind": "artwork"})
        alpha_target = target.with_suffix(".bgra")
        save_alpha(image, alpha_target)
        manifest["assets"].append({"file": alpha_target.name, "sha256": sha256(alpha_target),
                                   "source_sha256": sha256(source), "kind": "premultiplied-bgra",
                                   "width": image.width, "height": image.height})
    observation = evidence / 'physical-layout-observation.json'
    geometry = evidence / 'ornata-key-regions.json'
    if observation.is_file() and geometry.is_file():
        observed = json.loads(observation.read_text(encoding='utf-8'))
        layout = json.loads(geometry.read_text(encoding='utf-8'))
        if all(observed.get(key) == layout.get(key) for key in ('product_id', 'edition_id', 'layout_id')):
            # This exact cached product/layout tuple was recovered; no locale inference.
            if (layout['product_id'], layout['edition_id'], layout['layout_id']) == (605, 0, 6):
                target = output / 'keyboard-layout.json'
                shutil.copyfile(geometry, target)
                manifest['assets'].append({'file': target.name, 'sha256': sha256(target),
                                           'kind': 'source-key-geometry', 'source_sha256': layout['source_sha256']})
                source = unique(products / '605/ui/static/media', '6-1x.2f1978eb.avif*')
                image = Image.open(source).convert('RGBA')
                background = Image.new('RGBA', image.size, (34, 34, 34, 255))
                background.alpha_composite(image)
                target = output / 'keyboard-customize.bmp'
                background.convert('RGB').save(target, 'BMP')
                manifest['assets'].append({'file': target.name, 'sha256': sha256(target),
                                           'source_sha256': sha256(source), 'kind': 'layout-artwork',
                                           'width': image.width, 'height': image.height})
                alpha_target = target.with_suffix('.bgra')
                save_alpha(image, alpha_target)
                manifest['assets'].append({'file': alpha_target.name, 'sha256': sha256(alpha_target),
                                           'source_sha256': sha256(source), 'kind': 'premultiplied-bgra',
                                           'width': image.width, 'height': image.height})
    if args.icons_from:
        icon_dir = args.icons_from.resolve(strict=True)
        for source in icon_dir.iterdir():
            if source.is_file() and (source.name == 'icon-provenance.json'
                                    or source.name.startswith('icon-') and source.suffix in ('.bmp', '.svg', '.bgra')):
                target = output / source.name
                if source.resolve() != target.resolve():
                    shutil.copyfile(source, target)
                manifest['assets'].append({'file': target.name, 'sha256': sha256(target), 'kind': 'source-icon'})
    (output / "provenance.json").write_text(json.dumps(manifest, indent=2), encoding="utf-8")
    print(f"Provisioned {len(manifest['assets'])} local assets into {output}")


if __name__ == "__main__":
    main()
