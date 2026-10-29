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
    # -- final review I-8 and the ledger's deferred checks (T6; spec section 4 buddy skills) --

    def init_edit(self, name, edit):
        recs = records(name)
        for r in recs:
            if r.get("subtype") == "init":
                edit(r)
        return recs

    def test_full_without_buddy_skills_is_invalid(self):
        recs = self.init_edit("full_valid.jsonl", lambda r: r.update(
            skills=[s for s in r["skills"] if not s.startswith("buddy:")]))
        v = self.check("full", recs)
        self.assertEqual(v.reasons, ["full arm has no buddy:* skills visible in init"])

    def test_full_without_skill_or_the_subagent_tool_is_invalid(self):
        for drop, why in (("Skill", "lacks Skill"), ("Task", "lacks the subagent tool")):
            with self.subTest(drop):
                recs = self.init_edit("full_valid.jsonl", lambda r, d=drop: r.update(
                    tools=[t for t in r["tools"] if t != d]))
                v = self.check("full", recs)
                self.assertFalse(v.ok)
                self.assertEqual(len(v.reasons), 1, v.reasons)
                self.assertIn(why, v.reasons[0])
        # The CLI may list the subagent tool as Agent: either name counts.
        recs = self.init_edit("full_valid.jsonl", lambda r: r.update(
            tools=["Agent" if t == "Task" else t for t in r["tools"]]))
        self.assertTrue(self.check("full", recs).ok)

    def test_the_full_fixture_init_is_the_i8_capture(self):
        init = next(r for r in records("full_valid.jsonl") if r.get("subtype") == "init")
        native = sorted(t for t in init["tools"] if not t.startswith("mcp__"))
        self.assertEqual(native, ["Glob", "Grep", "Read", "Skill", "Task"])

    def test_bare_with_companion_output_is_contaminated(self):
        companion = next(r for r in records("cs_valid.jsonl") if integrity._is_evidence(r))
        recs = records("bare_valid.jsonl") + [companion]
        v = self.check("bare", recs)
        self.assertFalse(v.ok)
        self.assertTrue(v.reasons[0].startswith(integrity.CONTAMINATION), v.reasons)
        self.assertIn("companion hook output", v.reasons[0])

    def test_full_only_skills_or_tools_in_bare_or_cs_are_contamination(self):
        for arm, name in (("bare", "bare_valid.jsonl"), ("cs", "cs_valid.jsonl")):
            for edit, word in (
                    (lambda r: r.update(skills=r["skills"] + ["superpowers:brainstorming"]),
                     "superpowers:brainstorming"),
                    (lambda r: r.update(skills=r["skills"] + ["buddy:check"]), "buddy:check"),
                    (lambda r: r.update(tools=r["tools"] + ["Skill"]), "Skill"),
                    (lambda r: r.update(tools=r["tools"] + ["Task"]), "Task"),
                    (lambda r: r.update(tools=r["tools"] + ["Agent"]), "Agent")):
                with self.subTest(arm=arm, word=word):
                    v = self.check(arm, self.init_edit(name, edit))
                    self.assertEqual(len(v.reasons), 1, v.reasons)
                    self.assertTrue(v.reasons[0].startswith(integrity.CONTAMINATION))
                    self.assertIn(word, v.reasons[0])
        # The plain fixtures carry none of it.
        self.assertTrue(self.check("bare", records("bare_valid.jsonl")).ok)
        self.assertTrue(self.check("cs", records("cs_valid.jsonl")).ok)

    def test_the_real_stage1_streams(self):
        # Read only. bare and cs pass the new checks too; full fails exactly on the tools I-8
        # adds (Stage 1 ran before them).
        runs = Path("/home/marius/work/claude/setup-ab-data/study/runs")
        if not runs.is_dir():
            self.skipTest("no Stage 1 runs on this machine")
        seen = 0
        for d in sorted(runs.iterdir()):
            arm = d.name.split(".")[-2]
            result = next(r for r in (json.loads(l) for l in (d / "stream.dedup.jsonl")
                                      .read_text().splitlines() if l.strip())
                          if r.get("type") == "result")
            v = integrity.check(arm, d / "stream.dedup.jsonl", result)
            if arm == "full":
                self.assertEqual(v.reasons, ["full arm init tool list lacks Skill",
                                             "full arm init tool list lacks the subagent tool "
                                             "(Agent / Task)"], d.name)
            else:
                self.assertEqual(v.reasons, [], d.name)
            seen += 1
        self.assertEqual(seen, 12)

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
    def test_errored_run_with_no_model_work_is_invalid(self):
        # the real auth failure shape (round4c): is_error, zero api time, zero tokens, zero cost
        auth = {"is_error": True, "duration_api_ms": 0, "total_cost_usd": 0, "num_turns": 1,
                "usage": {"input_tokens": 0, "output_tokens": 0}, "result": "Not logged in"}
        recs = [r for r in records("cs_valid.jsonl") if r.get("type") != "assistant"]
        v = self.check("cs", recs, stdout=auth)
        self.assertFalse(v.ok)
        self.assertTrue(any("before any model work" in x for x in v.reasons), v.reasons)

    def test_errored_run_after_real_work_is_scored_not_invalid(self):
        # max-turns / budget failure: a model-side failure is a result, not a dropped run
        failed = {"is_error": True, "subtype": "error_max_turns", "duration_api_ms": 51000,
                  "total_cost_usd": 0.4, "num_turns": 30,
                  "usage": {"input_tokens": 900, "output_tokens": 300}}
        v = self.check("cs", records("cs_valid.jsonl"), stdout=failed)
        self.assertEqual(v.reasons, [])
        self.assertTrue(v.ok)

    def test_errored_run_with_zero_api_time_but_tool_calls_is_not_invalid(self):
        odd = {"is_error": True, "duration_api_ms": 0, "total_cost_usd": 0,
               "usage": {"input_tokens": 0, "output_tokens": 0}}
        self.assertTrue(self.check("cs", records("cs_valid.jsonl"), stdout=odd).ok)

    def test_errored_run_with_missing_usage_fields_is_not_assumed_no_work(self):
        self.assertTrue(self.check("cs", records("cs_valid.jsonl"),
                                   stdout={"is_error": True}).ok)

    def test_non_companion_hook_output_is_not_evidence_for_full(self):
        # real full stream: the superpowers hook (non-empty) precedes the companion's
        recs = records("full_valid.jsonl")
        companion = [r for r in recs if r.get("subtype") == "hook_response"
                     and "PROJECT BOOTSTRAP" in r["output"]]
        others = [r for r in recs if r.get("subtype") == "hook_response" and r["output"]
                  and r not in companion]
        self.assertEqual((len(companion), len(others)), (1, 1))  # fixture keeps both
        for r in companion:
            r["output"] = r["stdout"] = ""
        v = self.check("full", recs)
        self.assertFalse(v.ok)
        self.assertEqual(v.hook_events, 0)
        self.assertTrue(any("companion" in x for x in v.reasons), v.reasons)

    def test_cs_with_only_foreign_hook_output_is_invalid(self):
        recs = records("cs_valid.jsonl")
        for r in recs:
            if r.get("subtype") == "hook_response" and r["output"]:
                r["output"] = r["stdout"] = "<EXTREMELY_IMPORTANT> You have superpowers."
        self.assertFalse(self.check("cs", recs).ok)

    def test_tool_use_inside_a_user_tool_result_is_not_counted(self):
        recs = records("bare_valid.jsonl")
        recs.insert(1, {"type": "user", "message": {"role": "user", "content": [
            {"type": "tool_result", "tool_use_id": "toolu_REDACTED1",
             "content": [{"type": "tool_use", "name": "mcp__codescout__symbols"}]}]}})
        self.assertEqual(self.check("bare", recs).codescout_calls, 0)

    def test_sidechain_calls_are_counted_in_totals_and_separately(self):
        recs = records("cs_valid.jsonl")
        self.assertEqual(self.check("cs", recs).sidechain_calls, 0)  # main-thread fixture
        side = {"type": "assistant", "parent_tool_use_id": "toolu_PARENT", "message": {
            "content": [{"type": "tool_use", "id": "toolu_S", "name": "mcp__codescout__grep",
                         "input": {}},
                        {"type": "tool_use", "id": "toolu_T", "name": "Read", "input": {}}]}}
        v = self.check("cs", recs + [side])
        self.assertEqual((v.codescout_calls, v.native_calls, v.sidechain_calls), (3, 3, 2))
        self.assertTrue(v.ok)

    def test_fixture_tool_use_records_carry_parent_tool_use_id_null(self):
        calls = [r for r in records("cs_valid.jsonl") if r.get("type") == "assistant"]
        self.assertTrue(calls)
        for r in calls:
            self.assertIn("parent_tool_use_id", r)
            self.assertIsNone(r["parent_tool_use_id"])


    def test_stdout_json_not_a_dict_is_invalid(self):
        v = self.check("bare", records("bare_valid.jsonl"), stdout=None)
        self.assertFalse(v.ok)

    def test_codescout_name_in_a_text_block_is_not_counted(self):
        recs = records("bare_valid.jsonl")
        recs.insert(1, {"type": "assistant", "message": {"content": [
            {"type": "text", "text": "mcp__codescout__symbols"}]}})
        self.assertEqual(self.check("bare", recs).codescout_calls, 0)

    def test_verdict_is_a_namedtuple(self):
        self.assertEqual(Verdict._fields,
                         ("ok", "reasons", "codescout_calls", "native_calls", "hook_events",
                          "sidechain_calls"))


if __name__ == "__main__":
    unittest.main()
