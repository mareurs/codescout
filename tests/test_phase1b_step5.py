"""Phase-1b Stage 2's Step 5: step5_gate.py's gate texts, judge, determinism and parity checks, the adapter
over phase1-span-selector.py's gate and span gate, the refusals before the gate, the reservation and
crash record, and the registered readings. The model is faked; the gate code, the segmenter and the
registration are the real ones. Needs torch and sklearn: run with the training venv,
    ~/work/claude/jevk5/.venv/bin/python tests/test_phase1b_step5.py
"""
import argparse
import contextlib
import hashlib
import importlib.util
import io
import json
import math
import pathlib
import subprocess
import tempfile
import types
import unittest
from unittest import mock

ROOT = pathlib.Path(__file__).resolve().parents[1]
PHASE1B = ROOT / "docs/evals/data/2026-09-24-rule-tell/phase1b"


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


g = load("step5_gate", PHASE1B / "step5_gate.py")
# sel.sweep retries an errored row three times, sleeping 2, 4 and 8 s. The retries still run; only the
# wait goes, so a case (or a mutation) that errors on every row fails in milliseconds, not minutes.
g.sel.time = types.SimpleNamespace(sleep=lambda s: None)

# Column order of every fake logit row. LOAD-BEARING for the head-order cases: a judge built on a
# rotation of this list reads another head's column for every rule, which a reversal would not do for
# the middle one.
HEADS = ["d_red", "d_semicolon", "d_sessionid"]
TEMPS = {h: {"T": 1.0} for h in HEADS}
# recall90_t sits far below precision_t so a judge reading the wrong key fires where it should not.
THR = {h: {"precision_t": 0.9, "recall90_t": 0.05} for h in HEADS}

POS_SEMI = "I chained the two lanes with one command. The default lane ran after the lean one."
CLEAN_RED = "A red flag sentence lives here. Another plain sentence follows it."
CLEAN = "The build copies the assets into dist. It then writes a manifest file."
MEMBER = "The member sentence is long enough here. It says nothing else at all."
SEMI_UNIT = "cargo test --no-default-features && cargo test"      # phase 1's semicolon and span-semicolon unit
SESSION_UNIT = "The commit came from codescout-26"                   # phase 1's sessionid and span-sessionid unit
CUES = [("chained the two lanes", "d_semicolon", 5.0), ("red flag sentence", "d_red", 5.0),
        (SESSION_UNIT, "d_sessionid", 5.0), (SEMI_UNIT, "d_semicolon", 5.0)]
TEXTS = [("pos-semi", POS_SEMI, "d_semicolon"), ("clean-red", CLEAN_RED, "none"),
         ("clean-plain", CLEAN, "none"), ("pos-member", MEMBER, "member_vs_population")]
MENU = ["d_semicolon", "d_sessionid"]          # gate-able: two gate positives and both span texts apply


def fake_logits(cues, heads=HEADS, default=-20.0):
    """unit_logits over `heads`: a unit containing a cue's substring gets that cue's logit for its head."""
    def unit_logits(units):
        return [[next((z for s, h, z in cues if h == head and s in u), default) for head in heads] for u in units]
    return unit_logits


def judge(cues=CUES, heads=HEADS, temps=TEMPS, thr=THR, menu=HEADS):
    j = g.Judge(fake_logits(cues), heads, temps, thr)
    j.set_menu("test", menu)
    return j


def quiet(fn, *a):
    with contextlib.redirect_stdout(io.StringIO()):
        return fn(*a)


class GateTexts(unittest.TestCase):
    NEW = [("c6", "text six"), ("c7", "text seven"), ("c8", "text eight")]

    def test_phase1s_gate_is_its_ten_texts(self):
        self.assertEqual([c for c, _, _ in g.PHASE1_GATE],
                         ["clean-1", "clean-2", "semicolon", "sessionid", "cannot", "contradiction",
                          "clean-3", "clean-4", "clean-5", "member"])

    def test_survivors_follow_in_registration_order_expecting_no_rule(self):
        out = g.gate_texts(g.PHASE1_GATE, ["c8", "c6"], self.NEW)
        self.assertEqual(out[:10], g.PHASE1_GATE)
        self.assertEqual(out[10:], [("c6", "text six", "none"), ("c8", "text eight", "none")])

    def test_a_survivor_that_does_not_exist_is_refused(self):
        with self.assertRaises(SystemExit):
            g.gate_texts(g.PHASE1_GATE, ["c6", "c9"], self.NEW)

    def test_a_new_text_repeating_a_phase1_text_is_refused(self):
        with self.assertRaises(SystemExit):
            g.gate_texts(g.PHASE1_GATE, ["c6"], [("c6", g.PHASE1_GATE[0][1])])

    def test_a_new_id_repeating_a_phase1_id_is_refused(self):
        with self.assertRaises(SystemExit):
            g.gate_texts(g.PHASE1_GATE, ["clean-1"], [("clean-1", "a different text")])

    def test_a_phase1_gate_of_another_size_is_refused(self):
        with self.assertRaises(SystemExit):
            g.gate_texts(g.PHASE1_GATE[:9], [], self.NEW)


class JudgeRule(unittest.TestCase):
    def test_fires_at_p_equal_to_the_threshold_and_not_below(self):
        t = g.s4.sig(2.0)
        at = judge(cues=[("chained the two lanes", "d_semicolon", 2.0)],
                   thr={h: {"precision_t": t, "recall90_t": 0.0} for h in HEADS})
        self.assertEqual(at.judge_rule(POS_SEMI, "d_semicolon")["verdict"], "YES")
        below = judge(cues=[("chained the two lanes", "d_semicolon", 1.999)],
                      thr={h: {"precision_t": t, "recall90_t": 0.0} for h in HEADS})
        self.assertEqual(below.judge_rule(POS_SEMI, "d_semicolon")["verdict"], "NO")

    def test_the_temperature_divides_the_logit_in_both_directions(self):
        cues = [("chained the two lanes", "d_semicolon", 2.0)]     # sig(2) = 0.881, under 0.9
        sharper = judge(cues=cues, temps={h: {"T": 0.5} for h in HEADS})     # sig(4) = 0.982
        self.assertEqual(sharper.judge_rule(POS_SEMI, "d_semicolon")["verdict"], "YES")
        cues = [("chained the two lanes", "d_semicolon", 3.0)]     # sig(3) = 0.953, over 0.9
        flatter = judge(cues=cues, temps={h: {"T": 2.0} for h in HEADS})     # sig(1.5) = 0.818
        self.assertEqual(flatter.judge_rule(POS_SEMI, "d_semicolon")["verdict"], "NO")

    def test_reads_the_precision_threshold_not_the_recall_one(self):
        j = judge(cues=[("chained the two lanes", "d_semicolon", 1.0)])      # sig(1) = 0.731
        self.assertEqual(j.judge_rule(POS_SEMI, "d_semicolon")["verdict"], "NO")

    def test_each_rule_reads_its_own_head(self):
        j = judge()
        self.assertEqual(j.judge_rule(POS_SEMI, "d_semicolon")["verdict"], "YES")
        self.assertEqual(j.judge_rule(POS_SEMI, "d_sessionid")["verdict"], "NO")
        self.assertEqual(j.judge_rule(POS_SEMI, "d_red")["verdict"], "NO")

    def test_a_unit_shorter_than_min_span_is_never_a_claim(self):
        text = "Yes. The configuration file was read twice by the loader."
        # Fixture precondition: the first unit is under MIN_SPAN, the second over it.
        units = g.ta.segment(text)
        self.assertLess(len(units[0]), g.sel.MIN_SPAN)
        self.assertGreaterEqual(len(units[1]), g.sel.MIN_SPAN)
        only_short = judge(cues=[("Yes.", "d_red", 9.0)])
        self.assertEqual(only_short.judge_rule(text, "d_red")["verdict"], "NO")
        both = judge(cues=[("Yes.", "d_red", 9.0), ("configuration file", "d_red", 4.0)])
        self.assertEqual(both.judge_rule(text, "d_red")["claim"], units[1])

    def test_a_unit_of_exactly_min_span_is_a_candidate(self):
        text = "Twelve char. Another sentence that is longer follows it."
        self.assertEqual(len(g.ta.segment(text)[0]), g.sel.MIN_SPAN)      # fixture precondition
        j = judge(cues=[("Twelve char.", "d_red", 5.0)])
        self.assertEqual(j.judge_rule(text, "d_red")["claim"], "Twelve char.")

    def test_a_text_with_no_unit_long_enough_to_claim_is_a_no(self):
        text = "Yes. No. Fine."
        self.assertTrue(all(len(u) < g.sel.MIN_SPAN for u in g.ta.segment(text)))   # fixture precondition
        j = judge(cues=[("Yes.", "d_red", 9.0), ("No.", "d_red", 9.0), ("Fine.", "d_red", 9.0)])
        self.assertEqual(j.judge_rule(text, "d_red")["verdict"], "NO")

    def test_the_claim_is_the_highest_scoring_unit_verbatim(self):
        text = "The first long sentence is here. The second long sentence is here."
        j = judge(cues=[("first long", "d_red", 4.0), ("second long", "d_red", 6.0)])
        self.assertEqual(j.judge_rule(text, "d_red")["claim"], "The second long sentence is here.")

    def test_a_saturated_tie_is_broken_by_the_raw_logit(self):
        # Review finding 1, on the real span-semicolon text: at T = 0.25, z = 12 and z = 30 both give
        # P = 1.0 in floats, and the first unit (innocent) would win a tie on P.
        cid, rule, text, violating = g.PHASE1_SPAN[2]
        self.assertEqual(cid, "span-semicolon")
        j = judge(cues=[("lean lane builds", "d_semicolon", 12.0), (SEMI_UNIT, "d_semicolon", 30.0)],
                  temps={h: {"T": 0.25} for h in HEADS})
        self.assertEqual(g.s4.sig(12.0 / 0.25), g.s4.sig(30.0 / 0.25))      # fixture precondition: a tie
        r = j.judge_rule(text, "d_semicolon")
        self.assertTrue(g.sel.claim_on_target(r["claim"], violating))
        self.assertEqual(j.log[-1]["ties_at_p"], 2)
        self.assertEqual(j.log[-1]["z"], 30.0)

    def test_a_non_finite_logit_is_an_error_and_is_recorded(self):
        j = judge(cues=[("chained the two lanes", "d_semicolon", math.nan)])
        with self.assertRaises(ValueError):
            j.judge_rule(POS_SEMI, "d_semicolon")
        self.assertEqual([e["rule"] for e in j.errors], ["d_semicolon"])
        self.assertIn("non-finite", j.errors[0]["error"])

    def test_a_claim_verify_span_refuses_is_an_error_and_is_recorded(self):
        j = judge()
        with mock.patch.object(g.sel, "verify_span", return_value=None), self.assertRaises(ValueError):
            j.judge_rule(POS_SEMI, "d_semicolon")
        self.assertIn("verify_span", j.errors[0]["error"])
        self.assertEqual(j.log, [])

    def test_a_rule_off_the_menu_is_refused(self):
        j = judge(menu=["d_semicolon"])
        with self.assertRaises(ValueError):
            j.judge_rule(POS_SEMI, "d_red")

    def test_a_menu_head_without_a_temperature_is_refused(self):
        j = g.Judge(fake_logits(CUES), HEADS, {"d_red": {"T": 1.0}}, THR)
        with self.assertRaises(SystemExit):
            j.set_menu("m", ["d_red", "d_semicolon"])

    def test_a_menu_head_without_a_threshold_is_refused(self):
        j = g.Judge(fake_logits(CUES), HEADS, TEMPS, {"d_red": THR["d_red"]})
        with self.assertRaises(SystemExit):
            j.set_menu("m", ["d_red", "d_semicolon"])

    def test_a_menu_head_the_checkpoint_does_not_have_is_refused(self):
        # A temperature and a threshold exist, so only the head-index half of the check refuses.
        j = g.Judge(fake_logits(CUES), HEADS, {**TEMPS, "x": {"T": 1.0}}, {**THR, "x": THR["d_red"]})
        with self.assertRaises(SystemExit):
            j.set_menu("m", ["x"])


class Determinism(unittest.TestCase):
    def drifting(self, delta):
        calls = []
        def unit_logits(units):
            calls.append(1)
            # From 0.0, so the second call's drift is delta exactly: 0.5 + 1e-4 - 0.5 is not 1e-4 in floats.
            return [[delta * (len(calls) - 1)] * 3 for _ in units]
        return unit_logits

    def test_identical_scores_pass(self):
        self.assertTrue(g.determinism(self.drifting(0.0))["ok"])

    def test_drift_of_the_bound_fails_and_below_it_passes(self):
        self.assertFalse(g.determinism(self.drifting(1e-4))["ok"])
        self.assertTrue(g.determinism(self.drifting(5e-5))["ok"])

    def test_a_non_finite_logit_fails_even_when_it_repeats(self):
        res = g.determinism(lambda units: [[math.nan, 0.0, 0.0] for _ in units])
        self.assertFalse(res["ok"])
        self.assertFalse(res["finite"])


class Parity(unittest.TestCase):
    ROWS = [{"id": "v1", "text": POS_SEMI, "target": 0, "rule": "d_semicolon"}]

    def scored(self, own=5.0, cross=-20.0):
        return {"val": [{"id": "v1", "z": own}],
                "val_cross": [{"id": "v1", "unit": 1, "head": "d_red", "z": cross},
                              {"id": "v1", "unit": 0, "head": "d_sessionid", "z": -20.0}]}

    def test_equal_logits_pass_counting_own_and_cross_cells(self):
        res = g.parity(judge(), self.ROWS, self.scored())
        self.assertTrue(res["ok"])
        self.assertEqual(res["cells"], 3)

    def test_an_own_cell_off_by_any_amount_fails(self):
        self.assertFalse(g.parity(judge(), self.ROWS, self.scored(own=5.0 + 1e-12))["ok"])

    def test_a_cross_cell_off_by_any_amount_fails(self):
        self.assertFalse(g.parity(judge(), self.ROWS, self.scored(cross=-19.999))["ok"])

    def test_a_judge_on_another_head_order_fails(self):
        rotated = g.Judge(fake_logits(CUES), HEADS[1:] + HEADS[:1], TEMPS, THR)
        self.assertFalse(g.parity(rotated, self.ROWS, self.scored())["ok"])

    def test_no_rows_is_not_a_pass(self):
        self.assertFalse(g.parity(judge(), [], self.scored())["ok"])

    def test_a_frozen_row_missing_from_the_scored_file_is_refused(self):
        with self.assertRaises(SystemExit):
            g.parity(judge(), self.ROWS + [{"id": "v2", "text": CLEAN, "target": 0, "rule": "d_red"}],
                     self.scored())


class ReadMenu(unittest.TestCase):
    def read(self, menu, cues=CUES, texts=TEXTS):
        return quiet(g.read_menu, judge(cues=cues, menu=menu), "m", menu, texts)

    def test_a_head_off_the_menu_does_not_fail_a_clean_text(self):
        own = self.read(MENU)
        self.assertTrue(own["passed"])
        self.assertEqual(own["gate"], {"passed": 3, "of": 3, "errored": 0})
        wider = self.read(["d_red"] + MENU)
        self.assertFalse(wider["passed"])
        self.assertEqual(wider["gate_rc"], 1)
        self.assertEqual(next(e for e in wider["texts"] if e["id"] == "clean-red")["fired"], ["d_red"])

    def test_a_positive_whose_rule_is_off_the_menu_is_not_counted(self):
        res = self.read(MENU)
        member = next(e for e in res["texts"] if e["id"] == "pos-member")
        self.assertFalse(member["applicable"])
        self.assertNotIn("passed", member)

    def test_the_span_gate_keeps_only_menu_rules_and_passes_on_target(self):
        res = self.read(MENU)
        self.assertEqual([s["id"] for s in res["span_texts"]], ["span-sessionid", "span-semicolon"])
        self.assertTrue(all(s["on_target"] for s in res["span_texts"]))
        self.assertEqual(res["span_rc"], 0)

    def test_an_off_target_span_claim_fails_the_menu(self):
        cues = CUES + [("three commits on experiments", "d_sessionid", 9.0)]
        res = self.read(MENU, cues=cues)
        self.assertEqual(res["gate_rc"], 0)          # the main gate passes, so the span gate decides
        self.assertEqual(res["span_rc"], 1)
        self.assertFalse(res["passed"])

    def test_a_menu_that_is_not_gate_able_fails_though_both_gates_pass(self):
        res = self.read(["d_red", "d_semicolon"], texts=[("clean-plain", CLEAN, "none")])
        self.assertEqual((res["gate_rc"], res["span_rc"]), (0, 0))
        self.assertFalse(res["gate_ability"]["gate_able"])
        self.assertFalse(res["passed"])

    def test_a_menu_with_no_span_text_does_not_run_the_span_gate(self):
        res = self.read(["d_red"], texts=[("clean-plain", CLEAN, "none")])
        self.assertEqual(res["span_texts"], [])
        self.assertIsNone(res["span_rc"])       # not run: sel.span_gate reads SPAN_GATE[0]
        self.assertFalse(res["passed"])         # also not gate-able, which alone would fail it

    def test_an_errored_row_fails_the_menu_and_keeps_its_message(self):
        res = self.read(MENU, cues=[("chained the two lanes", "d_semicolon", math.nan)] + CUES)
        self.assertEqual(res["gate_rc"], 1)
        self.assertFalse(res["passed"])
        self.assertEqual(res["gate"]["errored"], 1)
        self.assertTrue(res["errors"] and all("non-finite" in e["error"] for e in res["errors"]))

    def test_one_run_on_one_thread(self):
        seen = []
        def gate(args):
            seen.append((args.runs, args.pool))
            return 0
        with mock.patch.object(g.sel, "gate", gate):
            self.read(MENU)
        self.assertEqual(seen, [(1, 1)])

    def test_the_gate_output_is_passed_on_as_it_is_printed(self):
        outer = io.StringIO()
        with contextlib.redirect_stdout(outer):
            res = g.read_menu(judge(menu=MENU), "m", MENU, TEXTS)
        self.assertIn("gate: 3/3   errored rows: 0", outer.getvalue())
        self.assertEqual(outer.getvalue(), res["output"])

    def test_a_gate_count_the_rows_do_not_support_is_refused(self):
        def lying_gate(args):
            # It judges nothing, so the rows re-derive 2 of 3 (the two clean texts): 3/3 is the lie.
            print("gate: 3/3   errored rows: 0")
            return 0
        with mock.patch.object(g.sel, "gate", lying_gate), self.assertRaises(SystemExit):
            self.read(MENU)

    def test_a_span_count_the_rows_do_not_support_is_refused(self):
        def lying_span(args):
            # It judges nothing, so the rows re-derive 0 of 2: 1/2 is the lie.
            print("span gate: 1/2   errored runs: 0")
            return 1
        with mock.patch.object(g.sel, "span_gate", lying_span), self.assertRaises(SystemExit):
            self.read(MENU)

    def test_rows_are_kept_per_menu(self):
        j = judge(menu=MENU)
        quiet(g.read_menu, j, "own", MENU, TEXTS)
        n = len(j.log)
        res = quiet(g.read_menu, j, "common", ["d_semicolon", "d_sessionid"], TEXTS[:1])
        self.assertEqual(len(res["rows"]), len(j.log) - n)
        self.assertTrue(all(r["menu"] == "common" for r in res["rows"]))


def result(own, common, learned=True, clean12=(), own_able=True, common_able=True, common_menu=("m",)):
    # The common menu's clean-12 row never fires, so a reading taken from the wrong menu miscounts.
    def texts(fired):
        return [{"id": "clean-12", "fired": list(fired), "p": {"d_semicolon": 0.5}}]
    return {"status": "completed", "learned": learned, "common": {"sha256": "c" * 64}, "gate_texts": ["clean-12"],
            "menus": {"own": {"passed": own, "texts": texts(clean12), "gate_ability": {"gate_able": own_able}},
                      "common": {"passed": common, "texts": texts(()), "menu": list(common_menu),
                                 "gate_ability": {"gate_able": common_able}}}}


def nine(**over):
    """B fails everywhere, N and NC pass everywhere, unless a name is overridden."""
    base = {f"{a}-{s}": result(a != "b", a != "b") for a in g.ARMS for s in g.SEEDS}
    base.update(over)
    return base


class Summary(unittest.TestCase):
    def test_nc_proceeds_only_at_three_of_three_on_its_own_menus(self):
        self.assertEqual(g.summarize(nine())["ship"],
                         {"nc_own_passing": 3, "proceeds_to_t": True, "checkpoint": "nc-20260935"})
        two = g.summarize(nine(**{"nc-20260940": result(False, True)}))["ship"]
        self.assertEqual(two, {"nc_own_passing": 2, "proceeds_to_t": False, "checkpoint": None})

    def test_the_ship_rule_reads_own_menus_and_the_causal_claim_reads_the_common_menu(self):
        s = g.summarize(nine(**{"nc-20260935": result(True, False)}))
        self.assertTrue(s["ship"]["proceeds_to_t"])
        self.assertEqual(s["seeds_passing"]["nc"], {"own": 3, "common": 2})

    def test_a_seed_that_did_not_learn_fails_on_both_menus(self):
        s = g.summarize(nine(**{"nc-20260937": result(True, True, learned=False)}))
        self.assertEqual(s["seeds_passing"]["nc"], {"own": 2, "common": 2})
        self.assertFalse(s["ship"]["proceeds_to_t"])

    def test_the_causal_pattern_needs_n_at_two_and_b_at_none(self):
        self.assertTrue(g.summarize(nine(**{"n-20260937": result(True, False)}))["causal"].startswith("pattern holds"))
        self.assertEqual(g.summarize(nine(**{"n-20260937": result(True, False), "n-20260940": result(True, False)}))
                         ["causal"], "withheld")
        self.assertEqual(g.summarize(nine(**{"b-20260940": result(False, True)}))["causal"], "withheld")

    def test_a_common_menu_that_is_not_gate_able_makes_the_comparison_not_possible(self):
        # Every own menu is gate-able, so only a reading of the common menu's gate-ability says "not possible".
        s = g.summarize({k: result(v["menus"]["own"]["passed"], v["menus"]["common"]["passed"], common_able=False)
                         for k, v in nine().items()})
        self.assertTrue(s["causal"].startswith("not possible"))

    def test_prediction_1_needs_b_to_fail_on_both_menus_at_every_seed(self):
        self.assertTrue(g.summarize(nine())["predictions"]["1"])
        self.assertFalse(g.summarize(nine(**{"b-20260935": result(True, False)}))["predictions"]["1"])
        self.assertFalse(g.summarize(nine(**{"b-20260935": result(False, True)}))["predictions"]["1"])

    def test_predictions_4_and_5_count_seeds_firing_d_semicolon_on_clean_12(self):
        fire = ("d_semicolon",)
        two_n = nine(**{"n-20260935": result(True, True, clean12=fire), "n-20260937": result(True, True, clean12=fire)})
        self.assertTrue(g.summarize(two_n)["predictions"]["4"])
        one_n = nine(**{"n-20260935": result(True, True, clean12=fire), "n-20260937": result(True, True, clean12=("d_red",))})
        self.assertFalse(g.summarize(one_n)["predictions"]["4"])
        self.assertTrue(g.summarize(nine(**{"nc-20260935": result(True, True, clean12=fire)}))["predictions"]["5"])
        two_nc = nine(**{"nc-20260935": result(True, True, clean12=fire), "nc-20260940": result(True, True, clean12=fire)})
        self.assertFalse(g.summarize(two_nc)["predictions"]["5"])

    def test_clean_12_not_judged_for_d_semicolon_cannot_be_read(self):
        unread = result(True, True)
        unread["menus"]["own"]["texts"][0]["p"] = {}
        with self.assertRaises(SystemExit):
            g.summarize(nine(**{"n-20260937": unread}))

    def test_results_that_disagree_on_the_common_menu_or_texts_are_refused(self):
        with self.assertRaises(SystemExit):
            g.summarize(nine(**{"n-20260937": result(True, True, common_menu=("other",))}))
        other_texts = result(True, True)
        other_texts["gate_texts"] = ["clean-12", "clean-13"]
        with self.assertRaises(SystemExit):
            g.summarize(nine(**{"n-20260937": other_texts}))
        other_common = result(True, True)
        other_common["common"] = {"sha256": "d" * 64}
        with self.assertRaises(SystemExit):
            g.summarize(nine(**{"n-20260937": other_common}))

    def test_a_missing_or_unfinished_result_is_refused(self):
        partial = nine()
        del partial["b-20260937"]
        with self.assertRaises(SystemExit):
            g.summarize(partial)
        running = nine(**{"nc-20260935": {**result(True, True), "status": "running"}})
        with self.assertRaises(SystemExit):
            g.summarize(running)


class CodeState(unittest.TestCase):
    def repo(self, d):
        root = pathlib.Path(d)
        git = lambda *a: subprocess.run(["git", "-C", d, *a], check=True, capture_output=True)  # noqa: E731
        git("init", "-q")
        (root / ".gitignore").write_text("ign.py\n")
        (root / "a.py").write_text("a\n")
        (root / "b.py").write_text("b\n")
        git("add", ".gitignore", "a.py", "b.py")
        git("-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "-m", "x")
        return root

    def test_a_modified_or_untracked_listed_path_is_dirty_and_an_unlisted_one_is_not(self):
        with tempfile.TemporaryDirectory() as d:
            root = self.repo(d)
            clean = g.code_state(root, ["a.py"])
            self.assertEqual(clean["dirty"], [])
            self.assertEqual(clean["sha256"]["a.py"], hashlib.sha256(b"a\n").hexdigest())
            (root / "a.py").write_text("changed\n")
            (root / "b.py").write_text("changed\n")
            (root / "new.py").write_text("n\n")
            self.assertEqual(g.code_state(root, ["a.py", "new.py"])["dirty"], ["a.py", "new.py"])

    def test_an_untracked_file_inside_an_untracked_directory_is_named(self):
        # CODE lists a directory (stage2/frozen). For a directory pathspec, git's default untracked mode
        # names only the untracked subdirectory, `sub/`; the refusal fires either way, and the flag makes
        # it name the file. (For a file pathspec both modes name the file, so that case proves nothing.)
        with tempfile.TemporaryDirectory() as d:
            root = self.repo(d)
            (root / "sub").mkdir()
            (root / "sub/new.py").write_text("n\n")
            self.assertEqual(g.code_state(root, ["sub"])["dirty"], ["sub/new.py"])

    def test_an_ignored_uncommitted_file_is_dirty(self):
        with tempfile.TemporaryDirectory() as d:
            root = self.repo(d)
            (root / "ign.py").write_text("i\n")
            self.assertEqual(g.code_state(root, ["ign.py"])["dirty"], ["ign.py"])


MENU14 = g.ta.menu_from_manifest()
FINAL = ["d_adjacency", "d_red", "d_semicolon", "d_sessionid"]
COMMON = ["d_adjacency", "d_semicolon", "d_sessionid"]


class CheckpointFixture(unittest.TestCase):
    """A run directory, a scored file, a common menu and a step4.json that pass every check."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        d = pathlib.Path(self.tmp.name)
        self.run = d / "s2-nc-20260935"        # the directory an nc checkpoint at this seed lives in
        self.run.mkdir()
        (self.run / "best.pt").write_bytes(b"checkpoint bytes")
        (self.run / "log.jsonl").write_text(json.dumps({"event": "start", "menu": MENU14}) + "\n")
        self.scored = d / "scored.json"
        self.scored.write_text("{}")
        self.common = d / "common.json"
        others = [{"run_dir": f"/elsewhere/{i}", "final_menu": COMMON + ["closed_population"]} for i in range(8)]
        self.common_doc = {"common_menu": COMMON, "runs": others + [{"run_dir": str(self.run), "final_menu": FINAL}]}
        self.common.write_text(json.dumps(self.common_doc))
        self.step = dict(input={"sha256": hashlib.sha256(b"{}").hexdigest()}, run_dir=str(self.run), seed=20260935,
                         checkpoint_sha256=hashlib.sha256(b"checkpoint bytes").hexdigest(), menu=MENU14,
                         arm="qwen", recipe="s1-r1", measured={"learned": True}, final_menu=FINAL,
                         temperatures={h: {"T": 1.0} for h in MENU14},
                         thresholds={h: {"precision_t": 0.9, "recall90_t": 0.05} for h in MENU14})
        self.out = d / "gate"
        self.out.mkdir()
        self.result = self.out / "nc-20260935.json"
        for p in (mock.patch.object(g, "code_state", return_value={"head": "h" * 40, "dirty": [], "sha256": {}}),
                  # Backstop: a check this suite deletes must not reach the real model, only this loud refusal.
                  mock.patch.object(g.ta, "Arm", side_effect=AssertionError("the model loaded"))):
            p.start()
            self.addCleanup(p.stop)

    def args(self, name="nc-20260935", **step):
        p = pathlib.Path(self.tmp.name) / f"step4-{name}.json"
        p.write_text(json.dumps({**self.step, **step}))
        return argparse.Namespace(step4=p, scored=self.scored, common=self.common,
                                  audit=PHASE1B / "audit", out_dir=self.out, device="cuda:0")

    def refused(self, args, needle):
        with self.assertRaises(SystemExit) as cm:
            quiet(g.run_checkpoint, args)
        self.assertIn(needle, str(cm.exception))


class RefusalsBeforeTheGate(CheckpointFixture):
    """Each case is built so every earlier check admits it: the check it names is the one that refuses."""

    def test_a_second_invocation_is_refused(self):
        self.result.write_text("{}")
        self.refused(self.args(), "runs once")

    def test_a_second_invocation_into_another_directory_is_refused_by_the_marker(self):
        (self.run / g.RESERVED).write_text("{}")
        self.refused(self.args(), "has already run")

    def test_a_name_whose_arm_is_not_the_run_directorys_is_refused(self):
        self.refused(self.args(name="n-20260935"), "Step 4 read")

    def test_a_name_whose_seed_is_not_step4s_is_refused(self):
        # The run directory still matches the name, so only the seed half refuses.
        (self.run.parent / "s2-nc-20260937").mkdir()
        self.refused(self.args(name="nc-20260937", run_dir=str(self.run.parent / "s2-nc-20260937")), "Step 4 read")

    def test_an_unknown_arm_is_refused(self):
        self.refused(self.args(name="x-20260935"), "Step 4 read")

    def test_a_scored_file_step4_did_not_read_is_refused(self):
        self.refused(self.args(input={"sha256": "0" * 64}), "is not the scored file")

    def test_a_checkpoint_step4_did_not_score_is_refused(self):
        self.refused(self.args(checkpoint_sha256="0" * 64), "is not the checkpoint")

    def test_a_step4_head_order_that_differs_from_the_manifest_is_refused(self):
        # The run agrees with Step 4 and both differ from the manifest, so only the manifest half refuses.
        # A run left in manifest order would be refused by the other half too, and prove nothing here.
        reversed_menu = list(reversed(MENU14))
        (self.run / "log.jsonl").write_text(json.dumps({"event": "start", "menu": reversed_menu}) + "\n")
        self.refused(self.args(menu=reversed_menu), "head order")

    def test_a_run_head_order_that_differs_from_step4_is_refused(self):
        # Step 4's menu is the manifest's, so only the run's start event disagrees.
        (self.run / "log.jsonl").write_text(json.dumps({"event": "start", "menu": list(reversed(MENU14))}) + "\n")
        self.refused(self.args(), "head order")

    def test_a_common_menu_over_other_than_nine_checkpoints_is_refused(self):
        self.common.write_text(json.dumps({**self.common_doc, "runs": self.common_doc["runs"][1:]}))
        self.refused(self.args(), "covers 8 checkpoints")

    def test_a_common_menu_this_checkpoint_is_not_part_of_is_refused(self):
        runs = self.common_doc["runs"][:8] + [{"run_dir": str(self.run), "final_menu": COMMON}]
        self.common.write_text(json.dumps({**self.common_doc, "runs": runs}))
        self.refused(self.args(), "not among")

    def test_a_common_menu_that_is_not_its_runs_intersection_is_refused(self):
        self.common.write_text(json.dumps({**self.common_doc, "common_menu": COMMON[:2]}))
        self.refused(self.args(), "not the intersection")

    def test_clean_texts_that_differ_from_the_registration_are_refused(self):
        changed = [(c, t + " ") if c == "clean-12" else (c, t) for c, t in g.ct.NEW_CLEAN]
        with mock.patch.object(g.ct, "NEW_CLEAN", changed):
            self.refused(self.args(), "differs from the Step 2 table")

    def test_uncommitted_code_is_refused(self):
        with mock.patch.object(g, "code_state", return_value={"head": "h", "dirty": ["scripts/x.py"], "sha256": {}}):
            self.refused(self.args(), "commit these")

    def test_the_committed_paths_include_the_gate_inputs_inside_the_repo(self):
        seen = []
        def state(root, paths):
            seen.extend(paths)
            return {"head": "h", "dirty": ["stop here"], "sha256": {}}
        args = self.args()
        args.common = PHASE1B / "stage2/common.json"       # inside the repo, so it must be committed too
        with mock.patch.object(g, "code_state", state), mock.patch.object(g, "check_common"):
            self.refused(args, "commit these")
        self.assertIn("docs/evals/data/2026-09-24-rule-tell/phase1b/stage2/common.json", seen)
        self.assertIn("docs/evals/data/2026-09-24-rule-tell/phase1b/step5_gate.py", seen)

    def test_another_device_is_refused(self):
        with mock.patch("torch.cuda.get_device_name", return_value="AMD Radeon RX 7900 XTX"), \
                mock.patch("torch.version.hip", None):
            self.refused(self.args(), "under CUDA")

    def test_the_a5000_under_rocm_is_refused(self):
        with mock.patch("torch.cuda.get_device_name", return_value=g.DEVICE_NAME), \
                mock.patch("torch.version.hip", "6.2"):
            self.refused(self.args(), "under CUDA")


class FakeTensor:
    def __init__(self, rows):
        self.rows = rows

    def cpu(self):
        return self

    def tolist(self):
        return self.rows


class FakeArm:
    """train_arm.Arm's surface as run_checkpoint uses it. Every logit is -20, so no rule fires."""
    cues: list = []

    def __init__(self, arm, n, device, recipe):
        self.n = n

    def load_state_dict(self, state, strict=False):
        return mock.Mock(unexpected_keys=[])

    def state_dict(self):
        return {}

    def eval(self):
        pass

    def unit_logits(self, units):
        return FakeTensor(fake_logits(self.cues, heads=MENU14)(units))


class CueArm(FakeArm):
    """Fires d_semicolon and d_sessionid on phase 1's violating units only: every text passes."""
    cues = [(SEMI_UNIT, "d_semicolon", 5.0), (SESSION_UNIT, "d_sessionid", 5.0)]


OK = {"ok": True, "max_abs_dz": 0.0, "cells": 1, "finite": True}
BAD = {"ok": False, "max_abs_dz": 1.0, "cells": 1, "finite": True}


class AfterTheModelLoads(CheckpointFixture):
    def setUp(self):
        super().setUp()
        for p in (mock.patch.object(g.ta, "Arm", FakeArm), mock.patch("torch.load", return_value={}),
                  mock.patch("torch.cuda.get_device_name", return_value=g.DEVICE_NAME),
                  mock.patch("torch.version.hip", None)):
            p.start()
            self.addCleanup(p.stop)

    def checked(self, det, par, **kw):
        return mock.patch.object(g, "determinism", return_value=det), \
            mock.patch.object(g, "parity", return_value=par, **kw)

    def test_a_failed_instrument_check_runs_no_gate_reserves_nothing_and_is_recorded(self):
        for det, par in ((BAD, {**OK, "differing": 0}), (OK, {**BAD, "differing": 1})):
            a, b = self.checked(det, par)
            with a, b:
                self.assertEqual(quiet(g.run_checkpoint, self.args()), 2)
            self.assertFalse(self.result.exists())
            self.assertFalse((self.run / g.RESERVED).exists())
        refusals = [json.loads(line) for line in (self.out / "refusals.jsonl").read_text().splitlines()]
        self.assertEqual([(r["determinism"]["ok"], r["parity"]["ok"]) for r in refusals], [(False, True), (True, False)])

    def test_a_completed_run_records_both_menus_its_provenance_and_refuses_a_second(self):
        a, b = self.checked(OK, {**OK, "differing": 0})
        # learned=False here, so a header that stopped copying Step 4's value would read True.
        with a, b as par:
            self.assertEqual(quiet(g.run_checkpoint, self.args(measured={"learned": False})), 0)
        val = g.ta.load_rows("val")
        self.assertEqual([r["id"] for r in par.call_args.args[1]], [r["id"] for r in val])
        res = json.loads(self.result.read_text())
        self.assertEqual(res["status"], "completed")
        self.assertIs(res["learned"], False)
        self.assertEqual(res["code"]["head"], "h" * 40)
        self.assertEqual((res["determinism"], res["parity"]), (OK, {**OK, "differing": 0}))
        self.assertEqual(res["checkpoint_sha256"], self.step["checkpoint_sha256"])
        self.assertEqual(res["menus"]["own"]["menu"], FINAL)
        self.assertEqual(res["menus"]["common"]["menu"], COMMON)
        self.assertEqual(len(res["gate_texts"]), 22)
        self.assertFalse(res["menus"]["own"]["passed"])      # nothing fires, so the positives fail
        rows = [json.loads(line) for line in (self.out / "nc-20260935.log.jsonl").read_text().splitlines()]
        self.assertEqual({r["menu"] for r in rows}, {"own", "common"})
        self.assertTrue((self.run / g.RESERVED).exists())
        self.refused(self.args(), "runs once")

    def test_a_checkpoint_that_fires_on_its_positives_alone_passes_on_both_menus(self):
        a, b = self.checked(OK, {**OK, "differing": 0})
        with a, b, mock.patch.object(g.ta, "Arm", CueArm):
            self.assertEqual(quiet(g.run_checkpoint, self.args()), 0)
        res = json.loads(self.result.read_text())
        for m in ("own", "common"):
            self.assertTrue(res["menus"][m]["passed"], m)
            self.assertEqual(res["menus"][m]["gate"], {"passed": 19, "of": 19, "errored": 0})
            self.assertEqual(res["menus"][m]["span"], {"passed": 2, "of": 2, "errored": 0})

    def test_the_gate_reads_logits_scored_in_the_main_thread(self):
        import threading
        threads = set()
        class Recording(CueArm):
            def unit_logits(self, units):
                threads.add(threading.get_ident())
                return super().unit_logits(units)
        a, b = self.checked(OK, {**OK, "differing": 0})
        with a, b, mock.patch.object(g.ta, "Arm", Recording):
            quiet(g.run_checkpoint, self.args())
        self.assertEqual(threads, {threading.get_ident()})

    def test_a_crash_during_the_gate_keeps_what_ran_and_the_reservation(self):
        real = g.read_menu
        def second_crashes(judge, name, menu, texts):
            if name == "common":
                raise RuntimeError("boom in the common menu")
            return real(judge, name, menu, texts)
        a, b = self.checked(OK, {**OK, "differing": 0})
        with a, b, mock.patch.object(g, "read_menu", second_crashes), self.assertRaises(RuntimeError):
            quiet(g.run_checkpoint, self.args())
        res = json.loads(self.result.read_text())
        self.assertEqual(res["status"], "crashed")
        self.assertIn("boom in the common menu", res["traceback"])
        self.assertEqual(list(res["menus"]), ["own"])
        self.assertNotIn("rows", res["menus"]["own"])
        self.assertTrue(res["rows"] and all(r["menu"] == "own" for r in res["rows"]))
        self.assertTrue((self.run / g.RESERVED).exists())
        self.refused(self.args(), "runs once")

    def test_an_invocation_that_starts_during_the_checks_is_refused_at_the_reservation(self):
        def peer_reserves(*a):
            self.result.write_text("{}")      # a second invocation reserved while this one was checking
            return {**OK, "differing": 0}
        a, b = self.checked(OK, None, side_effect=peer_reserves)
        with a, b, self.assertRaises(FileExistsError):
            quiet(g.run_checkpoint, self.args())
        self.assertEqual(self.result.read_text(), "{}")

    def test_a_marker_that_appears_during_the_checks_is_refused_at_the_reservation(self):
        def peer_marks(*a):
            (self.run / g.RESERVED).write_text("{}")
            return {**OK, "differing": 0}
        a, b = self.checked(OK, None, side_effect=peer_marks)
        with a, b, self.assertRaises(FileExistsError):
            quiet(g.run_checkpoint, self.args())

    def test_a_checkpoint_with_keys_the_model_does_not_have_is_refused(self):
        class Unexpected(FakeArm):
            def load_state_dict(self, state, strict=False):
                return mock.Mock(unexpected_keys=["head.extra"])
        with mock.patch.object(g.ta, "Arm", Unexpected):
            self.refused(self.args(), "does not fit")

    def test_a_checkpoint_holding_a_key_outside_the_model_is_refused(self):
        # load_state_dict reports nothing unexpected; only the state-dict comparison sees the key.
        with mock.patch("torch.load", return_value={"orphan.weight": 0}):
            self.refused(self.args(), "does not fit")


if __name__ == "__main__":
    unittest.main()
