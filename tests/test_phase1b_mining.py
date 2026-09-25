"""Phase-1b Stage 2's counterexample miner: which cues it mines, and the clean texts it filters on.

Run: python3 -B tests/test_phase1b_mining.py (no torch needed; mine_counterexamples imports
train_arm inside main()).
"""
import importlib.util
import pathlib
import sys
import unittest

PHASE1B = pathlib.Path(__file__).resolve().parents[1] / "docs/evals/data/2026-09-24-rule-tell/phase1b"
sys.path.insert(0, str(PHASE1B))
sys.path.insert(0, str(PHASE1B.parent / "stage2"))


def _load(name):
    spec = importlib.util.spec_from_file_location(name, PHASE1B / f"{name}.py")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


mc = _load("mine_counterexamples")
ct = _load("clean_texts")


def cue(feature, pos, neg, pool):
    # Only the three counts mined_cues reads; the denominators are inert here.
    return {"feature": feature, "train_pos": [pos, 60], "train_neg": [neg, 60],
            "cross_pool_train_units": [pool, 8000]}


class MinedCues(unittest.TestCase):
    def test_a_cue_the_pool_lacks_is_mined(self):
        got = mc.mined_cues({"heads": {"d_semicolon": [cue("&&", 56, 0, 0)]}})
        self.assertEqual(got, {"d_semicolon": ["&&"]})

    def test_one_more_positive_than_negatives_plus_pool_is_mined(self):
        # The boundary: 25 > 2 + 22. `>=` and `>` agree here; the next case separates them.
        self.assertEqual(mc.mined_cues({"heads": {"run_tool": [cue("exits", 25, 2, 22)]}}),
                         {"run_tool": ["exits"]})

    def test_a_tie_is_not_mined(self):
        # 24 == 2 + 22: the cue no longer predicts a violation among cue-bearing cells.
        self.assertEqual(mc.mined_cues({"heads": {"run_tool": [cue("exits", 24, 2, 22)]}}), {})

    def test_own_negatives_count_against_the_cue(self):
        # 20 > 0 + 19, but 20 is not > 2 + 19: dropping train_neg from the sum mines this.
        self.assertEqual(mc.mined_cues({"heads": {"h": [cue("x", 20, 2, 19)]}}), {})

    def test_the_pool_counts_against_the_cue(self):
        # 20 > 2 + 0, but not > 2 + 18: dropping the pool from the sum mines this.
        self.assertEqual(mc.mined_cues({"heads": {"h": [cue("x", 20, 2, 18)]}}), {})

    def test_a_head_with_no_mined_cue_is_absent_and_order_is_kept(self):
        got = mc.mined_cues({"heads": {
            "b": [cue("is", 19, 1, 2323)],
            "a": [cue("y", 9, 0, 1), cue("z", 1, 0, 0), cue("w", 1, 1, 0)]}})
        self.assertEqual(got, {"a": ["y", "z"]})


class CleanTexts(unittest.TestCase):
    def test_new_clean_matches_the_registered_table(self):
        ct.check_against_doc()

    def test_a_changed_text_is_refused(self):
        saved = list(ct.NEW_CLEAN)
        try:
            ct.NEW_CLEAN[6] = (ct.NEW_CLEAN[6][0], ct.NEW_CLEAN[6][1].replace("&&", "&"))
            with self.assertRaises(SystemExit):
                ct.check_against_doc()
        finally:
            ct.NEW_CLEAN[:] = saved


if __name__ == "__main__":
    unittest.main()
