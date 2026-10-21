"""Write the skill preview table the mod ships (step 4 of the agreed preview
plan, stage A; docs/backlog.md item 2).

    python tools/build_preview_table.py            # writes probe/src/preview_table.json
    python tools/build_preview_table.py --check    # only reports, writes nothing

It reads what tools/preview_viewer.py shows: the shapes and numbers the mod
recorded in play (research/skill-shapes.json in the mod's folder), the
author's assignments (skill-overrides.json) and picks (skill-sources.json),
and the game bundle's balance patch rules. For every skill the mod can draw
it writes the shape, with each size replaced by where it comes from
(tools/preview_sources.py): one of the game's numbers by name, the live cast
range, or a fixed number.

The report lists what still needs the author: sizes where two numbers fit,
and skills whose numbers no match has recorded yet. Stage B of the plan
compiles the table into the mod; until then the mod does not read it.
"""
import argparse
import json
import sys
from collections import Counter

import preview_sources
import preview_viewer as viewer

OUTPUT = viewer.ROOT / 'probe' / 'src' / 'preview_table.json'


def build(table, rules, assignments, picks, sheet=None):
    """The shipped table and its report. `sheet` holds the game's own
    numbers per champion and slot (the bundle's champion sheet), for the
    few fixed numbers the recorded shapes lack: Dancer R's most blades."""
    skills = preview_sources.all_skills(table, rules, assignments, picks)
    champions, statuses, pick, missing = {}, Counter(), [], []
    for champion, slots in skills.items():
        for slot, entry in slots.items():
            kept = {'from': entry['from']}
            if entry['from'] == 'assigned':
                kept['assigned'] = preview_sources.with_sources(entry['assigned'], entry['sizes'], 'assigned.')
                if entry.get('shapes'):
                    kept['shapes'] = [preview_sources.with_sources(shape, entry['sizes'], f'{i}.')
                                      for i, shape in enumerate(entry['shapes'])]
            elif entry['from'] in ('live', 'file'):
                kept['shapes'] = [preview_sources.with_sources(shape, entry['sizes'], f'{i}.')
                                  for i, shape in enumerate(entry['shapes'])]
            elif entry['from'] != 'no_area':
                # Passives and unread skills: the mod's cast-style cue.
                continue
            most = ((sheet or {}).get(champion) or {}).get(slot, {}).get('max_count')
            for shape in kept.get('shapes', []):
                # Fanned blades grow with the caster's stack up to this many.
                if shape.get('kind') == 'fan' and isinstance(most, (int, float)):
                    shape['max_count'] = {'fixed': most}
            if entry.get('kind'):
                kept['kind'] = entry['kind']
            champions.setdefault(champion, {})[slot] = kept
            for size in entry['sizes']:
                statuses[size['status']] += 1
                if size['status'] == 'pick':
                    names = ', '.join(c.get('name', 'cast range') for c in size['candidates'])
                    pick.append(f'{champion} {slot} {size["key"]} = {size["value"]}: {names}')
            if any(s['status'] == 'no_numbers' for s in entry['sizes']):
                missing.append(f'{champion} {slot}')
    shipped = {
        'about': 'Skill preview shapes the mod ships, written by tools/build_preview_table.py from the '
                 'shapes and numbers recorded in play, the author\'s assignments and picks. Sizes: '
                 '{"name"} is the game\'s number by name (distances divided by 1000, times "times"), '
                 '{"cast_range"} the live cast range, {"fixed"} a number in world units; "recorded" is '
                 'the value when it was recorded. Do not edit by hand: change the assignments or picks '
                 'in tools/preview_viewer.py and run the tool again.',
        'game': table.get('game'),
        'champions': champions,
    }
    return shipped, {'sizes': dict(statuses), 'pick': pick, 'missing_numbers': missing}


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--check', action='store_true', help='report only, write nothing')
    args = parser.parse_args()
    table = viewer.read_json(viewer.TABLE)
    if not isinstance(table, dict) or not table.get('champions'):
        sys.exit(f'No recorded shapes in {viewer.TABLE}: play a match with the mod first.')
    sheet = viewer.game_data()[0]  # also fills viewer.RULES from the game bundle
    if not viewer.RULES:
        sys.exit('Could not read the balance patch rules from the game bundle.')
    overrides = viewer.read_json(viewer.OVERRIDES)
    assignments = overrides.get('skills') if isinstance(overrides, dict) else {}
    shipped, report = build(table, viewer.RULES, assignments, viewer.read_picks(), sheet)
    sizes = report['sizes']
    print(f'{len(shipped["champions"])} champions, game {shipped["game"]}. Sizes: '
          + ', '.join(f'{n} {status}' for status, n in sorted(sizes.items())))
    if report['pick']:
        print(f'\n{len(report["pick"])} size(s) where two numbers fit (pick one in the viewer):')
        print('\n'.join('  ' + p for p in report['pick']))
    if report['missing_numbers']:
        print(f'\n{len(report["missing_numbers"])} skill(s) whose numbers no match has recorded yet '
              '(their sizes are fixed for now):')
        print('  ' + ', '.join(report['missing_numbers']))
    if args.check:
        return
    OUTPUT.write_text(json.dumps(shipped, indent=1, ensure_ascii=False) + '\n', encoding='utf-8')
    print(f'\nWrote {OUTPUT.relative_to(viewer.ROOT)}')


if __name__ == '__main__':
    main()
