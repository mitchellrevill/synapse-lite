// Convert locally recovered SVG assets using Chromium's SVG/CSS renderer.
// Generated vendor assets remain private/ignored. No fetches, screenshots, or
// vendor JavaScript execution are involved.
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');

const usage = `Usage: node provision-assignment-assets.cjs
  --media <recovered605/ui/static/media>
  --output <ignored-runtime-assets>
  [--mouse-media <recovered196/ui/static/media>]
  --stylesheet <recovered605-main.css>
  --playwright <absolute-playwright-module-path>
  --browser <local-Chromium-or-Edge-executable>
  [--scale <physical-pixels-per-CSS-pixel, default 1>]`;

const categories = ['DEFAULT', 'AI_LAUNCHER', 'KEYBOARD_FUNCTION', 'MOUSE_FUNCTION',
  'SENSITIVITY', 'MACRO', 'INTERDEVICE', 'SWITCH_PROFILE', 'SWITCH_LIGHTING',
  'DEVICE_BRIGHTNESS', 'RAZER_HYPERSHIFT', 'LAUNCH_PROGRAM', 'MULTIMEDIA',
  'WINDOWS_SHORTCUT', 'TEXT_FUNCTION', 'DISABLE', 'DIAL_MULTI_FUNCTION'];

function sourceIcons(stylesheet) {
  const css = fs.readFileSync(stylesheet, 'utf8');
  const rules = [...css.matchAll(/([^{}]+)\{([^{}]+)\}/g)];
  const icons = [];
  for (const category of categories) {
    for (const [variant, background] of [['normal', '#222222'], ['hover', '#393939'], ['active', '#111111']]) {
      const normal = `.action-wrapper.${category} .head:before`;
      const active = `.action-wrapper.${category}.open .head:before`;
      const find = selector => rules.filter(rule => rule[1].split(',').includes(selector) && /url\(/.test(rule[2])).at(-1);
      const rule = variant === 'active' ? (find(active) || find(normal)) : find(normal);
      if (!rule) throw new Error(`Missing source category icon: ${category}/${variant}`);
      const url = rule[2].match(/url\(([^)]+)\)/)[1].replace(/^['"]|['"]$/g, '');
      icons.push({ key: `assignment-${category}-${variant}`, source: url.startsWith('data:') ? url : path.basename(url), size: 20, background });
    }
  }
  icons.push(
    {key:'assignment-help',source:'icon_help.377359c3.svg',size:14,background:'#222222'},
    {key:'assignment-dialog-close',source:'icon_close.55fe41f1.svg',size:20,background:'#111111'},
    {key: 'assignment-close', source: 'icon_close.55fe41f1.svg', size:20, background:'#222222'},
    {key: 'assignment-panel', source: 'icon_sidepanel.e53fef93.svg', size:18, background:'#111111'},
    {key: 'assignment-panel-active', source:'icon_sidepanel_a.90d67a6e.svg',size:18,background:'#111111'},
    {key: 'assignment-panel-close', source: 'icon_closepanel.86903958.svg', size:20, background:'#2b2b2b'}
  );
  return icons;
}

function parseArguments(argv) {
  const accepted = new Set(['media', 'output', 'playwright', 'browser', 'scale', 'stylesheet', 'mouse-media']);
  const args = { scale: 1 };
  for (let index = 0; index < argv.length; index += 2) {
    const flag = argv[index];
    if (!flag?.startsWith('--') || !accepted.has(flag.slice(2)) || !argv[index + 1]) {
      throw new Error(`Invalid argument ${flag || ''}\n${usage}`);
    }
    args[flag.slice(2)] = argv[index + 1];
  }
  for (const required of ['media', 'output', 'playwright', 'browser', 'stylesheet']) {
    if (!args[required]) throw new Error(`Missing --${required}\n${usage}`);
    args[required] = path.resolve(args[required]);
  }
  args.scale = Number(args.scale);
  if (!Number.isFinite(args.scale) || args.scale < 0.5 || args.scale > 4) {
    throw new Error('--scale must be a finite value between 0.5 and 4');
  }
  return args;
}

function sha256(bytes) {
  return crypto.createHash('sha256').update(bytes).digest('hex');
}

function makeBitmap(rgb, width, height) {
  const stride = (width * 3 + 3) & ~3;
  const bitmap = Buffer.alloc(54 + stride * height);
  bitmap.write('BM');
  bitmap.writeUInt32LE(bitmap.length, 2);
  bitmap.writeUInt32LE(54, 10);
  bitmap.writeUInt32LE(40, 14);
  bitmap.writeInt32LE(width, 18);
  bitmap.writeInt32LE(height, 22);
  bitmap.writeUInt16LE(1, 26);
  bitmap.writeUInt16LE(24, 28);
  bitmap.writeUInt32LE(stride * height, 34);
  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      const source = (y * width + x) * 3;
      const target = 54 + (height - 1 - y) * stride + x * 3;
      bitmap[target] = rgb[source + 2];
      bitmap[target + 1] = rgb[source + 1];
      bitmap[target + 2] = rgb[source];
    }
  }
  return bitmap;
}

function bitmapPixels(bitmap) {
  if (bitmap.toString('ascii', 0, 2) !== 'BM' || bitmap.readUInt16LE(28) !== 24) {
    throw new Error('Expected uncompressed 24-bit BMP');
  }
  const width = bitmap.readInt32LE(18);
  const height = bitmap.readInt32LE(22);
  const stride = (width * 3 + 3) & ~3;
  const offset = bitmap.readUInt32LE(10);
  const rgb = Buffer.alloc(width * height * 3);
  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      const source = offset + (height - 1 - y) * stride + x * 3;
      const target = (y * width + x) * 3;
      rgb[target] = bitmap[source + 2];
      rgb[target + 1] = bitmap[source + 1];
      rgb[target + 2] = bitmap[source];
    }
  }
  return rgb;
}

function pixelStatistics(rgba) {
  const colors = new Map();
  const partialSamples = [];
  let transparent = 0;
  let partial = 0;
  let opaque = 0;
  for (let index = 0; index < rgba.length; index += 4) {
    const pixel = [...rgba.subarray(index, index + 4)];
    if (pixel[3] === 0) transparent += 1;
    else if (pixel[3] === 255) opaque += 1;
    else {
      partial += 1;
      if (partialSamples.length < 8) partialSamples.push(pixel);
    }
    const key = pixel.join(',');
    colors.set(key, (colors.get(key) || 0) + 1);
  }
  return {
    transparent_pixels: transparent,
    partial_alpha_pixels: partial,
    opaque_pixels: opaque,
    unique_rgba_colors: colors.size,
    partial_alpha_unpremultiplied_samples: partialSamples,
    most_frequent_colors: [...colors].sort((a, b) => b[1] - a[1]).slice(0, 10),
    folder_source_gray_opaque_pixels: colors.get('112,112,112,255') || 0,
    cone_source_orange_opaque_pixels: colors.get('253,134,17,255') || 0,
  };
}

async function main() {
  if (process.argv.slice(2).includes('--help')) {
    process.stdout.write(`${usage}\n`);
    return;
  }
  const args = parseArguments(process.argv.slice(2));
  const sourceFiles = fs.readdirSync(args.media);
  const icons=sourceIcons(args.stylesheet);
  if(args['mouse-media'])icons.push({key:'assignment-mouse-cycle',source:'litat2m-profile-button-3x.e01e76b1.avif',width:300,height:340,background:'#222222',mouse:true});
  const inputs = icons.map(icon => {
    if (icon.source.startsWith('data:')) {
      const source = Buffer.from(icon.source.split(',')[1], 'base64');
      return {...icon, source_file:'embedded-source-css-data-url', source_sha256:sha256(source), data_url:icon.source, physical_size:Math.round(icon.size * args.scale),physical_width:Math.round(icon.size * args.scale),physical_height:Math.round(icon.size * args.scale)};
    }
    const files=icon.mouse?fs.readdirSync(args['mouse-media']):sourceFiles;
    const matches = files.filter(name => name === icon.source || name.startsWith(`${icon.source}___WB_REVISION___`));
    if (matches.length !== 1) {
      throw new Error(`Expected one recovered ${icon.source}; found ${matches.length}`);
    }
    const source = fs.readFileSync(path.join(icon.mouse?args['mouse-media']:args.media, matches[0]));
    return {
      ...icon,
      source_file: matches[0],
      source_sha256: sha256(source),
      data_url: `data:${icon.mouse?'image/avif':'image/svg+xml'};base64,${source.toString('base64')}`,
      physical_size: Math.round(icon.size * args.scale),
      physical_width:Math.round((icon.width||icon.size)*args.scale),
      physical_height:Math.round((icon.height||icon.size)*args.scale),
    };
  });
  const browser = await require(args.playwright).chromium.launch({
    executablePath: args.browser,
    headless: true,
    args: ['--disable-background-networking', '--disable-component-update', '--no-first-run'],
  });
  const prepared = [];
  let version;
  try {
    version = await browser.version();
    const page = await browser.newPage();
    await page.route('**/*', request => request.abort());
    await page.setContent('<meta http-equiv="Content-Security-Policy" content="default-src \'none\'; img-src data:">');
    for (const input of inputs) {
      const rendered = await page.evaluate(async icon => {
        const image = new Image();
        image.src = icon.data_url;
        await image.decode();
        const width=icon.physical_width;
        const height=icon.physical_height;
        const transparent = document.createElement('canvas');
        transparent.width = width;
        transparent.height = height;
        const rgbaContext = transparent.getContext('2d', { colorSpace: 'srgb' });
        rgbaContext.drawImage(image, 0, 0, width, height);
        // getImageData exposes straight/unpremultiplied RGBA. These bytes must
        // never be passed into a premultiplied-alpha compositor as-is.
        const rgba = rgbaContext.getImageData(0, 0, width, height).data;
        const flat = document.createElement('canvas');
        flat.width = width;
        flat.height = height;
        const flatContext = flat.getContext('2d', { colorSpace: 'srgb' });
        flatContext.fillStyle = icon.background;
        flatContext.fillRect(0, 0, width, height);
        // Browser flattening directly from the SVG image is the reference.
        // Do not blend a second time from potentially premultiplied samples.
        flatContext.drawImage(image, 0, 0, width, height);
        const pixels = flatContext.getImageData(0, 0, width, height).data;
        const rgb = [];
        for (let index = 0; index < pixels.length; index += 4) {
          rgb.push(pixels[index], pixels[index + 1], pixels[index + 2]);
          if (pixels[index + 3] !== 255) throw new Error('Flattened canvas was not opaque');
        }
        return { rgba: Array.from(rgba), rgb };
      }, input);
      const rgba = Buffer.from(rendered.rgba);
      const rgb = Buffer.from(rendered.rgb);
      const stats = pixelStatistics(rgba);
      const background = Buffer.from(input.background.slice(1), 'hex');
      let maximumStraightAlphaError = 0;
      for (let pixel = 0; pixel < rgba.length / 4; pixel += 1) {
        const alpha = rgba[pixel * 4 + 3];
        for (let channel = 0; channel < 3; channel += 1) {
          const flattened = Math.round((rgba[pixel * 4 + channel] * alpha + background[channel] * (255 - alpha)) / 255);
          maximumStraightAlphaError = Math.max(maximumStraightAlphaError, Math.abs(flattened - rgb[pixel * 3 + channel]));
        }
      }
      // The browser's premultiplied internal storage can lose one integer unit
      // when getImageData unpremultiplies. Larger errors indicate wrong alpha
      // interpretation; use the original browser flattening for the BMP itself.
      if (maximumStraightAlphaError > 1) throw new Error(`${input.key} RGBA alpha interpretation failed`);
      stats.maximum_straight_alpha_flatten_error = maximumStraightAlphaError;
      if ((stats.transparent_pixels === 0 && input.key !== 'assignment-panel-close' && !input.mouse) || stats.unique_rgba_colors < 2) {
        throw new Error(`${input.key} did not render as a transparent source icon`);
      }
      if (input.key === 'profile-folder' && stats.folder_source_gray_opaque_pixels === 0) {
        throw new Error('Source folder CSS fill #707070 was not rendered');
      }
      if (input.key === 'profile-cone' && stats.cone_source_orange_opaque_pixels === 0) {
        throw new Error('Source cone CSS fill #fd8611 was not rendered');
      }
      const bitmap = makeBitmap(rgb, input.physical_width, input.physical_height);
      if (!bitmapPixels(bitmap).equals(rgb)) throw new Error('BMP differs from browser CanvasRGB');
      prepared.push({ input, rgba, rgb, bitmap, stats });
    }
  } finally {
    await browser.close();
  }
  fs.mkdirSync(args.output, { recursive: true });
  const records = [];
  for (const item of prepared) {
    const output = `icon-${item.input.key}.bmp`;
    const destination = path.join(args.output, output);
    fs.writeFileSync(destination, item.bitmap);
    if (!bitmapPixels(fs.readFileSync(destination)).equals(item.rgb)) {
      throw new Error(`Saved ${output} differs from browser CanvasRGB`);
    }
    records.push({
      key: item.input.key,
      source_file: item.input.source_file,
      source_sha256: item.input.source_sha256,
      output,
      output_sha256: sha256(item.bitmap),
      logical_width:item.input.width||item.input.size,
      logical_height:item.input.height||item.input.size,
      physical_width:item.input.physical_width,
      physical_height:item.input.physical_height,
      scale: args.scale,
      background: item.input.background,
      unpremultiplied_rgba_sha256: sha256(item.rgba),
      browser_flattened_rgb_sha256: sha256(item.rgb),
      bmp_rgb_matches_browser_canvas: true,
      pixel_statistics: item.stats,
    });
  }
  fs.writeFileSync(path.join(args.output, 'assignment-assets-provenance.json'), JSON.stringify({
    source: 'cached shipped Ornata605 Customize action-wrapper CSS',
    stylesheet_sha256:sha256(fs.readFileSync(args.stylesheet)),
    engine: 'Chromium SVG Image decode and Canvas2D sRGB',
    browser_version: version,
    scale: args.scale,
    network: false,
    vendor_javascript_executed: false,
    screenshots: false,
    records,
  }, null, 2));
  process.stdout.write(`Provisioned and pixel-verified ${records.length} private Customize bitmaps at scale ${args.scale}\n`);
}

main().catch(error => {
  process.stderr.write(`${error.stack || error}\n`);
  process.exitCode = 1;
});
