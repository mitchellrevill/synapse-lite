// Rasterize recovered local shell SVGs for their display role, with alpha.
// Usage: node provision-window-assets.cjs <icons-directory> <output> <sharp-module>
// Only SVG images are decoded; no vendor JavaScript or network is involved.
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');

const SOURCE_DENSITY = 216;
const HOST_ICON_EDGE = 96; // 32 CSS pixels at 3x; ordinary host icons use <= 32.
const GAME_PLACEHOLDER_EDGE = 180; // The game tile displays this at 60 CSS pixels.

async function rasterize(sharp, source, name) {
  const maxEdge = name === 'host-game-placeholder' ? GAME_PLACEHOLDER_EDGE
    : name.startsWith('host-') ? HOST_ICON_EDGE : null;
  let density = SOURCE_DENSITY;
  if (maxEdge !== null) {
    const metadata = await sharp(source).metadata();
    if (!(metadata.width > 0 && metadata.height > 0)) {
      throw new Error(`${name}: missing source dimensions`);
    }
    // Limit SVG rasterization itself, rather than allocating a huge intermediate
    // and only shrinking it afterward. Small source icons retain their 3x density.
    density = Math.min(SOURCE_DENSITY,
      Math.max(1, Math.ceil(72 * maxEdge / Math.max(metadata.width, metadata.height))));
  }
  let image = sharp(source, { density });
  if (maxEdge !== null) {
    image = image.resize({ width: maxEdge, height: maxEdge, fit: 'inside', withoutEnlargement: true });
  }
  const { data, info } = await image.ensureAlpha().raw().toBuffer({ resolveWithObject: true });
  if (info.channels !== 4) throw new Error(`${name}: expected RGBA`);
  return { data, info, density, maxEdge };
}

function encodePremultipliedBgra(data, info) {
  const header = Buffer.alloc(16);
  header.write('SLBGRA1\0', 0, 'ascii');
  header.writeUInt32LE(info.width, 8);
  header.writeUInt32LE(info.height, 12);
  const pixels = Buffer.alloc(data.length);
  for (let offset = 0; offset < data.length; offset += 4) {
    const alpha = data[offset + 3];
    pixels[offset] = Math.round(data[offset + 2] * alpha / 255);
    pixels[offset + 1] = Math.round(data[offset + 1] * alpha / 255);
    pixels[offset + 2] = Math.round(data[offset] * alpha / 255);
    pixels[offset + 3] = alpha;
  }
  return Buffer.concat([header, pixels]);
}

async function main() {
  const [input, output, sharpModule] = process.argv.slice(2);
  if (!input || !output || !sharpModule) {
    throw new Error('Expected <icons-directory> <output> <sharp-module>');
  }
  const sharp = require(path.resolve(sharpModule));
  const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
  const records = [];
  fs.mkdirSync(output, { recursive: true });
  const names = fs.readdirSync(input)
    .filter(file => file.startsWith('icon-') && file.endsWith('.svg'))
    .map(file => file.slice(5, -4));
  for (const name of names) {
    const source = fs.readFileSync(path.join(input, `icon-${name}.svg`));
    const { data, info, density, maxEdge } = await rasterize(sharp, source, name);
    const bytes = encodePremultipliedBgra(data, info);
    const file = `icon-${name}.bgra`;
    fs.writeFileSync(path.join(output, file), bytes);
    records.push({ file, source_sha256: hash(source), sha256: hash(bytes),
      width: info.width, height: info.height, density, max_edge: maxEdge,
      format: 'top-down premultiplied BGRA', network: false });
  }
  fs.writeFileSync(path.join(output, 'window-assets-provenance.json'), JSON.stringify(records, null, 2));
  process.stdout.write(`Provisioned ${records.length} transparent shell icons\n`);
}

main().catch(error => { process.stderr.write(`${error.stack}\n`); process.exitCode = 1; });
