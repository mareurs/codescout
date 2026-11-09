"""Tests for scripts/measure_buffer_misuse.py (bug 7579b32b1cd2362f, pilot step 2 check)."""

from __future__ import annotations

import importlib.util
import sqlite3
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts/measure_buffer_misuse.py"
SPEC = importlib.util.spec_from_file_location("measure_buffer_misuse", SCRIPT)
mbm = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = mbm
SPEC.loader.exec_module(mbm)

SCHEMA = """CREATE TABLE tool_calls (
    id INTEGER PRIMARY KEY AUTOINCREMENT, tool_name TEXT NOT NULL,
    called_at TEXT NOT NULL, outcome TEXT NOT NULL, overflowed INTEGER NOT NULL DEFAULT 0,
    error_msg TEXT, codescout_sha TEXT, err_family TEXT, agent_id TEXT)"""


def make_db(rows):
    """rows: (sha, called_at, overflowed, err_family, agent_id)."""
    conn = sqlite3.connect(":memory:")
    conn.execute(SCHEMA)
    for sha, at, over, fam, agent in rows:
        conn.execute(
            "INSERT INTO tool_calls(tool_name, called_at, outcome, overflowed, codescout_sha, err_family, agent_id)"
            " VALUES ('read_file', ?, ?, ?, ?, ?, ?)",
            (at, "recoverable_error" if fam else "success", over, sha, fam, agent),
        )
    return conn


class ClassifyTests(unittest.TestCase):
    def test_a_build_is_after_before_or_unknown_by_ancestry(self):
        answers = {"aaa": True, "bbb": False, "ccc": None}
        got = mbm.classify_builds(["aaa", "bbb", "ccc"], answers.__getitem__)
        self.assertEqual(got, {"aaa": "after", "bbb": "before", "ccc": "unknown"})


class CountTests(unittest.TestCase):
    def test_only_misuse_families_count_and_overflow_is_the_exposure(self):
        conn = make_db(
            [
                ("aaa", "2026-11-10 01:00:00", 1, None, None),
                ("aaa", "2026-11-10 01:00:01", 1, None, None),
                ("aaa", "2026-11-10 01:00:02", 0, "json_path_key_miss", None),
                ("aaa", "2026-11-10 01:00:03", 0, "buffer_ref_expired", None),
                ("aaa", "2026-11-10 01:00:04", 0, "il3_pipe_to_trimmer", None),  # not a misuse family
                ("aaa", "2026-11-10 01:00:05", 0, "path_not_found", None),  # not a misuse family
            ]
        )
        got = mbm.per_build_counts(conn, since="2026-11-01", agent="all")
        self.assertEqual(len(got), 1)
        self.assertEqual(got[0]["sha"], "aaa")
        self.assertEqual(got[0]["overflowed"], 2)
        self.assertEqual(got[0]["misuse"], 2)
        self.assertEqual(got[0]["by_family"], {"json_path_key_miss": 1, "buffer_ref_expired": 1})

    def test_the_window_and_the_agent_filter_apply(self):
        conn = make_db(
            [
                ("aaa", "2026-10-01 00:00:00", 1, "json_path_key_miss", None),  # before the window
                ("aaa", "2026-11-10 00:00:00", 1, "json_path_key_miss", None),  # parent
                ("aaa", "2026-11-10 00:00:01", 1, "json_path_key_miss", "agent-1"),  # subagent
            ]
        )
        self.assertEqual(mbm.per_build_counts(conn, "2026-11-01", "all")[0]["misuse"], 2)
        self.assertEqual(mbm.per_build_counts(conn, "2026-11-01", "parent")[0]["misuse"], 1)
        self.assertEqual(mbm.per_build_counts(conn, "2026-11-01", "subagent")[0]["misuse"], 1)

    def test_cohorts_add_up_per_build_and_keep_unknown_apart(self):
        builds = [
            {"sha": "aaa", "overflowed": 100, "misuse": 3, "by_family": {}},
            {"sha": "bbb", "overflowed": 200, "misuse": 5, "by_family": {}},
            {"sha": "ccc", "overflowed": 50, "misuse": 1, "by_family": {}},
            {"sha": "ddd", "overflowed": 10, "misuse": 0, "by_family": {}},
        ]
        cls = {"aaa": "before", "bbb": "before", "ccc": "after", "ddd": "unknown"}
        got = mbm.cohort_totals(builds, cls)
        self.assertEqual((got["before"]["overflowed"], got["before"]["misuse"]), (300, 8))
        self.assertEqual((got["after"]["overflowed"], got["after"]["misuse"]), (50, 1))
        self.assertEqual((got["unknown"]["overflowed"], got["unknown"]["misuse"]), (10, 0))


class StatsTests(unittest.TestCase):
    def test_rate_per_1000_and_no_exposure(self):
        self.assertAlmostEqual(mbm.rate_per_1000(14, 1000), 14.0)
        self.assertIsNone(mbm.rate_per_1000(5, 0))

    def test_rate_ratio_p_equal_rates_is_one_and_a_tripling_is_small(self):
        self.assertAlmostEqual(mbm.rate_ratio_p(10, 1000, 10, 1000), 1.0, places=6)
        self.assertAlmostEqual(mbm.rate_ratio_p(0, 1000, 0, 1000), 1.0, places=6)
        self.assertLess(mbm.rate_ratio_p(10, 1000, 30, 1000), 0.01)

    def test_rate_ratio_p_accounts_for_unequal_exposure(self):
        # 20 events in half the exposure is the same rate as 40 in the whole: not a difference.
        self.assertGreater(mbm.rate_ratio_p(40, 2000, 20, 1000), 0.9)


class VerdictTests(unittest.TestCase):
    def cohort(self, overflowed, misuse):
        return {"overflowed": overflowed, "misuse": misuse, "by_family": {}}

    def test_too_little_exposure_is_not_informative(self):
        v = mbm.verdict(self.cohort(6000, 90), self.cohort(300, 30))
        self.assertEqual(v["status"], "not-enough-data")

    def test_a_doubling_with_enough_events_raises_the_alarm(self):
        v = mbm.verdict(self.cohort(6000, 84), self.cohort(1600, 60))
        self.assertEqual(v["status"], "alarm")

    def test_a_similar_rate_reports_no_change_and_says_what_it_cannot_rule_out(self):
        v = mbm.verdict(self.cohort(6000, 84), self.cohort(1600, 22))
        self.assertEqual(v["status"], "no-change-detected")
        self.assertIn("modest", v["note"])


class EndToEndTests(unittest.TestCase):
    def test_run_reports_both_cohorts_from_a_database_file(self):
        with tempfile.TemporaryDirectory() as d:
            path = Path(d) / "usage.db"
            disk = sqlite3.connect(path)
            disk.execute(SCHEMA)
            for sha, over, fam in [("old", 1, None), ("old", 1, "json_path_key_miss"), ("new", 1, None)]:
                disk.execute(
                    "INSERT INTO tool_calls(tool_name, called_at, outcome, overflowed, codescout_sha, err_family)"
                    " VALUES ('grep', '2026-11-10 02:00:00', 'success', ?, ?, ?)",
                    (over, sha, fam),
                )
            disk.commit()
            disk.close()
            report = mbm.run(path, split="281ff16b3", since="2026-11-01", agent="all",
                             is_ancestor=lambda sha: sha == "new")
            self.assertEqual(report["cohorts"]["before"]["overflowed"], 2)
            self.assertEqual(report["cohorts"]["before"]["misuse"], 1)
            self.assertEqual(report["cohorts"]["after"]["overflowed"], 1)
            self.assertEqual(report["verdict"]["status"], "not-enough-data")


if __name__ == "__main__":
    unittest.main()
