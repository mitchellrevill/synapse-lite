// Recover exact local favicon SVGs; do not fetch assets or execute vendor code.
// node provision-host-assets.cjs <apps.razer.com-cache> <output> <sharp-module>
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const { execFileSync } = require('node:child_process');
const [cache, output, sharp] = process.argv.slice(2);
if (!cache || !output || !sharp) throw new Error('Expected <local-cache> <output> <sharp-module>');
const sources = [
  ['game-placeholder', 'synapse/profiles/static/media/placeholder_game_icon.17a60ecb.svg___WB_REVISION___459c32f134cab855a327bfe38b520e15', '6f2331c0a16867d9551b659db45a706d7bda559f7e5b9a44103430a4992cc92e'],
  ['linked-games', 'synapse/assets/imgs/favicon/GAMEPAD.svg___WB_REVISION___e1d447ce1f56269ea4360bc14a236aa3', '30e5ae82c08a102cbfe4f67aa8bb4eba2b866c5678e1527d277e8af9392c100c'],
  ['synapse', 'synapse/dashboard/favicon.svg___WB_REVISION___94ab882339447c137f3c5c03f5eca2d8', '76acb76f772f2696512943817219b912b3246c95a636876fe44dee835f711e14'],
  ['mouse', 'synapse/assets/imgs/favicon/MOUSE.svg___WB_REVISION___883f9809a60cca176c163e0724ca27e6', '7abd0c47857cd07599ad3ac1dd4da90421b72b051278f71bd4aa266d69b28580'],
  ['keyboard', 'synapse/assets/imgs/favicon/KEYBOARD.svg___WB_REVISION___6b5905b6b4286d086ce57874b5127ef3', 'a5c665f592beae2b7cab277af9c1ffc01784f2240497425d2d33841541e51811'],
  ['scroll-left', '../../icons/raw/left-arrow.svg', 'c67e22c86bbbf9644013fd78f21cccd1046b88d218fe3eea221f3b790ef42531'],
  ['scroll-right', '../../icons/raw/right-arrow.svg', '15d6cc42d1243c8dd8564ff55b8ad3435adaf188af0282a80b932e0ef4547653'],
  ['scroll-left-hover', '../../icons/raw/left-arrow-hover.svg', '97766c9c7761a1bea8cff5096cd5454b711e99b63b0b046f35d818e7a55470e5'],
  ['scroll-left-active', '../../icons/raw/left-arrow-active.svg', '6200a6c42b44e7a7d886fe19df8ff0a1aebdd2629686ad05f33df080eb349b1c'],
  ['scroll-left-disabled', '../../icons/raw/left-arrow-disabled.svg', '4e7995fe9aafc7a65705d13938b3e06d0077c24a223ea3ac7c22443c0310c44d'],
  ['scroll-right-hover', '../../icons/raw/right-arrow-hover.svg', 'd1036fee99ef0a92bf3c848ffaa9bdcefcb15f2cbbb007e637269b5213eedf24'],
  ['scroll-right-active', '../../icons/raw/right-arrow-active.svg', '5f3a805447c11a7aaf24bddcf6942ccb8cba445901d3a2cbdc294533b6b19d49'],
  ['scroll-right-disabled', '../../icons/raw/right-arrow-disabled.svg', '309d3e2e8518b4c62f0b6595520ea58673267926ac0fa06d51486cb220ac50ac'],
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
