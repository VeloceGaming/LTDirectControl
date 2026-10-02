"""Read-only instruction references in the installed PE; never runs its code.

Candidates are decoded from unwind-table function starts to filter byte-pattern
false positives. These references do not establish safe hooks or calling ABIs.
"""
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


class PeResearch:
    def __init__(self, path):
        self.pe = pefile.PE(str(path), fast_load=True)
        self.base = self.pe.OPTIONAL_HEADER.ImageBase
        sections = {s.Name.rstrip(b'\0').decode(): s for s in self.pe.sections}
        self.text = sections['.text']
        self.code = self.text.get_data()
        pd = sections['.pdata'].get_data()
        self.functions = sorted((a, b) for a, b, _ in
                                struct.iter_unpack('<III', pd[:len(pd)//12*12]) if a and b)
        self.starts = [a for a, _ in self.functions]
        self.cs = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_64)
        self.cs.detail = True

    def enclosing(self, rva):
        i = bisect.bisect_right(self.starts, rva) - 1
        if i >= 0 and self.functions[i][0] <= rva < self.functions[i][1]:
            return self.functions[i]

    def decode(self, bounds):
        begin, end = bounds
        return self.cs.disasm(self.pe.get_data(begin, end-begin), self.base+begin)

    def displacements(self, displacement):
        needle = struct.pack('<i', displacement)
        candidates = set()
        offset = 0
        while True:
            offset = self.code.find(needle, offset)
            if offset < 0:
                break
            bounds = self.enclosing(self.text.VirtualAddress+offset)
            if bounds:
                candidates.add(bounds)
            offset += 1
        result = []
        for bounds in sorted(candidates):
            refs = []
            for insn in self.decode(bounds):
                if any(op.type == capstone.x86.X86_OP_MEM and
                       op.mem.base != capstone.x86.X86_REG_RIP and
                       op.mem.disp == displacement for op in insn.operands):
                    refs.append({'rva': hex(insn.address-self.base),
                                 'asm': f'{insn.mnemonic} {insn.op_str}'})
            if refs:
                result.append({'function': hex(bounds[0]), 'size': bounds[1]-bounds[0],
                               'references': refs})
        return result

    def callers(self, target):
        import re
        candidates = set()
        for hit in re.finditer(rb'[\xe8\xe9]', self.code):
            offset = hit.start()
            if offset+5 > len(self.code):
                continue
            rva = self.text.VirtualAddress+offset
            if rva+5+struct.unpack_from('<i', self.code, offset+1)[0] == target:
                bounds = self.enclosing(rva)
                if bounds:
                    candidates.add(bounds)
        result = []
        for bounds in sorted(candidates):
            refs = [hex(i.address-self.base) for i in self.decode(bounds)
                    if i.mnemonic in ('call', 'jmp') and i.operands and
                    i.operands[0].type == capstone.x86.X86_OP_IMM and
                    i.operands[0].imm-self.base == target]
            if refs:
                result.append({'function': hex(bounds[0]), 'size': bounds[1]-bounds[0],
                               'calls': refs})
        return result

    def rip_references(self, target):
        """Find decoded RIP-relative references to a code/data RVA."""
        import re
        candidates = set()
        for hit in re.finditer(rb'[\x48\x4c]\x8d[\x05\x0d\x15\x1d\x25\x2d\x35\x3d].{4}', self.code, re.S):
            rva = self.text.VirtualAddress + hit.start()
            if rva + 7 + struct.unpack_from('<i', self.code, hit.start()+3)[0] == target:
                bounds = self.enclosing(rva)
                if bounds:
                    candidates.add(bounds)
        result = []
        for bounds in sorted(candidates):
            refs = []
            for insn in self.decode(bounds):
                if any(op.type == capstone.x86.X86_OP_MEM and
                       op.mem.base == capstone.x86.X86_REG_RIP and
                       insn.address + insn.size + op.mem.disp == self.base + target
                       for op in insn.operands):
                    refs.append({'rva': hex(insn.address-self.base),
                                 'asm': f'{insn.mnemonic} {insn.op_str}'})
            if refs:
                result.append({'function': hex(bounds[0]), 'size': bounds[1]-bounds[0],
                               'references': refs})
        return result

    def pointers(self, target):
        needle = struct.pack('<Q', self.base + target)
        data = self.pe.__data__
        result = []
        offset = 0
        while True:
            offset = data.find(needle, offset)
            if offset < 0:
                break
            rva = self.pe.get_rva_from_offset(offset)
            result.append({'pointer_rva': hex(rva), 'nearby_table_references': self.rip_references(rva-24)})
            offset += 1
        return result


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--exe', type=Path, default=Path(r'C:\Program Files (x86)\Steam\steamapps\common\Teamfight Manager2\TeamfightManager2.exe'))
    parser.add_argument('--disp', type=lambda x: int(x, 0))
    parser.add_argument('--call', type=lambda x: int(x, 0))
    parser.add_argument('--rip', type=lambda x: int(x, 0))
    parser.add_argument('--pointer', type=lambda x: int(x, 0))
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    pe = PeResearch(args.exe)
    if args.pointer is not None:
        result = pe.pointers(args.pointer)
    elif args.disp is not None:
        result = pe.displacements(args.disp)
    elif args.rip is not None:
        result = pe.rip_references(args.rip)
    else:
        result = pe.callers(args.call)
    args.output.write_text(json.dumps(result, indent=2), encoding='utf-8')
    print(json.dumps(result if args.pointer is not None else
                     [{'function': x['function'], 'size': x['size'],
                       'references': len(x.get('references', x.get('calls', [])))} for x in result], indent=2))
