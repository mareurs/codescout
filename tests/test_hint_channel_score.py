"""Tests for tools/hint-channel-eval/score.py: one run's transcript and events -> one scored row.

Synthetic transcripts mirror the shapes read from the pilot (CLI 2.1.309): assistant entries
split per content block and sharing message.id, tool results in user entries, and hook context
as attachment entries of type hook_additional_context carrying the triggering toolUseID."""
import importlib.util
import json
import os
import pathlib
import sys
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[1]
EVAL = ROOT / "tools" / "hint-channel-eval"
sys.path.insert(0, str(EVAL))


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


score = load("hint_channel_score", pathlib.Path(os.environ.get("HCE_SCORE_UNDER_TEST") or EVAL / "score.py"))
hook = load("hint_channel_hook_for_score", EVAL / "hook.py")

FN, PATH = "ensure_resident", "src/agent/mod.rs"
HINT = hook.hint_text(FN, PATH)
DIRECTIVE = hook.directive_text(FN, PATH)


def tool_use(msg, tid, name, inp):
    return {"type": "assistant", "message": {"id": msg, "content": [
        {"type": "tool_use", "id": tid, "name": name, "input": inp}]}}


def tool_result(tid, *texts):
    return {"type": "user", "message": {"content": [
        {"type": "tool_result", "tool_use_id": tid, "content": [{"type": "text", "text": t} for t in texts]}]}}


def context(tid, text):
    return {"type": "attachment", "attachment": {"type": "hook_additional_context", "toolUseID": tid,
                                                 "hookEvent": "PostToolUse", "content": [text]}}


def ev(event, tid, **kw):
    return {"event": event, "tool_use_id": tid, **kw}


def grep_call(msg="m1", tid="t_grep"):
    return tool_use(msg, tid, "mcp__codescout__grep", {"pattern": FN})


def refs_call(msg="m2", tid="t_refs"):
    return tool_use(msg, tid, "mcp__codescout__references", {"symbol": FN, "path": PATH})


def edit_call(msg="m3", tid="t_edit"):
    return tool_use(msg, tid, "mcp__codescout__edit_code", {"symbol": f"Agent/{FN}", "action": "replace"})


INIT_TOOLS = ["mcp__codescout__grep", "mcp__codescout__references", "mcp__codescout__call_graph"]


def run(arm, transcript, events, result=None, init_tools=INIT_TOOLS):
    return score.score_run(transcript, events, result or {"total_cost_usd": 0.2, "num_turns": 5},
                           arm=arm, fn=FN, path=PATH, init_tools=init_tools)


class TestPrimaryMetric(unittest.TestCase):
    def test_references_in_an_earlier_batch_counts(self):
        t = [grep_call(), tool_result("t_grep", "x", HINT), refs_call(), tool_result("t_refs", "y"), edit_call()]
        row = run("A0", t, [ev("trigger", "t_grep", carrier="result"), ev("stop", "t_edit")])
        self.assertTrue(row["refs_before_stop"])
        self.assertTrue(row["valid"])

    def test_references_in_the_same_batch_as_the_edit_does_not_count(self):
        t = [grep_call(), tool_result("t_grep", "x", HINT), refs_call(msg="m3"), edit_call(msg="m3")]
        row = run("A0", t, [ev("trigger", "t_grep", carrier="result"), ev("stop", "t_edit")])
        self.assertFalse(row["refs_before_stop"])

    def test_references_after_the_stop_does_not_count(self):
        t = [grep_call(), tool_result("t_grep", "x", HINT), edit_call(msg="m2"), refs_call(msg="m3")]
        row = run("A0", t, [ev("trigger", "t_grep", carrier="result"), ev("stop", "t_edit")])
        self.assertFalse(row["refs_before_stop"])

    def test_references_on_another_symbol_does_not_count(self):
        other = tool_use("m2", "t_r2", "mcp__codescout__references", {"symbol": "ensure_resident_bytes"})
        t = [grep_call(), tool_result("t_grep", "x", HINT), other, edit_call()]
        row = run("A0", t, [ev("trigger", "t_grep", carrier="result"), ev("stop", "t_edit")])
        self.assertFalse(row["refs_before_stop"])

    def test_call_graph_counts_as_a_caller_lookup(self):
        cg = tool_use("m2", "t_cg", "mcp__codescout__call_graph", {"symbol": FN, "direction": "callers"})
        t = [grep_call(), tool_result("t_grep", "x", HINT), cg, edit_call()]
        row = run("A0", t, [ev("trigger", "t_grep", carrier="result"), ev("stop", "t_edit")])
        self.assertTrue(row["refs_before_stop"])

    def test_a_grep_on_the_name_is_a_caller_check_but_not_the_primary_metric(self):
        t = [grep_call(), tool_result("t_grep", "x"), edit_call()]
        row = run("F", t, [ev("trigger", "t_grep", carrier="none"), ev("stop", "t_edit")])
        self.assertFalse(row["refs_before_stop"])
        self.assertTrue(row["caller_check_before_stop"])

    def test_a_bash_sed_naming_the_fn_is_not_a_caller_check(self):
        sed = tool_use("m1", "t_sed", "Bash", {"command": f"sed -i 's/{FN}(a, b)/{FN}(a)/' src/x.rs"})
        t = [sed, tool_result("t_sed", ""), tool_use("m2", "t_s", "mcp__codescout__symbols", {"name": FN}),
             tool_result("t_s", "z"), edit_call()]
        row = run("F", t, [ev("trigger", "t_s", carrier="none"), ev("stop", "t_edit")])
        self.assertFalse(row["caller_check_before_stop"])

    def test_a_bash_grep_naming_the_fn_is_a_caller_check(self):
        g = tool_use("m1", "t_bg", "Bash", {"command": f"rg -n '{FN}\\(' src"})
        t = [g, tool_result("t_bg", ""), tool_use("m2", "t_s", "mcp__codescout__symbols", {"name": FN}),
             tool_result("t_s", "z"), edit_call()]
        row = run("F", t, [ev("trigger", "t_s", carrier="none"), ev("stop", "t_edit")])
        self.assertTrue(row["caller_check_before_stop"])


class TestArrival(unittest.TestCase):
    def test_context_arm_needs_the_hint_in_an_attachment_keyed_to_the_trigger(self):
        t = [grep_call(), tool_result("t_grep", "x"), context("t_grep", HINT), edit_call(msg="m2")]
        row = run("A1", t, [ev("trigger", "t_grep", carrier="context"), ev("stop", "t_edit")])
        self.assertTrue(row["arrival_ok"], row["arrival_detail"])

    def test_context_arm_fails_when_the_attachment_is_keyed_to_another_call(self):
        t = [grep_call(), tool_result("t_grep", "x"), context("t_other", HINT), edit_call(msg="m2")]
        row = run("A1", t, [ev("trigger", "t_grep", carrier="context"), ev("stop", "t_edit")])
        self.assertFalse(row["arrival_ok"])
        self.assertFalse(row["valid"])

    def test_context_arm_fails_when_the_hint_also_leaks_into_the_result(self):
        t = [grep_call(), tool_result("t_grep", "x", HINT), context("t_grep", HINT), edit_call(msg="m2")]
        row = run("A1", t, [ev("trigger", "t_grep", carrier="context"), ev("stop", "t_edit")])
        self.assertFalse(row["arrival_ok"])

    def test_P_needs_the_directive_not_the_hint(self):
        t = [grep_call(), tool_result("t_grep", "x"), context("t_grep", HINT), edit_call(msg="m2")]
        row = run("P", t, [ev("trigger", "t_grep", carrier="context"), ev("stop", "t_edit")])
        self.assertFalse(row["arrival_ok"])
        t[2] = context("t_grep", DIRECTIVE)
        self.assertTrue(run("P", t, [ev("trigger", "t_grep", carrier="context"), ev("stop", "t_edit")])["arrival_ok"])

    def test_result_arm_needs_the_hint_in_the_triggers_result_and_not_in_context(self):
        ok = [grep_call(), tool_result("t_grep", "x", HINT), edit_call(msg="m2")]
        self.assertTrue(run("A0", ok, [ev("trigger", "t_grep", carrier="result"), ev("stop", "t_edit")])["arrival_ok"])
        leak = ok[:2] + [context("t_grep", HINT)] + ok[2:]
        self.assertFalse(run("A0", leak, [ev("trigger", "t_grep", carrier="result"), ev("stop", "t_edit")])["arrival_ok"])

    def test_L_needs_the_label(self):
        plain = [grep_call(), tool_result("t_grep", "x", HINT), edit_call(msg="m2")]
        self.assertFalse(run("L", plain, [ev("trigger", "t_grep", carrier="result"), ev("stop", "t_edit")])["arrival_ok"])
        labelled = [grep_call(), tool_result("t_grep", "x", "[harness] " + HINT), edit_call(msg="m2")]
        self.assertTrue(run("L", labelled, [ev("trigger", "t_grep", carrier="result"), ev("stop", "t_edit")])["arrival_ok"])

    def test_floor_fails_if_the_hint_appears_anywhere(self):
        t = [grep_call(), tool_result("t_grep", "x", HINT), edit_call(msg="m2")]
        self.assertFalse(run("F", t, [ev("trigger", "t_grep", carrier="none"), ev("stop", "t_edit")])["arrival_ok"])

    def test_A2_also_needs_the_declaration_in_its_result(self):
        act = tool_use("m0", "t_act", "mcp__codescout__workspace", {"action": "activate"})
        base = [grep_call(), tool_result("t_grep", "x"), context("t_grep", HINT), edit_call(msg="m2")]
        events = [ev("declaration", "t_act"), ev("trigger", "t_grep", carrier="context"), ev("stop", "t_edit")]
        without = [act, tool_result("t_act", "ok")] + base
        self.assertFalse(run("A2", without, events)["arrival_ok"])
        with_decl = [act, tool_result("t_act", "ok", hook.DECLARATION)] + base
        self.assertTrue(run("A2", with_decl, events)["arrival_ok"])


class TestLateLookup(unittest.TestCase):
    """AM-8's primary metric: a lookup AFTER the trigger's batch, in a run with none at or before it."""

    def test_a_lookup_in_the_trigger_batch_is_pre_hint_and_not_informative(self):
        t = [grep_call(msg="m1"), refs_call(msg="m1"), tool_result("t_grep", "x", HINT),
             tool_result("t_refs", "y"), edit_call(msg="m2")]
        row = run("A0", t, [ev("trigger", "t_grep", carrier="result"), ev("stop", "t_edit")])
        self.assertTrue(row["pre_hint_lookup"])
        self.assertFalse(row["informative"])
        self.assertFalse(row["late_lookup"])

    def test_a_lookup_before_the_trigger_is_pre_hint(self):
        t = [refs_call(msg="m0", tid="t_r0"), tool_result("t_r0", "y"), grep_call(msg="m1"),
             tool_result("t_grep", "x", HINT), edit_call(msg="m2")]
        row = run("A0", t, [ev("trigger", "t_grep", carrier="result"), ev("stop", "t_edit")])
        self.assertTrue(row["pre_hint_lookup"])
        self.assertFalse(row["informative"])

    def test_a_lookup_after_the_trigger_is_late_and_informative(self):
        t = [grep_call(msg="m1"), tool_result("t_grep", "x", HINT), refs_call(msg="m2"),
             tool_result("t_refs", "y"), edit_call(msg="m3")]
        row = run("A0", t, [ev("trigger", "t_grep", carrier="result"), ev("stop", "t_edit")])
        self.assertEqual((row["pre_hint_lookup"], row["informative"], row["late_lookup"]), (False, True, True))

    def test_no_lookup_at_all_is_informative_and_not_late(self):
        t = [grep_call(msg="m1"), tool_result("t_grep", "x", HINT), edit_call(msg="m2")]
        row = run("A0", t, [ev("trigger", "t_grep", carrier="result"), ev("stop", "t_edit")])
        self.assertEqual((row["pre_hint_lookup"], row["informative"], row["late_lookup"]), (False, True, False))

    def test_a_lookup_in_the_stop_batch_is_not_late(self):
        t = [grep_call(msg="m1"), tool_result("t_grep", "x", HINT), refs_call(msg="m2"), edit_call(msg="m2")]
        row = run("A0", t, [ev("trigger", "t_grep", carrier="result"), ev("stop", "t_edit")])
        self.assertEqual((row["informative"], row["late_lookup"]), (True, False))

    def test_a_pre_hint_lookup_cancels_a_later_one(self):
        t = [grep_call(msg="m1"), refs_call(msg="m1"), tool_result("t_grep", "x", HINT),
             tool_result("t_refs", "y"), refs_call(msg="m2", tid="t_refs2"), tool_result("t_refs2", "y"),
             edit_call(msg="m3")]
        row = run("A0", t, [ev("trigger", "t_grep", carrier="result"), ev("stop", "t_edit")])
        self.assertEqual((row["pre_hint_lookup"], row["informative"], row["late_lookup"]), (True, False, False))


    def test_an_invalid_run_is_never_informative(self):
        t = [grep_call(msg="m1"), tool_result("t_grep", "x", HINT), refs_call(msg="m2"), edit_call(msg="m3")]
        row = run("A0", t, [ev("trigger", "t_grep", carrier="result"), ev("stop", "t_edit")], init_tools=None)
        self.assertFalse(row["informative"])

    def test_a_lookup_on_another_symbol_is_neither_pre_hint_nor_late(self):
        other = tool_use("m1", "t_o", "mcp__codescout__references", {"symbol": "ensure_resident_bytes"})
        t = [grep_call(msg="m1"), other, tool_result("t_grep", "x", HINT), edit_call(msg="m2")]
        row = run("A0", t, [ev("trigger", "t_grep", carrier="result"), ev("stop", "t_edit")])
        self.assertEqual((row["pre_hint_lookup"], row["informative"]), (False, True))



DECOY, DECOY_PATH = "store_background", "src/tools/output_buffer.rs"
WRONG = hook.wrong_hint_text(FN, DECOY, DECOY_PATH)


def decoy_call(msg, tid="t_decoy"):
    return tool_use(msg, tid, "mcp__codescout__references", {"symbol": DECOY, "path": DECOY_PATH})


def wrun(arm, transcript, events):
    return score.score_run(transcript, events, {"total_cost_usd": 0.1, "num_turns": 4}, arm=arm, fn=FN,
                           path=PATH, init_tools=INIT_TOOLS, decoy=DECOY, decoy_path=DECOY_PATH)


class TestWrongHintScoring(unittest.TestCase):
    EV = [{"event": "trigger", "tool_use_id": "t_grep", "carrier": "context"},
          {"event": "stop", "tool_use_id": "t_edit"}]

    def test_W_arrival_needs_the_wrong_hint_keyed_to_the_trigger(self):
        t = [grep_call(), tool_result("t_grep", "x"), context("t_grep", WRONG), edit_call(msg="m2")]
        self.assertTrue(wrun("W1", t, self.EV)["arrival_ok"])
        t[2] = context("t_grep", HINT)
        self.assertFalse(wrun("W1", t, self.EV)["arrival_ok"])

    def test_W_arrival_fails_if_the_right_hint_also_appears(self):
        t = [grep_call(), tool_result("t_grep", "x", HINT), context("t_grep", WRONG), edit_call(msg="m2")]
        self.assertFalse(wrun("W1", t, self.EV)["arrival_ok"])

    def test_W_arrival_fails_if_the_wrong_hint_also_leaks_into_a_result(self):
        t = [grep_call(), tool_result("t_grep", "x", WRONG), context("t_grep", WRONG), edit_call(msg="m2")]
        self.assertFalse(wrun("W1", t, self.EV)["arrival_ok"])

    def test_a_lookup_on_the_target_is_not_a_decoy_lookup(self):
        t = [grep_call(), tool_result("t_grep", "x"), context("t_grep", WRONG), refs_call(msg="m2"),
             tool_result("t_refs", "y"), edit_call(msg="m3")]
        row = wrun("W1", t, self.EV)
        self.assertEqual((row["decoy_late_lookup"], row["late_lookup"]), (False, True))


    def test_W2_also_needs_the_declaration(self):
        act = tool_use("m0", "t_act", "mcp__codescout__workspace", {"action": "activate"})
        base = [grep_call(), tool_result("t_grep", "x"), context("t_grep", WRONG), edit_call(msg="m2")]
        ev = [{"event": "declaration", "tool_use_id": "t_act"}] + self.EV
        self.assertFalse(wrun("W2", [act, tool_result("t_act", "ok")] + base, ev)["arrival_ok"])
        self.assertTrue(wrun("W2", [act, tool_result("t_act", "ok", hook.DECLARATION)] + base, ev)["arrival_ok"])

    def test_a_decoy_lookup_after_the_trigger_is_counted(self):
        t = [grep_call(), tool_result("t_grep", "x"), context("t_grep", WRONG), decoy_call("m2"),
             tool_result("t_decoy", "y"), edit_call(msg="m3")]
        row = wrun("W1", t, self.EV)
        self.assertEqual((row["decoy_late_lookup"], row["decoy_pre_hint"]), (True, False))

    def test_a_decoy_lookup_in_the_trigger_batch_is_pre_hint_not_late(self):
        t = [grep_call(), decoy_call("m1"), tool_result("t_grep", "x"), context("t_grep", WRONG),
             edit_call(msg="m2")]
        row = wrun("W1", t, self.EV)
        self.assertEqual((row["decoy_late_lookup"], row["decoy_pre_hint"]), (False, True))

    def test_a_decoy_lookup_in_the_stop_batch_is_not_late(self):
        t = [grep_call(), tool_result("t_grep", "x"), context("t_grep", WRONG), decoy_call("m2"),
             edit_call(msg="m2")]
        self.assertFalse(wrun("W1", t, self.EV)["decoy_late_lookup"])

    def test_the_decoy_is_measured_in_A_arms_too_as_the_floor(self):
        t = [grep_call(), tool_result("t_grep", "x"), context("t_grep", HINT), decoy_call("m2"),
             tool_result("t_decoy", "y"), edit_call(msg="m3")]
        row = wrun("A1", t, self.EV)
        self.assertTrue(row["arrival_ok"])
        self.assertTrue(row["decoy_late_lookup"])

    def test_without_a_decoy_the_decoy_fields_are_null(self):
        t = [grep_call(), tool_result("t_grep", "x", HINT), edit_call(msg="m2")]
        row = run("A0", t, [{"event": "trigger", "tool_use_id": "t_grep", "carrier": "result"},
                            {"event": "stop", "tool_use_id": "t_edit"}])
        self.assertIsNone(row["decoy_late_lookup"])

    def test_an_invalid_run_never_counts_a_decoy_lookup(self):
        t = [grep_call(), tool_result("t_grep", "x"), context("t_grep", HINT), decoy_call("m2"),
             tool_result("t_decoy", "y"), edit_call(msg="m3")]
        row = score.score_run(t, self.EV, {}, arm="A1", fn=FN, path=PATH, init_tools=None,
                              decoy=DECOY, decoy_path=DECOY_PATH)
        self.assertFalse(row["decoy_late_lookup"])



class TestValidity(unittest.TestCase):
    def test_a_run_without_references_in_its_tool_list_is_invalid(self):
        # Stage 1's first pass: the server listed 18 tools, references among the missing (AM-6).
        t = [grep_call(), tool_result("t_grep", "x", HINT), edit_call(msg="m2")]
        events = [ev("trigger", "t_grep", carrier="result"), ev("stop", "t_edit")]
        row = run("A0", t, events, init_tools=["mcp__codescout__grep", "mcp__codescout__call_graph"])
        self.assertFalse(row["lookup_available"])
        self.assertFalse(row["valid"])
        self.assertTrue(run("A0", t, events)["lookup_available"])

    def test_an_unknown_tool_list_is_invalid(self):
        t = [grep_call(), tool_result("t_grep", "x", HINT), edit_call(msg="m2")]
        row = run("A0", t, [ev("trigger", "t_grep", carrier="result"), ev("stop", "t_edit")], init_tools=None)
        self.assertFalse(row["valid"])


    def test_no_trigger_is_unexposed_and_invalid(self):
        row = run("A1", [edit_call(msg="m1")], [ev("stop", "t_edit")])
        self.assertEqual(row["exposure"], "unexposed")
        self.assertFalse(row["valid"])

    def test_a_trigger_in_the_stop_batch_is_late_and_invalid(self):
        t = [tool_use("m1", "t_s", "mcp__codescout__symbols", {"name": FN}), edit_call(msg="m1"),
             tool_result("t_s", "x"), context("t_s", HINT)]
        row = run("A1", t, [ev("trigger", "t_s", carrier="context"), ev("stop", "t_edit")])
        self.assertEqual(row["exposure"], "late")
        self.assertFalse(row["valid"])

    def test_no_stop_is_recorded_and_invalid(self):
        t = [grep_call(), tool_result("t_grep", "x"), context("t_grep", HINT)]
        row = run("A1", t, [ev("trigger", "t_grep", carrier="context")])
        self.assertEqual(row["outcome"], "no_stop")
        self.assertFalse(row["valid"])

    def test_cost_and_turns_are_copied_from_the_result(self):
        t = [grep_call(), tool_result("t_grep", "x", HINT), edit_call(msg="m2")]
        row = run("A0", t, [ev("trigger", "t_grep", carrier="result"), ev("stop", "t_edit")],
                  {"total_cost_usd": 0.31, "num_turns": 9})
        self.assertEqual((row["cost_usd"], row["num_turns"]), (0.31, 9))

    def test_a_hook_error_event_invalidates_the_run(self):
        t = [grep_call(), tool_result("t_grep", "x", HINT), edit_call(msg="m2")]
        row = run("A0", t, [ev("trigger", "t_grep", carrier="result"), ev("error", None, why="x"), ev("stop", "t_edit")])
        self.assertFalse(row["valid"])


if __name__ == "__main__":
    unittest.main()
