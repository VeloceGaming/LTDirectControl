"""The shipped preview table never loses a champion (tools/build_preview_table.py,
1.5.1-dev3): the union of recorded, assigned and shipped champions, the keep
rule, the needs-a-match report, --drop and --check."""
import contextlib
import io
import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import build_preview_table as tool
import preview_viewer as viewer

RULES = {'actions': {'hammerer_skill2': {'attack_range': {'format': 'distance'}}}}
HAMMERER_W = {'cast': {'casting': 1}}
HAMMERER_NUMBERS = {'kind': 'hammerer_skill2', 'values': {'attack_range': 60000},
                    'live': [{'kind': 'circle', 'radius': 60, 'at': 'aim'}]}
# An entry as the table shipped it before, from a recording since lost.
GLADIATOR = {'R': {'from': 'assigned', 'assigned': {'type': 'aim_circle', 'radius': {'name': 'radius', 'recorded': 35}},
                   'kind': 'gladiator_ult'}}


def recording(*champions):
    t = {'game': '0.6.5', 'champions': {}, 'numbers': {}}
    for champion in champions:
        t['champions'][champion] = {'Q': {}, 'W': HAMMERER_W, 'R': {}}
        t['numbers'][champion] = {'W': HAMMERER_NUMBERS}
    return t


def shipped(**champions):
    return {'about': 'x', 'game': '0.6.4', 'champions': champions}


class UnionAndKeep(unittest.TestCase):
    def test_recorded_champions_are_built_and_shipped_ones_without_a_recording_are_kept(self):
        out, report = tool.build(recording('hammerer'), RULES, {}, {},
                                 shipped=shipped(gladiator=GLADIATOR, hammerer={'Q': {'from': 'old'}}))
        self.assertEqual(set(out['champions']), {'hammerer', 'gladiator'})
        self.assertEqual(out['champions']['gladiator'], GLADIATOR)  # unchanged
        self.assertEqual(report['kept'], ['gladiator'])
        self.assertEqual(out['champions']['hammerer']['W']['from'], 'live')
        # Hammerer Q: the recording draws nothing there, so the shipped entry stays.
        self.assertEqual(out['champions']['hammerer']['Q'], {'from': 'old'})
        self.assertEqual(report['kept_skills'], ['hammerer Q'])

    def test_a_custom_preview_ships_its_pieces_as_written(self):
        pieces = [{'kind': 'shot', 'radius': {'name': 'attack_range', 'recorded': 60}, 'length': {'fixed': 30},
                   'pierce': True, 'at': 'caster'},
                  {'kind': 'dash', 'at': {'end': {'name': 'speed', 'by': 'tick', 'recorded': 84}}}]
        out, _ = tool.build(recording('hammerer'), RULES, {'hammerer W': {'type': 'custom', 'pieces': pieces}}, {})
        self.assertEqual(out['champions']['hammerer']['W'], {'from': 'custom', 'pieces': pieces,
                                                             'kind': 'hammerer_skill2'})

    def test_an_assigned_champion_with_neither_recording_nor_shipped_entry_needs_a_match(self):
        assigned = {'ferryman W': {'type': 'aim_circle', 'radius': 20}}
        out, report = tool.build(recording('hammerer'), RULES, assigned, {}, shipped=shipped())
        self.assertNotIn('ferryman', out['champions'])
        self.assertEqual(report['needs_match'], ['ferryman'])
        self.assertEqual(report['kept'], [])

    def test_a_card_from_a_data_file_is_not_a_recording(self):
        # The viewer's table can hold file cards; only `recorded` counts.
        table = recording('hammerer', 'gladiator')
        out, report = tool.build(table, RULES, {}, {}, shipped=shipped(gladiator=GLADIATOR),
                                 recorded={'hammerer'})
        self.assertEqual(out['champions']['gladiator'], GLADIATOR)
        self.assertEqual(report['kept'], ['gladiator'])

    def test_the_three_cases_without_a_recording(self):
        # Bombardier's pack enabled: a card from its data file, as
        # tools/preview_viewer.py `with_file_cards` makes it.
        viewer.FILE_CARDS.clear()
        viewer.FILE_CARDS['bombardier'] = {'W': {'cast': {'casting': 1, 'target': 6, 'range': 100.0, 'growth': 0.0},
                                                 'kind': 'bombardier_skill2', 'cast_range': 100.0,
                                                 'values': {'radius': 22000}}}
        rules = {'actions': {'bombardier_skill2': {'radius': {'format': 'distance'}}}}
        table = viewer.with_file_cards(recording('hammerer'), {'bombardier'})
        viewer.FILE_CARDS.clear()
        old_bombardier = {'Q': {'from': 'assigned', 'assigned': {'type': 'attack_reach', 'extra': {'fixed': 50}}},
                          'W': {'from': 'old'}}
        assigned = {'bombardier W': {'type': 'aim_circle', 'radius': 22},
                    'gladiator R': {'type': 'aim_circle', 'radius': 99},
                    'ferryman W': {'type': 'aim_circle', 'radius': 20}}
        out, report = tool.build(table, rules, assigned, {}, recorded={'hammerer'},
                                 shipped=shipped(bombardier=old_bombardier, gladiator=GLADIATOR))
        # 1. Card + assignment: built from the file's numbers, no match
        # needed; its other skills keep what shipped.
        self.assertEqual(out['champions']['bombardier']['W']['assigned'],
                         {'type': 'aim_circle', 'radius': {'name': 'radius', 'recorded': 22}})
        self.assertEqual(out['champions']['bombardier']['Q'], old_bombardier['Q'])
        self.assertEqual(report['from_files'], ['bombardier'])
        # 2. No card (pack disabled), shipped: unchanged, even with a newer
        # assignment (it cannot be built without numbers).
        self.assertEqual(out['champions']['gladiator'], GLADIATOR)
        self.assertEqual(report['kept'], ['gladiator'])
        # 3. No recording, no card, not shipped: needs a match, nothing shipped.
        self.assertNotIn('ferryman', out['champions'])
        self.assertEqual(report['needs_match'], ['ferryman'])

    def test_only_drop_removes_a_champion_or_a_skill(self):
        before = shipped(gladiator=GLADIATOR, hammerer={'Q': {'from': 'old'}})
        out, report = tool.build(recording('hammerer'), RULES, {}, {}, shipped=before,
                                 drop=['gladiator', 'hammerer Q'])
        self.assertEqual(set(out['champions']), {'hammerer'})
        self.assertEqual(set(out['champions']['hammerer']), {'W'})
        self.assertEqual(report['dropped'], ['gladiator', 'hammerer Q'])
        # A recorded champion can be dropped too.
        out, _ = tool.build(recording('hammerer'), RULES, {}, {}, shipped=before, drop=['hammerer'])
        self.assertEqual(set(out['champions']), {'gladiator'})

    def test_differences_name_the_champions_and_skills(self):
        old = shipped(archer={'Q': {'a': 1}}, gladiator=GLADIATOR, knight={'W': {}})
        new = shipped(archer={'Q': {'a': 1}, 'W': {'b': 2}}, gladiator=GLADIATOR, zook={'R': {}})
        new['game'] = '0.6.5'  # the game version alone is not a difference
        self.assertEqual(tool.differences(old, new),
                         ['archer: W added', 'knight: removed', 'zook: added (R)'])
        self.assertEqual(tool.differences(old, old), [])


class Command(unittest.TestCase):
    """The command itself on a fake mod folder and a fake shipped table."""

    def run_tool(self, folder, *args):
        out = io.StringIO()
        with mock.patch.object(viewer, 'TABLE', folder / 'skill-shapes.json'), \
                mock.patch.object(viewer, 'OVERRIDES', folder / 'skill-overrides.json'), \
                mock.patch.object(viewer, 'PICKS', folder / 'skill-sources.json'), \
                mock.patch.object(tool, 'OUTPUT', folder / 'preview_table.json'), \
                mock.patch.object(viewer, 'game_data', lambda: ({}, {}, {}, set(), 'en')), \
                mock.patch.dict(viewer.RULES, RULES, clear=True), \
                mock.patch.object(sys, 'argv', ['build_preview_table.py', *args]), \
                contextlib.redirect_stdout(out):
            try:
                tool.main()
                code = 0
            except SystemExit as stop:
                code = stop.code if isinstance(stop.code, int) else 1
                if not isinstance(stop.code, int):
                    out.write(str(stop.code))
        return code, out.getvalue()

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.folder = Path(self.temp.name)
        (self.folder / 'skill-shapes.json').write_text(json.dumps(recording('hammerer')), encoding='utf-8')
        (self.folder / 'preview_table.json').write_text(json.dumps(shipped(gladiator=GLADIATOR)), encoding='utf-8')

    def tearDown(self):
        self.temp.cleanup()

    def test_check_fails_naming_what_differs_then_passes_after_a_build(self):
        code, out = self.run_tool(self.folder, '--check')
        self.assertEqual(code, 1)
        self.assertIn('hammerer: added (W)', out)
        self.assertIn('kept from the shipped table (no recording): 1', out)
        self.assertEqual(json.loads((self.folder / 'preview_table.json').read_text(encoding='utf-8')),
                         shipped(gladiator=GLADIATOR))  # --check writes nothing
        code, out = self.run_tool(self.folder)
        self.assertEqual(code, 0)
        self.assertTrue(out.strip().splitlines()[-1].startswith('Preview table: 2 champions; kept from '
                                                                'the shipped table 1; needs a match 0; wrote'))
        code, out = self.run_tool(self.folder, '--check')
        self.assertEqual(code, 0)
        self.assertTrue(out.strip().endswith('up to date'))

    def test_an_unchanged_table_is_not_rewritten(self):
        self.run_tool(self.folder)
        path = self.folder / 'preview_table.json'
        before = path.stat().st_mtime_ns
        path.touch()
        stamp = path.stat().st_mtime_ns
        code, out = self.run_tool(self.folder)
        self.assertEqual(code, 0)
        self.assertIn('nothing written', out)
        self.assertEqual(path.stat().st_mtime_ns, stamp)
        self.assertGreaterEqual(stamp, before)

    def test_no_recording_at_all_keeps_the_whole_table(self):
        (self.folder / 'skill-shapes.json').unlink()
        code, out = self.run_tool(self.folder, '--check')
        self.assertEqual(code, 0, out)
        self.assertIn('kept from the shipped table (no recording): 1', out)

    def test_drop_of_a_champion_not_shipped_is_refused(self):
        code, out = self.run_tool(self.folder, '--drop', 'nobody')
        self.assertEqual(code, 1)
        self.assertIn('nothing dropped: nobody', out)
        code, out = self.run_tool(self.folder, '--drop', 'gladiator')
        self.assertEqual(code, 0)
        self.assertNotIn('gladiator', json.loads((self.folder / 'preview_table.json').read_text(encoding='utf-8'))['champions'])


class ViewerSave(unittest.TestCase):
    """The viewer's Save runs the build and shows its result; a failed
    build never loses the save."""

    def test_a_failed_build_keeps_the_save_and_says_so(self):
        with tempfile.TemporaryDirectory() as temp:
            folder = Path(temp)
            failed = mock.Mock(returncode=1, stdout='', stderr='Could not read the balance patch rules from the game bundle.')
            with mock.patch.object(viewer, 'MOD_FOLDER', folder), \
                    mock.patch.object(viewer, 'OVERRIDES', folder / 'skill-overrides.json'), \
                    mock.patch('subprocess.run', return_value=failed):
                result = viewer.save({'skills': {'archer W': {'type': 'jump_back', 'distance': 30}}})
            self.assertTrue(result['ok'])
            self.assertEqual(result['table'], {'ok': False, 'line': 'Preview table not rebuilt (errors): '
                                                                    'Could not read the balance patch rules from the game bundle.'})
            saved = json.loads((folder / 'skill-overrides.json').read_text(encoding='utf-8'))
            self.assertEqual(saved['skills'], {'archer W': {'type': 'jump_back', 'distance': 30}})
            with mock.patch('subprocess.run', side_effect=OSError('no python')):
                self.assertEqual(viewer.rebuild_table(), {'ok': False, 'line': 'Preview table not rebuilt: no python'})

    def test_the_result_line_is_the_tools_last_summary(self):
        done = mock.Mock(returncode=0, stderr='', stdout='100 champions\n\nPreview table: 100 champions; kept from '
                                                         'the shipped table 4; needs a match 0; up to date\n')
        with mock.patch('subprocess.run', return_value=done):
            self.assertEqual(viewer.rebuild_table()['line'],
                             'Preview table: 100 champions; kept from the shipped table 4; needs a match 0; up to date')


if __name__ == '__main__':
    unittest.main()
