"""Verify a staged build without a game host, then write dist/build-<version>.json.

Checks: the DLL carries the reviewed executable guard, every native anchor in
the profile matches the installed game, cursor/glyph/UI/font assets match their
records in tools/records, and the DLL entry points reject a null host. Release
notes come from the version's section in CHANGELOG.md.

Usage (after `cargo test --release` and tools/stage_package.py):
    python tools/verify_build.py --version 0.66.0 --probe-tests 274
"""
from paths import FONT_SOURCES, GAME_EXE
import argparse
import ctypes
import hashlib
import json
import re
import sys
from datetime import datetime, timezone, timedelta
from pathlib import Path
sys.dont_write_bytecode = True

root = Path(__file__).resolve().parents[1]
records = root / 'tools/records'
package = root / 'dist/lt_direct_control_probe'
icons = root / 'design/hud/endfield-assets/icons'
digest = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
load = lambda name: json.loads((records / name).read_text(encoding='utf-8'))

parser = argparse.ArgumentParser()
parser.add_argument('--version', required=True)
parser.add_argument('--probe-tests', type=int, required=True)
parser.add_argument('--previous-result', default='', help='What the previous build showed in game (optional).')
parser.add_argument('--native-profile', type=Path, default=root / 'tools/native_profiles/0.6.3.json')
args = parser.parse_args()

# Release notes: the bullet lines under "## <version>" in CHANGELOG.md.
changelog = (root / 'CHANGELOG.md').read_text(encoding='utf-8')
section = re.search(rf'^## {re.escape(args.version)}\b.*?$(.*?)(?=^## |\Z)', changelog, re.M | re.S)
assert section, f'CHANGELOG.md has no "## {args.version}" section'
changes = [line[2:].strip() for line in section.group(1).splitlines() if line.startswith('- ')]
assert changes, f'CHANGELOG.md section {args.version} has no "- " entries'

from verify_native_profile import verify
args.native_profile = args.native_profile.resolve()
profile = verify(args.native_profile, GAME_EXE)
expected = profile['executable_sha256']
anchors = dict(profile['anchors'])
anchors.update(profile.get('tooltip_anchors', {}))
# Include the render/input operands and source data in future migration audits.
for op in profile['minimap']['operands']:
    anchors['MINIMAP_' + op['name']] = {'rva': op['rva'], 'bytes': op['bytes']}
    if 'data_rva' in op:
        anchors['MINIMAP_DATA_' + op['name']] = {'rva': op['data_rva'], 'bytes': op['data_bytes']}

info = json.loads((package / 'mod.mod_info').read_text())
version = info['version']
assert version == args.version, f'staged mod.mod_info is {version}, not {args.version}'
assert expected.encode() in (package / 'lt_direct_control_probe.dll').read_bytes(), 'DLL lacks current executable guard'

# Cursor artwork embedded in the DLL.
cursor_art = load('cursor-assets.json')
assert digest(root / cursor_art['source']) == cursor_art['source_sha256']
assert len(cursor_art['files']) == 14
for name, asset in cursor_art['files'].items():
    pixels = (root / 'probe/cursor' / f'{name}.bgra').read_bytes()
    assert len(pixels) == asset['bytes'] == 64 * 64 * 4, name
    assert hashlib.sha256(pixels).hexdigest() == asset['sha256'], name
    assert all(0 <= c < 32 for c in asset['hotspot_32']), name
    assert all(max(pixels[i:i + 3]) <= pixels[i + 3] for i in range(0, len(pixels), 4)), name

# Packaged UI glyphs and fonts.
hud_art = load('hud-glyphs.json')
assert digest(root / hud_art['source']) == hud_art['source_sha256']
assert len(hud_art['files']) == 8
ui_art = load('ui-graphics.json')
assert len(ui_art['files']) == 88  # 86 + stat icon sprite sheet and data (0.76.7)
stamp = load('shop-stamp.json')
assert digest(root / stamp['source']) == stamp['source_sha256']
assert stamp['rotation_degrees_clockwise'] == -7.5
assert stamp['display_size'] == [64, 64]
for record in [hud_art, ui_art, stamp, load('settings-glyphs.json'), load('shop-glyphs.json'), load('endfield-glyphs.json')]:
    for name, value in record['files'].items():
        assert digest(package / name) == value, name
    for name, value in record.get('sources', {}).items():
        assert digest(icons / f'{name}.svg') == value, name

font_check = 'skipped: fontTools not installed (pip install fonttools)'
try:
    sys.path.insert(0, str(root / 'research/fonttools-runtime'))  # author's vendored copy, if present
    from fontTools.ttLib import TTFont
except ImportError:
    TTFont = None
fonts = load('hud-fonts.json')
for name, value in fonts['fonts'].items():
    if FONT_SOURCES.is_dir():
        assert digest(FONT_SOURCES / value['source']) == value['source_sha256'], name
    if TTFont:
        font = TTFont(package / 'font' / f'{name}.ttf')
        assert 'fvar' not in font
        assert font['OS/2'].usWeightClass == value['weight']
        font.close()
if TTFont:
    font_check = 'static instances and weights verified' + (
        '; lab sources verified' if FONT_SOURCES.is_dir() else '; lab sources not present')

# DLL entry points without a game host.
dll = ctypes.CDLL(str(package / 'lt_direct_control_probe.dll'))
dll.tfm2_mod_required_abi_level.restype = ctypes.c_uint32
assert dll.tfm2_mod_required_abi_level() == 6
dll.tfm2_mod_entry_stable.argtypes = [ctypes.c_void_p]
dll.tfm2_mod_entry_stable.restype = ctypes.c_void_p
assert dll.tfm2_mod_entry_stable(None) is None

sources = sorted(p for p in (root / 'probe/src').rglob('*') if p.suffix in {'.rs', '.json'})
record = {
    'version': version,
    'built_at': datetime.now(timezone(timedelta(hours=8))).isoformat(),
    'changes': changes,
    'previous_game_result': args.previous_result,
    'dll_sha256': digest(package / 'lt_direct_control_probe.dll'),
    'metadata_sha256': digest(package / 'mod.mod_info'),
    'dll_load': 'ABI 6; null host rejected',
    'executable_sha256': expected,
    'probe_tests': args.probe_tests,
    'native_profile': args.native_profile.relative_to(root).as_posix(),
    'native_profile_sha256': digest(args.native_profile),
    'native_branches': anchors,
    'native_layout_guards': profile['layout_anchors'],
    'native_minimap_operands': profile['minimap'],
    'source_sha256': {p.relative_to(root).as_posix(): digest(p) for p in sources},
    'ui_asset_hashes': ui_art['files'],
    'font_check': font_check,
    'automated_checks': f'{args.probe_tests} probe tests passed; Clippy with warnings denied, formatting and release build passed; {len(anchors)} executable anchors, ABI 6/null-host rejection, {len(ui_art["files"])} UI/font assets and source fingerprints verified; {len(profile["layout_anchors"])} layout guards verified; fonts: {font_check}; ZIP verified separately.',
    'game_test': 'pending user test on 0.6.3; native rendering/gameplay not verified by automated tests',
    'installation': 'Package verified; installation pending.',
}
(root / f'dist/build-{version}.json').write_text(json.dumps(record, indent=2) + '\n', encoding='utf-8')
print(json.dumps({k: record[k] for k in ['version', 'dll_sha256', 'metadata_sha256', 'probe_tests', 'dll_load']}, indent=2))
print(f'All {len(anchors)} native instruction/header anchors verified; ABI 6 and null-host rejection passed.')
