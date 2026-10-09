// Render a code-native SVG wrapper around the user's unchanged transparent PNG.
// Match the existing UI PNG pipeline; do not tint or redraw the supplied art.
const fs = require('fs'), path = require('path'), crypto = require('crypto');
const sharp = require(process.env.LT_SHARP_MODULE || 'C:/Users/j9010/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/sharp');
const root = path.resolve(__dirname, '..');
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');

(async () => {
  const source = 'Nerdge_Stamp_Transparent.png';
  const original = fs.readFileSync(path.join(root, source));
  const metadata = await sharp(original).metadata();
  if (!metadata.hasAlpha) throw Error('The shop stamp must have transparency.');
  // 56 inside 64 leaves enough room for a rotated square (62.83 units at 7.5°).
  const angle = -7.5;
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="128" height="128" viewBox="0 0 64 64"><image x="4" y="4" width="56" height="56" transform="rotate(${angle} 32 32)" xlink:href="data:image/png;base64,${original.toString('base64')}"/></svg>`;
  const file = 'ui/nerdge_stamp.png';
  const output = path.join(root, 'probe', file);
  await sharp(Buffer.from(svg)).png().toFile(output);
  const { data, info } = await sharp(output).ensureAlpha().raw().toBuffer({ resolveWithObject: true });
  if (info.width !== 128 || info.height !== 128) throw Error('Unexpected stamp size.');
  for (let y = 0; y < 128; y++) for (let x = 0; x < 128; x++) {
    if ((x === 0 || y === 0 || x === 127 || y === 127) && data[(y * 128 + x) * 4 + 3]) throw Error('Stamp clips the canvas edge.');
  }
  const files = { [file]: hash(fs.readFileSync(output)) };
  fs.writeFileSync(path.join(root, 'tools/records/shop-stamp.json'), JSON.stringify({
    source, source_sha256: hash(original), rotation_degrees_clockwise: angle,
    display_size: [64, 64], raster_size: [128, 128], wrapper_sha256: hash(svg), files,
    rendering: 'Original PNG embedded unchanged in a centred SVG wrapper; rendered to PNG for the existing native image runner.'
  }, null, 2) + '\n');
  const manifestPath = path.join(root, 'tools/records/ui-graphics.json');
  const manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
  Object.assign(manifest.files, files);
  manifest.palette = 'Endfield masks tinted by native UI; full-colour user shop stamp; previous cursor/glyph art retained';
  fs.writeFileSync(manifestPath, JSON.stringify(manifest, null, 2) + '\n');
  console.log('Generated 128px shop stamp for 64px display, tilted 7.5 degrees counter-clockwise; transparent edges verified.');
})().catch(error => { console.error(error); process.exit(1); });
