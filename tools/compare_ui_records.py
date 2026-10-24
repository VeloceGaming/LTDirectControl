"""Compare a window's write record with the dev2 baseline (1.6.0-perf-dev3).

The window identity tests (probe/src/ui_identity.rs) record every write a
window sends to the game's UI tree. When nodes leave the tree for the draw
list, the record shrinks. This check proves the shrink is only that: for
every node still in the tree, frame by frame, the writes must be the same
lines as in the baseline, except the lines listed as expected changes.

    python tools/compare_ui_records.py [window ...]

Baseline: probe/testdata/ui_pixels/dev2/<window>/record.txt (made on the
unchanged 1.6.0-perf-dev2 windows with the dev3 test rig). New record:
probe/testdata/ui_identity/<window>.txt (or .actual.txt when present).
Prints, per window, the lines that left grouped by node, the kept nodes'
lines that changed, and lines that are new. Exit 1 on an unexpected change.
"""
import re
import sys
from collections import Counter, OrderedDict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BASE = ROOT / 'probe/testdata/ui_pixels/dev2'
NEW = ROOT / 'probe/testdata/ui_identity'
WINDOWS = ['hud', 'session', 'stats', 'effect']


def frames(path):
    out, current = [], None
    for line in path.read_text(encoding='utf-8').splitlines():
        if line.startswith('## '):
            current = (line, [])
            out.append(current)
        elif current is not None:
            # A text write's further lines (a wrapped tooltip) belong to it.
            if re.match(r'^([SPTVXR] |# )', line) or not current[1]:
                current[1].append(line)
            else:
                current[1][-1] += '\n' + line
    return out


def node(line):
    """(kind, path) of a record line; kind '#' for the frame's notes."""
    if line.startswith('# '):
        return '#', None
    kind, rest = line[0], line[2:]
    if kind == 'P':
        return kind, rest.split(' | ', 1)[0]
    if kind == 'T':
        return kind, rest.split(' = ', 1)[0]
    return kind, rest.split(' ', 1)[0]


def short(path):
    parts = path.split('.')
    return '.'.join(parts[2:]) or parts[-1]


# Lines that may differ on a kept node, by design (reason, pattern on the
# new line). A changed line is accepted only when one of these matches it.
EXPECTED = [
    ('template changed (nodes left)', re.compile(r'^S ')),
    ('container node: same place and visibility, no colour or button style',
     re.compile(r'^P \S+\.(skill\d|item\d+|next|lane\d|vision_(own|other|all)|lt_state_plate) \| ')),
    ('frame notes (bounds, sounds) — checked separately', re.compile(r'^# ')),
]
# Lines that may leave a kept node's record, by design.
EXPECTED_DROPPED = [
    ('a container lost its colour (the box is drawn now)',
     re.compile(r'^P \S+\.(skill\d|item\d+|next|lane\d|vision_(own|other|all)) \| (color|btn)')),
    ('a lane container is written only when its place or visibility changes (its '
     'border and fill colours, which also keyed the write, are drawn)',
     re.compile(r'^P \S+\.lane\d \| x: .*btn: \{')),
    ('the plate root is written only when its width changes (its colour, which '
     'carried the fade, is drawn)',
     re.compile(r'^P ingame\.lt_state_plate \| x: \d+px; width: \d+px; color: ')),
]
# Lines a kept node's record may gain.
EXPECTED_ADDED = [
    ('the plate root now shows by a visibility write: the fake tree read the old '
     'template header (with a `#~` colour) as visible, the game did not',
     re.compile(r'^V ingame\.lt_state_plate true$')),
]


def compare(window):
    base = frames(BASE / window / 'record.txt')
    new_path = NEW / f'{window}.actual.txt'
    if not new_path.exists():
        new_path = NEW / f'{window}.txt'
    new = frames(new_path)
    assert len(base) == len(new), f'{window}: {len(base)} baseline frames, {len(new)} new'
    kept = {node(l)[1] for _, lines in new for l in lines} - {None}
    kept_prefix = lambda p: p in kept or any(k.startswith(p + '.') for k in kept)
    left = Counter()
    changed = OrderedDict()
    added = Counter()
    same = 0
    unexpected = []
    for (head, old_lines), (head2, new_lines) in zip(base, new):
        assert head == head2, (head, head2)
        old_kept = []
        for l in old_lines:
            kind, path = node(l)
            if kind == '#' or (path and path in kept):
                old_kept.append(l)
            elif path:
                left[(kind, short(path))] += 1
        # Align the kept lines of the frame in order.
        i = j = 0
        while i < len(old_kept) or j < len(new_lines):
            a = old_kept[i] if i < len(old_kept) else None
            b = new_lines[j] if j < len(new_lines) else None
            if a == b:
                same += 1
                i += 1
                j += 1
                continue
            if a is not None and b is not None and node(a) == node(b) or (
                    a and b and a[0] == b[0] == 'S'):
                reason = next((r for r, p in EXPECTED if p.search(b)), None)
                key = (reason or 'UNEXPECTED', node(b)[0], short(node(b)[1]) if node(b)[1] else '#')
                changed.setdefault(key, []).append((head, a, b))
                if reason is None:
                    unexpected.append((head, a, b))
                i += 1
                j += 1
                continue
            # A line only one side has.
            if b is not None and (a is None or node(b) not in [node(x) for x in old_kept[i:]]):
                reason = next((r for r, p in EXPECTED_ADDED if p.search(b)), None)
                added[(reason or 'UNEXPECTED', node(b)[0], short(node(b)[1]) if node(b)[1] else '#')] += 1
                if reason is None:
                    unexpected.append((head, None, b))
                j += 1
            else:
                reason = next((r for r, p in EXPECTED_DROPPED if p.search(a)), None)
                if node(a)[0] == 'R':
                    reason = 'click registration (the mod hit-tests its own buttons now)'
                left[(node(a)[0], f'{short(node(a)[1])} ({reason or "UNEXPECTED"})')] += 1
                if reason is None:
                    unexpected.append((head, a, None))
                i += 1
    print(f'== {window}: {sum(len(l) for _, l in base)} baseline lines, '
          f'{sum(len(l) for _, l in new)} now; {same} identical lines on kept nodes')
    by_node = Counter()
    for (kind, path), n in left.items():
        by_node[re.sub(r'(slot_glow\.b)\d+_\d|(hp_tick)\d+|(charge)\d|(gap)\d_\d', lambda m: (m.group(1) or m.group(2) or m.group(3) or m.group(4)) + '*', path)] += n
    print(f'  left the record ({sum(left.values())} lines), by node:')
    for path, n in sorted(by_node.items(), key=lambda x: -x[1]):
        print(f'    {n:6d}  {path}')
    if changed:
        print('  changed lines on kept nodes:')
        for (reason, kind, path), items in changed.items():
            head, a, b = items[0]
            print(f'    {len(items):6d}  {kind} {path}: {reason}')
            print(f'            e.g. {head}\n              was: {a}\n              now: {b}')
    if added:
        print('  new lines:')
        for (reason, kind, path), n in added.items():
            print(f'    {n:6d}  {kind} {path}: {reason}')
    return unexpected


def main():
    windows = sys.argv[1:] or WINDOWS
    bad = []
    for w in windows:
        bad += [(w, *u) for u in compare(w)]
    if bad:
        print(f'\n{len(bad)} unexpected differences; first ones:')
        for w, head, a, b in bad[:12]:
            print(f'  {w} {head}\n    was: {a}\n    now: {b}')
        return 1
    print('\nRecord check passed: kept nodes receive the same writes as in dev2, '
          'apart from the expected changes listed.')
    return 0


if __name__ == '__main__':
    sys.exit(main())
