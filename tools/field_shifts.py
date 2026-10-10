"""Which object-field offsets moved between two preserved game builds.

For each given function pair the instructions are aligned with every memory
displacement masked (`[reg + N]`, not stack slots), then the real
displacements of each aligned pair are compared. A field that moved shows
up as old -> new with the number of instructions that agree. Review aid
only; it reads the two executables and prints.

    python tools/field_shifts.py --old research/game-builds/0.6.3 \
        --new research/game-builds/0.7.0 0x1703fa0=0x1562680 0x16f3e30=0x1551cc0

With `--sample OLD_FUNCTION` instead of pairs, the new function is looked
up by instruction context first (as tools/find_context.py does).
"""
import argparse
import difflib
import re
from collections import Counter
from pathlib import Path

import find_context
import patch_migration as pm
from compare_functions import normalised

FIELD = re.compile(r'\[(r(?!bp\b|sp\b|ip\b)\w+)((?: \+ r\w+(?:\*\d)?)?) ([+-]) (0x[0-9a-f]+|\d+)\]')


def masked(rows):
    return [FIELD.sub(lambda m: f'[{m.group(1)}{m.group(2)} + FIELD]', text) for _, text in rows]


def displacements(text):
    return [int(m.group(4), 0) * (1 if m.group(3) == '+' else -1) for m in FIELD.finditer(text)]


def locate(old, new, start):
    """The new function most sampled instructions of the old one land in."""
    instructions = old.decoded(old.enclosing(start))
    votes = Counter()
    for i in instructions[3::max(1, len(instructions) // 30)]:
        _, found = find_context.search(old, new, i.address - old.base, [(8, 10), (5, 5), (3, 3)])
        for _, bounds in found:
            votes[bounds[0]] += 1
    return votes.most_common(3)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--old', type=Path, required=True)
    parser.add_argument('--new', type=Path, required=True)
    parser.add_argument('pairs', nargs='*', help='old=new function addresses')
    parser.add_argument('--sample', type=lambda v: int(v, 0), action='append', default=[])
    parser.add_argument('--minimum', type=int, default=1)
    args = parser.parse_args()
    old = pm.Image(args.old / 'TeamfightManager2.exe')
    new = pm.Image(args.new / 'TeamfightManager2.exe')
    pairs = [tuple(int(v, 0) for v in pair.split('=')) for pair in args.pairs]
    for start in args.sample:
        votes = locate(old, new, start)
        print(f'{old.enclosing(start)[0]:#x} sampled: ' + ', '.join(f'{f:#x} x{n}' for f, n in votes))
        if votes:
            pairs.append((start, votes[0][0]))
    total = Counter()
    for a, b in pairs:
        x = normalised(old, old.decoded(old.enclosing(a)))
        y = normalised(new, new.decoded(new.enclosing(b)))
        matcher = difflib.SequenceMatcher(None, masked(x), masked(y), autojunk=False)
        equal, here = 0, Counter()
        for block in matcher.get_matching_blocks():
            equal += block.size
            for k in range(block.size):
                for before, after in zip(displacements(x[block.a + k][1]), displacements(y[block.b + k][1])):
                    here[(before, after)] += 1
        total.update(here)
        moved = sorted((k, n) for k, n in here.items() if k[0] != k[1])
        print(f'{old.enclosing(a)[0]:#x} -> {new.enclosing(b)[0]:#x}: {equal}/{len(x)} aligned (fields masked); '
              f'{sum(n for k, n in here.items() if k[0] == k[1])} field uses unchanged, '
              f'{sum(n for _, n in moved)} moved')
    print('--- moved fields over all pairs (old -> new: instructions)')
    by_old = {}
    for (before, after), n in total.items():
        by_old.setdefault(before, Counter())[after] += n
    for before in sorted(by_old):
        row = by_old[before]
        if any(after != before for after in row) and sum(row.values()) >= args.minimum:
            print(f'  {before:#7x} -> ' + ', '.join(f'{after:#x} x{n}' for after, n in row.most_common()))


if __name__ == '__main__':
    main()
