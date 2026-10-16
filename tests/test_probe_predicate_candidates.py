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
