"""Read-only contexts for unit-renderer command setters in game 0.6.3."""
from paths import GAME_EXE
import json
from pathlib import Path

from trace_pe import PeResearch

EXE = GAME_EXE
pe = PeResearch(EXE)
functions = [0x230EC30, 0x1C91F0, 0x21725A0, 0x2171AE0, 0x1C9050, 0x388DAD0, 0x2E4EC0]
targets = {0x1C91F0, 0x21725A0, 0x2171AE0, 0x1C9050, 0x388DAD0}
result = {}
for rva in functions:
    bounds = pe.enclosing(rva)
    assert bounds is not None
    insns = list(pe.decode(bounds))
    head = [f"{i.address-pe.base:x}: {i.mnemonic} {i.op_str}" for i in insns[:65]]
    calls = []
    if rva == 0x230EC30:
        for index, insn in enumerate(insns):
            if insn.mnemonic == 'call' and insn.operands and insn.operands[0].imm - pe.base in targets:
                calls.append({
                    'site': hex(insn.address - pe.base),
                    'target': hex(insn.operands[0].imm - pe.base),
                    'context': [f"{i.address-pe.base:x}: {i.mnemonic} {i.op_str}" for i in insns[max(0, index-15):index+10]],
                })
    result[hex(rva)] = {'size': bounds[1] - bounds[0], 'head': head, 'calls': calls}
out = Path(__file__).resolve().parents[1] / 'research' / 'outline-api-0.6.3.json'
out.write_text(json.dumps(result, indent=2) + '\n')
print(f'Wrote {out}; generic renderer setter calls: {len(result[hex(functions[0])]["calls"])}')
