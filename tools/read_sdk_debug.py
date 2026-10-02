"""Extract archived LLVM members and read textual CodeView field records.

This reads research artifacts only; it does not load or invoke game code.
"""
import argparse
import json
import re
from pathlib import Path


def extract(archive, offset, output):
    with Path(archive).open('rb') as source:
        source.seek(offset)
        header = source.read(60)
        size = int(header[48:58])
        data = source.read(size)
    if not data.startswith(b'BC\xc0\xde'):
        raise ValueError('Expected an LLVM bitcode member')
    Path(output).write_bytes(data)
    return {'member_offset': offset, 'bytes': size, 'output': str(output)}


def structures(path, pattern):
    text = Path(path).read_text(encoding='utf-8')
    records = {}
    for match in re.finditer(r'^\s*# (\w+) \((0x[0-9A-Fa-f]+)\)\n(.*?)(?=^\s*# \w+ \(0x|\Z)', text, re.M | re.S):
        records[int(match[2], 16)] = (match[1], match[3])
    result = []
    for kind, body in records.values():
        if kind != 'Struct':
            continue
        name = re.search(r'\.asciz\s+"([^"]+)"\s+# Name', body)
        fields = re.search(r'\.long\s+(0x[0-9a-f]+)\s+# FieldList', body)
        if not name or not re.search(pattern, name[1]) or not fields:
            continue
        field_body = records.get(int(fields[1], 16), ('', ''))[1]
        members = []
        for block in re.split(r'(?=^\s*\.short.*# Member kind:)', field_body, flags=re.M):
            member_name = re.search(r'\.asciz\s+"([^"]+)"\s+# Name', block)
            field_type = re.search(r'# Type: (.*)', block)
            field_offset = re.search(r'\.(?:short|long|quad)\s+(0x[0-9a-f]+|\d+)\s+# FieldOffset', block)
            if member_name and field_offset:
                members.append({'name': member_name[1], 'type': field_type[1] if field_type else None,
                                'offset': int(field_offset[1], 0)})
        if members:
            result.append({'name': name[1], 'fields': members})
    return result


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--archive')
    parser.add_argument('--offset', type=int)
    parser.add_argument('--assembly')
    parser.add_argument('--pattern', default='GameClient|GameViewSharedConfig|ClientData$')
    parser.add_argument('--output', required=True)
    args = parser.parse_args()
    if args.archive:
        result = extract(args.archive, args.offset, args.output)
    else:
        result = structures(args.assembly, args.pattern)
        Path(args.output).write_text(json.dumps(result, indent=2), encoding='utf-8')
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
