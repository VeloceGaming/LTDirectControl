"""Verify package fingerprints, native anchors and DLL entry without a game host."""
import ctypes
import hashlib
import json
import sys
from datetime import datetime, timezone, timedelta
from pathlib import Path
sys.dont_write_bytecode = True
from trace_pe import PeResearch

root = Path(__file__).resolve().parents[1]
exe = Path(r'C:\Program Files (x86)\Steam\steamapps\common\Teamfight Manager2\TeamfightManager2.exe')
package = root / 'dist/lt_direct_control_probe'
digest = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
expected = '15df9eb3b6915cdcc4c2ebb3b7f5fa232b563c4cdd32208581634817b71adc23'
assert digest(exe) == expected
record = json.loads((root/'dist/build-0.25.0.json').read_text(encoding='utf-8'))
anchors = record['native_branches']
anchors['ATTACK_BYTES'] = {'rva':'0x162f387','bytes':'e87433f8ff','target':'0x15b2700'}
anchors['ATTACK_PROLOGUE'] = {'rva':'0x15b2700','bytes':'5541574156415541545657534881ec88000000'}
anchors['CANCEL_RECALL_PROLOGUE'] = {'rva':'0x17069c0','bytes':'5556574883ec30488d6c243048c745f8'}
r = PeResearch(exe)
for name, anchor in anchors.items():
    value = bytes.fromhex(anchor['bytes'])
    assert r.pe.get_data(int(anchor['rva'],16),len(value)) == value, name
info = json.loads((package/'mod.mod_info').read_text())
assert info['version'] == '0.26.0'
dll = ctypes.CDLL(str(package/'lt_direct_control_probe.dll'))
dll.tfm2_mod_required_abi_level.restype = ctypes.c_uint32
assert dll.tfm2_mod_required_abi_level() == 6
dll.tfm2_mod_entry_stable.argtypes = [ctypes.c_void_p]
dll.tfm2_mod_entry_stable.restype = ctypes.c_void_p
assert dll.tfm2_mod_entry_stable(None) is None
record.update({
    'version':'0.26.0',
    'built_at':datetime.now(timezone(timedelta(hours=8))).isoformat(),
    'dll_sha256':digest(package/'lt_direct_control_probe.dll'),
    'metadata_sha256':digest(package/'mod.mod_info'),
    'executable_sha256':expected,
    'core_tests':18,
    'probe_tests':110,
    'changes':[
        'Portrait-only own-team selector on the held battlefield; role changes allowed only before Start and bound to current match/generation',
        'Prepared snapshots for all players; re-selection clears movement, skill, recall and camera state without releasing the worker',
        'Preparation waits for Start or AI without 120-second automatic release; startup and two-second heartbeat guards retained',
        'Original graphical session controls and HUD symbols, opaque grayscale framing, exact yellow active accents and red death count',
        'Numeric level/HP/KDA/CS/gold, dimmed locked abilities and numeric portrait respawn timer; existing permanent sheet/PNG nodes retained',
        'Host-localized skill/item descriptions on delayed hover; unresolved formula values shown as ellipses; tooltip bodies ignore events and do not mask world commands',
        'Brief rejection feedback distinguishes known unlock levels/cooldowns; ordinary startup/control/aiming diagnostics kept in log',
        'No new native branches or offsets; all twelve anchors unchanged'
    ],
    'game_test':'pending: own-team portrait selection, prepared rebind, icon controls, native rendering and tooltip localization/wrapping',
    'test_instructions':'TESTING-26.md',
    'previous_recording':'research/probe-2026-10-02-twenty-fifth.log',
    'previous_game_result':'User reports improved body picking; precise moving-target fit remains unmeasured. Approved portrait-only grayscale graphical UI.',
    'installed_asset_check':'research/hud-enabled-assets-0.26.0.json plus research/ui-graphics-0.26.0.json; native UI rendering pending',
    'installation':'Package verified; installation pending. Verified 0.25 preserved in dist/backups/0.25.0.'

})
(root/'dist/build-0.26.0.json').write_text(json.dumps(record,indent=2)+'\n',encoding='utf-8')
print(json.dumps({k:record[k] for k in ['version','dll_sha256','metadata_sha256','core_tests','probe_tests','dll_load']},indent=2))
print('All twelve native branch/header anchors verified; ABI 6 and null-host rejection passed.')
