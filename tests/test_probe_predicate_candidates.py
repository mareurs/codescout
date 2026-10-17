"""Regression coverage for scripts/probe-predicate-candidates.py's retrieval and redundancy figures.

docs/issues/2026-09-20-predicate-probe-overstates-retrieval-and-redundancy.md. The three
counterexamples in that file's Reproduction are reproduced here against synthetic rows, in the
form the issue itself used (importlib + hand-built rows):

  1. overflow -> grep -> read of the overflow's OWN buffer: `q4` classes the next call as
     `switched_tool`, so the existing label cannot see the later read. The new
     `own_handle_referenced_later` block must.
  2. overflow -> read of an UNRELATED buffer: `classify_next` says `queried_the_buffer`. The new
     block must count it as not-own and `next_call_queried_other_buffer` must size the overcount.
  3. read -> edit same path -> same read: `q2` counts one `repeat_with_identical_args`; the new
     `repeat_with_identical_args_no_named_edit_between` must not.

The existing keys keep their meaning (recorded figures must stay reproducible); the new keys are
additive. Rows lacking the `emitted_output_id` / `read_output_ids` columns (an older usage.db) must
degrade to "unlinkable", never raise.
"""

import importlib.util
import json
from pathlib import Path
import sys
import unittest


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts/probe-predicate-candidates.py"
SPEC = importlib.util.spec_from_file_location("probe_predicate_candidates", SCRIPT)
probe = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = probe
SPEC.loader.exec_module(probe)

_next_id = [0]


def row(tool, inp=None, *, sec=0, overflowed=0, emitted=None, read_ids=None, sid="s1"):
    _next_id[0] += 1
    return {
        "id": _next_id[0], "session_id": sid, "called_at": f"2026-10-01T00:00:{sec:02d}",
        "tool": tool, "tool_name": tool, "input_json": json.dumps(inp or {}),
        "output_json": None, "overflowed": overflowed,
        "emitted_output_id": emitted,
        "read_output_ids": json.dumps(read_ids) if read_ids else None,
    }


class Q4Linkage(unittest.TestCase):
    def test_delayed_read_of_own_buffer_is_visible_to_the_new_block_not_the_old_label(self):
        rows = [
            row("read_file", {"path": "a.rs"}, sec=0, overflowed=1, emitted="@tool_aaa"),
            row("grep", {"pattern": "x"}, sec=1),
            row("read_file", {"path": "@tool_aaa"}, sec=2, read_ids=["@tool_aaa"]),
        ]
        out = probe.q4(rows)
        # the existing label, unchanged (this is the documented undercount)
        self.assertEqual(out["next_call_classification"], {"switched_tool": 1})
        blk = out["own_handle_referenced_later"]
        self.assertEqual(blk["linkable_overflows"], 1)
        self.assertEqual(blk["within_1_calls"], 0)
        self.assertEqual(blk["within_3_calls"], 1)
        self.assertEqual(blk["by_session_end"], 1)

    def test_next_call_read_of_an_unrelated_buffer_is_sized_as_overcount(self):
        rows = [
            row("read_file", {"path": "a.rs"}, sec=0, overflowed=1, emitted="@tool_aaa"),
            row("read_file", {"path": "@tool_zzz"}, sec=1, read_ids=["@tool_zzz"]),
        ]
        out = probe.q4(rows)
        self.assertEqual(out["next_call_classification"], {"queried_the_buffer": 1})
        blk = out["own_handle_referenced_later"]
        self.assertEqual(blk["by_session_end"], 0)
        self.assertEqual(blk["next_call_queried_other_buffer"], 1)

    def test_own_handle_read_at_next_call_is_not_counted_as_other_buffer(self):
        rows = [
            row("read_file", {"path": "a.rs"}, sec=0, overflowed=1, emitted="@tool_aaa"),
            row("read_file", {"path": "@tool_aaa"}, sec=1, read_ids=["@tool_aaa"]),
        ]
        blk = probe.q4(rows)["own_handle_referenced_later"]
        self.assertEqual(blk["within_1_calls"], 1)
        self.assertEqual(blk["next_call_queried_other_buffer"], 0)

    def test_a_handle_read_in_another_session_does_not_link(self):
        rows = [
            row("read_file", {"path": "a.rs"}, sec=0, overflowed=1, emitted="@bg_00000001", sid="s1"),
            row("read_file", {"path": "@bg_00000001"}, sec=1, read_ids=["@bg_00000001"], sid="s2"),
        ]
        blk = probe.q4(rows)["own_handle_referenced_later"]
        self.assertEqual(blk["by_session_end"], 0)

    def test_overflow_without_a_recorded_handle_is_counted_unlinkable(self):
        rows = [row("read_file", {"path": "a.rs"}, sec=0, overflowed=1, emitted=None)]
        blk = probe.q4(rows)["own_handle_referenced_later"]
        self.assertEqual(blk["overflowed_calls_without_handle"], 1)
        self.assertEqual(blk["linkable_overflows"], 0)

    def test_rows_from_a_db_without_the_handle_columns_do_not_raise(self):
        r = row("read_file", {"path": "a.rs"}, sec=0, overflowed=1)
        del r["emitted_output_id"], r["read_output_ids"]
        blk = probe.q4([r])["own_handle_referenced_later"]
        self.assertEqual(blk["overflowed_calls_without_handle"], 1)

    def test_malformed_read_output_ids_do_not_raise(self):
        rows = [
            row("read_file", {"path": "a.rs"}, sec=0, overflowed=1, emitted="@tool_aaa"),
            dict(row("read_file", {"path": "x"}, sec=1), read_output_ids="not json"),
            dict(row("read_file", {"path": "x"}, sec=2), read_output_ids='{"a": 1}'),
        ]
        blk = probe.q4(rows)["own_handle_referenced_later"]
        self.assertEqual(blk["by_session_end"], 0)

    def test_empty_input(self):
        blk = probe.q4([])["own_handle_referenced_later"]
        self.assertEqual(blk["linkable_overflows"], 0)


class Q4Headline(unittest.TestCase):
    """User decision 2026-10-17: the headline eventual-retrieval figure is the NEXT CALL (horizon 1).

    `headline` is an additive key inside `own_handle_referenced_later`; every other key is untouched
    (the recorded 32.1% and the 1/3/10/session-end side-by-side stay reproducible).
    """

    def test_headline_is_horizon_one_and_names_what_it_measures_and_its_limit(self):
        rows = [
            row("read_file", {"path": "a.rs"}, sec=0, overflowed=1, emitted="@tool_aaa"),
            row("read_file", {"path": "@tool_aaa"}, sec=1, read_ids=["@tool_aaa"]),
            row("read_file", {"path": "b.rs"}, sec=2, overflowed=1, emitted="@tool_bbb"),
            row("grep", {"pattern": "x"}, sec=3),
            row("read_file", {"path": "@tool_bbb"}, sec=4, read_ids=["@tool_bbb"]),
        ]
        blk = probe.q4(rows)["own_handle_referenced_later"]
        head = blk["headline"]
        self.assertEqual(head["horizon_calls"], 1)
        self.assertEqual(head["count"], 1)  # only the first overflow was read at the NEXT call
        self.assertEqual(head["of_linkable_overflows"], 2)
        self.assertEqual(head["pct"], 50.0)
        self.assertEqual(head["count"], blk["within_1_calls"])
        self.assertEqual(head["pct"], blk["within_1_calls_pct"])
        self.assertIn("own handle referenced at the next call", head["measures"])
        self.assertIn("a mention in read_output_ids, not proof the content was needed", head["limit"])

    def test_headline_is_not_the_wider_horizons(self):
        # read 2 calls later: counts at horizon 3 and session end, NOT at the headline horizon.
        rows = [
            row("read_file", {"path": "a.rs"}, sec=0, overflowed=1, emitted="@tool_aaa"),
            row("grep", {"pattern": "x"}, sec=1),
            row("read_file", {"path": "@tool_aaa"}, sec=2, read_ids=["@tool_aaa"]),
        ]
        blk = probe.q4(rows)["own_handle_referenced_later"]
        self.assertEqual(blk["within_3_calls"], 1)
        self.assertEqual(blk["headline"]["count"], 0)
        self.assertEqual(blk["headline"]["pct"], 0.0)

    def test_headline_on_empty_input(self):
        head = probe.q4([])["own_handle_referenced_later"]["headline"]
        self.assertEqual((head["count"], head["of_linkable_overflows"], head["pct"]), (0, 0, 0.0))

    def test_every_pre_existing_key_is_still_present(self):
        blk = probe.q4([])["own_handle_referenced_later"]
        for k in ("overflowed_calls_without_handle", "linkable_overflows", "within_1_calls",
                  "within_3_calls", "within_10_calls", "by_session_end",
                  "next_call_queried_other_buffer", "within_1_calls_pct", "within_3_calls_pct",
                  "within_10_calls_pct", "by_session_end_pct"):
            self.assertIn(k, blk)

    def _run_main(self, *argv):
        import contextlib
        import io
        import sqlite3
        import tempfile

        with tempfile.TemporaryDirectory() as d:
            db = Path(d) / "usage.db"
            conn = sqlite3.connect(db)
            conn.execute(
                "CREATE TABLE tool_calls (id INTEGER PRIMARY KEY, tool_name TEXT, called_at TEXT, "
                "latency_ms INTEGER, outcome TEXT, overflowed INTEGER, session_id TEXT, "
                "project_root TEXT, input_json TEXT, output_json TEXT, "
                "emitted_output_id TEXT, read_output_ids TEXT)"
            )
            calls = [
                ("read_file", "2026-10-01T00:00:00", 1, '{"path": "a.rs"}', "@tool_aaa", None),
                ("read_file", "2026-10-01T00:00:01", 0, '{"path": "@tool_aaa"}', None, '["@tool_aaa"]'),
            ]
            for tool, at, ovf, inp, emitted, read_ids in calls:
                conn.execute(
                    "INSERT INTO tool_calls (tool_name, called_at, latency_ms, outcome, overflowed, "
                    "session_id, project_root, input_json, output_json, emitted_output_id, "
                    "read_output_ids) VALUES (?, ?, 1, 'success', ?, 's1', '/p', ?, NULL, ?, ?)",
                    (tool, at, ovf, inp, emitted, read_ids),
                )
            conn.commit()
            conn.close()
            buf = io.StringIO()
            old_argv = sys.argv
            sys.argv = ["probe", "--db", str(db), *argv]
            try:
                with contextlib.redirect_stdout(buf):
                    rc = probe.main()
            finally:
                sys.argv = old_argv
        self.assertEqual(rc, 0)
        return buf.getvalue()

    def test_human_readable_report_prints_the_labelled_headline_first(self):
        out = self._run_main()
        first = out.splitlines()[0]
        self.assertIn("own handle referenced at the next call", first)
        self.assertIn("a mention in read_output_ids, not proof the content was needed", first)
        self.assertIn("1 of 1", first)
        self.assertIn("100.0%", first)

    def test_json_mode_stays_pure_json_and_carries_the_headline(self):
        rep = json.loads(self._run_main("--json"))
        head = rep["q4_overflow_economics"]["own_handle_referenced_later"]["headline"]
        self.assertEqual(head["count"], 1)


class Q2Redundancy(unittest.TestCase):
    def test_reread_after_an_edit_of_the_same_path_is_not_a_no_edit_repeat(self):
        rows = [
            row("read_file", {"path": "a.rs"}, sec=0),
            row("edit_file", {"path": "a.rs", "old_string": "x", "new_string": "y"}, sec=1),
            row("read_file", {"path": "a.rs"}, sec=2),
        ]
        out = probe.q2(rows)
        self.assertEqual(out["repeat_with_identical_args"], 1)  # existing key: unchanged
        self.assertEqual(out["repeat_with_identical_args_no_named_edit_between"], 0)

    def test_reread_with_no_edit_between_still_counts(self):
        rows = [row("read_file", {"path": "a.rs"}, sec=0), row("read_file", {"path": "a.rs"}, sec=1)]
        out = probe.q2(rows)
        self.assertEqual(out["repeat_with_identical_args_no_named_edit_between"], 1)

    def test_edit_of_a_different_path_does_not_reset(self):
        rows = [
            row("read_file", {"path": "a.rs"}, sec=0),
            row("edit_code", {"path": "b.rs", "symbol": "f", "action": "replace", "body": "x"}, sec=1),
            row("read_file", {"path": "a.rs"}, sec=2),
        ]
        self.assertEqual(probe.q2(rows)["repeat_with_identical_args_no_named_edit_between"], 1)

    def test_edit_in_another_session_does_not_reset(self):
        rows = [
            row("read_file", {"path": "a.rs"}, sec=0),
            row("edit_file", {"path": "a.rs"}, sec=1, sid="s2"),
            row("read_file", {"path": "a.rs"}, sec=2),
        ]
        self.assertEqual(probe.q2(rows)["repeat_with_identical_args_no_named_edit_between"], 1)

    def test_edit_with_malformed_input_does_not_raise(self):
        rows = [
            row("read_file", {"path": "a.rs"}, sec=0),
            dict(row("edit_file", sec=1), input_json="not json"),
            dict(row("edit_file", sec=1), input_json=None),
            row("read_file", {"path": "a.rs"}, sec=2),
        ]
        self.assertEqual(probe.q2(rows)["repeat_with_identical_args_no_named_edit_between"], 1)

    def test_empty_input(self):
        self.assertEqual(probe.q2([])["repeat_with_identical_args_no_named_edit_between"], 0)


if __name__ == "__main__":
    unittest.main()
