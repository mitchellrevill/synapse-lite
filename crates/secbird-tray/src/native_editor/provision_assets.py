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

from PIL import Image


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def unique(directory, pattern):
    matches = list(directory.glob(pattern))
    if len(matches) != 1:
        raise ValueError(f"Expected exactly one {pattern} in {directory}; found {len(matches)}")
    return matches[0]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--evidence", type=pathlib.Path, required=True)
    parser.add_argument("--output", type=pathlib.Path, required=True)
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
    (output / "provenance.json").write_text(json.dumps(manifest, indent=2), encoding="utf-8")
    print(f"Provisioned {len(manifest['assets'])} local assets into {output}")


if __name__ == "__main__":
    main()
