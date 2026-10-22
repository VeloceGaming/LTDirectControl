"""Write the skill preview table the mod ships (step 4 of the agreed preview
plan, stage A; docs/backlog.md item 2).

    python tools/build_preview_table.py                 # writes probe/src/preview_table.json
    python tools/build_preview_table.py --check         # writes nothing; fails if the table is stale
    python tools/build_preview_table.py --drop <champion> [--drop <champion> ...]

It reads what tools/preview_viewer.py shows: the shapes and numbers the mod
recorded in play (research/skill-shapes.json in the mod's folder), the
author's assignments (skill-overrides.json) and picks (skill-sources.json),
and the game bundle's balance patch rules. For every skill the mod can draw
it writes the shape, with each size replaced by where it comes from
(tools/preview_sources.py): one of the game's numbers by name, the live cast
range, or a fixed number.

Nothing is lost (1.5.1-dev3; the author, 2026-10-22: a champion pack is
enabled only to set up its previews, then disabled). The champions shipped
are those recorded, those assigned and those already in the shipped table:

- recorded: built from the recording, as before;
- not recorded, but its pack is enabled and ships a data file (a card from
  `preview_viewer.with_file_cards`): its assignments are built with the
  file's numbers, as before, so editing its previews needs only the pack
  enabled, no match;
- in both cases a skill the table shipped that nothing builds now keeps its
  shipped entry;
- not recorded and no card (its pack disabled), but shipped: its shipped
  entry is kept unchanged;
- assigned, with no recording, no card and no shipped entry: nothing is
  shipped; it is listed as needing one match with the champion in play (a
  research build records it).

Only `--drop <champion>` (or `--drop "<champion> <Q|W|R>"`) removes
anything. A card is never taken for a recording.

The file is rewritten only when what it ships changes. `--check` compares
what a build would write with the shipped table and exits non-zero, naming
the champions and skills that differ, when they are not the same;
tools/preview_viewer.py runs the build on every Save, tools/verify_build.py
runs the check.

The report lists what still needs the author: sizes where two numbers fit,
and skills whose numbers no match has recorded yet.
"""
import argparse
import json
import sys
from collections import Counter

import preview_sources
import preview_viewer as viewer

OUTPUT = viewer.ROOT / 'probe' / 'src' / 'preview_table.json'
SLOTS = ('Q', 'W', 'R')
ABOUT = ('Skill preview shapes the mod ships, written by tools/build_preview_table.py from the '
         'shapes and numbers recorded in play, the author\'s assignments and picks. Sizes: '
         '{"name"} is the game\'s number by name (distances divided by 1000, times "times"), '
         '{"cast_range"} the live cast range, {"fixed"} a number in world units; "recorded" is '
         'the value when it was recorded. Champions no recording covers keep their entry from '
         'the table before. Do not edit by hand: change the assignments or picks in '
         'tools/preview_viewer.py and run the tool again.')


def built_skill(champion, slot, entry, sheet):
    """One recorded skill as the table ships it, or None (passives and
    unread skills: the mod's cast-style cue)."""
    kept = {'from': entry['from']}
    if entry['from'] == 'assigned':
        kept['assigned'] = preview_sources.with_sources(entry['assigned'], entry['sizes'], 'assigned.')
        if entry.get('shapes'):
            kept['shapes'] = [preview_sources.with_sources(shape, entry['sizes'], f'{i}.')
                              for i, shape in enumerate(entry['shapes'])]
    elif entry['from'] == 'custom':
        kept['pieces'] = json.loads(json.dumps(entry['pieces']))
    elif entry['from'] in ('live', 'file'):
        kept['shapes'] = [preview_sources.with_sources(shape, entry['sizes'], f'{i}.')
                          for i, shape in enumerate(entry['shapes'])]
    elif entry['from'] != 'no_area':
        return None
    most = ((sheet or {}).get(champion) or {}).get(slot, {}).get('max_count')
    for shape in kept.get('shapes', []):
        # A recording's mark (`preview_sources.widened`), not a size.
        shape.pop('hit_radius', None)
        # Fanned blades grow with the caster's stack up to this many.
        if shape.get('kind') == 'fan' and isinstance(most, (int, float)):
            shape['max_count'] = {'fixed': most}
    if entry.get('kind'):
        kept['kind'] = entry['kind']
    return kept


def assigned_champions(assignments):
    return {key.split(' ')[0] for key in (assignments or {})
            if isinstance(key, str) and ' ' in key}


def build(table, rules, assignments, picks, sheet=None, shipped=None, recorded=None, drop=()):
    """The shipped table and its report.

    `table` is what the viewer reads (recordings, maybe with file cards);
    `recorded` the champions a match recorded (default: every champion in
    `table`); `shipped` the table shipped before (its champions are kept
    when no recording covers them); `drop` champions, or "<champion> <slot>"
    skills, to remove on purpose. `sheet` holds the game's own numbers per
    champion and slot (the bundle's champion sheet), for the few fixed
    numbers the recorded shapes lack: Dancer R's most blades."""
    before = (shipped or {}).get('champions') or {}
    cards = set(table.get('file_cards') or [])
    if recorded is None:
        recorded = set(table.get('champions') or {}) - cards
    drop = set(drop)
    champions, statuses, pick, missing = {}, Counter(), [], []
    kept, kept_skills, needs, from_files = [], [], [], []
    for champion in sorted(set(recorded) | set(before) | assigned_champions(assignments)):
        if champion in drop:
            continue
        if champion not in recorded and champion in cards:
            # Its pack is enabled: assignments take their numbers from the
            # data file (no match needed); other skills keep what shipped.
            from_files.append(champion)
        elif champion not in recorded:
            if champion in before:
                champions[champion] = before[champion]
                kept.append(champion)
            else:
                needs.append(champion)
            continue
        for slot in SLOTS:
            if f'{champion} {slot}' in drop:
                continue
            entry = preview_sources.skill(champion, slot, table, rules, assignments, picks)
            skill = built_skill(champion, slot, entry, sheet)
            if skill is None:
                # The recording draws nothing here: keep what was shipped.
                old = (before.get(champion) or {}).get(slot)
                if old is not None:
                    champions.setdefault(champion, {})[slot] = old
                    kept_skills.append(f'{champion} {slot}')
                continue
            champions.setdefault(champion, {})[slot] = skill
            for size in entry['sizes']:
                statuses[size['status']] += 1
                if size['status'] == 'pick':
                    names = ', '.join(c.get('name', 'cast range') for c in size['candidates'])
                    pick.append(f'{champion} {slot} {size["key"]} = {size["value"]}: {names}')
            if any(s['status'] == 'no_numbers' for s in entry['sizes']):
                missing.append(f'{champion} {slot}')
    out = {'about': ABOUT, 'game': table.get('game') or (shipped or {}).get('game'),
           'champions': champions}
    dropped = sorted(d for d in drop if d in before or d.split(' ')[0] in before)
    return out, {'sizes': dict(statuses), 'pick': pick, 'missing_numbers': missing,
                 'kept': kept, 'kept_skills': kept_skills, 'needs_match': needs,
                 'from_files': from_files, 'dropped': dropped}


def differences(old, new):
    """Every champion and skill whose shipped entry differs: lines."""
    a = (old or {}).get('champions') or {}
    b = (new or {}).get('champions') or {}
    lines = []
    for champion in sorted(set(a) | set(b)):
        if champion not in a:
            lines.append(f'{champion}: added ({", ".join(s for s in SLOTS if s in b[champion])})')
        elif champion not in b:
            lines.append(f'{champion}: removed')
        else:
            changed = [f'{slot} {"added" if slot not in a[champion] else "removed" if slot not in b[champion] else "changed"}'
                       for slot in SLOTS if a[champion].get(slot) != b[champion].get(slot)]
            if changed:
                lines.append(f'{champion}: ' + ', '.join(changed))
    return lines


def summary(shipped, report, wrote):
    """The one-line result the viewer shows."""
    files = f'from data files {len(report["from_files"])}; ' if report['from_files'] else ''
    return (f'Preview table: {len(shipped["champions"])} champions; '
            f'kept from the shipped table {len(report["kept"])}; {files}'
            f'needs a match {len(report["needs_match"])}; {wrote}')


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--check', action='store_true',
                        help='write nothing; exit 1 when the shipped table differs from a build')
    parser.add_argument('--drop', action='append', default=[], metavar='CHAMPION',
                        help='remove this champion (or "<champion> <Q|W|R>") from the table')
    args = parser.parse_args()
    if args.check and args.drop:
        sys.exit('--check and --drop do not go together.')
    recorded_table = viewer.read_json(viewer.TABLE)
    if not isinstance(recorded_table, dict):
        recorded_table = {}
    # A recording is what a match recorded, never a data-file card.
    recorded = set(recorded_table.get('champions') or {})
    current = viewer.read_json(OUTPUT)
    if not isinstance(current, dict):
        sys.exit(f'Could not read the shipped table {OUTPUT}: restore it from git before building.')
    game = viewer.game_data()  # also fills viewer.RULES and viewer.FILE_CARDS
    sheet = game[0]
    # Champions no match has recorded: their cards from the data files.
    table = viewer.with_file_cards(recorded_table, game[3])
    if not viewer.RULES:
        sys.exit('Could not read the balance patch rules from the game bundle.')
    overrides = viewer.read_json(viewer.OVERRIDES)
    assignments = overrides.get('skills') if isinstance(overrides, dict) else {}
    shipped, report = build(table, viewer.RULES, assignments, viewer.read_picks(), sheet,
                            shipped=current, recorded=recorded, drop=args.drop)
    unknown = sorted(set(args.drop) - set(report['dropped']))
    if unknown:
        sys.exit(f'Not in the shipped table, nothing dropped: {", ".join(unknown)}')
    sizes = report['sizes']
    print(f'{len(shipped["champions"])} champions (before: {len(current.get("champions") or {})}), '
          f'game {shipped["game"]}. Sizes: '
          + ', '.join(f'{n} {status}' for status, n in sorted(sizes.items())))
    if report['kept']:
        print(f'\nkept from the shipped table (no recording): {len(report["kept"])}')
        print('  ' + ', '.join(report['kept']))
    if report['from_files']:
        print(f'\nbuilt from the champion\'s data file (pack enabled, no recording): {len(report["from_files"])}')
        print('  ' + ', '.join(report['from_files']))
    if report['kept_skills']:
        print(f'\nskills kept from the shipped table (nothing builds them now): {len(report["kept_skills"])}')
        print('  ' + ', '.join(report['kept_skills']))
    if report['needs_match']:
        print(f'\nneeds one match with the champion in play: {len(report["needs_match"])}')
        print('  ' + ', '.join(report['needs_match']) + ' (assigned, but not recorded and not shipped: nothing shipped)')
    if report['dropped']:
        print(f'\ndropped on request: {", ".join(report["dropped"])}')
    if report['pick']:
        print(f'\n{len(report["pick"])} size(s) where two numbers fit (pick one in the viewer):')
        print('\n'.join('  ' + p for p in report['pick']))
    if report['missing_numbers']:
        print(f'\n{len(report["missing_numbers"])} skill(s) whose numbers no match has recorded yet '
              '(their sizes are fixed for now):')
        print('  ' + ', '.join(report['missing_numbers']))
    changed = differences(current, shipped)
    print()
    if args.check:
        if changed:
            print(f'The shipped table is stale: {len(changed)} champion(s) differ from what a build would write.')
            print('\n'.join('  ' + line for line in changed))
            print('Run: python tools/build_preview_table.py')
            print(summary(shipped, report, 'STALE'))
            sys.exit(1)
        print(summary(shipped, report, 'up to date'))
        return
    if not changed:
        print(summary(shipped, report, 'up to date, nothing written'))
        return
    print('\n'.join(changed))
    # Whole or not at all: a temporary file, then renamed over the table.
    temp = OUTPUT.with_name(OUTPUT.name + '.tmp')
    temp.write_text(json.dumps(shipped, indent=1, ensure_ascii=False) + '\n', encoding='utf-8', newline='\n')
    temp.replace(OUTPUT)
    print(summary(shipped, report, f'wrote {OUTPUT.name} ({len(changed)} changed)'))


if __name__ == '__main__':
    main()
