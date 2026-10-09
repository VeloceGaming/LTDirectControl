"""Original 9x9 stat icons for stats the game has no icon for.

Same rules as the game's champion_stat_icon sheet: 9x9 pixels, one flat
colour, no outline, transparent background. Nothing is copied from game
art. Writes one PNG per icon plus a review page (design/hud/stat-icons/),
and the packaged sprite sheet probe/ui/stat_icons#sheet.png +
stat_icons#data.sprite_sheet (frames "<name>_0"; inline text images need a
sheet and a frame) with their digests in tools/records/ui-graphics.json.
Edit a grid below and re-run: python -I tools/generate_stat_icons.py
"""
import hashlib
import json
import struct
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'design' / 'hud' / 'stat-icons'
PACKAGED = ROOT / 'probe' / 'ui'
RECORD = ROOT / 'tools' / 'records' / 'ui-graphics.json'

# name: (colour, label, 9 rows of 9: X = pixel, . = transparent)
ICONS = {
    'crit': ('ffd23f', 'Critical strike chance', [
        '....X....',
        '.X..X..X.',
        '..XXXXX..',
        '..XX.XX..',
        'XXX...XXX',
        '..XX.XX..',
        '..XXXXX..',
        '.X..X..X.',
        '....X....',
    ]),
    'armor_pen': ('ff9028', 'Armor penetration', [
        '.XXXXXX..',
        'X.....X.X',
        'X.XXX.XX.',
        'X.XX.XX.X',
        'X.X.XX..X',
        '.X.XX..X.',
        '.XXX..X..',
        '.X..XX...',
        'X........',
    ]),
    'mr_pen': ('a974ff', 'Magic penetration', [
        '...XXX..X',
        '..X...XX.',
        '.X.XXXX..',
        'X.XXXX..X',
        'X.XXX.X.X',
        'X.XX.XX.X',
        '.XX.XX.X.',
        '.X.X..X..',
        'X..XXX...',
    ]),
    'hp_regen': ('6aff55', 'Health regeneration', [
        '.........',
        '.......X.',
        '..X...XXX',
        '..X....X.',
        'XXXXX..X.',
        '..X....X.',
        '..X....X.',
        '.......X.',
        '.........',
    ]),
    'shield': ('e0e0e0', 'Shield', [
        '.XXXXXXX.',
        'XX.XXXXXX',
        'XX.XXXXXX',
        'XX.XXXXXX',
        'XXX.XXXXX',
        '.XXXXXXX.',
        '..XXXXX..',
        '...XXX...',
        '....X....',
    ]),
    'lifesteal': ('ff5c6c', 'Lifesteal', [
        '....X....',
        '....X....',
        '...XXX...',
        '...XXX...',
        '..XXXXX..',
        '.XXXXXXX.',
        '.X.XXXXX.',
        '.XX.XXXX.',
        '..XXXXX..',
    ]),
    'haste': ('40e0d0', 'Ability haste', [
        '.XXXXXXX.',
        '..X...X..',
        '..XX.XX..',
        '...XXX...',
        '....X....',
        '...X.X...',
        '..X.X.X..',
        '..XXXXX..',
        '.XXXXXXX.',
    ]),
}


def rgba(rows, colour):
    r, g, b = (int(colour[i:i + 2], 16) for i in (0, 2, 4))
    return [[(r, g, b, 255) if c == 'X' else (0, 0, 0, 0) for c in row] for row in rows]


def write(path, pixels):
    raw = b''.join(b'\x00' + b''.join(bytes(p) for p in row) for row in pixels)

    def chunk(kind, data):
        return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data) & 0xffffffff)

    path.write_bytes(b'\x89PNG\r\n\x1a\n'
                     + chunk(b'IHDR', struct.pack('>IIBBBBB', len(pixels[0]), len(pixels), 8, 6, 0, 0, 0))
                     + chunk(b'IDAT', zlib.compress(raw, 9))
                     + chunk(b'IEND', b''))


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    record = json.loads(RECORD.read_text(encoding='utf-8'))
    # The packaged sheet: one 10 px slot per icon (a transparent column
    # between frames), frames named "<name>_0" like the game's sheets.
    slot, count = 10, len(ICONS)
    sheet = [[(0, 0, 0, 0)] * (slot * count) for _ in range(9)]
    frames = {}
    for i, (name, (colour, _, rows)) in enumerate(ICONS.items()):
        assert len(rows) == 9 and all(len(r) == 9 for r in rows), name
        pixels = rgba(rows, colour)
        write(OUT / f'{name}.png', pixels)
        for y, row in enumerate(pixels):
            sheet[y][i * slot:i * slot + 9] = row
        frames[f'{name}_0'] = {'x': i * slot / (slot * count), 'y': 0.0,
                               'w': 9 / (slot * count), 'h': 1.0}
    write(PACKAGED / 'stat_icons#sheet.png', sheet)
    (PACKAGED / 'stat_icons#data.sprite_sheet').write_text(json.dumps({'images': frames}), encoding='utf-8')
    for old in [k for k in record['files'] if k.startswith('ui/stat_') and 'stat_icons#' not in k]:
        del record['files'][old]
        (ROOT / 'probe' / old).unlink(missing_ok=True)
    for name in ['stat_icons#sheet.png', 'stat_icons#data.sprite_sheet']:
        record['files'][f'ui/{name}'] = hashlib.sha256((PACKAGED / name).read_bytes()).hexdigest()
    RECORD.write_text(json.dumps(record, indent=2) + '\n', encoding='utf-8')
    tiles = '\n'.join(
        f'<figure><img src="{n}.png" alt=""><img class="big" src="{n}.png" alt=""><figcaption>{label}<small>{n} · #{c}</small></figcaption></figure>'
        for n, (c, label, _) in ICONS.items()
    )
    (OUT / 'index.html').write_text(f"""<!doctype html>
<meta charset="utf-8"><title>Stat icons review</title>
<style>
body {{ background: #1c1a18; color: #eeecec; font: 14px system-ui, sans-serif; margin: 24px; }}
h1 {{ font-size: 18px; }} p {{ color: #989694; max-width: 70ch; }}
.grid {{ display: grid; grid-template-columns: repeat(auto-fill, minmax(180px, 1fr)); gap: 16px; }}
figure {{ margin: 0; padding: 12px; background: #242220; border: 1px solid #4b4a49; }}
img {{ image-rendering: pixelated; display: inline-block; vertical-align: middle; margin-right: 12px; }}
img.big {{ width: 72px; height: 72px; }}
figcaption {{ margin-top: 8px; }} small {{ display: block; color: #989694; }}
</style>
<h1>New 9×9 stat icons (review)</h1>
<p>Same rules as the game's champion_stat_icon sheet: 9×9 pixels, one flat colour, no outline.
Each icon is shown at native size and at 8×.</p>
<div class="grid">{tiles}</div>
""", encoding='utf-8')
    print(f'wrote {len(ICONS)} icons to {OUT}')


if __name__ == '__main__':
    main()
