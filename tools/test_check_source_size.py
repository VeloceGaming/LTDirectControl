import tempfile
import unittest
from pathlib import Path

from check_source_size import CEILINGS, LIMIT, SOURCE, oversized


def write(folder, name, lines):
    path = folder / name
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text('\n'.join('x' for _ in range(lines)) + '\n', encoding='utf-8')


class CheckSourceSize(unittest.TestCase):
    def test_files_over_the_limit_or_their_own_ceiling_are_listed(self):
        with tempfile.TemporaryDirectory() as temp:
            folder = Path(temp)
            write(folder, 'small.rs', 10)
            write(folder, 'at_limit.rs', 100)
            write(folder, 'over.rs', 101)
            write(folder, 'nested/over_too.rs', 150)
            write(folder, 'allowed.rs', 180)
            write(folder, 'allowed_but_grown.rs', 201)
            write(folder, 'ignored.txt', 500)
            ceilings = {'allowed.rs': 200, 'allowed_but_grown.rs': 200}
            self.assertEqual(
                oversized(folder, limit=100, ceilings=ceilings),
                [('allowed_but_grown.rs', 201, 200), ('nested/over_too.rs', 150, 100), ('over.rs', 101, 100)],
            )

    def test_every_ceiling_names_a_real_file_above_the_limit(self):
        for relative, ceiling in CEILINGS.items():
            self.assertTrue((SOURCE / relative).is_file(), relative)
            self.assertGreater(ceiling, LIMIT, relative)

    def test_the_mod_passes_its_own_check(self):
        self.assertEqual(oversized(), [])


if __name__ == '__main__':
    unittest.main()
