// Controlled replay of the private shipped converter. No DLL, device or network.
// node tools/recover-starlight-descriptors.cjs MODULE_2578 OUTPUT_JSON
const fs = require('node:fs');
const crypto = require('node:crypto');
const vm = require('node:vm');
const [modulePath, outputPath] = process.argv.slice(2);
if (!modulePath || !outputPath) throw Error('supply source module2578 and new output path');
const source = fs.readFileSync(modulePath);
const sourceSha = crypto.createHash('sha256').update(source).digest('hex');
if (sourceSha !== '008bf7b8aac4e86f47dd0c4e498402949c384d640553ca08d9c2424ad3527bda') {
  throw Error('converter source identity mismatch');
}
const target = {};
function load(id) {
  if (id === 2198) return { A: class { constructor() { throw Error('native loading forbidden'); } } };
  if (id === 5596) return { FU: 0x64000000, JG: 10, lG: 4096, eY: 12288, er: 4 };
  if (id === 7660) return {};
  throw Error('unresolved source module ' + id);
}
load.d = (obj, defs) => {
  for (const [key, value] of Object.entries(defs)) Object.defineProperty(obj, key, { get: value });
};
vm.runInNewContext(source.toString('utf8'), {
  n: load, t: target, console: { log() {}, warn() {}, error() {} }
}, { timeout: 1000 });
let captured;
target.A.lightingEngine = {
  AddEffect: async (engine, id, params) => {
    if (engine !== 1 || id !== 7) throw Error('unexpected native boundary');
    captured = JSON.parse(JSON.stringify(params));
    return 2;
  }
};
const cases = [];
for (const duration of [3000, 1700, 500]) {
  for (let density = 1; density <= 10; density++) {
    for (const randomColor of [false, true]) cases.push({
      name: `duration${duration}_density${density}_random${randomColor}`,
      params: { duration, density, randomColor, colorStops: [{ Stop: 0, Color: 0x123456 }, { Stop: 100 }] }
    });
  }
}
for (const [name, colorStops] of [
  ['single_none', [{ Stop: 35 }]],
  ['duplicate_stops', [{ Stop: 50, Color: 1 }, { Stop: 50 }, { Stop: 100, Color: 3 }]],
  ['unordered_stops', [{ Stop: 80, Color: 1 }, { Stop: 20, Color: 2 }]],
  ['unordered_anchors', [{ Stop: 100, Color: 1 }, { Stop: 0, Color: 2 }]],
  ['stop_metadata', [{ Stop: 20, Color: 7, future: { keep: [true, 'x'] } }, { Stop: 90 }]],
  ['empty_stops', []],
  ['missing_stops', undefined]
]) cases.push({ name, params: { duration: 9999, density: 4, randomColor: false, colorStops } });
cases.push({ name: 'missing_scalars', params: { colorStops: [{ Stop: 0, Color: 0 }, { Stop: 100 }] } });
cases.push({ name: 'random_missing_scalars', params: { randomColor: true } });
(async () => {
  for (const test of cases) {
    captured = undefined;
    await target.A.addEffect({ engineHandle: 1, effectId: 7, effectParams: test.params });
    if (!captured) throw Error('source failed to reach intercepted boundary');
    test.descriptor = captured;
  }
  fs.writeFileSync(outputPath, JSON.stringify({
    source_module_sha256: sourceSha,
    source_bundle_sha256: 'c82c7074e5da8abcffbff95f23a6b962c1e14b04586715b9d4b7a6f11fe9e6d2',
    native_calls: 0, cases
  }, null, 2) + '\n', { flag: 'wx' });
  console.log(`Replayed ${cases.length} Starlight descriptors; zero native calls.`);
})().catch(error => { console.error(error); process.exitCode = 1; });
