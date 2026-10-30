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
    report_bytes = (capture / 'dashboard-fetched.json').read_bytes()
    report = json.loads(report_bytes)
    catalog_bytes = catalog.read_bytes()
    if hashlib.sha256(catalog_bytes).hexdigest() != report['catalog_sha256']:
        raise ValueError('Catalogue differs from acquisition report')
    if len(catalog_bytes) > 2 * 1024 * 1024:
        raise ValueError('Catalogue exceeds runtime admission budget')
    if output.exists():
        raise ValueError('Use a new output directory; existing packs are not overwritten')
    output.mkdir(parents=True)
    records, identities = [], {}
    for record in sorted(report['records'], key=lambda r: (r.get('product_id', -1), r.get('edition_id', -1), r.get('layout_id', -1), r.get('fetched_path', ''))):
        if record['status'] != 'available':
            continue
        pid, edition, layout = (record[key] for key in ('product_id', 'edition_id', 'layout_id'))
        if not all(type(value) is int and 0 <= value <= 65535 for value in (pid, edition, layout)) or layout not in (0,1):
            raise ValueError('Invalid image identity')
        relative = record['fetched_path'].lstrip('/')
        expected = rf'synapse/products/{pid}/ui/{pid}_{edition}/PluginImages/{pid}_{edition}_{layout}_dashboard[13]x\.avif'
        if not re.fullmatch(expected, relative):
            raise ValueError('Image path does not match its declared identity')
        path = (capture / relative).resolve(strict=True)
        if not path.is_relative_to(capture.resolve(strict=True)):
            raise ValueError('Image escapes capture directory')
        with path.open('rb') as stream:
            source = stream.read(8 * 1024 * 1024 + 1)
        if len(source) > 8 * 1024 * 1024 or len(source) != record['bytes'] or hashlib.sha256(source).hexdigest() != record['sha256']:
            raise ValueError('Image capture failed size/digest verification')
        identity = (pid, edition)
        definition = (layout, record['sha256'])
        if identity in identities:
            if identities[identity] != definition:
                raise ValueError('Conflicting artwork for exact identity; no arbitrary winner')
            continue
        identities[identity] = definition
        with Image.open(io.BytesIO(source)) as original:
            if original.format != 'AVIF' or getattr(original, 'n_frames', 1) != 1:
                raise ValueError('Expected static AVIF dashboard artwork')
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
        # Retain original encoded bytes, alpha and full resolution separately
        # from the bounded native thumbnail. Native loading stays lazy/unchanged.
        source_file = f"source-{record['sha256']}.avif"
        if not (output / source_file).exists():
            (output / source_file).write_bytes(source)
        records.append({'product_id': pid, 'edition_id': edition, 'layout_id': layout,
            'file': filename, 'width': image.width, 'height': image.height,
            'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest(),
            'source_sha256': record['sha256'], 'source_size': source_size,
            'source_file': source_file, 'source_bytes': len(source),
            'source_path': record['fetched_path']})
    (output / 'catalog.json').write_bytes(catalog_bytes)
    index = {'format': 'synapse-lite-dashboard-pack-v2',
        'capture_report_sha256': hashlib.sha256(report_bytes).hexdigest(),
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
