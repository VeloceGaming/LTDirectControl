// Reuse the supplied SVG geometry; deterministic rasterization, no image generation.
const fs=require('fs'),path=require('path'),vm=require('vm'),crypto=require('crypto');
const sharp=require('C:/Users/j9010/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/sharp');
const root=path.resolve(__dirname,'..');
const html=fs.readFileSync(path.join(root,'design/cursor/index.html'),'utf8');
const code=html.slice(html.indexOf('const C='),html.indexOf('/* Sheet */'));
const art=vm.runInNewContext(code+'; ({CUR,reticle,badge,brackets,C,arrow})',{}, {timeout:1000});
// Keep the supplied signal colors, with neutral ink/light values.
art.C.ink='#1c1c1c';art.C.light='#eeeeee';
const defs=Object.fromEntries(Object.entries(art.CUR).map(([key,c])=>[key,{body:c.svg(),hot:c.hot}]));
defs.achamp={body:art.reticle(false)+art.badge(),hot:[16,16]};
defs.achampLock={body:art.reticle(true)+art.badge(),hot:[16,16]};
for(const [key,valid] of [['skill',null],['skillValid',true],['skillInvalid',false]]) {
 let body=art.reticle(valid===true).replaceAll(art.C.warn,valid===false?'#999999':art.C.yellow);
 if(valid===false) body+='<path d="M10 10L22 22" stroke="#1c1c1c" stroke-width="4.5"/><path d="M10 10L22 22" stroke="#eeeeee" stroke-width="2"/>';
 defs[key]={body,hot:[16,16]};defs[key+'Champ']={body:body+art.badge(),hot:[16,16]};
}
const out=path.join(root,'probe/cursor');fs.mkdirSync(out,{recursive:true});
const manifest={source:'design/cursor/index.html',source_sha256:crypto.createHash('sha256').update(html).digest('hex'),format:'64x64 premultiplied BGRA, top-down; embedded in DLL',files:{}};
(async()=>{
 for(const [key,c] of Object.entries(defs)) {
  const svg=`<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64" viewBox="0 0 32 32">${c.body}</svg>`;
  const rgba=await sharp(Buffer.from(svg)).ensureAlpha().raw().toBuffer();
  const bgra=Buffer.alloc(rgba.length);
  for(let i=0;i<rgba.length;i+=4) { const a=rgba[i+3];bgra[i]=Math.round(rgba[i+2]*a/255);bgra[i+1]=Math.round(rgba[i+1]*a/255);bgra[i+2]=Math.round(rgba[i]*a/255);bgra[i+3]=a; }
  fs.writeFileSync(path.join(out,key+'.bgra'),bgra);
  fs.writeFileSync(path.join(out,key+'.svg'),svg);
  await sharp(Buffer.from(svg)).png().toFile(path.join(out,key+'.png'));
  manifest.files[key]={hotspot_32:c.hot,sha256:crypto.createHash('sha256').update(bgra).digest('hex'),bytes:bgra.length};
 }
 const preview=`<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64" viewBox="0 0 32 32">${defs.normal.body}</svg>`;
 await sharp(Buffer.from(preview)).png().toFile(path.join(root,'probe/ui/cursor_preview.png'));
 const tiles=await Promise.all(Object.keys(defs).map(async(key,i)=>({input:await sharp(path.join(out,key+'.png')).resize(48,48).toBuffer(),left:(i%7)*80+16,top:Math.floor(i/7)*80+16})));
 await sharp({create:{width:560,height:160,channels:4,background:'#383838'}}).composite(tiles).png().toFile(path.join(root,'research/cursor-sheet-0.39.0.png'));
 fs.writeFileSync(path.join(root,'research/cursor-assets-0.39.0.json'),JSON.stringify(manifest,null,2)+'\n');
 console.log(`Rendered ${Object.keys(defs).length} SVG cursor states and settings preview.`);
})().catch(e=>{console.error(e);process.exitCode=1});
