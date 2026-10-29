"""Project hash-verified local Chroma presets. Never fetch or execute vendor code."""
import argparse
import base64
import hashlib
import json
import math
from pathlib import Path, PurePosixPath
import re
import xml.etree.ElementTree as ET
import svg_affine

MAX_MANIFEST = 8 * 1024 * 1024
MAX_LAYOUT = 2 * 1024 * 1024
GEOMETRY_STYLE = re.compile(r'(?<![\w-])(?:x|y|cx|cy|r|rx|ry|width|height)\s*:', re.I)
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
                if element.tag.rsplit('}', 1)[-1] == 'style' and ('transform' in (element.text or '').lower() or GEOMETRY_STYLE.search(element.text or '')):
                    raise ValueError('Untraced stylesheet geometry')
            mapped = {tuple(item['MatrixPos']) for item in config['LedMatrix']}
            if not mapped or len(mapped) > 512 or any(len(p) != 2 or any(type(v) is not int or not 0 <= v <= 65535 for v in p) for p in mapped):
                raise ValueError('Invalid or excessive matrix identity')
            parents = {child: parent for parent in root.iter() for child in parent}
            entries, seen = [], set()
            for element in root.iter():
                if 'selection' not in element.get('class', '').split():
                    continue
                shape = element.tag.rsplit('}', 1)[-1]
                if shape not in ('path', 'rect', 'circle', 'ellipse', 'polygon', 'polyline'):
                    raise ValueError('Untraced non-path selection shape')
                ancestor = element
                matrix = svg_affine.IDENTITY
                depth = 0
                while ancestor is not None:
                    if ancestor is not root and ancestor.tag.rsplit('}', 1)[-1] == 'svg':
                        raise ValueError('Untraced nested SVG viewport')
                    if GEOMETRY_STYLE.search(ancestor.get('style', '')):
                        raise ValueError('Untraced inline geometry style')
                    if 'transform' in ancestor.get('style', '').lower():
                        raise ValueError('Untraced CSS transform')
                    matrix = svg_affine.multiply(svg_affine.parse(ancestor.get('transform', '')), matrix)
                    depth += 1
                    if depth > 64:
                        raise ValueError('Excessive SVG nesting')
                    ancestor = parents.get(ancestor)
                key = (int(element.attrib['data-row']), int(element.attrib['data-col']))
                if key in seen or key not in mapped:
                    raise ValueError('Duplicate or unmapped SVG LED identity')
                seen.add(key)
                def number(name, default=None):
                    value = float(element.get(name, default))
                    if not math.isfinite(value) or abs(value) > 1_000_000:
                        raise ValueError('Invalid or excessive shape coordinate')
                    return value
                entry = {'inputID': f'{key[0]}:{key[1]}', 'buttonKey': element.get('data-selection-group', ''),
                    'counter': element.get('data-assumed-key-name', ''), 'isEnabled': True}
                path = None
                if shape == 'path':
                    path = element.attrib['d']
                elif shape in ('polygon', 'polyline'):
                    points = element.attrib['points']
                    if NUMBER.sub('', points).strip(' ,\t\n\r'):
                        raise ValueError('Unsupported polygon/polyline points')
                    values = [float(match.group()) for match in NUMBER.finditer(points)]
                    if len(values) < 6 or len(values) % 2 or not all(math.isfinite(v) and abs(v) <= 1_000_000 for v in values):
                        raise ValueError('Invalid polygon/polyline points or unsupported line-only hit area')
                    path = 'M' + 'L'.join(f'{values[i]} {values[i+1]}' for i in range(0, len(values), 2))
                    if shape == 'polygon':
                        path += 'Z'
                elif shape in ('circle', 'ellipse'):
                    if shape == 'circle':
                        rx = ry = number('r')
                    else:
                        rx, ry = number('rx'), number('ry')
                    if min(rx, ry) <= 0:
                        raise ValueError('Non-rendering or invalid ellipse radius')
                    entry['shape'] = {'kind': 'ellipse', 'cx': number('cx', '0'), 'cy': number('cy', '0'), 'rx': rx, 'ry': ry}
                else:
                    x, y = number('x', '0'), number('y', '0')
                    width, height = number('width'), number('height')
                    if width <= 0 or height <= 0:
                        raise ValueError('Invalid rectangle size')
                    rx_raw, ry_raw = element.get('rx'), element.get('ry')
                    rx = number('rx') if rx_raw not in (None, 'auto') else (number('ry') if ry_raw not in (None, 'auto') else 0.)
                    ry = number('ry') if ry_raw not in (None, 'auto') else rx
                    if rx < 0 or ry < 0:
                        raise ValueError('Negative rectangle radius')
                    rx, ry = min(rx, width / 2), min(ry, height / 2)
                    if min(rx, ry) == 0:
                        path = f'M{x} {y}H{x + width}V{y + height}H{x}Z'
                    else:
                        entry['shape'] = {'kind': 'rounded-rect', 'x': x, 'y': y, 'width': width, 'height': height, 'rx': rx, 'ry': ry}
                if path is not None:
                    if len(path.encode('utf-8')) > 16384 or any(c not in 'MmLlHhVvCcZz ,\t\n\r' for c in NUMBER.sub('', path)):
                        raise ValueError('Untraced or excessive SVG path command')
                    entry['d'] = path
                svg_affine.apply(entry, matrix)
                entries.append(entry)
            if not entries or len(entries) > 512:
                raise ValueError('No usable or excessive selection paths')
            data = {'format': 'synapse-studio-key-regions-v1', **row, 'view_box': view_box,
                'source_sha256': sha(svg), 'manifest_sha256': manifest_sha, 'keys': entries}
            blob = json.dumps(data, ensure_ascii=False, separators=(',', ':')).encode('utf-8')
            if len(blob) > MAX_LAYOUT:
                raise ValueError('Projected layout exceeds2MiB')
            yield {**row, 'file': f'studio-{product}-{edition}-{layout}.json', 'bytes': len(blob),
                'product_name': document.get('productName') if isinstance(document.get('productName'), str) else '',
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
    for item in sorted(capture['records'], key=lambda r: (r.get('product_id', -1), r.get('edition_id', -1), r.get('fetched_path', ''))):
        if item.get('status') != 'available':
            continue
        try:
            relative = PurePosixPath(item['fetched_path'])
            if not relative.is_absolute() or '..' in relative.parts or '\\' in str(relative) or ':' in str(relative):
                raise ValueError('Unsafe captured asset path')
            path = source_root.joinpath(*relative.parts[1:])
            if not path.resolve(strict=True).is_relative_to(source_root.resolve(strict=True)):
                raise ValueError('Manifest escapes capture directory')
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
    records = sorted(written.values(), key=lambda r: (r['product_id'], r['edition_id'], r['layout_id']))
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
