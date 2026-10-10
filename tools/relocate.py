"""Relocate instruction addresses from one preserved build to another.

For each given old address: find its function, find the corresponding new
function (from a map_functions.py result, else by sampling instruction
context as field_shifts.py does), align the two and print the aligned new
address, whether the bytes there are identical and, if not, both
instruction texts. Review aid only; reads the executables and prints JSON
to --output when given.

    python tools/relocate.py --old research/game-builds/0.6.3 \
        --new research/game-builds/0.7.0 \
        --map research/game-builds/function-map-0.6.3-to-0.7.0.json \
        0x146baf3 0xea216a
"""
import argparse
import difflib
import json
from pathlib import Path

import field_shifts
import patch_migration as pm
from compare_functions import normalised


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--old', type=Path, required=True)
    parser.add_argument('--new', type=Path, required=True)
    parser.add_argument('--map', type=Path)
    parser.add_argument('--pair', action='append', default=[], help='known old=new function')
    parser.add_argument('--output', type=Path)
    parser.add_argument('rva', type=lambda v: int(v, 0), nargs='+')
    args = parser.parse_args()
    old = pm.Image(args.old / 'TeamfightManager2.exe')
    new = pm.Image(args.new / 'TeamfightManager2.exe')
    known = {}
    if args.map:
        known = {int(a, 16): int(row['new'], 16)
                 for a, row in json.loads(args.map.read_text(encoding='utf-8'))['pairs'].items()}
    for pair in args.pair:
        a, b = (int(v, 0) for v in pair.split('='))
        known[old.enclosing(a)[0]] = new.enclosing(b)[0]
    aligned, result = {}, {}
    for rva in args.rva:
        bounds = old.enclosing(rva)
        if bounds is None:
            print(f'{rva:#x}: no function with unwind data (leaf or data)')
            result[hex(rva)] = None
            continue
        start = bounds[0]
        if start not in aligned:
            how = 'map'
            target = known.get(start)
            if target is None:
                votes = field_shifts.locate(old, new, start)
                how = 'sampled ' + ', '.join(f'{f:#x} x{n}' for f, n in votes)
                target = votes[0][0] if votes else None
            if target is None:
                aligned[start] = (None, {}, how, 0, 0)
            else:
                x = normalised(old, old.decoded(bounds))
                y = normalised(new, new.decoded(new.enclosing(target)))
                matcher = difflib.SequenceMatcher(None, [t for _, t in x], [t for _, t in y], autojunk=False)
                mapping, equal = {}, 0
                for block in matcher.get_matching_blocks():
                    equal += block.size
                    for k in range(block.size):
                        mapping[x[block.a + k][0]] = y[block.b + k][0]
                aligned[start] = (target, mapping, how, equal, len(x))
        target, mapping, how, equal, size = aligned[start]
        if target is None:
            print(f'{rva:#x}: function {start:#x} not located ({how})')
            result[hex(rva)] = None
            continue
        where = mapping.get(rva)
        line = f'{rva:#x}: fn {start:#x} -> {target:#x} ({equal}/{size}, {how}) '
        if where is None:
            print(line + 'NOT ALIGNED')
        else:
            same = old.pe.get_data(rva, 4) == new.pe.get_data(where, 4)
            print(line + f'-> {where:#x}' + ('' if same else ' (bytes differ at start)'))
        result[hex(rva)] = {'function': hex(target), 'new': hex(where) if where else None}
    if args.output:
        args.output.write_text(json.dumps(result, indent=1), encoding='utf-8')


if __name__ == '__main__':
    main()
