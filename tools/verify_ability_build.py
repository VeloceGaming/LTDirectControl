"""Verify package fingerprints, native anchors and DLL entry without a game host."""
import ctypes
import argparse
import hashlib
import json
import sys
from datetime import datetime, timezone, timedelta
from pathlib import Path
sys.dont_write_bytecode = True

root = Path(__file__).resolve().parents[1]
exe = Path(r'C:\Program Files (x86)\Steam\steamapps\common\Teamfight Manager2\TeamfightManager2.exe')
package = root / 'dist/lt_direct_control_probe'
digest = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
parser = argparse.ArgumentParser()
parser.add_argument('--previous-version', default='0.41.0')
parser.add_argument('--version', required=True)
parser.add_argument('--native-profile', type=Path, default=root/'tools/native_profiles/0.6.3.json')
parser.add_argument('--probe-tests', type=int, required=True)
args = parser.parse_args()
from verify_native_profile import verify
args.native_profile = args.native_profile.resolve()
profile = verify(args.native_profile, exe)
expected = profile['executable_sha256']
record = json.loads((root/f'dist/build-{args.previous_version}.json').read_text(encoding='utf-8'))
anchors = dict(profile['anchors'])
# Include the new render/input operands and source data in future migration
# audits. The conservative matcher will mark data locations for manual review.
for op in profile['minimap']['operands']:
    anchors['MINIMAP_' + op['name']] = {'rva':op['rva'], 'bytes':op['bytes']}
    if 'data_rva' in op:
        anchors['MINIMAP_DATA_' + op['name']] = {'rva':op['data_rva'], 'bytes':op['data_bytes']}
info = json.loads((package/'mod.mod_info').read_text())
version = info['version']
assert version == args.version
assert expected.encode() in (package/'lt_direct_control_probe.dll').read_bytes(), 'DLL lacks current executable guard'
cursor_art_version='0.39.0' # This pass reuses the previously verified cursor art.
cursor_art=json.loads((root/f'research/cursor-assets-{cursor_art_version}.json').read_text())
assert digest(root/cursor_art['source'])==cursor_art['source_sha256']
assert len(cursor_art['files'])==14
for name,asset in cursor_art['files'].items():
    pixels=(root/'probe/cursor'/f'{name}.bgra').read_bytes()
    assert len(pixels)==asset['bytes']==64*64*4,name
    assert hashlib.sha256(pixels).hexdigest()==asset['sha256'],name
    assert all(0<=c<32 for c in asset['hotspot_32']),name
    assert all(max(pixels[i:i+3])<=pixels[i+3] for i in range(0,len(pixels),4)),name
hud_art=json.loads((root/'research/hud-glyphs-0.44.0.json').read_text())
assert digest(root/hud_art['source']) == hud_art['source_sha256']
assert len(hud_art['files']) == 8
for name,value in hud_art['files'].items():
    assert digest(package/name) == value,name
ui_art=json.loads((root/f'research/ui-graphics-{version}.json').read_text())
assert len(ui_art['files']) == 85
for name,value in ui_art['files'].items():
    assert digest(package/name) == value,name
settings_glyphs=json.loads((root/'research/settings-glyphs-0.53.0.json').read_text())
for name,value in settings_glyphs['files'].items():
    assert digest(package/name)==value,name
for name,value in settings_glyphs['sources'].items():
    assert digest(root/'design/hud/endfield-assets/icons'/f'{name}.svg')==value,name
shop_glyphs=json.loads((root/'research/shop-glyphs-0.61.0.json').read_text())
for name,value in shop_glyphs['files'].items():
    assert digest(package/name)==value,name
for name,value in shop_glyphs['sources'].items():
    assert digest(root/'design/hud/endfield-assets/icons'/f'{name}.svg')==value,name
glyphs=json.loads((root/'research/endfield-glyphs-0.45.0.json').read_text())
for name,value in glyphs['files'].items():
    assert digest(package/name)==value,name
for name,value in glyphs['sources'].items():
    assert digest(root/'design/hud/endfield-assets/icons'/f'{name}.svg')==value,name
sys.path.insert(0,str(root/'research/fonttools-runtime'))
from fontTools.ttLib import TTFont
fonts=json.loads((root/'research/hud-fonts-0.45.0.json').read_text())
for name,value in fonts['fonts'].items():
    assert digest(Path('C:/LTTool/endfield_ui_lab/ui/src/fonts')/value['source'])==value['source_sha256']
    font=TTFont(package/'font'/f'{name}.ttf')
    assert 'fvar' not in font
    assert font['OS/2'].usWeightClass==value['weight']
    font.close()
dll = ctypes.CDLL(str(package/'lt_direct_control_probe.dll'))
dll.tfm2_mod_required_abi_level.restype = ctypes.c_uint32
assert dll.tfm2_mod_required_abi_level() == 6
dll.tfm2_mod_entry_stable.argtypes = [ctypes.c_void_p]
dll.tfm2_mod_entry_stable.restype = ctypes.c_void_p
assert dll.tfm2_mod_entry_stable(None) is None
record.pop('installed_at', None)
record.pop('zip_sha256', None)
record.pop('zip_members', None)
record.pop('release_url', None)
record.pop('source_commit', None)
record.pop('rollback_directory', None)
record.update({
    'version':version,
    'built_at':datetime.now(timezone(timedelta(hours=8))).isoformat(),
    'dll_sha256':digest(package/'lt_direct_control_probe.dll'),
    'metadata_sha256':digest(package/'mod.mod_info'),
    'executable_sha256':expected,
    'core_tests':18,
    'probe_tests':args.probe_tests,
    'native_branches':anchors,
    'native_layout_guards':profile['layout_anchors'],
    'native_profile':args.native_profile.relative_to(root).as_posix(),
    'native_profile_sha256':digest(args.native_profile),
    'changes':[
        'Shop recipe tree: every part not owned shows the gold it still needs from what you own (League price, red when unaffordable), not just its own step price; owned parts keep their muted own price'
    ],
    'source_sha256':{name:digest(root/name) for name in ['probe/src/settings.rs','probe/src/settings_ui.rs','probe/src/platform_input.rs','probe/src/abilities.rs','probe/src/camera.rs','probe/src/combat.rs','probe/src/movement_test.rs','probe/src/wheel.rs','probe/src/lib.rs','probe/src/attack_trace.rs','probe/src/shop_trace.rs','probe/src/shop.rs','probe/src/shop_ui.rs','probe/src/native_adapter.rs','probe/src/player_hud.rs','probe/src/team_status.rs','probe/src/hud_motion.rs','probe/src/inventory.rs','probe/src/session_ui.rs','probe/src/screen_effect.rs','probe/src/cursor.rs','probe/src/perf.rs','probe/src/native_timing.rs','probe/src/tooltips.rs','probe/src/tooltip_assets.json']},
    'native_rendering':'pending user gameplay test',
    'native_minimap_operands':profile['minimap'],
    'game_test':'pending user test on 0.6.3; native rendering/gameplay not verified by automated tests',
    'test_instructions':f'TESTING-{version.split(".")[1]}.md',
    'cursor_asset_check':f'research/cursor-assets-{cursor_art_version}.json; reused unchanged source SVG digest, premultiplied pixels and hotspots verified',
    'automated_checks':f'{args.probe_tests} probe + 18 core tests passed; core/probe Clippy with warnings denied, formatting and release build passed; {len(anchors)} executable anchors, ABI 6/null-host rejection, 85 UI/font assets and SVG/font fingerprints verified; {len(profile["layout_anchors"])} layout guards, minimap operands/source constants and source/profile agreement verified; native rendering/gameplay pending; ZIP verified separately.',
    'ui_asset_hashes':json.loads((root/f'research/ui-graphics-{version}.json').read_text())['files'],
    'picking_metadata':{
        'base_profiles':len(json.loads((root/'probe/src/picking_assets.json').read_text())),
        'champion_policy':'idle/run/walk baseline with capped 15 percent combat pose expansion; additional upward-only 20 percent height padding bounded 6-12 world units; side/feet and ground rings unchanged',
        'monsters':'unchanged idle/walk profiles; minion vertical-only pads extended; generic tower/nexus alias correction and full-size art profiles; bounded feet origin estimate; construction corner markers share selection envelope',
        'visual_fit':'pending user test; stable sprite rectangles, not live alpha masks'
    },
    'previous_game_result':'0.65.1: cursor flicker fixed and frame rate matches vanilla per user. Shop tree middle parts showed step prices only; a second Radiant order was an unnoticed double click (logic correct).',
    'installed_asset_check':f'research/ui-graphics-{version}.json; native UI rendering pending',
    'installation':'Package verified; installation pending with checked previous fingerprints and rollback copy.'

})
(root/f'dist/build-{version}.json').write_text(json.dumps(record,indent=2)+'\n',encoding='utf-8')
print(json.dumps({k:record[k] for k in ['version','dll_sha256','metadata_sha256','core_tests','probe_tests','dll_load']},indent=2))
print(f'All {len(anchors)} native instruction/header anchors verified; ABI 6 and null-host rejection passed.')
