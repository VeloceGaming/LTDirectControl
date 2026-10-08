"""Read original installed artwork for a local-only code-native HUD preview."""
from pathlib import Path
from PIL import Image
import io, json, struct, base64
out = Path(r'C:\Users\j9010\.codex\visualizations\2026\09\30\01a0f24c-b767-73b0-b835-26937b07649e')
game = Path(r'C:\Program Files (x86)\Steam\steamapps\common\Teamfight Manager2')
names=['lancer','exorcist','nightmare','circus_blade','archangel','boomerang_hunter','gunner','berserker','ogre','illusionist']
entries={}
with (game/'bundle.game_data').open('rb') as f:
    def integer():return struct.unpack('<I',f.read(4))[0]
    for _ in range(integer()):
        ext=f.read(integer()).decode();path=f.read(integer()).decode();n=integer()
        relevant = any(path.endswith('/'+name+'#'+suffix) for name in names for suffix in ['sheet','anim']) or 'UI_aseprite/skill_icon#' in path or 'ingame/item_icons_18x18#' in path
        if relevant and ext in ['png','sprite_sheet','fanim']:entries[(ext,path)]=f.read(n)
        else:f.seek(n,1)
def uri(im,fmt='PNG'):
    b=io.BytesIO();im.save(b,format=fmt,**({'quality':65} if fmt=='JPEG' else {}));return 'data:image/'+fmt.lower()+';base64,'+base64.b64encode(b.getvalue()).decode()
assets={'portraits':{},'skills':{}, 'items':[]}
for name in names:
    path='asset/base/aseprite_resources/champions/'+name
    if ('png',path+'#sheet') not in entries:continue
    animation=json.loads(entries[('fanim',path+'#anim')])['anims']['idle']['frames'][0]['data']
    x,y,w,h=[round(animation[k]) for k in ['x','y','w','h']]
    im=Image.open(io.BytesIO(entries[('png',path+'#sheet')])).crop((x,y,x+w,y+h))
    assets['portraits'][name]=uri(im)
def sheet_images(path,tags):
    im=Image.open(io.BytesIO(entries[('png',path+'#sheet')]))
    metadata=json.loads(entries[('sprite_sheet',path+'#data')])['images']
    result=[]
    for tag in tags:
        r=metadata[tag];x,y,w,h=[r[k]*im.size[i%2] for i,k in enumerate(['x','y','w','h'])]
        result.append(uri(im.crop(tuple(round(v) for v in (x,y,x+w,y+h)))))
    return result
for name in ['circus_blade','exorcist','lancer','nightmare','boomerang_hunter']:
    assets['skills'][name]=sheet_images('asset/base/aseprite_resources/UI_aseprite/skill_icon',[name+'_'+str(i) for i in range(3)])
assets['items']=sheet_images('asset/base/aseprite_resources/ingame/item_icons_18x18',['t1_0','t3_0','t4_0'])
# Compression for embedding only; the reference file is never modified.
reference=Image.open(r'C:\Users\j9010\AppData\Local\Temp\codex-clipboard-2a30b751-ea94-41d3-a189-adcf6c16ea48.png').convert('RGB')
assets['reference']=uri(reference,'JPEG')
(out/'hud-preview-assets.json').write_text(json.dumps(assets),encoding='utf-8')
print({k:(list(v) if isinstance(v,dict) else len(v)) for k,v in assets.items()})
