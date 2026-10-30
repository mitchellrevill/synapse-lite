// Local recovered Studio masks; no network or vendor code execution.
// Usage: node provision-studio-chrome.cjs <stock-media> <output> <sharp-module>
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
async function main() {
  const [input, output, modulePath] = process.argv.slice(2);
  if (!input || !output || !modulePath) throw new Error('Expected media, output and sharp paths');
  const sharp = require(path.resolve(modulePath));
  const roles = ['ambient', 'audio', 'breathing', 'chroma-generate', 'fire', 'reactive',
    'ripple', 'spectrum', 'starlight', 'static', 'tidal', 'wave', 'wheel', 'folder', 'eye-disabled'];
  const records = [];
  fs.mkdirSync(output, { recursive: true });
  for (const role of roles) {
    const matches = fs.readdirSync(input).filter(file => file.startsWith(`${role}-white.`) && file.endsWith('.svg'));
    if (matches.length !== 1) throw new Error(`Ambiguous or missing source for ${role}`);
    const source = fs.readFileSync(path.join(input, matches[0]));
    for (const [state, channel] of [['muted', 0x70], ['normal', 0xcc], ['active', null]]) {
      const { data, info } = await sharp(source, { density: 216 }).resize(90, 90, { fit: 'contain' }).ensureAlpha().raw().toBuffer({ resolveWithObject: true });
      const header = Buffer.alloc(16); header.write('SLBGRA1\0', 0, 'ascii');
      header.writeUInt32LE(info.width, 8); header.writeUInt32LE(info.height, 12);
      for (let i = 0; i < data.length; i += 4) {
        const a = data[i + 3] / 255;
        data[i] = Math.round((channel ?? 0x2c) * a);
        data[i + 1] = Math.round((channel ?? 0xd6) * a);
        data[i + 2] = Math.round((channel ?? 0x44) * a);
      }
      const bytes = Buffer.concat([header, data]);
      const file = `icon-studio-${role}-${state}.bgra`;
      fs.writeFileSync(path.join(output, file), bytes);
      records.push({ file, source: matches[0], source_sha256: crypto.createHash('sha256').update(source).digest('hex'),
        sha256: crypto.createHash('sha256').update(bytes).digest('hex'), width: info.width, height: info.height });
    }
  }
  // Visible eye uses the gray source mask, with the same normal tint.
  const eye = fs.readdirSync(input).find(file => /^eye-gray\..*\.svg$/.test(file));
  if (!eye) throw new Error('Missing eye mask');
  const { data, info } = await sharp(fs.readFileSync(path.join(input, eye)), { density: 216 }).resize(60, 60, { fit: 'contain' }).ensureAlpha().raw().toBuffer({ resolveWithObject: true });
  const header = Buffer.alloc(16); header.write('SLBGRA1\0'); header.writeUInt32LE(info.width,8); header.writeUInt32LE(info.height,12);
  for (let i=0;i<data.length;i+=4) { const c=Math.round(0xcc*data[i+3]/255);data[i]=c;data[i+1]=c;data[i+2]=c; }
  const bytes=Buffer.concat([header,data]); const file='icon-studio-eye-normal.bgra';fs.writeFileSync(path.join(output,file),bytes);
  records.push({file,source:eye,source_sha256:crypto.createHash('sha256').update(fs.readFileSync(path.join(input,eye))).digest('hex'),sha256:crypto.createHash('sha256').update(bytes).digest('hex'),width:info.width,height:info.height});
  fs.writeFileSync(path.join(output,'studio-chrome-provenance.json'),JSON.stringify(records,null,2));
  process.stdout.write(`Provisioned ${records.length} Studio masks\n`);
}
main().catch(error=>{process.stderr.write(`${error.stack}\n`);process.exitCode=1;});
