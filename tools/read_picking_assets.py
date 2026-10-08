"""Extract champion poses and stable monster bodies, excluding expanded attacks."""
from paths import GAME_DIR
import json
import struct
from pathlib import Path

root = Path(__file__).resolve().parents[1]
game = GAME_DIR
profiles = {}
with (game/'bundle.game_data').open('rb') as f:
    integer = lambda: struct.unpack('<I', f.read(4))[0]
    for _ in range(integer()):
        extension = f.read(integer()).decode()
        path = f.read(integer()).decode()
        size = integer()
        champion = path.startswith('asset/base/aseprite_resources/champions/')
        ingame = path.startswith('asset/base/aseprite_resources/ingame/')
        if extension != 'fanim' or not (champion or ingame) or not path.endswith('#anim'):
            f.seek(size, 1)
            continue
        value = json.loads(f.read(size))
        name = path.rsplit('/',1)[-1].removesuffix('#anim')
        monster = ingame and name in {'stump', 'mushroom', 'rhino', 'epic', 'serpen', 'bee'}
        tags = (['idle', 'run', 'walk'] if monster else
                ['idle', 'run', 'walk', 'hit', 'attack', 'skill', 'skill2', 'ult',
                 'skill_pre', 'skill2_pre', 'ult_pre'])
        # Serpen's attack contains a 227x225 frame despite a 59x79 idle body.
        # A stable mouse target must not permanently include that attack frame.
        frames = [frame['data'] for tag, animation in value['anims'].items()
                  if tag in tags for frame in animation['frames']]
        if frames:
            dimensions=[max(x[k] for x in frames) for k in ['w','h']]
            if champion:
                stable=[fr['data'] for tag,a in value['anims'].items()
                        if tag in ['idle','run','walk'] for fr in a['frames']]
                baseline=[max(x[k] for x in stable) if stable else
                          sorted(x[k] for x in frames)[(len(frames)-1)//2] for k in ['w','h']]
                dimensions=[round(min(dimensions[i],baseline[i]*1.15),3) for i in range(2)]
            profiles[name if champion else 'ingame/'+name] = dimensions
(root/'probe/src/picking_assets.json').write_text(json.dumps(profiles, indent=2)+'\n', encoding='utf-8')
print(f'Extracted {len(profiles)} base body-pose profiles; no pixels copied.')
