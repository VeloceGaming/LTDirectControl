"""Draft the tables of a new game version from those of a supported one.

Chains the migration tools (patch_migration compare, map_functions,
migrate_tooltips, the vtable matching of match_vtables and the alignment of
relocate) over every address in `probe/src/native_adapter/v<old>/` and in
`tools/native_profiles/<old>.json`, and writes under
`research/game-builds/draft-<new>/`:

- `v<new>/*.rs`: the old folder with each address and byte pattern carried
  over. An address the tools could not carry becomes `UNKNOWN_0x<old>`
  (which does not compile); one carried with less than full confidence
  keeps a `// REVIEW` note on its line.
- `<new>.json`: the profile, with `"reviewed": false` (the verifier refuses
  it until a person sets it) and a `draft` section giving each anchor's
  status.
- `report.json`: every address with its status and how it was found.

Statuses: `exact` (the enclosing function is in the function map and aligns
almost entirely, or a vtable/pointer match with no rival), `likely` (found,
with less agreement: review), `unknown` (not found, or the candidate
function barely aligns). A function found by its code alone may exist in
several identical copies; the note then says so, and the right copy has to
be chosen by its caller.

Review aid only: it reads the two executables and writes under research/.
Nothing in probe/ or tools/native_profiles/ is changed, and "exact" still
means "check the family in game", not "approved".

    python tools/draft_version.py --old 0.7.0 --new 0.7.1

Both versions need a copy under research/game-builds/<version>/
(`python tools/patch_migration.py snapshot --label <version> --output
research/game-builds/<version>`). `--check <folder>` scores the draft
against an already reviewed folder (how the tool itself was validated:
0.6.3 -> 0.7.0 against v070).
"""
import argparse
import difflib
import hashlib
import json
import re
import struct
import subprocess
import sys
from collections import Counter, defaultdict
from pathlib import Path

import field_shifts
import match_vtables
import patch_migration as pm
from compare_functions import normalised

ROOT = Path(__file__).resolve().parents[1]
BUILDS = ROOT / 'research/game-builds'
ADAPTER = ROOT / 'probe/src/native_adapter'
ADDRESS = re.compile(r'\b0x[0-9a-f]{5,}\b')
BYTES = re.compile(r'\[\s*(?:0x[0-9a-f]{1,2}\s*,\s*){2,}0x[0-9a-f]{1,2}\s*,?\s*\]')
ORDER = {'exact': 0, 'likely': 1, 'unknown': 2}
# Below this share of aligned instructions a candidate function is not a match.
WEAK = 0.3


def folder_name(version):
    return 'v' + version.replace('.', '')


def run(*arguments):
    print('>', ' '.join(str(a) for a in arguments), flush=True)
    subprocess.run([sys.executable, *map(str, arguments)], cwd=ROOT, check=True)


class Draft:
    def __init__(self, old_dir, new_dir, pairs, report, tooltips, tables):
        self.old = pm.Image(old_dir / 'TeamfightManager2.exe')
        self.new = pm.Image(new_dir / 'TeamfightManager2.exe')
        self.pairs = pairs
        self.report = report
        self.fixed = dict(tooltips)
        # Tooltip tables carried by migrate_tooltips: old start -> new start.
        self.tables = dict(tables)
        # Anchors of the comparison report with one candidate, by old address.
        self.candidates = {int(row['old_rva'], 16): int(row['candidates'][0]['rva'], 16)
                           for row in report.values() if row['status'] == 'unique_candidate_requires_review'}
        self.done, self.aligned = {}, {}
        self.old_tables = self.new_tables = self.pointers = None
        self.sections = {}
        new_sections = {s.Name.rstrip(b'\0').decode(): s for s in self.new.pe.sections}
        for s in self.old.pe.sections:
            other = new_sections.get(s.Name.rstrip(b'\0').decode())
            if other:
                self.sections[s.VirtualAddress] = other.VirtualAddress
                self.sections[s.VirtualAddress + s.Misc_VirtualSize] = other.VirtualAddress + other.Misc_VirtualSize

    def is_code(self, image, rva):
        return image.text_rva <= rva < image.text_rva + len(image.text)

    def vtables(self):
        if self.old_tables is None:
            print('scanning vtables...', flush=True)
            self.old_tables, self.new_tables = match_vtables.tables(self.old), match_vtables.tables(self.new)
        return self.old_tables, self.new_tables

    def pointer_index(self):
        """Code pointer -> the data addresses of the new build that hold it."""
        if self.pointers is None:
            print('indexing data pointers...', flush=True)
            self.pointers = defaultdict(list)
            low = self.new.base + self.new.text_rva
            high = low + len(self.new.text)
            for section in self.new.pe.sections:
                if section.Name.rstrip(b'\0').decode() not in ('.rdata', '.data'):
                    continue
                data = section.get_data()
                for at, (value,) in enumerate(struct.iter_unpack('<Q', data[:len(data) // 8 * 8])):
                    if low <= value < high:
                        self.pointers[value - self.new.base].append(section.VirtualAddress + at * 8)
        return self.pointers

    def align(self, start, deep):
        """(new function, {old instruction: new instruction}, how, ratio)."""
        if (start, deep) in self.aligned:
            return self.aligned[start, deep]
        how, target = 'function map', self.pairs.get(start)
        if target is None and deep:
            before, after = self.vtables()
            sizes = [size for size, row in before.items() if start in row]
            if sizes:
                ranked = sorted(((match_vtables.ratio(self.old, self.new, start, candidate), candidate)
                                 for size in sizes for candidate in after[size]), reverse=True)[:2]
                if ranked and ranked[0][0] >= 0.5 and (len(ranked) == 1 or ranked[0][0] - ranked[1][0] >= 0.2):
                    how, target = f'vtable method ({ranked[0][0]:.2f})', ranked[0][1]
                else:
                    how = 'vtable method without a clear match'
            if target is None:
                votes = field_shifts.locate(self.old, self.new, start)
                if votes and (len(votes) == 1 or votes[0][1] > votes[1][1]):
                    how, target = f'sampled context x{votes[0][1]}', votes[0][0]
                    # A close second is a similar function: say so.
                    if len(votes) > 1 and votes[1][1] >= 0.6 * votes[0][1]:
                        how += f' (runner-up {votes[1][0]:#x} x{votes[1][1]}, choose by caller)'
            if target is None:
                # An anchor inside this function that the report placed.
                bounds = self.old.enclosing(start)
                inside = {self.new.enclosing(b)[0] for a, b in self.candidates.items()
                          if bounds[0] <= a < bounds[1] and self.new.enclosing(b)}
                if len(inside) == 1:
                    how, target = 'masked context of an anchor', inside.pop()
        result = (None, {}, how if target is None and how != 'function map' else 'function not found', 0.)
        bounds = self.new.enclosing(target) if target is not None else None
        if bounds:
            x = normalised(self.old, self.old.decoded(self.old.enclosing(start)))
            y = normalised(self.new, self.new.decoded(bounds))
            matcher = difflib.SequenceMatcher(None, [t for _, t in x], [t for _, t in y], autojunk=False)
            mapping, equal = {}, 0
            for block in matcher.get_matching_blocks():
                equal += block.size
                for k in range(block.size):
                    mapping[x[block.a + k][0]] = y[block.b + k][0]
            if how != 'function map':
                how += self.copies(bounds)
            result = (bounds[0], mapping, how, equal / max(len(x), len(y), 1))
        self.aligned[start, deep] = result
        return result

    def copies(self, bounds):
        """A note when the new build holds this function more than once.

        The function map tells copies apart by their callers; a match by
        code alone (sampling, a vtable slot, an anchor's context) cannot.
        """
        value, mask = pm.masked_signature(self.new.decoded(bounds), self.new.base, self.new.image_size)
        if len(value) != bounds[1] - bounds[0]:
            return ''
        hits = pm.pattern_hits(self.new.text, value, mask)
        if not hits:
            return '; too short to rule out identical copies'
        starts = [h for h in hits if self.new.by_start.get(self.new.text_rva + h) == self.new.text_rva + h + len(value)]
        if len(starts) <= 1:
            return ''
        return f'; {len(starts)}{"+" if len(hits) >= 33 else ""} identical copies in the new build, choose by caller'

    def code(self, rva, deep=True):
        bounds = self.old.enclosing(rva)
        if bounds is None:
            # A leaf without unwind data: only the comparison report knows it.
            if rva in self.candidates:
                return self.candidates[rva], 'likely', 'unique masked context (leaf)'
            return None, 'unknown', 'no unwind data and no unique context'
        target, mapping, how, ratio = self.align(bounds[0], deep)
        if target is None:
            return None, 'unknown', how
        if ratio < WEAK:
            return None, 'unknown', f'{how}: candidate {target:#x} aligns only {ratio:.2f}'
        where = mapping.get(rva)
        if where is None:
            return None, 'unknown', f'{how}: function {target:#x} found, instruction not aligned'
        sure = ratio >= 0.9 and (how == 'function map' or how.endswith('(1.00)')) and 'copies' not in how
        return where, 'exact' if sure else 'likely', f'{how}, {ratio:.2f} aligned'

    def data(self, rva):
        """A table or pointer slot, by the code pointers stored around it."""
        index, votes, slots = self.pointer_index(), None, 0
        for k in range(-6, 8):
            try:
                value = struct.unpack('<Q', self.old.pe.get_data(rva + 8 * k, 8))[0] - self.old.base
            except Exception:
                continue
            if not self.is_code(self.old, value):
                continue
            target, status, _ = self.code(value, deep=False)
            if target is None or status != 'exact':
                continue
            here = {address - 8 * k for address in index.get(target, [])}
            votes = here if votes is None else votes & here
            slots += 1
        if votes and len(votes) == 1:
            return votes.pop(), 'exact' if slots >= 2 else 'likely', f'{slots} mapped pointers nearby'
        return None, 'unknown', f'{slots} mapped pointers nearby, {len(votes or [])} candidates'

    def resolve(self, rva):
        if rva not in self.done:
            if rva in self.fixed:
                self.done[rva] = (self.fixed[rva], 'exact', 'tooltip migration')
            elif any(0 <= rva - a < 0x400 for a in self.tables):
                a = next(a for a in self.tables if 0 <= rva - a < 0x400)
                self.done[rva] = (self.tables[a] + rva - a, 'exact', 'inside a carried tooltip table')
            elif rva in self.sections:
                self.done[rva] = (self.sections[rva], 'exact', 'section bound')
            elif self.is_code(self.old, rva):
                self.done[rva] = self.code(rva)
            else:
                self.done[rva] = self.data(rva)
        return self.done[rva]

    def carried_bytes(self, rva, length):
        """The new bytes for `length` old bytes at `rva`, or (None, why)."""
        where, status, _ = self.resolve(rva)
        if where is None:
            return None, 'address unknown'
        if not self.is_code(self.old, rva):
            return self.new.pe.get_data(where, length), status
        old = list(self.old.cs.disasm(self.old.pe.get_data(rva, length), self.old.base + rva))
        if sum(i.size for i in old) != length:
            return self.new.pe.get_data(where, length), status
        new = list(self.new.cs.disasm(self.new.pe.get_data(where, length + 64), self.new.base + where))[:len(old)]
        value = b''.join(bytes(i.bytes) for i in new)
        same = [t for _, t in normalised(self.old, old)] == [t for _, t in normalised(self.new, new)]
        return value, status if same else 'likely'


def identity(image, executable):
    return {'sha': hashlib.sha256(executable.read_bytes()).hexdigest(),
            'timestamp': image.pe.FILE_HEADER.TimeDateStamp, 'size': image.image_size}


def draft_source(draft, text, special, records, name):
    """`text` with every address and byte pattern carried over."""
    comments = [m.span() for m in re.finditer(r'//[^\n]*', text)]
    in_comment = lambda at: any(a <= at < b for a, b in comments)
    # (start, end, replacement, note for the end of that line or None)
    edits, addresses = [], []
    for m in ADDRESS.finditer(text):
        value = int(m.group(0), 16)
        if in_comment(m.start()):
            continue
        if value in special:
            edits.append((m.start(), m.end(), f'{special[value]:#x}', None))
            continue
        if not 0x10000 <= value < draft.old.image_size:
            continue
        where, status, how = draft.resolve(value)
        addresses.append((m.start(), value))
        records.append({'file': name, 'old': hex(value), 'new': hex(where) if where is not None else None,
                        'status': status, 'how': how})
        if where is None:
            edits.append((m.start(), m.end(), f'UNKNOWN_{value:#x}', f'unknown {value:#x}: {how}'))
        else:
            note = None if status == 'exact' else f'likely {value:#x} -> {where:#x}: {how}'
            edits.append((m.start(), m.end(), f'{where:#x}', note))
    for m in BYTES.finditer(text):
        if in_comment(m.start()):
            continue
        value = bytes(int(b, 16) for b in re.findall(r'0x([0-9a-f]{1,2})\b', m.group(0)))
        # The address these bytes sit at: the nearest one before them first.
        ordered = sorted(addresses, key=lambda a: (a[0] > m.start(), abs(m.start() - a[0])))
        homes = list(dict.fromkeys(a for _, a in ordered if draft.old.pe.get_data(a, len(value)) == value))
        record = {'file': name, 'bytes_at': hex(homes[0]) if homes else None, 'old_bytes': value.hex()}
        records.append(record)
        if not homes:
            record.update(status='unknown', how='bytes not found at any address of this file')
            edits.append((m.start(), m.end(), m.group(0), 'bytes not located: kept from the old version'))
            continue
        carried = {draft.carried_bytes(home, len(value)) for home in homes}
        results = {c[0] for c in carried}
        if len(results) != 1 or None in results:
            record.update(status='unknown', how='address unknown, or its occurrences disagree')
            edits.append((m.start(), m.end(), m.group(0),
                          f'bytes at {homes[0]:#x} not carried: kept from the old version'))
            continue
        new, status = results.pop(), max((c[1] for c in carried), key=ORDER.get)
        record.update(status=status, new_bytes=new.hex(), how='read at the carried address')
        note = None
        if status != 'exact' or len(new) != len(value):
            note = (f'bytes at {homes[0]:#x}: instruction changed or address only likely'
                    + (f'; length {len(value)} -> {len(new)}' if len(new) != len(value) else ''))
        edits.append((m.start(), m.end(), '[' + ', '.join(f'{b:#04x}' for b in new) + ']', note))
    # From the end backwards, so earlier positions stay valid.
    for a, b, replacement, note in sorted(edits, reverse=True):
        text = text[:a] + replacement + text[b:]
        if note:
            end = text.find('\n', a + len(replacement))
            end = len(text) if end < 0 else end
            text = text[:end] + '  // REVIEW ' + note + text[end:]
    return text


def draft_profile(draft, profile, tooltip_result, new_version, ident):
    status = {}

    def anchor(name, a):
        where, state, how = draft.resolve(int(a['rva'], 16))
        status[name] = f'{state}: {how}'
        if where is None:
            return dict(a, rva=None)
        row = {'rva': hex(where)}
        if 'bytes' in a:
            value, state_bytes = draft.carried_bytes(int(a['rva'], 16), len(bytes.fromhex(a['bytes'])))
            row['bytes'] = value.hex()
            if state_bytes != 'exact' and state == 'exact':
                status[name] = f'likely: instruction changed at an exact address ({how})'
            if 'target' in a:
                if value[0] in (0xe8, 0xe9) and len(value) == 5:
                    row['target'] = hex(where + 5 + struct.unpack_from('<i', value, 1)[0])
                else:
                    row['target'] = None
        else:
            row['pointer_target'] = hex(struct.unpack('<Q', draft.new.pe.get_data(where, 8))[0] - draft.new.base)
        return row

    out = dict(profile, game_version=new_version, reviewed=False, executable_sha256=ident['sha'],
               timestamp=hex(ident['timestamp']), size_of_image=hex(ident['size']), image_base=hex(draft.new.base),
               review_notes='Draft from tools/draft_version.py; nothing reviewed.', native_game_test='none')
    out['anchors'] = {name: anchor(name, a) for name, a in profile['anchors'].items()}
    out['layout_anchors'] = {name: anchor(name, a) for name, a in profile['layout_anchors'].items()}
    if tooltip_result:
        out['tooltip_anchors'] = tooltip_result['tooltip_anchors']
        for name, why in tooltip_result['unresolved'].items():
            status['TOOLTIP ' + name] = f'unknown: {why}'
    if 'minimap' in profile:
        operands = []
        for op in profile['minimap']['operands']:
            rva = int(op['rva'], 16)
            where, state, how = draft.resolve(rva)
            status['MINIMAP_' + op['name']] = f'{state}: {how}'
            row = dict(op)
            if where is None:
                row['rva'] = None
            else:
                value, _ = draft.carried_bytes(rva, len(bytes.fromhex(op['bytes'])))
                row.update(rva=hex(where), bytes=value.hex())
                if op['kind'] == 'rip_f32x4' and len(value) == len(bytes.fromhex(op['bytes'])):
                    target = where + len(value) + struct.unpack_from('<i', value, op['offset'])[0]
                    data = draft.new.pe.get_data(target, len(bytes.fromhex(op['data_bytes'])))
                    row.update(data_rva=hex(target), data_bytes=data.hex())
                    if data.hex() != op['data_bytes']:
                        status['MINIMAP_' + op['name']] = 'likely: the source constant changed value'
                elif value.hex() != op['bytes'] and op['kind'] != 'rip_f32x4':
                    status['MINIMAP_' + op['name']] = 'likely: the immediate instruction changed'
            operands.append(row)
        out['minimap'] = dict(profile['minimap'], reviewed=False, operands=operands)
    out['draft'] = status
    return out


def check(records, reviewed, new_image):
    """Score the drafted values against an already reviewed folder."""
    truth, arrays = defaultdict(set), defaultdict(set)
    for path in reviewed.glob('*.rs'):
        text = '\n'.join(line.split('//')[0] for line in path.read_text(encoding='utf-8').splitlines())
        truth[path.name] = {int(v, 16) for v in ADDRESS.findall(text)}
        arrays[path.name] = {bytes(int(b, 16) for b in re.findall(r'0x([0-9a-f]{1,2})\b', m.group(0))).hex()
                             for m in BYTES.finditer(text)}
    score, wrong = Counter(), []
    for r in records:
        kind = 'bytes' if 'old_bytes' in r else 'address'
        value = r.get('new_bytes') if kind == 'bytes' else r.get('new')
        if value is None:
            score[kind, r['status'], 'not carried'] += 1
            continue
        right = value in arrays[r['file']] if kind == 'bytes' else int(value, 16) in truth[r['file']]
        score[kind, r['status'], 'right' if right else 'wrong'] += 1
        if not right:
            wrong.append(r)
    return score, wrong


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--old', required=True, help='supported version, e.g. 0.7.0')
    parser.add_argument('--new', required=True, help='version to draft, e.g. 0.7.1')
    parser.add_argument('--output', type=Path)
    parser.add_argument('--fresh', action='store_true', help='redo the comparison report, function map and tooltip run')
    parser.add_argument('--check', type=Path, help='a reviewed v<new> folder to score the draft against')
    args = parser.parse_args()
    old_dir, new_dir = BUILDS / args.old, BUILDS / args.new
    for directory in (old_dir, new_dir):
        assert (directory / 'TeamfightManager2.exe').is_file(), (
            f'No copy of the game at {directory}; see patch_migration.py snapshot')
    source = ADAPTER / folder_name(args.old)
    profile_path = ROOT / 'tools/native_profiles' / f'{args.old}.json'
    assert source.is_dir() and profile_path.is_file(), f'{args.old} is not a supported version'
    profile = json.loads(profile_path.read_text(encoding='utf-8'))
    out = args.output or BUILDS / f'draft-{args.new}'
    (out / folder_name(args.new)).mkdir(parents=True, exist_ok=True)
    stem = f'{args.old}-to-{args.new}'
    report_path, map_path = BUILDS / f'migration-{stem}.json', BUILDS / f'function-map-{stem}.json'
    tooltip_path = out / 'tooltips.json'

    # 1. Anchors by masked function or context (also the seeds of the map).
    if args.fresh or not report_path.exists():
        receipt = {'executable_sha256': profile['executable_sha256'], 'native_branches': {
            name: a for group in ('anchors', 'layout_anchors', 'tooltip_anchors')
            for name, a in profile.get(group, {}).items() if 'bytes' in a}}
        report = pm.compare(old_dir / 'TeamfightManager2.exe', new_dir / 'TeamfightManager2.exe', receipt)
        report_path.write_text(json.dumps(report, indent=1), encoding='utf-8')
    report = json.loads(report_path.read_text(encoding='utf-8'))['proposals']
    # 2. Functions, by following aligned calls outward.
    if args.fresh or not map_path.exists():
        run('tools/map_functions.py', '--old', old_dir, '--new', new_dir, '--report', report_path, '--output', map_path)
    pairs = {int(a, 16): int(row['new'], 16)
             for a, row in json.loads(map_path.read_text(encoding='utf-8'))['pairs'].items()}
    # 3. Tooltip tables and their consumers.
    tooltip_result, tooltips = None, {}
    if 'tooltip_anchors' in profile:
        if args.fresh or not tooltip_path.exists():
            run('tools/migrate_tooltips.py', '--old', old_dir, '--new', new_dir, '--profile', profile_path,
                '--map', map_path, '--output', tooltip_path)
        tooltip_result = json.loads(tooltip_path.read_text(encoding='utf-8'))
        tables = {int(a, 16): int(b, 16) for a, b in tooltip_result['tables']['old_to_new'].items()}
        for name, a in profile['tooltip_anchors'].items():
            b = tooltip_result['tooltip_anchors'].get(name)
            if b:
                tooltips[int(a['rva'], 16)] = int(b['rva'], 16)

    draft = Draft(old_dir, new_dir, pairs, report, tooltips, tables if tooltip_result else {})
    # A function a carried table points to: what the same slot holds now.
    for a in profile.get('tooltip_anchors', {}).values():
        if 'pointer_target' in a:
            where, _, _ = draft.resolve(int(a['rva'], 16))
            if where is not None:
                target = struct.unpack('<Q', draft.new.pe.get_data(where, 8))[0] - draft.new.base
                draft.fixed.setdefault(int(a['pointer_target'], 16), target)
    before = identity(draft.old, old_dir / 'TeamfightManager2.exe')
    after = identity(draft.new, new_dir / 'TeamfightManager2.exe')
    special = {before['timestamp']: after['timestamp'], before['size']: after['size']}
    records = []
    for path in sorted(source.glob('*.rs')):
        print(f'drafting {path.name}...', flush=True)
        text = path.read_text(encoding='utf-8')
        text = text.replace(before['sha'], after['sha']).replace(f'"{args.old}"', f'"{args.new}"')
        text = text.replace(f'native_profiles/{args.old}.json', f'native_profiles/{args.new}.json')
        text = draft_source(draft, text, special, records, path.name)
        header = (f'//! DRAFT from {args.old} by tools/draft_version.py: nothing here is reviewed.\n'
                  '//! UNKNOWN_0x... could not be carried; lines marked REVIEW need a look.\n//!\n')
        (out / folder_name(args.new) / path.name).write_text(header + text, encoding='utf-8', newline='')
    drafted = draft_profile(draft, profile, tooltip_result, args.new, after)
    (out / f'{args.new}.json').write_text(json.dumps(drafted, indent=1), encoding='utf-8')
    (out / 'report.json').write_text(json.dumps(records, indent=1), encoding='utf-8')

    print(f'\nDraft of game {args.new} written to {out}')
    for name in sorted({r["file"] for r in records}):
        counts = Counter(('bytes ' if 'old_bytes' in r else 'address ') + r['status']
                         for r in records if r['file'] == name)
        print(f'  {name}: ' + ', '.join(f'{n} {k}' for k, n in sorted(counts.items())))
    states = Counter(v.split(':')[0] for v in drafted['draft'].values())
    print('  profile anchors: ' + ', '.join(f'{n} {k}' for k, n in sorted(states.items())))
    if tooltip_result:
        print(f"  tooltips: {len(tooltip_result['tooltip_anchors'])} anchors, {len(tooltip_result['unresolved'])} unresolved,"
              f" {len(tooltip_result['tables']['added'])} new tables")
    if args.check:
        score, wrong = check(records, args.check, draft.new)
        print(f'\nScored against {args.check}:')
        for key, n in sorted(score.items()):
            print(f'  {key[0]:8} {key[1]:8} {key[2]:12} {n}')
        for r in wrong[:40]:
            print('  wrong:', {k: r[k] for k in ('file', 'old', 'new', 'status', 'how', 'bytes_at') if k in r})


if __name__ == '__main__':
    main()
