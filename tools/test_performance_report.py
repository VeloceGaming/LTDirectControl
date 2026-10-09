import unittest
from summarize_performance import summarize


class PerformanceReportTests(unittest.TestCase):
    def test_records_preserve_versions_units_and_inclusive_scope_names(self):
        report = summarize("""1 INIT game=0.6.3 abi=6 probe=0.79.0
2 PERF window_s=10.0 scene=InGame frames=600 updates_per_s=60.0 frame_ms p50=16 p95=17 p99=100+ max=130.1 slow_25ms=3 | mod_total_ms=12
3 PERF WORK outline_incl_native count=18 avg_us=33 max_us=55 p95_us=<= 64 p99_us=<= 64 total_us=594; overlapping scopes
4 INIT game=0.6.3 abi=6 probe=0.79.1
5 PERF WORK capture_to_playback count=4 avg_us=30 max_us=50 p95_us=<= 64 p99_us=<= 64 total_us=120; overlapping scopes""")
        self.assertEqual(report["versions"], ["0.79.0", "0.79.1"])
        self.assertEqual(report["frame_windows"][0]["frame_ms"]["p99"], "100+")
        self.assertEqual(report["work_windows"][0]["scope"], "outline_incl_native")
        self.assertEqual(report["work_windows"][1]["version"], "0.79.1")
        self.assertEqual(report["work_windows"][0]["p95_us"], "<= 64")

    def test_missing_or_incomplete_records_do_not_invent_zero_cost(self):
        report = summarize("PERF SLOW frame_ms=99 mod_us=10\nPERF window_s=10 frames=4\n")
        self.assertFalse(report["work_windows"])
        self.assertFalse(report["frame_windows"])
        self.assertIn("No detailed timings", report["coverage"])


if __name__ == "__main__":
    unittest.main()
