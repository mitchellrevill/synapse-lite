// Local source196 Rotation SVG. No screenshots, vendor scripts or network.
const fs=require('node:fs');const path=require('node:path');const crypto=require('node:crypto');
async function main(){
 const [media,output,sharpModule]=process.argv.slice(2);if(!media||!output||!sharpModule)throw Error('Expected <196-media> <native-assets> <sharp-module>');
 const sharp=require(path.resolve(sharpModule));const hash=b=>crypto.createHash('sha256').update(b).digest('hex');
 const source=fs.readFileSync(path.join(media,'icon_rotation_mouse.6099cba7.svg'));
 if(hash(source)!=='b1ee704bd4c09e21fb5ec58283bd8fd3a6ca0256cf116890512c36297dd93d52')throw Error('Rotation SVG source hash mismatch');
 const {data,info}=await sharp(source,{density:288}).resize(280,432,{fit:'fill'}).ensureAlpha().raw().toBuffer({resolveWithObject:true});
 const header=Buffer.alloc(16);header.write('SLBGRA1\0');header.writeUInt32LE(info.width,8);header.writeUInt32LE(info.height,12);
 const pixels=Buffer.alloc(data.length);for(let n=0;n<data.length;n+=4){const a=data[n+3];pixels[n]=Math.round(data[n+2]*a/255);pixels[n+1]=Math.round(data[n+1]*a/255);pixels[n+2]=Math.round(data[n]*a/255);pixels[n+3]=a;}
 const bytes=Buffer.concat([header,pixels]);fs.mkdirSync(output,{recursive:true});fs.writeFileSync(path.join(output,'icon-rotation-mouse.bgra'),bytes);
 fs.writeFileSync(path.join(output,'sensor-assets-provenance.json'),JSON.stringify({source_sha256:hash(source),file:'icon-rotation-mouse.bgra',sha256:hash(bytes),width:info.width,height:info.height,network:false},null,2));
 console.log('Provisioned source Rotation mouse image');
}main().catch(e=>{console.error(e);process.exitCode=1;});
