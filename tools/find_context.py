"""Find where an instruction of one preserved build went in another by its
surrounding instructions, trying smaller and smaller windows.

patch_migration compare uses one fixed window (16 before, 24 after) and
gives up when anything in it changed. This tries narrower windows around
the same instruction and lists the new functions that contain a match, so
the enclosing functions can then be aligned with compare_functions.py.
Review aid only; it reads the two executables and prints.

    python tools/find_context.py --old research/game-builds/0.6.3 \
        --new research/game-builds/0.7.0 0x23228d4 0x2322906
"""
import argparse
from pathlib import Path

import patch_migration as pm


def search(old, new, rva, windows):
    bounds = old.enclosing(rva)
    if bounds is None:
        return None, []
    instructions = old.decoded(bounds)
    positions = [i.address - old.base for i in instructions]
    if rva not in positions:
        return None, []
    index = positions.index(rva)
    for before, after in windows:
        context = instructions[max(0, index - before):index + after + 1]
        value, mask = pm.masked_signature(context, old.base, old.image_size)
        offset = rva - (context[0].address - old.base)
        hits = pm.pattern_hits(new.text, value, mask)
        if 0 < len(hits) <= 8:
            found = []
            for hit in hits:
                candidate = new.text_rva + hit + offset
                parent = new.enclosing(candidate)
                if parent and any(i.address - new.base == candidate for i in new.decoded(parent)):
                    found.append((candidate, parent))
            if found:
                return (before, after), found
    return None, []


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--old', type=Path, required=True)
    parser.add_argument('--new', type=Path, required=True)
    parser.add_argument('rva', type=lambda v: int(v, 0), nargs='+')
    args = parser.parse_args()
    old = pm.Image(args.old / 'TeamfightManager2.exe')
    new = pm.Image(args.new / 'TeamfightManager2.exe')
    windows = [(12, 16), (8, 10), (6, 6), (4, 4), (3, 3), (2, 2)]
    for rva in args.rva:
        window, found = search(old, new, rva, windows)
        parent = old.enclosing(rva)
        print(f'{rva:#x} (old function {parent[0]:#x})' if parent else f'{rva:#x} (no function)', end=': ')
        if not found:
            print('no window gives 1-8 matches')
            continue
        print(f'window -{window[0]}/+{window[1]}: ' + ', '.join(
            f'{candidate:#x} in {bounds[0]:#x}' for candidate, bounds in found))


if __name__ == '__main__':
    main()
