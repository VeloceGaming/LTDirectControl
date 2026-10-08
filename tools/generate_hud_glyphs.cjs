// Deterministically rasterize the reusable SVGs supplied in the approved HUD.
const fs=require('fs'),path=require('path'),crypto=require('crypto');
const sharp=require('C:/Users/j9010/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/sharp');
const root=path.resolve(__dirname,'..');
const version=process.argv[2]||'0.41.0';
const source='design/hud/index.html';
const html=fs.readFileSync(path.join(root,source),'utf8');
const block=html.slice(html.indexOf('const I={'),html.indexOf('const SK='));
const names=['pause','more','swords','minion','list','coin','recall','skull'];
const files={};
(async()=>{
 for(const name of names){
  const match=block.match(new RegExp('\\b'+name+":'([^']+)'"));
  if(!match)throw new Error('Missing supplied glyph '+name);
  const svg=match[1].replace('<svg ','<svg xmlns="http://www.w3.org/2000/svg" ')
   .replaceAll('currentColor','#eeeeee').replaceAll('#1c1a18',version==='0.41.0'?'#1c1c1c':'#1c1a18');
  const file=path.join(root,'probe/ui/hud_'+name+'.png');
  await sharp(Buffer.from(svg)).resize(32,32).png().toFile(file);
  files['ui/hud_'+name+'.png']=crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
 }
 fs.writeFileSync(path.join(root,'research/hud-glyphs-'+version+'.json'),JSON.stringify({source,source_sha256:crypto.createHash('sha256').update(html).digest('hex'),files},null,2)+'\n');
 console.log('Rendered '+names.length+' supplied SVG HUD glyphs.');
})().catch(e=>{console.error(e);process.exitCode=1});
