"""Read installed UI/icon metadata; no texture extraction or game changes."""
from paths import GAME_DIR
import json
import struct
from pathlib import Path

root = Path(__file__).resolve().parents[1]
bundle = GAME_DIR / 'bundle.game_data'
records = []
with bundle.open('rb') as f:
    def integer():
        return struct.unpack('<I', f.read(4))[0]
    for _ in range(integer()):
        extension = f.read(integer()).decode()
        path = f.read(integer()).decode()
        size = integer()
        offset = f.tell()
        if ('item' in path or 'skill_icon' in path or extension == 'champion_view') and extension in ['sprite_sheet', 'item_setting', 'champion_view']:
            data = f.read(size)
            record = dict(extension=extension, path=path, offset=offset, size=size)
            records.append(record)
            output = path.rsplit('/', 1)[-1].replace('#', '-')
            (root/'research'/('hud-'+output+'.json')).write_bytes(data)
        else:
            f.seek(size, 1)
(root/'research/hud-assets-index.json').write_text(json.dumps(records, indent=2), encoding='utf-8')
print(json.dumps(records, indent=2))

# Ship references only. Textures stay in the installed game bundle.
skills = json.loads((root/'research/hud-skill_icon-data.json').read_text())['images']
champions = {}
for tag in skills:
    name, index = tag.rsplit('_', 1)
    if index == '0' and all(f'{name}_{i}' in skills for i in range(3)):
        champions[name] = dict(source='asset/base/aseprite_resources/UI_aseprite/skill_icon', tags=[f'{name}_{i}' for i in range(3)])
settings = json.loads((root/'research/champion_info.sheet').read_text())
for champion in settings['mod_champions']:
    if 'skill_icon' in champion:
        champions[champion['id']] = champion['skill_icon']
items = {}
for name, item in json.loads((root/'research/hud-item_setting.json').read_text()).items():
    if not isinstance(item, dict) or not all(k in item for k in ['icon', 'tier', 'price', 'category', 'key']):
        continue
    spec = {k: item[k] for k in ['icon', 'tier', 'price', 'category']}
    spec['name'] = name
    for key in [name, item['key']]:
        assert key not in items or items[key] == spec
        items[key] = spec
(root/'probe/src/hud_assets.json').write_text(json.dumps(dict(champions=champions, items=items), indent=2), encoding='utf-8')
print(f'Generated icon references: {len(champions)} champions, {len(items)} item keys/aliases.')
