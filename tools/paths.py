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
