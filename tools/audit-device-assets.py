"""Audit explicitly supplied private packs; optionally make a deterministic ZIP.

No network, account databases, discovery outside supplied directories, or runtime
installation. Counts describe these exact inputs, never the current retail market.
"""
import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import zipfile

MAX_FILE = 8 * 1024 * 1024
MAX_PACK = 256 * 1024 * 1024


def sha(data):
    return hashlib.sha256(data).hexdigest()


def read(path, limit=MAX_FILE):
    with path.open('rb') as stream:
        data = stream.read(limit + 1)
    if len(data) > limit:
        raise ValueError('File exceeds audit budget')
    return data


def encode(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True, indent=2).encode('utf-8')


def local(root, relative):
    p = PurePosixPath(relative)
    if p.is_absolute() or '..' in p.parts or '\\' in relative or ':' in relative:
        raise ValueError('Unsafe pack path')
    result = root.joinpath(*p.parts).resolve(strict=True)
    if not result.is_relative_to(root.resolve(strict=True)):
        raise ValueError('Pack path escapes root')
    return result


def identity(row):
    result = tuple(row[k] for k in ('product_id', 'edition_id', 'layout_id'))
    if not all(type(v) is int and 0 <= v <= 65535 for v in result):
        raise ValueError('Invalid pack identity')
    return result


def audit(catalog, capture, artwork, studio):
    catalogue = read(catalog, 2*1024*1024)
    entries = json.loads(catalogue)
    if not isinstance(entries, list) or len(entries) > 4096:
        raise ValueError('Invalid catalogue')
    products = set()
    for row in entries:
        pid = row.get('productId')
        if type(pid) is not int or not 0 <= pid <= 65535:
            raise ValueError('Invalid catalogue product identity')
        products.add(pid)
    files, provenance, results = {}, {}, {}
    total = 0
    originals = set()
    identities_without_original = 0

    def include(name, path, limit=MAX_FILE):
        nonlocal total
        data = read(path, limit)
        if name not in files:
            total += len(data)
        if total > MAX_PACK:
            raise ValueError('Pack exceeds audit budget')
        files[name] = (path, len(data), sha(data))
        return data

    for role, root, key, format_name in (
        ('artwork', artwork, 'images', 'synapse-lite-dashboard-pack-v2'),
        ('studio', studio, 'records', 'synapse-lite-studio-presets-v1'),
    ):
        prefix = 'device-pack' if role == 'artwork' else 'studio-presets'
        index_bytes = include(prefix+'/index.json', root/'index.json', 1024*1024 if role == 'artwork' else 2*1024*1024)
        index = json.loads(index_bytes)
        if index.get('format') != format_name or len(index.get(key, [])) > 4096:
            raise ValueError('Unsupported or excessive pack index')
        seen, by_product = set(), {}
        for row in index[key]:
            ids = identity(row)
            selected = ids[:2] if role == 'artwork' else ids
            if selected in seen or (role == 'artwork' and ids[2] not in (0,1)):
                raise ValueError('Duplicate or ambiguous pack identity')
            seen.add(selected)
            filename = row['file']
            pattern = r'dashboard-[a-f0-9]{64}\.png' if role == 'artwork' else rf'studio-{ids[0]}-{ids[1]}-{ids[2]}\.json'
            if not re.fullmatch(pattern, filename):
                raise ValueError('Pack filename differs from its identity')
            data = include(prefix+'/'+filename, local(root, filename), 2*1024*1024)
            if len(data) != row['bytes'] or sha(data) != row['sha256']:
                raise ValueError('Pack file size/hash mismatch')
            if role == 'studio':
                document = json.loads(data)
                if identity(document) != ids or len(document.get('keys', [])) != row['leds']:
                    raise ValueError('Studio file identity/count differs from index')
            else:
                from PIL import Image
                import io
                with Image.open(io.BytesIO(data)) as image:
                    if image.format != 'PNG' or image.size != (row['width'], row['height']) or image.width*image.height > 16_000_000:
                        raise ValueError('Artwork dimensions/format mismatch')
                    image.load()
                    if image.mode != 'RGBA':
                        raise ValueError('Artwork must preserve alpha channel')
                if 'source_file' in row:
                    source_name = row['source_file']
                    if source_name != f"source-{row['source_sha256']}.avif":
                        raise ValueError('Invalid original artwork filename')
                    original = include(prefix+'/'+source_name, local(root,source_name))
                    if len(original) != row['source_bytes'] or sha(original) != row['source_sha256']:
                        raise ValueError('Original artwork size/hash mismatch')
                    with Image.open(io.BytesIO(original)) as image:
                        if image.format != 'AVIF' or list(image.size) != row['source_size'] or image.width*image.height>16_000_000:
                            raise ValueError('Original artwork dimensions/format mismatch')
                        image.load()
                    originals.add(source_name)
                else:
                    identities_without_original += 1
            by_product.setdefault(ids[0], []).append(list(ids[1:]))
        results[role] = by_product
        provenance[role+'_index_sha256'] = sha(index_bytes)
    art_catalogue = include('device-pack/catalog.json', artwork/'catalog.json', 2*1024*1024)
    if art_catalogue != catalogue:
        raise ValueError('Artwork catalogue differs from selected catalogue')

    # Reconcile declarations from captured product manifests, preserving edition
    # and regional lists separately. Their cross product is NOT physical evidence.
    report_bytes = read(capture/'products-fetched.json')
    report = json.loads(report_bytes)
    if report.get('stage') != 'products' or report.get('catalog_sha256') != sha(catalogue):
        raise ValueError('Product capture catalogue/stage differs')
    provenance['products_report_sha256'] = sha(report_bytes)
    declarations = {}
    for row in report['records']:
        if row.get('status') != 'available':
            continue
        pid, role = row['product_id'], row['role']
        expected = f'/synapse/products/{pid}/{role}/manifest.json'
        if pid not in products or role not in ('ui','mw') or row['fetched_path'] != expected:
            raise ValueError('Unexpected product manifest identity/path')
        data = read(local(capture, expected[1:]))
        if len(data) != row['bytes'] or sha(data) != row['sha256']:
            raise ValueError('Product manifest failed size/hash verification')
        doc = json.loads(data.decode('utf-8-sig'))
        def ids(field):
            values = doc.get(field, [])
            if not isinstance(values, list) or len(values)>256 or not all(type(v) is int and 0<=v<=65535 for v in values):
                raise ValueError('Invalid declared edition/layout list')
            return sorted(set(values))
        translated = []
        for name in doc.get('productTranslations', {}):
            match = re.fullmatch(rf'{pid}_(\d+)', name)
            if match and int(match[1]) <= 65535:
                translated.append(int(match[1]))
        key = (pid,role)
        declaration = {'manifest_sha256':sha(data), 'name':doc.get('deviceName'),
            'edition_ids':ids('editionId'), 'layout_ids':ids('layoutId'),
            'translated_edition_ids':sorted(set(translated))}
        if key in declarations and declarations[key] != declaration:
            raise ValueError('Conflicting product manifest declarations')
        declarations[key] = declaration
    records = []
    for pid in sorted(products):
        mw, ui = declarations.get((pid,'mw')), declarations.get((pid,'ui'))
        images, layouts = sorted(results['artwork'].get(pid,[])), sorted(results['studio'].get(pid,[]))
        declared = set(mw['edition_ids']) if mw else set()
        records.append({'product_id':pid, 'mw':mw, 'ui':ui,
            'artwork_category_default_identities':images, 'studio_exact_identities':layouts,
            'mw_editions_without_artwork':sorted(declared-{pair[0] for pair in images}),
            'mw_editions_without_studio':sorted(declared-{pair[0] for pair in layouts}),
            'unresolved':(['No captured MW manifest'] if not mw else [])+
                ['DeviceInfo runtime edition/layout matching, chassis/legends, regional artwork and device APIs not verified']})
    unknown = sorted((set(results['artwork'])|set(results['studio']))-products)
    if unknown:
        raise ValueError('Pack contains products outside selected catalogue')
    inventory = [{'path':name,'bytes':size,'sha256':digest} for name,(_,size,digest) in sorted(files.items())]
    return {'format':'synapse-lite-asset-audit-v1','catalog_sha256':sha(catalogue),
        'provenance':provenance,'scope':'exact selected catalogue and private packs; no all-market or API compatibility claim',
        'redistribution_basis_established':False,'hardware_support_verified':False,
        'summary':{'catalogue_primary_products':len(products),
            'artwork_identities':sum(map(len,results['artwork'].values())),
            'artwork_products':len(results['artwork']),
            'studio_identities':sum(map(len,results['studio'].values())),
            'studio_products':len(results['studio']), 'unique_payload_files':len(files), 'payload_bytes':total,
            'original_artwork_files':len(originals),'artwork_identities_without_original':identities_without_original},
        'products':records,'files':inventory}, files


def package(destination, report, files):
    """Fixed metadata/order and stored bytes make ZIP independent of mtimes/zlib."""
    # Exclusive creation preserves existing archives. Revalidate immediately before
    # writing; only explicitly indexed files enter the private package.
    with destination.open('xb') as output:
        try:
            with zipfile.ZipFile(output, 'w', compression=zipfile.ZIP_STORED) as archive:
                for name in sorted(set(files)|{'audit.json'}):
                    if name == 'audit.json':
                        data = encode(report)
                    else:
                        path, size, digest = files[name]
                        data = read(path)
                        if len(data) != size or sha(data) != digest:
                            raise ValueError('Pack changed after audit')
                    info = zipfile.ZipInfo(name, date_time=(1980,1,1,0,0,0))
                    info.create_system = 3
                    info.external_attr = 0o100600 << 16
                    archive.writestr(info, data)
        except Exception:
            output.close()
            destination.unlink()
            raise


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('catalog','capture','artwork','studio','report'):
        parser.add_argument(name, type=Path)
    parser.add_argument('--private-zip', type=Path)
    args = parser.parse_args()
    result, files = audit(args.catalog,args.capture,args.artwork,args.studio)
    if args.private_zip:
        package(args.private_zip,result,files)
    with args.report.open('xb') as target:
        target.write(encode(result))
    print(json.dumps(result['summary']))
