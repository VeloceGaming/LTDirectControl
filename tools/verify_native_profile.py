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


def tables_folder(profile, root=ROOT):
    """The folder holding one game version's tables: 0.7.0 -> native_adapter/v070."""
    return root / 'probe/src/native_adapter' / ('v' + profile['game_version'].replace('.', ''))


def adapter_source(profile, root=ROOT):
    """The hook code shared by every game version, then this version's tables, joined."""
    folder = root / 'probe/src/native_adapter'
    shared = [folder / 'mod.rs', *sorted((folder / 'windows').rglob('*.rs'))]
    tables = sorted(tables_folder(profile, root).glob('*.rs'))
    assert tables, f'No tables for game {profile["game_version"]}'
    return '\n'.join(p.read_text(encoding='utf-8') for p in shared + tables)


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


# The signal probe (research builds, 1.7.1-dev4): each field offset of the
# version's signals.rs, and the anchors whose instructions use it.
SIGNAL_FIELDS = {
    'SIGNAL_WORLD_TICK': ['SIGNAL_TICK_READ', 'SIGNAL_DELAY_READ'],
    'SIGNAL_PLAYER_SIDE': ['SIGNAL_IDENTITY_READ'],
    'SIGNAL_PLAYER_INDEX': ['SIGNAL_INDEX_READ', 'SIGNAL_GATE_OWN'],
    'SIGNAL_STATE_DELAY': ['SIGNAL_DELAY_READ'],
    'SIGNAL_JOIN_STATS': ['SIGNAL_JOIN_FIRST'],
    # The scene call's answer (1.7.1-dev8).
    'SIGNAL_SCENE_KIND': ['SIGNAL_SCENE_KIND_READ', 'SIGNAL_IN_BATTLE_WRITE'],
    'SIGNAL_SCENE_ARG': ['SIGNAL_IN_BATTLE_WRITE'],
    'SIGNAL_SIM_UNITS': ['SIGNAL_SCENE_KIND_READ'],
    'SIGNAL_UNIT_POS': ['SIGNAL_UNIT_POS_READ'],
    'SIGNAL_UNIT_SPEED': ['SIGNAL_UNIT_HP_READ'],
    'SIGNAL_UNIT_MAX_HP': ['SIGNAL_UNIT_HP_READ'],
    'SIGNAL_UNIT_HP': ['SIGNAL_UNIT_HP_READ'],
    'SIGNAL_SCENE_BLOCK': ['SIGNAL_SCENE_WRITE'],
    'SIGNAL_SCENE_TICK': ['SIGNAL_SCENE_WRITE'],
    'SIGNAL_SCENE_POS': ['SIGNAL_SCENE_WRITE'],
    'SIGNAL_SCENE_DIST': ['SIGNAL_SCENE_WRITE'],
    'SIGNAL_SCENE_FLAG': ['SIGNAL_SCENE_WRITE'],
    # The per-player levers (1.7.1-dev10).
    'SIGNAL_BIG_PLAN': ['SIGNAL_SCENE_KIND_READ', 'SIGNAL_IN_BATTLE_WRITE', 'SIGNAL_HIDE_CASE'],
    'SIGNAL_BIG_PLAN_ARG': ['SIGNAL_IN_BATTLE_WRITE'],
    'SIGNAL_OBJECTIVE': ['SIGNAL_HIDE_OBJECTIVE'],
    'SIGNAL_TEAM_PLAN': ['SIGNAL_HIDE_OBJECTIVE'],
}
# The delivery probe's hooked calls (1.7.1-dev6): each call anchor, its
# site and bytes constants, and the original's opening bytes. The two
# receive-handler calls are arrays, checked in verify_signal_delivery.
SIGNAL_CALLS = {
    'SIGNAL_LATE_CALL': ('SIGNAL_LATE_SITE', 'SIGNAL_LATE_BYTES'),
    'SIGNAL_LATE_PROLOGUE': ('SIGNAL_LATE_ORIGINAL', 'SIGNAL_LATE_PROLOGUE'),
    'SIGNAL_GATE_CALL': ('SIGNAL_GATE_SITE', 'SIGNAL_GATE_BYTES'),
    'SIGNAL_GATE_PROLOGUE': ('SIGNAL_GATE_ORIGINAL', 'SIGNAL_GATE_PROLOGUE'),
    'SIGNAL_HEARD_PROLOGUE': ('SIGNAL_HEARD_ORIGINAL', 'SIGNAL_HEARD_PROLOGUE'),
    'SIGNAL_SCENE_CALL': ('SIGNAL_SCENE_SITE', 'SIGNAL_SCENE_BYTES'),
    'SIGNAL_SCENE_PROLOGUE': ('SIGNAL_SCENE_ORIGINAL', 'SIGNAL_SCENE_PROLOGUE'),
    # Called, not hooked (1.7.1-dev10).
    'SIGNAL_OBJECTIVE_PROLOGUE': ('SIGNAL_OBJECTIVE_SETTER', 'SIGNAL_OBJECTIVE_PROLOGUE'),
    # Called, not hooked (1.7.1-dev18): the plan replace helper.
    'SIGNAL_PLAN_REPLACE_PROLOGUE': ('SIGNAL_PLAN_REPLACE', 'SIGNAL_PLAN_REPLACE_PROLOGUE'),
}
# The lever tests' offsets a version without anchors keeps as placeholders.
SIGNAL_LEVERS = ['SIGNAL_TRACE_ON', 'SIGNAL_VISION_POS', 'SIGNAL_VISION_SEEN',
                 'SIGNAL_VISION_CHECKED', 'SIGNAL_VISION_DISTANCE', 'SIGNAL_OBJECTIVE_REASON',
                 'SIGNAL_OBJECTIVE_REASON_TICK']
# The driven windows' fields (1.7.1-dev17): placeholders without anchors.
SIGNAL_DRIVE = ['SIGNAL_PLAN_HOLD_WAVE', 'SIGNAL_PLAN_WAITING', 'SIGNAL_PLAN_SPLIT_ROLE',
                'SIGNAL_PLAN_REGROUP_NOW', 'SIGNAL_PLAN_PUSH_BASIS', 'SIGNAL_BATTLE_RUNAWAY',
                'SIGNAL_BATTLE_RUNAWAY_TICK', 'SIGNAL_SUPPORT_SIM_WORDS']
# The take order's reads (1.7.1-dev18): placeholders without anchors.
SIGNAL_PLAY = ['SIGNAL_SYNC_FOLLOWER', 'SIGNAL_CONTEXT_SHARED', 'SIGNAL_SHARED_SYNC',
               'SIGNAL_SHARED_RECORDS', 'SIGNAL_BRAIN_RECORDS', 'SIGNAL_RECEIVER_WORLD',
               'SIGNAL_WORLD_OTHER_RALLY', 'SIGNAL_RALLY', 'SIGNAL_ENV_OBJECT', 'SIGNAL_SPAWN_TICK',
               'SIGNAL_SERPEN_ALIVE']
# The big plan's lane by tag (1.7.1-dev12): the formatter's case for the
# tag, the payload formatter it calls, and that formatter's lea of the
# line field (opcode, ModRM, displacement size).
PLAN_LANES = {
    3: ('SIGNAL_PLAN_CASE_PASSIVE_LINE', 'SIGNAL_LANE_PASSIVE_LINE', '4c8d92', 4),
    4: ('SIGNAL_PLAN_CASE_SINGLE_LINE', 'SIGNAL_LANE_SINGLE_LINE', '488d5e', 1),
    10: ('SIGNAL_PLAN_CASE_LINE_GANKER', 'SIGNAL_LANE_LINE_GANKER', '4c8d42', 1),
    13: ('SIGNAL_PLAN_CASE_ATTACK_NEXUS', 'SIGNAL_LANE_ATTACK_NEXUS', '488d47', 1),
}
# Vectors {capacity, pointer, length}: the capacity where the code compares
# the length with it, the pointer and length where it appends.
SIGNAL_LISTS = {
    'SIGNAL_TEAM_QUEUE_A': ('SIGNAL_QUEUE_A_GROW', 'SIGNAL_QUEUE_A_APPEND'),
    'SIGNAL_TEAM_QUEUE_B': ('SIGNAL_QUEUE_B_GROW', 'SIGNAL_QUEUE_B_APPEND'),
    'SIGNAL_TEAM_LIST_C': ('SIGNAL_LIST_C_GROW', 'SIGNAL_LIST_C_APPEND'),
}


def verify_signal_fields(anchors, folder):
    """A version without SIGNAL_* anchors keeps placeholders only; one with
    them names exactly the offsets its anchored instructions use."""
    signals = (folder/'signals.rs').read_text(encoding='utf-8')
    if 'SIGNAL_STEP_CALL' not in anchors:
        placeholders = ['SIGNAL_SITE', 'SIGNAL_ORIGINAL', *SIGNAL_FIELDS, *SIGNAL_LISTS,
                        'SIGNAL_RECEIVER_MODE', 'SIGNAL_RECEIVER_STATE', 'SIGNAL_HEARD_SITES',
                        *(site for site, _ in SIGNAL_CALLS.values())]
        for name in [*placeholders, *SIGNAL_LEVERS, *SIGNAL_DRIVE, *SIGNAL_PLAY,
                     'SIGNAL_PLAYER_ENTITY']:
            assert 'usize::MAX' in declaration(signals, name), name
        assert set(numbers(declaration(signals, 'SIGNAL_PLAN_REPLACE_PROLOGUE'))) <= {0, 51}
        for name in ['SIGNAL_SHARED_STRIDE', 'SIGNAL_PLAN_SIZE', 'SIGNAL_RALLY_STRIDE',
                     'SIGNAL_WALK_SLACK']:
            assert numbers(declaration(signals, name)) == [0], name
        for name in ['SIGNAL_PLAN_PLAY', 'SIGNAL_FORCE_PASSIVE', 'SIGNAL_RECORD_NONE']:
            assert '::MAX' in declaration(signals, name), name
        for name in ['SIGNAL_PLAN_BATTLE', 'SIGNAL_PLAN_PASSIVE_LINE', 'SIGNAL_SUPPORT_SLOT']:
            assert '::MAX' in declaration(signals, name), name
        assert numbers(declaration(signals, 'SIGNAL_SUPPORT_MODES')) == [0]
        assert set(numbers(declaration(signals, 'SIGNAL_OBJECTIVE_PROLOGUE'))) <= {0, 30}
        assert set(numbers(declaration(signals, 'SIGNAL_GATE_RULES'))) <= {0, 5}
        assert declaration(signals, 'SIGNAL_ENV_VTABLES').strip() == '&[]'
        assert declaration(signals, 'SIGNAL_PLAN_LANES').strip() == '&[]'
        assert numbers(declaration(signals, 'SIGNAL_MARK')) == [0]
        return
    def code(name):
        return bytes.fromhex(anchors[name]['bytes'])
    for name, users in SIGNAL_FIELDS.items():
        value = struct.pack('<I', numbers(declaration(signals, name))[0])
        for user in users:
            assert value in code(user), (name, user)
    for name, (grow, append) in SIGNAL_LISTS.items():
        capacity = numbers(declaration(signals, name))[0]
        assert struct.pack('<I', capacity) in code(grow), name
        assert struct.pack('<I', capacity + 16) in code(grow), name
        assert struct.pack('<I', capacity + 8) in code(append), name
        assert struct.pack('<I', capacity + 16) in code(append), name
    # Entry sizes the shared probe assumes: queue A and B index * 5 * 8
    # (lea rcx, [r15 + r15*4]), list C index * 3 * 8 (lea rcx, [rbx + rbx*2]).
    assert bytes.fromhex('4b8d0cbf') in code('SIGNAL_QUEUE_A_APPEND')
    assert bytes.fromhex('4b8d0cbf') in code('SIGNAL_QUEUE_B_APPEND')
    assert bytes.fromhex('488d0c5b') in code('SIGNAL_LIST_C_APPEND')
    # The hooked call passes team, context, player and simulation in
    # rcx, rdx, r8, r9 (mov rcx, rbx; mov rdx, [rbp+0x120]; mov r8, rdi;
    # mov r9, r15) right before it.
    assert code('SIGNAL_STEP_ARGUMENTS').endswith(bytes.fromhex('4889d9488b95200100004989f84d89f9'))
    step = int(anchors['SIGNAL_STEP_ARGUMENTS']['rva'], 16) + len(code('SIGNAL_STEP_ARGUMENTS'))
    assert step == int(anchors['SIGNAL_STEP_CALL']['rva'], 16)
    # The step unpacks the pair: r9 = [pair], the stack argument = [pair + 8]
    # (mov r15, [rsi]; mov rax, [rsi + 8]; ...; mov [rsp + 0x20], rax).
    assert code('SIGNAL_STEP_ARGUMENTS').startswith(bytes.fromhex('4c8b3e488b4608'))
    assert bytes.fromhex('4889442420') in code('SIGNAL_STEP_ARGUMENTS')
    verify_signal_delivery(anchors, signals, code)
    verify_signal_environment(anchors, signals, code)
    verify_signal_answer(anchors, signals, code)
    verify_signal_levers(anchors, signals, code)
    verify_plan_lanes(anchors, signals, code)
    verify_ping_orders(anchors, signals, code)
    verify_drive_windows(anchors, signals, code)
    verify_take_play(anchors, signals, code)


def verify_take_play(anchors, signals, code):
    """The take order and the team's play (1.7.1-dev18): every offset the
    step hook reads for it, at the instructions the game reads it with (the
    brain's fields through the team plan, `SIGNAL_TEAM_PLAN` below them),
    and the plan replace helper the hook calls: its trace test, its drop
    and its 0x400-byte copy, and the Serpen handler's own call with
    ForcePassive."""
    def value(name):
        return numbers(declaration(signals, name))[0]
    def values(name):
        return numbers(declaration(signals, name))
    def disp(offset):
        return struct.pack('<I', offset)
    def rva(name):
        return int(anchors[name]['rva'], 16)
    def target(name):
        b = code(name)
        assert b[0] == 0xe8, name
        return rva(name) + 5 + struct.unpack('<i', b[1:5])[0]
    team = value('SIGNAL_TEAM_PLAN')
    # The publisher pick (its shared block read is 14 bytes into it) and
    # sync_follower.
    assert target('SIGNAL_PLAY_PUBLISHER_CALL') == rva('SIGNAL_PLAY_PICK_SHARED') - 0xe
    assert code('SIGNAL_PLAY_FOLLOWER_WRITE') == bytes.fromhex('8886') + disp(value('SIGNAL_SYNC_FOLLOWER') - team)
    # The shared block: [context + 0x10] + side * stride, its sync at + 0xd8.
    shared, stride = value('SIGNAL_CONTEXT_SHARED'), value('SIGNAL_SHARED_STRIDE')
    assert code('SIGNAL_PLAY_SHARED') == bytes([0x48, 0x8b, 0x5f, shared]) + bytes.fromhex('4969ce') + disp(stride)
    assert code('SIGNAL_PLAY_PICK_SHARED') == bytes([0x48, 0x8b, 0x42, shared]) + bytes.fromhex('4c69c9') + disp(stride)
    assert code('SIGNAL_PLAY_PICK_UNIT') == bytes.fromhex('4883bcd0') + disp(value('SIGNAL_SIM_UNITS')) + b'\0'
    sync = value('SIGNAL_SHARED_SYNC')
    assert code('SIGNAL_PLAY_MIRROR') == bytes.fromhex('4c8b940b') + disp(sync)
    assert code('SIGNAL_PLAY_SYNC_TEST') == bytes.fromhex('4983bc24') + disp(sync) + b'\xff'
    # The records' None bytes: the brain's (team plan + 0xa8b, + 0xaab) and
    # the sync's Serpen one (+ 0x9b), the Morgard one 0x20 further.
    none = value('SIGNAL_RECORD_NONE')
    own = values('SIGNAL_BRAIN_RECORDS')
    for name, at in (('SIGNAL_PLAY_SERPEN_RECORD', own[0]), ('SIGNAL_PLAY_MORGARD_RECORD', own[1])):
        assert code(name) == bytes.fromhex('80be') + disp(at - team) + bytes([none]), name
    records = values('SIGNAL_SHARED_RECORDS')
    assert code('SIGNAL_PLAY_SYNC_RECORD') == bytes.fromhex('80b8') + disp(records[0] - sync) + bytes([none])
    assert records[1] - records[0] == own[1] - own[0] == 0x20
    # The hunt plans, built by the builders and tested by the handler.
    serpen, morgard = values('SIGNAL_PLAN_PLAY')
    assert code('SIGNAL_WALK_HUNT') == bytes.fromhex('49c70424') + disp(serpen)
    assert code('SIGNAL_HUNT_PLAN_TEST') == bytes.fromhex('833a') + bytes([serpen])
    assert code('SIGNAL_WALK_MORGARD_HUNT') == bytes.fromhex('48c706') + disp(morgard)
    # The replace helper: traces only when trace_on, else drops the old
    # plan and copies the buffer; the handler passes ForcePassive.
    trace = value('SIGNAL_TRACE_ON') - team
    assert code('SIGNAL_REPLACE_TRACE') == bytes.fromhex('80b9') + disp(trace) + b'\0'
    assert code('SIGNAL_PLAN_REPLACE_PROLOGUE').endswith(code('SIGNAL_REPLACE_TRACE') + bytes.fromhex('0f84fa020000'))
    assert rva('SIGNAL_REPLACE_TRACE') == value('SIGNAL_PLAN_REPLACE') + 0x26
    # With tracing off the jump lands on the drop: je +0x2fa from 0xec3bd3.
    assert rva('SIGNAL_REPLACE_DROP') == rva('SIGNAL_REPLACE_TRACE') + 7 + 6 + 0x2fa + 3
    assert code('SIGNAL_REPLACE_COPY').startswith(b'\x41\xb8' + disp(value('SIGNAL_PLAN_SIZE')))
    assert rva('SIGNAL_REPLACE_COPY') == rva('SIGNAL_REPLACE_DROP') + 5
    assert target('SIGNAL_HANDLER_REPLACE_CALL') == value('SIGNAL_PLAN_REPLACE')
    # The plan the hook passes is the one the handler passes on to the
    # cascade (lea rax, [r13 + 0x16a0], the brain in r13), and the handler
    # rebuilds a ForcePassive plan after the cascade (cmp dword [rax], 2).
    assert code('SIGNAL_HANDLER_PLAN') == bytes.fromhex('498d85') + disp(value('SIGNAL_BIG_PLAN'))
    assert code('SIGNAL_REBUILD_TEST') == bytes.fromhex('8338') + bytes([value('SIGNAL_FORCE_PASSIVE')])
    assert code('SIGNAL_FORCE_PASSIVE_ARG') == bytes.fromhex('48c7442478') + disp(value('SIGNAL_FORCE_PASSIVE'))
    # The walk rule: the world at receiver + 0x20 and its other branch, the
    # rally points, the spawn ticks against the world tick, the speed, and
    # travel + 2 x the clock word.
    world, other = value('SIGNAL_RECEIVER_WORLD'), value('SIGNAL_WORLD_OTHER_RALLY')
    assert code('SIGNAL_WALK_WORLD') == bytes([0x4d, 0x8b, 0x62, world]) + bytes.fromhex('418b8424') + disp(other)
    assert code('SIGNAL_WALK_MORGARD_WORLD') == bytes([0x4d, 0x8b, 0x6e, world]) + bytes.fromhex('418b85') + disp(other)
    rally = values('SIGNAL_RALLY')
    assert code('SIGNAL_WALK_RALLY') == (bytes.fromhex('4f8bb42c') + disp(rally[0])
                                         + bytes.fromhex('4f8ba42c') + disp(rally[0] + 8))
    assert code('SIGNAL_WALK_MORGARD_RALLY') == (bytes.fromhex('4d8ba405') + disp(rally[1])
                                                 + bytes.fromhex('4d8bac05') + disp(rally[1] + 8))
    assert value('SIGNAL_RALLY_STRIDE') == 0x10
    spawn = values('SIGNAL_SPAWN_TICK')
    assert code('SIGNAL_WALK_SPAWN') == (bytes.fromhex('4c8b9a') + disp(spawn[0]) + bytes.fromhex('4531d2')
                                         + bytes.fromhex('4c2b9e') + disp(value('SIGNAL_WORLD_TICK')))
    assert code('SIGNAL_WALK_MORGARD_SPAWN') == bytes.fromhex('488bba') + disp(spawn[1])
    assert code('SIGNAL_WALK_ALIVE') == bytes.fromhex('4883ba') + disp(value('SIGNAL_SERPEN_ALIVE')) + b'\0'
    assert code('SIGNAL_WALK_SPEED') == bytes.fromhex('498b8d') + disp(value('SIGNAL_UNIT_SPEED'))
    clock = disp(value('SIGNAL_STATE_DELAY'))
    # lea rax, [rax + rcx*2]: the clock word times two.
    assert value('SIGNAL_WALK_SLACK') == 2
    assert code('SIGNAL_WALK_RULE') == bytes.fromhex('488b89') + clock + bytes.fromhex('488d04484939c3')
    assert code('SIGNAL_WALK_MORGARD_RULE') == bytes.fromhex('488b89') + clock + bytes.fromhex('488d04484839c7')
    # The kind-0 environment's kind method returns its object beside kind 0.
    assert code('SIGNAL_ENV_KIND0_OBJECT') == bytes.fromhex('488d91') + disp(value('SIGNAL_ENV_OBJECT'))


def verify_drive_windows(anchors, signals, code):
    """The driven windows (1.7.1-dev17): each PassiveLine flag at the
    displacement the sub plan choice, the builder and the support rule use
    (payload-relative where the code addresses the payload), the run-away
    latch where the fight plan reads, sets and clears it, the plan tags, and
    every gate of the support rule the mod mirrors."""
    def value(name):
        return numbers(declaration(signals, name))[0]
    def disp(offset):
        return struct.pack('<I', offset)
    payload = value('SIGNAL_BIG_PLAN_ARG')
    plan = value('SIGNAL_BIG_PLAN')
    hold, waiting = value('SIGNAL_PLAN_HOLD_WAVE'), value('SIGNAL_PLAN_WAITING')
    regroup, push = value('SIGNAL_PLAN_REGROUP_NOW'), value('SIGNAL_PLAN_PUSH_BASIS')
    split = value('SIGNAL_PLAN_SPLIT_ROLE')
    passive = value('SIGNAL_PLAN_PASSIVE_LINE')
    assert payload == plan + 8
    # The sub plan choice: cmp byte [rdx + flag - payload], 0.
    assert code('SIGNAL_REGROUP_READ') == bytes.fromhex('80ba') + disp(regroup - payload) + b'\0'
    assert code('SIGNAL_HOLD_READ') == bytes.fromhex('80ba') + disp(hold - payload) + b'\0'
    # The builder writes hold_wave and waiting as one word of 1, 1, and
    # push_basis 5.
    assert waiting == hold + 1
    assert code('SIGNAL_HOLD_BUILDER_WRITE') == bytes.fromhex('66c786') + disp(hold - payload) + bytes.fromhex('0101')
    assert code('SIGNAL_PUSH_BASIS_WRITE') == bytes.fromhex('c686') + disp(push - payload) + b'\5'
    # line: PassiveLine's entry of the plan lanes, compared by the support
    # rule with the objective's lane byte.
    lanes = dict(re.findall(r'\((\d+),\s*(0x[0-9a-f]+)\)', declaration(signals, 'SIGNAL_PLAN_LANES')))
    line = int(lanes[str(passive)], 16)
    # The support rule: slot, mode byte and mask, the two simulation words,
    # the plan, split_role, the objective against the line, its writes.
    assert code('SIGNAL_SUPPORT_SLOT') == bytes.fromhex('83bd901b0000') + bytes([value('SIGNAL_SUPPORT_SLOT')])
    assert code('SIGNAL_SUPPORT_MODE_READ') == bytes.fromhex('0fb651') + bytes([value('SIGNAL_RECEIVER_MODE')])
    assert code('SIGNAL_SUPPORT_MODE_MASK') == b'\xb8' + struct.pack('<I', value('SIGNAL_SUPPORT_MODES'))
    words = numbers(declaration(signals, 'SIGNAL_SUPPORT_SIM_WORDS'))
    assert [code('SIGNAL_SUPPORT_SIM_A'), code('SIGNAL_SUPPORT_SIM_B')] == \
        [bytes.fromhex('4883b8') + disp(w) + b'\0' for w in words]
    assert code('SIGNAL_SUPPORT_PLAN') == bytes.fromhex('4183ba') + disp(plan) + bytes([passive])
    assert code('SIGNAL_SUPPORT_SPLIT') == bytes.fromhex('4180ba') + disp(split) + b'\0'
    objective = value('SIGNAL_OBJECTIVE')
    assert code('SIGNAL_SUPPORT_OBJECTIVE') == bytes.fromhex('410fb68a') + disp(objective) + bytes.fromhex('410fb682') + disp(objective + 1)
    assert code('SIGNAL_SUPPORT_LANE') == bytes.fromhex('413a82') + disp(line)
    assert code('SIGNAL_SUPPORT_HOLD_WRITE') == bytes.fromhex('4188b2') + disp(hold)
    assert code('SIGNAL_SUPPORT_WAIT_WRITE') == bytes.fromhex('418882') + disp(waiting)
    # The latch: BattlePlan + 0x20 (tag) and + 0x28 (tick), the plan at
    # brain + SIGNAL_BIG_PLAN; set to (1, now + 5 x the clock word), read
    # against the world tick, cleared once reached.
    tag, tick = value('SIGNAL_BATTLE_RUNAWAY') - plan, value('SIGNAL_BATTLE_RUNAWAY_TICK') - plan
    assert (tag, tick) == (0x20, 0x28)
    world = disp(value('SIGNAL_WORLD_TICK'))
    assert code('SIGNAL_RUNAWAY_SET') == (bytes.fromhex('488b80') + disp(value('SIGNAL_STATE_DELAY'))
                                          + bytes.fromhex('488d0480') + bytes.fromhex('480386') + world
                                          + bytes.fromhex('48c747') + bytes([tag]) + disp(1)
                                          + bytes.fromhex('488947') + bytes([tick]))
    assert code('SIGNAL_RUNAWAY_TAG_READ') == bytes.fromhex('807f') + bytes([tag]) + b'\0'
    assert code('SIGNAL_RUNAWAY_TICK_READ') == bytes.fromhex('488b86') + world + bytes.fromhex('483b47') + bytes([tick])
    assert code('SIGNAL_RUNAWAY_CLEAR') == bytes.fromhex('48c747') + bytes([tag]) + disp(0)
    # Battle ends only at sub goal End (4): RunAway (3) keeps the plan.
    assert code('SIGNAL_BATTLE_DONE') == bytes.fromhex('83b9e000000004')
    # The plan tags: Battle raw 0 and 1, PassiveLine 3 (its lane entry).
    assert numbers(declaration(signals, 'SIGNAL_PLAN_BATTLE')) == [0, 1]
    assert passive == 3


def verify_ping_orders(anchors, signals, code):
    """The ping orders (1.7.1-dev13): the drop reason's tick next to its
    byte, the receive handler's Mia and BattleStop cases (the AI
    environment's kind check that ends both in kind 0; where the Mia case
    writes its stamps, between the vision positions and the seen ticks),
    and the objective formatter's two
    Morgard/Serpen fields (byte 1 phase, byte 2 with_battle).

    Superseded 2026-11-04 (1.7.1-dev17): the cases' tags and dev15's
    BattleStop record are no longer checked: the mod sends neither call."""
    def value(name):
        return numbers(declaration(signals, name))[0]
    def disp(offset):
        return struct.pack('<I', offset)
    def rva(name):
        return int(anchors[name]['rva'], 16)
    def rel(data, at):
        return struct.unpack_from('<i', data, at)[0]
    plan = value('SIGNAL_TEAM_PLAN')
    # The tick, a u64 right after the byte: mov [r13 + tick], rax after the
    # 0x20 store; mov [rcx + tick], rax after the 0x21 store.
    tick = value('SIGNAL_OBJECTIVE_REASON_TICK')
    assert tick == value('SIGNAL_OBJECTIVE_REASON') + 8
    assert code('SIGNAL_REASON_TICK_MODE_GATE') == bytes.fromhex('498985') + disp(tick - plan)
    assert code('SIGNAL_REASON_TICK_SERPEN') == bytes.fromhex('488981') + disp(tick - plan)
    assert rva('SIGNAL_REASON_TICK_MODE_GATE') == rva('SIGNAL_REASON_MODE_GATE') + len(code('SIGNAL_REASON_MODE_GATE'))
    assert rva('SIGNAL_REASON_TICK_SERPEN') == rva('SIGNAL_REASON_SERPEN') + len(code('SIGNAL_REASON_SERPEN'))
    for case in ('SIGNAL_MIA_CASE', 'SIGNAL_STOP_CASE'):
        # [pair] (the environment), call [vtable + 0x40], test rax, rax; je.
        assert bytes.fromhex('488b3f488b0f488b4708ff50404885c00f84') in code(case), case
    # Mia: mov ebx, [rax + 4] (the enemy index), then after the check the
    # world tick into [rsi + rbx * 8 + 0xc40].
    mia = code('SIGNAL_MIA_CASE')
    assert mia.startswith(bytes.fromhex('8b5804'))
    assert bytes.fromhex('488b87') + disp(value('SIGNAL_WORLD_TICK')) in mia
    # The stamps lie between the positions and the seen ticks, never
    # written by the fog ping (SIGNAL_MIA_STAMPS went with the research key
    # F9 in 1.7.1-dev21).
    stamps = value('SIGNAL_VISION_POS') + 5 * 16
    assert mia.endswith(bytes.fromhex('488984de') + disp(stamps))
    assert stamps + 5 * 8 == value('SIGNAL_VISION_SEEN')
    # The formatter: rdi = &byte 1, [rsp + 0x48] = &byte 2; "phase" (5)
    # printed from rdi, "with_battle" (11) from [rsp + 0x48].
    assert code('SIGNAL_OBJECTIVE_FIELDS') == bytes.fromhex('488d78014883c0024889442448')
    names = code('SIGNAL_OBJECTIVE_FIELD_NAMES')
    strings = code('SIGNAL_OBJECTIVE_NAME_STRINGS')
    assert strings == b'phasewith_battle'
    leas = [i for i in range(len(names) - 7) if names[i:i + 3] == bytes.fromhex('488d15')]
    assert len(leas) == 2, leas
    targets = [rva('SIGNAL_OBJECTIVE_FIELD_NAMES') + i + 7 + rel(names, i + 3) for i in leas]
    assert targets == [rva('SIGNAL_OBJECTIVE_NAME_STRINGS'), rva('SIGNAL_OBJECTIVE_NAME_STRINGS') + 5], targets
    assert bytes.fromhex('41b8050000004889f14989f9') in names
    assert names.endswith(bytes.fromhex('4c8d4c244841b80b000000'))


def verify_signal_answer(anchors, signals, code):
    """The scene call's answer (1.7.1-dev8): the second word of each pair the
    probe reads, the Ok reply onto list C and the join counter after it, the
    unsigned hp division, and the mark."""
    def value(name):
        return numbers(declaration(signals, name))[0]
    def disp(offset):
        return struct.pack('<I', offset)
    # x, then y 8 bytes on, in the opener and in the scene block.
    assert disp(value('SIGNAL_UNIT_POS') + 8) in code('SIGNAL_UNIT_POS_READ')
    assert disp(value('SIGNAL_SCENE_POS') + 8) in code('SIGNAL_SCENE_WRITE')
    # The scene block, each field by its own store: the first set to 1
    # (mov qword [rsi + block], 1), the tick from r14, x and y from rcx and
    # rax, the distance from rdi, the byte from bl.
    scene = code('SIGNAL_SCENE_WRITE')
    for store in [bytes.fromhex('48c786') + disp(value('SIGNAL_SCENE_BLOCK')) + disp(1),
                  bytes.fromhex('4c89b6') + disp(value('SIGNAL_SCENE_TICK')),
                  bytes.fromhex('48898e') + disp(value('SIGNAL_SCENE_POS')),
                  bytes.fromhex('488986') + disp(value('SIGNAL_SCENE_POS') + 8),
                  bytes.fromhex('4889be') + disp(value('SIGNAL_SCENE_DIST')),
                  bytes.fromhex('889e') + disp(value('SIGNAL_SCENE_FLAG'))]:
        assert store in scene, store.hex()
    # The units table: [rdx + rcx*8 + units], rdx the side's row (lea r8,
    # [r10 + r10*4]: side * 5).
    assert bytes.fromhex('4c8bb4ca') + disp(value('SIGNAL_SIM_UNITS')) in code('SIGNAL_SCENE_KIND_READ')
    assert bytes.fromhex('4f8d0492') in code('SIGNAL_SCENE_KIND_READ')
    # hp * 100 (imul rax, [r14 + hp], 0x64) over max hp, unsigned (div rcx).
    assert bytes.fromhex('496b86') + disp(value('SIGNAL_UNIT_HP')) + bytes([0x64]) in code('SIGNAL_UNIT_HP_READ')
    assert code('SIGNAL_UNIT_HP_READ').count(bytes.fromhex('48f7f1')) == 2
    # The Ok reply: list C's length, capacity and pointer; tag 9 at +0
    # (mov byte [rax + rcx*8], 9), a qword 0 at +8, the length stored back,
    # then join (the eighth counter) += 1.
    push = code('SIGNAL_REPLY_OK_PUSH')
    c = value('SIGNAL_TEAM_LIST_C')
    for offset in (c, c + 8, c + 16):
        assert disp(offset) in push, offset
    assert bytes.fromhex('488d0c5b') in push
    assert bytes.fromhex('c604c809') in push
    assert bytes.fromhex('48c744c80800000000') in push
    assert push.endswith(bytes.fromhex('48ff86') + disp(value('SIGNAL_JOIN_STATS') + 7 * 8))
    # The BattleHelp case reads the target (mov r14, [rax + 8]) and goes
    # straight on to the kind check: no read of the word at +4.
    assert code('SIGNAL_HELP_TARGET_READ') == bytes.fromhex('4c8b7008')
    end = int(anchors['SIGNAL_HELP_TARGET_READ']['rva'], 16) + 4
    assert end == int(anchors['SIGNAL_HELP_KIND_CHECK']['rva'], 16)
    mark = int(declaration(signals, 'SIGNAL_MARK').strip().replace('_', ''), 0)
    assert 0 < mark < 1 << 32 and mark != 0xffffffff, mark


def verify_signal_levers(anchors, signals, code):
    """The per-player lever tests (1.7.1-dev10): the objective setter (its
    trace test, the side fields it clears by tag, the 3 bytes it writes),
    the inbox HideLine's path to Gank(lane) through it, and the vision
    table's fields with the squared distance, against the anchored code."""
    def value(name):
        return numbers(declaration(signals, name))[0]
    def disp(offset):
        return struct.pack('<I', offset)
    def rva(name):
        return int(anchors[name]['rva'], 16)
    plan = value('SIGNAL_TEAM_PLAN')
    def in_plan(name):
        return disp(value(name) - plan)
    # The setter tests the trace flag on its team plan argument (cmp byte
    # [rcx + trace], 0) and jumps to the stores when it is 0.
    trace = code('SIGNAL_OBJECTIVE_TRACE_TEST')
    assert trace[:7] == bytes.fromhex('80b9') + in_plan('SIGNAL_TRACE_ON') + bytes(1)
    assert trace[7:9] == bytes.fromhex('0f84')
    jump = rva('SIGNAL_OBJECTIVE_TRACE_TEST') + len(trace) + struct.unpack('<i', trace[9:13])[0]
    assert jump == rva('SIGNAL_OBJECTIVE_STORES')
    # The stores by the new tag (cmp dl, 2 Defense; cmp eax, 0xff none;
    # cmp dl, 8 PressTower; cmp dl, 5 Repair): def_depart (+0x778, twice),
    # press_follower (+0xd58, twice), repair_line (+0xd5b = 0xff),
    # repair_owner (+0x798); byte 2, then the word; then ret, no call.
    stores = code('SIGNAL_OBJECTIVE_STORES')
    for test in ['80fa02', '3dff000000', '80fa08', '80fa05']:
        assert bytes.fromhex(test) in stores, test
    assert stores.count(bytes.fromhex('48c786') + disp(0x778) + disp(0)) == 2
    assert stores.count(bytes.fromhex('c686') + disp(0xd58) + bytes(1)) == 2
    assert bytes.fromhex('c686') + disp(0xd5b) + bytes([0xff]) in stores
    assert bytes.fromhex('48c786') + disp(0x798) + disp(0) in stores
    assert bytes.fromhex('8886') + disp(value('SIGNAL_OBJECTIVE') - plan + 2) in stores
    assert bytes.fromhex('668986') + in_plan('SIGNAL_OBJECTIVE') in stores
    assert bytes.fromhex('4881c4080100005b5f5e415e415f5dc3') in stores
    assert bytes.fromhex('e8') not in stores[:stores.index(bytes.fromhex('5b5f5e415e415f5dc3'))]
    # The handler's dispatch: tag - 1 (dec ecx; cmp ecx, 10) through the
    # table (lea r10, [rip + d]); HideLine and HideLineToo share the case.
    dispatch = code('SIGNAL_HANDLER_DISPATCH')
    assert dispatch.startswith(bytes.fromhex('0fb608ffc983f90a'))
    at = dispatch.index(bytes.fromhex('4c8d15'))
    table = rva('SIGNAL_HANDLER_DISPATCH') + at + 7 + struct.unpack('<i', dispatch[at+3:at+7])[0]
    assert table == rva('SIGNAL_HANDLER_TAGS')
    tags = code('SIGNAL_HANDLER_TAGS')
    # HideLine's tag, 6 (the research key F5 sent it until 1.7.1-dev21; the
    # case still guards the objective and plan offsets the pings use).
    hide = 6
    for tag in (hide, hide + 1):
        assert table + struct.unpack_from('<i', tags, (tag - 1) * 4)[0] == rva('SIGNAL_HIDE_CASE'), tag
    # The case reads the lane (movzx ebx, byte [rax + 1]), then tests the
    # big plan for PassiveLine (cmp dword [rsi + plan], 3).
    assert code('SIGNAL_HIDE_CASE') == (bytes.fromhex('0fb6580183be') + disp(value('SIGNAL_BIG_PLAN'))
                                         + bytes([3]))
    # Its end: no objective (cmp byte [rsi + objective], 0xff), slots 0, 2,
    # 3, 4 (bt 0x1d on the player index), the slot's lane from 0x202010000
    # equal to the call's (cmp bl, al), then the setter with Gank(lane):
    # lea rcx, [rsi + team plan]; edx = lane << 8 | 6.
    gank = code('SIGNAL_HIDE_OBJECTIVE')
    assert gank.startswith(bytes.fromhex('80be') + disp(value('SIGNAL_OBJECTIVE')) + bytes([0xff]))
    assert bytes.fromhex('418b89') + disp(value('SIGNAL_PLAYER_INDEX')) in gank
    assert bytes.fromhex('b81d0000000fa3c8') in gank
    assert bytes.fromhex('48b80000010202000000') in gank
    assert bytes.fromhex('38c3') in gank
    assert bytes.fromhex('488d8e') + disp(plan) in gank
    at = gank.index(bytes.fromhex('0fb6d3c1e20883ca06')) + 9 + 7
    assert gank[at] == 0xe8
    target = rva('SIGNAL_HIDE_OBJECTIVE') + at + 5 + struct.unpack('<i', gank[at+1:at+5])[0]
    assert target == value('SIGNAL_OBJECTIVE_SETTER')
    # The vision table: the position base kept at [rbp + 0x2d0] from the
    # team plan at [rbp + 0x2f0].
    assert code('SIGNAL_VISION_BASE') == (bytes.fromhex('488b85f00200004c8db8') + in_plan('SIGNAL_VISION_POS')
                                          + bytes.fromhex('4c89bdd0020000'))
    vision = code('SIGNAL_VISION_STORES')
    # By the enemy's player index (r9d), his unit from the units table.
    assert vision.startswith(bytes.fromhex('458b88') + disp(value('SIGNAL_PLAYER_INDEX')))
    assert bytes.fromhex('488d97') + disp(value('SIGNAL_SIM_UNITS')) in vision
    # Seen and checked = the world tick.
    assert bytes.fromhex('488b8f') + disp(value('SIGNAL_WORLD_TICK')) in vision
    for name in ('SIGNAL_VISION_SEEN', 'SIGNAL_VISION_CHECKED'):
        assert bytes.fromhex('4b898ccf') + in_plan(name) in vision, name
    # The distance is squared: imul, imul, add, stored; no root.
    assert bytes.fromhex('488b88') + disp(value('SIGNAL_UNIT_POS')) in vision
    assert bytes.fromhex('488b90') + disp(value('SIGNAL_UNIT_POS') + 8) in vision
    assert (bytes.fromhex('4d0fafd24d0fafdb4d01d34f899ccf') + in_plan('SIGNAL_VISION_DISTANCE')) in vision
    # The position: index * 16 into the base, x then y.
    assert bytes.fromhex('4c8bbdd0020000') in vision
    assert bytes.fromhex('41c1e2044b890c174b89541708') in vision
    # Five entries each: positions (16 bytes), the Mia stamps, then seen,
    # checked and the distance (8 bytes); the big plan is the dev8 kind.
    assert value('SIGNAL_VISION_POS') + 5 * 16 + 5 * 8 == value('SIGNAL_VISION_SEEN')
    assert value('SIGNAL_VISION_SEEN') + 40 == value('SIGNAL_VISION_CHECKED')
    assert value('SIGNAL_VISION_CHECKED') + 40 == value('SIGNAL_VISION_DISTANCE')
    assert value('SIGNAL_BIG_PLAN') == value('SIGNAL_SCENE_KIND')
    assert value('SIGNAL_BIG_PLAN_ARG') == value('SIGNAL_SCENE_ARG')


def verify_plan_lanes(anchors, signals, code):
    """The standing request's reads (1.7.1-dev12): the objective handlers'
    drop reason byte (two of its stores), and the big plan's lane by tag:
    the formatter's jump table by tag - 2, each case passing the payload at
    plan + 8 to its payload formatter, and that formatter's line field."""
    def value(name):
        return numbers(declaration(signals, name))[0]
    def rva(name):
        return int(anchors[name]['rva'], 16)
    def rel(data, at):
        return struct.unpack_from('<i', data, at)[0]
    reason = struct.pack('<I', value('SIGNAL_OBJECTIVE_REASON') - value('SIGNAL_TEAM_PLAN'))
    # mov byte [r13 + reason], 0x20; mov rcx, [rsp + 0x48]; mov byte [rcx + reason], 0x21.
    assert code('SIGNAL_REASON_MODE_GATE') == bytes.fromhex('41c685') + reason + bytes([0x20])
    assert code('SIGNAL_REASON_SERPEN') == bytes.fromhex('488b4c2448c681') + reason + bytes([0x21])
    # mov rax, [rcx]; sub rax, 2; ...; lea rax, [rip + table]; jmp.
    dispatch = code('SIGNAL_PLAN_DISPATCH')
    assert dispatch.startswith(bytes.fromhex('488b014883e802'))
    assert dispatch.endswith(bytes.fromhex('ffe2'))
    at = dispatch.index(bytes.fromhex('488d05'))
    table = rva('SIGNAL_PLAN_DISPATCH') + at + 7 + rel(dispatch, at + 3)
    assert table == rva('SIGNAL_PLAN_TABLE')
    tags = code('SIGNAL_PLAN_TABLE')
    payload = value('SIGNAL_BIG_PLAN') + 8
    assert payload == value('SIGNAL_BIG_PLAN_ARG')
    lanes = {int(tag): int(offset, 16) for tag, offset in
             re.findall(r'\(\s*([0-9]+)\s*,\s*(0x[0-9a-f]+)\s*\)', declaration(signals, 'SIGNAL_PLAN_LANES'))}
    assert set(lanes) == set(PLAN_LANES), lanes
    for tag, offset in lanes.items():
        case, inner, lea, size = PLAN_LANES[tag]
        assert table + rel(tags, (tag - 2) * 4) == rva(case), tag
        body = code(case)
        # add rcx, 8 (the payload); ...; call the payload's formatter.
        assert body.startswith(bytes.fromhex('4883c108')), tag
        assert body[-5] == 0xe8, tag
        assert rva(case) + len(body) + rel(body, len(body) - 4) == rva(inner), tag
        # mov reg, [rcx] (the payload), then lea reg, [reg + field].
        field = offset - payload
        assert 0 <= field < 1 << (8 * size) and (size == 4 or field < 0x80), tag
        assert bytes.fromhex(lea) + field.to_bytes(size, 'little') in code(inner), tag


def verify_signal_environment(anchors, signals, code):
    """The AI environment (1.7.1-dev7): each listed vtable is a Rust vtable
    (drop pointer, size, align 8) whose slot 0x40 is the kind method of its
    kind, and the receive handler asks that method before each scene."""
    bodies = {255: 'SIGNAL_ENV_WRAPPER_BODY', 0: 'SIGNAL_ENV_KIND0_BODY',
              1: 'SIGNAL_ENV_KIND1_BODY', 2: 'SIGNAL_ENV_KIND2_BODY'}
    table = re.findall(r'\(\s*(0x[0-9a-f]+)\s*,\s*(u8::MAX|[0-9]+)\s*\)',
                       declaration(signals, 'SIGNAL_ENV_VTABLES'))
    assert len(table) == 6, table
    for rva, kind in table:
        kind = 255 if kind == 'u8::MAX' else int(kind)
        tag = f'SIGNAL_ENV_{int(rva, 16):X}'
        assert int(anchors[f'{tag}_DROP']['rva'], 16) == int(rva, 16), rva
        assert int(anchors[f'{tag}_LAYOUT']['rva'], 16) == int(rva, 16) + 8, rva
        assert code(f'{tag}_LAYOUT')[8:] == struct.pack('<Q', 8), rva
        assert int(anchors[f'{tag}_KIND']['rva'], 16) == int(rva, 16) + 0x40, rva
        target = int(anchors[f'{tag}_KIND']['pointer_target'], 16)
        assert target == int(anchors[bodies[kind]]['rva'], 16), (rva, kind)
    # Each kind method: lea rdx, [rcx + d]; then eax = kind; ret. The
    # wrapper forwards to slot 0x40 of the inner object's vtable.
    assert code('SIGNAL_ENV_KIND0_BODY').endswith(bytes.fromhex('31c0c3'))
    for kind in (1, 2):
        assert code(f'SIGNAL_ENV_KIND{kind}_BODY').endswith(b'\xb8' + struct.pack('<I', kind) + b'\xc3')
    assert code('SIGNAL_ENV_WRAPPER_BODY') == bytes.fromhex('488b01488b51084889c148ff6240')
    # The handler's cases: call [vtable + 0x40], mov cl, reason, test rax,
    # rax; no scene when it is 0 (Battle and BattleDive jump to the scene
    # call only when not 0, BattleHelp jumps past it when 0).
    for name, reason, branch in [('SIGNAL_BATTLE_KIND_CHECK', 1, '0f85'),
                                 ('SIGNAL_DIVE_KIND_CHECK', 3, '75'),
                                 ('SIGNAL_HELP_KIND_CHECK', 2, '0f84')]:
        body = code(name)
        assert bytes.fromhex('ff5040') in body, name
        at = body.index(bytes([0xb1, reason]) + bytes.fromhex('4885c0'))
        assert body[at + 5:].startswith(bytes.fromhex(branch)), name
        end = int(anchors[name]['rva'], 16) + len(body)
        jump = struct.unpack_from('<i' if len(branch) == 4 else '<b', body, at + 5 + len(branch) // 2)[0]
        scene = int(anchors['SIGNAL_SCENE_CALL']['rva'], 16)
        if reason == 2:
            assert end == scene - 0x1a, name
        else:
            assert end + jump == scene - 0x1a, name


def verify_signal_delivery(anchors, signals, code):
    """The delivery probe (1.7.1-dev6): hooked calls, the receiver fields
    and the gate's rules per sender slot, against the anchored code."""
    def rva(name):
        return int(anchors[name]['rva'], 16)
    for name, (site, value) in SIGNAL_CALLS.items():
        assert numbers(declaration(signals, site)) == [rva(name)], name
        assert bytes(numbers(declaration(signals, value))).hex() == anchors[name]['bytes'], name
    sites = numbers(declaration(signals, 'SIGNAL_HEARD_SITES'))
    calls = numbers(declaration(signals, 'SIGNAL_HEARD_BYTES'))
    assert sites == [rva('SIGNAL_HEARD_CALL_1'), rva('SIGNAL_HEARD_CALL_2')]
    assert bytes(calls).hex() == code('SIGNAL_HEARD_CALL_1').hex() + code('SIGNAL_HEARD_CALL_2').hex()
    for call in ['SIGNAL_HEARD_CALL_1', 'SIGNAL_HEARD_CALL_2']:
        assert int(anchors[call]['target'], 16) == rva('SIGNAL_HEARD_PROLOGUE'), call
    assert int(anchors['SIGNAL_SCENE_CALL']['target'], 16) == rva('SIGNAL_SCENE_PROLOGUE')
    # Arguments right before the late drop (rcx brain, rdx due, r8d sender,
    # r9 &signal) and the gate (pair, sender, &signal, flag, extra on the
    # stack; brain, context, ?, player state in registers).
    for arguments, call in [('SIGNAL_LATE_ARGUMENTS', 'SIGNAL_LATE_CALL'),
                            ('SIGNAL_GATE_ARGUMENTS', 'SIGNAL_GATE_CALL')]:
        assert rva(arguments) + len(code(arguments)) == rva(call), arguments
    assert code('SIGNAL_LATE_ARGUMENTS').endswith(bytes.fromhex('4c89f24189d84c8d4d50'))
    # The handler takes the signal from its seventh argument.
    assert code('SIGNAL_HANDLER_SIGNAL') == bytes.fromhex('488b8424e0040000')
    # BattleHelp: the target at signal + 8, reason 2 (mov cl, 2).
    assert code('SIGNAL_HELP_CASE').startswith(bytes.fromhex('4c8b7008'))
    assert code('SIGNAL_HELP_CASE').endswith(bytes.fromhex('b102'))
    # The receiver: its mode byte (mov r12, [pair + 8]; movzx ecx, byte
    # [r12 + mode]) and the state holding the delay (mov rax, [r12 + state]).
    mode = numbers(declaration(signals, 'SIGNAL_RECEIVER_MODE'))[0]
    state = numbers(declaration(signals, 'SIGNAL_RECEIVER_STATE'))[0]
    assert code('SIGNAL_MODE_READ') == bytes.fromhex('4c8b6008410fb64c24') + bytes([mode])
    assert code('SIGNAL_DELAY_READ').startswith(bytes.fromhex('498b4424') + bytes([state]))
    # Eight join counters: the last at +0x38.
    join = numbers(declaration(signals, 'SIGNAL_JOIN_STATS'))[0]
    assert code('SIGNAL_JOIN_LAST') == bytes.fromhex('48ff86') + struct.pack('<I', join + 0x38)
    # The gate's rule per sender slot: the jump table (lea r8, [rip + d] at
    # dispatch + 3) sends slots 0, 2, 3, 4 to a mask case (cmp mode, 8; ja
    # reject; mov ecx, mask) and slot 1 to its own code.
    dispatch = code('SIGNAL_GATE_DISPATCH')
    assert dispatch[3:6] == bytes.fromhex('4c8d05')
    assert rva('SIGNAL_GATE_DISPATCH') + 10 + struct.unpack_from('<i', dispatch, 6)[0] == rva('SIGNAL_GATE_TABLE')
    table = code('SIGNAL_GATE_TABLE')
    values = numbers(declaration(signals, 'SIGNAL_GATE_RULES'))
    above = re.findall(r'\b(true|false)\b', declaration(signals, 'SIGNAL_GATE_RULES'))
    assert len(values) == 5 and len(above) == 5
    masks = code('SIGNAL_GATE_MASKS')
    for slot in range(5):
        case = rva('SIGNAL_GATE_TABLE') + struct.unpack_from('<i', table, 4 * slot)[0]
        if slot == 1:
            assert case == rva('SIGNAL_GATE_SLOT1')
            # Rejects 1 <= mode <= 7 unless mode == 6: accepts 0, 6, 8 and up.
            assert code('SIGNAL_GATE_SLOT1').startswith(bytes.fromhex('8d41f83cf90f93c080f906'))
            assert (values[slot], above[slot]) == (0x141, 'true')
            continue
        at = case - rva('SIGNAL_GATE_MASKS')
        assert 0 <= at < len(masks), slot
        assert masks[at:at + 3] in (bytes.fromhex('80f908'), bytes.fromhex('83f808')), slot
        assert masks[at + 3:at + 5] == bytes.fromhex('0f87'), slot
        assert masks[at + 9:at + 14] == b'\xb9' + struct.pack('<I', values[slot]), slot
        assert above[slot] == 'false', slot


def verify_sources(profile, root=ROOT):
    adapter = adapter_source(profile, root)
    folder = tables_folder(profile, root)
    identity = (folder/'identity.rs').read_text(encoding='utf-8')
    items = (folder/'items.rs').read_text(encoding='utf-8')
    preview = (folder/'effects.rs').read_text(encoding='utf-8')
    # The hook code may not name an address itself: a copy built for another
    # game version would use it unchecked.
    for path in sorted((root/'probe/src/native_adapter/windows').glob('*.rs')):
        if path.name == 'tests.rs':
            continue
        code = '\n'.join(line.split('//')[0] for line in path.read_text(encoding='utf-8').splitlines())
        found = re.findall(r'\b0x[0-9a-f]{6,7}\b', code)
        assert not found, f'{path.name} names addresses outside the tables: {found}'
    assert json.loads(declaration(identity, 'GAME')) == profile['game_version']
    if 'minimap' in profile:
        assert f'include_str!("../../../../tools/native_profiles/{profile["game_version"]}.json")' in identity
        assert 'crate::minimap::native::plan(MINIMAP_PROFILE, base, constants_address)' in adapter
        assert 'minimap_plan.verify()' in adapter
        assert 'crate::minimap::content_rect(wide, left, custom)' in adapter
    anchors = profile['anchors']
    assert json.loads(declaration(identity, 'EXPECTED_SHA')) == profile['executable_sha256']
    assert numbers(declaration(identity, 'PE_TIMESTAMP')) == [int(profile['timestamp'], 16)]
    assert numbers(declaration(identity, 'IMAGE_SIZE')) == [int(profile['size_of_image'], 16)]
    assert 'GUARDS.verify(base, families)' in adapter
    byte_names = {
        'WORKER_BYTES':('WORKER_SITE','WORKER_BYTES'), 'VIEW_BYTES':('VIEW_SITE','VIEW_BYTES'),
        'MOVE_BYTES':('MOVE_SITE','MOVE_BYTES'), 'INPUT_BYTES':('INPUT_SITE','INPUT_BYTES'),
        'ATTACK_BYTES':('ATTACK_SITE','ATTACK_BYTES'),
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
        'SIGNAL_STEP_CALL':('SIGNAL_SITE','SIGNAL_BYTES'),
        'SIGNAL_BROADCAST_PROLOGUE':('SIGNAL_ORIGINAL','SIGNAL_PROLOGUE'),
    }
    # A profile may review only some hook families (its "families" list);
    # constants of the others are not compared.
    for name, (site, value) in byte_names.items():
        if name not in anchors:
            continue
        assert numbers(declaration(adapter, site)) == [int(anchors[name]['rva'], 16)], name
        assert bytes(numbers(declaration(adapter, value))).hex() == anchors[name]['bytes'], name
    verify_signal_fields(anchors, folder)
    # Further worker sends (0.6.5: a drain loop): the table's list and the
    # profile's WORKER_EXTRA_BYTES_<n> anchors name the same calls.
    extra = numbers(declaration(adapter, 'WORKER_EXTRA_SITES'))
    expected = []
    for name in sorted(n for n in anchors if n.startswith('WORKER_EXTRA_BYTES_')):
        a = anchors[name]
        assert int(a['target'], 16) == int(anchors['WORKER_PROLOGUE']['rva'], 16), name
        expected += [int(a['rva'], 16), *bytes.fromhex(a['bytes'])]
    assert extra == expected, 'WORKER_EXTRA_SITES'
    # In-place rewrites (0.6.5: the screen delay): each (rva, the game's 7
    # bytes, the mod's 7 bytes) matches a VIEW_DELAY_BYTES_<n> anchor.
    delay = numbers(declaration(adapter, 'VIEW_DELAY_SITES'))
    assert len(delay) % 15 == 0, 'VIEW_DELAY_SITES'
    rows = [delay[i:i + 15] for i in range(0, len(delay), 15)]
    names = sorted(n for n in anchors if n.startswith('VIEW_DELAY_BYTES_'))
    assert [r[:8] for r in rows] == [[int(anchors[n]['rva'], 16), *bytes.fromhex(anchors[n]['bytes'])] for n in names], 'VIEW_DELAY_SITES'
    for idx, slot in enumerate('QWR'):
        if f'SKILL_{slot}_CALL' not in anchors:
            continue
        a = anchors[f'SKILL_{slot}_CALL']
        assert numbers(declaration(adapter, 'SKILL_SITES'))[idx] == int(a['rva'], 16)
        assert numbers(declaration(adapter, 'SKILL_ORIGINALS'))[idx] == int(a['target'], 16)
        assert bytes(numbers(declaration(adapter, 'SKILL_BYTES'))[idx*5:(idx+1)*5]).hex() == a['bytes']
        assert bytes(numbers(declaration(adapter, 'SKILL_PROLOGUE'))).hex() == anchors[f'SKILL_{slot}_PROLOGUE']['bytes']
    pairs = {}
    for source, name in [(adapter,'AIM_WRITES'),(adapter,'SKILL_QUEUE_ANCHORS'),(items,'ITEM_ANCHORS'),(identity,'LAYOUT_BYTES')]:
        pairs.update(byte_pairs(declaration(source, name)))
    for name, a in (anchors | profile['layout_anchors']).items():
        if name.startswith(('ITEM_', 'ENTITY_', 'VISION_', 'VIEW_LAYOUT_')) or name in ('AIM_POINT_WRITE','AIM_DIRECTION_WRITE') or name.endswith('QUEUE_LAYOUT'):
            assert pairs[int(a['rva'],16)].hex() == a['bytes'], name
    pointers = numbers(declaration(identity, 'LAYOUT_POINTERS'))
    for a in profile['layout_anchors'].values():
        if 'pointer_target' in a:
            at = pointers.index(int(a['rva'],16))
            assert pointers[at+1] == int(a['pointer_target'],16)
    if 'tooltip_anchors' in profile:
        tooltip_bytes = byte_pairs(declaration(adapter, 'TOOLTIP_BYTES'))
        tooltip_pointers = numbers(declaration(adapter, 'TOOLTIP_POINTERS'))
        for name, a in profile['tooltip_anchors'].items():
            rva = int(a['rva'], 16)
            if 'bytes' in a:
                assert tooltip_bytes[rva].hex() == a['bytes'], name
            else:
                at = tooltip_pointers.index(rva)
                assert tooltip_pointers[at+1] == int(a['pointer_target'],16), name
        for constant, anchor in [('TOOLTIP_SHEET_GETTER','TOOLTIP_SHEET_GETTER_ENTRY'),
                                 ('TOOLTIP_INFO_LOOKUP','TOOLTIP_INFO_LOOKUP_ENTRY'),
                                 ('TOOLTIP_ARC_DROP','TOOLTIP_ARC_DROP_ENTRY')]:
            assert numbers(declaration(adapter,constant)) == [int(profile['tooltip_anchors'][anchor]['rva'],16)]
        assert numbers(declaration(adapter,'TOOLTIP_DESCRIPTION_SLOTS')) == [0xe0,0xe8,0xf0]
        # Item effect text (1.5.0-dev29): the panel's getter, list builder and
        # list drop, the key/text slots, and every reviewed item table with
        # its key and text methods guarded.
        tips = profile['tooltip_anchors']
        if 'TOOLTIP_ITEM_SHEET_GETTER_ENTRY' in tips:
            for constant, anchor in [('TOOLTIP_ITEM_SHEET_GETTER','TOOLTIP_ITEM_SHEET_GETTER_ENTRY'),
                                     ('TOOLTIP_ITEM_LIST','TOOLTIP_ITEM_LIST_ENTRY'),
                                     ('TOOLTIP_ITEM_LIST_DROP','TOOLTIP_ITEM_LIST_DROP_ENTRY')]:
                assert numbers(declaration(adapter,constant)) == [int(tips[anchor]['rva'],16)], constant
            key_slot = numbers(declaration(adapter,'TOOLTIP_ITEM_KEY_SLOT'))
            text_slot = numbers(declaration(adapter,'TOOLTIP_ITEM_TEXT_SLOT'))
            assert key_slot == [0x58] and text_slot == [0x90]
            assert tips['TOOLTIP_ITEM_TEXT_CALL']['bytes'].endswith('90000000'), 'item text slot'
            tables = numbers(declaration(adapter,'TOOLTIP_ITEM_TABLES'))
            assert tables and len(set(tables)) == len(tables)
            for table in tables:
                for slot, what in ((0x58,'KEY'),(0x90,'TEXT')):
                    anchor = tips[f'TOOLTIP_ITEM_{table:X}_{what}_POINTER']
                    assert int(anchor['rva'],16) == table+slot, f'item table {table:#x}'
        layouts = numbers(declaration(adapter,'TOOLTIP_TABLE_LAYOUTS'))
        for n in range(0,len(layouts),3):
            table,size,alignment = layouts[n:n+3]
            assert tooltip_bytes[table+8] == struct.pack('<QQ',size,alignment)
    for name, anchor in [('COMBINE','PREVIEW_COMBINE_APPLY'),('RANGE','PREVIEW_RANGE_APPLY'),
                         ('LINEAR','PREVIEW_LINEAR_APPLY'),('WHIP_LINE','PREVIEW_CHANNEL_LINE_APPLY')]:
        if anchor not in anchors:
            continue
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
    for name, a in (profile['anchors'] | profile['layout_anchors'] | profile.get('tooltip_anchors', {})).items():
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
    if 'STATS_SITE_0' in profile['anchors']:
        import verify_stat_sites
        verify_stat_sites.verify(profile, image, tables_folder(profile))
    if check_sources:
        verify_sources(profile)
    return profile


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--profile', type=Path, default=ROOT/'tools/native_profiles/0.6.5.json')
    parser.add_argument('--executable', type=Path, default=DEFAULT_EXE)
    args = parser.parse_args()
    p = verify(args.profile, args.executable)
    print(f"Verified game {p['game_version']} (families: {', '.join(p.get('families', ['all']))}): "
          f"exact identity, {len(p['anchors'])} anchors, "
          f"{len(p['layout_anchors'])} layout guards, {len(p.get('tooltip_anchors', {}))} tooltip guards, "
          "call targets, pointers and Rust constants.")
