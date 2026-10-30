"""Recover body and legend layers from a local, source-matched Studio SVG."""
import argparse
import base64
import copy
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import xml.etree.ElementTree as ET
ET.register_namespace('', 'http://www.w3.org/2000/svg')


def project(manifest, layout):
    """Keep visual paths separate from the existing editable LED geometry."""
    if hashlib.sha256(manifest).hexdigest() != layout['manifest_sha256']:
        raise ValueError('Manifest differs from the admitted layout')
    document = json.loads(manifest.decode('utf-8-sig'))
    key = f"svg_{layout['edition_id']}_{layout['layout_id']}"
    svg = base64.b64decode(document['svg'][key], validate=True)
    if hashlib.sha256(svg).hexdigest() != layout['source_sha256']:
        raise ValueError('Artwork differs from the admitted LED geometry')
    if b'<!ENTITY' in svg or b'<!DOCTYPE' in svg:
        raise ValueError('External XML declarations are unsupported')
    root = ET.fromstring(svg)
    if list(map(float, root.attrib['viewBox'].split())) != layout['view_box']:
        raise ValueError('Artwork viewport differs from the LED geometry')
    for element in root.iter():
        if element.tag.rsplit('}', 1)[-1] not in ('svg', 'g', 'path', 'style'):
            raise ValueError('Untraced artwork element')
        if any('href' in name.lower() for name in element.attrib) or 'url(' in (element.text or '').lower() or '@import' in (element.text or '').lower():
            raise ValueError('Artwork must not reference external resources')
    result = {}
    for role in ('body', 'legends'):
        layer = copy.deepcopy(root)
        count = 0
        for parent in layer.iter():
            for element in list(parent):
                if element.tag.rsplit('}', 1)[-1] != 'path':
                    continue
                classes = set(element.get('class', '').split())
                keep = bool(classes & {'characters'}) if role == 'legends' else bool(classes & {'productfill', 'productoutline1', 'productoutline2'})
                if keep:
                    count += 1
                else:
                    parent.remove(element)
        if not count:
            raise ValueError(f'No traced {role} paths')
        result[role] = ET.tostring(layer, encoding='utf-8')
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('manifest', type=Path)
    parser.add_argument('layout', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('sharp_module', type=Path)
    args = parser.parse_args()
    layout = json.loads(args.layout.read_bytes())
    layers = project(args.manifest.read_bytes(), layout)
    args.output.mkdir(parents=True, exist_ok=True)
    render = r'''
const fs = require('node:fs');
const sharp = require(process.argv[1]);
(async () => {
  const {data, info} = await sharp(process.argv[2], {density: 144}).ensureAlpha().raw().toBuffer({resolveWithObject:true});
  const header=Buffer.alloc(16);header.write('SLBGRA1\0');header.writeUInt32LE(info.width,8);header.writeUInt32LE(info.height,12);
  for(let i=0;i<data.length;i+=4){const r=data[i],g=data[i+1],b=data[i+2],a=data[i+3]/255;data[i]=Math.round(b*a);data[i+1]=Math.round(g*a);data[i+2]=Math.round(r*a);}
  fs.writeFileSync(process.argv[3],Buffer.concat([header,data]));
})().catch(error=>{console.error(error);process.exitCode=1;});
'''
    with tempfile.TemporaryDirectory(prefix='studio-artwork-') as temporary:
        for role, svg in layers.items():
            source = Path(temporary) / f'{role}.svg'
            source.write_bytes(svg)
            destination = args.output / f"{layout['source_sha256']}-{role}.bgra"
            subprocess.run(['node', '-e', render, str(args.sharp_module.resolve()), str(source), str(destination)], check=True)
    print(json.dumps({'source_sha256': layout['source_sha256'], 'layers': list(layers)}))


if __name__ == '__main__':
    main()
