// Compare region geometry using Chromium's independent SVG renderer. No network,
// scripts, legends or chassis are rendered; this checks selectable interiors.
// Usage: node verify-studio-projection.cjs PLAYWRIGHT BROWSER CASES OUTPUT
// CASES: [{identity,svg,projection}] where svg is a private SVG file path.
const fs = require('node:fs');
const crypto = require('node:crypto');

(async () => {
  const [modulePath, executablePath, casePath, output] = process.argv.slice(2);
  const {chromium} = require(modulePath);
  const cases = JSON.parse(fs.readFileSync(casePath));
  if (!cases.length || cases.length > 32) throw Error('Select 1..32 explicit cases');
  const browser = await chromium.launch({executablePath, headless:true});
  try {
    const page = await browser.newPage();
    await page.route('**/*', route => route.abort());
    const records = [];
    for (const item of cases) {
      const source = fs.readFileSync(item.svg);
      const projectionBytes = fs.readFileSync(item.projection);
      if (source.length > 8*1024*1024 || projectionBytes.length > 2*1024*1024) throw Error('Case exceeds budget');
      const projection = JSON.parse(projectionBytes);
      if (crypto.createHash('sha256').update(source).digest('hex') !== projection.source_sha256) throw Error('Source hash differs');
      const result = await page.evaluate(async ({source, projection}) => {
        const ns = 'http://www.w3.org/2000/svg';
        const parsed = new DOMParser().parseFromString(source,'image/svg+xml');
        if (parsed.querySelector('parsererror')) throw Error('Invalid source XML');
        const root = parsed.documentElement;
        const create = tag => document.createElementNS(ns,tag);
        const empty = () => {
          const el = create('svg');
          el.setAttribute('viewBox',projection.view_box.join(' '));
          el.setAttribute('width',String(Math.ceil(projection.view_box[2])));
          el.setAttribute('height',String(Math.ceil(projection.view_box[3])));
          return el;
        };
        const stock = empty(), normalized = empty();
        for (const selection of root.querySelectorAll('.selection')) {
          // Construct inert geometry only. No source scripts/URLs/styles copied.
          const shape = create(selection.localName);
          for (const key of ['d','x','y','width','height','rx','ry','cx','cy','r','points']) {
            if (selection.hasAttribute(key)) shape.setAttribute(key,selection.getAttribute(key));
          }
          shape.setAttribute('fill','white'); shape.setAttribute('stroke','none');
          let current = shape;
          for (let ancestor = selection; ancestor; ancestor = ancestor.parentElement) {
            if (ancestor.hasAttribute('transform')) {
              const group = create('g'); group.setAttribute('transform',ancestor.getAttribute('transform'));
              group.append(current); current = group;
            }
            if (ancestor === root) break;
          }
          const wrapper = create('g'); wrapper.append(current); stock.append(wrapper);
        }
        for (const key of projection.keys) {
          const primitive = key.shape;
          const el = create(primitive ? (primitive.kind === 'ellipse' ? 'ellipse' : 'rect') : 'path');
          if (primitive) {
            for (const [name,value] of Object.entries(primitive)) if (name !== 'kind') el.setAttribute(name,String(value));
          } else el.setAttribute('d',key.d);
          el.setAttribute('fill','white'); el.setAttribute('stroke','none');
          const wrapper = create('g'); wrapper.append(el); normalized.append(wrapper);
        }
        document.body.append(stock,normalized);
        let maximumBoundsDelta = 0;
        if (stock.children.length !== normalized.children.length) throw Error('Region count differs');
        for (let i=0;i<stock.children.length;i++) {
          const a=stock.children[i].getBBox(), b=normalized.children[i].getBBox();
          for (const field of ['x','y','width','height']) maximumBoundsDelta=Math.max(maximumBoundsDelta,Math.abs(a[field]-b[field]));
        }
        stock.remove();normalized.remove();
        // Chromium uses float geometry internally; compare in source units.
        if (maximumBoundsDelta > 0.001) throw Error('Region bounds differ: '+maximumBoundsDelta);
        async function pixels(svg) {
          const image = new Image();
          await new Promise((resolve,reject) => {image.onload=resolve;image.onerror=reject; image.src='data:image/svg+xml;base64,'+btoa(new XMLSerializer().serializeToString(svg));});
          const canvas = document.createElement('canvas'); canvas.width=image.width; canvas.height=image.height;
          if (canvas.width*canvas.height > 16000000) throw Error('Raster dimensions exceed budget');
          const context=canvas.getContext('2d');context.drawImage(image,0,0);
          return context.getImageData(0,0,canvas.width,canvas.height).data;
        }
        const a=await pixels(stock), b=await pixels(normalized);
        let changed=0,maxDelta=0,nonempty=0,totalDelta=0;
        for (let i=3;i<a.length;i+=4) {const delta=Math.abs(a[i]-b[i]);if(delta) changed++;totalDelta+=delta;maxDelta=Math.max(maxDelta,delta);if(a[i]) nonempty++;}
        // Report pixel differences without converting a geometric-bound check
        // into a raster-equivalence claim. Curve tessellation needs separate QA.
        if (!nonempty) throw Error('Empty comparison');
        return {raster_equal:changed===0,changed_pixels:changed,max_alpha_delta:maxDelta,total_alpha_delta:totalDelta,maximum_bounds_delta:maximumBoundsDelta,nonempty_pixels:nonempty,regions:projection.keys.length};
      },{source:source.toString('utf8'),projection});
      records.push({identity:item.identity,source_sha256:projection.source_sha256,projection_sha256:crypto.createHash('sha256').update(projectionBytes).digest('hex'),...result});
    }
    fs.writeFileSync(output,JSON.stringify({renderer:await browser.version(),scope:'selection interiors only; no chassis, legend, CSS stroke or native pixel parity claim',records},null,2));
    console.log(JSON.stringify({cases:records.length,maximum_alpha_delta:Math.max(...records.map(r=>r.max_alpha_delta))}));
  } finally {await browser.close();}
})().catch(error=>{console.error(error);process.exitCode=1;});
