#!/usr/bin/env python3
import unittest

import summarize_short_validation as summarize


def summary(cpp_elapsed: str | None = None, rust_elapsed: str | None = None):
    entry = {}
    if cpp_elapsed is not None:
        entry["cpp_time"] = {
            "elapsed": cpp_elapsed,
            "user_sec": "1.00",
            "sys_sec": "0.10",
            "cpu_pct": "90%",
            "max_rss_kib": "1000",
        }
    if rust_elapsed is not None:
        entry["rust_time"] = {
            "elapsed": rust_elapsed,
            "user_sec": "1.00",
            "sys_sec": "0.10",
            "cpu_pct": "90%",
            "max_rss_kib": "1000",
        }
    return {"example1": entry}


class TestP17SummarizeShortValidation(unittest.TestCase):
    def test_compare_without_baseline_marks_stability_unknown(self):
        result = summarize.compare(None, summary(cpp_elapsed="0:02.00"))

        self.assertFalse(result["benchmark_stability"]["baseline_available"])
        self.assertFalse(result["benchmark_stability"]["control_drift_checked"])
        self.assertIsNone(result["benchmark_stability"]["unstable"])

    def test_cpp_control_drift_under_threshold_is_stable(self):
        result = summarize.compare(
            summary(cpp_elapsed="0:02.00", rust_elapsed="0:01.00"),
            summary(cpp_elapsed="0:02.10", rust_elapsed="0:00.90"),
        )

        stability = result["benchmark_stability"]
        self.assertTrue(stability["control_drift_checked"])
        self.assertFalse(stability["unstable"])
        self.assertEqual(stability["max_abs_control_wall_pct"], 5.0)

    def test_cpp_control_drift_over_threshold_is_unstable(self):
        result = summarize.compare(
            summary(cpp_elapsed="0:02.00", rust_elapsed="0:01.00"),
            summary(cpp_elapsed="0:02.50", rust_elapsed="0:00.90"),
        )

        stability = result["benchmark_stability"]
        self.assertTrue(stability["control_drift_checked"])
        self.assertTrue(stability["unstable"])
        self.assertEqual(stability["max_abs_control_wall_pct"], 25.0)


if __name__ == "__main__":
    unittest.main()
