"""Which champion skills have a real preview, from the mod's logs.

The mod writes, for every champion of a watched match, what its preview
reader made of each skill (PREVIEW COVERAGE, by actor; the PREVIEW ROSTER
line before it names the actors) and what the champion's definition file
alone gives (PREVIEW DECLARED). This joins them the way the mod does: a
shape read from the live skill wins, else a declared shape, else whether
the skill is known to have no area.

Each skill ends up as one of:
  a shape      strip, circle, ally-circle, cone, rectangle, dash, blink, ...
  no-area      every effect is recognised and none has an area
  unknown      not understood: the crosshair fallback is drawn

It says whether a shape exists, not whether it is right. Reads logs only.

    python tools/audit_previews.py            # the mod's log folder
    python tools/audit_previews.py --logs a.log b.log --output report.json
"""
import argparse
import json
import os
import re
from pathlib import Path


def logs_default():
    root = Path(os.environ.get('LOCALAPPDATA', '')) / 'LTDirectControl'
    return sorted(root.glob('probe*.log')) + sorted(root.glob('*/probe*.log'))


def read(paths):
    found, reasons = {}, {}
    for path in paths:
        roster, declared = {}, {}
        for line in path.read_text(encoding='utf-8', errors='replace').splitlines():
            m = re.search(r'PREVIEW ROSTER .*actors=\[(.*)\]', line)
            if m:
                roster = dict(p.split('=', 1) for p in m.group(1).split(', ') if '=' in p)
                continue
            m = re.search(r'PREVIEW DECLARED champion=(\S+) Q=(\S+) W=(\S+) R=(\S+)', line)
            if m:
                declared[m.group(1)] = m.groups()[1:]
                continue
            m = re.search(r'PREVIEW COVERAGE actor=(\d+) Q=(\S+) W=(\S+) R=(\S+)', line)
            if m and roster.get(m.group(1)):
                champion = roster[m.group(1)]
                for slot, live, file in zip('QWR', m.groups()[1:], declared.get(champion, ('absent',) * 3)):
                    if live == 'absent':
                        continue
                    shaped = lambda v: not v.startswith(('unknown', 'no-area', 'absent'))
                    if shaped(live):
                        result = live
                    elif shaped(file):
                        result = file
                    elif live == 'no-area':
                        result = 'no-area'
                    else:
                        result = 'unknown'
                        reasons[(champion, slot)] = live.partition(':')[2].replace('_', ' ')
                    found.setdefault(champion, {})[slot] = result
    return found, reasons


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--logs', type=Path, nargs='*')
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    found, reasons = read(args.logs or logs_default())
    if not found:
        print('No PREVIEW COVERAGE lines found (they are written from mod 1.1.17 on, by research '
              'builds from 1.5.0-dev18, and only at Log detail Verbose from 1.5.0-dev20).')
        return
    width = max(map(len, found)) + 2
    print(f"{'champion':<{width}}{'Q':<22}{'W':<22}{'R':<22}")
    for champion in sorted(found):
        row = found[champion]
        print(f'{champion:<{width}}' + ''.join(f"{row.get(s, '-'):<22}" for s in 'QWR'))
    unknown = sorted(reasons)
    skills = sum(len(r) for r in found.values())
    print(f'\n{len(found)} champions, {skills} skills seen; {len(unknown)} not understood:')
    for champion, slot in unknown:
        print(f'  {champion} {slot}: {reasons[(champion, slot)]}')
    if args.output:
        args.output.write_text(json.dumps({'champions': found, 'unknown': [
            {'champion': c, 'slot': s, 'reason': reasons[(c, s)]} for c, s in unknown]}, indent=1), encoding='utf-8')


if __name__ == '__main__':
    main()
