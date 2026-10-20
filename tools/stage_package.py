"""Stage the mod folder for verification and packaging.

Copies the release DLL, mod.mod_info and the packaged UI/font assets into
dist/lt_direct_control (recreated from scratch, so nothing stale is kept).
Run after `cargo build --release` in probe/.
"""
import json
import shutil
from pathlib import Path

root = Path(__file__).resolve().parents[1]
probe = root / 'probe'
package = root / 'dist/lt_direct_control'
dll = probe / 'target/release/lt_direct_control.dll'
# Refuse a stale DLL (2026-10-20: a skipped `cargo build --release` shipped
# the 1.4.4 DLL as 1.4.5 and 1.4.6; tests and the verifier passed). The DLL
# must be newer than every source input and carry mod.mod_info's version.
version = json.loads((probe / 'mod.mod_info').read_text(encoding='utf-8-sig'))['version']
inputs = [probe / 'Cargo.toml', probe / 'Cargo.lock', *(probe / 'src').rglob('*.rs')]
newest = max(inputs, key=lambda p: p.stat().st_mtime)
if not dll.exists() or dll.stat().st_mtime < newest.stat().st_mtime:
    raise SystemExit(f'Stale DLL: {newest.relative_to(root)} is newer. Run `cargo build --release` in probe/ first.')
if version.encode() not in dll.read_bytes():
    raise SystemExit(f'The DLL does not carry version {version}. Run `cargo build --release` in probe/ first.')
if package.resolve() != root / 'dist/lt_direct_control':
    raise RuntimeError('Staging target resolves outside the intended workspace package folder.')
if package.exists():
    shutil.rmtree(package)
package.mkdir(parents=True)
shutil.copy2(dll, package)
shutil.copy2(probe / 'mod.mod_info', package)
for folder in ['ui', 'font', 'sound']:
    shutil.copytree(probe / folder, package / folder)
count = sum(1 for p in package.rglob('*') if p.is_file())
print(f'Staged {count} files in {package.relative_to(root)}')
