"""Custom previews built from pieces (1.5.2-dev2): what the viewer saves
(tools/preview_viewer.py `validate`) and the names a skill holds twice."""
import unittest

import preview_viewer as viewer

BULLDOZER_Q = {'type': 'custom', 'pieces': [
    {'kind': 'shot', 'radius': {'name': 'width', 'recorded': 60}, 'length': {'fixed': 30},
     'pierce': True, 'at': 'caster'},
    {'kind': 'strip', 'radius': {'fixed': 10}, 'length': {'fixed': 84}, 'at': 'caster'},
    {'kind': 'dash', 'at': {'end': {'fixed': 84}}},
]}


class CustomPreviews(unittest.TestCase):
    def test_a_complete_custom_preview_is_saved_as_written(self):
        good, problems = viewer.validate({'bulldozer Q': BULLDOZER_Q})
        self.assertEqual(problems, [])
        self.assertEqual(good['bulldozer Q'], BULLDOZER_Q)

    def test_every_source_kind_is_accepted(self):
        sources = [{'fixed': 5}, {'cast_range': True}, {'cast_range': True, 'recorded': 70},
                   {'name': 'width', 'recorded': 60}, {'name': 'width', 'times': 0.5, 'recorded': 30},
                   {'name': 'speed', 'by': 'tick', 'recorded': 84}]
        for source in sources:
            entry = {'type': 'custom', 'pieces': [{'kind': 'circle', 'radius': source, 'at': 'aim'}]}
            self.assertEqual(viewer.validate({'x Q': entry})[1], [], source)

    def test_what_the_mod_cannot_draw_is_refused_with_a_reason(self):
        cases = {
            'piece 1: unknown block segment': [{'kind': 'segment', 'at': 'caster'}],
            'piece 1: radius missing': [{'kind': 'circle', 'at': 'aim'}],
            'piece 1: radius is not a size': [{'kind': 'circle', 'radius': {'fixed': -1}, 'at': 'aim'}],
            'piece 1: where it starts is unknown': [{'kind': 'circle', 'radius': {'fixed': 5}, 'at': 'behind'}],
            'piece 1: cosine is not a size': [{'kind': 'cone', 'radius': {'fixed': 5}, 'cosine': {'fixed': 2},
                                               'at': 'caster'}],
            # A number by name needs the value the mod uses until it reads it.
            'piece 1: radius is not a size ': [{'kind': 'circle', 'radius': {'name': 'width'}, 'at': 'aim'}],
            'at most 8 pieces': [{'kind': 'dash', 'at': 'aim'}] * 9,
        }
        for reason, pieces in cases.items():
            good, problems = viewer.validate({'x Q': {'type': 'custom', 'pieces': pieces}})
            self.assertEqual(good, {})
            self.assertEqual(problems, [f'x Q: {reason.strip()}'])

    def test_unknown_keys_and_flags_are_dropped(self):
        entry = {'type': 'custom', 'extra': 1, 'pieces': [
            {'kind': 'circle', 'radius': {'fixed': 5}, 'at': 'aim', 'allies': True, 'blink': True, 'colour': 'red'},
            {'kind': 'strip', 'radius': {'fixed': 35}, 'length': {'fixed': 35}, 'at': 'caster', 'rounded': True}]}
        good, _ = viewer.validate({'x Q': entry})
        self.assertEqual(good['x Q'], {'type': 'custom', 'pieces': [
            {'kind': 'circle', 'radius': {'fixed': 5}, 'at': 'aim', 'allies': True},
            {'kind': 'strip', 'radius': {'fixed': 35}, 'length': {'fixed': 35}, 'at': 'caster', 'rounded': True}]})

    def test_names_held_twice_in_a_skill_are_noted(self):
        viewer.DUPLICATES.clear()
        skill = {'effect': {'effects': [{'speed': 7, 'tick': 1}, {'speed': 4200, 'tick': 20, 'width': 60000}]}}
        viewer.note_duplicates('bulldozer', 'Q', skill, replace=False)
        self.assertEqual(viewer.DUPLICATES['bulldozer']['Q'], {'speed': [7, 4200], 'tick': [1, 20]})
        # The base game's sheet wins over a mod file for the same skill.
        viewer.note_duplicates('bulldozer', 'Q', {'speed': 1}, replace=False)
        self.assertEqual(viewer.DUPLICATES['bulldozer']['Q'], {'speed': [7, 4200], 'tick': [1, 20]})
        viewer.DUPLICATES.clear()

    def test_unrecorded_skills_take_what_the_mods_file_reader_found(self):
        import json, tempfile
        from pathlib import Path
        from unittest import mock
        report = {'champions': {
            'test_mod_jhin': {'Q': {'source': 'file', 'shapes': [{'kind': 'circle', 'radius': 35.0, 'at': 'aim'}]},
                              'W': {'source': 'file', 'shapes': [], 'no_area': True},
                              'R': {'source': 'file', 'unread': 'opaque Native'}},
            'archer': {'Q': {'source': 'file', 'shapes': [{'kind': 'circle', 'radius': 9.0, 'at': 'aim'}]}}}}
        table = {'champions': {
            'test_mod_jhin': {'Q': {'cast': {'casting': 0}, 'source': 'files'}, 'W': {'source': 'files'},
                              'R': {'source': 'files'}},
            'archer': {'Q': {'shapes': [], 'file': {'shapes': [{'kind': 'circle', 'radius': 7.0, 'at': 'aim'}]}}}}}
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / 'file-shapes.json'
            path.write_text(json.dumps(report), encoding='utf-8')
            with mock.patch.object(viewer, 'FILE_SHAPES', path):
                out = viewer.with_file_shapes(table)
        jhin = out['champions']['test_mod_jhin']
        self.assertEqual(jhin['Q']['file']['shapes'], [{'kind': 'circle', 'radius': 35.0, 'at': 'aim'}])
        self.assertTrue(jhin['W']['file']['no_area'])
        self.assertNotIn('file', jhin['R'])  # unreadable: nothing to show
        # A recording keeps its own: the shipped table is built from it.
        self.assertEqual(out['champions']['archer']['Q']['file']['shapes'][0]['radius'], 7.0)

    def test_no_report_changes_nothing(self):
        from pathlib import Path
        from unittest import mock
        table = {'champions': {'x': {'Q': {'source': 'files'}}}}
        with mock.patch.object(viewer, 'FILE_SHAPES', Path('does-not-exist.json')):
            self.assertEqual(viewer.with_file_shapes(table), {'champions': {'x': {'Q': {'source': 'files'}}}})


if __name__ == '__main__':
    unittest.main()
