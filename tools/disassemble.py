"""Read-only disassembly for build-specific integration research; never patches."""
from paths import GAME_EXE
import argparse
import bisect
import json
import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / '.tools' / 'python'))
import capstone
import pefile


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--exe', type=Path, default=GAME_EXE)
    parser.add_argument('--rva', type=lambda value: int(value, 0), action='append', default=[])
    parser.add_argument('--anchor', default='game-view/src/view/game.rs')
    parser.add_argument('--output', type=Path, default=ROOT / 'research' / 'view-game.asm')
    args = parser.parse_args()
    pe = pefile.PE(str(args.exe), fast_load=True)
    base = pe.OPTIONAL_HEADER.ImageBase
    sections = {section.Name.rstrip(b'\0').decode(): section for section in pe.sections}
    pdata = sections['.pdata'].get_data()
    functions = sorted((begin, end) for begin, end, _ in struct.iter_unpack('<III', pdata[:len(pdata)//12*12]) if begin and end)
    starts = [begin for begin, _ in functions]
    mapping = json.loads((ROOT / 'research' / 'game-0.6.2-map.json').read_text())
    targets = args.rva or [int(fn['rva'], 16) for fn in mapping['candidate_functions'] if any(anchor['path'] == args.anchor for anchor in fn['anchors'])]
    disassembler = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_64)
    disassembler.detail = True
    output = []
    for target in targets:
        index = bisect.bisect_right(starts, target) - 1
        if index < 0 or not functions[index][0] <= target < functions[index][1]:
            raise ValueError(f'No enclosing function for {target:#x}')
        begin, end = functions[index]
        output.append(f'FUNCTION {begin:#x}..{end:#x}')
        for insn in disassembler.disasm(pe.get_data(begin, end-begin), base+begin):
            annotations = []
            for operand in insn.operands:
                if operand.type == capstone.x86.X86_OP_MEM and operand.mem.base == capstone.x86.X86_REG_RIP:
                    address = insn.address + insn.size + operand.mem.disp - base
                    annotations.append(f'rip_target={address:#x}')
                    raw = pe.get_data(address, 120)
                    printable = raw.split(b'\0')[0]
                    if len(printable) >= 4 and all(32 <= byte < 127 for byte in printable):
                        annotations.append(repr(printable.decode()))
            output.append(f'{insn.address-base:08x} {insn.bytes.hex():24s} {insn.mnemonic:8s} {insn.op_str}' + (' ; ' + ', '.join(annotations) if annotations else ''))
    args.output.write_text('\n'.join(output), encoding='utf-8')
    print(json.dumps({'functions': len(targets), 'lines': len(output), 'output': str(args.output)}))


if __name__ == '__main__':
    main()
