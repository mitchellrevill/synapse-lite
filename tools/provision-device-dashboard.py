"""Build a private native dashboard pack from verified inert AVIF captures.

Does not fetch anything or execute vendor code. Requires Pillow's AVIF decoder.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import io
from PIL import Image


def build(capture, catalog, output):
    report = json.loads((capture / 'dashboard-fetched.json').read_text(encoding='utf-8'))
    catalog_bytes = catalog.read_bytes()
    if hashlib.sha256(catalog_bytes).hexdigest() != report['catalog_sha256']:
        raise ValueError('Catalogue differs from acquisition report')
    if len(catalog_bytes) > 2 * 1024 * 1024:
        raise ValueError('Catalogue exceeds runtime admission budget')
    if output.exists():
        raise ValueError('Use a new output directory; existing packs are not overwritten')
    output.mkdir(parents=True)
    records = []
    for record in report['records']:
        if record['status'] != 'available':
            continue
        pid, edition, layout = (record[key] for key in ('product_id', 'edition_id', 'layout_id'))
        if not all(type(value) is int and 0 <= value <= 65535 for value in (pid, edition, layout)):
            raise ValueError('Invalid image identity')
        relative = record['fetched_path'].lstrip('/')
        expected = rf'synapse/products/{pid}/ui/{pid}_{edition}/PluginImages/{pid}_{edition}_{layout}_dashboard[13]x\.avif'
        if not re.fullmatch(expected, relative):
            raise ValueError('Image path does not match its declared identity')
        path = (capture / relative).resolve(strict=True)
        if not path.is_relative_to(capture.resolve(strict=True)):
            raise ValueError('Image escapes capture directory')
        source = path.read_bytes()
        if len(source) != record['bytes'] or hashlib.sha256(source).hexdigest() != record['sha256']:
            raise ValueError('Image capture failed size/digest verification')
        with Image.open(path) as original:
            if original.width * original.height > 16_000_000:
                raise ValueError('Unexpected dashboard image dimensions')
            image = original.convert('RGBA')
            source_size = list(image.size)
        image.thumbnail((768, 512), Image.Resampling.LANCZOS)
        encoded = io.BytesIO()
        image.save(encoded, format='PNG', optimize=True)
        data = encoded.getvalue()
        filename = f'dashboard-{hashlib.sha256(data).hexdigest()}.png'
        if not (output / filename).exists():
            (output / filename).write_bytes(data)
        records.append({'product_id': pid, 'edition_id': edition, 'layout_id': layout,
            'file': filename, 'width': image.width, 'height': image.height,
            'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest(),
            'source_sha256': record['sha256'], 'source_size': source_size,
            'source_path': record['fetched_path']})
    (output / 'catalog.json').write_bytes(catalog_bytes)
    index = {'format': 'synapse-lite-dashboard-pack-v2',
        'catalog_sha256': report['catalog_sha256'], 'images': records,
        'role': 'category-default dashboard artwork; no physical layout or hardware capability claim'}
    (output / 'index.json').write_text(json.dumps(index, indent=2), encoding='utf-8')
    return len(records)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('capture', type=Path)
    parser.add_argument('catalog', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    print(json.dumps({'images': build(args.capture, args.catalog, args.output), 'output': str(args.output)}))
