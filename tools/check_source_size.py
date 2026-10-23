"""Refuse a build that grows the mod's biggest source files.

A brief code review on 2026-10-23 found five files under probe/src over
3,000 lines, with 200 to 320 line functions at the movement and input
plug-in points. The author declined a refactor on the condition that the
files stop growing (docs/backlog.md, Declined; docs/modding.md, "Keep
files small"): a new feature goes in its own module, an existing file
takes only the lines that wire it in. This check makes that rule hold
across tools and sessions.

    python tools/check_source_size.py

lists every `.rs` file under probe/src over its limit and exits 1 if
there is one. tools/verify_build.py runs it. When a file is split, lower
its ceiling in CEILINGS or delete the exception.
"""
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / 'probe' / 'src'

# Any .rs file under probe/src, counted with its tests.
LIMIT = 3000

# Files already over LIMIT on 2026-10-23, at their size then plus a little
# room for wiring. Paths are relative to probe/src. They may only shrink.
CEILINGS = {
    'movement.rs': 4150,
    'skill_preview.rs': 3700,
    'shop_ui.rs': 3550,
    'player_hud.rs': 3200,
    'abilities.rs': 3200,
}


def line_count(path):
    return len(path.read_text(encoding='utf-8').splitlines())


def oversized(source=SOURCE, limit=LIMIT, ceilings=CEILINGS):
    """(path relative to `source`, lines, ceiling) for every .rs file over its ceiling."""
    found = []
    for path in sorted(source.rglob('*.rs')):
        relative = path.relative_to(source).as_posix()
        ceiling = ceilings.get(relative, limit)
        lines = line_count(path)
        if lines > ceiling:
            found.append((relative, lines, ceiling))
    return found


def main():
    over = oversized()
    for relative, lines, ceiling in over:
        print(f'{relative}: {lines} lines, limit {ceiling}')
    if over:
        print('Source size check failed: move the new code into its own module or split the file '
              '(docs/modding.md, "Keep files small").')
        return 1
    print(f'Source size check passed: no file under probe/src over {LIMIT} lines '
          f'({len(CEILINGS)} files with their own ceiling in tools/check_source_size.py).')
    return 0


if __name__ == '__main__':
    sys.exit(main())
