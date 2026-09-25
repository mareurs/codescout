"""The synthetic pairs' construction checks and the Stage 3 segmenter.

Every case mutates ONE property of BASE, a pair that passes every check, so the check the
case names is the only one that can refuse it (CLAUDE.md: a case only exercises the guard it
names if every other guard admits its input).
"""
import importlib.util
import pathlib
import unittest

STAGE2 = pathlib.Path(__file__).resolve().parents[1] / "docs/evals/data/2026-09-24-rule-tell/stage2"


def _load(name):
    spec = importlib.util.spec_from_file_location(name, STAGE2 / f"{name}.py")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


gen = _load("generate_synthetic")
seg = _load("segment")

VIOL = "The retry loop cannot leak handles on any platform."
FIXED = "The retry loop released every handle in the four Linux runs we traced."
# 60-200 words, no banned word, VIOL once, each sentence one segmenter unit. Load-bearing:
# the sentences before and after VIOL start with a capital, so SENT_SPLIT isolates it.
PARA = ("We traced the handle counts across the nightly suite after the scheduler change. "
        "Each run opened roughly two hundred sockets and closed them in the teardown phase. "
        f"{VIOL} "
        "The tracing script sampled the descriptor table every second and wrote it to a local file. "
        "Counts returned to the idle baseline within three seconds of the last test finishing. "
        "Windows and macOS runners were not part of this trace, and their counts stay unmeasured here. "
        "Next week the same script will run on the release branch before tagging.")
SEED = "Unrelated seed paragraph about documentation builds and link checking in the handbook."
BASE = {"seed_id": 7, "rule": "cannot_happen", "paragraph": PARA, "violating_sentence": VIOL,
        "fixed_sentence": FIXED, "why": "claims impossibility beyond what was traced"}


def check(**over):
    return gen.check_pair({**BASE, **over}, "cannot_happen", over.pop("_seed", SEED), set())


class ConstructionChecks(unittest.TestCase):
    def test_base_pair_passes_every_check(self):
        self.assertEqual(gen.check_pair(BASE, "cannot_happen", SEED, set()), [])

    def test_missing_field(self):
        pair = {k: v for k, v in BASE.items() if k != "why"}
        self.assertEqual(gen.check_pair(pair, "cannot_happen", SEED, set()), ["missing or mistyped field"])

    def test_rule_mismatch(self):
        self.assertEqual(check(rule="count_unit"), ["rule mismatch"])

    def test_violating_sentence_absent(self):
        self.assertIn("violating sentence not exactly once in paragraph",
                      check(violating_sentence="The retry loop never leaks handles anywhere at all."))

    def test_violating_sentence_twice(self):
        self.assertEqual(check(paragraph=PARA + " " + VIOL),
                         ["violating sentence not exactly once in paragraph"])

    def test_fixed_equals_violating(self):
        self.assertIn("fixed sentence equals violating sentence", check(fixed_sentence="  " + VIOL))

    def test_paragraph_too_short(self):
        short = f"We traced handle counts. {VIOL} Counts returned to baseline."
        self.assertEqual(check(paragraph=short), ["paragraph length outside 60-200 words"])

    def test_banned_word_in_fix(self):
        self.assertEqual(check(fixed_sentence="The retry loop released every handle, which was the wrong claim before."),
                         ["banned word"])

    def test_violating_sentence_not_one_unit(self):
        # Two sentences given as one "violating sentence": present once, but two units.
        two = "Each run opened roughly two hundred sockets and closed them in the teardown phase. " + VIOL
        self.assertEqual(check(violating_sentence=two), ["violating sentence is not one segmenter unit"])

    def test_fixed_sentence_not_one_unit(self):
        self.assertEqual(check(fixed_sentence="The loop released every handle. We traced four Linux runs."),
                         ["fixed sentence is not one segmenter unit"])

    def test_shingle_with_seed(self):
        self.assertEqual(check(_seed="Each run opened roughly two hundred sockets and closed them in the teardown phase."),
                         ["shares an 8-token shingle with its seed"])

    def test_shingle_with_held_out(self):
        held = gen.mp.shingles("Counts returned to the idle baseline within three seconds of the last test")
        self.assertEqual(gen.check_pair(BASE, "cannot_happen", SEED, held),
                         ["shares an 8-token shingle with a held-out text or T row"])


class Segmenter(unittest.TestCase):
    def test_prose_splits_into_sentences(self):
        self.assertEqual(seg.segment("One thing. Two things here."), ["One thing.", "Two things here."])

    def test_fence_is_one_unit(self):
        units = seg.segment("Before it.\n\n```\na. B. C.\n```\n\nAfter it.")
        self.assertEqual(len(units), 3)
        self.assertTrue(units[1].startswith("```"))

    def test_table_rows_and_headings_are_units(self):
        self.assertEqual(seg.segment("# Title\n| a | b |\n| 1. | 2. |"), ["# Title", "| a | b |", "| 1. | 2. |"])

    def test_list_items_start_new_units(self):
        self.assertEqual(seg.segment("- first item\n- second item"), ["first item", "second item"])


fz = _load("freeze_stage2")


def _rows(rule, pos, neg=0):
    return [{"rule": rule, "label": 1}] * pos + [{"rule": rule, "label": 0}] * neg


class FreezeMenuGuard(unittest.TestCase):
    """check_menu_positives counts WRITTEN train rows (docs/issues/2026-09-25-codex-freeze-positive-count-guard.md)."""

    def test_exactly_the_target_passes(self):
        self.assertEqual(fz.check_menu_positives(_rows("a", 50), ["a"]), {"a": 50})

    def test_one_under_the_target_raises(self):
        with self.assertRaises(AssertionError):
            fz.check_menu_positives(_rows("a", 50) + _rows("b", 49), ["a", "b"])

    def test_menu_rule_with_no_rows_raises(self):
        # the probe the review ran: every positive train row gone
        with self.assertRaises(AssertionError):
            fz.check_menu_positives(_rows("a", 0, neg=60), ["a"])

    def test_negatives_do_not_count(self):
        with self.assertRaises(AssertionError):
            fz.check_menu_positives(_rows("a", 49, neg=10), ["a"])

    def test_off_menu_positives_do_not_count(self):
        with self.assertRaises(AssertionError):
            fz.check_menu_positives(_rows("a", 49) + _rows("z", 10), ["a"])


# Last, after every TestCase: a direct run stops defining tests at this line
# (docs/issues/2026-09-25-codex-freeze-tests-after-main.md).
if __name__ == "__main__":
    unittest.main()
