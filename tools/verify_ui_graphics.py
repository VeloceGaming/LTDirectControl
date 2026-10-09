"""Check original packaged glyphs, their palette, and file fingerprints."""
import hashlib,json
from pathlib import Path
from PIL import Image
root=Path(__file__).resolve().parents[1]
package=root/'dist/lt_direct_control'
version=json.loads((package/'mod.mod_info').read_text(encoding='utf-8'))['version']
files=sorted((package/'ui').glob('*.png'))
assert len(files)==74,len(files)
fingerprints={}
for file in files:
    im=Image.open(file).convert('RGBA');assert im.size==((64,64) if file.stem=='cursor_preview' else (32,32))
    colors={(r,g,b) for r,g,b,a in im.get_flattened_data() if a}
    assert colors
    if file.stem=='cursor_preview':
        assert (253,238,0) in colors and (238,238,238) in colors,file
        assert Image.open(file).getextrema()[3][0]==0,file
    elif file.stem.startswith('ef_'):assert colors=={(255,255,255)},file
    elif file.stem=='hud_coin':assert (253,238,0) in colors and (28,26,24) in colors,file
    elif file.stem.endswith('_yellow'):assert colors=={(253,238,0)},file
    elif file.stem.endswith('_dark'):assert colors=={(20,20,20)},file
    else:assert all(r==g==b for r,g,b in colors),file
    source=root/'probe/ui'/file.name
    assert source.read_bytes()==file.read_bytes(),file
    fingerprints['ui/'+file.name]=hashlib.sha256(file.read_bytes()).hexdigest()
for folder in ['ui','font']:
    for file in sorted((package/folder).glob('*')):
        assert file.is_file(),file
        source=root/'probe'/folder/file.name
        assert source.read_bytes()==file.read_bytes(),file
        fingerprints[folder+'/'+file.name]=hashlib.sha256(file.read_bytes()).hexdigest()
assert len(fingerprints)==82,len(fingerprints)
record={'version':version,'files':fingerprints,'palette':'white Endfield SVG masks tinted by native UI; previous cursor/glyph art retained',
        'rendering':'source paths, palette and typography assets verified; native rendering pending user test'}
(root/'tools/records/ui-graphics.json').write_text(json.dumps(record,indent=2)+'\n',encoding='utf-8')
print(f'Verified {len(files)} original packaged glyphs: dimensions, palette, fingerprints.')
