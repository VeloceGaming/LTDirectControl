"""Read-only recall and movement behavior in the fingerprinted executable."""
import sys
sys.dont_write_bytecode = True
from trace_pe import PeResearch
from pathlib import Path
r = PeResearch(r'C:\Program Files (x86)\Steam\steamapps\common\Teamfight Manager2\TeamfightManager2.exe')
lines = []
for addr in [0x15b2c70,0x15c4210,0x15b53e0]:
    lines.append(f'FUNCTION {addr:x}')
    for i in r.decode(r.enclosing(addr)):
        lines.append(f'{i.address-r.base:x}: {i.mnemonic} {i.op_str}')
Path('research/recall-native-current.txt').write_text('\n'.join(lines))
print('\n'.join(lines[:200]))
