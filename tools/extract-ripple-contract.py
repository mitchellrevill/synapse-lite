"""Extract inert shipped Ripple literals; never execute the renderer bundle."""
import argparse
import hashlib
import json
import re
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument('--constants', type=Path, required=True)
parser.add_argument('--defaults', type=Path, required=True)
parser.add_argument('--ripple-chunk', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
constants = args.constants.read_text(encoding='utf-8')
defaults = args.defaults.read_text(encoding='utf-8')
chunk = args.ripple_chunk.read_text(encoding='utf-8')

def literal(name):
    raw = re.search(r'[,;]' + re.escape(name) + r'=("[^"]*"|-?\d+)(?=[,;])', constants)
    if not raw:
        raise ValueError(f'Constant not found: {name}')
    return json.loads(raw[1])

# Bound the extraction to this exact known factory and UI wiring. A different
# producer must be reviewed rather than generating a fixture from assumptions.
for source, required in [
    (defaults, 'case"ripple":o={colorStops:t?i.Mh:void 0,colorStopsCustom:!1,speed:i.d_,width:i.Yq,playbackStart:i.ic,playbackEnd:i.nf,cycles:i.i_}'),
    (chunk, 'min:x.Mn,max:x.s8'),
    (chunk, 'min:x.jw,max:x.Ph,step:x.h1'),
    (chunk, '"random"===n?(s.playbackEnd=m.$e[0],s.cycles=-1)'),
    (chunk, '"toggle"!==s||"random"!==t?'),
    (chunk, 'min:m.YZ,max:m.QG'),
]:
    if required not in source:
        raise ValueError(f'Source contract changed: {required}')
raw = re.search(r'k=(\[\[\{stop:.*?\]\]),j=k\[0\]\.length', constants)[1]
raw = re.sub(r'\b(stop|rgb):', r'"\1":', raw)
raw = re.sub(r'(?<=:)\.(\d+)', r'0.\1', raw)
patterns = json.loads(raw)
presets = [[{'Stop': round(stop['stop'] * 100),
             'Color': (stop['rgb'][0] << 16) | (stop['rgb'][1] << 8) | stop['rgb'][2]}
            for stop in pattern] for pattern in patterns]
fixture = {
    'producer': 'inert literal extraction from Studio module6257 / factory / chunk255',
    'source_sha256': {path.name: hashlib.sha256(path.read_bytes()).hexdigest()
                      for path in [args.constants, args.defaults, args.ripple_chunk]},
    'speed': {'min': literal('q'), 'max': literal('Q'), 'default': literal('ee')},
    'width': {'min': literal('X'), 'max': literal('z'), 'step': literal('J'), 'default': literal('K')},
    'defaults': {'speed': literal('ee'), 'width': literal('K'), 'playbackStart': literal('te'),
                 'playbackEnd': literal('oe'), 'cycles': literal('ne'),
                 'colorStopsCustom': False, 'colorStops': presets[0]},
    'presets': presets,
    'repeat': {'min': literal('G'), 'max': literal('Y')},
}
args.output.write_text(json.dumps(fixture, indent=2) + '\n', encoding='utf-8')
