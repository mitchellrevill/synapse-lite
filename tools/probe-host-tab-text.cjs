const fs=require('node:fs'),path=require('node:path'),crypto=require('node:crypto');
// Inert local CSS/font replay only; no vendor JavaScript or network.
// node probe-host-tab-text.cjs <evidence> <output> <playwright-module> <edge-exe>
const [input, target, playwrightModule, edge] = process.argv.slice(2);
if (!input || !target || !playwrightModule || !edge) throw new Error('Expected <evidence> <output> <playwright-module> <edge-exe>');
const pw=require(path.resolve(playwrightModule));
const root=path.resolve(input),output=path.resolve(target);
fs.mkdirSync(output,{recursive:true});
const hash=b=>crypto.createHash('sha256').update(b).digest('hex');
(async()=>{
 const css=fs.readFileSync(path.join(root,'electron_index.css'),'utf8');
 const fontCSS=fs.readFileSync(path.join(root,'electron_assets_style_Roboto.css'),'utf8');
 const font=fs.readFileSync(path.join(root,'fonts-sfnt/Roboto-Light.ttf'));
 const browser=await pw.chromium.launch({executablePath:path.resolve(edge),headless:true,args:['--disable-background-networking','--disable-component-update','--no-first-run']});
 const rows=[];
 try {
  for(const scale of [1,1.25,1.5,2]){
   const page=await browser.newPage({viewport:{width:1280,height:720},deviceScaleFactor:scale});
   await page.route('**/*',r=>r.abort());
   for(const offset of [0,0.25]) for(const text of ['SYNAPSE','DEATHADDER V3 HYPERSPEED','ORNATA V2','AVATAR To fi ffi']){
    await page.setContent(`<meta http-equiv="Content-Security-Policy" content="default-src 'none';style-src 'unsafe-inline';font-src data:"><style>${css}@font-face{font-family:Roboto;font-style:normal;font-weight:300;src:url(data:font/ttf;base64,${font.toString('base64')}) format('truetype')}</style><div class="etabs-tabgroup visible" style="left:${offset}px;top:${offset}px"><div class="etabs-tab visible active main-tab non-collapsible-tab"><div class="etabs-content"><div class="tab-pos"></div><div class="etabs-tab-icon"></div><div class="etabs-tab-title"><span id="label">${text}</span><i id="baseline" style="display:inline-block;width:0;height:0;vertical-align:baseline"></i></div><div class="btn-close"></div><div class="etabs-tab-buttons"></div></div></div><div class="etabs-tabs"></div></div>`);
    await page.evaluate(()=>document.fonts.ready);
    rows.push(await page.evaluate(({scale,offset,text})=>{
      const e=document.querySelector('#label'),title=e.parentElement,content=title.parentElement,tab=content.parentElement,style=getComputedStyle(e),r=document.createRange();r.selectNodeContents(e);
      const rect=e=>{const b=e.getBoundingClientRect();return{x:b.x,y:b.y,width:b.width,height:b.height}};
      const c=document.createElement('canvas').getContext('2d');c.font=`${style.fontWeight} ${style.fontSize} ${style.fontFamily}`;c.fontKerning='normal';const effectiveText=style.textTransform==='uppercase'?e.textContent.toUpperCase():e.textContent;const variants={};for(const k of ['auto','normal','none']){c.fontKerning=k;variants[k]=c.measureText(effectiveText).width;}c.fontKerning='normal';const m=c.measureText(effectiveText);
      const baseline=document.querySelector('#baseline').getBoundingClientRect().top;
      return{scale,offset,text,rendered_text:effectiveText,devicePixelRatio,computed:{font:style.font,fontFamily:style.fontFamily,fontSize:style.fontSize,fontWeight:style.fontWeight,lineHeight:style.lineHeight,letterSpacing:style.letterSpacing,fontKerning:style.fontKerning,fontFeatureSettings:style.fontFeatureSettings},font_check:document.fonts.check('300 12px Roboto'),tab:{...rect(tab),clientWidth:tab.clientWidth,offsetWidth:tab.offsetWidth},content:rect(content),title:{...rect(title),clientWidth:title.clientWidth,offsetWidth:title.offsetWidth,scrollWidth:title.scrollWidth},label:rect(e),dom_range:rect(r),baseline_css:baseline,baseline_from_tab:baseline-tab.getBoundingClientRect().top,baseline_from_content:baseline-content.getBoundingClientRect().top,baseline_from_title:baseline-title.getBoundingClientRect().top,canvas:{kerning_variants:variants,advance_css:m.width,fontAscent:m.fontBoundingBoxAscent,fontDescent:m.fontBoundingBoxDescent,actualAscent:m.actualBoundingBoxAscent,actualDescent:m.actualBoundingBoxDescent},physical:{baseline:baseline*scale,dom_advance:r.getBoundingClientRect().width*scale}};
    },{scale,offset,text}));
   }
   await page.close();
  }
  const out={authority:'controlled inert CSS/font replay',browser:await browser.version(),network:false,vendor_javascript_executed:false,screenshots:false,source:{css_sha256:hash(css),font_stylesheet_sha256:hash(fontCSS),font_sha256:hash(font),font_bytes:font.length,original_font_mapping:fontCSS.match(/@font-face\{[^}]*font-weight:300[^}]*font-style:normal[^}]*\}/)?.[0]||fontCSS.slice(0,fontCSS.indexOf('}')+1)},rows};
  fs.writeFileSync(path.join(output,'metrics.json'),JSON.stringify(out,null,2));
  console.log(JSON.stringify({fixtures:rows.length,browser:out.browser,network:false,vendor_javascript_executed:false,output:path.join(output,'metrics.json')}));
 }finally{await browser.close();}
})().catch(e=>{console.error(e);process.exitCode=1});

