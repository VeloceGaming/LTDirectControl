// Settings window and strip glyphs from Lucide sources (ISC) in design/hud/endfield-assets/icons.
// White masks: the native renderer applies the exact tint. Fails if any visible pixel is not white.
const fs=require('fs'),path=require('path'),crypto=require('crypto');
const sharp=require('C:/Users/j9010/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/sharp');
const root=path.resolve(__dirname,'..'),source=path.join(root,'design/hud/endfield-assets/icons');
const names={keyboard:'keyboard',panels:'panels',settings:'settings',x:'x',chevron_down:'chevron-down',chevron_up:'chevron-up',chevron_right:'chevron-right'};
(async()=>{const files={},sources={};
for(const [name,sourceName] of Object.entries(names)){
 const raw=fs.readFileSync(path.join(source,sourceName+'.svg'),'utf8');
 // keyboard/panels are the approved preview's own navigation icons; the rest must be Lucide.
 if(!['keyboard','panels'].includes(sourceName)&&!raw.includes('lucide'))throw Error(sourceName+' is not a Lucide source icon');
 const svg=raw.replaceAll('currentColor','#ffffff');
 const output=path.join(root,'probe/ui','ef_'+name+'.png');
 await sharp(Buffer.from(svg)).resize(32,32).png().toFile(output);
 const {data}=await sharp(output).ensureAlpha().raw().toBuffer({resolveWithObject:true});
 let visible=0;for(let i=0;i<data.length;i+=4){if(data[i+3]){visible++;if(data[i]!==255||data[i+1]!==255||data[i+2]!==255)throw Error(name+' has a non-white pixel');}}
 if(!visible)throw Error(name+' is empty');
 files['ui/ef_'+name+'.png']=crypto.createHash('sha256').update(fs.readFileSync(output)).digest('hex');
 sources[sourceName]=crypto.createHash('sha256').update(raw).digest('hex');
}
fs.writeFileSync(path.join(root,'research/settings-glyphs-0.53.0.json'),JSON.stringify({files,sources,palette:'white-only masks verified at generation'},null,2)+'\n');
console.log('Generated '+Object.keys(files).length+' Lucide settings/strip glyphs (white-only verified).');})().catch(e=>{console.error(e);process.exit(1)});
