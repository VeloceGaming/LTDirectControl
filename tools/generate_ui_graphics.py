"""Original monochrome UI glyphs. No game/reference artwork is copied."""
from pathlib import Path
from PIL import Image, ImageDraw

root = Path(__file__).resolve().parents[1]
out = root / 'probe/ui'
out.mkdir(exist_ok=True)
names = ['play', 'pause', 'controller', 'chip', 'camera', 'camera_lock',
         'target', 'coin', 'minion', 'lock', 'recall', 'hourglass']
sheet = Image.new('RGB', (len(names) * 64, 64), '#141414')
for i, name in enumerate(names):
    im = Image.new('RGBA', (128, 128)); d = ImageDraw.Draw(im)
    white = '#eeeeee'; line = lambda xy: d.line(xy, fill=white, width=8, joint='curve')
    if name == 'play': d.polygon([(40, 24), (104, 64), (40, 104)], fill=white)
    elif name == 'pause':
        for x in [32, 76]: d.rounded_rectangle((x, 26, x+20, 102), radius=3, fill=white)
    elif name == 'controller':
        d.rounded_rectangle((12, 36, 116, 96), radius=20, outline=white, width=7)
        line([(28, 64), (56, 64)]); line([(42, 50), (42, 78)])
        for x,y in [(83,57),(99,72)]: d.ellipse((x-5,y-5,x+5,y+5),fill=white)
    elif name == 'chip':
        d.rounded_rectangle((32,32,96,96),radius=7,outline=white,width=7)
        d.rectangle((48,48,80,80),outline=white,width=6)
        for n in [44,64,84]:
            for pts in [[(n,16),(n,32)],[(n,96),(n,112)],[(16,n),(32,n)],[(96,n),(112,n)]]: line(pts)
    elif name in ['camera','camera_lock']:
        d.rounded_rectangle((12,38,88,94),radius=9,outline=white,width=7)
        d.polygon([(88,53),(115,38),(115,94),(88,79)],outline=white,width=6)
        if name == 'camera_lock':
            d.arc((39,9,67,42),180,360,fill=white,width=6)
            d.rounded_rectangle((35,24,72,48),radius=3,fill=white)
    elif name == 'target':
        d.ellipse((22,22,106,106),outline=white,width=6)
        for pts in [[(64,8),(64,29)],[(64,99),(64,120)],[(8,64),(29,64)],[(99,64),(120,64)]]: line(pts)
        d.ellipse((53,40,75,62),fill=white); d.rounded_rectangle((45,66,83,88),radius=10,fill=white)
    elif name == 'coin':
        d.ellipse((22,16,106,112),outline=white,width=8)
        d.ellipse((42,24,86,104),outline=white,width=5)
    elif name == 'minion':
        d.rounded_rectangle((35,16,93,63),radius=12,outline=white,width=7)
        line([(42,47),(86,47)]); d.polygon([(35,73),(93,73),(105,108),(23,108)],fill=white)
    elif name == 'lock':
        d.arc((36,12,92,78),180,360,fill=white,width=8)
        d.rounded_rectangle((26,47,102,113),radius=9,outline=white,width=8)
        d.ellipse((58,68,70,80),fill=white); line([(64,76),(64,91)])
    elif name == 'recall':
        d.arc((20,20,108,108),35,310,fill=white,width=8)
        d.polygon([(92,15),(111,46),(78,45)],fill=white)
        d.polygon([(43,74),(64,54),(85,74)],outline=white,width=6); line([(49,72),(49,94),(79,94),(79,72)])
    else:
        line([(30,18),(98,18)]); line([(30,110),(98,110)])
        line([(38,20),(38,38),(90,90),(90,108)]); line([(90,20),(90,38),(38,90),(38,108)])
    im = im.resize((32,32), Image.Resampling.LANCZOS)
    im.save(out / f'{name}.png')
    for suffix,color in [('_yellow',(255,215,0)),('_dark',(20,20,20))]:
        variant=Image.new('RGBA',im.size,color+(0,));variant.putalpha(im.getchannel('A'))
        variant.save(out / f'{name}{suffix}.png')
    sheet.paste(im.resize((40,40)), (i*64+12,12), im.resize((40,40)))
sheet.save(root/'research/ui-glyphs-0.26.0.png')
print(f'Generated {len(names)} original grayscale glyphs.')
