"""Read explicit base effect declarations from the installed bundle; no pixels."""
import argparse
import json
import struct
from pathlib import Path

root = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser()
parser.add_argument('--game', type=Path, default=Path(r'C:\Program Files (x86)\Steam\steamapps\common\Teamfight Manager2'))
args = parser.parse_args()
shapes = {}
with (args.game / 'bundle.game_data').open('rb') as bundle:
    def integer():
        return struct.unpack('<I', bundle.read(4))[0]
    count = integer()
    assert count <= 20000
    for _ in range(count):
        extension = bundle.read(integer()).decode()
        path = bundle.read(integer()).decode()
        size = integer()
        if extension != 'data_champion' or not path.startswith('asset/base/'):
            bundle.seek(size, 1)
            continue
        assert size <= 2_000_000
        value = json.loads(bundle.read(size))
        shapes[value['id']] = {
            key: {'range': value[key].get('range', 0), 'effect': value[key].get('effect')}
            for key in ('skill', 'skill2', 'ult') if key in value
        }
output = root / 'probe/src/preview_assets.json'
output.write_text(json.dumps(shapes, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
print(f'{len(shapes)} base data-defined champions; explicit declarations only.')
