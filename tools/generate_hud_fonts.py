"""Package static instances of the approved lab fonts; preserve native fallbacks."""
from paths import FONT_SOURCES, GAME_DIR
import hashlib
import json
from pathlib import Path
import shutil
import struct
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'research/fonttools-runtime'))
from fontTools.ttLib import TTFont
from fontTools.varLib.instancer import instantiateVariableFont

LAB = FONT_SOURCES
OUT = ROOT / 'probe/font'
OUT.mkdir(exist_ok=True)
records = {}
widths = {}
for name, source, weight in [('manrope_medium', 'Manrope[wght].ttf', 500),
                             ('manrope_numeric', 'Manrope[wght].ttf', 650),
                             ('noto_tc_medium', 'NotoSansTC[wght].ttf', 500)]:
    font = instantiateVariableFont(TTFont(LAB / source), {'wght': weight}, inplace=True)
    assert 'fvar' not in font
    font.save(OUT / (name + '.ttf'))
    records[name] = {'source': source, 'weight': weight,
                     'source_sha256': hashlib.sha256((LAB / source).read_bytes()).hexdigest()}
    if weight == 500:
        for code, glyph in font.getBestCmap().items():
            # First font owns Latin, second supplies CJK and other glyphs.
            widths.setdefault(str(code), round(font['hmtx'][glyph][0] * 1000 / font['head'].unitsPerEm))
    font.close()

from font_fallbacks import complete

game = GAME_DIR
with (game / 'bundle.game_data').open('rb') as f:
    integer = lambda: struct.unpack('<I', f.read(4))[0]
    base = None
    for _ in range(integer()):
        ext, key = f.read(integer()).decode(), f.read(integer()).decode()
        n = integer()
        if ext == 'font_set' and key == 'asset/base/font/set/regular':
            base = json.loads(f.read(n))
        else:
            f.seek(n, 1)
assert base
for role in ['medium', 'numeric']:
    mapping = {}
    for locale, fallback in base.items():
        latin = 'asset/lt_direct_control/font/manrope_' + role
        mapping[locale] = complete([latin] + (['asset/lt_direct_control/font/noto_tc_medium'] if locale == 'zh-hant' else []) + fallback[1:])
    (OUT / (role + '.font_set')).write_text(json.dumps(mapping, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
for license in ['Manrope-OFL.txt', 'NotoSansTC-OFL.txt']:
    shutil.copyfile(LAB / license, OUT / license)
(ROOT / 'probe/src/hud_font_metrics.json').write_text(json.dumps(widths, separators=(',', ':')), encoding='utf-8')
(ROOT / 'tools/records/hud-fonts.json').write_text(json.dumps({'fonts': records, 'glyph_metrics': len(widths)}, indent=2) + '\n', encoding='utf-8')
print('Static HUD font weights and native language fallback sets packaged.')
