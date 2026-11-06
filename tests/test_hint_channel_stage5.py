"""Tests for tools/hint-channel-eval/stage5.py: the scorer of the product-path run, the exact test, the plan.

Synthetic transcripts use the shapes of score.parse: assistant entries sharing message.id form a batch,
tool results come in user entries, and the hook's context is an attachment entry keyed by toolUseID.

    python3 -I tests/test_hint_channel_stage5.py
"""
import importlib.util
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


stage5 = load("hint_channel_stage5", EVAL / "stage5.py")
score = stage5.score

FN, PATH = "ensure_resident", "src/agent/mod.rs"
RELOCATED = f'[cs-verified] verify callers: references(symbol="{FN}", path="{PATH}")'
PLAIN_RESULT = '{"status": "ok", "replaced_lines": "10-20", "rel_path": "src/agent/mod.rs"}'
ADVICE_RESULT = '{"status": "ok", "advice": {"text": "verify callers", "verified": true}}'


def use(msg, tid, name, inp):
    return {"type": "assistant", "message": {"id": msg, "content": [
        {"type": "tool_use", "id": tid, "name": name, "input": inp}]}}


def result(tid, *texts):
    return {"type": "user", "message": {"content": [
        {"type": "tool_result", "tool_use_id": tid, "content": [{"type": "text", "text": t} for t in texts]}]}}


def context(tid, text):
    return {"type": "attachment", "attachment": {"type": "hook_additional_context", "toolUseID": tid,
                                                 "hookEvent": "PostToolUse", "content": [text]}}


def replace_call(msg="m1", tid="t_edit", symbol=f"Agent/{FN}", path=PATH):
    return use(msg, tid, "mcp__codescout__edit_code",
               {"action": "replace", "symbol": symbol, "path": path, "body": "fn x() {}"})


def refs_call(msg, tid="t_refs", symbol=FN):
    return use(msg, tid, "mcp__codescout__references", {"symbol": symbol, "path": PATH})


def edit_again(msg, tid="t_edit2"):
    return use(msg, tid, "mcp__codescout__edit_code",
               {"action": "replace", "symbol": "Other/caller", "path": "src/server.rs", "body": "fn y() {}"})


def relocated_edit(msg="m1", tid="t_edit"):
    return [replace_call(msg, tid), result(tid, PLAIN_RESULT), context(tid, RELOCATED)]


def run(transcript, arm="A", raw_text=""):
    calls, results, contexts = score.parse(transcript)
    return stage5.score_calls(calls, results, contexts, fn=FN, path=PATH, arm=arm, raw_text=raw_text)


class TestPrimaryMetric(unittest.TestCase):
    def test_references_in_a_later_batch_before_the_next_edit_counts(self):
        row = run(relocated_edit() + [refs_call("m2"), edit_again("m3")])
        self.assertTrue(row["informative"])
        self.assertTrue(row["primary"])
        self.assertEqual(row["lookup_tool"], "mcp__codescout__references")

    def test_references_in_the_same_batch_as_the_edit_does_not_count(self):
        row = run(relocated_edit() + [refs_call("m1")])
        self.assertTrue(row["informative"])
        self.assertFalse(row["primary"])
        self.assertFalse(row["secondary"])

    def test_references_after_the_next_edit_is_secondary_only(self):
        row = run(relocated_edit() + [edit_again("m2"), refs_call("m3")])
        self.assertFalse(row["primary"])
        self.assertTrue(row["secondary"])

    def test_references_in_the_batch_of_the_next_edit_does_not_count_as_primary(self):
        row = run(relocated_edit() + [refs_call("m2"), edit_again("m2")])
        self.assertFalse(row["primary"])
        self.assertTrue(row["secondary"])

    def test_references_on_another_symbol_does_not_count(self):
        row = run(relocated_edit() + [refs_call("m2", symbol="note_activation"), edit_again("m3")])
        self.assertFalse(row["primary"])
        self.assertFalse(row["secondary"])

    def test_a_longer_name_that_contains_the_fn_does_not_count(self):
        row = run(relocated_edit() + [refs_call("m2", symbol=FN + "_all"), edit_again("m3")])
        self.assertFalse(row["primary"])

    def test_call_graph_counts(self):
        cg = use("m2", "t_cg", "mcp__codescout__call_graph", {"symbol": FN, "path": PATH, "direction": "callers"})
        self.assertTrue(run(relocated_edit() + [cg, edit_again("m3")])["primary"])

    def test_symbol_at_counts_only_on_the_target_path(self):
        on = use("m2", "t_at", "mcp__codescout__symbol_at", {"path": PATH, "line": 12, "column": 4})
        off = use("m2", "t_at", "mcp__codescout__symbol_at", {"path": "src/server.rs", "line": 12, "column": 4})
        self.assertTrue(run(relocated_edit() + [on, edit_again("m3")])["primary"])
        self.assertFalse(run(relocated_edit() + [off, edit_again("m3")])["primary"])

    def test_a_native_grep_is_not_a_lookup(self):
        grep = use("m2", "t_g", "Grep", {"pattern": FN})
        row = run(relocated_edit() + [grep, edit_again("m3")])
        self.assertFalse(row["primary"])
        self.assertFalse(row["secondary"])

    def test_no_mutation_after_the_trigger_leaves_the_window_open(self):
        self.assertTrue(run(relocated_edit() + [refs_call("m2")])["primary"])


class TestInformative(unittest.TestCase):
    def test_a_replace_whose_advice_stayed_in_the_result_is_not_informative(self):
        t = [replace_call(), result("t_edit", ADVICE_RESULT)]
        row = run(t)
        self.assertFalse(row["informative"])
        self.assertEqual(row["reason"], "advice_not_relocated")
        self.assertIsNone(row["primary"])

    def test_a_replace_with_neither_advice_nor_context_is_no_advice(self):
        row = run([replace_call(), result("t_edit", PLAIN_RESULT)])
        self.assertEqual(row["reason"], "no_advice")

    def test_context_keyed_to_another_call_does_not_make_the_run_informative(self):
        t = [replace_call(), result("t_edit", PLAIN_RESULT), context("t_other", RELOCATED)]
        self.assertFalse(run(t)["informative"])

    def test_context_without_the_tag_does_not_make_the_run_informative(self):
        t = [replace_call(), result("t_edit", PLAIN_RESULT), context("t_edit", "verify callers: references(...)")]
        self.assertFalse(run(t)["informative"])

    def test_no_replace_on_the_target(self):
        other = [replace_call(symbol="Other/fn", path="src/server.rs"), result("t_edit", PLAIN_RESULT)]
        self.assertEqual(run(other)["reason"], "no_edit_code_replace_on_target")
        self.assertEqual(run([])["reason"], "no_edit_code_replace_on_target")

    def test_a_native_edit_of_the_target_is_counted_but_not_a_trigger(self):
        t = [use("m1", "t_n", "Edit", {"file_path": "/testbed/" + PATH, "old_string": "a", "new_string": "b"})]
        row = run(t)
        self.assertEqual(row["native_edit_on_target"], 1)
        self.assertFalse(row["informative"])

    def test_the_first_relocated_replace_is_the_trigger_when_an_earlier_one_was_not(self):
        t = [replace_call("m1", "t_a"), result("t_a", PLAIN_RESULT)] + relocated_edit("m2", "t_b") + [refs_call("m3")]
        row = run(t)
        self.assertTrue(row["informative"])
        self.assertTrue(row["primary"])

    def test_the_declaration_in_an_arm_B_run_voids_it(self):
        row = run(relocated_edit(), arm="B", raw_text="x " + stage5.DECLARATION_MARK + " y")
        self.assertFalse(row["informative"])
        self.assertEqual(row["reason"], "leak_declaration_in_arm_B")

    def test_the_declaration_in_an_arm_A_run_is_expected(self):
        row = run(relocated_edit(), arm="A", raw_text=stage5.DECLARATION)
        self.assertTrue(row["informative"])
        self.assertTrue(row["decl_in_text"])

    def test_the_declaration_mark_is_inside_the_declaration_and_outside_the_hint(self):
        self.assertIn(stage5.DECLARATION_MARK, stage5.DECLARATION)
        self.assertNotIn(stage5.DECLARATION_MARK, RELOCATED)


class TestPriorLookup(unittest.TestCase):
    def test_a_references_call_before_the_edit_marks_a_prior_lookup(self):
        t = [refs_call("m0", "t_pre")] + relocated_edit("m1", "t_edit") + [edit_again("m2")]
        row = run(t)
        self.assertTrue(row["prior_lookup"])
        self.assertFalse(row["primary"])

    def test_a_native_grep_naming_the_fn_before_the_edit_marks_a_prior_lookup(self):
        pre = use("m0", "t_g", "Grep", {"pattern": FN + r"\("})
        self.assertTrue(run([pre] + relocated_edit("m1", "t_edit"))["prior_lookup"])

    def test_a_bash_grep_naming_the_fn_marks_a_prior_lookup_but_a_sed_does_not(self):
        g = use("m0", "t_b", "Bash", {"command": f"grep -rn {FN} src"})
        s = use("m0", "t_b", "Bash", {"command": f"sed -n 1,5p src/agent/mod.rs # {FN}"})
        self.assertTrue(run([g] + relocated_edit("m1", "t_edit"))["prior_lookup"])
        self.assertFalse(run([s] + relocated_edit("m1", "t_edit"))["prior_lookup"])
    def test_a_bash_grep_for_another_name_is_not_a_prior_lookup(self):
        g = use("m0", "t_b", "Bash", {"command": "grep -rn note_activation src"})
        self.assertFalse(run([g] + relocated_edit("m1", "t_edit"))["prior_lookup"])


    def test_a_grep_for_another_name_is_not_a_prior_lookup(self):
        pre = use("m0", "t_g", "mcp__codescout__grep", {"pattern": "note_activation"})
        self.assertFalse(run([pre] + relocated_edit("m1", "t_edit"))["prior_lookup"])

    def test_a_lookup_in_the_batch_of_the_trigger_is_a_prior_lookup(self):
        self.assertTrue(run(relocated_edit("m1", "t_edit") + [refs_call("m1")])["prior_lookup"])

    def test_a_lookup_after_the_trigger_is_not_a_prior_lookup(self):
        row = run(relocated_edit() + [refs_call("m2")])
        self.assertFalse(row["prior_lookup"])
        self.assertTrue(row["primary"])

    def test_the_trigger_itself_is_not_a_prior_lookup(self):
        self.assertFalse(run(relocated_edit())["prior_lookup"])

    def test_an_uninformative_run_has_no_prior_lookup_value(self):
        self.assertIsNone(run([replace_call(), result("t_edit", PLAIN_RESULT)])["prior_lookup"])


class TestStratum(unittest.TestCase):
    def row(self, task, arm, primary, prior):
        return {"task_id": task, "arm": arm, "informative": True, "reason": None, "primary": primary,
                "secondary": primary, "prior_lookup": prior}

    def test_the_stratum_leaves_out_runs_with_a_prior_lookup(self):
        rows = [self.row("t1", "A", True, False), self.row("t1", "B", False, False),
                self.row("t1", "A", False, True), self.row("t1", "B", False, True)]
        out = stage5.analyze(rows)
        self.assertEqual(out["informative"], 4)
        self.assertEqual(out["no_prior_lookup"]["informative"], 2)
        self.assertEqual(out["no_prior_lookup"]["per_arm_n_primary_secondary"]["A"], [1, 1, 1])
        self.assertEqual(out["prior_lookup_by_arm"], {"A": 1, "B": 1})

    def test_the_primary_result_does_not_depend_on_the_stratum(self):
        rows = [self.row("t1", "A", True, True), self.row("t1", "B", False, True)]
        out = stage5.analyze(rows)
        self.assertEqual(out["per_arm_n_primary_secondary"]["A"], [1, 1, 1])
        self.assertEqual(out["no_prior_lookup"]["informative"], 0)
        self.assertIsNone(out["no_prior_lookup"]["stratified_exact_one_sided_p"])



class TestExactTest(unittest.TestCase):
    def test_one_stratum_all_successes_in_A(self):
        p, obs = stage5.stratified_exact_p({"t": (2, 2, 2, 0)})
        self.assertEqual(obs, 2)
        self.assertAlmostEqual(p, 1 / 6)

    def test_two_strata_multiply(self):
        p, obs = stage5.stratified_exact_p({"t1": (1, 1, 1, 0), "t2": (1, 1, 1, 0)})
        self.assertEqual(obs, 2)
        self.assertAlmostEqual(p, 0.25)

    def test_a_stratum_with_no_difference_adds_nothing(self):
        p1, _ = stage5.stratified_exact_p({"t": (2, 2, 2, 0)})
        p2, _ = stage5.stratified_exact_p({"t": (2, 2, 2, 0), "u": (3, 3, 3, 3)})
        self.assertAlmostEqual(p1, p2)

    def test_no_effect_gives_a_large_p(self):
        p, _ = stage5.stratified_exact_p({"t": (4, 2, 4, 2)})
        self.assertGreater(p, 0.5)

    def test_B_better_than_A_gives_p_near_one(self):
        p, _ = stage5.stratified_exact_p({"t": (3, 0, 3, 3)})
        self.assertAlmostEqual(p, 1.0)

    def test_the_pmf_sums_to_one(self):
        self.assertAlmostEqual(sum(stage5.hypergeom_pmf(9, 4, 5).values()), 1.0)


class TestAnalyze(unittest.TestCase):
    def row(self, task, arm, primary, informative=True, reason=None):
        return {"task_id": task, "arm": arm, "informative": informative, "reason": reason,
                "primary": primary, "secondary": primary}

    def test_only_informative_rows_enter_the_counts_and_reasons_are_tallied(self):
        rows = [self.row("t1", "A", True), self.row("t1", "B", False), self.row("t1", "A", None, False, "no_advice")]
        out = stage5.analyze(rows)
        self.assertEqual(out["informative"], 2)
        self.assertEqual(out["per_arm_n_primary_secondary"]["A"][0], 1)
        self.assertEqual(out["not_informative_by_reason"], {"no_advice": 1})

    def test_a_task_seen_in_one_arm_only_is_left_out_of_the_test(self):
        rows = [self.row("t1", "A", True), self.row("t2", "A", True), self.row("t2", "B", False)]
        out = stage5.analyze(rows)
        self.assertEqual(out["tasks_with_both_arms"], 1)
        self.assertAlmostEqual(out["stratified_exact_one_sided_p"], 0.5)

    def test_no_rows(self):
        out = stage5.analyze([])
        self.assertIsNone(out["stratified_exact_one_sided_p"])


class TestPlan(unittest.TestCase):
    def test_a_round_has_each_task_once_per_arm_and_is_reproducible(self):
        p = stage5.plan(1)
        self.assertEqual(len(p), 2 * len(stage5.FIRING_TASKS))
        self.assertEqual(sorted(p), sorted((t, a, 1) for t in stage5.FIRING_TASKS for a in stage5.ARMS5))
        self.assertEqual(p, stage5.plan(1))

    def test_arms_of_one_task_sit_next_to_each_other(self):
        p = stage5.plan(2)
        for i in range(0, len(p), 2):
            self.assertEqual(p[i][0], p[i + 1][0])
            self.assertEqual({p[i][1], p[i + 1][1]}, set(stage5.ARMS5))

    def test_rounds_differ(self):
        self.assertNotEqual(stage5.plan(1), stage5.plan(2))


if __name__ == "__main__":
    unittest.main()
