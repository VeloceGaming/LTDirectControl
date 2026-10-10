"""Match effect vtables between two preserved game builds.

A Rust trait-object vtable is [drop, size, align, methods...]. For each old
method address given (the `apply` at +0x20 of an effect vtable), this
collects the old vtables that hold it, then ranks the new build's vtables
of the same size by how well their method at the same slot aligns with the
old one. Review aid only; reads the executables and prints.

    python tools/match_vtables.py --old research/game-builds/0.6.3 \
        --new research/game-builds/0.7.0 0x14294c0 0x1489140
"""
import argparse
import difflib
import struct
from collections import Counter, defaultdict
from pathlib import Path

import patch_migration as pm
from compare_functions import normalised

SLOT = 0x20


def tables(image):
    """size -> Counter(method at SLOT) over every plausible vtable."""
    out = defaultdict(Counter)
    for section in image.pe.sections:
        if section.Name.rstrip(b'\0').decode() != '.rdata':
            continue
        data, start = section.get_data(), section.VirtualAddress
        low, high = image.base + image.text_rva, image.base + image.text_rva + len(image.text)
        for at in range(0, len(data) - SLOT - 8, 8):
            size, align, method = struct.unpack_from('<QQ', data, at + 8) + struct.unpack_from('<Q', data, at + SLOT)
            if align == 8 and 0 < size <= 1024 and size % 8 == 0 and low <= method < high:
                drop = struct.unpack_from('<Q', data, at)[0]
                if drop == 0 or low <= drop < high:
                    out[size][method - image.base] += 1
    return out


def ratio(old, new, a, b):
    x, y = old.enclosing(a), new.enclosing(b)
    if not x or not y or x[0] != a or y[0] != b:
        return 0.
    p = [t for _, t in normalised(old, old.decoded(x))]
    q = [t for _, t in normalised(new, new.decoded(y))]
    if max(len(p), len(q)) > 4 * min(len(p), len(q)):
        return 0.
    return difflib.SequenceMatcher(None, p, q, autojunk=False).ratio()


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--old', type=Path, required=True)
    parser.add_argument('--new', type=Path, required=True)
    parser.add_argument('method', type=lambda v: int(v, 0), nargs='+')
    args = parser.parse_args()
    old = pm.Image(args.old / 'TeamfightManager2.exe')
    new = pm.Image(args.new / 'TeamfightManager2.exe')
    before, after = tables(old), tables(new)
    for method in args.method:
        sizes = [(size, row[method]) for size, row in before.items() if method in row]
        if not sizes:
            print(f'{method:#x}: in no old vtable')
            continue
        for size, count in sizes:
            ranked = sorted(((ratio(old, new, method, candidate), candidate, n)
                             for candidate, n in after[size].items()), reverse=True)[:3]
            print(f'{method:#x}: size {size}, {count} old vtables -> ' + ', '.join(
                f'{candidate:#x} ({score:.2f}, {n} vtables)' for score, candidate, n in ranked))


if __name__ == '__main__':
    main()
