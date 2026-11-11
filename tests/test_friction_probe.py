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
class CommandFailureTests(unittest.TestCase):
    """FR-2: `outcome='success'` on a `run_command` row says the tool answered, not that the
    command passed. The probe reads the result's own `exit_code` and `timed_out`."""

    def make_db(self, rows, with_output_column=True):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        db = Path(tmp.name) / "usage.db"
        extra = ", output_json TEXT" if with_output_column else ""
        with sqlite3.connect(db) as conn:
            conn.execute(f"""CREATE TABLE tool_calls (
                id INTEGER PRIMARY KEY, tool_name TEXT, called_at TEXT, outcome TEXT,
                err_family TEXT, error_msg TEXT, session_id TEXT, cc_session_id TEXT,
                codescout_sha TEXT, codescout_dirty INTEGER{extra})""")
            now = datetime.datetime.now(datetime.timezone.utc).replace(tzinfo=None, microsecond=0)
            for i, (tool, outcome, output) in enumerate(rows, 1):
                if isinstance(output, dict):
                    # The shape `usage.db` really stores: an MCP content list whose text is
                    # the result JSON, pretty-printed.
                    output = json.dumps([{"type": "text", "text": json.dumps(output, indent=2)}])
                values = [i, tool, now.isoformat(" "), outcome, None, None, "A", "c", "b", 0]
                if with_output_column:
                    values.append(output)
                conn.execute(
                    f"INSERT INTO tool_calls VALUES ({','.join('?' * len(values))})", values)
        return db

    def failures(self, db):
        result = subprocess.run(
            [sys.executable, "-I", str(PROBE), "--db", str(db), "--json"],
            capture_output=True, text=True, timeout=15)
        self.assertEqual(result.returncode, 0, result.stderr)
        return json.loads(result.stdout)["run_command_failures_recorded_as_success"]

    def test_a_failed_command_and_a_timeout_are_counted_from_the_result(self):
        db = self.make_db([
            ("run_command", "success", {"exit_code": 0, "stdout": "ok"}),
            ("run_command", "success", {"exit_code": 101, "stdout": "failed"}),
            ("run_command", "success", {"timed_out": True, "exit_code": None}),
            ("run_command", "success", {"exit_code": 1, "timed_out": True}),
            ("run_command", "success", None),
            ("run_command", "success", "not json at all"),
            # A bare result object, not wrapped in a content list, is read as well.
            ("run_command", "success", json.dumps({"exit_code": 3})),
            # Not judged: another tool, and a row whose OUTCOME already says it failed.
            ("grep", "success", {"exit_code": 2}),
            ("run_command", "error", {"exit_code": 1}),
        ])
        self.assertEqual(self.failures(db), {
            "available": True, "success_rows": 7, "with_output": 6, "unreadable": 1,
            "readable": 5, "nonzero_exit": 3, "timed_out": 2, "failed": 4,
            "pct_of_readable": 80.0})
    def test_a_cut_off_result_is_unreadable_not_guessed(self):
        whole = json.dumps([{"type": "text", "text": json.dumps({"exit_code": 101})}])
        db = self.make_db([("run_command", "success", whole[:-8])])
        r = self.failures(db)
        self.assertEqual((r["with_output"], r["unreadable"], r["failed"]), (1, 1, 0))

    def test_a_command_that_only_prints_exit_code_is_not_a_failure(self):
        # The text-pattern count of FR-2 (1,734 rows) cannot tell these apart; parsed JSON can.
        db = self.make_db([
            ("run_command", "success",
             {"exit_code": 0, "stdout": "\"exit_code\": 101 timed_out: true"}),
        ])
        self.assertEqual(self.failures(db)["failed"], 0)

    def test_a_corpus_with_no_output_column_says_so_instead_of_failing(self):
        db = self.make_db([("run_command", "success", None)], with_output_column=False)
        r = self.failures(db)
        self.assertFalse(r["available"])
        self.assertIn("output_json", r["reason"])

    def test_no_readable_rows_gives_no_rate(self):
        db = self.make_db([("run_command", "success", None)])
        self.assertIsNone(self.failures(db)["pct_of_readable"])
    def test_result_fact_columns_are_read_without_any_stored_output(self):
        # A server without --debug stores no output_json; its rows still carry the facts.
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        db = Path(tmp.name) / "usage.db"
        now = datetime.datetime.now(datetime.timezone.utc).replace(tzinfo=None, microsecond=0)
        with sqlite3.connect(db) as conn:
            conn.execute("""CREATE TABLE tool_calls (
                id INTEGER PRIMARY KEY, tool_name TEXT, called_at TEXT, outcome TEXT,
                err_family TEXT, error_msg TEXT, session_id TEXT, cc_session_id TEXT,
                codescout_sha TEXT, codescout_dirty INTEGER, output_json TEXT,
                command_exit INTEGER, command_timed_out INTEGER)""")
            rows = [
                ("run_command", "success", 0, 0),      # passed
                ("run_command", "success", 101, None), # failed
                ("run_command", "success", 0, 1),      # timed out
                ("run_command", "success", None, None),  # not recorded: not "passed"
                ("run_command", "error", 1, None),     # outcome already says it failed
                ("grep", "success", 2, None),          # another tool
            ]
            for i, (tool, outcome, code, timed_out) in enumerate(rows, 1):
                conn.execute(
                    "INSERT INTO tool_calls VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?)",
                    (i, tool, now.isoformat(" "), outcome, None, None, "A", "c", "b", 0,
                     None, code, timed_out))
        r = self.failures(db)
        self.assertEqual(r["readable"], 0)
        self.assertEqual(r["from_columns"], {
            "success_rows": 4, "recorded": 3, "nonzero_exit": 1, "timed_out": 1,
            "failed": 2, "pct_of_recorded": 66.7})
    def by_client(self, db):
        result = subprocess.run(
            [sys.executable, "-I", str(PROBE), "--db", str(db), "--json"],
            capture_output=True, text=True, timeout=15)
        self.assertEqual(result.returncode, 0, result.stderr)
        return json.loads(result.stdout)["by_client"]

    def make_client_db(self, with_facts):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        db = Path(tmp.name) / "usage.db"
        now = datetime.datetime.now(datetime.timezone.utc).replace(tzinfo=None, microsecond=0)
        facts = ", output_json TEXT, output_bytes INTEGER, command_exit INTEGER, " \
                "command_timed_out INTEGER" if with_facts else ""
        with sqlite3.connect(db) as conn:
            conn.execute(f"""CREATE TABLE tool_calls (
                id INTEGER PRIMARY KEY, tool_name TEXT, called_at TEXT, outcome TEXT,
                err_family TEXT, error_msg TEXT, session_id TEXT, cc_session_id TEXT,
                codescout_sha TEXT, codescout_dirty INTEGER, client_name TEXT{facts})""")
            # (tool, outcome, client, output_json, bytes, exit, timed_out)
            rows = [
                ("run_command", "success", "alpha", "{}", 5, 0, None),
                ("run_command", "success", "alpha", None, 5, 101, None),
                ("run_command", "success", "beta", None, 7, None, 1),
                ("grep", "success", "beta", None, 3, None, None),
                ("run_command", "success", None, None, None, None, None),  # an older build
            ]
            for i, (tool, outcome, client, out, nbytes, code, timed_out) in enumerate(rows, 1):
                values = [i, tool, now.isoformat(" "), outcome, None, None, "A", "c", "b", 0, client]
                if with_facts:
                    values += [out, nbytes, code, timed_out]
                conn.execute(
                    f"INSERT INTO tool_calls VALUES ({','.join('?' * len(values))})", values)
        return db

    def test_calls_are_grouped_by_the_client_that_made_them(self):
        r = self.by_client(self.make_client_db(with_facts=True))
        self.assertEqual(r, {"available": True, "clients": [
            {"client": "alpha", "calls": 2, "with_output": 1, "with_facts": 2,
             "recorded": 2, "failed": 1},
            {"client": "beta", "calls": 2, "with_output": 0, "with_facts": 2,
             "recorded": 1, "failed": 1},
            {"client": "<null>", "calls": 1, "with_output": 0, "with_facts": 0,
             "recorded": 0, "failed": 0},
        ]})

    def test_a_client_column_without_the_fact_columns_still_counts_calls(self):
        r = self.by_client(self.make_client_db(with_facts=False))
        self.assertEqual([(c["client"], c["calls"]) for c in r["clients"]],
                         [("alpha", 2), ("beta", 2), ("<null>", 1)])
        self.assertNotIn("with_output", r["clients"][0])

    def test_a_corpus_with_no_client_column_says_so(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        db = Path(tmp.name) / "usage.db"
        with sqlite3.connect(db) as conn:
            conn.execute("""CREATE TABLE tool_calls (
                id INTEGER PRIMARY KEY, tool_name TEXT, called_at TEXT, outcome TEXT,
                err_family TEXT, error_msg TEXT, session_id TEXT, cc_session_id TEXT,
                codescout_sha TEXT, codescout_dirty INTEGER)""")
            conn.execute("INSERT INTO tool_calls VALUES (1,'grep',datetime('now'),'success',"
                         "NULL,NULL,'A','c','b',0)")
        r = self.by_client(db)
        self.assertFalse(r["available"])
        self.assertIn("client_name", r["reason"])


if __name__ == "__main__":
    unittest.main()
