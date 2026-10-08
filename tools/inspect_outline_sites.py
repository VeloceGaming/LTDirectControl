"""Read-only opcode/context audit for the proposed 0.6.3 unit render redirects."""
from paths import GAME_EXE
import json
from pathlib import Path

from trace_pe import PeResearch

EXE = GAME_EXE
SITES = [
    0x1F077EB, 0x215AE74, 0x215C5C5, 0x231FAA6, 0x2347DEF,
    0x2348C33, 0x234A2C9, 0x234B334, 0x2350312, 0x2370FE0,
    0x2371024, 0x2516BD7, 0x2518CC9, 0x2519BE1, 0x251B4B4,
]
TARGET = 0x230EC30
pe = PeResearch(EXE)
rows = []
for site in SITES:
    bounds = pe.enclosing(site)
    assert bounds is not None, hex(site)
    insns = list(pe.decode(bounds))
    index = next(i for i, insn in enumerate(insns) if insn.address - pe.base == site)
    call = insns[index]
    assert call.mnemonic == "call" and call.operands[0].imm - pe.base == TARGET, hex(site)
    rows.append({
        "site": hex(site),
        "function": hex(bounds[0]),
        "bytes": pe.pe.get_data(site, 5).hex(),
        "context": [f"{i.address-pe.base:x}: {i.mnemonic} {i.op_str}" for i in insns[max(0, index-25):index+8]],
    })
renderer = pe.enclosing(TARGET)
assert renderer and renderer[0] == TARGET
render_insns = list(pe.decode(renderer))
out = Path(__file__).resolve().parents[1] / "research" / "outline-call-sites-0.6.3.json"
out.write_text(json.dumps({
    "target": hex(TARGET),
    "renderer_size": renderer[1] - renderer[0],
    "renderer_head": [f"{i.address-pe.base:x}: {i.mnemonic} {i.op_str}" for i in render_insns[:100]],
    "renderer_id_refs": [f"{i.address-pe.base:x}: {i.mnemonic} {i.op_str}" for i in render_insns if "+ 0xe8]" in i.op_str],
    "sites": rows,
}, indent=2) + "\n")
print(f"Verified {len(rows)} decoded CALLs to {hex(TARGET)}; wrote {out}")
