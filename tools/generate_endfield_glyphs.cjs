// Reuse the exact approved lab/library SVG paths with a consistent optical box.
const fs=require('fs'),path=require('path'),crypto=require('crypto');
const sharp=require('C:/Users/j9010/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/sharp');
const root=path.resolve(__dirname,'..'),source=path.join(root,'design/hud/endfield-assets/icons');
const names={camera:'camera',more:'ellipsis',pause:'pause',play:'play',cpu:'cpu',reset:'rotate-ccw',eye:'eye',skull:'skull',swords:'swords',coin:'coins',list:'list',clock:'clock',range:'scan-line',recall:'house',check:'check',unavailable:'circle-alert'};
(async()=>{const files={},sources={};for(const [name,sourceName] of Object.entries(names)){
 const raw=fs.readFileSync(path.join(source,sourceName+'.svg'),'utf8');
 // White masks let the native renderer apply the exact warm/accent tint.
 const svg=raw.replaceAll('currentColor','#ffffff').replace('stroke-linecap="round"','stroke-linecap="square"').replace('stroke-linejoin="round"','stroke-linejoin="miter"');
 const file=path.join(root,'probe/ui/ef_'+name+'.png');await sharp(Buffer.from(svg)).resize(32,32).png().toFile(file);
 files['ui/ef_'+name+'.png']=crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');sources[sourceName]=crypto.createHash('sha256').update(raw).digest('hex');
}fs.copyFileSync(path.join(source,'LICENSE'),path.join(root,'probe/ui/Endfield-icons-LICENSE.txt'));
fs.writeFileSync(path.join(root,'research/endfield-glyphs-0.45.0.json'),JSON.stringify({files,sources},null,2)+'\n');console.log('Packaged 16 approved-source menu/HUD glyphs.');})().catch(e=>{console.error(e);process.exitCode=1});
