// Authoring only: reproduce documented source Canvas2D recipes and convert
// existing local SVG assets. No vendor JavaScript executes, no network fetches,
// no desktop capture. Generated vendor assets must remain ignored/private.
const fs=require('fs'),path=require('path'),crypto=require('crypto');
const args={};for(let i=2;i<process.argv.length;i+=2)args[process.argv[i].replace(/^--/,'')]=process.argv[i+1];
for(const key of['output','playwright','browser','media','sharp'])if(!args[key])throw Error(`Missing --${key}`);
const output=path.resolve(args.output);fs.mkdirSync(output,{recursive:true});
const hash=b=>crypto.createHash('sha256').update(b).digest('hex');
function bmp(rgb,width,height){const stride=(width*3+3)&~3,b=Buffer.alloc(54+stride*height);b.write('BM');b.writeUInt32LE(b.length,2);b.writeUInt32LE(54,10);b.writeUInt32LE(40,14);b.writeInt32LE(width,18);b.writeInt32LE(height,22);b.writeUInt16LE(1,26);b.writeUInt16LE(24,28);for(let y=0;y<height;y++)for(let x=0;x<width;x++){const src=(y*width+x)*3,dst=54+(height-1-y)*stride+x*3;b[dst]=rgb[src+2];b[dst+1]=rgb[src+1];b[dst+2]=rgb[src];}return b;}
(async()=>{
const browser=await require(args.playwright).chromium.launch({executablePath:args.browser,headless:true,args:['--disable-background-networking','--disable-component-update','--no-first-run']});const page=await browser.newPage();await page.route('**/*',r=>r.abort());
const rendered=await page.evaluate(()=>{
 const encode=a=>{let s='';for(let i=0;i<a.length;i+=32768)s+=String.fromCharCode(...a.subarray(i,i+32768));return btoa(s)};
 const rgb=data=>{const out=new Uint8Array(data.length/4*3);for(let i=0,j=0;i<data.length;i+=4){out[j++]=data[i];out[j++]=data[i+1];out[j++]=data[i+2]}return out};
 const canvas=document.createElement('canvas');canvas.width=198;canvas.height=118;const c=canvas.getContext('2d');const hue=c.createLinearGradient(0,0,198,0);['#ff0000','#ffff00','#00ff00','#00ffff','#0000ff','#ff00ff','#ff0000'].forEach((v,i)=>hue.addColorStop(i/6,v));c.fillStyle=hue;c.fillRect(0,0,198,118);const white=c.createLinearGradient(0,0,0,118);white.addColorStop(0,'rgba(0,0,0,0)');white.addColorStop(1,'rgba(255,255,255,1)');c.fillStyle=white;c.fillRect(0,0,198,118);const plane=encode(rgb(c.getImageData(0,0,198,118).data));
 const bar=document.createElement('canvas');bar.width=130;bar.height=16;const b=bar.getContext('2d');const lut=new Uint8Array(256*130*16*3);for(let value=0;value<256;value++){b.clearRect(0,0,130,16);b.fillStyle=`rgb(${value},${value},${value})`;b.fillRect(0,0,130,16);const black=b.createLinearGradient(0,0,130,0);black.addColorStop(0,'rgba(0,0,0,1)');black.addColorStop(1,'rgba(0,0,0,0)');b.fillStyle=black;b.fillRect(0,0,130,16);lut.set(rgb(b.getImageData(0,0,130,16).data),value*130*16*3)}return{plane,lut:encode(lut)};
});const version=await browser.version();await browser.close();
const records=[];for(const[name,data]of[['lighting-hs-plane.rgb',rendered.plane],['lighting-value.lut',rendered.lut]]){const bytes=Buffer.from(data,'base64');fs.writeFileSync(path.join(output,name),bytes);records.push({file:name,bytes:bytes.length,sha256:hash(bytes),authority:'controlled-source-recipe-replay',browser:version});}
const media=path.resolve(args.media),sharp=require(args.sharp);
const icons=[
 ['lighting-eyedropper','color_picker.b2c4481d',16,'#111111'],['lighting-eyedropper-hover','color_picker_hover.c1c1c043',16,'#111111'],
 ['lighting-plus','plus.a386742c',8,'#111111'],['lighting-no-color','disable_palette.03c60895',18,'#c83200'],
 ['lighting-sync','chroma_sync_v3_static.c8ddf315',26,'#111111'],
 ['lighting-left','icon_direction_left_999.3397283a',20,'#111111'],['lighting-left-active','icon_direction_left.cc325d65',20,'#44d62c'],
 ['lighting-right','icon_direction_right_999.8c251efa',20,'#111111'],['lighting-right-active','icon_direction_right.70dccfc5',20,'#44d62c'],
 ['lighting-cw','icon_direction_cw.075c1730',20,'#111111'],['lighting-cw-active','icon_direction_cw_1.0d8f9301',20,'#44d62c'],
 ['lighting-ccw','icon_direction_ccw.e34b12f6',20,'#111111'],['lighting-ccw-active','icon_direction_ccw_1.3dffaadc',20,'#44d62c'],
 ['lighting-out','icon_direction_outward_999.4cd91b14',20,'#111111'],['lighting-out-active','icon_direction_outward.7c388a12',20,'#44d62c'],
 ['lighting-in','icon_direction_inward_999.29221403',20,'#111111'],['lighting-in-active','icon_direction_inward.29c0ed8a',20,'#44d62c'],
 ];
 for(const[name,stem,size,background]of icons){const file=fs.readdirSync(media).find(n=>n.startsWith(stem+'.svg'));if(!file){records.push({file:'icon-'+name+'.bmp',missing_source:stem});continue}const source=fs.readFileSync(path.join(media,file));const{data,info}=await sharp(source).resize(size,size,{fit:'contain',background:'#00000000'}).flatten({background}).removeAlpha().raw().toBuffer({resolveWithObject:true});const bytes=bmp(data,info.width,info.height),dest='icon-'+name+'.bmp';fs.writeFileSync(path.join(output,dest),bytes);records.push({file:dest,source_file:file,source_sha256:hash(source),sha256:hash(bytes),background,size});}
 fs.writeFileSync(path.join(output,'lighting-asset-provenance.json'),JSON.stringify({source:'Ornata605 main.fcf07de2.js classCr; main.f65da71b.css',vendor_javascript_executed:false,desktop_capture:false,records},null,2));process.stdout.write(`Provisioned ${records.filter(r=>!r.missing_source).length} local Lighting assets\n`);
})().catch(e=>{process.stderr.write(String(e)+'\n');process.exitCode=1});
