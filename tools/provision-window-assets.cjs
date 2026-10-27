// Rasterize recovered local shell SVGs at their intrinsic size, with alpha.
// Usage: node provision-window-assets.cjs <icons-directory> <output> <sharp-module>
// Only SVG images are decoded; no vendor JavaScript or network is involved.
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');

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
    // 3x raster density retains the intrinsic aspect ratio and small strokes.
    const { data, info } = await sharp(source, { density: 216 })
      .ensureAlpha().raw().toBuffer({ resolveWithObject: true });
    if (info.channels !== 4) throw new Error(`${name}: expected RGBA`);
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
    const bytes = Buffer.concat([header, pixels]);
    const file = `icon-${name}.bgra`;
    fs.writeFileSync(path.join(output, file), bytes);
    records.push({ file, source_sha256: hash(source), sha256: hash(bytes),
      width: info.width, height: info.height, density: 216,
      format: 'top-down premultiplied BGRA', network: false });
  }
  fs.writeFileSync(path.join(output, 'window-assets-provenance.json'), JSON.stringify(records, null, 2));
  process.stdout.write(`Provisioned ${records.length} transparent shell icons\n`);
}

main().catch(error => { process.stderr.write(`${error.stack}\n`); process.exitCode = 1; });
