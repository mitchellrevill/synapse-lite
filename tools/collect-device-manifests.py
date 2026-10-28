"""Collect inert public device metadata; never install or execute vendor packages.

The default is an offline plan. --fetch explicitly enables fixed-origin HTTPS GETs.
Use a private output directory: vendor assets are not repository source files.
"""
import argparse
import concurrent.futures
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import threading
import time
import urllib.error
import urllib.request

ORIGIN = 'https://apps.razer.com'
CATALOG_LIMIT = 2 * 1024 * 1024
RESOURCE_LIMIT = 8 * 1024 * 1024
RUN_LIMIT = 256 * 1024 * 1024


def read_json(path, limit):
    with path.open('rb') as source:
        data = source.read(limit + 1)
    if len(data) > limit:
        raise ValueError(f'{path}: size budget exceeded')
    return json.loads(data.decode('utf-8-sig'))


def valid_id(value):
    return type(value) is int and 0 <= value <= 65535


def plan(catalog, output, stage):
    if not isinstance(catalog, list) or len(catalog) > 4096:
        raise ValueError('Expected at most 4096 catalogue records')
    products = set()
    for entry in catalog:
        if not isinstance(entry, dict) or not valid_id(entry.get('productId')):
            raise ValueError('Invalid catalogue product ID')
        products.add(entry['productId'])
    requests, todos = [], []
    for pid in sorted(products):
        if stage == 'products':
            for role in ('ui', 'mw'):
                path = f'/synapse/products/{pid}/{role}/manifest.json'
                requests.append({'product_id': pid, 'role': role, 'path': path})
            continue
        mw = output / f'synapse/products/{pid}/mw/manifest.json'
        try:
            manifest = read_json(mw, RESOURCE_LIMIT)
            editions = manifest.get('editionId')
            if (not isinstance(editions, list) or not editions or len(editions) > 64
                    or not all(valid_id(value) for value in editions)):
                raise ValueError('No usable explicit editionId array; no editions guessed')
        except (OSError, ValueError, AttributeError) as error:
            todos.append({'product_id': pid, 'todo': str(error)})
            continue
        for edition in sorted(set(editions)):
            if stage == 'dashboard':
                # Named stock category-default artwork role, not a claim about
                # a physical device's actual edition/layout or every layout pair.
                icons = manifest.get('icons', [])
                keyboard = any(isinstance(icon, dict) and
                    icon.get('src') == '/synapse/assets/imgs/favicon/KEYBOARD.svg'
                    for icon in icons) if isinstance(icons, list) else False
                layout = 1 if keyboard else 0
                stem = f'/synapse/products/{pid}/ui/{pid}_{edition}/PluginImages/{pid}_{edition}_{layout}_dashboard'
                requests.append({'product_id': pid, 'edition_id': edition,
                    'layout_id': layout, 'role': 'dashboard',
                    'selection': 'category-default artwork only; physical layout unverified',
                    'path': stem + '3x.avif', 'fallback': stem + '1x.avif'})
                continue
            filename = f'DeviceManifest_{pid}_{edition}.json'
            requests.append({'product_id': pid, 'edition_id': edition,
                'role': 'lighting',
                'path': f'/synapse/products/{pid}/mw/lighting-manifests/{filename}',
                'fallback': f'/synapse/lighting-engine/manifest/{filename}'})
    return requests, todos


class SameOriginRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, request, response, code, message, headers, new_url):
        if not new_url.startswith(ORIGIN + '/'):
            raise ValueError('Refused redirect outside the public asset origin')
        return super().redirect_request(request, response, code, message, headers, new_url)


def decode_resource(record, data):
    if record.get('role') == 'dashboard':
        if data[4:8] != b'ftyp' or not any(brand in data[8:64] for brand in (b'avif', b'avis')):
            raise ValueError('Response does not identify an AVIF image')
        return {}
    document = json.loads(data.decode('utf-8-sig'))
    if not isinstance(document, dict):
        raise ValueError('Manifest is not a JSON object')
    return document


class Collector:
    def __init__(self, output, resume=False):
        self.output = output
        self.resume = resume
        self.total = 0
        self.lock = threading.Lock()
        self.stopped = threading.Event()

    def collect(self, record):
        result = dict(record)
        result['checked_utc'] = datetime.now(timezone.utc).isoformat()
        paths = [record['path']] + ([record['fallback']] if 'fallback' in record else [])
        if self.resume:
            for path in paths:
                captured = self.output / path.lstrip('/')
                if captured.is_file():
                    try:
                        with captured.open('rb') as source:
                            data = source.read(RESOURCE_LIMIT + 1)
                        if len(data) > RESOURCE_LIMIT:
                            raise ValueError('Captured resource exceeds size budget')
                        decode_resource(record, data)
                        return dict(result, status='available', fetched_path=path,
                            source='previous_snapshot_capture_not_refreshed', bytes=len(data),
                            sha256=hashlib.sha256(data).hexdigest())
                    except (OSError, ValueError) as error:
                        return dict(result, status='todo', todo=str(error))
        if self.stopped.is_set():
            return dict(result, status='todo', todo='Batch stopped on server rate limit or run budget')
        for path in paths:
            try:
                # Only paths constructed from validated numeric catalogue IDs enter here.
                if not path.startswith('/synapse/') or '..' in path or '?' in path:
                    raise ValueError('Invalid asset path')
                request = urllib.request.Request(ORIGIN + path,
                    headers={'User-Agent': 'SynapseLite-MetadataCollector/1'})
                opener = urllib.request.build_opener(SameOriginRedirect())
                with opener.open(request, timeout=12) as response:
                    declared = response.headers.get('Content-Length')
                    if declared and int(declared) > RESOURCE_LIMIT:
                        raise ValueError('Declared resource size exceeds budget')
                    data = response.read(RESOURCE_LIMIT + 1)
                    if len(data) > RESOURCE_LIMIT:
                        raise ValueError('Resource size exceeds budget')
                    document = decode_resource(record, data)
                    with self.lock:
                        if self.total + len(data) > RUN_LIMIT:
                            self.stopped.set()
                            raise ValueError('Run download budget exhausted; continue another batch')
                        self.total += len(data)
                    destination = self.output / path.lstrip('/')
                    destination.parent.mkdir(parents=True, exist_ok=True)
                    # Do not silently overwrite a captured revision.
                    if destination.exists() and destination.read_bytes() != data:
                        raise ValueError('Captured file differs; use a new snapshot directory')
                    destination.write_bytes(data)
                    result.update(status='available', fetched_path=path, bytes=len(data),
                        source='public_https', retrieved_utc=datetime.now(timezone.utc).isoformat(),
                        sha256=hashlib.sha256(data).hexdigest(),
                        etag=response.headers.get('ETag'),
                        last_modified=response.headers.get('Last-Modified'),
                        version=document.get('version'),
                        editions=document.get('editionId'), layouts=document.get('layoutId'),
                        device_name=document.get('deviceName'))
                    return result
            except urllib.error.HTTPError as error:
                if error.code in (429, 503):
                    self.stopped.set()
                if error.code == 404:
                    result['status'] = 'missing'
                    result['todo'] = 'Public route returned HTTP 404'
                    continue
                result.update(status='todo', todo=f'HTTP {error.code}; no retry loop')
                return result
            except (OSError, ValueError, urllib.error.URLError) as error:
                result.update(status='todo', todo=str(error))
                return result
        return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('catalog', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--stage', choices=['products', 'lighting', 'dashboard'], default='products')
    parser.add_argument('--fetch', action='store_true')
    parser.add_argument('--resume', action='store_true', help='Reuse validated captures without refreshing them')
    args = parser.parse_args()
    requests, todos = plan(read_json(args.catalog, CATALOG_LIMIT), args.output, args.stage)
    args.output.mkdir(parents=True, exist_ok=True)
    if args.fetch:
        collector = Collector(args.output, args.resume)
        results = []
        with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
            for result in pool.map(collector.collect, requests):
                results.append(result)
                if len(results) % 40 == 0:
                    print(f'{args.stage}: {len(results)}/{len(requests)} records checked', flush=True)
    else:
        results = [dict(record, status='planned') for record in requests]
    report = {'format': 'synapse-lite-device-acquisition-v1', 'stage': args.stage,
        'catalog_sha256': hashlib.sha256(args.catalog.read_bytes()).hexdigest(),
        'fetch_enabled': args.fetch, 'records': results, 'todos': todos,
        'hardware_support_verified': False}
    target = args.output / f'{args.stage}-{"fetched" if args.fetch else "plan"}.json'
    if target.exists():
        # Preserve earlier acquisition dates, headers and failures when resuming.
        history = args.output / f'{target.stem}-{time.time_ns()}.json'
        history.write_bytes(target.read_bytes())
    target.write_text(json.dumps(report, indent=2, ensure_ascii=False), encoding='utf-8')
    counts = {status: sum(record['status'] == status for record in results)
              for status in sorted({record['status'] for record in results})}
    print(json.dumps({'report': str(target), 'counts': counts, 'planning_todos': len(todos)}))


if __name__ == '__main__':
    main()
