"""Print the instructions around an address in a preserved game build.
Review aid only; reads the executable and prints.

    python tools/show_code.py research/game-builds/0.7.0 0xaa89a4 --before 30 --after 6
"""
import argparse
from pathlib import Path

import patch_migration as pm


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('snapshot', type=Path)
    parser.add_argument('rva', type=lambda v: int(v, 0), nargs='+')
    parser.add_argument('--before', type=int, default=20)
    parser.add_argument('--after', type=int, default=8)
    args = parser.parse_args()
    image = pm.Image(args.snapshot / 'TeamfightManager2.exe')
    for rva in args.rva:
        bounds = image.enclosing(rva)
        if bounds is None:
            print(f'{rva:#x}: not inside a function with unwind data')
            continue
        instructions = image.decoded(bounds)
        index = next((n for n, i in enumerate(instructions) if i.address - image.base >= rva), None)
        if index is None:
            continue
        print(f'function {bounds[0]:#x}..{bounds[1]:#x}, around {rva:#x}')
        for i in instructions[max(index - args.before, 0):index + args.after + 1]:
            at = i.address - image.base
            target = ''
            if i.mnemonic in ('call', 'jmp') and i.op_str.startswith('0x'):
                target = f'   ; -> {int(i.op_str, 16) - image.base:#x}'
            print(f"{'>' if at == rva else ' '} {at:#9x}  {i.mnemonic} {i.op_str}{target}")


if __name__ == '__main__':
    main()
