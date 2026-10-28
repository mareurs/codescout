"""Tests for integrity.py: an arm that lost its setup is invalid, not scored."""
import json
import tempfile
import unittest
from pathlib import Path

import integrity
from integrity import Verdict

FIX = Path(__file__).resolve().parent / "fixtures"
OK_STDOUT = {"type": "result", "subtype": "success", "is_error": False}


def records(name):
    return [json.loads(line) for line in (FIX / name).read_text().splitlines()]


def write(tmp, recs, raw_extra=""):
    p = Path(tmp) / "t.jsonl"
    p.write_text("".join(json.dumps(r) + "\n" for r in recs) + raw_extra)
    return p


class IntegrityTest(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.tmp = self._tmp.name
        self.addCleanup(self._tmp.cleanup)

    def check(self, arm, recs, stdout=OK_STDOUT, raw_extra=""):
        return integrity.check(arm, write(self.tmp, recs, raw_extra), stdout)

    # -- the six named tests from the brief --

    def test_cs_without_codescout_tools_in_init_is_invalid(self):
        recs = records("cs_valid.jsonl")
        for r in recs:
            if r.get("subtype") == "init":
                r["tools"] = [t for t in r["tools"] if not t.startswith("mcp__")]
        v = self.check("cs", recs)
        self.assertFalse(v.ok)
        self.assertTrue(any("mcp__codescout__references" in x for x in v.reasons), v.reasons)

    def test_cs_with_tools_but_zero_calls_is_invalid(self):
        recs = [r for r in records("cs_valid.jsonl") if r.get("type") != "assistant"]
        v = self.check("cs", recs)
        self.assertFalse(v.ok)
        self.assertEqual(v.codescout_calls, 0)
        self.assertTrue(any("zero codescout calls" in x for x in v.reasons), v.reasons)

    def test_cs_without_hook_evidence_is_invalid(self):
        recs = [r for r in records("cs_valid.jsonl")
                if not str(r.get("subtype", "")).startswith("hook")]
        v = self.check("cs", recs)
        self.assertFalse(v.ok)
        self.assertEqual(v.hook_events, 0)
        self.assertTrue(any("hook" in x for x in v.reasons), v.reasons)

    def test_bare_with_any_mcp_tool_is_invalid(self):
        recs = records("bare_valid.jsonl")
        for r in recs:
            if r.get("subtype") == "init":
                r["tools"] = r["tools"] + ["mcp__other__thing"]
        v = self.check("bare", recs)
        self.assertFalse(v.ok)
        self.assertTrue(any("mcp__other__thing" in x for x in v.reasons), v.reasons)

    def test_full_without_superpowers_skills_is_invalid(self):
        recs = records("full_valid.jsonl")
        for r in recs:
            if r.get("subtype") == "init":
                r["skills"] = [s for s in r["skills"] if not s.startswith("superpowers:")]
        v = self.check("full", recs)
        self.assertFalse(v.ok)
        self.assertTrue(any("superpowers" in x for x in v.reasons), v.reasons)

    def test_valid_cs_counts_calls(self):
        v = self.check("cs", records("cs_valid.jsonl"))
        self.assertEqual(v.reasons, [])
        self.assertTrue(v.ok)
        self.assertEqual(v.codescout_calls, 2)
        self.assertEqual(v.native_calls, 2)  # Read, Bash
        self.assertEqual(v.hook_events, 1)  # the empty UserPromptSubmit response is not evidence

    # -- the controller's rulings --

    def test_valid_full_and_bare_pass(self):
        self.assertTrue(self.check("full", records("full_valid.jsonl")).ok)
        v = self.check("bare", records("bare_valid.jsonl"))
        self.assertEqual(v.reasons, [])
        self.assertEqual((v.codescout_calls, v.native_calls, v.hook_events), (0, 2, 0))

    def test_every_failed_rule_is_listed_not_only_the_first(self):
        recs = [r for r in records("full_valid.jsonl") if r.get("type") != "assistant"
                and not str(r.get("subtype", "")).startswith("hook")]
        for r in recs:
            if r.get("subtype") == "init":
                r["tools"] = [t for t in r["tools"] if not t.startswith("mcp__")]
                r["skills"] = []
        v = self.check("full", recs)
        self.assertFalse(v.ok)
        self.assertGreaterEqual(len(v.reasons), 4, v.reasons)  # tools, skills, calls, hooks
        self.assertEqual(v.ok, not v.reasons)

    def test_empty_hook_output_is_not_evidence(self):
        recs = records("cs_valid.jsonl")
        for r in recs:
            if r.get("subtype") == "hook_response":
                r["output"] = r["stdout"] = "  \n"
        v = self.check("cs", recs)
        self.assertFalse(v.ok)
        self.assertEqual(v.hook_events, 0)

    def test_hook_started_alone_is_not_evidence(self):
        recs = [r for r in records("cs_valid.jsonl") if r.get("subtype") != "hook_response"]
        self.assertEqual(self.check("cs", recs).hook_events, 0)

    def test_cs_missing_symbol_at_is_invalid(self):
        recs = records("cs_valid.jsonl")
        for r in recs:
            if r.get("subtype") == "init":
                r["tools"].remove("mcp__codescout__symbol_at")
        v = self.check("cs", recs)
        self.assertFalse(v.ok)
        self.assertTrue(any("mcp__codescout__symbol_at" in x for x in v.reasons), v.reasons)
        self.assertFalse(any("mcp__codescout__references" in x for x in v.reasons), v.reasons)

    def test_empty_transcript_is_invalid(self):
        p = Path(self.tmp) / "e.jsonl"
        p.write_text("")
        for arm in ("bare", "cs", "full"):
            v = integrity.check(arm, p, OK_STDOUT)
            self.assertFalse(v.ok, arm)
            self.assertTrue(any("empty" in x for x in v.reasons), v.reasons)

    def test_missing_transcript_file_is_invalid(self):
        v = integrity.check("cs", Path(self.tmp) / "nope.jsonl", OK_STDOUT)
        self.assertFalse(v.ok)
        self.assertTrue(any("unreadable" in x for x in v.reasons), v.reasons)

    def test_malformed_line_is_a_reason_not_skipped_into_a_pass(self):
        v = self.check("cs", records("cs_valid.jsonl"), raw_extra="{not json\n")
        self.assertFalse(v.ok)
        self.assertTrue(any("malformed" in x for x in v.reasons), v.reasons)

    def test_no_init_record_is_invalid(self):
        recs = [r for r in records("bare_valid.jsonl") if r.get("subtype") != "init"]
        v = self.check("bare", recs)
        self.assertFalse(v.ok)
        self.assertTrue(any("init" in x for x in v.reasons), v.reasons)

    def test_unknown_arm_is_invalid(self):
        v = self.check("weird", records("bare_valid.jsonl"))
        self.assertFalse(v.ok)
        self.assertTrue(any("unknown arm" in x for x in v.reasons), v.reasons)

    def test_errored_run_is_invalid(self):
        v = self.check("cs", records("cs_valid.jsonl"), stdout={"is_error": True})
        self.assertFalse(v.ok)
        self.assertTrue(any("is_error" in x for x in v.reasons), v.reasons)

    def test_stdout_json_not_a_dict_is_invalid(self):
        v = self.check("bare", records("bare_valid.jsonl"), stdout=None)
        self.assertFalse(v.ok)

    def test_codescout_tool_use_in_user_or_text_blocks_is_not_counted(self):
        recs = records("bare_valid.jsonl")
        recs.insert(1, {"type": "assistant", "message": {"content": [
            {"type": "text", "text": "mcp__codescout__symbols"}]}})
        self.assertEqual(self.check("bare", recs).codescout_calls, 0)

    def test_verdict_is_a_namedtuple(self):
        self.assertEqual(Verdict._fields,
                         ("ok", "reasons", "codescout_calls", "native_calls", "hook_events"))


if __name__ == "__main__":
    unittest.main()
