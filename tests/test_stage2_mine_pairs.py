"""Regression tests for the Stage 2 miner, `mine_pairs.py`.

Run directly (`python3 tests/test_stage2_mine_pairs.py`) or by discovery; the entry guard is last
so both reach every test (tests/python_test_entry_guard.rs fails the build if it is not).

Two bugs, both fixed in a63adc78 and 98dbd016 and neither pinned by anything until this file:

* `docs/issues/archive/2026-09-24-stage2-miner-positive-context-is-the-corrected-text.md`: a row's
  context for a POSITIVE (a removed sentence) was built from the hunk's NEW side, so the correction sat
  beside the sentence it corrects, a label leak. Pinned by `MineKeepsEachSidesContext`, `ChangeBlocks`
  and `Window`.
* `docs/issues/archive/2026-09-24-codex-miner-shingle-pair-undercount.md`: the overlap census kept one
  owner per shingle, so a shingle held by A, B and C recorded A-B and A-C and never B-C: 20 star edges
  published as 25 pairs. Pinned by `OverlappingGroupPairs`.

Machine-local, like the other tests of this campaign: `mine_pairs.py` loads another script from an
absolute path at import.
"""
import collections
import difflib
import importlib.util
import json
import pathlib
import re
import tempfile
import unittest

STAGE2 = pathlib.Path(__file__).resolve().parents[1] / "docs/evals/data/2026-09-24-rule-tell/stage2"


def _load(name):
    spec = importlib.util.spec_from_file_location(name, STAGE2 / f"{name}.py")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


mp = _load("mine_pairs")


class OverlappingGroupPairs(unittest.TestCase):
    """Every unordered pair of document groups sharing a shingle, not one owner per shingle."""

    def test_a_shingle_held_by_three_groups_yields_all_three_pairs(self):
        # The defect's own shape. The star version returned A-B and A-C and never B-C.
        self.assertEqual(
            mp.overlapping_group_pairs({"A": {"s"}, "B": {"s"}, "C": {"s"}}),
            {("A", "B"), ("A", "C"), ("B", "C")},
        )

    def test_a_shingle_held_by_four_groups_yields_six_pairs(self):
        # n choose 2, so a star (n - 1 edges) cannot satisfy it for any n above 2.
        by_group = {g: {"s"} for g in "ABCD"}
        self.assertEqual(len(mp.overlapping_group_pairs(by_group)), 6)

    def test_groups_sharing_nothing_yield_no_pairs(self):
        # The control for the rows above: a function returning every pair of groups passes them.
        self.assertEqual(mp.overlapping_group_pairs({"A": {"x"}, "B": {"y"}, "C": {"z"}}), set())

    def test_a_group_alone_with_a_shingle_yields_no_pair(self):
        self.assertEqual(mp.overlapping_group_pairs({"A": {"x", "y"}}), set())

    def test_a_pair_is_counted_once_however_many_shingles_it_shares(self):
        by_group = {"B": {"s1", "s2", "s3"}, "A": {"s1", "s2", "s3"}}
        # Also pins the orientation: sorted, whatever order the groups were given in.
        self.assertEqual(mp.overlapping_group_pairs(by_group), {("A", "B")})

    def test_the_committed_candidates_give_the_published_pair_count(self):
        # The original defect was a wrong PUBLISHED number, so tie the function to the one printed
        # in the committed summary rather than to a literal that could drift from it.
        rows = [json.loads(l) for l in (STAGE2 / "mined-candidates.jsonl").read_text().splitlines()]
        by_group = collections.defaultdict(set)
        for r in rows:
            by_group[r["doc_group"]] |= mp.shingles(r["positive"]) | mp.shingles(r["twin"] or "")
        published = re.search(
            r"doc-group pairs sharing an 8-token shingle: (\d+)", (STAGE2 / "summary.txt").read_text()
        )
        self.assertIsNotNone(published, "summary.txt no longer prints the pair count")
        self.assertEqual(len(mp.overlapping_group_pairs(by_group)), int(published.group(1)))


class ChangeBlocks(unittest.TestCase):
    """A change carries BOTH sides: a positive is a removed sentence, so its context is the old side."""

    HUNK = [
        " a line of context before the change",
        "-the removed sentence",
        "+the added sentence",
        " a line of context after the change",
    ]

    def test_the_old_side_holds_the_removed_text_and_not_the_added(self):
        (_, _, old_side, _), = mp.change_blocks(self.HUNK)
        self.assertIn("the removed sentence", old_side)
        self.assertNotIn("the added sentence", old_side)

    def test_the_new_side_holds_the_added_text_and_not_the_removed(self):
        (_, _, _, new_side), = mp.change_blocks(self.HUNK)
        self.assertIn("the added sentence", new_side)
        self.assertNotIn("the removed sentence", new_side)

    def test_both_sides_keep_the_surrounding_context(self):
        (_, _, old_side, new_side), = mp.change_blocks(self.HUNK)
        for side in (old_side, new_side):
            self.assertIn("a line of context before the change", side)
            self.assertIn("a line of context after the change", side)

    def test_the_removed_and_added_lines_are_returned_separately(self):
        (rem, add, _, _), = mp.change_blocks(self.HUNK)
        self.assertEqual((rem, add), (["the removed sentence"], ["the added sentence"]))

    def test_two_changes_in_one_hunk_each_carry_both_sides(self):
        hunk = ["-first old", "+first new", " between", "-second old", "+second new"]
        blocks = list(mp.change_blocks(hunk))
        self.assertEqual(len(blocks), 2)
        for _, _, old_side, new_side in blocks:
            self.assertIn("first old", old_side)
            self.assertIn("second new", new_side)

    def test_a_hunk_with_no_change_yields_nothing(self):
        self.assertEqual(list(mp.change_blocks([" only context", " more context"])), [])


class Window(unittest.TestCase):
    """`width` characters CENTRED on the sentence; a prefix left 147 of 940 positives outside theirs."""

    SENT = "THE-POSITIVE-SENTENCE"

    def test_a_sentence_far_from_the_start_is_inside_its_window(self):
        text = "x" * 3000 + self.SENT + "y" * 3000
        # The load-bearing precondition: a prefix of this text does NOT contain the sentence, which
        # is the 147-row defect. If this ever stops holding the test below proves nothing.
        self.assertNotIn(self.SENT, text[:1500])
        out = mp.window(text, self.SENT)
        self.assertIn(self.SENT, out)
        self.assertEqual(len(out), 1500)

    def test_the_window_is_centred_on_the_sentence(self):
        text = "x" * 3000 + self.SENT + "y" * 3000
        out = mp.window(text, self.SENT)
        before, after = out.index(self.SENT), len(out) - out.index(self.SENT) - len(self.SENT)
        self.assertLessEqual(abs(before - after), 1)

    def test_text_no_longer_than_the_width_is_returned_whole(self):
        text = "short " + self.SENT + " text"
        self.assertEqual(mp.window(text, self.SENT), text)

    def test_a_sentence_at_the_start_clamps_to_the_start(self):
        text = self.SENT + "y" * 3000
        self.assertEqual(mp.window(text, self.SENT), text[:1500])

    def test_a_sentence_at_the_end_clamps_to_the_end(self):
        text = "x" * 3000 + self.SENT
        self.assertEqual(mp.window(text, self.SENT), text[-1500:])

    def test_a_sentence_not_in_the_text_falls_back_to_the_prefix(self):
        text = "abc" * 1000
        self.assertEqual(mp.window(text, "not present anywhere"), text[:1500])
    def test_a_missing_sentence_longer_than_the_width_still_falls_back_to_the_prefix(self):
        # The clause that returns the prefix for a missing sentence only matters here. For a short
        # missing sentence the clamp arithmetic already lands on 0, so the row above passes with or
        # without it (a mutation run showed exactly that). A sentence longer than `width` makes
        # `(width - len(sentence)) // 2` negative and pushes the start off 0.
        # NOT a homogeneous string: every window of "x" * 3000 equals every other, so the assertion
        # would hold whatever start the code picked and the clause would again go unpinned (this
        # test first shipped that way and survived the mutation). The period is not a divisor of 249,
        # the start the mutated code picks, so a window from there differs from the prefix.
        text = "abcdefghij" * 300
        self.assertNotEqual(text[249:1749], text[:1500])
        self.assertEqual(mp.window(text, "q" * 2000), text[:1500])


    def test_the_width_is_honoured(self):
        text = "x" * 3000 + self.SENT + "y" * 3000
        self.assertEqual(len(mp.window(text, self.SENT, width=200)), 200)


class MineKeepsEachSidesContext(unittest.TestCase):
    """End to end through `mine()`: no row may carry the correction beside the sentence it corrects."""

    POS = "The retry count is five and that was measured last week on staging with the old build."
    TWIN = "The retry count is three and the earlier figure of five is withdrawn after the staging rerun."
    PATCH = (
        "@@@COMMIT 1111111111111111111111111111111111111111\t2026-09-24\tdocs: settle the retry count\n"
        "diff --git a/docs/x.md b/docs/x.md\n"
        "@@ -1,3 +1,3 @@\n"
        " A leading line of ordinary prose that frames the claim below it for the reader here.\n"
        f"-{POS}\n"
        f"+{TWIN}\n"
        " A trailing line of ordinary prose that closes the paragraph after the claim above it.\n"
    )

    def mine_one(self):
        with tempfile.TemporaryDirectory() as d:
            path = pathlib.Path(d) / "gitlog.patch"
            path.write_text(self.PATCH)
            rows, _, _ = mp.mine(path)
        return rows

    def test_the_fixture_produces_exactly_one_rewrite_row(self):
        # Without this the rows below could pass over an empty list. The similarity the miner needs
        # is checked too, so a reworded fixture cannot quietly stop yielding a pair.
        self.assertGreaterEqual(difflib.SequenceMatcher(None, self.POS, self.TWIN).ratio(), mp.PAIR_MIN)
        rows = self.mine_one()
        self.assertEqual([r["kind"] for r in rows], ["rewrite"])

    def test_the_positives_context_is_the_old_side_and_does_not_hold_the_twin(self):
        (row,) = self.mine_one()
        self.assertIn(self.POS, row["context_before"])
        self.assertNotIn(self.TWIN, row["context_before"])

    def test_the_twins_context_is_the_new_side_and_does_not_hold_the_positive(self):
        (row,) = self.mine_one()
        self.assertIn(self.TWIN, row["context_after"])
        self.assertNotIn(self.POS, row["context_after"])

    def test_no_row_carries_the_old_single_sided_paragraph_field(self):
        (row,) = self.mine_one()
        self.assertNotIn("paragraph", row)
class MineKeepsNoteContext(unittest.TestCase):
    """The same separation for the OTHER row kind: a kept sentence with a correction note appended.

    `mine()` builds contexts at a second pair of sites for these rows, so the rewrite-row tests above
    say nothing about them: a mutation there would survive.
    """

    KEPT = "The retry count is five and that was measured last week on staging with the old build."
    NOTE = "*Corrected 2026-09-24: the earlier figure of five came from a stale build and is not established.*"
    PATCH = (
        "@@@COMMIT 2222222222222222222222222222222222222222\t2026-09-24\tdocs: note on the retry count\n"
        "diff --git a/docs/y.md b/docs/y.md\n"
        "@@ -1,3 +1,4 @@\n"
        " A leading line of ordinary prose that frames the claim below it for the reader here.\n"
        f"-{KEPT}\n"
        f"+{KEPT}\n"
        f"+{NOTE}\n"
        " A trailing line of ordinary prose that closes the paragraph after the claim above it.\n"
    )

    def mine_one(self):
        with tempfile.TemporaryDirectory() as d:
            path = pathlib.Path(d) / "gitlog.patch"
            path.write_text(self.PATCH)
            rows, _, _ = mp.mine(path)
        return rows

    def test_the_fixture_produces_exactly_one_note_row(self):
        self.assertEqual([r["kind"] for r in self.mine_one()], ["note"])

    def test_the_kept_sentences_context_does_not_hold_the_note(self):
        (row,) = self.mine_one()
        self.assertIn(self.KEPT, row["context_before"])
        self.assertNotIn("stale build", row["context_before"])

    def test_the_notes_context_is_the_new_side(self):
        (row,) = self.mine_one()
        self.assertIn("stale build", row["context_after"])




if __name__ == "__main__":
    unittest.main()
