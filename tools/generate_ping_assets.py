"""Make the ping system's PNG assets (1.7.1-dev17) from the author's design.

    python tools/generate_ping_assets.py

Reads design/pings/glyphs-pixel.svg and glyphs-pixel-extra.svg (the
author's pixel glyphs; the extra sheet adds personal, team, assist me and
poke), never writes them, and writes
probe/ui/pings/*.png at exactly the size the mod shows them (UI pixels,
1920 x 1080), colours baked in. Writes tools/records/ping-assets.json
(every file's SHA-256, its size, each glyph's colour, the sources'
digests) and the same file digests into tools/records/ui-graphics.json.

- **Glyphs.** Each glyph is 11 x 11 art pixels of one colour inside a
  13 x 13 grid with the dark #1c1a18 outline, drawn 6 screen pixels per art
  pixel on the sheet. The cells are read back from the sheet's rectangles
  (matched to their names by the labels under them, not by position) and
  written at four screen pixels per art pixel (52 px: the fan's slots),
  two (26 px: the pin, the answer stack) and one (13 px: the minimap pin),
  nearest-neighbour, so every pixel is exactly an art pixel's colour. The
  halves' markers (personal, team) are written at three (39 px) only. The
  "all in" glyph is the sheets' `all in` label when one has it, else the
  contest swords (no code change when the author adds it). The sheet's
  "greyed" glyph is not used (nothing greys since dev15).
- **The wheel** (1.7.1-dev17, design/pings/mock-wheel-2level.png): the
  ring 34..100 px in two halves split by the vertical axis (a 6 px gap),
  the hub 26 px, and the fan's 72 px slot square, in the emote wheel's
  materials: the flat fill #1c1a18 at 0xee over a 2 px frame #4b4a49, the
  soft shadow, the hovered one framed #fdee00 with a slightly lighter
  fill; rasterised with sharp (tools/rasterise_svg.cjs). Superseded: dev14
  to dev16's four sectors 62..148 px and the centre disc 57 px.
- **The pin.** A placeholder until the author draws one in this style: a
  dark box with a 1 art pixel #1c1a18 outline and a point under it, at two
  screen pixels per art pixel (34 x 40) and one (17 x 20, the minimap);
  the glyph is its own image node over it.

Superseded 2026-11-04 (1.7.1-dev15): dev14's 45 pictures from the
celestial sheets (glyphs-celestial-a.svg, mark-celestial.svg), rasterised
with sharp: the rim arc's notches, the glyphs and pins with their colour
baked in, the card's state glyphs. Removed.
"""
import hashlib
import json
import math
import re
import struct
import subprocess
import tempfile
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DESIGN = ROOT / 'design' / 'pings'
OUT = ROOT / 'probe' / 'ui' / 'pings'
RECORD = ROOT / 'tools' / 'records' / 'ping-assets.json'
UI_RECORD = ROOT / 'tools' / 'records' / 'ui-graphics.json'
SOURCES = ['glyphs-pixel.svg', 'glyphs-pixel-extra.svg']


def svg(width, height, view, body):
    return (f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" '
            f'viewBox="{view}">{body}</svg>')


# ---- Pixel art (written directly: nearest neighbour, no rasteriser) ----

OUTLINE = (0x1c, 0x1a, 0x18, 0xff)
GRID = 13  # cells per side, the outline included
CELL = 6  # screen pixels per art pixel on the design sheet
# The sheet's labels by glyph name (the label under each glyph).
LABELS = {'go': 'go', 'defend': 'defend', 'enemy missing': 'missing',
          'take objective': 'take', 'contest': 'contest', 'leave (retreat)': 'leave',
          'push': 'push', "I'm coming": 'coming', 'coming / accepted': 'accepted',
          'busy': 'busy', 'declined': 'declined', 'personal (me)': 'personal',
          'team (everyone)': 'team', 'assist me': 'assist', 'poke (range)': 'poke',
          'all in': 'allin'}
# Every glyph the mod draws at the three sizes, in the order the record
# lists them, and the markers drawn at 39 px only.
GLYPHS = ['go', 'defend', 'missing', 'take', 'contest', 'leave', 'push', 'coming',
          'accepted', 'busy', 'declined', 'assist', 'poke', 'allin']
MARKERS = ['personal', 'team']
# Pixel sizes: four, two and one screen pixels per art pixel; the markers
# three.
SCALES = {52: 4, 26: 2, 13: 1}
MARKER_SCALE = {39: 3}


def colour(text):
    text = text.lstrip('#')
    return tuple(int(text[i:i + 2], 16) for i in (0, 2, 4)) + (0xff,)


def png(width, height, pixels):
    """An RGBA PNG from rows of (r, g, b, a) tuples."""
    raw = b''.join(b'\0' + b''.join(bytes(p) for p in row) for row in pixels)
    def chunk(kind, data):
        body = kind + data
        return struct.pack('>I', len(data)) + body + struct.pack('>I', zlib.crc32(body) & 0xffffffff)
    head = struct.pack('>IIBBBBB', width, height, 8, 6, 0, 0, 0)
    return (b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', head) + chunk(b'IDAT', zlib.compress(raw, 9))
            + chunk(b'IEND', b''))


def scaled(grid, k):
    """A cell grid (rows of colours or None) at `k` screen pixels a cell."""
    clear = (0, 0, 0, 0)
    rows = []
    for row in grid:
        line = [c or clear for c in row for _ in range(k)]
        rows.extend([line] * k)
    return rows


RECT = re.compile(r'M(\d+),(\d+)h(\d+)v(\d+)h-(\d+)z')


def cells(group, origin_rows=0):
    """A 13 x 13 grid of colours from one glyph group's two paths."""
    grid = [[None] * GRID for _ in range(GRID)]
    for d, fill in re.findall(r'<path d="([^"]*)" fill="(#[0-9a-fA-F]{6})"', group):
        for x, y, w, h, back in RECT.findall(d):
            x, y, w, h = int(x), int(y), int(w), int(h)
            assert int(back) == w and x % CELL == 0 and y % CELL == 0 and w % CELL == 0 and h == CELL, d
            for cx in range(x // CELL, (x + w) // CELL):
                row = y // CELL + origin_rows
                assert grid[row][cx] is None, ('overlap', group[:60])
                grid[row][cx] = colour(fill)
    return grid


def glyph_groups(sheet):
    """Each named glyph's group: the groups drawn at 6x (translate, then
    shape-rendering) matched to the label centred under them."""
    groups = re.findall(r'(<g transform="translate\((\d+),(\d+)\)" shape-rendering="crispEdges">.*?</g>)', sheet)
    labels = {(int(float(x)), int(float(y))): t for x, y, t in
              re.findall(r'<text x="([\d.]+)" y="([\d.]+)"[^>]*>([^<]*)</text>', sheet)}
    out = {}
    for group, gx, gy in groups:
        gx, gy = int(gx), int(gy)
        label = labels.get((gx + GRID * CELL // 2, gy + 94))
        if label in LABELS:
            out[LABELS[label]] = group
    return out


def glyph_grids():
    grids = {}
    for source in SOURCES:
        sheet = (DESIGN / source).read_text(encoding='utf-8')
        for name, group in glyph_groups(sheet).items():
            assert name not in grids, ('two sheets name', name)
            grids[name] = cells(group)
    # "all in" until the author's sheet has it: the contest swords.
    grids.setdefault('allin', grids['contest'])
    assert set(grids) == set(GLYPHS) | set(MARKERS), sorted((set(GLYPHS) | set(MARKERS)) ^ set(grids))
    return grids


def main_colour(grid):
    """The glyph's one colour besides the outline (asserted: exactly one)."""
    found = {c for row in grid for c in row if c}
    assert OUTLINE in found and len(found) == 2, found
    (main,) = found - {OUTLINE}
    return '#%02x%02x%02x' % main[:3]


def pin_grid(scale_cells=17):
    """The placeholder pin: a 17 x 17 box (outline, dark fill) and a 3-row
    point under its centre, 17 x 20 cells; the glyph's 13 cells sit inside
    at (2, 2)."""
    fill = (0x1a, 0x15, 0x10, 0xcc)
    w = scale_cells
    grid = [[None] * w for _ in range(20)]
    for y in range(17):
        for x in range(w):
            edge = x in (0, w - 1) or y in (0, 16)
            grid[y][x] = OUTLINE if edge else fill
    # The point: outlined, the box's bottom edge open above it.
    for x in (7, 8, 9):
        grid[16][x] = fill
    for y, (a, b) in zip((17, 18, 19), ((6, 10), (7, 9), (8, 8))):
        for x in range(a, b + 1):
            grid[y][x] = OUTLINE if x in (a, b) else fill
    return grid


# ---- The ring and the sub-ring (rasterised shapes) ----

RING_BOX = 220  # the halves and the hub: 220 x 220 centred on the wheel's centre
SLOT_BOX = 80  # a fan slot: the 72 px square and its shadow
FRAME, FILL, HOT_FRAME, HOT_FILL = '#4b4a49', '#1c1a18', '#fdee00', '#2c2925'
FILL_ALPHA = 0xee / 255
SHADOW = 'fill="#000000" fill-opacity="0.4"'  # the emote slot shadow, 0x66
GAP = 3.0  # each sector edge sits this far from its diagonal (a 6 px gap)
HALVES = {'right': (-90, 90), 'left': (90, 270)}


def point(c, r, degrees):
    a = math.radians(degrees)
    return c + r * math.cos(a), c + r * math.sin(a)


def band(c, r0, r1, a0, a1, inset=0.0):
    """An annular sector around (c, c) whose straight edges run GAP + inset
    px inside the rays at a0 and a1 (parallel to them), radii r0 + inset
    .. r1 - inset."""
    r0, r1 = r0 + inset, r1 - inset
    d = GAP + inset
    def edge(r, a, side):
        return a + side * math.degrees(math.asin(min(1.0, d / r)))
    o0, o1 = edge(r1, a0, 1), edge(r1, a1, -1)
    i0, i1 = edge(r0, a0, 1), edge(r0, a1, -1)
    (x0, y0), (x1, y1) = point(c, r1, o0), point(c, r1, o1)
    (x2, y2), (x3, y3) = point(c, r0, i1), point(c, r0, i0)
    large = 1 if (o1 - o0) > 180 else 0
    return (f'M{x0:.2f},{y0:.2f} A{r1:.2f},{r1:.2f} 0 {large} 1 {x1:.2f},{y1:.2f} '
            f'L{x2:.2f},{y2:.2f} A{r0:.2f},{r0:.2f} 0 {large} 0 {x3:.2f},{y3:.2f} Z')


def disc(c, r):
    return f'M{c - r:.2f},{c} A{r:.2f},{r:.2f} 0 1 1 {c + r:.2f},{c} A{r:.2f},{r:.2f} 0 1 1 {c - r:.2f},{c} Z'


def material(outer, inner, hot):
    """The emote slot's material: shadow, frame, fill inset by 2 px."""
    frame, fill = (HOT_FRAME, HOT_FILL) if hot else (FRAME, FILL)
    return (f'<path d="{outer}" {SHADOW} transform="translate(-1,3)"/>'
            f'<path d="{outer}" fill="{frame}"/>'
            f'<path d="{inner}" fill="{fill}" fill-opacity="{FILL_ALPHA:.3f}"/>')


def square_path(c, half):
    return f'M{c - half:.2f},{c - half:.2f} h{2 * half:.2f} v{2 * half:.2f} h{-2 * half:.2f} Z'


def ring_jobs():
    jobs = []
    def put(name, body, size):
        jobs.append((name, svg(size, size, f'0 0 {size} {size}', body), size))
    c = RING_BOX / 2
    for name, (a0, a1) in HALVES.items():
        for hot in (False, True):
            outer = band(c, 34, 100, a0, a1)
            inner = band(c, 34, 100, a0, a1, 2)
            put(f'half_{name}' + ('_hot' if hot else ''), material(outer, inner, hot), RING_BOX)
    put('hub', material(disc(c, 26), disc(c, 24), False), RING_BOX)
    c = SLOT_BOX / 2
    for hot in (False, True):
        put('slot' + ('_hot' if hot else ''), material(square_path(c, 36), square_path(c, 34), hot), SLOT_BOX)
    return jobs


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    for old in list(OUT.glob('*.svg')) + list(OUT.glob('*.png')):
        old.unlink()
    record = {'version': '1.7.1-dev17', 'sources': {}, 'sizes': {}, 'colours': {}, 'files': {}}
    # The pixel art.
    grids = glyph_grids()
    pixel = {}
    for name in GLYPHS + MARKERS:
        record['colours'][name] = main_colour(grids[name])
        for size, k in (SCALES if name in GLYPHS else MARKER_SCALE).items():
            pixel[f'glyph{size}_{name}'] = (scaled(grids[name], k), size, size)
    pin = pin_grid()
    pixel['pin'] = (scaled(pin, 2), 34, 40)
    pixel['pin_mini'] = (scaled(pin, 1), 17, 20)
    for name, (rows, w, h) in pixel.items():
        assert len(rows) == h and all(len(r) == w for r in rows), name
        (OUT / f'{name}.png').write_bytes(png(w, h, rows))
        record['sizes'][name] = [w, h]
    # The ring.
    jobs = ring_jobs()
    names = [j[0] for j in jobs] + list(pixel)
    assert len(names) == len(set(names)), 'duplicate asset names'
    batch = [{'svg': s, 'out': str(OUT / f'{n}.png'), 'width': size, 'height': size}
             for n, s, size in jobs]
    with tempfile.TemporaryDirectory() as folder:
        listing = Path(folder) / 'jobs.json'
        listing.write_text(json.dumps(batch), encoding='utf-8')
        subprocess.run(['node', str(ROOT / 'tools' / 'rasterise_svg.cjs'), str(listing)], check=True)
    for n, _, size in jobs:
        record['sizes'][n] = [size, size]
    record['sizes'] = dict(sorted(record['sizes'].items()))
    for source in SOURCES:
        record['sources'][source] = hashlib.sha256((DESIGN / source).read_bytes()).hexdigest()
    for path in sorted(OUT.glob('*.png')):
        record['files'][f'ui/pings/{path.name}'] = hashlib.sha256(path.read_bytes()).hexdigest()
    RECORD.write_text(json.dumps(record, indent=2) + '\n', encoding='utf-8', newline='\n')
    ui = json.loads(UI_RECORD.read_text(encoding='utf-8'))
    ui['files'] = {k: v for k, v in ui['files'].items() if not k.startswith('ui/pings/')}
    ui['files'].update(record['files'])
    UI_RECORD.write_text(json.dumps(ui, indent=2) + '\n', encoding='utf-8', newline='\n')
    print(f'{len(record["files"])} ping assets written to {OUT.relative_to(ROOT)}')


if __name__ == '__main__':
    main()
