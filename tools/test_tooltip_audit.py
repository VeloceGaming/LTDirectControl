import json
import unittest

from audit_tooltips import observations, report


class TooltipAuditTest(unittest.TestCase):
    def inventory(self):
        return {'source_sha256': 'fixture', 'champion_count': 2,
                'skills': [{'champion': 'illusionist', 'slot': 0},
                           {'champion': 'alchemist', 'slot': 0}]}

    def test_legacy_success_never_counts_unobserved_skills_as_fixed(self):
        value = report(self.inventory(), ['1 INIT game=0.6.3 probe=0.69.0\n'
            '2 NATIVE TOOLTIP champion=illusionist slot=0 bytes=250 unresolved=0 source=game'])
        self.assertEqual(value['summary']['native-resolved'], 1)
        self.assertEqual(value['summary']['unobserved'], 1)
        self.assertEqual(value['skills'][0]['observation']['version'], '0.69.0')

    def test_structured_parameters_win_over_the_companion_summary_and_keep_punctuation_separate(self):
        row = {'champion': 'illusionist', 'slot': 0, 'source': 'game', 'version': '0.69.1',
               'unresolved_parameters': ['Coef'], 'ellipsis_count': 2}
        logs = ['1 NATIVE TOOLTIP_AUDIT ' + json.dumps(row) + '\n'
                '2 NATIVE TOOLTIP champion=illusionist slot=0 bytes=250 unresolved=1 source=game']
        value = report(self.inventory(), logs)
        self.assertEqual(value['summary']['native-unresolved'], 1)
        observation = value['skills'][0]['observation']
        self.assertEqual(observation['unresolved_parameters'], ['Coef'])
        self.assertEqual(observation['ellipsis_count'], 2)

    def test_latest_failure_and_nonbase_mod_results_remain_visible(self):
        lines = ['4 NATIVE TOOLTIP fallback champion=illusionist slot=0 reason=unreviewed table',
                 '2 NATIVE TOOLTIP champion=illusionist slot=0 bytes=40 unresolved=0 source=game',
                 '3 NATIVE TOOLTIP champion=workshop_mod slot=2 bytes=80 unresolved=0 source=game']
        value = report(self.inventory(), lines)
        self.assertEqual(value['summary']['fallback'], 1)
        self.assertEqual(value['summary']['native-resolved'], 0)
        self.assertEqual(value['summary']['nonbase_observations'], 1)
        self.assertEqual(value['nonbase_observations'][0]['champion'], 'workshop_mod')

    def test_malformed_and_partial_records_do_not_invent_coverage(self):
        bad = [{'champion': 'x', 'slot': 9, 'source': 'game', 'unresolved_parameters': []},
               {'champion': 'x', 'slot': 0, 'source': 'game', 'unresolved_parameters': 'Coef'},
               {'champion': 'x', 'slot': 0, 'source': 'mystery', 'unresolved_parameters': []},
               None, []]
        logs = ['1 NATIVE TOOLTIP_AUDIT ' + json.dumps(row) for row in bad]
        logs.append('2 NATIVE TOOLTIP_AUDIT {truncated')
        self.assertEqual(observations(logs), {})


if __name__ == '__main__':
    unittest.main()
