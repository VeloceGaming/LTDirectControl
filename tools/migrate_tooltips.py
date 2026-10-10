"""Carry a profile's tooltip anchors from one preserved game build to another.

The native ability-description reader checks ~400 anchors: the entry bytes
of the functions it calls, the consumers that show how the game calls them,
and for every ChampionInfo implementation its vtable header, drop/Q/W/R
pointers and the instruction that returns it from the built-in lookup.

For each anchor of the old profile this finds the new place:
- code bytes: the instruction aligned with the old one (as relocate.py),
  with the new bytes of the same instruction count;
- built-in tables: the table the aligned `lea rdx, [table]` of the lookup
  now returns, plus every table the new lookup returns that the old one
  did not (new champions);
- the mod and data tables: the vtable whose Q/W/R slots hold the functions
  the old ones map to;
- strings: the same bytes, where they occur once.

Review aid only: reads the executables and writes JSON. Everything it
could not carry is listed under "unresolved"; nothing is guessed.

    python tools/migrate_tooltips.py --old research/game-builds/0.6.3 \
        --new research/game-builds/0.7.0 --profile old-profile.json \
        --map research/game-builds/function-map-0.6.3-to-0.7.0.json --output out.json
"""
import argparse
import difflib
import json
import re
import struct
from pathlib import Path

import field_shifts
import patch_migration as pm
from compare_functions import normalised

SLOTS = {'DROP': 0, 'Q': 0xe0, 'W': 0xe8, 'R': 0xf0}


class Carry:
    def __init__(self, old, new, known):
        self.old, self.new, self.known, self.aligned = old, new, known, {}

    def function(self, start):
        """New function for the old one starting at `start`, with the
        instruction mapping (old rva -> new rva) and how it was found."""
        if start not in self.aligned:
            bounds = self.old.enclosing(start)
            target, how = self.known.get(start), 'map'
            if target is None:
                votes = field_shifts.locate(self.old, self.new, start)
                target, how = (votes[0][0] if votes else None), 'sampled'
            mapping, equal, size = {}, 0, 0
            if target is not None:
                x = normalised(self.old, self.old.decoded(bounds))
                y = normalised(self.new, self.new.decoded(self.new.enclosing(target)))
                matcher = difflib.SequenceMatcher(None, [t for _, t in x], [t for _, t in y], autojunk=False)
                for block in matcher.get_matching_blocks():
                    equal += block.size
                    for k in range(block.size):
                        mapping[x[block.a + k][0]] = y[block.b + k][0]
                size = len(x)
            self.aligned[start] = (target, mapping, how, equal, size)
        return self.aligned[start]

    def code(self, rva, length):
        """(new rva, new bytes, note) for `length` old bytes at `rva`."""
        bounds = self.old.enclosing(rva)
        if bounds is None:
            return None, None, 'not in a function'
        target, mapping, how, equal, size = self.function(bounds[0])
        if target is None or rva not in mapping:
            return None, None, f'not aligned ({how})'
        where = mapping[rva]
        before = [i for i in self.old.decoded(bounds) if rva <= i.address - self.old.base < rva + length]
        if sum(i.size for i in before) != length:
            return None, None, 'old bytes do not end on an instruction'
        new_bounds = self.new.enclosing(where)
        after = [i for i in self.new.decoded(new_bounds) if i.address - self.new.base >= where][:len(before)]
        a = [t for _, t in normalised(self.old, before)]
        b = [t for _, t in normalised(self.new, after)]
        note = f'{how} {equal}/{size}' + ('' if a == b else '; INSTRUCTIONS DIFFER: ' + ' | '.join(
            f'{x} -> {y}' for x, y in zip(a, b) if x != y))
        return where, self.new.pe.get_data(where, sum(i.size for i in after)), note


def similar(old, new, a, b):
    """How well the functions starting at old `a` and new `b` align (0..1)."""
    x, y = old.enclosing(a), new.enclosing(b)
    if not x or not y or x[0] != a or y[0] != b:
        return 0.
    p = [t for _, t in normalised(old, old.decoded(x))]
    q = [t for _, t in normalised(new, new.decoded(y))]
    return difflib.SequenceMatcher(None, p, q, autojunk=False).ratio()


def lea_target(image, rva):
    """Target of `lea reg, [rip + d]` at rva."""
    data = image.pe.get_data(rva, 7)
    if data[0] in (0x48, 0x4c) and data[1] == 0x8d and data[2] & 0xc7 == 0x05:
        return rva + 7 + struct.unpack_from('<i', data, 3)[0]
    return None


def table(image, rva):
    drop, size, align = struct.unpack('<QQQ', image.pe.get_data(rva, 24))
    slots = {name: struct.unpack('<Q', image.pe.get_data(rva + off, 8))[0] - image.base for name, off in SLOTS.items()}
    return size, align, slots


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--old', type=Path, required=True)
    parser.add_argument('--new', type=Path, required=True)
    parser.add_argument('--profile', type=Path, required=True, help='old profile with tooltip_anchors')
    parser.add_argument('--map', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    old = pm.Image(args.old / 'TeamfightManager2.exe')
    new = pm.Image(args.new / 'TeamfightManager2.exe')
    known = {int(a, 16): int(row['new'], 16)
             for a, row in json.loads(args.map.read_text(encoding='utf-8'))['pairs'].items()}
    carry = Carry(old, new, known)
    anchors = json.loads(args.profile.read_text(encoding='utf-8'))['tooltip_anchors']
    out, notes, unresolved = {}, {}, {}
    text_end = lambda image: image.text_rva + len(image.text)

    # Tables: old address -> (name stem, new address).
    stems = {}
    for name, a in anchors.items():
        m = re.fullmatch(r'TOOLTIP_(.+)_LAYOUT', name)
        if m:
            stems[int(a['rva'], 16) - 8] = m.group(1)
    lookup_old = int(anchors['TOOLTIP_BUILTIN_LOOKUP_ENTRY']['rva'], 16)
    lookup_new, mapping, how, equal, size = carry.function(lookup_old)
    notes['builtin lookup'] = f'{lookup_old:#x} -> {lookup_new:#x} ({how}, {equal}/{size} instructions align)'
    returns = lambda image, start: {i.address - image.base: lea_target(image, i.address - image.base)
                                    for i in image.decoded(image.enclosing(start))
                                    if i.mnemonic == 'lea' and i.op_str.startswith('rdx, [rip')}
    old_returns, new_returns = returns(old, lookup_old), returns(new, lookup_new)
    tables = {}  # old table -> new table
    sites = {}   # new table -> new lea site
    for site, target in old_returns.items():
        if target in stems and site in mapping and mapping[site] in new_returns:
            tables[target] = new_returns[mapping[site]]
            sites[tables[target]] = mapping[site]
    # The mod and data tables are not returned by the built-in lookup: find
    # the vtable holding the functions their Q/W/R map to.
    rdata = next(s for s in new.pe.sections if s.Name.rstrip(b'\0') == b'.rdata')
    blob, start = rdata.get_data(), rdata.VirtualAddress
    low, high = new.text_rva, text_end(new)
    code = lambda v: low <= v - new.base < high
    shaped = {}  # (Q, W, R) -> [vtable] over everything shaped like a ChampionInfo table
    for at in range(0, len(blob) - 0xf8, 8):
        drop, size, align = struct.unpack_from('<QQQ', blob, at)
        if align == 8 and 0x100 <= size <= 0x2000 and size % 8 == 0 and (drop == 0 or code(drop)):
            triple = struct.unpack_from('<QQQ', blob, at + 0xe0)
            if all(code(v) for v in triple):
                shaped.setdefault(tuple(v - new.base for v in triple), []).append(start + at)
    for target, stem in sorted(stems.items()):
        if target in tables:
            continue
        size_old, align_old, slots = table(old, target)
        # The functions are small and alike, so sampling cannot locate them:
        # rank every Q/W/R triple by how its three functions align with the old ones.
        ranked = sorted(((min(similar(old, new, slots[s], triple[n]) for n, s in enumerate('QWR')), triple)
                         for triple in shaped), reverse=True)[:2]
        score, triple = ranked[0]
        free = [h for h in shaped[triple] if h not in tables.values()]
        if score < 0.8 or not free:
            unresolved[f'table {stem}'] = f'best Q/W/R triple aligns {score:.2f}, {len(free)} free tables'
            continue
        # Tables sharing Q/W/R (the data variants) are paired by their drop function.
        best = max(free, key=lambda h: similar(old, new, slots['DROP'], table(new, h)[2]['DROP']))
        tables[target] = best
        notes[f'table {stem}'] = (f'{target:#x} -> {best:#x}: Q/W/R align {score:.2f} (next triple {ranked[1][0]:.2f}), '
                                  f'{len(shaped[triple])} tables share them, drop aligns '
                                  f"{similar(old, new, slots['DROP'], table(new, best)[2]['DROP']):.2f}")

    # Entry bytes of a table's own Q/W/R functions come from the new table.
    entry = {}
    for name in anchors:
        m = re.fullmatch(r'TOOLTIP_(ILLUSION|MOD|DATA)_([QWR])_ENTRY', name)
        if m:
            owner = next((t for t, stem in sorted(stems.items())
                          if stem == m.group(1) or stem.startswith(m.group(1) + '_')), None)
            if owner in tables:
                entry[name] = table(new, tables[owner])[2][m.group(2)]
    renamed = {}
    for target, stem in stems.items():
        if target in tables:
            renamed[stem] = f'BUILTIN_{tables[target]:X}' if stem.startswith('BUILTIN_') else stem

    def emit_table(stem, address, site):
        size, align, slots = table(new, address)
        out[f'TOOLTIP_{stem}_LAYOUT'] = {'rva': hex(address + 8), 'bytes': struct.pack('<QQ', size, align).hex()}
        for slot, off in SLOTS.items():
            out[f'TOOLTIP_{stem}_{slot}_POINTER'] = {'rva': hex(address + off), 'pointer_target': hex(slots[slot])}
        if site is not None:
            out[f'TOOLTIP_{stem}_LOOKUP_RETURN'] = {'rva': hex(site), 'bytes': new.pe.get_data(site, 12).hex()}
        return size, align, slots

    for name, a in anchors.items():
        rva = int(a['rva'], 16)
        m = re.fullmatch(r'TOOLTIP_(.+)_(LAYOUT|DROP_POINTER|Q_POINTER|W_POINTER|R_POINTER|LOOKUP_RETURN)', name)
        if m and m.group(1) in {s for s in stems.values()}:
            continue  # tables are emitted whole below
        if 'bytes' not in a:
            unresolved[name] = 'pointer anchor outside a known table'
            continue
        length = len(a['bytes']) // 2
        if rva >= text_end(old):
            raw = bytes.fromhex(a['bytes'])
            hits = [start + m.start() for m in re.finditer(re.escape(raw), blob)]
            if len(hits) == 1:
                out[name] = {'rva': hex(hits[0]), 'bytes': a['bytes']}
            else:
                # The copy an aligned `lea` of a carried function refers to.
                via = set()
                for fn_name in ('TOOLTIP_SHEET_GETTER_ENTRY', 'TOOLTIP_INFO_LOOKUP_ENTRY'):
                    f = int(anchors[fn_name]['rva'], 16)
                    fmap = carry.function(f)[1]
                    for i in old.decoded(old.enclosing(f)):
                        site = i.address - old.base
                        if i.mnemonic == 'lea' and lea_target(old, site) == rva and site in fmap:
                            via.add(lea_target(new, fmap[site]))
                via &= set(hits)
                if len(via) == 1:
                    out[name] = {'rva': hex(via.pop()), 'bytes': a['bytes']}
                    continue
                unresolved[name] = f'{len(hits)} occurrences of the bytes, {len(via)} referenced'
            continue
        if name in entry:
            before = [i for i in old.decoded(old.enclosing(rva)) if rva <= i.address - old.base < rva + length]
            after = new.decoded(new.enclosing(entry[name]))[:len(before)]
            x = [t for _, t in normalised(old, before)]
            y = [t for _, t in normalised(new, after)]
            where, data = entry[name], new.pe.get_data(entry[name], sum(i.size for i in after))
            note = 'from the table slot' + ('' if x == y else '; INSTRUCTIONS DIFFER: ' + ' | '.join(
                f'{u} -> {v}' for u, v in zip(x, y) if u != v))
        else:
            where, data, note = carry.code(rva, length)
        if where is None:
            unresolved[name] = note
            continue
        out[name] = {'rva': hex(where), 'bytes': data.hex()}
        if 'DIFFER' in note or len(data) != length:
            notes[name] = note + (f'; length {length} -> {len(data)}' if len(data) != length else '')

    checks = []
    for target, stem in stems.items():
        if target not in tables:
            continue
        new_stem = renamed[stem]
        had_return = f'TOOLTIP_{stem}_LOOKUP_RETURN' in anchors
        size, align, slots = emit_table(new_stem, tables[target], sites.get(tables[target]) if had_return else None)
        o_size, o_align, o_slots = table(old, target)
        scores = {s: similar(old, new, o_slots[s], slots[s]) for s in ('Q', 'W', 'R')}
        if min(scores.values()) < 0.8 or align != o_align:
            checks.append(f'{stem} -> {new_stem}: ' + ', '.join(f'{s} aligns {v:.2f}' for s, v in scores.items()))
        if size != o_size:
            notes[f'size {new_stem}'] = f'{o_size:#x} -> {size:#x}'
    added = sorted(set(new_returns.values()) - set(tables.values()))
    for address in added:
        site = next(s for s, t in new_returns.items() if t == address)
        emit_table(f'BUILTIN_{address:X}', address, site)
    result = {
        'tooltip_anchors': out,
        'tables': {'carried': len(tables), 'of': len(stems), 'added': [hex(a) for a in added],
                   'old_to_new': {hex(a): hex(b) for a, b in sorted(tables.items())}},
        'notes': notes, 'checks': checks, 'unresolved': unresolved,
    }
    args.output.write_text(json.dumps(result, indent=1), encoding='utf-8')
    print(f"{len(out)} anchors ({len(anchors)} before); tables {len(tables)}/{len(stems)} carried, {len(added)} added; "
          f"{len(notes)} notes, {len(checks)} checks, {len(unresolved)} unresolved")


if __name__ == '__main__':
    main()
