import unittest

from preview_sources import all_skills, landed, skill, widened, with_sources
from build_preview_table import build

RULES = {'actions': {
    'hammerer_skill2': {'attack': {'format': 'default'}, 'attack_range': {'format': 'distance'},
                        'slow_speed': {'format': 'default'}},
    'berserker_skill2': {'range': {'format': 'distance'}, 'attack_range': {'format': 'distance'}},
    'data_action': {'range': {'format': 'distance'}, 'speed': {'format': 'move_speed'}},
}}


def table(champion, slot, recorded, numbers=None):
    t = {'game': '0.6.4', 'champions': {champion: {'Q': {}, 'W': {}, 'R': {}}}, 'numbers': {}}
    t['champions'][champion][slot] = recorded
    if numbers is not None:
        t['numbers'][champion] = {slot: numbers}
    return t


class PreviewSourcesTest(unittest.TestCase):
    def test_a_size_is_the_one_distance_number_with_its_value(self):
        # Hammerer W: radius 62 is attack_range (patched 60000 x 1.033); the
        # forward offset 12 is not a number the game answers by name.
        t = table('hammerer', 'W', {'cast': {'casting': 1, 'range': 12}},
                  {'kind': 'hammerer_skill2', 'values': {'attack': 50, 'attack_range': 62000, 'slow_speed': 50},
                   'cast_range': 12, 'live': [{'kind': 'circle', 'radius': 62, 'at': {'forward': 12}}]})
        s = skill('hammerer', 'W', t, RULES, {}, {})
        self.assertEqual(s['from'], 'live')
        sizes = {x['key']: x for x in s['sizes']}
        self.assertEqual(sizes['0.radius']['source'], {'name': 'attack_range', 'recorded': 62})
        self.assertEqual(sizes['0.radius']['status'], 'named')
        # 12 equals the cast range, but a forward offset is not a length.
        self.assertEqual(sizes['0.at.forward']['source'], {'fixed': 12})

    def test_numbers_that_are_not_distances_never_pass_for_sizes(self):
        # slow_speed 50 is not a distance, even if a size were 0.05 or 50.
        t = table('hammerer', 'W', {}, {'kind': 'hammerer_skill2', 'values': {'slow_speed': 50000},
                                        'live': [{'kind': 'circle', 'radius': 50, 'at': 'aim'}]})
        self.assertEqual(skill('hammerer', 'W', t, RULES, {}, {})['sizes'][0]['status'], 'fixed')

    def test_two_numbers_with_the_same_value_wait_for_the_authors_pick(self):
        pair = {'kind': 'berserker_skill2', 'values': {'range': 15000, 'attack_range': 15000},
                'cast_range': 30, 'live': [{'kind': 'circle', 'radius': 15, 'at': 'caster'}]}
        t = table('berserker', 'W', {}, pair)
        size = skill('berserker', 'W', t, RULES, {}, {})['sizes'][0]
        self.assertEqual(size['status'], 'pick')
        self.assertEqual(size['source'], {'fixed': 15})
        self.assertEqual([c['name'] for c in size['candidates']], ['attack_range', 'range'])
        picks = {'berserker W': {'0.radius': {'name': 'attack_range'}}}
        size = skill('berserker', 'W', t, RULES, {}, picks)['sizes'][0]
        self.assertEqual((size['status'], size['source']), ('picked', {'name': 'attack_range', 'recorded': 15}))

    def test_a_length_equal_to_the_cast_range_follows_the_cast_range(self):
        # Knight R: range 60 is the area, the cast range 80 is separate.
        pair = {'kind': 'data_action', 'values': {'range': 65000, 'projectile_radius': 10000},
                'cast_range': 65, 'live': [{'kind': 'strip', 'radius': 10, 'length': 65, 'at': 'caster'}]}
        sizes = {x['key']: x for x in skill('cf_sun_mage', 'Q', table('cf_sun_mage', 'Q', {}, pair),
                                             RULES, {}, {})['sizes']}
        self.assertEqual(sizes['0.length']['source'], {'cast_range': True, 'recorded': 65})
        self.assertEqual(sizes['0.radius']['source'], {'name': 'projectile_radius', 'recorded': 10})

    def test_assignments_and_skills_without_numbers(self):
        pair = {'kind': 'data_action', 'values': {'radius': 32000}, 'cast_range': 25}
        assignments = {'zook W': {'type': 'jump_back', 'distance': 26, 'add': True}}
        recorded = {'shapes': [{'kind': 'circle', 'radius': 32, 'at': 'caster'}]}
        s = skill('zook', 'W', table('zook', 'W', recorded, pair), RULES, assignments, {})
        self.assertEqual(s['from'], 'assigned')
        sizes = {x['key']: x['source'] for x in s['sizes']}
        self.assertEqual(sizes['assigned.distance'], {'fixed': 26})
        self.assertEqual(sizes['0.radius'], {'name': 'radius', 'recorded': 32})
        s = skill('zook', 'W', table('zook', 'W', recorded), RULES, {}, {})
        self.assertEqual(s['sizes'][0]['status'], 'no_numbers')

    def test_circles_that_go_with_a_move_to_the_aim_are_where_the_caster_lands(self):
        pole = [{'kind': 'dash', 'at': 'aim'}, {'kind': 'circle', 'radius': 32, 'at': 'caster'}]
        self.assertEqual(landed(pole)[1]['at'], 'aim')
        self.assertEqual(pole[1]['at'], 'caster')  # the recording itself is left alone
        line = [{'kind': 'dash', 'at': 'aim'}, {'kind': 'strip', 'radius': 5, 'length': 50, 'at': 'caster'}]
        self.assertEqual(landed(line), line)
        alone = [{'kind': 'circle', 'radius': 32, 'at': 'caster'}]
        self.assertEqual(landed(alone), alone)

    def test_the_shipped_table_replaces_sizes_with_their_sources(self):
        shape = {'kind': 'strip', 'radius': 10, 'length': 65, 'at': {'end': 65}}
        sizes = [{'key': '0.radius', 'source': {'name': 'r'}}, {'key': '0.at.end', 'source': {'cast_range': True}}]
        self.assertEqual(with_sources(shape, sizes, '0.'),
                         {'kind': 'strip', 'radius': {'name': 'r'}, 'length': 65, 'at': {'end': {'cast_range': True}}})
        self.assertEqual(shape['at'], {'end': 65})
        t = table('hammerer', 'W', {'cast': {'casting': 1}},
                  {'kind': 'hammerer_skill2', 'values': {'attack_range': 60000},
                   'live': [{'kind': 'circle', 'radius': 60, 'at': 'aim'}]})
        t['champions']['hammerer']['Q'] = {'cast': {'casting': 4294967295}}
        shipped, report = build(t, RULES, {}, {})
        self.assertEqual(shipped['champions']['hammerer'],
                         {'W': {'from': 'live', 'kind': 'hammerer_skill2',
                                'shapes': [{'kind': 'circle', 'radius': {'name': 'attack_range', 'recorded': 60},
                                            'at': 'aim'}]}})
        self.assertEqual(report['sizes'], {'named': 1})
        self.assertEqual(set(all_skills(t, RULES, {}, {})['hammerer']), {'Q', 'W', 'R'})


    def test_line_hits_recorded_before_the_fix_get_their_full_width(self):
        # Bulldozer Q as 1.5.1 recorded it: `width` 60 halved to radius 30.
        old = [{'kind': 'shot', 'radius': 30.0, 'length': 30.0, 'pierce': True, 'at': 'caster'},
               {'kind': 'strip', 'radius': 10.0, 'length': 84.0, 'at': 'caster'}]
        fixed = widened(old, 'bulldozer')
        self.assertEqual(fixed[0]['radius'], 60.0)
        self.assertEqual(fixed[1], old[1])  # the charge is another rule
        # Marked: a second pass, or a shot recorded since, stays as it is.
        self.assertEqual(widened(fixed, 'bulldozer'), fixed)
        new = [{'kind': 'shot', 'radius': 60.0, 'length': 30.0, 'pierce': True, 'hit_radius': True}]
        self.assertEqual(widened(new, 'bulldozer'), new)
        # The four champion-skill champions always recorded the hit radius.
        self.assertEqual(widened(old, 'hypnotist'), old)

    def test_a_pick_that_no_longer_fits_gives_way_to_the_number_that_does(self):
        # The author picked `width x 0.5` for the halved radius; the true
        # radius is the whole `width`.
        t = table('bulldozer', 'Q', {'cast': {'casting': 2, 'range': 72}},
                  {'kind': 'data_action', 'values': {'width': 60000, 'length': 30000},
                   'cast_range': 72,
                   'live': [{'kind': 'shot', 'radius': 30.0, 'length': 30.0, 'pierce': True, 'at': 'caster'}]})
        picks = {'bulldozer Q': {'0.radius': {'name': 'width', 'times': 0.5}}}
        sizes = {x['key']: x for x in skill('bulldozer', 'Q', t, RULES, {}, picks)['sizes']}
        self.assertEqual(sizes['0.radius']['value'], 60.0)
        self.assertEqual(sizes['0.radius']['source'], {'name': 'width', 'recorded': 60.0})
        self.assertEqual(sizes['0.radius']['status'], 'named')
        # A pick that still fits holds.
        picks = {'bulldozer Q': {'0.radius': {'name': 'width'}}}
        sizes = {x['key']: x for x in skill('bulldozer', 'Q', t, RULES, {}, picks)['sizes']}
        self.assertEqual(sizes['0.radius']['status'], 'picked')


if __name__ == '__main__':
    unittest.main()
