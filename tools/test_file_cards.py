"""Skill cards made from champion data files (tools/preview_viewer.py
`file_card`, `with_file_cards`) for champions no match has recorded."""
import unittest

import preview_sources
import preview_viewer as viewer

# Bombardier as the 0.7.0 Beta 2 bundle declares it (shortened).
BOMBARDIER = {
    'id': 'bombardier',
    'skill': {'casting_type': 'Targeting', 'casting_target': 'AllyOnlySelf', 'range': 0,
              'effect': {'type': 'Native', 'move_distance': 50000}},
    'skill2': {'patch_type_name': 'bombardier_skill2', 'casting_type': 'Position',
               'casting_target': 'EnemyWithoutTower', 'range': 100000,
               'effect': {'type': 'Native', 'radius': 22000, 'damage': 100}},
    'ult': {'casting_type': 'Position', 'casting_target': 'EnemyChampionInCC', 'range': 120000,
            'growth_range': 5000, 'effect': {'type': 'Native', 'radius': 35000}},
}
RULES = {'actions': {'bombardier_skill2': {'radius': {'format': 'distance'},
                                           'range': {'format': 'distance'},
                                           'damage': {'format': 'default'}}}}


class FileCards(unittest.TestCase):
    def setUp(self):
        viewer.FILE_CARDS.clear()

    def test_a_card_holds_how_each_skill_is_cast_its_range_and_numbers(self):
        card = viewer.file_card(BOMBARDIER)
        self.assertEqual(card['Q']['cast'], {'casting': 0, 'target': 4, 'range': 0.0, 'growth': 0.0})
        self.assertEqual(card['W']['cast'], {'casting': 1, 'target': 6, 'range': 100.0, 'growth': 0.0})
        # The game's names in other capitals, and range growth per level.
        self.assertEqual(card['R']['cast'], {'casting': 1, 'target': 8, 'range': 120.0, 'growth': 5.0})
        self.assertEqual(card['W']['kind'], 'bombardier_skill2')
        self.assertEqual(card['W']['values']['radius'], 22000)

    def test_a_name_held_twice_with_different_values_is_not_a_number(self):
        # Alchemist Q's file: two `radius` (9000 and 32000); the game answers one.
        data = {'skill': {'casting_type': 'Position', 'casting_target': 'Enemy', 'range': 70000,
                          'effect': {'type': 'Combine', 'effects': [{'radius': 9000, 'damage': 8},
                                                                    {'radius': 32000, 'damage': 8}]}}}
        values = viewer.file_card(data)['Q']['values']
        self.assertNotIn('radius', values)
        self.assertEqual(values['damage'], 8)
        # An unknown cast kind gives no card for that skill.
        self.assertEqual(viewer.file_card({'skill': {'casting_type': 'Sideways', 'casting_target': 'Enemy'}}), {})

    def test_unrecorded_champions_get_file_cards_and_a_recording_always_wins(self):
        viewer.FILE_CARDS['bombardier'] = viewer.file_card(BOMBARDIER)
        viewer.FILE_CARDS['harpooner'] = viewer.file_card(BOMBARDIER)
        recorded = {'champions': {'harpooner': {'Q': {'cast': {'casting': 2, 'target': 7}}}},
                    'numbers': {'harpooner': {}}}
        table = viewer.with_file_cards(recorded, {'bombardier', 'harpooner', 'no_file_champion'})
        self.assertEqual(table['file_cards'], ['bombardier'])
        self.assertEqual(table['champions']['harpooner'], recorded['champions']['harpooner'])
        self.assertEqual(table['champions']['bombardier']['W']['source'], 'files')
        self.assertNotIn('no_file_champion', table['champions'])
        self.assertNotIn('bombardier', recorded['champions'])  # the recording itself is untouched

    def test_an_assignment_on_a_file_card_takes_its_size_from_the_number_by_name(self):
        viewer.FILE_CARDS['bombardier'] = viewer.file_card(BOMBARDIER)
        table = viewer.with_file_cards({'champions': {}, 'numbers': {}}, {'bombardier'})
        assigned = {'bombardier W': {'type': 'aim_circle', 'radius': 22}}
        entry = preview_sources.skill('bombardier', 'W', table, RULES, assigned, {})
        self.assertEqual(entry['from'], 'assigned')
        self.assertEqual([s['source'] for s in entry['sizes']], [{'name': 'radius', 'recorded': 22}])

    def test_the_attack_range_ring_takes_its_extra_from_the_number_by_name(self):
        viewer.FILE_CARDS['bombardier'] = viewer.file_card(BOMBARDIER)
        table = viewer.with_file_cards({'champions': {}, 'numbers': {}}, {'bombardier'})
        rules = {'actions': {None: {}}}  # Q's file has no patch type name: common size names
        assigned = {'bombardier Q': {'type': 'attack_reach', 'extra': 50}}
        entry = preview_sources.skill('bombardier', 'Q', table, rules, assigned, {})
        self.assertEqual([s['source'] for s in entry['sizes']], [{'name': 'move_distance', 'recorded': 50}])
        self.assertEqual(viewer.REQUIRED['attack_reach'], ['extra'])


if __name__ == '__main__':
    unittest.main()
