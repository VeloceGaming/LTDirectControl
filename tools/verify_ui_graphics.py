"""Check original packaged glyphs, their palette, and file fingerprints."""
import hashlib,json
from pathlib import Path
from PIL import Image
root=Path(__file__).resolve().parents[1]
package=root/'dist/lt_direct_control_probe'
version=json.loads((package/'mod.mod_info').read_text(encoding='utf-8'))['version']
files=sorted((package/'ui').glob('*.png'))
assert len(files)==36,len(files)
fingerprints={}
for file in files:
    im=Image.open(file).convert('RGBA');assert im.size==(32,32)
    colors={(r,g,b) for r,g,b,a in im.get_flattened_data() if a}
    assert colors
    if file.stem.endswith('_yellow'):assert colors=={(255,215,0)},file
    elif file.stem.endswith('_dark'):assert colors=={(20,20,20)},file
    else:assert all(r==g==b for r,g,b in colors),file
    source=root/'probe/ui'/file.name
    assert source.read_bytes()==file.read_bytes(),file
    fingerprints['ui/'+file.name]=hashlib.sha256(file.read_bytes()).hexdigest()
record={'version':version,'files':fingerprints,'palette':'grayscale glyphs; exact yellow active variants',
        'rendering':'glyph contact sheet inspected; native game rendering pending'}
(root/f'research/ui-graphics-{version}.json').write_text(json.dumps(record,indent=2)+'\n',encoding='utf-8')
print(f'Verified {len(files)} original packaged glyphs: dimensions, palette, fingerprints.')
