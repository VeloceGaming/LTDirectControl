"""Read-only PE inspection of our installed game; no process memory or patches.

Find source-path panic metadata and candidate enclosing functions. These are
research leads, not callable addresses or verified hook locations.
"""
import argparse
import bisect
import hashlib
import json
import re
import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / '.tools' / 'python'))
import pefile


def inspect(path, all_paths=False):
    data = path.read_bytes()
    pe = pefile.PE(data=data, fast_load=True)
    base = pe.OPTIONAL_HEADER.ImageBase
    sections = {s.Name.rstrip(b'\0').decode(): s for s in pe.sections}
    rdata = sections['.rdata']
    text = sections['.text']
    ro = rdata.get_data()
    code = text.get_data()
    funcs = []
    for begin, end, unwind in struct.iter_unpack('<III', sections['.pdata'].get_data()[:sections['.pdata'].SizeOfRawData // 12 * 12]):
        if begin and end:
            funcs.append((begin, end))
    funcs.sort()
    starts = [f[0] for f in funcs]

    # Paths adjacent to string literals may concatenate. Keep the complete
    # printable path matched here and verify its pointer/length before use.
    paths = {}
    pattern = rb'(?:game-core|game-view|game-client|game-server|engine-core)[\\/]src[\\/][A-Za-z0-9_./\\-]*\.rs'
    for match in re.finditer(pattern, ro):
        name = match.group().decode().replace('\\', '/')
        if all_paths or re.search('sim|match|replay|game_view|watch|input|champion|in_game|game_scene|view|scene|render', name):
            paths.setdefault(name, []).append(rdata.VirtualAddress + match.start())

    locations = {}
    for name, rvas in paths.items():
        for rva in rvas:
            start = 0
            needle = struct.pack('<Q', base + rva)
            while True:
                i = ro.find(needle, start)
                if i < 0:
                    break
                start = i + 1
                if i + 24 > len(ro):
                    continue
                length, line, col = struct.unpack_from('<QII', ro, i + 8)
                # Reject unrelated pointers and implausible Location records.
                if not (0 < length < 300 and 0 < line < 100000 and 0 < col < 10000):
                    continue
                actual = pe.get_data(rva, length).decode(errors='replace').replace('\\', '/')
                if not actual.endswith('.rs'):
                    continue
                locations[rdata.VirtualAddress + i] = {'path': actual, 'line': line, 'column': col}

    hits = {}
    for match in re.finditer(rb'[\x48\x4c]\x8d[\x05\x0d\x15\x1d\x25\x2d\x35\x3d].{4}', code, re.DOTALL):
        offset = match.start()
        disp = struct.unpack_from('<i', code, offset + 3)[0]
        insn = text.VirtualAddress + offset
        target = insn + 7 + disp
        if target not in locations:
            continue
        index = bisect.bisect_right(starts, insn) - 1
        if index < 0 or not funcs[index][0] <= insn < funcs[index][1]:
            continue
        begin, end = funcs[index]
        hit = hits.setdefault(begin, {'rva': hex(begin), 'end_rva': hex(end), 'size': end - begin, 'anchors': []})
        hit['anchors'].append({'instruction_rva': hex(insn), **locations[target]})

    return {
        'executable': str(path),
        'sha256': hashlib.sha256(data).hexdigest(),
        'timestamp': hex(pe.FILE_HEADER.TimeDateStamp),
        'size_of_image': hex(pe.OPTIONAL_HEADER.SizeOfImage),
        'image_base': hex(base),
        'source_paths': sorted(paths),
        'candidate_functions': sorted(hits.values(), key=lambda h: int(h['rva'], 16)),
        'warning': 'Source anchors identify research candidates only; no hook or ABI is verified.',
    }


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--exe', type=Path, default=Path(r'C:\Program Files (x86)\Steam\steamapps\common\Teamfight Manager2\TeamfightManager2.exe'))
    parser.add_argument('--output', type=Path, default=ROOT / 'research' / 'game-0.6.2-map.json')
    parser.add_argument('--all-paths', action='store_true', help='Include paths outside the original gameplay-name filter.')
    args = parser.parse_args()
    result = inspect(args.exe, all_paths=args.all_paths)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2), encoding='utf-8')
    print(json.dumps({'paths': len(result['source_paths']), 'candidate_functions': len(result['candidate_functions']), 'output': str(args.output)}, indent=2))
