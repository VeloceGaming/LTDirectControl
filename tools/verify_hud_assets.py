"""Check enabled installed asset declarations without loading or copying textures."""
from paths import GAME_DIR
import json
from pathlib import Path

root = Path(__file__).resolve().parents[1]
version = json.loads((root/'probe/mod.mod_info').read_text(encoding='utf-8'))['version']
game = GAME_DIR
read = lambda path: json.loads(path.read_text(encoding='utf-8-sig'))
config = read(game/'config/game/mods.json')
roots = {}
for directory in (game/'mods').iterdir():
    metadata = directory/'mod.mod_info'
    if metadata.is_file():
        info = read(metadata)
        if 'mod_id' in info:
            roots[info['mod_id']] = directory
for info in config['known_workshop_items']:
    directory = game.parents[1]/'workshop/content/3009300'/str(info['published_file_id'])
    metadata = directory/'mod.mod_info'
    if metadata.is_file():
        value = read(metadata)
        if 'mod_id' in value:
            roots.setdefault(value['mod_id'], directory)
champions = {}
tags = []
sheet_source = None
for name in config['enabled_mods']:
    directory = roots.get(name)
    if directory is None:
        continue
    for file in sorted(directory.rglob('*.data_champion')):
        value = read(file)
        icons = value.get('skill_icons')
        if not isinstance(icons, list) or len(icons) != 3:
            continue
        for source in icons:
            assert source.startswith(f'asset/{name}/'), (file, source)
            relative = source.removeprefix(f'asset/{name}/')
            assert all(p not in ['', '.', '..'] for p in relative.split('/'))
            assert (directory/relative).with_suffix('.png').is_file(), source
        champions[value['id']] = {'mod_id': name, 'icons': icons}
    overrides = directory/'mod.override_info'
    if overrides.is_file():
        rule = read(overrides).get('asset/base/aseprite_resources/ingame/item_icons_18x18#data')
        if rule and rule['type'] == 'override':
            sheet_source = rule['remapping']
            assert sheet_source.startswith(f'asset/{name}/')
            sheet = directory/sheet_source.removeprefix(f'asset/{name}/')
            tags = sorted(read(sheet.with_suffix('.sprite_sheet'))['images'])
assert 'candygel' in champions and 'amazon' in champions
assert 'collector' in tags and 'bf_sword' in tags
report = {'version': version, 'enabled_mods': config['enabled_mods'],
          'png_champions': champions, 'item_sheet_override': sheet_source,
          'item_tags': tags, 'rendering': 'not verified; requires in-game test'}
(root/f'research/hud-enabled-assets-{version}.json').write_text(json.dumps(report, indent=2)+'\n', encoding='utf-8')
print(f'Verified {len(champions)} enabled PNG champion declarations and {len(tags)} overridden item tags. No textures copied.')
