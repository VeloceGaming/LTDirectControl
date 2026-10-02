"""Read SDK archive symbols and COFF objects, without loading game code.

Some SDK archive members are LLVM bitcode rather than COFF. Their archive-index
names can be inspected; this tool deliberately does not decode their bodies.
"""
import argparse
import re
import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / '.tools' / 'python'))
import capstone


def index(path):
    with Path(path).open('rb') as f:
        assert f.read(8) == b'!<arch>\n'
        header = f.read(60)
        assert header[:16].strip() == b'/'
        data = f.read(int(header[48:58]))
    count = struct.unpack_from('>I', data)[0]
    offsets = struct.unpack_from('>' + str(count) + 'I', data, 4)
    names = data[4 + 4 * count:].split(b'\0')
    return [(offset, name.decode(errors='replace')) for offset, name in zip(offsets, names)]


class Coff:
    def __init__(self, archive, offset):
        with Path(archive).open('rb') as f:
            f.seek(offset)
            header = f.read(60)
            self.data = data = f.read(int(header[48:58]))
        if data[:4] == b'BC\xc0\xde':
            raise ValueError('Member is LLVM bitcode, not COFF; use archive-index mode for names only.')
        big = data[:4] == b'\0\0\xff\xff'
        if big:
            nsec, psym, nsym = struct.unpack_from('<III', data, 44)
            section_start, symbol_size = 56, 20
        else:
            nsec = struct.unpack_from('<H', data, 2)[0]
            psym, nsym = struct.unpack_from('<II', data, 8)
            section_start = 20 + struct.unpack_from('<H', data, 16)[0]
            symbol_size = 18
        strings = data[psym + nsym * symbol_size:]

        def name(raw):
            if raw[:4] == b'\0\0\0\0':
                pos = struct.unpack_from('<I', raw, 4)[0]
                return strings[pos:].split(b'\0', 1)[0].decode(errors='replace')
            return raw.rstrip(b'\0').decode(errors='replace')

        self.symbols = {}
        i = 0
        while i < nsym:
            pos = psym + i * symbol_size
            value = struct.unpack_from('<I', data, pos + 8)[0]
            if big:
                sec, typ, storage, aux = struct.unpack_from('<iHBB', data, pos + 12)
            else:
                sec, typ, storage, aux = struct.unpack_from('<hHBB', data, pos + 12)
            self.symbols[i] = dict(name=name(data[pos:pos+8]), value=value,
                                   section=sec, type=typ, storage=storage)
            i += 1 + aux
        self.sections = []
        for i in range(nsec):
            pos = section_start + i * 40
            size, raw, reloc = struct.unpack_from('<III', data, pos + 16)
            nreloc = struct.unpack_from('<H', data, pos + 32)[0]
            flags = struct.unpack_from('<I', data, pos + 36)[0]
            start = 0
            if nreloc == 65535 and flags & 0x1000000:
                nreloc = struct.unpack_from('<I', data, reloc)[0]
                start = 1
            rels = []
            for j in range(start, nreloc):
                addr, sym, typ = struct.unpack_from('<IIH', data, reloc + j * 10)
                rels.append((addr, self.symbols.get(sym, {}).get('name', str(sym)), typ))
            self.sections.append(dict(size=size, raw=raw, flags=flags, relocations=rels))

    def disassemble(self, symbol):
        sec = self.sections[symbol['section']-1]
        code = self.data[sec['raw']:sec['raw']+sec['size']]
        cs = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_64)
        lines = [f"FUNCTION {symbol['name']} section={symbol['section']} size={sec['size']}"]
        for ins in cs.disasm(code[symbol['value']:], symbol['value']):
            refs = [f'{typ}: {name}' for addr, name, typ in sec['relocations']
                    if ins.address <= addr < ins.address + ins.size]
            lines.append(f'{ins.address:08x} {ins.mnemonic:8} {ins.op_str}' +
                         (' ; ' + ' | '.join(refs) if refs else ''))
        return '\n'.join(lines)


def main():
    p = argparse.ArgumentParser()
    p.add_argument('--archive', required=True)
    p.add_argument('--pattern', required=True)
    p.add_argument('--member', type=int)
    p.add_argument('--disassemble', action='store_true')
    p.add_argument('--output', required=True)
    args = p.parse_args()
    pattern = re.compile(args.pattern)
    if args.member is None:
        rows = [(o, s) for o, s in index(args.archive) if pattern.search(s)]
        Path(args.output).write_text('\n'.join(f'{o}\t{s}' for o, s in rows), encoding='utf-8')
        print(f'Saved {len(rows)} indexed symbols to {args.output}')
    else:
        obj = Coff(args.archive, args.member)
        rows = [s for s in obj.symbols.values() if s['section'] > 0 and pattern.search(s['name'])]
        if args.disassemble:
            output = '\n\n'.join(obj.disassemble(s) for s in rows)
        else:
            output = '\n'.join(str(s) for s in rows)
        Path(args.output).write_text(output, encoding='utf-8')
        print(f'Saved {len(rows)} object symbols to {args.output}')


if __name__ == '__main__':
    main()
