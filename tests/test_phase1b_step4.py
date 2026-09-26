"""Phase-1b Stage 2's Step 4: score_run.py's parity check and step4.py's cell sets, calibration and
thresholds input, pre-gate check, gate-ability, measurements and common menu. Needs torch and
sklearn: run with the training venv,
    ~/work/claude/jevk5/.venv/bin/python tests/test_phase1b_step4.py
"""
import importlib.util
import pathlib
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[1]
PHASE1B = ROOT / "docs/evals/data/2026-09-24-rule-tell/phase1b"


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


s4 = load("step4", PHASE1B / "step4.py")
sr = load("score_run", PHASE1B / "score_run.py")
ta = s4.ta


def row(i, rule, label, z, source="synthetic"):
    return dict(id=f"r{i}", rule=rule, label=label, source=source, z=z)


def xcell(i, head, z):
    return dict(id=f"x{i}", unit=0, head=head, z=z)


def scored(menu, admitted, val, val_cross, cal, cal_cross):
    return dict(run_dir="fixture", arm="qwen", recipe="s1-r1", seed=0, checkpoint_sha256="0" * 64,
                menu=list(menu), cross=dict(admitted=list(admitted), sha256="0" * 64, masked=0),
                extra_rows=None, val=val, val_cross=val_cross, cal=cal, cal_cross=cal_cross)


class HeadCells(unittest.TestCase):
    def test_kinds(self):
        s = {"val": [row(0, "h", 1, 2.0, source="synthetic"),
                     # LOAD-BEARING: frozen rows carry "synthetic" or "mined", never "frozen". A
                     # classifier keyed on "frozen" files these as counterexamples, which the smoke
                     # run on s1-r1-20260935 caught (every head "no cal positive").
                     row(1, "h", 0, -1.0, source="mined"),
                     row(2, "h", 0, 3.0, source="counterexample"),
                     row(3, "g", 1, 1.0)],                       # another rule's row: not h's cell
             "val_cross": [xcell(0, "h", 0.5), xcell(1, "g", 9.0)]}
        self.assertEqual(sorted(s4.head_cells(s, "val", "h")),
                         sorted([(2.0, 1, "own"), (-1.0, 0, "own"), (3.0, 0, "counterexample"),
                                 (0.5, 0, "cross")]))


class Pregate(unittest.TestCase):
    """T = 1 and t = 0.5, so a cell fires exactly when z >= 0. Each case sets the bound it does not
    test to pass, so the case exercises only the bound it names."""

    def cells(self, cross_fire, cross_quiet, pos_hit, pos_miss, extra=()):
        return ([(1.0, 0, "cross")] * cross_fire + [(-1.0, 0, "cross")] * cross_quiet
                + [(1.0, 1, "own")] * pos_hit + [(-1.0, 1, "own")] * pos_miss + list(extra))

    def test_cross_firing_at_exactly_5_percent_passes(self):
        r = s4.pregate(self.cells(2, 38, 2, 0), 1.0, 0.5)
        self.assertEqual((r["cross_fired"], r["cross_n"]), (2, 40))
        self.assertTrue(r["passed"], r["reasons"])

    def test_cross_firing_above_5_percent_fails(self):
        r = s4.pregate(self.cells(3, 37, 2, 0), 1.0, 0.5)
        self.assertFalse(r["passed"])
        self.assertEqual(r["reasons"], ["cross firing 3/40 > 5%"])

    def test_recall_at_exactly_half_passes(self):
        r = s4.pregate(self.cells(0, 10, 1, 1), 1.0, 0.5)
        self.assertEqual((r["pos_hit"], r["pos_n"]), (1, 2))
        self.assertTrue(r["passed"], r["reasons"])

    def test_recall_below_half_fails(self):
        r = s4.pregate(self.cells(0, 10, 1, 2), 1.0, 0.5)
        self.assertEqual(r["reasons"], ["own-positive recall 1/3 < 0.5"])

    def test_counterexamples_and_own_negatives_count_toward_neither_bound(self):
        # Firing counterexamples and own negatives: counted as cross cells they would fail the cross
        # bound (2/12), and counted as positives they would change the recall denominator.
        r = s4.pregate(self.cells(0, 10, 1, 0, extra=[(5.0, 0, "counterexample"), (5.0, 0, "own")]), 1.0, 0.5)
        self.assertEqual((r["cross_fired"], r["cross_n"], r["pos_hit"], r["pos_n"]), (0, 10, 1, 1))
        self.assertTrue(r["passed"], r["reasons"])

    def test_a_cell_exactly_at_the_threshold_fires(self):
        # sigmoid(0) = 0.5 = t: thresholds() fires at p >= t, and the check must agree.
        r = s4.pregate([(0.0, 0, "cross")] + [(-1.0, 0, "cross")] * 19 + [(1.0, 1, "own")], 1.0, 0.5)
        self.assertEqual(r["cross_fired"], 1)

    def test_a_head_that_cannot_be_checked_leaves(self):
        self.assertEqual(s4.pregate([(1.0, 1, "own")], 1.0, 0.5)["reasons"], ["no cal cross cell"])
        self.assertEqual(s4.pregate([(-1.0, 0, "cross")], 1.0, 0.5)["reasons"], ["no cal positive"])


class GateAbility(unittest.TestCase):
    # SPAN_MIN is implied by GATE_MIN under today's constants: any 2 of the 3 gate positives include
    # d_semicolon or d_sessionid, each a span text's rule. So no menu separates the span condition,
    # and a mutation deleting it survives. That is inertness, not a missing case.
    def test_two_gate_positives_are_enough(self):
        for menu in (["d_semicolon", "d_sessionid"], ["member_vs_population", "d_semicolon"],
                     ["member_vs_population", "d_sessionid", "run_tool"]):
            self.assertTrue(s4.gate_ability(menu)["gate_able"], menu)

    def test_one_gate_positive_is_not(self):
        for menu in (["d_semicolon"], ["member_vs_population", "run_tool"], []):
            self.assertFalse(s4.gate_ability(menu)["gate_able"], menu)


class GateConstantsMatchTheGateCode(unittest.TestCase):
    def test_positives_are_the_gate_texts_whose_rule_is_on_the_local_menu(self):
        gate = load("span_selector", ROOT / "scripts/phase1-span-selector.py")
        menu = set(ta.menu_from_manifest())
        self.assertEqual({cid: want for cid, _, want in gate.GATE if want in menu}, s4.GATE_POSITIVES)
        self.assertEqual({cid: rule for cid, rule, *_ in gate.SPAN_GATE if rule in menu}, s4.SPAN_POSITIVES)


class Step4CellSets(unittest.TestCase):
    """Head h is admitted; g is admitted and fails the pre-gate check; k is not admitted."""

    def fixture(self):
        val = [row(0, "h", 1, 3.0), row(1, "h", 1, 4.0), row(2, "h", 0, -3.0), row(3, "h", 0, -4.0),
               row(4, "g", 1, 2.0), row(5, "g", 0, -2.0), row(6, "k", 1, 0.0)]
        # LOAD-BEARING: 3.5 sits between h's two val positives, so the precision threshold is
        # sigmoid(4/T) with the cross cell and sigmoid(3/T) without it.
        val_cross = [xcell(0, "h", 3.5), xcell(1, "g", -5.0)]
        cal = [row(10, "h", 1, 3.0), row(11, "h", 1, 4.0), row(12, "h", 0, -3.0), row(13, "h", 0, -4.0),
               row(14, "g", 1, 2.0), row(15, "g", 0, -2.0), row(16, "k", 1, 0.0)]
        # LOAD-BEARING: 3.2 is a negative among h's cal positives, so the temperature fit differs with
        # and without the cross cells (without them h's cal cells separate, and T runs to 0.25).
        # g's 10.0 fires above any threshold g can have: 1 of 1 cross cells, so g leaves at Step 4.
        cal_cross = [xcell(10, "h", 3.2), xcell(11, "h", -5.0), xcell(12, "g", 10.0)]
        return scored(["h", "g", "k"], ["h", "g"], val, val_cross, cal, cal_cross)

    def test_calibration_uses_own_plus_cross_cells_on_cal(self):
        res = s4.step4(self.fixture())
        with_cross = ta.fit_temperature([3.0, 4.0, -3.0, -4.0, 3.2, -5.0], [1, 1, 0, 0, 0, 0])[0]
        own_only = ta.fit_temperature([3.0, 4.0, -3.0, -4.0], [1, 1, 0, 0])[0]
        self.assertNotEqual(with_cross, own_only)              # the fixture discriminates
        self.assertEqual(res["temperatures"]["h"]["T"], with_cross)
        self.assertEqual((res["temperatures"]["h"]["cal_n"], res["temperatures"]["h"]["cal_cross"]), (6, 2))

    def test_thresholds_use_own_plus_cross_cells_on_val(self):
        res = s4.step4(self.fixture())
        T = res["temperatures"]["h"]["T"]
        self.assertEqual(res["thresholds"]["h"]["precision_t"], s4.sig(4.0 / T))
        self.assertEqual(res["thresholds"]["h"]["val_cross"], 1)

    def test_menus(self):
        res = s4.step4(self.fixture())
        self.assertEqual(res["removed"]["step1"], ["k"])
        self.assertEqual([r["head"] for r in res["removed"]["step4"]], ["g"])
        self.assertEqual(res["removed"]["step4"][0]["reasons"], ["cross firing 1/1 > 5%"])
        self.assertEqual(res["final_menu"], ["h"])
        self.assertNotIn("k", res["temperatures"])             # a head Step 1 removed is not calibrated


class Measurements(unittest.TestCase):
    def fixture(self):
        # LOAD-BEARING: the counterexample's z = 5 is above h's only positive, so counted as an own
        # cell it would drop own-cell AUC to 0.5; the cross cell's -1 is below it.
        val = [row(0, "h", 1, 1.0), row(1, "h", 0, 0.0), row(2, "h", 0, 5.0, source="counterexample")]
        cal = [row(10, "h", 1, 1.0), row(11, "h", 0, 0.0), row(12, "h", 0, 5.0, source="counterexample")]
        return scored(["h"], ["h"], val, [xcell(0, "h", -1.0)], cal, [xcell(10, "h", -1.0)])

    def test_own_cell_auc_is_over_frozen_rows_only(self):
        m = s4.step4(self.fixture())["measured"]
        self.assertEqual(m["own_cell_val_auc"]["pooled"], 1.0)
        self.assertTrue(m["learned"])

    def test_all_cells_auc_counts_counterexamples_and_cross_cells(self):
        # positive 1.0 against negatives {0.0, 5.0, -1.0}: 2 of 3 pairs ordered correctly.
        m = s4.step4(self.fixture())["measured"]
        self.assertAlmostEqual(m["all_cells_val_auc"]["per_head"]["h"], 2 / 3)

    def test_cal_firing_is_split_by_kind(self):
        m = s4.step4(self.fixture())["measured"]
        self.assertEqual(m["cal_counterexamples_fired"]["h"], [1, 1])
        self.assertEqual(m["cal_own_negatives_fired"]["h"][1], 1)


class CommonMenu(unittest.TestCase):
    def test_intersection_in_head_order(self):
        runs = [dict(run_dir="a", menu=["x", "y", "z"], final_menu=["z", "x"]),
                dict(run_dir="b", menu=["x", "y", "z"], final_menu=["x", "y", "z"])]
        self.assertEqual(s4.common_menu(runs)["common_menu"], ["x", "z"])

    def test_refuses_runs_with_different_head_orders(self):
        runs = [dict(run_dir="a", menu=["x", "y"], final_menu=["x"]),
                dict(run_dir="b", menu=["y", "x"], final_menu=["x"])]
        with self.assertRaises(SystemExit):
            s4.common_menu(runs)


class Parity(unittest.TestCase):
    frozen = {"val": {"r0", "r1"}, "cal": {"r2"}}

    def got(self, dz=0.0):
        return {"val": [dict(id="r0", z=1.0 + dz), dict(id="r1", z=-2.0), dict(id="cx-1", z=0.3)],
                "cal": [dict(id="r2", z=0.5)],
                "val_cross": [dict(id="r0", unit=0, head="g", z=-1.0)],
                "cal_cross": [dict(id="r2", unit=1, head="g", z=-0.5)]}

    def ref(self):
        # A run trained without counterexamples: its fold-logits.json has no "cx-1".
        return {"val": [dict(id="r0", z=1.0), dict(id="r1", z=-2.0)], "cal": [dict(id="r2", z=0.5)],
                "val_cross": [dict(id="r0", unit=0, head="g", z=-1.0)],
                "cal_cross": [dict(id="r2", unit=1, head="g", z=-0.5)]}

    def test_identical_logits_pass(self):
        r = sr.parity(self.ref(), self.got(), self.frozen, compare_cross=True)
        self.assertTrue(r["ok"], r)
        self.assertEqual(r["val"]["compared"], 2)

    def test_any_difference_fails(self):
        self.assertFalse(sr.parity(self.ref(), self.got(dz=1e-12), self.frozen, compare_cross=False)["ok"])

    def test_a_frozen_row_missing_from_the_run_fails(self):
        ref = self.ref()
        ref["val"] = ref["val"][:1]
        self.assertFalse(sr.parity(ref, self.got(), self.frozen, compare_cross=False)["ok"])

    def test_cross_cells_compared_only_when_asked(self):
        ref = self.ref()
        ref["cal_cross"][0]["z"] = 9.0
        self.assertTrue(sr.parity(ref, self.got(), self.frozen, compare_cross=False)["ok"])
        self.assertFalse(sr.parity(ref, self.got(), self.frozen, compare_cross=True)["ok"])

    def test_different_cross_cells_fail(self):
        ref = self.ref()
        ref["val_cross"].append(dict(id="r1", unit=0, head="g", z=-3.0))
        r = sr.parity(ref, self.got(), self.frozen, compare_cross=True)
        self.assertFalse(r["ok"])
        self.assertFalse(r["val_cross"]["same_cells"])


class TrainingMismatch(unittest.TestCase):
    A, B = "a" * 64, "b" * 64

    def test_arm_b_takes_any_admission_and_counterexample_file(self):
        self.assertEqual(sr.training_mismatch({}, self.A, self.B), [])

    def test_a_run_trained_with_cross_must_be_scored_with_that_file(self):
        start = {"cross": {"sha256": self.A}}
        self.assertEqual(sr.training_mismatch(start, self.A, None), [])
        self.assertEqual(len(sr.training_mismatch(start, self.B, None)), 1)

    def test_a_run_trained_with_counterexamples_must_be_scored_with_that_file(self):
        # The admission file matches, so only the counterexample refusal can fire.
        start = {"cross": {"sha256": self.A}, "extra_rows": {"sha256": self.B}}
        self.assertEqual(sr.training_mismatch(start, self.A, self.B), [])
        self.assertEqual(len(sr.training_mismatch(start, self.A, None)), 1)
        self.assertEqual(len(sr.training_mismatch(start, self.A, self.A)), 1)


if __name__ == "__main__":
    unittest.main()
