"""Behavioral acceptance for friction-probe's complete error population."""
import datetime
import json
import os
from pathlib import Path
import sqlite3
import subprocess
import sys
import tempfile
import unittest

PROBE = Path(os.environ.get("FRICTION_PROBE", Path(__file__).resolve().parents[1] / "scripts/friction-probe.py"))
FAMILY = "il3_pipe_to_trimmer"


class FrictionProbeTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.db = Path(self.tmp.name) / "usage.db"
        now = datetime.datetime.now(datetime.timezone.utc).replace(tzinfo=None, microsecond=0)
        self.cut = (now - datetime.timedelta(days=2)).isoformat(" ")
        old = (now - datetime.timedelta(days=3)).isoformat(" ")
        recent = now.isoformat(" ")
        conn = sqlite3.connect(self.db)
        conn.execute("""CREATE TABLE tool_calls (
            id INTEGER PRIMARY KEY, tool_name TEXT, called_at TEXT, outcome TEXT,
            err_family TEXT, error_msg TEXT, session_id TEXT, cc_session_id TEXT,
            codescout_sha TEXT, codescout_dirty INTEGER)""")
        entries = [
            (1, "grep", "recoverable_error", FAMILY, "guard", "A", 0),
            (2, "grep", "recoverable_error", FAMILY, "guard", "B", 0),
            (3, "grep", "error", FAMILY, "guard", "A", 1),
            (4, "grep", "success", None, None, "A", 0),
            (5, "grep", "recoverable_error", FAMILY, "guard", "A", 0),
            (6, "grep", "unknown", None, None, "A", 0),
            (7, "read_file", "recoverable_error", None, "needs bounds", "A", 0),
            (8, "read_file", "error", None, "needs bounds", "A", 1),
            (9, "read_file", "success", None, None, "A", 0),
            (10, "grep", "success", None, None, "B", 0),
        ]
        conn.executemany("INSERT INTO tool_calls VALUES (?,?,?,?,?,?,?,?,?,?)", [
            (i, tool, old if i <= 4 else recent, outcome, family, msg,
             session, "conversation", "build", dirty)
            for i, tool, outcome, family, msg, session, dirty in entries
        ])
        conn.commit()
        conn.close()

    def report(self, *args):
        result = subprocess.run(
            [sys.executable, "-I", str(PROBE), "--db", str(self.db), "--json", *args],
            capture_output=True, text=True, timeout=15)
        self.assertEqual(result.returncode, 0, result.stderr)
        return json.loads(result.stdout)

    def test_total_counts_exactly_hard_and_recoverable_errors(self):
        r = self.report()
        self.assertEqual((r["calls"], r["errors"], r["error_pct"], r["unclassified"]),
                         (10, 6, 60.0, 2))

    def test_coverage_includes_both_error_outcomes(self):
        self.assertEqual(self.report()["coverage_by_tool"], [
            {"tool": "read_file", "errors": 2, "unclassified": 2, "pct_null": 100.0},
            {"tool": "grep", "errors": 4, "unclassified": 0, "pct_null": 0.0},
        ])

    def test_repeat_uses_next_call_and_separate_process_sessions(self):
        rows = {r["err_family"]: r for r in self.report()["immediate_repeat"]}
        self.assertEqual(rows[FAMILY], {
            "err_family": FAMILY, "errors": 4, "immediate_repeats": 1,
            "repeat_rate": 0.25, "via_msg_proxy": 0})
        self.assertEqual(rows["<unclassified>"], {
            "err_family": "<unclassified>", "errors": 2, "immediate_repeats": 1,
            "repeat_rate": 0.5, "via_msg_proxy": 2})
    def test_matching_signature_does_not_make_unknown_next_call_an_error(self):
        with sqlite3.connect(self.db) as conn:
            conn.execute("UPDATE tool_calls SET err_family=?, error_msg=? WHERE id=6",
                         (FAMILY, "guard"))
        rows = {r["err_family"]: r for r in self.report()["immediate_repeat"]}
        self.assertEqual((rows[FAMILY]["errors"], rows[FAMILY]["immediate_repeats"]),
                         (4, 1))

    def test_recovery_preserves_success_and_other_calls_in_sequence(self):
        rows = {r["err_family"]: r for r in self.report()["calls_to_recovery"]}
        self.assertEqual(rows[FAMILY], {
            "err_family": FAMILY, "errors": 4, "recovered": 4, "unrecovered": 0,
            "pct_unrecovered": 0.0, "median_calls": 2, "p90_calls": 4, "mean_calls": 2.0})
        self.assertEqual(rows["<unclassified>"]["mean_calls"], 1.5)

    def test_mixed_error_runs_stop_at_nonerror_outcomes(self):
        self.assertEqual(self.report()["error_runs"], {
            "run_length_histogram": {"1": 2, "2": 2}, "runs_total": 4,
            "runs_len_ge_2": 2, "errors_inside_runs_ge_2": 4, "errors_total": 6,
            "pct_errors_in_a_run": 66.7, "longest_run": 2})

    def test_null_detail_and_calibration_receive_complete_population(self):
        r = self.report("--null-detail", "--calibrate")
        self.assertEqual(r["null_population"],
                         [{"count": 2, "tool": "read_file", "msg": "needs bounds"}])
        family = next(x for x in r["calibration_vs_TU7"] if x["err_family"] == FAMILY)
        self.assertEqual((family["n_here"], family["here"]), (4, 0.25))

    def test_sql_split_counts_both_outcomes_and_keeps_call_denominators(self):
        r = self.report("--split-at", self.cut)
        self.assertEqual(r["before"], {"calls": 4, "errors": 3, "rate": 75.0})
        self.assertEqual(r["after"], {
            "calls": 6, "errors": 3, "rate_crude": 50.0,
            "rate_mix_adjusted": 33.33, "mix_coverage": 100.0})

    def test_clean_split_applies_same_population_filter(self):
        r = self.report("--split-at", self.cut, "--clean-only")
        self.assertEqual(r["before"], {"calls": 3, "errors": 2, "rate": 66.67})
        self.assertEqual((r["after"]["calls"], r["after"]["errors"]), (5, 2))
        self.assertEqual(r["arm_blend"]["before"]["dirty_excluded"], 1)
        self.assertEqual(r["arm_blend"]["after"]["dirty_excluded"], 1)

    def test_date_window_is_not_replaced_by_an_error_only_selection(self):
        r = self.report("--days", "1")
        self.assertEqual((r["calls"], r["errors"], r["unclassified"]), (6, 3, 2))

    def test_explicit_conversation_key_remains_available(self):
        rows = {r["err_family"]: r for r in self.report("--session-key", "cc")["immediate_repeat"]}
        self.assertEqual(rows[FAMILY]["immediate_repeats"], 2)

    def test_hard_only_corpus_remains_supported(self):
        with sqlite3.connect(self.db) as conn:
            conn.execute("DELETE FROM tool_calls WHERE outcome != 'error'")
        r = self.report()
        self.assertEqual((r["calls"], r["errors"], r["error_pct"]), (2, 2, 100.0))

    def test_success_only_corpus_has_no_errors(self):
        with sqlite3.connect(self.db) as conn:
            conn.execute("DELETE FROM tool_calls WHERE outcome != 'success'")
        r = self.report()
        self.assertEqual((r["calls"], r["errors"], r["error_pct"]), (3, 0, 0.0))
        self.assertEqual(r["immediate_repeat"], [])
        self.assertEqual(r["calls_to_recovery"], [])
        self.assertEqual(r["error_runs"]["errors_total"], 0)

    def test_help_describes_recoverable_error_population(self):
        r = subprocess.run([sys.executable, "-I", str(PROBE), "--help"],
                           capture_output=True, text=True, timeout=15)
        self.assertEqual(r.returncode, 0)
        self.assertIn("recoverable_error", r.stdout)


if __name__ == "__main__":
    unittest.main()
