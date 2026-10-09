// Original code-native vector faces, rasterized for the existing native image
// runner. No reference art. Run with LT_SHARP_MODULE or the bundled runtime.
const fs = require('fs'), path = require('path'), crypto = require('crypto');
const sharp = require(process.env.LT_SHARP_MODULE || 'C:/Users/j9010/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/sharp');
const root = path.resolve(__dirname, '..');
const hash = b => crypto.createHash('sha256').update(b).digest('hex');
const ink = '#201d19', ivory = '#fff6dd', yellow = '#fdee00';
const line = d => `<path d="${d}" fill="none" stroke="${ink}" stroke-width="5" stroke-linecap="round" stroke-linejoin="round"/>`;
const star = (x,y,s) => `<path d="M${x} ${y-s}l${s*.28} ${s*.72}L${x+s} ${y}l-${s*.72} ${s*.28}L${x} ${y+s}l-${s*.28} -${s*.72}L${x-s} ${y}l${s*.72} -${s*.28}Z" fill="${ivory}" stroke="${ink}" stroke-width="2"/>`;
const face = content => `<path d="M30 33L43 22h44l14 12 6 48-18 19H42L22 83Z" fill="#000" opacity=".4" transform="translate(0 5)"/><path d="M30 29L43 18h44l14 12 6 48-18 19H42L22 79Z" fill="${ink}"/><path d="M34 32l12-9h38l12 10 5 43-15 15H45L28 76Z" fill="${yellow}"/><path d="M39 29h47l8 8H34Z" fill="${ivory}" opacity=".65"/>${content}`;
const faces = {
  gg: face(line('M41 48l6-6 7 6M73 48l6-6 7 6') + `<path d="M42 63h43l-7 17H50Z" fill="${ink}"/><path d="M47 65h33v6H47Z" fill="${ivory}"/>`) + star(108,22,12) + `<path d="M14 99h54v21H14Z" fill="${ink}"/><text x="41" y="115" font-family="sans-serif" font-weight="900" font-size="18" text-anchor="middle" fill="${ivory}">GG</text>`,
  nice: face(line('M41 48l7-5 8 5M74 48l7-5 7 5M46 67q19 18 37-1') + `<path d="M95 73l7-13q3-5 6-1l-1 14h9q5 0 4 6l-5 22H94l-6-6 1-18Z" fill="${ivory}" stroke="${ink}" stroke-width="4"/>`) + star(15,24,11),
  hype: face(`<path d="M37 42l10 3 7-8-1 12 9 6-12 1-5 10-3-12-11-3 10-4ZM73 43l10 3 7-8-1 12 9 6-12 1-5 10-3-12-11-3 10-4Z" fill="${ink}"/><path d="M48 69h32l-5 14H54Z" fill="${ink}"/>`) + star(108,18,13) + `<path d="M15 42L8 23M19 19l-1-11M111 96l10 12" stroke="${ivory}" stroke-width="5" stroke-linecap="round"/>`,
  oops: face(line('M40 41l15 5M74 46l15-5') + `<ellipse cx="49" cy="54" rx="4" ry="6" fill="${ink}"/><ellipse cx="81" cy="54" rx="4" ry="6" fill="${ink}"/><ellipse cx="65" cy="77" rx="7" ry="9" fill="${ink}"/><path d="M105 31q18 21 7 28-16 3-7-28Z" fill="${ivory}" stroke="${ink}" stroke-width="3"/>`),
  focus: face(`<path d="M32 44h28v18H34ZM69 44h28l-2 18H69Z" fill="${ink}"/><path d="M60 49h9" stroke="${ink}" stroke-width="5"/><path d="M38 48h16v5H38ZM75 48h16v5H75Z" fill="${ivory}"/>` + line('M55 76h22')) + `<path d="M11 37V15h22M95 15h22v22M117 91v22H95M33 113H11V91" fill="none" stroke="${ivory}" stroke-width="4"/>`,
};
(async () => {
  const files = {}, sheets = [];
  const design = path.join(root, 'design/hud/emotes'); fs.mkdirSync(design, { recursive:true });
  for (const [name, content] of Object.entries(faces)) {
    const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="128" height="128" viewBox="0 0 128 128">${content}</svg>`;
    fs.writeFileSync(path.join(design, `${name}.svg`), svg);
    const output = `ui/emote_${name}.png`;
    await sharp(Buffer.from(svg)).png().toFile(path.join(root, 'probe', output));
    const {data, info} = await sharp(path.join(root,'probe',output)).ensureAlpha().raw().toBuffer({resolveWithObject:true});
    if (info.width!==128 || info.height!==128 || !data.some((v,i)=>i%4===3 && v===0)) throw Error('Bad emote raster');
    files[output] = hash(fs.readFileSync(path.join(root,'probe',output)));
    sheets.push({input: path.join(root,'probe',output),left:sheets.length*160+16,top:16});
  }
  await sharp({create:{width:800,height:160,channels:4,background:'#1c1a18'}}).composite(sheets).png().toFile(path.join(design,'sheet.png'));
  // A short two-note confirmation, original PCM, no loop or external sound.
  const rate=24000, samples=Math.round(rate*.16), wav=Buffer.alloc(44+samples*2);
  wav.write('RIFF'); wav.writeUInt32LE(wav.length-8,4); wav.write('WAVEfmt ',8); wav.writeUInt32LE(16,16);
  wav.writeUInt16LE(1,20); wav.writeUInt16LE(1,22); wav.writeUInt32LE(rate,24); wav.writeUInt32LE(rate*2,28);
  wav.writeUInt16LE(2,32); wav.writeUInt16LE(16,34); wav.write('data',36); wav.writeUInt32LE(samples*2,40);
  for(let i=0;i<samples;i++) { const t=i/rate, freq=t<.075?660:880;
    const envelope=Math.min(t/.008,1)*Math.min((.16-t)/.04,1);
    wav.writeInt16LE(Math.round(Math.sin(2*Math.PI*freq*t)*envelope*4500),44+i*2); }
  const sound='sound/sfx/emote.wav'; fs.mkdirSync(path.dirname(path.join(root,'probe',sound)),{recursive:true});
  fs.writeFileSync(path.join(root,'probe',sound),wav); files[sound]=hash(wav);
  const record={source:'tools/generate_emotes.cjs',source_sha256:hash(fs.readFileSync(__filename)),files,
    art:'Five original vector faces: GG, Nice, Hype, Oops, Focus. No third-party reference art.',
    sound:'Original 160ms mono 24kHz 16-bit PCM two-note confirmation. Optional, off by default.'};
  fs.writeFileSync(path.join(root,'tools/records/emotes.json'),JSON.stringify(record,null,2)+'\n');
  const manifest=path.join(root,'tools/records/ui-graphics.json'), m=JSON.parse(fs.readFileSync(manifest));
  Object.assign(m.files, files); fs.writeFileSync(manifest,JSON.stringify(m,null,2)+'\n');
  console.log('Generated five original 128px emotes, contact sheet and optional PCM sound.');
})().catch(e=>{console.error(e);process.exit(1);});
