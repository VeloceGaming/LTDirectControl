"""Map functions of one preserved game build onto another by following
calls outward from pairs already known to correspond.

Two functions that align instruction for instruction call corresponding
functions at their aligned direct calls. Starting from seed pairs (given, or
the whole-function matches of a patch_migration compare report), each
aligned direct call votes for a (old callee, new callee) pair, and voted
pairs are aligned in turn. Review aid only: it reads the two executables
and writes a JSON map under research/; every pair still needs review.

    python tools/map_functions.py --old research/game-builds/0.6.3 \
        --new research/game-builds/0.7.0 \
        --report research/game-builds/migration-0.6.3-to-0.7.0.json \
        --seed 0x930640=0xb194c0 \
        --output research/game-builds/function-map-0.6.3-to-0.7.0.json
"""
import argparse
import difflib
import json
import time
from collections import Counter, deque
from pathlib import Path

import patch_migration as pm
from compare_functions import normalised


def calls(image, bounds):
    """(index, target function start) of each direct call or tail jump that
    leaves the function."""
    out = {}
    for index, i in enumerate(image.decoded(bounds)):
        if i.mnemonic in ('call', 'jmp') and i.op_str.startswith('0x'):
            target = int(i.op_str, 16) - image.base
            if not bounds[0] <= target < bounds[1]:
                parent = image.enclosing(target)
                if parent and parent[0] == target:
                    out[index] = target
    return out


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--old', type=Path, required=True)
    parser.add_argument('--new', type=Path, required=True)
    parser.add_argument('--report', type=Path)
    parser.add_argument('--seed', action='append', default=[], help='old=new function addresses')
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--max-instructions', type=int, default=30000)
    parser.add_argument('--minimum-ratio', type=float, default=0.5)
    parser.add_argument('--seconds', type=int, default=1500)
    args = parser.parse_args()
    old = pm.Image(args.old / 'TeamfightManager2.exe')
    new = pm.Image(args.new / 'TeamfightManager2.exe')
    queue = deque()
    for seed in args.seed:
        a, b = (int(v, 0) for v in seed.split('='))
        queue.append((old.enclosing(a)[0], new.enclosing(b)[0], 'seed'))
    if args.report:
        for row in json.loads(args.report.read_text(encoding='utf-8'))['proposals'].values():
            if row['status'] == 'unique_candidate_requires_review' and row['candidates'][0]['basis'] == 'masked_function':
                a, b = old.enclosing(int(row['old_rva'], 16)), new.enclosing(int(row['candidates'][0]['rva'], 16))
                if a and b:
                    queue.append((a[0], b[0], 'report'))
    mapping, votes, rejected = {}, {}, {}
    started = time.time()
    while queue and time.time() - started < args.seconds:
        a, b, source = queue.popleft()
        if a in mapping:
            continue
        old_bounds, new_bounds = old.enclosing(a), new.enclosing(b)
        x, y = normalised(old, old.decoded(old_bounds)), normalised(new, new.decoded(new_bounds))
        if max(len(x), len(y)) > args.max_instructions and source != 'seed':
            rejected[a] = 'too large'
            continue
        matcher = difflib.SequenceMatcher(None, [t for _, t in x], [t for _, t in y], autojunk=False)
        blocks = matcher.get_matching_blocks()
        equal = sum(block.size for block in blocks)
        ratio = equal / max(len(x), len(y), 1)
        if ratio < args.minimum_ratio and source != 'seed':
            rejected[a] = f'{hex(b)} aligns only {ratio:.2f}'
            continue
        mapping[a] = {'new': b, 'equal': equal, 'old_instructions': len(x), 'new_instructions': len(y), 'via': source}
        old_calls, new_calls = calls(old, old_bounds), calls(new, new_bounds)
        for block in blocks:
            for k in range(block.size):
                p, q = old_calls.get(block.a + k), new_calls.get(block.b + k)
                if p is not None and q is not None:
                    votes.setdefault(p, Counter())[q] += 1
        for p, counter in votes.items():
            if p not in mapping and p not in rejected:
                (q, top), = counter.most_common(1)
                # One clear winner; a tie or a split vote is left for review.
                if top >= 0.75 * sum(counter.values()):
                    queue.append((p, q, f'call from {a:#x}'))
        if len(mapping) % 200 == 0:
            print(f'{len(mapping)} mapped, {len(queue)} queued, {int(time.time() - started)} s', flush=True)
    result = {
        'warning': 'Review aid. A pair means two functions align and were reached by aligned calls; nothing is approved.',
        'pairs': {hex(a): {**row, 'new': hex(row['new'])} for a, row in sorted(mapping.items())},
        'split_votes': {hex(p): {hex(q): n for q, n in c.items()} for p, c in votes.items()
                        if p not in mapping and len(c) > 1},
        'rejected': {hex(a): why for a, why in rejected.items()},
        'unfinished': len(queue),
    }
    args.output.write_text(json.dumps(result, indent=1), encoding='utf-8')
    print(f'{len(mapping)} functions mapped, {len(rejected)} rejected, {len(queue)} left in the queue')


if __name__ == '__main__':
    main()
