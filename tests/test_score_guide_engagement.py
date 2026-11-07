"""Coverage for scripts/score_guide_engagement.py: did a delivery's taught behaviour follow it?

Layer 1 of the subagent guide measurement (docs/issues/2026-08-27-guide-topics-are-atomic-nodes-in-an-unmodelled-graph.md).
Input is the census (scripts/census_guide_deliveries.py), one row per delivered block. For each row
the scorer reads the next K tool calls of the same transcript and asks a mechanical question per
topic: did one of them do what the guide teaches?

This measures RELEVANCE, not causation. A zero is strong evidence the block went unused; a one is an
upper bound. Predicates here are chosen by the author and are validated only by layer 2 (a reader).
"""

import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]


def load(name: str):
    spec = importlib.util.spec_from_file_location(name, ROOT / "scripts" / f"{name}.py")
    mod = importlib.util.module_from_spec(spec)
    sys.modules[name] = mod
    spec.loader.exec_module(mod)
    return mod


census = load("census_guide_deliveries")
score = load("score_guide_engagement")


def call(ts, cid, name, inp):
    return {
        "type": "assistant",
        "timestamp": ts,
        "message": {"content": [{"type": "tool_use", "id": cid, "name": name, "input": inp}]},
    }


def result_items(ts, cid, items):
    return {
        "type": "user",
        "timestamp": ts,
        "message": {
            "content": [
                {
                    "type": "tool_result",
                    "tool_use_id": cid,
                    "content": [{"type": "text", "text": t} for t in items],
                }
            ]
        },
    }


def block(topic):
    return (
        f"\n<!-- auto-injected get_guide('{topic}') — first call this session. -->\n"
        f"body\n<!-- end auto-injected get_guide('{topic}') -->"
    )


def plain_result(ts, cid, text):
    return {
        "type": "user",
        "timestamp": ts,
        "message": {"content": [{"type": "tool_result", "tool_use_id": cid, "content": text}]},
    }


class PredicateTests(unittest.TestCase):
    def test_progressive_disclosure_sees_a_buffer_handle(self):
        e = score.engaged
        t = "progressive-disclosure"
        self.assertTrue(e(t, "mcp__codescout__run_command", {"command": "grep ERROR @cmd_ab12cd34"}))
        self.assertTrue(e(t, "mcp__codescout__read_file", {"path": "@tool_0f3a9b21"}))
        self.assertTrue(e(t, "mcp__codescout__read_file", {"path": "@cmd_abc.err"}))
        self.assertTrue(e(t, "mcp__codescout__read_file", {"path": "@tool_x1", "json_path": "$.a"}))
        self.assertTrue(e(t, "mcp__codescout__run_command", {"command": "sleep 1", "run_in_background": True}))

    def test_progressive_disclosure_ignores_an_ordinary_call(self):
        e = score.engaged
        t = "progressive-disclosure"
        self.assertFalse(e(t, "mcp__codescout__grep", {"pattern": "foo", "path": "src"}))
        self.assertFalse(e(t, "mcp__codescout__run_command", {"command": "ls -la", "run_in_background": False}))
        self.assertFalse(e(t, "Bash", {"command": "echo cmd tool file"}))

    def test_symbol_navigation_sees_the_taught_forms(self):
        e = score.engaged
        t = "symbol-navigation"
        self.assertTrue(e(t, "mcp__codescout__symbols", {"name": "Container/member"}))
        self.assertTrue(e(t, "mcp__codescout__symbols", {"path": "src/", "kind": "struct"}))
        self.assertTrue(e(t, "mcp__codescout__symbols", {"name": "x", "include_body": True}))
        for tool in ("references", "call_graph", "symbol_at"):
            self.assertTrue(e(t, f"mcp__codescout__{tool}", {"symbol": "x"}))

    def test_symbol_navigation_ignores_a_bare_name_lookup(self):
        e = score.engaged
        self.assertFalse(e("symbol-navigation", "mcp__codescout__symbols", {"name": "foo"}))
        self.assertFalse(e("symbol-navigation", "mcp__codescout__grep", {"pattern": "a/b"}))

    def test_a_topic_without_a_predicate_is_not_scored(self):
        self.assertIsNone(score.engaged("project-activation-bootstrap", "x", {}))


class WilsonTests(unittest.TestCase):
    def test_empty_sample_has_no_interval(self):
        self.assertEqual(score.wilson(0, 0), (0.0, 0.0))

    def test_the_interval_brackets_the_rate_and_stays_in_range(self):
        lo, hi = score.wilson(5, 10)
        self.assertLess(lo, 0.5)
        self.assertGreater(hi, 0.5)
        self.assertGreaterEqual(lo, 0.0)
        self.assertLessEqual(hi, 1.0)

    def test_all_hits_still_has_a_lower_bound_below_one(self):
        lo, hi = score.wilson(10, 10)
        self.assertLess(lo, 1.0)
        self.assertEqual(round(hi, 6), 1.0)


class ScoringTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)

    def tearDown(self):
        self.tmp.cleanup()

    def transcript(self, name, rows):
        p = self.root / "proj" / "s1" / "subagents" / f"agent-{name}.jsonl"
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text("\n".join(json.dumps(r) for r in rows) + "\n", encoding="utf-8")
        return p

    def scored(self, rows, name="a1"):
        p = self.transcript(name, rows)
        events = census.census_transcript(p, profile="p")
        return score.score_events(events, windows=(10, 50))

    def test_the_triggering_call_itself_does_not_count(self):
        rows = [
            call("2026-10-20T10:00:00Z", "t1", "mcp__codescout__symbols", {"name": "A/b"}),
            result_items("2026-10-20T10:00:01Z", "t1", ["{}", block("symbol-navigation")]),
        ]
        ev = self.scored(rows)[0]
        self.assertFalse(ev["engaged_k10"])
        self.assertFalse(ev["engaged_k50"])

    def test_a_behaviour_inside_the_window_counts(self):
        rows = [
            call("2026-10-20T10:00:00Z", "t1", "mcp__codescout__symbols", {"name": "x"}),
            result_items("2026-10-20T10:00:01Z", "t1", ["{}", block("symbol-navigation")]),
            call("2026-10-20T10:00:02Z", "t2", "mcp__codescout__references", {"symbol": "x"}),
            plain_result("2026-10-20T10:00:03Z", "t2", "ok"),
        ]
        ev = self.scored(rows)[0]
        self.assertTrue(ev["engaged_k10"])
        self.assertTrue(ev["engaged_k50"])

    def test_a_behaviour_after_call_twelve_counts_only_in_the_long_window(self):
        rows = [
            call("2026-10-20T10:00:00Z", "t1", "mcp__codescout__symbols", {"name": "x"}),
            result_items("2026-10-20T10:00:01Z", "t1", ["{}", block("symbol-navigation")]),
        ]
        for i in range(11):
            rows.append(call("2026-10-20T10:01:00Z", f"g{i}", "mcp__codescout__grep", {"pattern": "p"}))
            rows.append(plain_result("2026-10-20T10:01:01Z", f"g{i}", "ok"))
        rows.append(call("2026-10-20T10:02:00Z", "late", "mcp__codescout__references", {"symbol": "x"}))
        rows.append(plain_result("2026-10-20T10:02:01Z", "late", "ok"))
        ev = self.scored(rows)[0]
        self.assertFalse(ev["engaged_k10"])
        self.assertTrue(ev["engaged_k50"])

    def test_an_unscored_topic_has_no_flag(self):
        rows = [
            call("2026-10-20T10:00:00Z", "t1", "mcp__codescout__workspace", {}),
            result_items("2026-10-20T10:00:01Z", "t1", ["{}", block("project-activation-bootstrap")]),
        ]
        ev = self.scored(rows)[0]
        self.assertIsNone(ev["engaged_k10"])

    def test_the_summary_reports_rate_and_interval_per_topic(self):
        rows = [
            {"topic": "symbol-navigation", "kind": "subagent", "engaged_k10": True, "engaged_k50": True},
            {"topic": "symbol-navigation", "kind": "subagent", "engaged_k10": False, "engaged_k50": True},
            {"topic": "project-activation-bootstrap", "kind": "subagent", "engaged_k10": None, "engaged_k50": None},
        ]
        text = score.summarize(rows, windows=(10, 50))
        self.assertIn("symbol-navigation", text)
        self.assertIn("1/2", text)
        self.assertIn("2/2", text)
        self.assertIn("not scored", text)

    def test_the_comparison_group_has_a_trigger_call_and_no_delivery(self):
        with_delivery = self.transcript(
            "d1",
            [
                call("2026-10-20T10:00:00Z", "t1", "mcp__codescout__symbols", {"name": "x"}),
                result_items("2026-10-20T10:00:01Z", "t1", ["{}", block("symbol-navigation")]),
            ],
        )
        without = self.transcript(
            "n1",
            [
                call("2026-10-20T10:00:00Z", "t1", "mcp__codescout__symbols", {"name": "x"}),
                plain_result("2026-10-20T10:00:01Z", "t1", "ok"),
                call("2026-10-20T10:00:02Z", "t2", "mcp__codescout__references", {"symbol": "x"}),
                plain_result("2026-10-20T10:00:03Z", "t2", "ok"),
            ],
        )
        never_called = self.transcript(
            "z1",
            [
                call("2026-10-20T10:00:00Z", "t1", "mcp__codescout__grep", {}),
                plain_result("2026-10-20T10:00:01Z", "t1", "ok"),
            ],
        )
        group = score.comparison_group(
            [("p", with_delivery), ("p", without), ("p", never_called)],
            since="2026-10-08",
            windows=(10, 50),
        )
        self.assertEqual(len(group), 1)
        self.assertTrue(group[0]["engaged_k10"])


if __name__ == "__main__":
    unittest.main()
