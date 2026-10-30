"""Select one newly admitted layout per product for offline SVG comparison."""
import argparse
import base64
import hashlib
import json
from pathlib import Path


def prepare(capture, report, pack, previous, output):
    index = json.loads((pack/'index.json').read_bytes())
    report_bytes = report.read_bytes()
    if hashlib.sha256(report_bytes).hexdigest() != index['capture_report_sha256']:
        raise ValueError('Source report differs from pack')
    records = {r['sha256']:r for r in json.loads(report_bytes)['records'] if r.get('status') == 'available'}
    old = {r['file'] for r in json.loads(previous.read_bytes())['records']}
    selected, seen = [], set()
    for row in index['records']:
        if row['file'] in old or row['product_id'] in seen:
            continue
        selected.append(row); seen.add(row['product_id'])
    if not 1 <= len(selected) <= 32:
        raise ValueError('Select a comparison batch with 1..32 new products')
    output.mkdir(exist_ok=False)
    cases = []
    for row in selected:
        record = records[row['manifest_sha256']]
        relative = record['fetched_path']
        if not relative.startswith('/synapse/') or '..' in relative or '\\' in relative or ':' in relative:
            raise ValueError('Unsafe capture path')
        path = (capture/relative[1:]).resolve(strict=True)
        if not path.is_relative_to(capture.resolve(strict=True)):
            raise ValueError('Capture escapes root')
        with path.open('rb') as stream:
            blob = stream.read(8*1024*1024+1)
        if len(blob)>8*1024*1024 or len(blob)!=record['bytes'] or hashlib.sha256(blob).hexdigest()!=record['sha256']:
            raise ValueError('Source manifest failed size/hash verification')
        doc = json.loads(blob.decode('utf-8-sig'))
        svg = base64.b64decode(doc['svg'][f"svg_{row['edition_id']}_{row['layout_id']}"],validate=True)
        if hashlib.sha256(svg).hexdigest() != row['svg_sha256']:
            raise ValueError('SVG digest differs from projection')
        source = output/f"case-{row['product_id']}.svg"
        source.write_bytes(svg)
        cases.append({'identity':[row[k] for k in ('product_id','edition_id','layout_id')],
            'svg':str(source.resolve()),'projection':str((pack/row['file']).resolve())})
    (output/'cases.json').write_text(json.dumps(cases,indent=2),encoding='utf-8')
    return len(cases)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('capture','report','pack','previous','output'):
        parser.add_argument(name,type=Path)
    args=parser.parse_args()
    print(prepare(args.capture,args.report,args.pack,args.previous,args.output))
