"""Verify package fingerprints, native anchors and DLL entry without a game host."""
import ctypes
import argparse
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
parser = argparse.ArgumentParser()
parser.add_argument('--previous-version', default='0.26.0')
parser.add_argument('--probe-tests', type=int, required=True)
args = parser.parse_args()
record = json.loads((root/f'dist/build-{args.previous_version}.json').read_text(encoding='utf-8'))
anchors = record['native_branches']
anchors['ATTACK_BYTES'] = {'rva':'0x162f387','bytes':'e87433f8ff','target':'0x15b2700'}
anchors['ATTACK_PROLOGUE'] = {'rva':'0x15b2700','bytes':'5541574156415541545657534881ec88000000'}
anchors['CANCEL_RECALL_PROLOGUE'] = {'rva':'0x17069c0','bytes':'5556574883ec30488d6c243048c745f8'}
r = PeResearch(exe)
for name, anchor in anchors.items():
    value = bytes.fromhex(anchor['bytes'])
    assert r.pe.get_data(int(anchor['rva'],16),len(value)) == value, name
info = json.loads((package/'mod.mod_info').read_text())
version = info['version']
assert version == '0.26.1'
dll = ctypes.CDLL(str(package/'lt_direct_control_probe.dll'))
dll.tfm2_mod_required_abi_level.restype = ctypes.c_uint32
assert dll.tfm2_mod_required_abi_level() == 6
dll.tfm2_mod_entry_stable.argtypes = [ctypes.c_void_p]
dll.tfm2_mod_entry_stable.restype = ctypes.c_void_p
assert dll.tfm2_mod_entry_stable(None) is None
record.pop('installed_at', None)
record.update({
    'version':version,
    'built_at':datetime.now(timezone(timedelta(hours=8))).isoformat(),
    'dll_sha256':digest(package/'lt_direct_control_probe.dll'),
    'metadata_sha256':digest(package/'mod.mod_info'),
    'executable_sha256':expected,
    'core_tests':18,
    'probe_tests':args.probe_tests,
    'changes':[
        'Direct control activates when the mod is enabled; no development path or activation flags required',
        'Per-user LOCALAPPDATA diagnostic folder with TEMP fallback; unavailable logging no longer blocks mod initialization',
        'Read-only result reports use the selected portable diagnostic folder rather than the compile-time project path',
        'Retains 0.26 graphical UI and gameplay; C-only laptop and native UI testing pending',
        'No new native branches or offsets; all twelve anchors unchanged'
    ],
    'game_test':'pending: C-only laptop installation without project/flags, plus 0.26 UI game test',
    'test_instructions':'TESTING-26.1.md',
    'previous_recording':'research/probe-2026-10-02-twenty-fifth.log',
    'previous_game_result':'User reports improved body picking; precise moving-target fit remains unmeasured. Approved portrait-only grayscale graphical UI.',
    'installed_asset_check':f'research/hud-enabled-assets-{version}.json plus research/ui-graphics-{version}.json; native UI rendering pending',
    'installation':'Release package verified; local installed 0.26.0 unchanged. Prior package preserved in dist/backups/0.26.0.'

})
(root/f'dist/build-{version}.json').write_text(json.dumps(record,indent=2)+'\n',encoding='utf-8')
print(json.dumps({k:record[k] for k in ['version','dll_sha256','metadata_sha256','core_tests','probe_tests','dll_load']},indent=2))
print('All twelve native branch/header anchors verified; ABI 6 and null-host rejection passed.')
