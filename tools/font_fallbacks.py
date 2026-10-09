"""Complete the HUD font sets' fallback chains so every game language's
script renders whatever the game's own language is.

The game picks a font set entry by ITS language; the mod's text can be in
another (Settings > Interface > Mod language). Each entry therefore keeps
its own fonts first (so shared characters keep that language's style) and
then gains every other script font the game ships: Latin/Cyrillic/
Vietnamese, Simplified and Traditional Chinese, Japanese, Korean and Thai.

    python tools/font_fallbacks.py    (rewrites probe/font/*.font_set and
                                       their digests in tools/records)
tools/generate_hud_fonts.py applies the same rule when it regenerates.
"""
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = [
    'asset/base/font/NotoSans/NotoSans-Regular',
    'asset/base/font/NotoSansSC/NotoSansSC-Regular',
    'asset/base/font/NotoSansTC/NotoSansTC-Regular',
    'asset/base/font/NotoSansJP/NotoSansJP-Regular',
    'asset/base/font/NotoSansKR/NotoSansKR-Regular',
    'asset/base/font/NotoSansThai/NotoSansThai-Regular',
]


def complete(chain):
    """`chain` followed by every script font it lacks, in SCRIPTS order."""
    return chain + [font for font in SCRIPTS if font not in chain]


def main():
    record_path = ROOT / 'tools/records/ui-graphics.json'
    record = json.loads(record_path.read_text(encoding='utf-8'))
    for role in ['medium', 'numeric']:
        path = ROOT / 'probe/font' / f'{role}.font_set'
        sets = json.loads(path.read_text(encoding='utf-8'))
        sets = {locale: complete(chain) for locale, chain in sets.items()}
        path.write_text(json.dumps(sets, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
        record['files'][f'font/{role}.font_set'] = hashlib.sha256(path.read_bytes()).hexdigest()
    record_path.write_text(json.dumps(record, indent=2) + '\n', encoding='utf-8')
    print('Font sets now fall back to every game script.')


if __name__ == '__main__':
    main()
