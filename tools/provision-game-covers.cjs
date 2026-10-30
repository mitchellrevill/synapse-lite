// Explicit build-time artwork import. The native application stays offline.
// node tools/provision-game-covers.cjs <catalogue-artwork.json> <output> <sharp>
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const https = require('node:https');
const [manifest, output, sharpPath] = process.argv.slice(2);
if (!manifest || !output || !sharpPath) throw new Error('Expected artwork manifest, output and sharp module');
const sharp = require(path.resolve(sharpPath));
const records = JSON.parse(fs.readFileSync(manifest, 'utf8'));
if (!Array.isArray(records) || records.length > 512) throw new Error('Invalid artwork manifest');
const hash = data => crypto.createHash('sha256').update(data).digest('hex');
function download(url) {
  return new Promise((resolve, reject) => {
    const request = https.get(url, { headers: { Accept: 'image/webp' } }, response => {
      if (response.statusCode !== 200) { response.resume(); reject(new Error(`HTTP ${response.statusCode}`)); return; }
      const chunks = []; let length = 0;
      response.on('data', chunk => {
        length += chunk.length;
        if (length > 4 * 1024 * 1024) { response.destroy(new Error('Artwork exceeds 4 MiB')); return; }
        chunks.push(chunk);
      });
      response.on('end', () => resolve(Buffer.concat(chunks)));
      response.on('error', reject);
    });
    request.setTimeout(15000, () => request.destroy(new Error('Artwork request timed out')));
    request.on('error', reject);
  });
}
async function main() {
  fs.mkdirSync(output, { recursive: true });
  const results = [];
  for (const record of records) {
    if (!/^[a-f0-9-]{1,128}$/i.test(record.id)) throw new Error('Invalid catalogue image ID');
    const url = new URL(record.url);
    if (url.protocol !== 'https:' || url.hostname !== 'deals-assets-cdn.razerzone.com' || url.port ||
        url.username || url.password || url.search || url.hash || !/^\/game_covers\/[a-f0-9-]+\.webp$/i.test(url.pathname)) {
      throw new Error('Artwork URL is outside the official static game-cover directory');
    }
    try {
      const file = path.join(output, `${record.id}.bgra`);
      if (fs.existsSync(file)) { results.push({ id: record.id, cached: true }); continue; }
      const source = await download(url);
      const { data, info } = await sharp(source, { limitInputPixels: 16 * 1024 * 1024 })
        .resize(576, 300, { fit: 'cover' }).ensureAlpha().raw().toBuffer({ resolveWithObject: true });
      const header = Buffer.alloc(16); header.write('SLBGRA1\0');
      header.writeUInt32LE(info.width, 8); header.writeUInt32LE(info.height, 12);
      const pixels = Buffer.alloc(data.length);
      for (let index = 0; index < data.length; index += 4) {
        const alpha = data[index + 3];
        pixels[index] = Math.round(data[index + 2] * alpha / 255);
        pixels[index + 1] = Math.round(data[index + 1] * alpha / 255);
        pixels[index + 2] = Math.round(data[index] * alpha / 255);
        pixels[index + 3] = alpha;
      }
      const bytes = Buffer.concat([header, pixels]); fs.writeFileSync(file, bytes);
      results.push({ id: record.id, url: url.href, source_sha256: hash(source), sha256: hash(bytes), width: info.width, height: info.height });
    } catch (error) { results.push({ id: record.id, error: String(error.message) }); }
  }
  fs.writeFileSync(path.join(output, 'provenance.json'), JSON.stringify(results, null, 2));
  console.log(JSON.stringify({ imported: results.filter(row => !row.error).length, unavailable: results.filter(row => row.error).length }));
}
main().catch(error => { console.error(error.message); process.exitCode = 1; });
