"""Stage the mod folder for verification and packaging.

Copies the release DLL, mod.mod_info and the packaged UI/font assets into
dist/lt_direct_control (recreated from scratch, so nothing stale is kept).
Run after `cargo build --release` in probe/.
"""
import shutil
from pathlib import Path

root = Path(__file__).resolve().parents[1]
probe = root / 'probe'
package = root / 'dist/lt_direct_control'
if package.resolve() != root / 'dist/lt_direct_control':
    raise RuntimeError('Staging target resolves outside the intended workspace package folder.')
if package.exists():
    shutil.rmtree(package)
package.mkdir(parents=True)
shutil.copy2(probe / 'target/release/lt_direct_control.dll', package)
shutil.copy2(probe / 'mod.mod_info', package)
for folder in ['ui', 'font', 'sound']:
    shutil.copytree(probe / folder, package / folder)
count = sum(1 for p in package.rglob('*') if p.is_file())
print(f'Staged {count} files in {package.relative_to(root)}')
