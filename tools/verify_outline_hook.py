"""Read-only verification of the 0.6.3 hover-outline call redirects.

This does not prove that the visual effect works in a match. It verifies that
the Rust constants still name the reviewed instructions in the exact game PE.
"""
import hashlib
import re
import struct
from pathlib import Path

from trace_pe import PeResearch

ROOT = Path(__file__).resolve().parents[1]
EXE = Path(r"C:\Program Files (x86)\Steam\steamapps\common\Teamfight Manager2\TeamfightManager2.exe")
adapter = (ROOT / "probe/src/native_adapter.rs").read_text(encoding="utf-8")
profile = (ROOT / "probe/src/native_profile.rs").read_text(encoding="utf-8")
sha = re.search(r'const EXPECTED_SHA: &str = "([0-9a-f]+)"', profile)
assert sha, "Missing native executable fingerprint"
assert hashlib.sha256(EXE.read_bytes()).hexdigest() == sha[1], "Game executable changed"
pe = PeResearch(EXE)

site_list = re.search(r"const OUTLINE_SITES:.*?=\s*\[(.*?)\];", adapter, re.S)
assert site_list, "Missing outline site list"
sites = re.findall(r"\(\s*(0x[0-9a-f]+)\s*,\s*\[([^]]+)\]\s*\)", site_list[1])
assert len(sites) == 15, f"Expected 15 renderer calls, got {len(sites)}"
for rva, literal in sites:
    site = int(rva, 16)
    expected = bytes(int(part.strip(), 0) for part in literal.split(",") if part.strip())
    assert len(expected) == 5 and expected[0] == 0xE8
    assert pe.pe.get_data(site, 5) == expected, hex(site)
    assert site + 5 + struct.unpack_from("<i", expected, 1)[0] == 0x230EC30, hex(site)

for name, rva in [
    ("OUTLINE_PROLOGUE", 0x230EC30),
    ("OUTLINE_PARAM_PROLOGUE", 0x21725A0),
    ("OUTLINE_PARAM_PROLOGUE", 0x2171AE0),
    ("OUTLINE_LINE_BYTES", 0x2311692),
    ("OUTLINE_NINEPATCH_BYTES", 0x1C7A8A),
    ("OUTLINE_DROP_BYTES", 0x1C5190),
    ("OUTLINE_DROP_TABLE_BYTES", 0x3A199AC),
    ("OUTLINE_DROP_CALL_BYTES", 0x1CA533),
]:
    declaration = re.search(rf"const {name}:.*?=\s*\[([^]]+)\]", adapter, re.S)
    assert declaration, name
    expected = bytes(int(part.strip(), 0) for part in declaration[1].split(",") if part.strip())
    assert pe.pe.get_data(rva, len(expected)) == expected, name

drop = pe.pe.get_data(0x1C5190, 669)
assert hashlib.sha256(drop).hexdigest() == "2b5dfb85d2666f30990a67df5e4435acf655c4cf8bd78ccb368c1b65caf55c2e"
table = pe.pe.get_data(0x3A199AC, 52)
assert [0x3A199AC + v for v in struct.unpack("<13i", table)] == [
    0x1C51E1, 0x1C51E1, 0x1C51E1, 0x1C52D5, 0x1C53A2,
    0x1C5248, 0x1C5425, 0x1C5234, 0x1C5425, 0x1C5234,
    0x1C5364, 0x1C5425, 0x1C5234,
], "RenderCommand cleanup dispatch changed"
assert 0x1CA538 + struct.unpack("<i", pe.pe.get_data(0x1CA534, 4))[0] == 0x1C5190
print("Verified 15 outline CALLs, renderer/setter guards, line/circle tags, full command cleanup and dispatch on game 0.6.3")
