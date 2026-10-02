"""Extract normal-pose dimensions from base animation metadata, no textures."""
import json
import struct
from pathlib import Path

root = Path(__file__).resolve().parents[1]
game = Path(r'C:\Program Files (x86)\Steam\steamapps\common\Teamfight Manager2')
profiles = {}
with (game/'bundle.game_data').open('rb') as f:
    integer = lambda: struct.unpack('<I', f.read(4))[0]
    for _ in range(integer()):
        extension = f.read(integer()).decode()
        path = f.read(integer()).decode()
        size = integer()
        if extension != 'fanim' or not path.startswith('asset/base/aseprite_resources/champions/') or not path.endswith('#anim'):
            f.seek(size, 1)
            continue
        value = json.loads(f.read(size))
        frames = [frame['data'] for name, animation in value['anims'].items()
                  if name in ['idle', 'run', 'walk'] for frame in animation['frames']]
        if frames:
            profiles[path.rsplit('/',1)[-1].removesuffix('#anim')] = [max(x['w'] for x in frames), max(x['h'] for x in frames)]
(root/'probe/src/picking_assets.json').write_text(json.dumps(profiles, indent=2)+'\n', encoding='utf-8')
print(f'Extracted {len(profiles)} base normal-pose dimension profiles; no pixels copied.')
