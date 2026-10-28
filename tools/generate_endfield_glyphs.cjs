// Reuse the exact approved lab/library SVG paths with a consistent optical box.
const fs=require('fs'),path=require('path'),crypto=require('crypto');
const sharp=require('C:/Users/j9010/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/sharp');
const root=path.resolve(__dirname,'..'),source=path.join(root,'design/hud/endfield-assets/icons');
// --drawn (1.6.0-perf-dev3): the glyphs the drawn HUD shows with
// `draw_sprite`, which draws a PNG at its own size and cannot tint. Each
// variant in tools/records/drawn-glyphs.json is the shipped 32 px PNG
// (probe/ui/<glyph>.png, untouched) scaled to the size its tree node showed
// it at, with a linear filter like the node's default smooth sampling, and
// multiplied by the node's tint (the tree's image colour), saved as
// probe/ui/drawn_<glyph>_<size>[_<rrggbb>].png. Only these files are written.
if(process.argv.includes('--drawn')){(async()=>{
 const recordFile=path.join(root,'tools/records/drawn-glyphs.json'),record=JSON.parse(fs.readFileSync(recordFile,'utf8'));
 const files={};
 for(const v of record.variants){
  const tint=parseInt(v.tint,16)>>>0,t=[tint>>>24&255,tint>>>16&255,tint>>>8&255,tint&255];
  const {data,info}=await sharp(path.join(root,'probe/ui',v.glyph+'.png')).ensureAlpha().resize(v.size,v.size,{kernel:'linear'}).raw().toBuffer({resolveWithObject:true});
  for(let i=0;i<data.length;i+=4)for(let c=0;c<4;c++)data[i+c]=Math.round(data[i+c]*t[c]/255);
  const name='drawn_'+v.glyph+'_'+v.size+(v.tint==='ffffffff'?'':'_'+v.tint.slice(0,6))+'.png',file=path.join(root,'probe/ui',name);
  await sharp(data,{raw:{width:info.width,height:info.height,channels:4}}).png().toFile(file);
  files['ui/'+name]=crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
 }
 record.files=files;fs.writeFileSync(recordFile,JSON.stringify(record,null,2)+'\n');
 console.log('Baked '+Object.keys(files).length+' drawn glyph variants.');
})().catch(e=>{console.error(e);process.exitCode=1});return;}
const names={camera:'camera',more:'ellipsis',pause:'pause',play:'play',cpu:'cpu',reset:'rotate-ccw',eye:'eye',skull:'skull',swords:'swords',coin:'coins',list:'list',clock:'clock',range:'scan-line',recall:'house',check:'check',unavailable:'circle-alert',swap:'arrow-left-right',star:'star'};
(async()=>{const files={},sources={};for(const [name,sourceName] of Object.entries(names)){
 const raw=fs.readFileSync(path.join(source,sourceName+'.svg'),'utf8');
 // White masks let the native renderer apply the exact warm/accent tint.
 const svg=raw.replaceAll('currentColor','#ffffff').replace('stroke-linecap="round"','stroke-linecap="square"').replace('stroke-linejoin="round"','stroke-linejoin="miter"');
 const file=path.join(root,'probe/ui/ef_'+name+'.png');await sharp(Buffer.from(svg)).resize(32,32).png().toFile(file);
 files['ui/ef_'+name+'.png']=crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');sources[sourceName]=crypto.createHash('sha256').update(raw).digest('hex');
}fs.copyFileSync(path.join(source,'LICENSE'),path.join(root,'probe/ui/Endfield-icons-LICENSE.txt'));
fs.writeFileSync(path.join(root,'tools/records/endfield-glyphs.json'),JSON.stringify({files,sources},null,2)+'\n');console.log('Packaged the approved-source menu/HUD glyphs.');})().catch(e=>{console.error(e);process.exitCode=1});
