"""Verify a reviewed native profile against the exact executable and Rust sources.

This verifies explicit locations; it never searches, updates, builds or installs.
"""
from paths import GAME_EXE
import argparse
import hashlib
import json
import re
import struct
from pathlib import Path
from patch_migration import Image

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_EXE = GAME_EXE


def adapter_source(root=ROOT):
    """All native adapter sources (probe/src/native_adapter/**), joined."""
    folder = root / 'probe/src/native_adapter'
    return '\n'.join(p.read_text(encoding='utf-8') for p in sorted(folder.rglob('*.rs')))


def declaration(source, name):
    match = re.search(rf'const {name}\s*:[^=]+?=\s*(.*?);', source, re.S)
    if not match:
        raise ValueError(f'Missing Rust constant {name}')
    return match[1]


def numbers(value):
    return [int(x, 0) for x in re.findall(r'\b(?:0x[0-9a-f]+|[0-9]+)\b', value)]


def byte_pairs(value):
    return {int(rva, 0): bytes(numbers(data)) for rva, data in
            re.findall(r'\(\s*(0x[0-9a-f]+)\s*,\s*&\[(.*?)\]\s*,?\s*\)', value, re.S)}


def verify_sources(profile, root=ROOT):
    adapter = adapter_source(root)
    identity = (root/'probe/src/native_profile.rs').read_text()
    items = (root/'probe/src/native_items.rs').read_text()
    preview = (root/'probe/src/native_preview.rs').read_text()
    if 'minimap' in profile:
        minimap = (root/'probe/src/minimap.rs').read_text()
        assert 'include_str!("../../tools/native_profiles/0.6.3.json")' in minimap
        assert 'minimap_plan.verify()' in adapter
        assert 'crate::minimap::content_rect(wide, left, custom)' in adapter
    anchors = profile['anchors']
    assert json.loads(declaration(identity, 'EXPECTED_SHA')) == profile['executable_sha256']
    assert numbers(declaration(identity, 'PE_TIMESTAMP')) == [int(profile['timestamp'], 16)]
    assert numbers(declaration(identity, 'IMAGE_SIZE')) == [int(profile['size_of_image'], 16)]
    assert 'native_profile::verify_layout(base)' in adapter
    byte_names = {
        'WORKER_BYTES':('WORKER_SITE','WORKER_BYTES'), 'VIEW_BYTES':('VIEW_SITE','VIEW_BYTES'),
        'MOVE_BYTES':('MOVE_SITE','MOVE_BYTES'), 'INPUT_BYTES':('INPUT_SITE','INPUT_BYTES'),
        'ATTACK_BYTES':('ATTACK_SITE','ATTACK_BYTES'), 'SHADER_BYTES':('SHADER_SITE','SHADER_BYTES'),
        'STEER_CALL':('STEER_SITE','STEER_BYTES'), 'AUTO_ATTACK_CALL':('AUTO_ATTACK_SITE','AUTO_ATTACK_BYTES'),
        'AIM_CORRECTION_CALL':('AIM_SITE','AIM_BYTES'),
        'WORKER_PROLOGUE':('WORKER_ORIGINAL','WORKER_PROLOGUE'),
        'VIEW_PROLOGUE':('VIEW_ORIGINAL','VIEW_PROLOGUE'), 'MOVE_PROLOGUE':('MOVE_ORIGINAL','MOVE_PROLOGUE'),
        'INPUT_PROLOGUE':('INPUT_ORIGINAL','INPUT_PROLOGUE'), 'STOP_PROLOGUE':('STOP_EVENT','STOP_PROLOGUE'),
        'ATTACK_PROLOGUE':('ATTACK_ORIGINAL','ATTACK_PROLOGUE'),
        'SHADER_PROLOGUE':('SHADER_ORIGINAL','SHADER_PROLOGUE'),
        'STEER_PROLOGUE':('STEER_ORIGINAL','STEER_PROLOGUE'),
        'DIRECT_STEP_PROLOGUE':('DIRECT_STEP','DIRECT_PROLOGUE'),
        'AIM_CORRECTION_PROLOGUE':('AIM_ORIGINAL','AIM_PROLOGUE'),
        'CANCEL_RECALL_PROLOGUE':('CANCEL_RECALL_EVENT','CANCEL_RECALL_PROLOGUE'),
        'GOLD_GETTER_BYTES':('GOLD_GETTER','GOLD_GETTER_BYTES'),
        'BUILD_LEN_BYTES':('BUILD_LEN_SITE','BUILD_LEN_BYTES'), 'BUILD_PTR_BYTES':('BUILD_PTR_SITE','BUILD_PTR_BYTES'),
    }
    for name, (site, value) in byte_names.items():
        assert numbers(declaration(adapter, site)) == [int(anchors[name]['rva'], 16)], name
        assert bytes(numbers(declaration(adapter, value))).hex() == anchors[name]['bytes'], name
    for idx, slot in enumerate('QWR'):
        a = anchors[f'SKILL_{slot}_CALL']
        assert numbers(declaration(adapter, 'SKILL_SITES'))[idx] == int(a['rva'], 16)
        assert numbers(declaration(adapter, 'SKILL_ORIGINALS'))[idx] == int(a['target'], 16)
        assert bytes(numbers(declaration(adapter, 'SKILL_BYTES'))[idx*5:(idx+1)*5]).hex() == a['bytes']
        assert bytes(numbers(declaration(adapter, 'SKILL_PROLOGUE'))).hex() == anchors[f'SKILL_{slot}_PROLOGUE']['bytes']
    pairs = {}
    for source, name in [(adapter,'AIM_WRITES'),(adapter,'SKILL_QUEUE_ANCHORS'),(items,'ANCHORS'),(identity,'LAYOUT_BYTES')]:
        pairs.update(byte_pairs(declaration(source, name)))
    for name, a in (anchors | profile['layout_anchors']).items():
        if name.startswith(('ITEM_', 'ENTITY_', 'VISION_', 'VIEW_LAYOUT_')) or name in ('AIM_POINT_WRITE','AIM_DIRECTION_WRITE') or name.endswith('QUEUE_LAYOUT'):
            assert pairs[int(a['rva'],16)].hex() == a['bytes'], name
    pointers = numbers(declaration(identity, 'LAYOUT_POINTERS'))
    for a in profile['layout_anchors'].values():
        if 'pointer_target' in a:
            at = pointers.index(int(a['rva'],16))
            assert pointers[at+1] == int(a['pointer_target'],16)
    for name, anchor in [('COMBINE','PREVIEW_COMBINE_APPLY'),('RANGE','PREVIEW_RANGE_APPLY'),
                         ('LINEAR','PREVIEW_LINEAR_APPLY'),('WHIP_LINE','PREVIEW_CHANNEL_LINE_APPLY')]:
        assert numbers(declaration(preview,name)) == [int(anchors[anchor]['rva'],16)]


def verify(profile_path, executable=DEFAULT_EXE, check_sources=True):
    profile = json.loads(Path(profile_path).read_text())
    if profile.get('schema') != 1 or profile.get('reviewed') is not True:
        raise ValueError('Profile is not explicitly reviewed')
    if hashlib.sha256(Path(executable).read_bytes()).hexdigest() != profile['executable_sha256']:
        raise ValueError('Executable fingerprint differs; profile verification refused')
    image = Image(Path(executable))
    assert image.identity()['timestamp'] == profile['timestamp']
    assert image.identity()['size_of_image'] == profile['size_of_image']
    for name, a in (profile['anchors'] | profile['layout_anchors']).items():
        rva = int(a['rva'], 16)
        if 'bytes' in a:
            value = bytes.fromhex(a['bytes'])
            assert image.pe.get_data(rva,len(value)) == value, name
            if 'target' in a:
                assert value[0] in (0xe8,0xe9) and len(value) == 5, name
                assert rva+5+struct.unpack_from('<i',value,1)[0] == int(a['target'],16), name
        else:
            target = int.from_bytes(image.pe.get_data(rva,8),'little')-image.base
            assert target == int(a['pointer_target'],16), name
    if 'minimap' in profile:
        map_profile=profile['minimap']
        assert map_profile['reviewed'] is True
        assert map_profile['map_size']==352 and map_profile['padding']==4
        seen=set()
        for op in map_profile['operands']:
            rva=int(op['rva'],16)
            assert rva not in seen,op['name']
            seen.add(rva)
            value=bytes.fromhex(op['bytes'])
            assert image.pe.get_data(rva,len(value))==value,op['name']
            ins=list(image.cs.disasm(value,image.base+rva))
            assert len(ins)==1 and ins[0].size==len(value),op['name']
            ins=ins[0]
            offset=op['offset']
            assert 0<=offset<=len(value)-4,op['name']
            if op['kind']=='rip_f32x4':
                # RIP addressing always uses disp32, including UNPCKLPD with
                # 0x66. Capstone reports disp_size=2 for that prefixed opcode.
                from capstone.x86 import X86_OP_MEM, X86_REG_RIP
                assert ins.disp_offset==offset and offset+4==len(value),op['name']
                assert any(o.type==X86_OP_MEM and o.mem.base==X86_REG_RIP for o in ins.operands),op['name']
                target=rva+len(value)+struct.unpack_from('<i',value,offset)[0]
                assert target==int(op['data_rva'],16),op['name']
                data=bytes.fromhex(op['data_bytes'])
                assert len(data) in (4,8,16),op['name']
                assert image.pe.get_data(target,len(data))==data,op['name']
                assert len(op['values'])==4 and all(abs(v)<4096 for v in op['values']),op['name']
            else:
                assert op['kind']=='immediate' and len(bytes.fromhex(op['value']))==4,op['name']
                assert ins.imm_offset<=offset and offset+4<=ins.imm_offset+ins.imm_size,op['name']
    if check_sources:
        verify_sources(profile)
    return profile


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--profile', type=Path, default=ROOT/'tools/native_profiles/0.6.3.json')
    parser.add_argument('--executable', type=Path, default=DEFAULT_EXE)
    args = parser.parse_args()
    p = verify(args.profile, args.executable)
    print(f"Verified game {p['game_version']}: exact identity, {len(p['anchors'])} anchors, "
          f"{len(p['layout_anchors'])} layout guards, call targets, pointers and Rust constants.")
