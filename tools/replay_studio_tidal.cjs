const fs=require('fs'),vm=require('vm'),path=require('path'),crypto=require('crypto');
const root=process.argv[2];if(!root)throw Error('Pass cached apps.razer.com/synapse directory');
const studio=fs.readFileSync(path.join(root,'chroma-studio/static/js/main.88ac681e.js'),'utf8');
const engine=fs.readFileSync(path.join(root,'lighting-engine/static/js/main.a383ea9e.js'),'utf8');
function body(source,id){const re=new RegExp('\\b'+id+':\\([^)]*\\)=>\\{');const m=re.exec(source);if(!m)throw Error('module '+id);let pos=m.index+String(id).length+1,start=source.indexOf('{',pos),depth=0,quote=null,escape=false;for(let i=start;i<source.length;i++){const c=source[i];if(escape){escape=false;continue;}if(c==='\\'){escape=true;continue;}if(quote){if(c===quote)quote=null;continue;}if('"\'`'.includes(c)){quote=c;continue;}if(c==='{')depth++;if(c==='}'&&!--depth)return source.slice(pos,i+1);}throw Error('unbalanced');}
function loader(source,stubs={}){const cache={};function load(id){if(stubs[id])return stubs[id];if(cache[id])return cache[id].exports;const m={exports:{}};cache[id]=m;vm.runInNewContext('('+body(source,id)+')',{console:{log(){},warn(){},error(){}},URL},{timeout:1000})(m,m.exports,load);return m.exports;}load.d=(target,defs)=>{for(const [k,get]of Object.entries(defs))Object.defineProperty(target,k,{get});};load.n=m=>()=>m;load.r=t=>Object.defineProperty(t,'__esModule',{value:true});return load;}
// Non-Tidal preset conversion and inactive canvas dependencies are inert.
const requireStudio=loader(studio,{6524:{A:JSON.parse},9220:{A:()=>[]},9170:{},597:{},1415:{},7760:{e:{}}});
const settings=requireStudio(2904),constants=requireStudio(6257);
const requireEngine=loader(engine,{2198:{A:class{constructor(){throw Error('vendor initialization forbidden')}}},7660:{Gl:JSON.parse}});
const converter=requireEngine(2578).A,captures=[];converter.lightingEngine={AddEffect:async(e,id,params)=>{captures.push({effect:id,params:JSON.parse(JSON.stringify(params))});return 1;}};
(async()=>{
 const cases=[{}, {color:65280,color2:255,speed:10,angle:160,split:true,randomColor:false,cycles:-1,playbackStart:'random',playbackEnd:'never',centerPointX:125,centerPointY:123,counterclockwise:false},
 {color:0x112233,color2:0xabcdef,randomColor:true,split:true,counterclockwise:true,speed:50,cycles:7,playbackStart:'onSelectedKeys',playbackEnd:'toggle'},
 {color2:0x123456}, {color:0x64000000,color2:0,split:false,speed:0},
 {randomColor:true,colorStops:[]}, {randomColor:true,colorStops:[{Stop:80,Color:1,future:9},{Stop:20}],width:400,pause:60000,angle:360,cycles:100,playbackStart:'onPress',playbackEnd:'after'},
 {colorStops:[{Stop:0},{Stop:50,Color:0},{Stop:50,Color:0xabcdef},{Stop:100,Color:0x64000000}],width:10,pause:1,centerPointX:0,centerPointY:-2},
 ];
 for(const input of cases)await converter.addEffect({engineHandle:1,effectId:19,effectParams:input,appName:'isolated-tidal-converter-replay'});
 const result={ui_defaults:settings.rH('tidal',true),ui_layout:settings.pR('tidal'),ui_speed_range:[constants.y8,constants.Sl],studio_sha256:crypto.createHash('sha256').update(studio).digest('hex'),scope:'Actual cached JS converter, stubbed AddEffect; no DLL, application, network, account, profile or hardware access',source_sha256:crypto.createHash('sha256').update(engine).digest('hex'),cases:cases.map((input,i)=>({input,output:captures[i]}))};
 fs.writeFileSync('crates/secbird-core/tests/fixtures/studio-tidal-converter.json',JSON.stringify(result,null,2)+'\n');
 console.log(JSON.stringify({cases:cases.length,source_sha256:result.source_sha256}));
})().catch(e=>{console.error(e);process.exitCode=1;});
