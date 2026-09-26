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


# Fixture texts. Each is 10 tokens, so it has three 8-token shingles. OVERLAP_A repeats A's first
# eight tokens and so shares exactly one shingle with A; B and C share nothing with anything.
# Load-bearing: shorten A below 8 tokens and it has no shingles, so every collision case below
# would pass without any filter firing.
A = "alpha beta gamma delta epsilon zeta eta theta iota kappa"
OVERLAP_A = "alpha beta gamma delta epsilon zeta eta theta lambda mu"
B = "one two three four five six seven eight nine ten"
C = "red orange yellow green blue indigo violet black white grey"


def cand(cid, fold, text, head="d_semicolon"):
    return {"cid": cid, "head": head, "fold": fold, "text": text,
            "text_sha1": mc.hashlib.sha1(text.encode()).hexdigest(), "manifest_id": 0, "unit_index": 0}


class FilterAgainstExisting(unittest.TestCase):
    # Each case gives exactly one guard a reason to refuse; the other two receive empty sets.
    def run_filter(self, cands, held=(), by_fold=None, by_rule=None):
        kept, _ = mc.filter_against_existing(cands, set(held), by_fold or {}, by_rule or {})
        return sorted(c["cid"] for c in kept)

    def test_held_out_text_is_dropped(self):
        got = self.run_filter([cand("a", "train", A), cand("b", "train", B)], held=mc.shingles(A))
        self.assertEqual(got, ["b"])

    def test_a_frozen_row_in_another_fold_drops_the_candidate(self):
        got = self.run_filter([cand("a", "train", A), cand("b", "train", B)], by_fold={"val": mc.shingles(A)})
        self.assertEqual(got, ["b"])

    def test_a_frozen_row_in_the_same_fold_does_not(self):
        got = self.run_filter([cand("a", "val", A)], by_fold={"val": mc.shingles(A)})
        self.assertEqual(got, ["a"])

    def test_a_frozen_row_of_the_heads_own_rule_drops_it_in_any_fold(self):
        cands = [cand("a", "train", A, head="d_red"), cand("b", "train", A, head="run_tool")]
        got = self.run_filter(cands, by_rule={"d_red": mc.shingles(A)})
        self.assertEqual(got, ["b"])


class ResolveNewVsNew(unittest.TestCase):
    def kept(self, cands):
        k, _ = mc.resolve_new_vs_new(cands)
        return sorted(c["cid"] for c in k)

    def test_train_colliding_with_val_is_dropped(self):
        self.assertEqual(self.kept([cand("t", "train", A), cand("v", "val", OVERLAP_A)]), ["v"])

    def test_cal_colliding_with_val_is_dropped(self):
        self.assertEqual(self.kept([cand("c", "cal", A), cand("v", "val", OVERLAP_A)]), ["v"])

    def test_train_colliding_with_cal_is_dropped(self):
        self.assertEqual(self.kept([cand("t", "train", A), cand("c", "cal", OVERLAP_A)]), ["c"])

    def test_collisions_within_one_fold_are_kept(self):
        self.assertEqual(self.kept([cand("t1", "train", A), cand("t2", "train", OVERLAP_A)]), ["t1", "t2"])

    def test_a_non_colliding_candidate_is_kept_in_every_fold(self):
        cands = [cand("t", "train", B), cand("v", "val", A), cand("c", "cal", C)]
        self.assertEqual(self.kept(cands), ["c", "t", "v"])

    def test_only_kept_candidates_block_later_folds(self):
        # cal "c" collides with val and is dropped; train "t" collides only with the dropped "c".
        cands = [cand("v", "val", A), cand("c", "cal", OVERLAP_A + " " + B), cand("t", "train", B)]
        self.assertEqual(self.kept(cands), ["t", "v"])


class CheckNoCrossFoldOverlap(unittest.TestCase):
    def test_drawn_candidates_sharing_text_across_folds_are_refused(self):
        with self.assertRaises(SystemExit):
            mc.check_no_cross_fold_overlap([cand("t", "train", A), cand("v", "val", OVERLAP_A)], {})

    def test_a_drawn_candidate_sharing_text_with_a_frozen_row_in_another_fold_is_refused(self):
        with self.assertRaises(SystemExit):
            mc.check_no_cross_fold_overlap([cand("t", "train", A)], {"cal": mc.shingles(A)})

    def test_overlap_within_a_fold_passes(self):
        mc.check_no_cross_fold_overlap([cand("t1", "train", A), cand("t2", "train", OVERLAP_A)],
                                       {"train": mc.shingles(A)})


class CrossFoldOverlap(unittest.TestCase):
    def test_counts_shared_shingles_and_the_texts_holding_them(self):
        cands = [cand("t", "train", A), cand("v", "val", OVERLAP_A), cand("t2", "train", B)]
        self.assertEqual(mc.cross_fold_overlap(cands), (1, 2))



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
