"""Machine-specific locations, overridable with environment variables.

TFM2_GAME_DIR     Teamfight Manager 2 install folder (default: Steam's default path).
LT_UI_LAB         The author's UI design lab (optional; design-preview and font
                  generation tools only). Fonts are read from <lab>/ui/src/fonts.
"""
import os
from pathlib import Path

GAME_DIR = Path(os.environ.get(
    'TFM2_GAME_DIR', r'C:\Program Files (x86)\Steam\steamapps\common\Teamfight Manager2'))
GAME_EXE = GAME_DIR / 'TeamfightManager2.exe'
UI_LAB = Path(os.environ.get('LT_UI_LAB', 'C:/LTTool/endfield_ui_lab'))
FONT_SOURCES = UI_LAB / 'ui/src/fonts'


def dll_variant(dll):
    """'public' or 'research', from the marker the mod's INIT line carries
    (probe/src/lib.rs `VARIANT`; step 4 of the preview plan, stage C). A
    research build (`cargo build --release --features research`) also
    records skill shapes and numbers; it is never for the Workshop."""
    data = Path(dll).read_bytes()
    found = [v for v in ('public', 'research') if f'variant={v}'.encode() in data]
    if len(found) != 1:
        raise SystemExit(f'{dll} carries no single build variant marker: {found}')
    return found[0]


def build_name(version, variant):
    """The name of a build's record and package: a research build gets its
    own, so both variants of one version can be kept."""
    return version if variant == 'public' else f'{version}-research'
