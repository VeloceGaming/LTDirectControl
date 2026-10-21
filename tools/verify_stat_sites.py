"""Check the athlete stat rule table of one game version against its executable.

The table (probe/src/native_adapter/v<version>/stat_sites.rs) names, for every
site, the registers, frame slots and offsets the hook reads and writes. This
decodes each site in the executable and refuses a table that disagrees with
the instructions: an `r14` that was `rdi` in another version, a frame slot that
moved, an operand that is not what the rule assumes.

`site_fields` works those fields out from the instructions, so
draft_version.py can write them for a new game version, and `census` finds
every instruction of the executable that has the shape of a stat site, so a
draft can say which sites a patch added or removed.

Used by verify_native_profile.py and draft_version.py; it never updates or
installs anything.
"""
import re

REGISTERS = {'rax': 'Rax', 'rbx': 'Rbx', 'rcx': 'Rcx', 'rdx': 'Rdx', 'rsi': 'Rsi', 'rdi': 'Rdi', 'rbp': 'Rbp',
             'r8': 'R8', 'r9': 'R9', 'r10': 'R10', 'r11': 'R11', 'r12': 'R12', 'r13': 'R13', 'r14': 'R14',
             'r15': 'R15'}


# Every name of each 64-bit register's parts, so a write to `eax` counts as
# a write to `rax`.
FAMILY = {}
for _wide, _parts in {'rax': 'eax ax al ah', 'rbx': 'ebx bx bl bh', 'rcx': 'ecx cx cl ch', 'rdx': 'edx dx dl dh',
                      'rsi': 'esi si sil', 'rdi': 'edi di dil', 'rbp': 'ebp bp bpl', 'rsp': 'esp sp spl'}.items():
    for _name in (_wide, *_parts.split()):
        FAMILY[_name] = _wide
for _n in range(8, 16):
    for _suffix in ('', 'd', 'w', 'b'):
        FAMILY[f'r{_n}{_suffix}'] = f'r{_n}'


def wide(name):
    return FAMILY.get(name.strip(), name.strip())


def numbers(value):
    return [int(x, 0) for x in re.findall(r'\b(?:0x[0-9a-f]+|[0-9]+)\b', value)]


def squash(text):
    """The table as rustfmt may lay it out: one line, no trailing commas."""
    text = re.sub(r'\s+', ' ', text)
    text = re.sub(r',\s*(?=[})\]])', '', text)
    text = re.sub(r'\{\s*', '{ ', text)
    return re.sub(r'\s*\}', ' }', text)


def parse_table(folder):
    """The sites of STAT_SITES as (rva, bytes, kind, text of the kind); rva
    is None for a draft's `UNKNOWN_0x...`."""
    source = (folder / 'stat_sites.rs').read_text(encoding='utf-8')
    table = source.split('pub(crate) const STAT_SITES')[1].split('];')[0]
    sites = []
    for block in re.split(r'\n    Site \{', table)[1:]:
        rva = re.search(r'rva:\s*(0x[0-9a-f]+)', block)
        rva = int(rva[1], 16) if rva else None
        data = bytes(numbers(re.search(r'bytes:\s*&\[(.*?)\]', block, re.S)[1]))
        kind = re.search(r'Kind::(\w+)', block)[1]
        sites.append((rva, data, kind, squash(block.split('kind:')[1])))
    return sites


def constants_of(source):
    return {m[1]: int(m[2], 16) for m in re.finditer(r'const (STAT_\w+): usize = (0x[0-9a-f]+);', source)}


def reg(name):
    return 'R::' + REGISTERS[name]


class Code:
    """Decoding helpers over one executable (patch_migration.Image)."""

    def __init__(self, image):
        from capstone.x86 import X86_OP_IMM, X86_OP_MEM, X86_OP_REG
        self.image, self.IMM, self.MEM, self.REG = image, X86_OP_IMM, X86_OP_MEM, X86_OP_REG

    def around(self, rva):
        bounds = self.image.enclosing(rva)
        assert bounds, f'{rva:#x}: not inside a known function'
        instructions = self.image.decoded(bounds)
        at = next((n for n, i in enumerate(instructions) if i.address - self.image.base == rva), None)
        assert at is not None, f'{rva:#x}: not at an instruction boundary'
        return instructions, at

    def memory(self, i):
        operands = [o for o in i.operands if o.type == self.MEM]
        return operands[0] if operands else None

    def base_of(self, i):
        return self.image.cs.reg_name(self.memory(i).mem.base)

    def writes(self, i):
        """The 64-bit registers the instruction writes, implicit ones included."""
        return {wide(self.image.cs.reg_name(r)) for r in i.regs_access()[1]}

    def target(self, i):
        """The RVA a direct jump or call goes to, else None."""
        if (i.mnemonic.startswith('j') or i.mnemonic == 'call') and i.operands and i.operands[0].type == self.IMM:
            return i.operands[0].imm - self.image.base
        return None


def leading_to(code, ins, end):
    """Indices of the instructions from which `ins[end]` is reached by
    fall-through and direct jumps (code entered only through a jump table
    has no known way in and ends the walk)."""
    previous = {}
    for n, i in enumerate(ins):
        t = code.target(i) if i.mnemonic != 'call' else None
        if t is not None:
            previous.setdefault(t, []).append(n)
        if n + 1 < len(ins) and i.mnemonic not in ('jmp', 'ret', 'int3', 'ud2'):
            previous.setdefault(ins[n + 1].address - code.image.base, []).append(n)
    reach, todo = set(), [end]
    while todo:
        n = todo.pop()
        if n in reach:
            continue
        reach.add(n)
        todo += previous.get(ins[n].address - code.image.base, [])
    return reach


def on_paths(code, ins, start, end):
    """Indices of the instructions on some path from `ins[start]` to
    `ins[end]`, both included, following fall-through and direct jumps. An
    indirect jump that could lead there makes the answer unknown: refused."""
    index = {i.address - code.image.base: n for n, i in enumerate(ins)}

    def successors(n):
        i = ins[n]
        if i.mnemonic in ('ret', 'int3', 'ud2'):
            return []
        t = code.target(i) if i.mnemonic != 'call' else None
        if i.mnemonic == 'jmp':
            return None if t is None else [index[t]] if t in index else []
        return ([n + 1] if n + 1 < len(ins) else []) + ([index[t]] if t in index else [])
    forward, todo, unknown = set(), [start], []
    while todo:
        n = todo.pop()
        if n in forward:
            continue
        forward.add(n)
        if n == end:
            continue
        following = successors(n)
        if following is None:
            unknown.append(n)
            continue
        todo += following
    assert end in forward, f'{ins[end].address - code.image.base:#x} not reached'
    previous = {}
    for n in forward - {end}:
        for m in successors(n) or []:
            previous.setdefault(m, []).append(n)
    reach, todo = set(), [end]
    while todo:
        n = todo.pop()
        if n in reach:
            continue
        reach.add(n)
        todo += previous.get(n, [])
    # Where an indirect jump goes is not known, so it might lead here.
    assert not unknown, f'indirect jump at {ins[unknown[0]].address - code.image.base:#x} on the way'
    return forward & reach


def span_of(code, rva, length, entry_ok=False):
    """The whole instructions the site replaces, after the relay's checks.
    `entry_ok`: a branch may land on the first of them (it then runs the
    hook), for kinds whose hook needs nothing the previous instruction did."""
    ins, at = code.around(rva)
    span, size = [], 0
    while size < length:
        span.append(ins[at + len(span)])
        size += span[-1].size
    assert size == length >= 5, f'{rva:#x}: not whole instructions of at least 5 bytes'
    for i in span:
        assert i.mnemonic not in ('call', 'jmp') and 'rip' not in i.op_str and 'rsp' not in i.op_str, \
            f'{rva:#x}: {i.mnemonic} {i.op_str} cannot be replayed'
    # No branch of the function lands on a replaced instruction: inside
    # the span it would land mid-patch; on the first, code reaching it by a
    # jump would skip the instruction before it, which most rules assume ran.
    replaced = {i.address for i in (span[1:] if entry_ok else span)}
    for i in ins:
        if (i.mnemonic.startswith('j') or i.mnemonic == 'call') and i.operands \
                and i.operands[0].type == code.IMM:
            assert i.operands[0].imm not in replaced, f'{rva:#x}: branch from {i.address - code.image.base:#x}'
    return ins, at, span


def site_fields(code, sites, index, constants):
    """[(field, Rust text)] of the site's kind as its instructions say they
    must be. Raises AssertionError when the instructions are not the kind's."""
    rva, data, kind, text = sites[index]
    ins, at, span = span_of(code, rva, len(data), entry_ok=kind in ('AccuracyProducer', 'DodgingProducer'))
    first, memory, base_of = span[0], code.memory, code.base_of
    where = f'{rva:#x}'
    if kind == 'Collision':
        assert first.mnemonic == 'sub' and memory(first) and memory(first).mem.disp == 0x6a0, where
        out = first.op_str.split(',')[0].strip()
        previous = ins[at - 1]
        assert previous.mnemonic == 'add' and previous.op_str.startswith(out + ',') and memory(previous), where
        source = (code.image.cs.reg_name(memory(previous).mem.base), memory(previous).mem.disp)
        assert source[0] != out, where
        assert ins[at + 1].mnemonic == 'test', where
        return [('output', reg(out)), ('entity', reg(base_of(first))),
                ('accuracy', f'({reg(source[0])}, {source[1]:#x})')]
    if kind == 'FlooredCollision':
        # cmp output, 5001 right after sub output, [entity + Dodging];
        # then the floor: a cmovl of the output to 5000, or a cmovge of
        # the output over a register holding 5000.
        out = first.op_str.split(',')[0].strip()
        assert first.mnemonic == 'cmp' and first.op_str == f'{out}, 0x1389' and len(span) == 1, where
        previous = ins[at - 1]
        assert previous.mnemonic == 'sub' and previous.op_str.startswith(out + ',') \
            and memory(previous) and memory(previous).mem.disp == 0x6a0, where
        after = ins[at + 1:at + 3]
        floor = next(i for i in after if i.mnemonic.startswith('cmov'))
        target, source = [o.strip() for o in floor.op_str.split(',')]
        assert (floor.mnemonic == 'cmovl' and target == out) or \
            (floor.mnemonic == 'cmovge' and source == out), where
        assert all(i is floor or i.mnemonic == 'mov' and i.op_str.endswith(', 0x1388')
                   for i in after[:after.index(floor) + 1]), where
        return [('output', reg(out)), ('entity', reg(base_of(previous)))]
    if kind == 'AccuracyProducer':
        assert first.mnemonic == 'imul' and first.op_str.endswith('0x46') and len(span) == 2, where
        store = span[1]
        assert store.mnemonic == 'mov' and memory(store) and base_of(store) == 'rbp', where
        # The caster: the id loaded into rdx for the player lookup.
        back = [f'{i.mnemonic} {i.op_str}' for i in ins[max(at - 60, 0):at]]
        found = [m for m in (re.match(r'mov rdx, qword ptr \[(\w+) \+ (0x5b8|0xf8)\]$', t) for t in back) if m]
        assert found, f'{where}: no caster id loaded for the player lookup'
        found = found[-1]
        projectile = found[2] == '0xf8'
        name = 'STAT_PROJECTILE_CASTER' if projectile else 'STAT_ENTITY_ID'
        assert constants[name] == int(found[2], 16), where
        # The traced word: the game reads it from the caster's object (or
        # a register copied from it) close to the producer.
        trace = 'STAT_TRACE_PROJECTILE_KIND' if projectile else 'STAT_TRACE_ACTION_STATE'
        near = [f'{i.mnemonic} {i.op_str}' for i in ins[max(at - 200, 0):at + 12]]
        # Registers holding the caster's object: copies of copies, also
        # through a frame slot (0.6.3: rsi -> [rbp + 0x258] -> r14).
        holders, slots = {found[1]}, set()
        for _ in range(3):
            for t in near:
                pattern = "|".join(holders)
                if m := re.match(rf'mov (\w+), ({pattern})$', t):
                    holders.add(m[1])
                if m := re.match(rf'mov qword ptr (\[rbp \+ 0x[0-9a-f]+\]), ({pattern})$', t):
                    slots.add(m[1])
                if (m := re.match(r'mov (\w+), qword ptr (\[rbp \+ 0x[0-9a-f]+\])$', t)) and m[2] in slots:
                    holders.add(m[1])
        assert any(re.match(rf'mov \w+, qword ptr \[({"|".join(holders)}) \+ {constants[trace]:#x}\]$', t)
                   for t in near), f'{where}: the traced word is not read near the producer'
        return [('frame', f'{memory(store).mem.disp:#x}'),
                ('caster', f'Caster::Field {{ reg: {reg(found[1])}, offset: {name} }}'), ('trace', trace)]
    if kind == 'DodgingProducer':
        # Dodging x 150 (0.6.4) or x 200 (0.6.5): the table's factor, which
        # the hook uses to write a neutral athlete's value.
        assert first.mnemonic == 'imul' and first.op_str.endswith(('0x96', '0xc8')), where
        assert int(first.op_str.split(',')[-1], 16) == constants['STAT_DODGING_FACTOR'],             f'{where}: the code multiplies by {first.op_str.split(",")[-1].strip()}'
        store = ins[at + 1]
        assert store.mnemonic == 'mov' and memory(store) and memory(store).mem.disp == 0x6a0, where
        out = first.op_str.split(',')[0].strip()
        assert store.op_str.endswith(', ' + out), where
        return [('output', reg(out)), ('entity', reg(base_of(store)))]
    if kind == 'Capture':
        assert first.mnemonic == 'movabs' and first.op_str.endswith('0x9e3779b97f4a7c15'), where
        end = next((r for r, _, k, t in sites[index + 1:] if k == 'Complete' and f'capture: {index}' in t), None)
        assert end is not None, f'{where}: no Complete names this capture'
        forward = ins[at:code.around(end)[1] + 1]
        squared = next(i for i in forward if i.mnemonic == 'imul' and len(i.operands) == 2
                       and i.operands[0].type == i.operands[1].type == code.REG
                       and i.operands[0].reg == i.operands[1].reg)
        accuracy = {'ecx': 'rcx', 'r8d': 'r8'}[squared.op_str.split(',')[0].strip()]
        n = next(n for n, i in enumerate(forward) if i.mnemonic == 'shr' and i.op_str.endswith('0x2d'))
        q = forward[n + 1]
        assert q.mnemonic == 'imul', where
        if memory(q):
            assert base_of(q) == 'rbp' and memory(q).size == 4, where
            inverse = f'Source::Frame({memory(q).mem.disp:#x})'
        else:
            assert q.op_str.startswith('r8d'), where
            inverse = 'Source::Register(R::R8)'
        back = [f'{i.mnemonic} {i.op_str}' for i in ins[max(at - 45, 0):at]]
        expected = None
        for t, following in zip(back, back[1:] + ['']):
            m = re.match(r'mov rax, qword ptr \[(\w+) \+ 0xf0\]$', t)
            if m and following == 'shl rax, 0x14':
                expected = f'Caster::Field {{ reg: {reg(m[1])}, offset: STAT_PROJECTILE_CASTER }}'
            m = re.match(r'shl (\w+), 0x14$', t)
            if m and m[1] != 'rax':
                seed = constants['STAT_PLAYER_SEED']
                mixed = re.match(rf'xor {m[1]}, qword ptr \[(\w+) \+ {seed:#x}\]$', following)
                expected = (f'Caster::Mixed {{ reg: {reg(m[1])}, other: {reg(mixed[1])}, '
                            f'offset: STAT_PLAYER_SEED, shift: 20 }}' if mixed
                            else f'Caster::Shifted {{ reg: {reg(m[1])}, shift: 20 }}')
        assert expected, f'{where}: no caster found before the gate'
        # The id the hook reads must be the one the game looked the
        # caster's Accuracy up with: the `rdx` of the call whose result's
        # +0x1f0 (Accuracy) is read before the capture.
        read = max(n for n, t in enumerate(back) if t.endswith('+ 0x1f0]'))
        call = max(n for n in range(read) if back[n].startswith('call '))
        lookup = next(back[n] for n in range(call - 1, -1, -1) if back[n].startswith('mov rdx, '))
        if 'Caster::Field' in expected:
            field = re.match(r'mov rdx, qword ptr \[(\w+) \+ (0x[0-9a-f]+)\]$', lookup)
            assert field and f'reg: {reg(field[1])}' in expected, where
            assert int(field[2], 16) == constants['STAT_PROJECTILE_CASTER'], where
        else:
            caster = re.search(r'reg: R::(\w+)', expected)[1]
            assert lookup == f'mov rdx, {caster.lower()}', where
        age = 'true' if any(t.startswith('shl rax, 0x28') for t in back) else 'false'
        return [('accuracy', reg(accuracy)), ('inverse_dodge', inverse), ('caster', expected), ('age_mixed', age)]
    if kind == 'Complete':
        assert first.mnemonic == 'shr' and first.op_str.endswith('0x11') and len(span) == 2, where
        out = first.op_str.split(',')[0].strip()
        wide = 'r' + out[1:]
        assert span[1].mnemonic == 'add' and span[1].op_str == f'{wide}, rax', where
        capture = int(re.search(r'capture:\s*(\d+)', text)[1])
        assert sites[capture][2] == 'Capture', where
        return [('capture', str(capture)), ('output', reg(wide))]
    if kind == 'Inputs':
        return inputs_fields(code, sites, index, constants, ins, at, span)
    if kind == 'Band':
        # The load of the hash constant. The game then hashes SEED, scales
        # it by RANGE + 1 (skipped when that wraps), adds LOW and EXTRA:
        #   sub RANGE, LOW; movabs rdx, K; add SEED, rdx; <hash>;
        #   inc RANGE; je end; mul H; add LOW, rdx; mov OUT, LOW;
        #   end: add OUT, EXTRA
        assert first.op_str == 'rdx, 0x9e3779b97f4a7c15' and len(span) == 1, where
        before = ins[at - 1]
        assert before.mnemonic == 'sub', where
        width, low = [x.strip() for x in before.op_str.split(',')]
        assert ins[at + 1].mnemonic == 'add' and ins[at + 1].op_str.endswith(', rdx'), where
        seed = ins[at + 1].op_str.split(',')[0].strip()
        tail = ins[at + 1:at + 40]
        n = next(n for n, i in enumerate(tail) if i.mnemonic == 'inc')
        skip, mul, add, move, end = tail[n + 1:n + 6]
        assert tail[n].op_str == width and skip.mnemonic == 'je' and mul.mnemonic == 'mul', where
        assert add.mnemonic == 'add' and add.op_str == f'{low}, rdx', where
        assert move.mnemonic == 'mov' and move.op_str.endswith(f', {low}'), where
        out = move.op_str.split(',')[0].strip()
        assert end.mnemonic == 'add' and end.op_str.startswith(out + ','), where
        assert code.target(skip) == end.address - code.image.base, where
        extra = end.op_str.split(',')[1].strip()
        assert len({seed, low, width, extra, 'rdx'}) == 5, where
        # The hash itself leaves the band's registers alone.
        for i in tail[:n]:
            assert not code.writes(i) & {low, width, extra}, f'{where}: {i.mnemonic} {i.op_str} in the hash'
        inputs = int(re.search(r'inputs:\s*(\d+)', text)[1])
        assert sites[inputs][2] == 'Inputs', where
        return [('inputs', str(inputs)), ('seed', reg(seed)), ('low', reg(low)), ('range', reg(width)),
                ('extra', reg(extra))]
    raise AssertionError(f'Unknown stat rule kind {kind}')


def inputs_fields(code, sites, index, constants, ins, at, span):
    """The fields of an `Inputs` site: where the gate holds the caster's
    Accuracy, the defender's Dodging and the caster's id when the hook runs
    (after the replayed instructions)."""
    rva = sites[index][0]
    where = f'{rva:#x}'
    base = code.image.base
    after = at + len(span)
    band = next((s for s in sites if s[2] == 'Band' and re.search(rf'inputs: {index}\b', s[3])), None)
    assert band, f'{where}: no Band names these inputs'
    stop = code.around(band[0])[1]
    assert stop > at, where
    # From here to the band: no call, and no branch in or out, so the band
    # always follows these inputs on the same thread.
    for n, i in enumerate(ins):
        t = code.target(i)
        if at <= n < stop:
            assert i.mnemonic != 'call', f'{where}: call at {i.address - base:#x} before the band'
            assert i.mnemonic not in ('ret', 'int3') and not (i.mnemonic == 'jmp' and t is None), where
            if t is not None:
                assert rva < t <= band[0], f'{where}: branch from {i.address - base:#x} leaves the gate'
        elif t is not None:
            assert not rva < t <= band[0], f'{where}: branch from {i.address - base:#x} into the gate'
    gate = ins[at:stop]

    def untouched(register, upto):
        """`register` keeps its value from the hook to ins[upto]."""
        for i in ins[at:max(upto, after)]:
            assert register not in code.writes(i), f'{where}: {i.mnemonic} {i.op_str} changes {register}'

    # Accuracy: the register the gate squares.
    n = next(at + k for k, i in enumerate(gate) if i.mnemonic == 'imul' and len(i.operands) == 2
             and i.operands[0].type == i.operands[1].type == code.REG and i.operands[0].reg == i.operands[1].reg)
    square = ins[n].op_str.split(',')[0].strip()
    copy = ins[n - 1]
    assert copy.mnemonic == 'mov' and copy.op_str.startswith(square + ','), where
    accuracy = wide(copy.op_str.split(',')[1])
    untouched(accuracy, n - 1)
    # Dodging: `100 - Dodging` formed in the gate from a register, or the
    # inverse read from the frame where the gate multiplies with it.
    dodging = None
    for k, i in enumerate(gate[:-1]):
        nxt = gate[k + 1]
        if i.mnemonic == 'mov' and i.op_str.endswith(', 0x64') and nxt.mnemonic == 'sub' \
                and wide(nxt.op_str.split(',')[0]) == wide(i.op_str.split(',')[0]) and not code.memory(nxt):
            value = wide(nxt.op_str.split(',')[1])
            untouched(value, at + k + 1)
            dodging = f'Dodging::Value(Source::Register({reg(value)}))'
            break
    if dodging is None:
        k = next(k for k, i in enumerate(gate) if i.mnemonic == 'shr' and i.op_str.endswith('0x2d'))
        load, use = gate[k + 1], gate[k + 2]
        if load.mnemonic == 'mov':
            assert code.memory(load) and code.base_of(load) == 'rbp', where
            assert use.mnemonic == 'imul' and wide(use.op_str.split(',')[1]) == wide(load.op_str.split(',')[0]), where
        else:
            assert load.mnemonic == 'imul' and code.memory(load) and code.base_of(load) == 'rbp', where
        slot = code.memory(load).mem.disp
        untouched('rbp', at + k + 1)
        for i in ins[at:at + k + 1]:
            stored = code.memory(i) and code.base_of(i) == 'rbp' and code.memory(i).mem.disp == slot
            assert not (stored and i.op_str.startswith(('qword ptr', 'dword ptr'))), f'{where}: the frame slot changes'
        dodging = f'Dodging::Inverse(Source::Frame({slot:#x}))'
    # The caster: the id the game looked the caster's Accuracy up with (the
    # `rdx` of the call whose result's +0x1f0 is read), followed along every
    # path from that lookup to the hook.
    read = max(n for n in leading_to(code, ins, at) if n < at and ins[n].op_str.endswith('+ 0x1f0]'))
    call = max(n for n in range(read) if ins[n].mnemonic == 'call')
    lookup = next(n for n in range(call - 1, -1, -1)
                  if ins[n].mnemonic == 'mov' and ins[n].op_str.startswith('rdx, '))
    path = on_paths(code, ins, lookup + 1, after - 1)
    line = f'{ins[lookup].mnemonic} {ins[lookup].op_str}'
    if m := re.match(r'mov rdx, qword ptr \[(\w+) \+ (0x[0-9a-f]+)\]$', line):
        names = {constants['STAT_PROJECTILE_CASTER']: 'STAT_PROJECTILE_CASTER',
                 constants['STAT_ENTITY_ID']: 'STAT_ENTITY_ID'}
        assert int(m[2], 16) in names, where
        assert not any(m[1] in code.writes(ins[n]) for n in path), f'{where}: the caster object changes'
        caster = f'Caster::Field {{ reg: {reg(m[1])}, offset: {names[int(m[2], 16)]} }}'
    else:
        m = re.match(r'mov rdx, (\w+)$', line)
        assert m, f'{where}: {line}'
        held = m[1]
        steps = [f'{ins[n].mnemonic} {ins[n].op_str}' for n in sorted(path) if held in code.writes(ins[n])]
        seed = constants['STAT_PLAYER_SEED']
        if not steps:
            caster = f'Caster::Shifted {{ reg: {reg(held)}, shift: 0 }}'
        elif steps == [f'shl {held}, 0x14']:
            caster = f'Caster::Shifted {{ reg: {reg(held)}, shift: 20 }}'
        else:
            assert len(steps) == 2 and steps[0] == f'shl {held}, 0x14', f'{where}: {steps}'
            mixed = re.match(rf'xor {held}, qword ptr \[(\w+) \+ {seed:#x}\]$', steps[1])
            assert mixed, f'{where}: {steps[1]}'
            assert not any(mixed[1] in code.writes(ins[n]) for n in path), where
            caster = (f'Caster::Mixed {{ reg: {reg(held)}, other: {reg(mixed[1])}, offset: STAT_PLAYER_SEED,'
                      f' shift: 20 }}')
    return [('accuracy', reg(accuracy)), ('dodging', dodging), ('caster', caster)]


def check(profile, image, folder):
    """Every disagreement between the table and the executable, as
    (rva or None, reason); empty when the table is right."""
    sites = parse_table(folder)
    constants = constants_of((folder / 'stat_sites.rs').read_text(encoding='utf-8'))
    anchors = {int(a['rva'], 16): a for n, a in profile['anchors'].items()
               if n.startswith('STATS_SITE_') and a.get('rva')}
    errors = []
    if len(sites) != len(anchors):
        errors.append((None, f'{len(sites)} sites in the table, {len(anchors)} in the profile'))
    code = Code(image)
    for index, (rva, data, kind, text) in enumerate(sites):
        if rva is None:
            errors.append((None, f'site {index} ({kind}) has no address'))
            continue
        if rva not in anchors or anchors[rva]['bytes'] != data.hex():
            errors.append((rva, 'address or bytes differ from the profile'))
            continue
        try:
            for name, value in site_fields(code, sites, index, constants):
                if f'{name}: {value}' not in text:
                    errors.append((rva, f'the code says {name}: {value}'))
        except AssertionError as e:
            errors.append((rva, str(e) or f'{rva:#x}: not a {kind}'))
        except (StopIteration, ValueError, KeyError, IndexError, TypeError) as e:
            errors.append((rva, f'{rva:#x}: not a {kind} ({type(e).__name__})'))
    return errors


def verify(profile, image, folder):
    errors = check(profile, image, folder)
    assert not errors, '; '.join(reason if rva is None else f'{rva:#x}: {reason}' for rva, reason in errors)
    return len(parse_table(folder))


def census(image):
    """{kind: [rva]}: every instruction of the executable shaped like a stat
    site. `Capture` and `Band` list the gate-shaped uses of the gates' hash
    constant (the constant itself is a common hash; a gate also squares a
    value and takes `>> 0x2d` before it), candidates only: not every one
    reads Accuracy. `Capture` gates end in `shr 0x11; add r, rax` (0.6.4);
    `Band` gates roll in a band, `inc; je; mul` after the hash (0.6.5). The
    `Inputs` site of a band gate is found by hand. `Unclassified` lists
    Dodging subtractions that are neither consumer shape."""
    code = Code(image)
    text, start = image.text, image.text_rva
    patterns = [rb'\xa0\x06\x00\x00',                          # [reg + 0x6a0]
                rb'[\x48-\x4f]\x6b[\xc0-\xff]\x46',             # imul r64, r64, 0x46
                rb'[\x48-\x4f]\x69[\xc0-\xff][\x96\xc8]\x00\x00\x00']  # imul r64, r64, 0x96 or 0xc8
    functions = set()
    for pattern in patterns:
        for m in re.finditer(pattern, text):
            bounds = image.enclosing(start + m.start())
            if bounds:
                functions.add(bounds)
    found = {k: [] for k in ('Collision', 'FlooredCollision', 'AccuracyProducer', 'DodgingProducer',
                             'Capture', 'Band', 'Unclassified')}
    for m in re.finditer(re.escape(bytes.fromhex('157c4a7fb979379e')), text):
        rva = start + m.start() - 2
        bounds = image.enclosing(rva)
        ins = image.decoded(bounds) if bounds else []
        at = next((n for n, i in enumerate(ins) if i.address - image.base == rva), None)
        if at is None or ins[at].mnemonic != 'movabs':
            continue
        tail = ins[at:at + 60]
        if any(i.mnemonic == 'shr' and i.op_str.endswith('0x2d') for i in tail) and any(
                i.mnemonic == 'shr' and i.op_str.endswith('0x11') and n + 1 < len(tail)
                and tail[n + 1].mnemonic == 'add' and tail[n + 1].op_str.endswith(', rax')
                for n, i in enumerate(tail)):
            found['Capture'].append(rva)
        elif any(i.mnemonic == 'shr' and i.op_str.endswith('0x2d') for i in ins[max(at - 120, 0):at]) and any(
                i.mnemonic == 'inc' and [j.mnemonic for j in tail[n + 1:n + 3]] == ['je', 'mul']
                for n, i in enumerate(tail[:30])):
            found['Band'].append(rva)
    for bounds in sorted(functions):
        ins = image.decoded(bounds)
        for n, i in enumerate(ins):
            rva = i.address - image.base
            nxt = ins[n + 1] if n + 1 < len(ins) else None
            prev = ins[n - 1] if n else None
            mem = code.memory(i)
            if i.mnemonic == 'sub' and mem and mem.mem.disp == 0x6a0 and code.image.cs.reg_name(mem.mem.base) != 'rip':
                out = i.op_str.split(',')[0].strip()
                if nxt and nxt.mnemonic == 'cmp' and nxt.op_str == f'{out}, 0x1389':
                    found['FlooredCollision'].append(nxt.address - image.base)
                elif nxt and nxt.mnemonic == 'test' and prev and prev.mnemonic == 'add' \
                        and prev.op_str.startswith(out + ','):
                    found['Collision'].append(rva)
                else:
                    found['Unclassified'].append(rva)
            elif i.mnemonic == 'imul' and i.op_str.endswith(', 0x46') and nxt and nxt.mnemonic == 'mov' \
                    and code.memory(nxt) and code.base_of(nxt) == 'rbp':
                found['AccuracyProducer'].append(rva)
            elif i.mnemonic == 'imul' and i.op_str.endswith((', 0x96', ', 0xc8')) and nxt and nxt.mnemonic == 'mov' \
                    and code.memory(nxt) and code.memory(nxt).mem.disp == 0x6a0:
                found['DodgingProducer'].append(rva)
    return {k: sorted(set(v)) for k, v in found.items()}
