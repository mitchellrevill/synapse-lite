// Recover exact local favicon SVGs; do not fetch assets or execute vendor code.
// node provision-host-assets.cjs <apps.razer.com-cache> <output> <sharp-module>
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const { execFileSync } = require('node:child_process');
const [cache, output, sharp] = process.argv.slice(2);
if (!cache || !output || !sharp) throw new Error('Expected <local-cache> <output> <sharp-module>');
const sources = [
  ['macro-delete', 'synapse/macro/static/media/icon_delete.d1a6a9ad.svg', '5a7c6f9a8e6f7719c2d2332551367359a05038b6d61fff42a64920ff8eaa88e7'],
  ['tutorial-book', 'synapse/macro/static/media/icon_tutorial_book.8b20c805.svg', '518c5e047f9016354567f46a5f5d66f5920284efb423998f22b33a0fe6c57683'],
  ['macro-record', 'synapse/macro/static/media/icon_record.9183ffe6.svg', 'a0049ffbdbd338594d01c69994b2943e1f2371fc0d4216c3e2de0594373e600f'],
  ['macro-undo-enabled', 'synapse/macro/static/media/icon_undo_enable.dc068f5b.svg', '7a1de677bbc16577f5e1a7409b524bd410419c8c3892f09c6e1c32bd0048a601'],
  ['macro-redo-enabled', 'synapse/macro/static/media/icon_redo_enable.9fb6a098.svg', '17900148c1b4f17fd935c0473e77cbe59c4da26a1474a7211303449e4c5e28a4'],
  ['macro-delay', 'synapse/macro/static/media/icon_delay_g.5050a3f7.svg___WB_REVISION___fcfaf5a6c7afda10aec8744baf3e7a4a', '1583896befa623e3365ea1fa9a8ee15de5aa58f36018dea742f57639814c6ad0'],
  ['macro-keyboard', 'synapse/macro/static/media/icon_config_keyboard_a.7051c99b.svg', '76caf7c16ab72f4c0b32472bface5da779a7c517ecb03eaa6c2aacd26f7b0cb9'],
  ['macro-mouse', 'synapse/macro/static/media/icon_config_mouse_o.8a44fbe7.svg___WB_REVISION___43fbf7cffb4d8dd12d968e74d2f4cf40', '1fd5593b525c2a66097dbba2c69d04aff9ca3484b2083e77ce0f3487611e0521'],
  ['macro-launch', 'synapse/macro/static/media/icon_config_launch_p.482fbff5.svg___WB_REVISION___abaa1d64e7d9a5db02dd6d451fc405a4', 'a61cf24445c687cc4b24ebb7e9b8489535ecd7fca7347b0815c87c748a82d5a3'],
  ['macro-command', 'synapse/macro/static/media/icon_runcmd_b.10c00024.svg___WB_REVISION___5487e16c6ba00152334203fb5a2317ce', '37a2394fb6980581c6827bdd61897f05440a8df486ff99360c93c3f7cae2318b'],
  ['macro-text', 'synapse/macro/static/media/icon_config_text_b.bc93ac89.svg___WB_REVISION___76266abfb02288526de0a594315c36b8', 'aace7b2ef9ee0486dd8f617c3a93d8340a64059c1652f1b0bd5adea06907a353'],
  ['macro-loop', 'synapse/macro/static/media/icon_refresh-1_r.ff48f955.svg___WB_REVISION___f782e58a79cd01f8827d5dac503ec86b', '613c38b8dba5e15c3f57a1c83577e3d94e5ca55fc3dba9773401d8f45d146a85'],
  ['macro-undo', 'synapse/macro/static/media/icon_undo.638cc83d.svg', 'f681c0dff2389bfac89524620c51ce23f0e3e6308ae23361d7e15812c5d990ae'],
  ['macro-redo', 'synapse/macro/static/media/icon_redo.6df14b75.svg', '3c49a5dcb8d174de49dd45b760611c144604a3b4bbaea5bd5831e96a6b6b5408'],
  ['settings', 'synapse/dashboard/static/media/icon_settings-2.07e96d4c.svg', '7bf93d2d7e3c9c409f10383fbfdd21dc7dfbb06fc0eeeea11b3b62ddf9da75e5'],
  ['macro', 'synapse/assets/imgs/apps/logo_macro.svg___WB_REVISION___392d69f6c20fb077830ac8f773385a53', '6e78e4b830dd65110071ec967fa859e74478fa2a1c5eaa1693f72b57e5c80b4e'],
  ['chroma', 'systray/systrayv2/static/media/logo_chroma-app.79c77366.svg', '7e68411fce5c77757edbbd74f521e7a6c154c82b87f23401ebbd45d1e798e832'],
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
