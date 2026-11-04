// Rasterise SVG sources to PNG files with sharp (1.7.1-dev14, the ping
// assets). Called by tools/generate_ping_assets.py with a JSON job list:
//     node tools/rasterise_svg.cjs jobs.json
// Each job is {"svg": "<svg ...>", "out": "path.png", "width": n, "height": n}.
// The SVG's own width and height are the PNG's pixel size.
const fs = require('fs');
const sharp = require('C:/Users/j9010/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/sharp');

(async () => {
  const jobs = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
  for (const job of jobs) {
    const png = await sharp(Buffer.from(job.svg)).png({ compressionLevel: 9 }).toBuffer();
    const meta = await sharp(png).metadata();
    if (meta.width !== job.width || meta.height !== job.height) {
      throw new Error(`${job.out}: ${meta.width}x${meta.height}, wanted ${job.width}x${job.height}`);
    }
    fs.writeFileSync(job.out, png);
  }
  console.log(`${jobs.length} PNG files written`);
})().catch((e) => { console.error(e); process.exit(1); });
