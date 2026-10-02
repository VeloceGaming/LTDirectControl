"""Read-only current executable skill validation / cached Effect layout."""
import sys
sys.dont_write_bytecode = True
from trace_pe import PeResearch
from pathlib import Path
r = PeResearch(r'C:\Program Files (x86)\Steam\steamapps\common\Teamfight Manager2\TeamfightManager2.exe')
lines = []
for addr in [0x15b2700, 0x15c1640, 0x162f0d0, 0x15b2d20, 0x15c4980, 0x15c3560]:
    lines.append(f'FUNCTION {addr:x}')
    for i in r.decode(r.enclosing(addr)):
        lines.append(f'{i.address-r.base:x}: {i.mnemonic} {i.op_str}')
Path('research/ability-native-current.txt').write_text('\n'.join(lines))
print('\n'.join(lines[:125]))
for addr, count in [(0x162f387,5),(0x15b2700,19)]:
    print(hex(addr), r.pe.get_data(addr,count).hex())
import struct
table = 0x162f22b+7+0x255d7e6
print('Native Input table',hex(table),[hex(table+n) for n in struct.unpack('<6i',r.pe.get_data(table,24))])
