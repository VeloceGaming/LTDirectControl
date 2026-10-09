"""Inventory base skill templates and audit captured native tooltip results.

Reads bundle metadata/logs only; never starts the game or calls native code.
Unobserved skills remain unobserved, rather than being counted as fixed.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import struct

from paths import GAME_DIR

ROOT = Path(__file__).resolve().parents[1]
INVENTORY = ROOT / 'tools/records/tooltip-skills.json'
SLOTS = ('skill', 'skill2', 'ult')
PARAMETER = re.compile(r'\{([A-Za-z][A-Za-z0-9_]*)\}')


def inventory_from_bundle(game):
    profile = json.loads((ROOT / 'tools/native_profiles/0.6.3.json').read_text(encoding='utf-8'))
    executable_hash = hashlib.sha256((Path(game) / 'TeamfightManager2.exe').read_bytes()).hexdigest()
    if executable_hash != profile['executable_sha256']:
        raise ValueError('Inventory refresh requires the reviewed game executable')
    translations = None
    data_champions = set()
    with (Path(game) / 'bundle.game_data').open('rb') as stream:
        def integer():
            value = stream.read(4)
            if len(value) != 4:
                raise ValueError('Truncated bundle header')
            return struct.unpack('<I', value)[0]

        def label():
            size = integer()
            if size > 4096:
                raise ValueError('Unexpected bundle label length')
            return stream.read(size).decode('utf-8')

        count = integer()
        if count > 20000:
            raise ValueError('Unexpected asset count')
        for _ in range(count):
            extension, path, size = label(), label(), integer()
            needed = (extension == 'i18n' and path == 'asset/base/text/champion') or (
                extension == 'data_champion' and path.startswith('asset/base/'))
            if not needed:
                stream.seek(size, 1)
                continue
            if size > 2_000_000:
                raise ValueError('Unexpected metadata size')
            raw = stream.read(size)
            value = json.loads(raw)
            if extension == 'i18n':
                translations = value['en']['description']
                source_hash = hashlib.sha256(raw).hexdigest()
            else:
                data_champions.add(value['id'])
    if translations is None:
        raise ValueError('Base champion translations not found')
    skills = []
    for champion, descriptions in sorted(translations.items()):
        for slot, tag in enumerate(SLOTS):
            text = descriptions.get(tag)
            if isinstance(text, str) and text.strip():
                skills.append({
                    'champion': champion, 'slot': slot, 'key': 'QWR'[slot],
                    'definition': 'data' if champion in data_champions else 'builtin',
                    'template_parameters': sorted(set(PARAMETER.findall(re.sub(r'<[^>]*>', '', text)))),
                })
    return {'schema': 1, 'game_version': profile['game_version'], 'language': 'en',
            'executable_sha256': executable_hash,
            'source': 'asset/base/text/champion', 'source_sha256': source_hash,
            'champion_count': len(translations), 'skill_count': len(skills), 'skills': skills}


def observations(logs):
    rows = []
    for text in logs:
        version = 'unknown'
        for line in text.splitlines():
            if match := re.search(r'INIT .*\bprobe=(\S+)', line):
                version = match[1]
            stamp = int(line.split(' ', 1)[0]) if re.match(r'^\d+ ', line) else 0
            if 'NATIVE TOOLTIP_AUDIT ' in line:
                try:
                    row = json.loads(line.split('NATIVE TOOLTIP_AUDIT ', 1)[1])
                    if not isinstance(row.get('champion'), str) or not row['champion']:
                        continue
                    if type(row.get('slot')) is not int or row['slot'] not in range(3):
                        continue
                    if row.get('source') not in ('game', 'fallback'):
                        continue
                    parameters = row.get('unresolved_parameters')
                    if not isinstance(parameters, list) or not all(isinstance(p, str) for p in parameters):
                        continue
                    row.update(timestamp=stamp, structured=True,
                               unresolved_count=len(set(parameters)))
                    row.setdefault('version', version)
                    if not isinstance(row['version'], str):
                        continue
                    # Rotated log segments may not carry the original INIT.
                    version = row['version']
                    rows.append(row)
                except (ValueError, TypeError, AttributeError):
                    continue
            elif match := re.search(r'NATIVE TOOLTIP champion=(\S+) slot=([012]) bytes=(\d+) unresolved=(\d+) source=game', line):
                champion, slot, size, unresolved = match.groups()
                rows.append({'champion': champion, 'slot': int(slot), 'bytes': int(size),
                             'source': 'game', 'version': version, 'timestamp': stamp,
                             'structured': False, 'unresolved_count': int(unresolved)})
            elif match := re.search(r'NATIVE TOOLTIP fallback champion=(\S+) slot=([012]) reason=(.*)', line):
                champion, slot, reason = match.groups()
                rows.append({'champion': champion, 'slot': int(slot), 'reason': reason,
                             'source': 'fallback', 'version': version, 'timestamp': stamp,
                             'structured': False, 'unresolved_count': None})
    # A structured record carries more evidence than the companion legacy
    # summary. Prefer it within a build, then choose the latest observed build.
    structured = {(r['champion'], r['slot'], r['version']) for r in rows if r['structured']}
    result = {}
    for row in sorted(rows, key=lambda r: r['timestamp']):
        if not row['structured'] and (row['champion'], row['slot'], row['version']) in structured:
            continue
        result[row['champion'], row['slot']] = row
    return result


def report(inventory, logs):
    seen = observations(logs)
    skills = []
    base_keys = set()
    for skill in inventory['skills']:
        key = skill['champion'], skill['slot']
        base_keys.add(key)
        row = dict(skill)
        observation = seen.get(key)
        if observation is None:
            row['status'] = 'unobserved'
        else:
            row['observation'] = observation
            row['status'] = ('fallback' if observation['source'] == 'fallback' else
                             'native-unresolved' if observation['unresolved_count'] else 'native-resolved')
        skills.append(row)
    statuses = ('native-resolved', 'native-unresolved', 'fallback', 'unobserved')
    counts = {status: sum(row['status'] == status for row in skills) for status in statuses}
    workshop = [value for key, value in seen.items() if key not in base_keys]
    return {'schema': 1, 'scope': 'captured native formatter output; not a gameplay/rendering test',
            'inventory_source_sha256': inventory['source_sha256'],
            'summary': {'base_champions': inventory['champion_count'], 'base_skills': len(skills),
                        **counts, 'nonbase_observations': len(workshop)},
            'skills': skills, 'nonbase_observations': workshop}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--refresh-inventory', action='store_true')
    parser.add_argument('--game', type=Path, default=GAME_DIR)
    parser.add_argument('--inventory', type=Path, default=INVENTORY)
    parser.add_argument('--logs', type=Path, nargs='*')
    parser.add_argument('--output', type=Path, default=ROOT / 'local/tooltip-coverage.json')
    args = parser.parse_args()
    if args.refresh_inventory:
        inventory = inventory_from_bundle(args.game)
        args.inventory.parent.mkdir(parents=True, exist_ok=True)
        args.inventory.write_text(json.dumps(inventory, indent=2) + '\n', encoding='utf-8')
    else:
        inventory = json.loads(args.inventory.read_text(encoding='utf-8'))
    logs = args.logs
    if logs is None:
        folder = Path(os.environ.get('LOCALAPPDATA', str(Path.home() / 'AppData/Local'))) / 'LTDirectControl'
        logs = sorted(folder.glob('probe*.log'))
    value = report(inventory, [p.read_text(encoding='utf-8', errors='replace') for p in logs])
    value['logs'] = [str(p.resolve()) for p in logs]
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(value['summary']))
    print(f'Report: {args.output}')


if __name__ == '__main__':
    main()
