// Replay only the recovered converter with AddEffect stubbed; never load a DLL.
const fs = require('node:fs');
const vm = require('node:vm');
const crypto = require('node:crypto');
const source = fs.readFileSync(process.argv[2], 'utf8');
const target = {};
function load(id) {
  if (id === 2198) return { A: class { constructor() { throw Error('native forbidden'); } } };
  if (id === 5596) return { FU: 0x64000000, JG: 10, lG: 4096, eY: 12288, er: 4 };
  if (id === 7660) return {};
  throw Error('Unexpected module ' + id);
}
load.d = (exports, definitions) => {
  for (const [name, value] of Object.entries(definitions)) Object.defineProperty(exports, name, { get: value });
};
vm.runInNewContext(source, { n: load, t: target, console: { log() {}, warn() {}, error() {} } }, { timeout: 1000 });
const captures = [];
target.A.lightingEngine = { AddEffect: async (_, effect, params) => { captures.push(params); return 2; } };
const cases = [
  { randomColor: true, duration: 6000, cycles: 2, playbackStart: 'random', playbackEnd: 'after' },
  { randomColor: true, cycles: -1, playbackStart: 'onPress', playbackEnd: 'toggle' },
  { cycles: 3, playbackStart: 'onSelectedKeys', playbackEnd: 'never' },
  { randomColor: false, colorStops: null, colorStopsCustom: false, color: 0x112233, color2: 0xaabbcc },
  { color: 0x64000000, color2: 0xaabbcc, playbackEnd: 'after', cycles: 1 },
];
(async () => {
  for (const params of cases) await target.A.addEffect({ engineHandle: 1, effectId: 5, effectParams: params });
  fs.writeFileSync(process.argv[3], JSON.stringify({
    source_sha256: crypto.createHash('sha256').update(source).digest('hex'),
    native_calls: 0,
    cases: cases.map((params, index) => ({ params, descriptor: captures[index] })),
  }, null, 2) + '\n');
})();
