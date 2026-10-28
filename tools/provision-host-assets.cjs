// Recover exact local favicon SVGs; do not fetch assets or execute vendor code.
// node provision-host-assets.cjs <apps.razer.com-cache> <output> <sharp-module>
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const { execFileSync } = require('node:child_process');
const [cache, output, sharp] = process.argv.slice(2);
if (!cache || !output || !sharp) throw new Error('Expected <local-cache> <output> <sharp-module>');
const sources = [
  ['synapse', 'synapse/dashboard/favicon.svg___WB_REVISION___94ab882339447c137f3c5c03f5eca2d8', '76acb76f772f2696512943817219b912b3246c95a636876fe44dee835f711e14'],
  ['mouse', 'synapse/assets/imgs/favicon/MOUSE.svg___WB_REVISION___883f9809a60cca176c163e0724ca27e6', '7abd0c47857cd07599ad3ac1dd4da90421b72b051278f71bd4aa266d69b28580'],
  ['keyboard', 'synapse/assets/imgs/favicon/KEYBOARD.svg___WB_REVISION___6b5905b6b4286d086ce57874b5127ef3', 'a5c665f592beae2b7cab277af9c1ffc01784f2240497425d2d33841541e51811'],
  ['scroll-left', '../../icons/raw/left-arrow.svg', 'c67e22c86bbbf9644013fd78f21cccd1046b88d218fe3eea221f3b790ef42531'],
  ['scroll-right', '../../icons/raw/right-arrow.svg', '15d6cc42d1243c8dd8564ff55b8ad3435adaf188af0282a80b932e0ef4547653'],
];
// Verify every input before writing any output. Assets remain private build data.
const verified = sources.map(([role, source, sha256]) => {
  const bytes = fs.readFileSync(path.join(cache, source));
  if (crypto.createHash('sha256').update(bytes).digest('hex') !== sha256) throw new Error(`Source hash mismatch: ${role}`);
  return { role, source, sha256, bytes };
});
fs.mkdirSync(output, { recursive: true });
for (const item of verified) fs.writeFileSync(path.join(output, `icon-host-${item.role}.svg`), item.bytes);
execFileSync(process.execPath, [path.join(__dirname, 'provision-window-assets.cjs'), output, output, sharp], { stdio: 'inherit' });
fs.writeFileSync(path.join(output, 'host-assets-provenance.json'), JSON.stringify(verified.map(({ bytes, ...item }) => item), null, 2));
