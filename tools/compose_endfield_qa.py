"""Normalize UI lab and HUD component evidence in one comparison image."""
from pathlib import Path
from PIL import Image, ImageDraw

root = Path(__file__).resolve().parents[1]
out = root / 'research/endfield-preview'
source = Image.open(root / 'research/endfield-audit-tooltip.png').convert('RGB')
new = Image.open(out / 'new-tooltip.png').convert('RGB')
canvas = Image.new('RGB', (1106, max(source.height, new.height) + 72), '#1c1a18')
draw = ImageDraw.Draw(canvas)
draw.text((16, 16), 'FINAL UI LAB / source component (529 px)', fill='white')
draw.text((561, 16), 'NEW HUD / same type, palette, density (529 px)', fill='white')
canvas.paste(source, (16, 48))
canvas.paste(new, (561, 48))
canvas.save(out / 'source-new-tooltip.png')
print('Source:', source.size, 'HUD:', new.size, 'Both device scale 1, no resampling.')
