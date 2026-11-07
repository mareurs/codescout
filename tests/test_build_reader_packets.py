"""Coverage for scripts/build_reader_packets.py: what a layer-2 reader is shown.

Layer 2 of the subagent guide measurement (docs/issues/2026-08-27-guide-topics-are-atomic-nodes-in-an-unmodelled-graph.md).
A reader judges one delivery at a time: the block the subagent was delivered, then what the subagent
did in its next calls. The packet must hold exactly that and nothing that gives the answer away:
no predicate flag, no later census row. The answer key is a separate file.

Controls: a shuffled packet pairs one event's block with ANOTHER event's continuation. A reader that
says "used" there is measuring its own bias.
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
packets = load("build_reader_packets")


def call(cid, name, inp, ts="2026-10-20T10:00:00Z"):
    return {
        "type": "assistant",
        "timestamp": ts,
        "message": {"content": [{"type": "tool_use", "id": cid, "name": name, "input": inp}]},
    }


def say(text, ts="2026-10-20T10:00:00Z"):
    return {
        "type": "assistant",
        "timestamp": ts,
        "message": {"content": [{"type": "text", "text": text}]},
    }


def result(cid, items, ts="2026-10-20T10:00:01Z"):
    return {
        "type": "user",
        "timestamp": ts,
        "message": {
            "content": [
                {"type": "tool_result", "tool_use_id": cid, "content": [{"type": "text", "text": t} for t in items]}
            ]
        },
    }


def block(topic, body="the guide says do X"):
    return (
        f"\n<!-- auto-injected get_guide('{topic}') — first call this session. -->\n"
        f"{body}\n<!-- end auto-injected get_guide('{topic}') -->"
    )


class PacketTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)

    def tearDown(self):
        self.tmp.cleanup()

    def make(self, rows, name="a1", session="s1"):
        p = self.root / "proj" / session / "subagents" / f"agent-{name}.jsonl"
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text("\n".join(json.dumps(r) for r in rows) + "\n", encoding="utf-8")
        return p

    def event_rows(self, rows, **kw):
        p = self.make(rows, **kw)
        return census.census_transcript(p, profile="p")

    def basic(self, n_after=12):
        rows = [
            call("t1", "mcp__codescout__symbols", {"name": "x"}),
            result("t1", ["{}", block("symbol-navigation")]),
            say("Now I look at callers."),
        ]
        for i in range(n_after):
            rows.append(call(f"c{i}", "mcp__codescout__grep", {"pattern": f"p{i}"}))
            rows.append(result(f"c{i}", [f"hit {i}"]))
        return rows

    def test_the_packet_holds_the_delivered_block_text(self):
        ev = self.event_rows(self.basic())[0]
        pk = packets.build_packet(ev)
        self.assertIn("the guide says do X", pk["block"])
        self.assertTrue(pk["block"].startswith("<!-- auto-injected get_guide('symbol-navigation')"))

    def test_the_continuation_is_the_next_n_calls_and_no_earlier_one(self):
        ev = self.event_rows(self.basic())[0]
        pk = packets.build_packet(ev, n_calls=10)
        uses = [c for c in pk["continuation"] if c["role"] == "tool_use"]
        self.assertEqual(len(uses), 10)
        self.assertEqual(uses[0]["input"]["pattern"], "p0")
        self.assertNotIn("symbols", json.dumps(pk["continuation"]))

    def test_assistant_text_and_results_are_in_the_continuation_in_order(self):
        ev = self.event_rows(self.basic())[0]
        roles = [c["role"] for c in packets.build_packet(ev, n_calls=2)["continuation"]]
        self.assertEqual(roles, ["assistant_text", "tool_use", "tool_result", "tool_use", "tool_result"])

    def test_long_strings_are_truncated_and_marked(self):
        rows = [
            call("t1", "mcp__codescout__symbols", {"name": "x"}),
            result("t1", ["{}", block("symbol-navigation")]),
            call("c1", "mcp__codescout__grep", {"pattern": "q" * 5000}),
            result("c1", ["r" * 5000]),
        ]
        pk = packets.build_packet(self.event_rows(rows)[0], n_calls=1, max_chars=200)
        use = next(c for c in pk["continuation"] if c["role"] == "tool_use")
        res = next(c for c in pk["continuation"] if c["role"] == "tool_result")
        self.assertLess(len(use["input"]["pattern"]), 260)
        self.assertIn("[truncated", use["input"]["pattern"])
        self.assertIn("[truncated", res["text"])

    def test_an_unanswered_call_does_not_stretch_the_window(self):
        rows = [
            call("t1", "mcp__codescout__symbols", {"name": "x"}),
            result("t1", ["{}", block("symbol-navigation")]),
            call("c0", "mcp__codescout__grep", {"pattern": "p0"}),
            result("c0", ["hit"]),
            call("c1", "mcp__codescout__grep", {"pattern": "p1"}),  # no result: the session was cut
            call("c2", "mcp__codescout__grep", {"pattern": "p2"}),
            call("c3", "mcp__codescout__grep", {"pattern": "p3"}),
        ]
        pk = packets.build_packet(self.event_rows(rows)[0], n_calls=2)
        uses = [c for c in pk["continuation"] if c["role"] == "tool_use"]
        self.assertEqual([u["input"]["pattern"] for u in uses], ["p0", "p1"])


    def test_a_short_session_gives_a_short_continuation_not_an_error(self):
        pk = packets.build_packet(self.event_rows(self.basic(n_after=3))[0], n_calls=10)
        self.assertEqual(len([c for c in pk["continuation"] if c["role"] == "tool_use"]), 3)

    def test_the_packet_does_not_carry_the_answer(self):
        ev = self.event_rows(self.basic())[0]
        ev["engaged_k10"] = True
        ev["engaged_k50"] = True
        text = json.dumps(packets.build_packet(ev))
        self.assertNotIn("engaged", text)

    def test_the_answer_key_carries_the_flags_and_the_ids(self):
        ev = self.event_rows(self.basic())[0]
        ev["engaged_k10"] = True
        ev["engaged_k50"] = False
        pk = packets.build_packet(ev)
        key = packets.answer_key(ev, pk)
        self.assertEqual(key["id"], pk["id"])
        self.assertTrue(key["engaged_k10"])
        self.assertFalse(key["engaged_k50"])

    def test_a_shuffled_packet_pairs_one_block_with_another_continuation(self):
        a = self.event_rows(self.basic(), name="a1", session="s1")[0]
        rows_b = [
            call("t1", "mcp__codescout__symbols", {"name": "y"}),
            result("t1", ["{}", block("symbol-navigation", "OTHER BODY")]),
            call("z1", "mcp__codescout__doc", {"action": "find", "marker": "ONLY-IN-B"}),
            result("z1", ["found"]),
        ]
        b = self.event_rows(rows_b, name="b1", session="s2")[0]
        pa, pb = packets.build_packet(a), packets.build_packet(b)
        sh = packets.shuffled(pa, pb)
        self.assertIn("the guide says do X", sh["block"])
        self.assertIn("ONLY-IN-B", json.dumps(sh["continuation"]))
        self.assertEqual(sh["control"], "shuffled")
        self.assertNotEqual(sh["id"], pa["id"])

    def test_the_reader_view_hides_the_control_label_and_the_answer(self):
        ev = self.event_rows(self.basic())[0]
        pk = packets.build_packet(ev)
        sh = packets.shuffled(pk, pk)
        view = packets.reader_view(sh)
        self.assertNotIn("control", view)
        self.assertNotIn("shuffled", json.dumps(view))
        self.assertEqual(set(view), {"id", "topic", "heading", "trigger_tool", "trigger_action", "block", "continuation"})


    def test_the_sample_is_deterministic_and_stratified(self):
        rows = []
        for i in range(40):
            rows.append({"topic": "symbol-navigation", "engaged_k10": i % 2 == 0, "trigger_tool": "mcp__codescout__symbols", "n": i})
            rows.append({"topic": "project-activation-bootstrap", "engaged_k10": None, "trigger_tool": "mcp__codescout__read_file", "n": i})
        a = packets.sample(rows, per_stratum=6, seed=7)
        b = packets.sample(rows, per_stratum=6, seed=7)
        self.assertEqual([r["n"] for r in a], [r["n"] for r in b])
        by = {}
        for r in a:
            by.setdefault(packets.stratum(r), []).append(r)
        self.assertEqual({k: len(v) for k, v in by.items()}, {"sn": 6, "bootstrap": 6})

    def test_a_balanced_stratum_takes_half_engaged_and_half_not(self):
        rows = [
            {"topic": "symbol-navigation", "engaged_k10": i < 30, "trigger_tool": "mcp__codescout__symbols", "n": i}
            for i in range(40)
        ]
        s = packets.sample(rows, per_stratum=6, seed=1)
        self.assertEqual(sum(1 for r in s if r["engaged_k10"]), 3)

    def test_progressive_disclosure_is_split_by_how_it_was_triggered(self):
        self.assertEqual(packets.stratum({"topic": "progressive-disclosure", "trigger_tool": "mcp__codescout__run_command"}), "pd-run_command")
        self.assertEqual(packets.stratum({"topic": "progressive-disclosure", "trigger_tool": "mcp__codescout__grep"}), "pd-other")
        self.assertIsNone(packets.stratum({"topic": "librarian", "trigger_tool": "x"}))

    def test_measure_reports_bytes_and_a_token_estimate(self):
        pk = {"block": "a" * 400, "continuation": [{"role": "tool_result", "text": "b" * 600}]}
        text = packets.measure([pk, pk])
        self.assertIn("packets: 2", text)
        self.assertIn("est. tokens", text)


if __name__ == "__main__":
    unittest.main()
