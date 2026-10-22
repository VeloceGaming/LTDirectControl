"""Skill preview viewer and assignment editor: the main tool for checking and
fixing skill previews (steps 2 and 3 of the agreed preview plan,
docs/backlog.md item 2).

Double-click tools/preview_viewer.cmd, or run:

    python tools/preview_viewer.py            # opens the viewer in the browser
    python tools/preview_viewer.py --static   # writes one self-contained page instead

The viewer runs on this PC only (127.0.0.1). Every page load reads the
latest data, so there is nothing to regenerate:

- research/skill-shapes.json in the mod's folder: every champion's skills as
  the mod recorded them in play (probe/src/shape_record.rs);
- the game's own numbers for each skill (the bundle's champion_info sheet,
  the champion files of installed mods), English names and descriptions
  (asset/base/text/champion and each mod's champion.i18n), and which mod
  each champion comes from;
- the preview style (probe/src/preview_style.json plus the user's override);
- skill-overrides.json in the mod's folder: the author's assignments, which
  "Save" rewrites (the previous file is kept as skill-overrides.json.bak).
  The mod reads it when the game starts (probe/src/skill_preview.rs).
  Every Save (and every size pick) also runs tools/build_preview_table.py,
  so probe/src/preview_table.json follows the viewer; its one-line result
  shows at the top of the page. A failed build never undoes the save.
- skill-checked.json in the mod's folder: the skills the author has ticked
  "Checked by me" after comparing them with the game (the viewer only).
- skill-sources.json in the mod's folder: which of the game's numbers the
  author picked for a size when two fit (step 4, stage A); tools/preview_sources.py
  decides the rest, and tools/build_preview_table.py writes the table the mod
  ships from all of these.

Champions the player's game can field (base game and enabled mods) that no
match has recorded get a card made from their data file when they have one
(`with_file_cards`): how each skill is cast, its range and its numbers,
marked as not seen in play. The others are listed as not recorded yet.

The page itself is tools/preview_viewer.html.
"""
import argparse
import json
import math
import os
import re
import struct
import sys
import threading
import webbrowser
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

import preview_sources

ROOT = Path(__file__).resolve().parent.parent
PAGE = Path(__file__).resolve().parent / 'preview_viewer.html'
MOD_FOLDER = Path(os.environ.get('LOCALAPPDATA', '')) / 'LTDirectControl'
TABLE = MOD_FOLDER / 'research' / 'skill-shapes.json'
OVERRIDES = MOD_FOLDER / 'skill-overrides.json'
# Skills the author has compared with the game ("Checked by me").
CHECKED = MOD_FOLDER / 'skill-checked.json'
# The author's picks where two of the game's numbers fit one size.
PICKS = MOD_FOLDER / 'skill-sources.json'
GAME = Path(os.environ.get('TFM2_GAME_DIR', r'C:\Program Files (x86)\Steam\steamapps\common\Teamfight Manager2'))
SLOTS = {'skill': 'Q', 'skill1': 'Q', 'skill2': 'W', 'ult': 'R'}
# What the mod accepts (probe/src/skill_preview.rs `assigned`): required
# values per type; every number must be 0..10000.
REQUIRED = {
    'unit_target': [], 'self_circle': ['radius'], 'aim_circle': ['radius'],
    'aim_rectangle': ['width', 'depth'], 'line': ['width'], 'fan': ['count', 'width', 'spread'],
    'cone': ['radius', 'angle'], 'dash': [], 'jump_back': ['distance'], 'no_preview': [],
    'attack_reach': ['extra'],
}
OPTIONAL = {'unit_target': ['area_radius'], 'line': ['length'], 'fan': ['length'], 'dash': ['distance']}
FLAGS = ['allies', 'blink', 'add']
# A custom preview (1.5.2-dev2): the author's pieces, each a block the mod
# draws (probe/src/preview_table.rs `footprint`) with its sizes as sources.
PIECE_SIZES = {
    'circle': ['radius'], 'strip': ['radius', 'length'], 'shot': ['radius', 'length'],
    'rectangle': ['width', 'height'], 'cone': ['radius', 'cosine'],
    'spread': ['count', 'radius', 'length', 'degrees'], 'fan': ['count', 'radius', 'length'],
    'dash': [], 'blink': [], 'backstep': ['length'], 'reach': ['extra'],
}
PIECE_OPTIONAL = {'strip': ['length'], 'spread': ['length']}
PIECE_FLAGS = ['allies', 'projectile', 'pierce']
PLACES = ('forward', 'end', 'landing')
MAX_PIECES = 8


def read_json(path):
    try:
        return json.loads(path.read_text(encoding='utf-8-sig'))
    except (OSError, ValueError):
        return None


def style():
    base = read_json(ROOT / 'probe/src/preview_style.json') or {}
    user = read_json(MOD_FOLDER / 'preview_style.json')
    if isinstance(user, dict):
        base.update(user)
    keys = ['yellow', 'white', 'gray', 'ink', 'ally', 'range_ready', 'range_ally',
            'area_fill', 'range_fill', 'range_width', 'range_opacity']
    return {k: base[k] for k in keys if k in base}


def flatten(value, out, depth=0):
    """Every numeric field in a skill's data, as name -> value (first seen)."""
    if depth > 12:
        return
    if isinstance(value, dict):
        for key, item in value.items():
            if isinstance(item, bool):
                continue
            if isinstance(item, (int, float)):
                out.setdefault(key, item)
            else:
                flatten(item, out, depth + 1)
    elif isinstance(value, list):
        for item in value:
            flatten(item, out, depth + 1)


def clean(text):
    """Game text without its colour and icon markup."""
    return re.sub(r'<[^>]*>', '', text or '').strip()


def game_language():
    """English names and descriptions (the author, 2026-10-21: the internal
    id and the English name are enough for this private tool)."""
    return 'en'


def add_text(names, text, lang):
    """Champion and skill names and descriptions from one champion text file."""
    for section in (text.get(lang), text.get('en')):
        if not isinstance(section, dict):
            continue
        for champion, entry in (section.get('description') or {}).items():
            if not isinstance(entry, dict):
                continue
            n = names.setdefault(champion, {})
            n.setdefault('name', clean(entry.get('name')))
            for key, slot in SLOTS.items():
                if entry.get(key):
                    n.setdefault(f'desc_{slot}', clean(entry.get(key)))
        for champion, entry in (section.get('skill_name') or {}).items():
            if not isinstance(entry, dict):
                continue
            n = names.setdefault(champion, {})
            for key, slot in SLOTS.items():
                if entry.get(key):
                    n.setdefault(f'skill_{slot}', clean(entry.get(key)))


def mod_roots():
    """(mod name, folder, enabled) of every installed mod, enabled ones first."""
    config = read_json(GAME / 'config/game/mods.json') or {}
    enabled = config.get('enabled_mods') or []
    found = {}
    folders = [p for p in (GAME / 'mods').iterdir() if p.is_dir()] if (GAME / 'mods').is_dir() else []
    workshop = GAME.parent.parent / 'workshop/content/3009300'
    for item in config.get('known_workshop_items') or []:
        folders.append(workshop / str(item.get('published_file_id')))
    for folder in folders:
        info = read_json(folder / 'mod.mod_info')
        if isinstance(info, dict) and info.get('mod_id') and info['mod_id'] not in found:
            found[info['mod_id']] = (info.get('name') or info['mod_id'], folder)
    order = [m for m in enabled if m in found] + [m for m in found if m not in enabled]
    return [(found[m][0], found[m][1], m in enabled) for m in order]


# Names that hold more than one value in a skill's data, by champion and
# slot, from the last read: the game cannot be asked for one of them by name.
DUPLICATES = {}
# The bundle's balance patch rules from the last read (which numbers are
# distances), kept for recomputing sources between page loads.
RULES = {}
# Skill cards from data files (`file_card`), by champion, from the last read;
# the base game's file first, then the first mod's.
FILE_CARDS = {}
# Each champion's basic attack range (world units) from the game's files, for
# drawing the attack-range ring.
ATTACK_RANGES = {}
# The game's names for how a skill is cast and what it targets, as the
# codes a recording uses (probe/src/shape_record.rs `Cast`;
# mod-api-stable `CastingTargetV1`).
CASTING = {'targeting': 0, 'position': 1, 'direction': 2, 'none': 3}
TARGETS = {name.lower(): code for code, name in enumerate((
    'Ally', 'AllyChampion', 'AllyChampionInCc', 'AllyNotSelf', 'AllyOnlySelf', 'Enemy',
    'EnemyWithoutTower', 'EnemyChampion', 'EnemyChampionInCc', 'EnemyChampionRecentlyAttacked',
    'Both', 'BothWithoutTower', 'BothChampion', 'None'))}


def all_values(value, out, depth=0):
    """Every numeric field in a skill's data, as name -> every value seen."""
    if depth > 12:
        return
    if isinstance(value, dict):
        for key, item in value.items():
            if isinstance(item, bool):
                continue
            if isinstance(item, (int, float)):
                out.setdefault(key, set()).add(item)
            else:
                all_values(item, out, depth + 1)
    elif isinstance(value, list):
        for item in value:
            all_values(item, out, depth + 1)


def file_card(data):
    """{slot: {cast, kind, cast_range, values}} from a champion's data file.
    A name that appears more than once with different values (nested
    effects: Alchemist Q has two `radius`) is left out of the values: the
    game answers only one of them in play. Skills with an unknown cast
    kind are left out."""
    card = {}
    for key, slot in SLOTS.items():
        skill = data.get(key)
        if not isinstance(skill, dict) or slot in card:
            continue
        casting = CASTING.get(str(skill.get('casting_type', '')).lower())
        target = TARGETS.get(str(skill.get('casting_target', '')).lower())
        if casting is None or target is None:
            continue
        number = lambda name: skill.get(name) if isinstance(skill.get(name), (int, float)) \
            and not isinstance(skill.get(name), bool) else 0
        found = {}
        all_values(skill, found)
        card[slot] = {
            'cast': {'casting': casting, 'target': target, 'range': number('range') / 1000,
                     'growth': number('growth_range') / 1000},
            'kind': skill.get('patch_type_name'),
            'cast_range': number('range') / 1000,
            'values': {name: next(iter(v)) for name, v in sorted(found.items()) if len(v) == 1},
        }
    return card


def attack_range(champion, entry):
    """Keep a champion's basic attack range from its sheet entry or file."""
    attack = entry.get('attack') if isinstance(entry, dict) else None
    value = attack.get('range') if isinstance(attack, dict) else None
    if isinstance(value, (int, float)) and not isinstance(value, bool) and value > 0:
        ATTACK_RANGES.setdefault(champion, value / 1000)


def with_file_cards(table, playable):
    """The recorded table plus a card from the data file for every champion
    the game can field that no match has recorded. A recording always wins.
    `file_cards` lists the champions whose cards come from files."""
    out = dict(table)
    champions = dict(out.get('champions') or {})
    numbers = dict(out.get('numbers') or {})
    added = []
    for champion in sorted(playable):
        card = FILE_CARDS.get(champion)
        if champion in champions or not card:
            continue
        champions[champion] = {slot: {'cast': entry['cast'], 'source': 'files'}
                               for slot, entry in card.items()}
        numbers[champion] = {slot: {'cast_range': entry['cast_range'], 'kind': entry['kind'],
                                    'values': entry['values']} for slot, entry in card.items()}
        added.append(champion)
    out.update({'champions': champions, 'numbers': numbers, 'file_cards': added})
    return out


def note_duplicates(champion, slot, skill, replace):
    """Keep the names used with more than one value in this skill (Bulldozer
    Q: `speed` 7 for the push and 4200 for the charge)."""
    seen = {}
    all_values(skill, seen)
    found = {name: sorted(values) for name, values in seen.items() if len(values) > 1}
    slots = DUPLICATES.setdefault(champion, {})
    if replace or slot not in slots:
        slots[slot] = found


def game_data():
    """Numbers, names and source of every champion the game and its mods
    know, which of them the player's game can field (the base game and
    the enabled mods), and the bundle's balance patch rules."""
    numbers, names, source, playable = {}, {}, {}, set()
    FILE_CARDS.clear()
    ATTACK_RANGES.clear()
    DUPLICATES.clear()
    lang = game_language()
    try:
        with (GAME / 'bundle.game_data').open('rb') as b:
            word = lambda: struct.unpack('<I', b.read(4))[0]
            for _ in range(word()):
                extension = b.read(word()).decode()
                path = b.read(word()).decode()
                size = word()
                wanted = path.startswith('asset/base/') and (
                    extension in ('champion_info_sheet', 'data_champion', 'patch_setting')
                    or (extension == 'i18n' and path == 'asset/base/text/champion'))
                if not wanted:
                    b.seek(size, 1)
                    continue
                data = json.loads(b.read(size))
                if extension == 'patch_setting':
                    RULES.clear()
                    RULES.update(data)
                    continue
                if extension == 'i18n':
                    add_text(names, data, lang)
                    continue
                entries = {k: v for k, v in data.items() if k != 'mod_champions'} \
                    if extension == 'champion_info_sheet' else {data.get('id'): data}
                if extension == 'data_champion' and data.get('id'):
                    FILE_CARDS.setdefault(data['id'], file_card(data))
                for champion, entry in entries.items():
                    source.setdefault(champion, 'Base game')
                    attack_range(champion, entry)
                    playable.add(champion)
                    for key, slot in SLOTS.items():
                        if isinstance(entry, dict) and isinstance(entry.get(key), dict):
                            found = {}
                            flatten(entry[key], found)
                            numbers.setdefault(champion, {})[slot] = found
                            note_duplicates(champion, slot, entry[key], replace=True)
    except (OSError, ValueError, struct.error):
        pass
    for mod_name, folder, enabled in mod_roots():
        for path in folder.rglob('*.data_champion'):
            data = read_json(path)
            champion = data.get('id') if isinstance(data, dict) else None
            if not champion:
                continue
            source.setdefault(champion, mod_name)
            if enabled:
                playable.add(champion)
            FILE_CARDS.setdefault(champion, file_card(data))
            attack_range(champion, data)
            for key, slot in SLOTS.items():
                if isinstance(data.get(key), dict):
                    found = {}
                    flatten(data[key], found)
                    numbers.setdefault(champion, {}).setdefault(slot, found)
                    note_duplicates(champion, slot, data[key], replace=False)
        for path in folder.rglob('champion.i18n'):
            text = read_json(path)
            if isinstance(text, dict):
                add_text(names, text, lang)
    return numbers, names, source, playable, lang


def read_picks():
    picks = read_json(PICKS)
    return picks.get('picks') if isinstance(picks, dict) and isinstance(picks.get('picks'), dict) else {}


def data():
    """Everything the page shows, read fresh."""
    table = read_json(TABLE) or {}
    # Shapes recorded before 1.5.0-dev19, drawn with the readers' landing
    # rule (tools/preview_sources.py `landed`), and line hits recorded before
    # 1.5.2-dev1 at their true width (`widened`), as the shipped table has them.
    for champion, slots in (table.get('champions') or {}).items():
        for entry in slots.values():
            if isinstance(entry, dict):
                if entry.get('shapes'):
                    entry['shapes'] = preview_sources.landed(
                        preview_sources.widened(entry['shapes'], champion))
                if isinstance(entry.get('file'), dict) and entry['file'].get('shapes'):
                    entry['file']['shapes'] = preview_sources.landed(entry['file']['shapes'])
    overrides = read_json(OVERRIDES) or {}
    checked = read_json(CHECKED) or {}
    numbers, names, source, playable, lang = game_data()
    table = with_file_cards(table, playable)
    champions = table.get('champions') or {}
    missing = sorted(c for c in playable if c not in champions)
    saved = overrides.get('skills') if isinstance(overrides, dict) else {}
    picks = read_picks()
    return {
        'table': table,
        'style': style(),
        'numbers': {c: numbers[c] for c in champions if c in numbers},
        'attack_ranges': {c: ATTACK_RANGES[c] for c in champions if c in ATTACK_RANGES},
        # Names with more than one value in a skill: typed values only.
        'number_uses': {c: {s: d for s, d in DUPLICATES[c].items() if d}
                        for c in champions if c in DUPLICATES and any(DUPLICATES[c].values())},
        'names': {c: names[c] for c in list(champions) + missing if c in names},
        'source': {c: source.get(c, 'Unknown mod') for c in list(champions) + missing},
        # Champions the player's game can field that no match has shown yet.
        'missing': missing,
        'saved': saved,
        'checked': checked.get('skills') if isinstance(checked, dict) else [],
        # Where each size comes from (step 4, stage A) and the author's picks.
        'sources': preview_sources.all_skills(table, RULES, saved, picks),
        'picks': picks,
        'paths': {'table': str(TABLE), 'overrides': str(OVERRIDES), 'picks': str(PICKS)},
        'lang': lang,
    }


def sources(body):
    """Sources for assignments and picks not saved yet (nothing is written)."""
    table = read_json(TABLE) or {}
    skills = body.get('skills') if isinstance(body.get('skills'), dict) else {}
    picks = body.get('picks') if isinstance(body.get('picks'), dict) else {}
    return {'ok': True, 'sources': preview_sources.all_skills(table, RULES, skills, picks)}


def save_picks(body):
    """The author's picks: {"<champion> <Q|W|R>": {size key: source}}."""
    picks = body.get('picks')
    clean = {}
    for skill, sizes in (picks or {}).items():
        if not (isinstance(skill, str) and re.fullmatch(r'\S+ [QWR]', skill) and isinstance(sizes, dict)):
            continue
        kept = {key: source for key, source in sizes.items()
                if isinstance(key, str) and isinstance(source, dict)
                and (source.get('cast_range') is True
                     or isinstance(source.get('fixed'), (int, float))
                     or isinstance(source.get('name'), str))}
        if kept:
            clean[skill] = dict(sorted(kept.items()))
    MOD_FOLDER.mkdir(parents=True, exist_ok=True)
    temp = PICKS.with_name(PICKS.name + '.tmp')
    temp.write_text(json.dumps({'about': 'Which of the game\'s numbers the author picked for a preview size, '
                                         'written by tools/preview_viewer.py; read by tools/build_preview_table.py.',
                                'picks': dict(sorted(clean.items()))}, indent=2), encoding='utf-8')
    temp.replace(PICKS)
    return {'ok': True, 'picked': sum(len(v) for v in clean.values()), 'table': rebuild_table()}


def save_checked(body):
    """The skills the author has compared with the game."""
    skills = body.get('skills')
    keys = sorted({k for k in skills or [] if isinstance(k, str) and re.fullmatch(r'\S+ [QWR]', k)})
    MOD_FOLDER.mkdir(parents=True, exist_ok=True)
    temp = CHECKED.with_name(CHECKED.name + '.tmp')
    temp.write_text(json.dumps({'about': 'Skills the author compared with the game in tools/preview_viewer.py.',
                                'skills': keys}, indent=2), encoding='utf-8')
    temp.replace(CHECKED)
    return {'ok': True, 'checked': len(keys)}


def number(value):
    return isinstance(value, (int, float)) and not isinstance(value, bool) and math.isfinite(value)


def valid_source(source, low=0, high=100000):
    """A custom size: a fixed number, the cast range, or one of the game's
    numbers by name (with `recorded`, the value the mod uses until it has
    read the number), maybe times another (`by`)."""
    if not isinstance(source, dict):
        return False
    if 'fixed' in source:
        return set(source) == {'fixed'} and number(source['fixed']) and low <= source['fixed'] <= high
    if source.get('cast_range') is True:
        return set(source) <= {'cast_range', 'recorded'} and number(source.get('recorded', 0))
    if isinstance(source.get('name'), str) and source['name']:
        return (set(source) <= {'name', 'times', 'by', 'unit', 'recorded'}
                and number(source.get('recorded'))
                and number(source.get('times', 1))
                and ('by' not in source or (isinstance(source['by'], str) and bool(source['by'])))
                and source.get('unit') in (None, 'count', 'degrees'))
    return False


def clean_custom(entry):
    """A custom preview as the mod accepts it, or why not."""
    pieces = entry.get('pieces')
    if not isinstance(pieces, list):
        return None, 'pieces missing'
    if len(pieces) > MAX_PIECES:
        return None, f'at most {MAX_PIECES} pieces'
    out = []
    for index, piece in enumerate(pieces, 1):
        kind = piece.get('kind') if isinstance(piece, dict) else None
        if kind not in PIECE_SIZES:
            return None, f'piece {index}: unknown block {kind}'
        clean = {'kind': kind}
        for field in PIECE_SIZES[kind]:
            if field not in piece:
                if field in PIECE_OPTIONAL.get(kind, []):
                    continue
                return None, f'piece {index}: {field} missing'
            low, high = (-1, 1) if field == 'cosine' else (0, 100000)
            if not valid_source(piece[field], low, high):
                return None, f'piece {index}: {field} is not a size'
            clean[field] = piece[field]
        at = piece.get('at')
        if at in ('caster', 'aim'):
            clean['at'] = at
        elif (isinstance(at, dict) and len(at) == 1 and next(iter(at)) in PLACES
              and valid_source(next(iter(at.values())))):
            clean['at'] = at
        else:
            return None, f'piece {index}: where it starts is unknown'
        for flag in PIECE_FLAGS:
            if piece.get(flag) is True:
                clean[flag] = True
        out.append(clean)
    return {'type': 'custom', 'pieces': out}, None


def validate(skills):
    """The entries the mod would accept, and why the others are not."""
    good, problems = {}, []
    if not isinstance(skills, dict):
        return good, ['no skills']
    for key, entry in skills.items():
        if not (isinstance(key, str) and re.fullmatch(r'\S+ [QWR]', key) and isinstance(entry, dict)):
            problems.append(f'{key}: not a skill')
            continue
        kind = entry.get('type')
        if kind == 'custom':
            clean, bad = clean_custom(entry)
            if bad:
                problems.append(f'{key}: {bad}')
            else:
                good[key] = clean
            continue
        if kind not in REQUIRED:
            problems.append(f'{key}: unknown type {kind}')
            continue
        clean_entry, bad = {'type': kind}, None
        for field in REQUIRED[kind] + OPTIONAL.get(kind, []):
            value = entry.get(field)
            if value is None:
                if field in REQUIRED[kind]:
                    bad = f'{field} missing'
                continue
            if not isinstance(value, (int, float)) or isinstance(value, bool) or not 0 <= value <= 10000:
                bad = f'{field} must be 0 to 10000'
                continue
            clean_entry[field] = value
        for flag in FLAGS:
            if entry.get(flag) is True:
                clean_entry[flag] = True
        if bad:
            problems.append(f'{key}: {bad}')
        else:
            good[key] = clean_entry
    return good, problems


# One table build at a time: picks save on their own, so two can overlap.
REBUILD = threading.Lock()


def rebuild_table():
    """Run tools/build_preview_table.py after a save (1.5.1-dev3), so the
    shipped table follows the viewer. It runs as its own process: it reads
    the files just saved, and nothing it does can undo the save. Builds run
    one at a time, each after the save that started it, so the last one
    reads the newest files. Returns {ok, line}: its one-line result, or why
    it failed."""
    import subprocess
    with REBUILD:
        return run_build(subprocess)


def run_build(subprocess):
    try:
        run = subprocess.run([sys.executable, str(Path(__file__).with_name('build_preview_table.py'))],
                             capture_output=True, text=True, encoding='utf-8', errors='replace',
                             timeout=300, cwd=str(ROOT))
    except (OSError, subprocess.SubprocessError) as error:
        return {'ok': False, 'line': f'Preview table not rebuilt: {error}'}
    lines = [l for l in (run.stdout + '\n' + run.stderr).splitlines() if l.strip()]
    if run.returncode == 0:
        result = [l for l in lines if l.startswith('Preview table:')]
        return {'ok': True, 'line': (result or lines or ['Preview table rebuilt'])[-1]}
    return {'ok': False, 'line': 'Preview table not rebuilt (errors): ' + (lines[-1] if lines else f'exit {run.returncode}')}


def save(body):
    good, problems = validate(body.get('skills'))
    body = {
        'about': 'The author\'s skill preview assignments, written by tools/preview_viewer.py. '
                 'Keyed "<champion> <Q|W|R>"; sizes in world units. The mod reads this file when the game starts.',
        'skills': dict(sorted(good.items())),
    }
    MOD_FOLDER.mkdir(parents=True, exist_ok=True)
    if OVERRIDES.is_file():
        OVERRIDES.replace(OVERRIDES.with_name(OVERRIDES.name + '.bak'))
    temp = OVERRIDES.with_name(OVERRIDES.name + '.tmp')
    temp.write_text(json.dumps(body, indent=2, ensure_ascii=False), encoding='utf-8')
    temp.replace(OVERRIDES)
    return {'ok': True, 'saved': len(good), 'problems': problems, 'path': str(OVERRIDES),
            'table': rebuild_table()}


class Handler(BaseHTTPRequestHandler):
    def send(self, status, kind, body):
        payload = body if isinstance(body, bytes) else body.encode('utf-8')
        self.send_response(status)
        self.send_header('Content-Type', kind)
        self.send_header('Cache-Control', 'no-store')
        self.send_header('Content-Length', str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)

    def do_GET(self):
        if self.path in ('/', '/index.html', '/preview-viewer.html'):
            self.send(200, 'text/html; charset=utf-8', PAGE.read_text(encoding='utf-8'))
        elif self.path == '/api/data':
            self.send(200, 'application/json', json.dumps(data(), ensure_ascii=False))
        else:
            self.send(404, 'text/plain', 'not found')

    def do_POST(self):
        handlers = {'/api/save': save, '/api/checked': save_checked,
                    '/api/sources': sources, '/api/picks': save_picks}
        if self.path not in handlers or self.headers.get('Origin', '').split('//')[-1].split(':')[0] not in ('', '127.0.0.1', 'localhost'):
            self.send(403, 'text/plain', 'refused')
            return
        try:
            length = min(int(self.headers.get('Content-Length', '0')), 2_000_000)
            body = json.loads(self.rfile.read(length))
            if not isinstance(body, dict):
                raise ValueError('not an object')
            self.send(200, 'application/json', json.dumps(handlers[self.path](body)))
        except (ValueError, AttributeError, OSError) as error:
            self.send(400, 'application/json', json.dumps({'ok': False, 'problems': [str(error)]}))

    def log_message(self, *args):
        pass


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--port', type=int, default=8766)
    parser.add_argument('--no-browser', action='store_true', help='do not open the browser')
    parser.add_argument('--static', action='store_true', help='write one self-contained page and exit')
    args = parser.parse_args()
    if args.static:
        out = MOD_FOLDER / 'research' / 'preview-viewer.html'
        page = PAGE.read_text(encoding='utf-8').replace('/*DATA*/null', json.dumps(data(), ensure_ascii=False))
        out.write_text(page, encoding='utf-8')
        print(f'Wrote {out}. Saving from this page downloads the file instead of writing it.')
        return
    server = None
    for port in range(args.port, args.port + 20):
        try:
            server = ThreadingHTTPServer(('127.0.0.1', port), Handler)
            break
        except OSError:
            continue
    if server is None:
        sys.exit('No free port for the viewer.')
    url = f'http://127.0.0.1:{server.server_address[1]}/'
    print(f'Skill preview viewer: {url}')
    print('Keep this window open while you use the viewer; close it when you are done.')
    if not args.no_browser:
        threading.Timer(0.5, lambda: webbrowser.open(url)).start()
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass


if __name__ == '__main__':
    main()
