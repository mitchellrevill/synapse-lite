const fs=require('node:fs'),path=require('node:path'),crypto=require('node:crypto');
async function main(){
 const [media,output,modulePath]=process.argv.slice(2);if(!modulePath)throw Error('Expected stock media, output, and sharp module');
 const sharp=require(path.resolve(modulePath));
 const roles={forward:'icon_arrow_right_thin.',cursor:'cursor-white.',pen:'pen-white.',bucket:'bucket-white.',move:'move-white.',trash:'trash-white.',undo:'undo-white.',redo:'redo-white.',reset:'icon_reset.', 'folder-add':'folder-add-gray.',preview:'icon_chroma_preview.',keyboard:'keyboard-white.e5c30778',settings:'icon_settings-2.'};
 const records=[];
 for(const [role,prefix]of Object.entries(roles)){
  const matches=fs.readdirSync(media).filter(name=>name.startsWith(prefix)&&name.endsWith('.svg'));if(matches.length!==1)throw Error(`Ambiguous ${role}`);
  const source=fs.readFileSync(path.join(media,matches[0]));
  for(const[tint,rgb]of [['normal',[204,204,204]],['muted',[85,85,85]],['active',[68,214,44]]]){
   const{data,info}=await sharp(source,{density:216}).resize(60,60,{fit:'contain'}).ensureAlpha().raw().toBuffer({resolveWithObject:true});
   for(let i=0;i<data.length;i+=4){const a=data[i+3]/255;data[i]=Math.round(rgb[2]*a);data[i+1]=Math.round(rgb[1]*a);data[i+2]=Math.round(rgb[0]*a);}
   const header=Buffer.alloc(16);header.write('SLBGRA1\0');header.writeUInt32LE(info.width,8);header.writeUInt32LE(info.height,12);
   const bytes=Buffer.concat([header,data]),file=`icon-studio-${role}-${tint}.bgra`;fs.writeFileSync(path.join(output,file),bytes);
   records.push({file,source:matches[0],source_sha256:crypto.createHash('sha256').update(source).digest('hex'),sha256:crypto.createHash('sha256').update(bytes).digest('hex')});
  }
 }
 fs.writeFileSync(path.join(output,'studio-controls-provenance.json'),JSON.stringify(records,null,2));console.log(`Provisioned ${records.length} stock Studio control masks`);
}main().catch(error=>{console.error(error);process.exitCode=1;});
