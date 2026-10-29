"""Project hash-verified local Chroma presets. Never fetch or execute vendor code."""
import argparse
import base64
import hashlib
import json
import math
from pathlib import Path, PurePosixPath
import re
import xml.etree.ElementTree as ET

MAX_MANIFEST = 8 * 1024 * 1024
MAX_LAYOUT = 2 * 1024 * 1024
NUMBER = re.compile(r'[-+]?(?:\d*\.\d+|\d+\.?\d*)(?:[eE][-+]?\d+)?')


def sha(blob):
    return hashlib.sha256(blob).hexdigest()


def project(document, manifest_sha, product):
    if not isinstance(document, dict):
        raise ValueError('Manifest is not an object')
    if document.get('productId') != product or document.get('vendorId') != 5426:
        raise ValueError('Manifest product/vendor mismatch')
    configs, svgs = document.get('ledConfig'), document.get('svg')
    if not isinstance(configs, dict) or not isinstance(svgs, dict):
        raise ValueError('Manifest requires explicit LED and SVG maps')
    for name, config in configs.items():
        match = re.fullmatch(r'ledConfig_(\d+)_(\d+)', name)
        if not match:
            yield {'todo': 'Unrecognized layout key', 'key': name}
            continue
        edition, layout = map(int, match.groups())
        row = {'product_id': product, 'edition_id': edition, 'layout_id': layout}
        try:
            if not all(0 <= value <= 65535 for value in [product, edition, layout]):
                raise ValueError('Identity exceeds native range')
            for field, expected in [('VID', 5426), ('PID', product), ('EID', edition), ('Layout', layout)]:
                if field in config and config[field] != expected:
                    raise ValueError('LED configuration identity mismatch')
            encoded = svgs.get(f'svg_{edition}_{layout}')
            if not encoded:
                raise ValueError('No exact SVG; no layout fallback')
            svg = base64.b64decode(encoded, validate=True)
            root = ET.fromstring(svg)
            view_box = [float(value) for value in root.attrib['viewBox'].split()]
            if len(view_box) != 4 or not all(math.isfinite(x) for x in view_box) or min(view_box[2:]) <= 0:
                raise ValueError('Invalid or absent explicit viewBox')
            for element in root.iter():
                if element.tag.rsplit('}', 1)[-1] == 'style' and 'transform' in (element.text or '').lower():
                    raise ValueError('Untraced stylesheet transform')
            mapped = {tuple(item['MatrixPos']) for item in config['LedMatrix']}
            if not mapped or len(mapped) > 512 or any(len(p) != 2 or any(type(v) is not int or not 0 <= v <= 65535 for v in p) for p in mapped):
                raise ValueError('Invalid or excessive matrix identity')
            parents = {child: parent for parent in root.iter() for child in parent}
            entries, seen = [], set()
            for element in root.iter():
                if 'selection' not in element.get('class', '').split():
                    continue
                shape = element.tag.rsplit('}', 1)[-1]
                if shape not in ('path', 'rect'):
                    raise ValueError('Untraced non-path selection shape')
                ancestor = element
                while ancestor is not None:
                    if ancestor is not root and ancestor.tag.rsplit('}', 1)[-1] == 'svg':
                        raise ValueError('Untraced nested SVG viewport')
                    if ancestor.get('transform') or 'transform' in ancestor.get('style', '').lower():
                        raise ValueError('Untraced selection/ancestor transform')
                    ancestor = parents.get(ancestor)
                key = (int(element.attrib['data-row']), int(element.attrib['data-col']))
                if key in seen or key not in mapped:
                    raise ValueError('Duplicate or unmapped SVG LED identity')
                seen.add(key)
                if shape == 'path':
                    path = element.attrib['d']
                else:
                    # Plain SVG rectangles map exactly to the same four straight edges.
                    def number(name, default=None):
                        raw = element.get(name, default)
                        value = float(raw)
                        if not math.isfinite(value):
                            raise ValueError('Invalid rectangle coordinate')
                        return value
                    if number('rx', '0') != 0 or number('ry', '0') != 0:
                        raise ValueError('Untraced rounded rectangle')
                    x, y = number('x', '0'), number('y', '0')
                    width, height = number('width'), number('height')
                    if width <= 0 or height <= 0:
                        raise ValueError('Invalid rectangle size')
                    path = f'M{x} {y}H{x + width}V{y + height}H{x}Z'
                if len(path.encode('utf-8')) > 16384 or any(c not in 'MmLlHhVvCcZz ,\t\n\r' for c in NUMBER.sub('', path)):
                    raise ValueError('Untraced or excessive SVG path command')
                entries.append({'inputID': f'{key[0]}:{key[1]}', 'buttonKey': element.get('data-selection-group', ''),
                    'counter': element.get('data-assumed-key-name', ''), 'd': path, 'isEnabled': True})
            if not entries or len(entries) > 512:
                raise ValueError('No usable or excessive selection paths')
            data = {'format': 'synapse-studio-key-regions-v1', **row, 'view_box': view_box,
                'source_sha256': sha(svg), 'manifest_sha256': manifest_sha, 'keys': entries}
            blob = json.dumps(data, ensure_ascii=False, separators=(',', ':')).encode('utf-8')
            if len(blob) > MAX_LAYOUT:
                raise ValueError('Projected layout exceeds2MiB')
            yield {**row, 'file': f'studio-{product}-{edition}-{layout}.json', 'bytes': len(blob),
                'sha256': sha(blob), 'svg_sha256': sha(svg), 'manifest_sha256': manifest_sha,
                'leds': len(entries), 'matrix_without_selection_path': [list(key) for key in sorted(mapped - seen)], '_blob': blob}
        except (ValueError, KeyError, TypeError, ET.ParseError) as error:
            yield {**row, 'todo': str(error)}


def provision(report, source_root, output, expected_report_sha):
    report_bytes = report.read_bytes()
    if sha(report_bytes) != expected_report_sha:
        raise ValueError('Capture report hash mismatch')
    capture = json.loads(report_bytes)
    if capture.get('stage') != 'lighting':
        raise ValueError('Expected lighting capture report')
    # A new private directory is required; never replace an installed pack.
    output.mkdir(exist_ok=False)
    records, errors, written, conflicts = [], [], {}, set()
    for item in capture['records']:
        if item.get('status') != 'available':
            continue
        try:
            relative = PurePosixPath(item['fetched_path'])
            if not relative.is_absolute() or '..' in relative.parts or '\\' in str(relative) or ':' in str(relative):
                raise ValueError('Unsafe captured asset path')
            path = source_root.joinpath(*relative.parts[1:])
            blob = path.read_bytes()
            if len(blob) > MAX_MANIFEST or len(blob) != item['bytes'] or sha(blob) != item['sha256']:
                raise ValueError('Manifest size/hash mismatch')
            document = json.loads(blob.decode('utf-8-sig'))
            for row in project(document, item['sha256'], item['product_id']):
                filename = row.get('file')
                if not filename:
                    errors.append(row)
                    continue
                data = row.pop('_blob')
                if filename in conflicts:
                    continue
                if filename in written:
                    if written[filename]['sha256'] != row['sha256']:
                        # Two edition files may carry identical geometry but different manifest digests.
                        old = json.loads((output / filename).read_bytes())
                        new = json.loads(data)
                        old.pop('manifest_sha256', None); new.pop('manifest_sha256', None)
                        if old != new:
                            (output / filename).unlink()
                            written.pop(filename)
                            conflicts.add(filename)
                            errors.append({'file': filename, 'todo': 'Conflicting source layout definitions; no winner selected'})
                    continue
                (output / filename).write_bytes(data)
                written[filename] = row
        except (ValueError, KeyError, TypeError, OSError) as error:
            errors.append({'product_id': item.get('product_id'), 'edition_id': item.get('edition_id'), 'todo': str(error)})
    records = list(written.values())
    index = {'format': 'synapse-lite-studio-presets-v1', 'capture_report_sha256': expected_report_sha,
        'records': records, 'todos': errors, 'hardware_support_verified': False}
    (output / 'index.json').write_text(json.dumps(index, indent=2), encoding='utf-8')
    return {'layouts': len(records), 'products': len({r['product_id'] for r in records}), 'todos': len(errors),
        'bytes': sum(r['bytes'] for r in records), 'index_sha256': sha((output / 'index.json').read_bytes())}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('report', type=Path)
    parser.add_argument('source_root', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--report-sha256', required=True)
    args = parser.parse_args()
    print(json.dumps(provision(args.report, args.source_root, args.output, args.report_sha256)))
