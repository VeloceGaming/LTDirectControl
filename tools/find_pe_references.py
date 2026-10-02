"""Read-only string and RIP-reference research; does not identify safe hooks."""
import argparse
import bisect
import json
import re
import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / '.tools' / 'python'))
import pefile


def inspect(path, needles):
    pe = pefile.PE(str(path), fast_load=True)
    sections = {s.Name.rstrip(b'\0').decode(): s for s in pe.sections}
    pdata = sections['.pdata'].get_data()
    functions = sorted((start, end) for start, end, _ in
                       struct.iter_unpack('<III', pdata[:len(pdata)//12*12]) if start and end)
    starts = [start for start, _ in functions]
    data = path.read_bytes()
    targets = {}
    for needle in needles:
        offset = 0
        while True:
            offset = data.find(needle.encode(), offset)
            if offset < 0:
                break
            rva = pe.get_rva_from_offset(offset)
            targets[rva] = {'needle': needle, 'rva': hex(rva), 'references': []}
            offset += len(needle)
    section = sections['.text']
    code = section.get_data()
    for hit in re.finditer(rb'[\x48\x4c]\x8d[\x05\x0d\x15\x1d\x25\x2d\x35\x3d].{4}', code, re.DOTALL):
        insn = section.VirtualAddress + hit.start()
        target = insn + 7 + struct.unpack_from('<i', code, hit.start()+3)[0]
        if target not in targets:
            continue
        i = bisect.bisect_right(starts, insn)-1
        if i >= 0 and functions[i][0] <= insn < functions[i][1]:
            start, end = functions[i]
            targets[target]['references'].append({'instruction_rva': hex(insn), 'function_rva': hex(start), 'end_rva': hex(end)})
    return {'executable': str(path), 'strings': list(targets.values()),
            'warning': 'Static research only. Function roles, data layouts and calling conventions remain unverified.'}


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--exe', type=Path, required=True)
    parser.add_argument('--needle', action='append', required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    result = inspect(args.exe, args.needle)
    args.output.write_text(json.dumps(result, indent=2), encoding='utf-8')
    print(json.dumps(result, indent=2))
