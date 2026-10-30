// Replays only hash-pinned, recovered factories in an isolated VM. No app bootstrap,
// network, device engine, profile writes or bundled source is included here.
// Usage: node tools/verify-studio-property-defaults.cjs PRIVATE_MAIN_JS
const fs = require('node:fs');
const vm = require('node:vm');
const crypto = require('node:crypto');
const path = require('node:path');
const input = process.argv[2];
if (!input) throw Error('Select the private recovered main.88ac681e.js');
const bytes = fs.readFileSync(input);
if (crypto.createHash('sha256').update(bytes).digest('hex') !== '109b483f129044e1c7c17b9fcae0e30fa775885667976807cf2c94bd1c696d19') throw Error('Source digest differs');
const source = bytes.toString('utf8');
function factory(id) {
  const marker = id + ':(e,t,o)=>';
  const start = source.indexOf(marker);
  if (start < 0) throw Error('Missing source module ' + id);
  const tail = source.slice(start + String(id).length + 1);
  const end = tail.search(/},\d+:(?:\(|e=>)/) + 1;
  if (end < 1) throw Error('Missing factory boundary');
  return vm.runInNewContext('(' + tail.slice(0,end) + ')', {URL}, {timeout:1000});
}
const define = (target, values) => {
  for (const [name,get] of Object.entries(values)) Object.defineProperty(target,name,{get});
};
const constants = {};
const constantsRequire = id => {
  if (id === 6524) return {A:()=>null};
  if (id === 9220) return {A:rows=>rows.map(v=>({Stop:Math.round(v.stop*100),Color:(v.rgb[0]<<16)|(v.rgb[1]<<8)|v.rgb[2]}))};
  throw Error('Untraced dependency ' + id);
};
constantsRequire.d = define;
factory(6257)({}, constants, constantsRequire);
const defaults = {};
const defaultsRequire = id => id === 6257 ? constants : {};
defaultsRequire.d = define;
factory(2904)({}, defaults, defaultsRequire);
const expected = JSON.parse(fs.readFileSync(path.join(__dirname,'../crates/secbird-core/src/studio/property_defaults.json')));
let checked = 0;
for (const [name,values] of Object.entries(expected)) {
  for (const [key,reset] of [['selected',false],['reset',true]]) {
    if (JSON.stringify(defaults.rH(name,reset)) !== JSON.stringify(values[key])) throw Error('Default mismatch: ' + name + '/' + key);
    checked++;
  }
}
const actions = {};
const types = new Proxy({}, {get:(_,key)=>key});
const modal = [];
const engine = {A:{previewEffect:()=>{}}};
const actionsRequire = id => {
  if (id===2904) return defaults;
  if (id===4048) return engine;
  if (id===6092) return types;
  if (id===9046) return {I:value=>()=>modal.push(value)};
  return {};
};
actionsRequire.d=define;
factory(9286)({},actions,actionsRequire);
(async()=>{
  let cases=0;
  for (const count of [0,1,2]) for (const mixed of [false,true]) {
    const dispatch=[];modal.length=0;
    const state={device:{itemsSelected:Array.from({length:count},()=>({leds:[{}]}))},effectLayer:{paramsMixed:mixed?['color']:[]}};
    actions.cF()(value=>dispatch.push(value),()=>state);
    if (dispatch.some(v=>v.type==='v6') !== (count>0&&!mixed)) throw Error('Copy thunk rule differs');
    if (count>0&&mixed && modal[0]?.unableToCopyType!==1) throw Error('Mixed-copy refusal missing');
    cases++;
  }
  for (const [name,values] of Object.entries(expected)) {
    const dispatched=[];
    await actions.g7()(value=>dispatched.push(value),()=>({effectLayer:{current:{name}},editor:{tool:'select'}}));
    if (JSON.stringify(dispatched[0].payload)!==JSON.stringify(values.reset)) throw Error('Reset action differs');
    cases++;
  }
  console.log(JSON.stringify({source_sha256:crypto.createHash('sha256').update(bytes).digest('hex'),default_cases:checked,action_cases:cases,coverage:'controlled factory replay; source UI device-count guard checked separately; no vendor app or hardware execution'}));
})().catch(error=>{console.error(error);process.exitCode=1;});
