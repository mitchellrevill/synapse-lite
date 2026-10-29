// Replay only recovered pure helpers, never the vendor bundle bootstrap.
// node Replay-MacroLoops.cjs PRIVATE_PROJECTION_DIR OUTPUT_JSON
// The projection manifest and source ranges are documented in MACRO-LOOPS.md.
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');

const [directory, output] = process.argv.slice(2);
if (!directory || !output) throw new Error('Expected projection directory and output JSON');
const read = name => fs.readFileSync(path.join(directory, name + '.js'), 'utf8');
const context = vm.createContext({ console: { log() {} } });
vm.runInContext([
  read('templates'),
  'let b=0; const D=e=>JSON.parse(JSON.stringify(e)); const a={Ay:u},d={gL:6};',
  read('create'), read('update'), read('converter'),
].join('\n'), context, { timeout: 1000 });

const cases = [];
for (const highest of [0, 17, 9007199254740989]) {
  for (const repeat of [1, 2, 99999]) {
    context.highest = highest;
    context.repeat = repeat;
    cases.push(vm.runInContext(`
      b=highest;
      (()=>{
        const events=O('loop');
        C(events,repeat,1);
        return {highest,repeat,events,output:events.flatMap(e=>m({},e))};
      })()
    `, context, { timeout: 1000 }));
  }
}
fs.writeFileSync(output, JSON.stringify({
  provenance: 'Controlled Node VM replay of recovered O/C/m source helpers; no bundle bootstrap, browser, I/O, native conversion or playback',
  cases,
}, null, 2) + '\n');
