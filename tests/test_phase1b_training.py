"""Phase-1b Stage 2's training changes in train_arm.py: the cross-rule cells, the counterexample
rows, and the epoch order. Needs torch: run with the training venv,
    ~/work/claude/jevk5/.venv/bin/python tests/test_phase1b_training.py
"""
import importlib.util
import json
import pathlib
import random
import tempfile
import unittest

STAGE3 = pathlib.Path(__file__).resolve().parents[1] / "docs/evals/data/2026-09-24-rule-tell/stage3"
spec = importlib.util.spec_from_file_location("train_arm", STAGE3 / "train_arm.py")
ta = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ta)


def stage1_order(train, seed, ep, pair_windows):
    """Stage 1's epoch_order, copied verbatim from the closure in train_arm.main at 24426921, the
    commit Stage 1 ran at. The parity test holds the new function to it."""
    if not pair_windows:
        order = list(range(len(train)))
        random.Random(seed + ep).shuffle(order)
        return order
    groups = {}
    for i, r in enumerate(train):
        groups.setdefault(r["id"].rpartition(":")[0], []).append(i)
    pairs = [g for g in groups.values() if len(g) == 2]
    rest = [i for g in groups.values() if len(g) != 2 for i in g]
    random.Random(seed + ep).shuffle(pairs)
    return [i for g in pairs for i in g] + rest


def cx_row(k, rule="d_semicolon", fold="train"):
    return {"id": f"cx-test-{k}", "set": fold, "rule": rule, "source": "counterexample",
            "text": "One sentence here. Another sentence there.", "target": 0, "label": 0}


class EpochOrder(unittest.TestCase):
    train = ta.load_rows("train")

    def test_without_counterexamples_the_order_is_stage_1s(self):
        for seed in (20260935, 20260937, 20260940):
            for ep in range(5):
                for pw in (True, False):
                    self.assertEqual(ta.epoch_order(self.train, seed, ep, pw),
                                     stage1_order(self.train, seed, ep, pw), (seed, ep, pw))

    def test_counterexamples_keep_every_slot_two_rows_and_window_aligned(self):
        # 7 is odd on purpose: one counterexample has no partner and must go last.
        train = self.train + [cx_row(k) for k in range(7)]
        order = ta.epoch_order(train, 20260935, 0, True)
        self.assertEqual(sorted(order), list(range(len(train))))
        key = lambda i: "cx" if train[i].get("source") == "counterexample" else train[i]["id"].rpartition(":")[0]
        n_pairs = sum(1 for g in {key(i) for i in range(len(self.train))}
                      if sum(key(i) == g for i in range(len(self.train))) == 2)
        block = 2 * (n_pairs + 3)                       # frozen pairs plus 3 counterexample slots
        for k in range(0, block, 2):
            self.assertEqual(key(order[k]), key(order[k + 1]), k)
        cx_positions = [p for p, i in enumerate(order) if key(i) == "cx"]
        self.assertEqual(len([p for p in cx_positions if p < block]), 6)
        self.assertEqual(len([p for p in cx_positions if p >= block]), 1)

    def test_counterexamples_are_mixed_among_pairs_not_appended(self):
        train = self.train + [cx_row(k) for k in range(40)]
        order = ta.epoch_order(train, 20260935, 0, True)
        first_cx = min(p for p, i in enumerate(order) if train[i].get("source") == "counterexample")
        self.assertLess(first_cx, len(self.train) // 2)


class CrossCells(unittest.TestCase):
    units = ["alpha unit.", "beta unit.", "gamma unit."]
    row = {"id": "r:pos", "rule": "d_red", "text": "", "target": 1, "label": 1}

    def cross(self, admitted, masked=()):
        return ta.Cross(frozenset(admitted), frozenset(masked), "x")

    def test_every_unit_times_every_admitted_other_head(self):
        got = ta.cross_cells(self.row, self.units, self.cross({"d_semicolon", "run_tool"}))
        self.assertEqual(sorted(got), [(i, h) for i in range(3) for h in ("d_semicolon", "run_tool")])

    def test_the_rows_own_rule_is_never_a_cross_cell(self):
        # d_red admitted, but it is the row's rule: its non-target units stay masked.
        got = ta.cross_cells(self.row, self.units, self.cross({"d_red", "run_tool"}))
        self.assertEqual(sorted(got), [(0, "run_tool"), (1, "run_tool"), (2, "run_tool")])

    def test_a_head_not_admitted_contributes_nothing(self):
        self.assertEqual(ta.cross_cells(self.row, self.units, self.cross(set())), [])

    def test_a_masked_unit_text_is_masked_for_that_head_only(self):
        got = ta.cross_cells(self.row, self.units,
                             self.cross({"d_semicolon", "run_tool"}, {("beta unit.", "run_tool")}))
        self.assertNotIn((1, "run_tool"), got)
        self.assertIn((1, "d_semicolon"), got)
        self.assertEqual(len(got), 5)

    def test_a_counterexample_row_has_no_cross_cells(self):
        got = ta.cross_cells({**self.row, "source": "counterexample"}, self.units,
                             self.cross({"d_semicolon", "run_tool"}))
        self.assertEqual(got, [])


class FoldLossCross(unittest.TestCase):
    def test_rows_without_cross_cells_give_stage_1s_val_loss(self):
        rows = [{"rule": "d_red", "label": 1}, {"rule": "d_red", "label": 0}]
        pw = {"d_red": 1.3}
        own = [2.0, -1.5]
        self.assertEqual(ta.fold_loss_cross(own, [[], []], rows, pw), ta.fold_loss(own, rows, pw))

    def test_cross_cells_add_their_mean_bce_toward_zero(self):
        rows = [{"rule": "d_red", "label": 0}]
        pw = {"d_red": 1.0}
        base = ta.fold_loss([0.0], rows, pw)
        got = ta.fold_loss_cross([0.0], [[(0, "x", 0.0), (1, "x", 0.0)]], rows, pw)
        self.assertAlmostEqual(got - base, ta.CROSS_LAMBDA * 0.6931, places=4)


class LoadExtraRows(unittest.TestCase):
    menu = ["d_red", "d_semicolon"]

    def load(self, rows, taken=()):
        with tempfile.NamedTemporaryFile("w", suffix=".jsonl", delete=False) as f:
            f.write("".join(json.dumps(r) + "\n" for r in rows))
        return ta.load_extra_rows(pathlib.Path(f.name), self.menu, set(taken))

    def test_rows_land_in_their_fold(self):
        got = self.load([cx_row(0), cx_row(1, fold="val"), cx_row(2, fold="cal")])
        self.assertEqual({f: len(v) for f, v in got.items()}, {"train": 1, "val": 1, "cal": 1})

    def test_refuses_a_positive_label(self):
        with self.assertRaises(SystemExit):
            self.load([{**cx_row(0), "label": 1}])

    def test_refuses_a_row_that_is_not_a_counterexample(self):
        with self.assertRaises(SystemExit):
            self.load([{**cx_row(0), "source": "synthetic"}])

    def test_refuses_a_rule_off_the_menu(self):
        with self.assertRaises(SystemExit):
            self.load([cx_row(0, rule="question_asked")])

    def test_refuses_an_id_already_used(self):
        with self.assertRaises(SystemExit):
            self.load([cx_row(0)], taken={"cx-test-0"})
        with self.assertRaises(SystemExit):
            self.load([cx_row(0), cx_row(0)])

    def test_refuses_a_target_outside_the_text(self):
        with self.assertRaises(SystemExit):
            self.load([{**cx_row(0), "target": 5}])

    def test_refuses_an_unknown_fold(self):
        with self.assertRaises(SystemExit):
            self.load([cx_row(0, fold="T")])


if __name__ == "__main__":
    unittest.main()
