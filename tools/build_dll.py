"""Build the mod's DLL without the build machine's folders in it (1.5.2).

    python tools/build_dll.py               # cargo build --release --offline
    python tools/build_dll.py --research    # the research build

Rust keeps the source paths of its dependencies for its error messages, so a
plain build carries the builder's user folder (with the account name) and
the project folder. This runs the same `cargo build` with each of those
folders rewritten to a neutral name (`--remap-path-prefix`), worked out on
the machine that builds, so no personal path is written into the repository
either. tools/verify_build.py refuses a DLL that still holds one.
"""
import argparse
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def remaps():
    """(folder, neutral name) pairs, most general first: rustc applies the
    last one that matches."""
    home = Path.home()
    return [
        (home, 'home'),
        (Path(os.environ.get('CARGO_HOME') or home / '.cargo'), 'cargo'),
        (Path(os.environ.get('RUSTUP_HOME') or home / '.rustup'), 'rustup'),
        (ROOT, 'ltdc'),
    ]


def flags():
    """The compiler flags, one per entry: CARGO_ENCODED_RUSTFLAGS keeps
    folders with spaces whole."""
    return [f'--remap-path-prefix={folder}={name}' for folder, name in remaps()]


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--research', action='store_true', help='build with the research feature')
    args = parser.parse_args()
    env = dict(os.environ)
    env.pop('RUSTFLAGS', None)
    env['CARGO_ENCODED_RUSTFLAGS'] = '\x1f'.join(flags())
    command = ['cargo', 'build', '--release', '--offline'] + (['--features', 'research'] if args.research else [])
    sys.exit(subprocess.run(command, cwd=ROOT / 'probe', env=env).returncode)


if __name__ == '__main__':
    main()
