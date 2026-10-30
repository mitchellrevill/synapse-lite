"""Project pinned local Studio SVG paths; never execute vendor code or fetch assets."""
import argparse
import base64
import hashlib
import json
from pathlib import Path
import xml.etree.ElementTree as ET

PIN = '3595e11f1d8778180d1dd6edd2463c8b3b8e446542c5020dc1fa038c67937a93'


def project(manifest, output):
    blob = manifest.read_bytes()
    if hashlib.sha256(blob).hexdigest() != PIN:
        raise ValueError('Manifest differs from the researched Ornata artifact')
    document = json.loads(blob.decode('utf-8-sig'))
    if document['productId'] != 605 or document['vendorId'] != 5426:
        raise ValueError('Unexpected device identity')
    records = []
    for name, config in document['ledConfig'].items():
        edition, layout = map(int, name.removeprefix('ledConfig_').split('_'))
        svg_key = f'svg_{edition}_{layout}'
        if svg_key not in document['svg']:
            records.append({'layout': layout, 'unavailable': 'No exact SVG; no fallback inferred'})
            continue
        svg = base64.b64decode(document['svg'][svg_key], validate=True)
        root = ET.fromstring(svg)
        parents = {child: parent for parent in root.iter() for child in parent}
        entries = []
        seen = set()
        for element in root.iter():
            if 'selection' not in element.get('class', '').split():
                continue
            if element.tag.rsplit('}', 1)[-1] != 'path' or element.get('transform'):
                raise ValueError('Untraced selection shape/transform')
            ancestor = parents.get(element)
            while ancestor is not None:
                if ancestor.get('transform'):
                    raise ValueError('Untraced ancestor transform')
                ancestor = parents.get(ancestor)
            row, col = int(element.attrib['data-row']), int(element.attrib['data-col'])
            if (row, col) in seen:
                raise ValueError('Duplicate LED selection identity')
            seen.add((row, col))
            entries.append({'inputID': f'{row}:{col}',
                'buttonKey': element.get('data-selection-group', ''),
                'counter': element.get('data-assumed-key-name', ''),
                'd': element.attrib['d'], 'isEnabled': True})
        mapped = {tuple(item['MatrixPos']) for item in config['LedMatrix']}
        if not seen.issubset(mapped):
            raise ValueError(f'SVG selects an unmapped LED in {name}')
        data = {'format': 'synapse-studio-key-regions-v1', 'product_id': 605,
            'edition_id': edition, 'layout_id': layout,
            'view_box': list(map(float, root.attrib['viewBox'].split())),
            'source_sha256': hashlib.sha256(svg).hexdigest(),
            'manifest_sha256': PIN, 'keys': entries}
        filename = f'studio-605-{edition}-{layout}.json'
        (output / filename).write_text(json.dumps(data, ensure_ascii=False), encoding='utf-8')
        records.append({'file': filename, 'leds': len(entries),
            'matrix_without_selection_path': sorted(mapped - seen),
            'svg_sha256': data['source_sha256']})
    (output / 'studio-layout-provenance.json').write_text(json.dumps(
        {'manifest_sha256': PIN, 'records': records}, indent=2), encoding='utf-8')
    return records


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('manifest', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    if not args.output.is_dir():
        parser.error('Use an existing private asset directory')
    print(json.dumps(project(args.manifest, args.output)))
