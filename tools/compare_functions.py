"""Compare one function between two preserved game builds, instruction by
instruction. Addresses that relocation changes (relative call/jump targets,
RIP-relative references, image-address immediates) are masked; opcodes,
registers, constants and object-field displacements are compared. Review
aid only: it reads the two executables and prints; nothing is applied.

    python tools/compare_functions.py --old research/game-builds/0.6.3 \
        --new research/game-builds/0.7.0 --old-rva 0x930640 --new-rva 0xb194c0

`--old-rva`/`--new-rva` may be any address inside the function. With
`--at` (old RVA) the matching new instruction is printed when the two
functions align.
"""
import argparse
import difflib
import re
from pathlib import Path

import patch_migration as pm


def normalised(image, instructions):
    rows = []
    for i in instructions:
        text = f'{i.mnemonic} {i.op_str}'.strip()
        # Relative branch/call targets and RIP-relative references move.
        if i.mnemonic.startswith(('j', 'call', 'loop')) and re.fullmatch(r'0x[0-9a-f]+', i.op_str):
            text = f'{i.mnemonic} <code>'
        text = re.sub(r'rip [+-] 0x[0-9a-f]+', 'rip + <ref>', text)

        def image_address(match):
            value = int(match.group(0), 16)
            return '<image>' if image.base <= value < image.base + image.image_size else match.group(0)
        text = re.sub(r'0x[0-9a-f]{8,}', image_address, text)
        rows.append((i.address - image.base, text))
    return rows


def function(image, rva):
    bounds = image.enclosing(rva)
    if bounds is None:
        raise SystemExit(f'{rva:#x} is not inside a function with unwind data')
    return bounds, normalised(image, image.decoded(bounds))


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--old', type=Path, required=True)
    parser.add_argument('--new', type=Path, required=True)
    parser.add_argument('--old-rva', type=lambda v: int(v, 0), required=True)
    parser.add_argument('--new-rva', type=lambda v: int(v, 0), required=True)
    parser.add_argument('--at', type=lambda v: int(v, 0), action='append', default=[])
    parser.add_argument('--context', type=int, default=0)
    parser.add_argument('--limit', type=int, default=60)
    args = parser.parse_args()
    old = pm.Image(args.old / 'TeamfightManager2.exe')
    new = pm.Image(args.new / 'TeamfightManager2.exe')
    (old_bounds, a), (new_bounds, b) = function(old, args.old_rva), function(new, args.new_rva)
    print(f'old {old_bounds[0]:#x}..{old_bounds[1]:#x} {len(a)} instructions | '
          f'new {new_bounds[0]:#x}..{new_bounds[1]:#x} {len(b)} instructions')
    matcher = difflib.SequenceMatcher(None, [t for _, t in a], [t for _, t in b], autojunk=False)
    equal = sum(block.size for block in matcher.get_matching_blocks())
    print(f'matching instructions: {equal}/{len(a)} old, {equal}/{len(b)} new')
    mapping = {}
    shown = 0
    for tag, i1, i2, j1, j2 in matcher.get_opcodes():
        if tag == 'equal':
            for k in range(i2 - i1):
                mapping[a[i1 + k][0]] = b[j1 + k][0]
            continue
        if shown >= args.limit:
            continue
        shown += 1
        print(f'--- {tag}')
        for rva, text in a[max(i1 - args.context, 0):i2]:
            print(f'  old {rva:#9x}  {text}')
        for rva, text in b[max(j1 - args.context, 0):j2]:
            print(f'  new {rva:#9x}  {text}')
    for rva in args.at:
        print(f'old {rva:#x} -> new ' + (f'{mapping[rva]:#x}' if rva in mapping else 'no aligned instruction'))


if __name__ == '__main__':
    main()
