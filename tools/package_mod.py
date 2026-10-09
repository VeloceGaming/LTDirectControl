"""Package verified staged files, then compare every zip member with its source."""
import argparse
import hashlib
import json
from pathlib import Path
from zipfile import ZipFile, ZIP_DEFLATED

parser = argparse.ArgumentParser()
parser.add_argument('--version', required=True)
args = parser.parse_args()
root = Path(__file__).resolve().parents[1]
package = root / 'dist/lt_direct_control'
record_path = root / f'dist/build-{args.version}.json'
record = json.loads(record_path.read_text(encoding='utf-8'))
assert json.loads((package / 'mod.mod_info').read_text())['version'] == args.version
files = sorted(p for p in package.rglob('*') if p.is_file())
for name, key in [('lt_direct_control.dll', 'dll_sha256'), ('mod.mod_info', 'metadata_sha256')]:
    assert hashlib.sha256((package / name).read_bytes()).hexdigest() == record[key]
graphics = json.loads((root / 'tools/records/ui-graphics.json').read_text())
assert {p.relative_to(package).as_posix() for p in files} == set(graphics['files']) | {'mod.mod_info','lt_direct_control.dll'}
for name, digest in graphics['files'].items():
    assert hashlib.sha256((package / name).read_bytes()).hexdigest() == digest
archive = root / f'dist/LTDirectControl-{args.version}.zip'
with ZipFile(archive, 'w', ZIP_DEFLATED) as z:
    for p in files:
        z.write(p, p.relative_to(package.parent).as_posix())
with ZipFile(archive) as z:
    assert z.testzip() is None
    assert len(z.namelist()) == len(files)
    for p in files:
        assert z.read(p.relative_to(package.parent).as_posix()) == p.read_bytes()
record['zip_sha256'] = hashlib.sha256(archive.read_bytes()).hexdigest()
record['zip_members'] = len(files)
record_path.write_text(json.dumps(record, indent=2) + '\n', encoding='utf-8')
print(f'Verified {archive.name}: {len(files)} matching files; SHA256 {record["zip_sha256"]}')
