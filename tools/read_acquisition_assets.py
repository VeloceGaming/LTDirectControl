"""Extract numeric base AA declarations; no game code is loaded or executed."""
import hashlib
import json
import struct
from pathlib import Path
from paths import GAME_DIR, GAME_EXE

root = Path(__file__).resolve().parents[1]
profile = json.loads((root / 'tools/native_profiles/0.6.3.json').read_text())
assert hashlib.sha256(GAME_EXE.read_bytes()).hexdigest() == profile['executable_sha256']
rows = {}
names = {}
with (GAME_DIR / 'bundle.game_data').open('rb') as stream:
    def integer():
        return struct.unpack('<I', stream.read(4))[0]
    count = integer()
    assert count <= 20000
    for _ in range(count):
        extension = stream.read(integer()).decode()
        path = stream.read(integer()).decode()
        size = integer()
        if extension == 'i18n' and path == 'asset/base/text/champion':
            assert size <= 2_000_000
            value = json.loads(stream.read(size))
            for language in value.values():
                for champion, description in language.get('description', {}).items():
                    if isinstance(description, dict) and isinstance(description.get('name'), str):
                        names.setdefault(champion, set()).add(description['name'])
            continue
        if extension not in ('champion_info_sheet', 'data_champion') or not path.startswith('asset/base/'):
            stream.seek(size, 1)
            continue
        assert size <= 2_000_000
        value = json.loads(stream.read(size))
        champions = [(value['id'], value)] if extension == 'data_champion' else [
            (name, c) for name, c in value.items() if isinstance(c, dict)] + [
            (c['id'], c) for c in value.get('mod_champions', [])]
        for name, champion in champions:
            attack = champion.get('attack')
            if isinstance(attack, dict):
                rows[name] = {key: attack.get(key, 0) for key in ('range', 'growth_range')}
output = root / 'probe/src/acquisition_defaults.json'
output.write_text(json.dumps(rows, indent=2, sort_keys=True) + '\n', encoding='utf-8')
print(f'{len(rows)} numeric base AA declarations; runtime metadata remains authoritative.')
name_output = root / 'probe/src/acquisition_names.json'
name_output.write_text(json.dumps({key: sorted(value) for key, value in sorted(names.items())},
                                 ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
print(f'{len(names)} champion name alias lists from all bundled locales.')
