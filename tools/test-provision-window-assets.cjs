// Exercise the real CLI, including SVG decoding, dimensions and BGRA serialization.
const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { execFileSync } = require('node:child_process');

test('host icons have bounded pixels while device artwork keeps its resolution and alpha', () => {
  const sharpModule = process.env.SECBIRD_SHARP_MODULE;
  assert.ok(sharpModule, 'Set SECBIRD_SHARP_MODULE to the installed Sharp module');
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'window-assets-'));
  try {
    const source = '<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="512" viewBox="0 0 1024 512"><rect width="1024" height="512" fill="#ff8000" opacity="0.5"/></svg>';
    for (const role of ['host-macro', 'host-game-placeholder', 'device-art']) {
      fs.writeFileSync(path.join(root, `icon-${role}.svg`), source);
    }
    execFileSync(process.execPath, [path.join(__dirname, 'provision-window-assets.cjs'), root, root, sharpModule]);
    const provenance = JSON.parse(fs.readFileSync(path.join(root, 'window-assets-provenance.json')));
    for (const [role, width, height] of [['host-macro',96,48], ['host-game-placeholder',180,90], ['device-art',3072,1536]]) {
      const data = fs.readFileSync(path.join(root, `icon-${role}.bgra`));
      assert.equal(data.subarray(0,8).toString('ascii'), 'SLBGRA1\0');
      assert.equal(data.readUInt32LE(8), width);
      assert.equal(data.readUInt32LE(12), height);
      assert.equal(data.length, 16+width*height*4);
      const center = 16 + (Math.floor(height/2)*width+Math.floor(width/2))*4;
      assert.ok(data[center+3]>=126 && data[center+3]<=129, 'Alpha must survive rasterization');
      for (let offset=16;offset<data.length;offset+=4) {
        assert.ok(data[offset]<=data[offset+3] && data[offset+1]<=data[offset+3] && data[offset+2]<=data[offset+3]);
      }
      const record = provenance.find(item=>item.file===`icon-${role}.bgra`);
      assert.equal(record.width,width);
      assert.equal(record.height,height);
    }
  } finally {
    const resolved = fs.realpathSync(root);
    assert.equal(path.dirname(resolved),fs.realpathSync(os.tmpdir()));
    assert.ok(path.basename(resolved).startsWith('window-assets-'));
    fs.rmSync(resolved, {recursive:true,force:true});
  }
});
