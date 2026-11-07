"""Coverage for scripts/judge_guide_use.py: sharding packets for readers, and scoring their labels.

Layer 2 of the subagent guide measurement (docs/issues/2026-08-27-guide-topics-are-atomic-nodes-in-an-unmodelled-graph.md).
`shard` splits the reader packets into batches a reader can finish. `score` joins the labels of two
readers with the answer key and reports what the labels are worth before reporting what they say:
the false-positive rate on shuffled packets, the agreement between readers, and how the layer-1
predicate compares with the readers in both directions.
"""

import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("judge_guide_use", ROOT / "scripts/judge_guide_use.py")
judge = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = judge
SPEC.loader.exec_module(judge)


def key(i, topic="symbol-navigation", k10=True, control=None):
    return {"id": f"p{i}", "topic": topic, "engaged_k10": k10, "engaged_k50": k10, "control": control, "source_ids": [f"p{i}"]}


def lab(i, label):
    return {"id": f"p{i}", "label": label, "evidence": "e"}


class ShardTests(unittest.TestCase):
    def test_every_packet_lands_in_exactly_one_shard(self):
        packets = [{"id": f"p{i}"} for i in range(50)]
        shards = judge.shard(packets, size=24)
        self.assertEqual([len(s) for s in shards], [24, 24, 2])
        self.assertEqual([p["id"] for s in shards for p in s], [p["id"] for p in packets])

    def test_a_size_below_one_is_refused(self):
        with self.assertRaises(ValueError):
            judge.shard([], size=0)


class LabelTests(unittest.TestCase):
    def test_used_means_explicit_or_plausible(self):
        self.assertTrue(judge.is_used("USED_EXPLICIT"))
        self.assertTrue(judge.is_used("USED_PLAUSIBLE"))
        self.assertFalse(judge.is_used("NOT_USED"))

    def test_an_unknown_label_is_an_error_not_a_silent_not_used(self):
        with self.assertRaises(ValueError):
            judge.is_used("MAYBE")

    def test_loading_labels_rejects_a_missing_or_duplicate_id(self):
        with tempfile.TemporaryDirectory() as d:
            p = Path(d) / "a.jsonl"
            p.write_text(json.dumps(lab(1, "NOT_USED")) + "\n" + json.dumps(lab(1, "USED_EXPLICIT")) + "\n")
            with self.assertRaises(ValueError):
                judge.load_labels([p])


class AgreementTests(unittest.TestCase):
    def test_identical_labels_have_kappa_one(self):
        a = {f"p{i}": l for i, l in enumerate(["USED_EXPLICIT", "NOT_USED", "USED_PLAUSIBLE", "NOT_USED"])}
        self.assertEqual(round(judge.kappa(a, dict(a)), 6), 1.0)

    def test_opposite_labels_have_negative_kappa(self):
        a = {"p0": "NOT_USED", "p1": "USED_EXPLICIT", "p2": "NOT_USED", "p3": "USED_EXPLICIT"}
        b = {"p0": "USED_EXPLICIT", "p1": "NOT_USED", "p2": "USED_EXPLICIT", "p3": "NOT_USED"}
        self.assertLess(judge.kappa(a, b), 0)

    def test_a_single_constant_label_does_not_divide_by_zero(self):
        a = {"p0": "NOT_USED", "p1": "NOT_USED"}
        self.assertEqual(judge.kappa(a, dict(a)), 1.0)


class ScoreTests(unittest.TestCase):
    def report(self, keys, a, b):
        return judge.score(keys, {"A": a, "B": b})

    def test_the_false_positive_rate_is_read_off_the_shuffled_packets_only(self):
        keys = [key(0, control="shuffled"), key(1, control="shuffled"), key(2), key(3)]
        a = {"p0": "USED_EXPLICIT", "p1": "NOT_USED", "p2": "USED_EXPLICIT", "p3": "NOT_USED"}
        r = self.report(keys, a, dict(a))
        self.assertEqual(r["controls"]["A"], {"n": 2, "used": 1, "explicit": 1})

    def test_the_predicate_is_compared_with_the_reader_in_both_directions(self):
        keys = [key(0, k10=True), key(1, k10=True), key(2, k10=False), key(3, k10=False)]
        a = {"p0": "USED_EXPLICIT", "p1": "USED_PLAUSIBLE", "p2": "NOT_USED", "p3": "NOT_USED"}
        r = self.report(keys, a, dict(a))
        # reader says used among predicate-yes: 2 of 2; among predicate-no: 0 of 2
        self.assertEqual(r["predicate"]["A"]["symbol-navigation"], {"yes": (2, 2), "no": (0, 2)})

    def test_disagreements_are_listed_by_id(self):
        keys = [key(0), key(1)]
        a = {"p0": "USED_EXPLICIT", "p1": "NOT_USED"}
        b = {"p0": "NOT_USED", "p1": "NOT_USED"}
        self.assertEqual(self.report(keys, a, b)["disagreements"], ["p0"])

    def test_a_label_missing_for_a_key_is_an_error(self):
        with self.assertRaises(ValueError):
            judge.score([key(0), key(1)], {"A": {"p0": "NOT_USED"}, "B": {"p0": "NOT_USED", "p1": "NOT_USED"}})

    def test_the_text_report_names_the_controls_before_the_findings(self):
        keys = [key(0, control="shuffled"), key(1)]
        a = {"p0": "NOT_USED", "p1": "USED_EXPLICIT"}
        text = judge.format_report(self.report(keys, a, dict(a)))
        self.assertLess(text.index("shuffled"), text.index("predicate"))
class RenderTests(unittest.TestCase):
    def pk(self, **kw):
        base = {
            "id": "pabc", "topic": "symbol-navigation", "heading": None,
            "trigger_tool": "mcp__codescout__symbols", "trigger_action": None,
            "block": "GUIDE TEXT",
            "continuation": [
                {"role": "assistant_text", "text": "look\nat callers"},
                {"role": "tool_use", "name": "mcp__codescout__grep", "input": {"pattern": "p"}},
                {"role": "tool_result", "text": "hit one\nhit two"},
            ],
        }
        base.update(kw)
        return base

    def test_the_same_block_gets_the_same_reference_and_another_block_does_not(self):
        self.assertEqual(judge.guide_ref("A"), judge.guide_ref("A"))
        self.assertNotEqual(judge.guide_ref("A"), judge.guide_ref("B"))

    def test_a_rendered_packet_names_its_guide_file_instead_of_repeating_the_text(self):
        text = judge.render_packet(self.pk(), "guides/g-1.md")
        self.assertIn("guide: guides/g-1.md", text)
        self.assertNotIn("GUIDE TEXT", text)
        self.assertIn("## pabc", text)

    def test_each_event_is_one_line_in_order(self):
        text = judge.render_packet(self.pk(), "guides/g-1.md")
        events = [l for l in text.splitlines() if l.startswith("[")]
        self.assertEqual([e.split("]")[0] + "]" for e in events], ["[assistant_text]", "[tool_use]", "[tool_result]"])
        self.assertIn("look⏎at callers", events[0])

    def test_no_line_is_wider_than_the_limit(self):
        long_ = {"role": "tool_result", "text": "x" * 5000}
        text = judge.render_packet(self.pk(continuation=[long_]), "g.md", width=1000)
        self.assertLessEqual(max(len(l) for l in text.splitlines()), 1000)
        self.assertEqual(text.count("x"), 5000)

    def test_a_rendered_packet_does_not_carry_control_or_answer_fields(self):
        text = judge.render_packet(self.pk(control="shuffled", engaged_k10=True), "g.md")
        self.assertNotIn("shuffled", text)
        self.assertNotIn("engaged", text)


class WriteShardsTests(unittest.TestCase):
    def packets(self):
        def pk(i, block):
            return {
                "id": f"p{i}", "topic": "symbol-navigation", "heading": None,
                "trigger_tool": "mcp__codescout__symbols", "trigger_action": None, "block": block,
                "continuation": [{"role": "assistant_text", "text": f"event {i}"}],
            }
        return [pk(0, "GUIDE A"), pk(1, "GUIDE A"), pk(2, "GUIDE B")]

    def test_one_file_per_packet_an_index_per_shard_and_one_file_per_distinct_guide(self):
        with tempfile.TemporaryDirectory() as d:
            n_shards, n_guides = judge.write_shards(self.packets(), Path(d), size=2)
            self.assertEqual((n_shards, n_guides), (2, 2))
            idx0 = (Path(d) / "shard-00" / "index.txt").read_text().splitlines()
            idx1 = (Path(d) / "shard-01" / "index.txt").read_text().splitlines()
            self.assertEqual((len(idx0), len(idx1)), (2, 1))
            self.assertTrue(all(Path(p).is_absolute() and Path(p).exists() for p in idx0 + idx1))
            self.assertEqual([Path(p).stem for p in idx0], ["p0", "p1"])

    def test_a_packet_file_names_the_guide_file_of_its_own_block(self):
        with tempfile.TemporaryDirectory() as d:
            judge.write_shards(self.packets(), Path(d), size=2)
            text = (Path(d) / "shard-01" / "p2.txt").read_text()
            ref = judge.guide_ref("GUIDE B")
            self.assertIn(f"{ref}.md", text)
            self.assertEqual((Path(d) / "guides" / f"{ref}.md").read_text().strip(), "GUIDE B")


if __name__ == "__main__":
    unittest.main()
