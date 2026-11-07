"""Coverage for scripts/census_guide_deliveries.py: one row per guide delivery in a transcript.

docs/issues/2026-08-27-guide-topics-are-atomic-nodes-in-an-unmodelled-graph.md: the older probe
(scripts/probe_guide_section_use.py) scores a topic as if it were delivered whole, so its
"never engaged" share is an upper bound. The census counts what was delivered: one row per
injected block, with the block's real byte size, the call that triggered it and what the
session did next. It reads the transcript and nothing else.

Fixtures are synthetic transcripts laid out as Claude Code writes them
(`<project>/<session>/subagents/agent-<id>.jsonl` for a subagent). The marker wording is pinned
to the emitter by `test_markers_match_the_emitter`, which reads src/tools/core/guide_emit.rs.
"""

import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts/census_guide_deliveries.py"
SPEC = importlib.util.spec_from_file_location("census_guide_deliveries", SCRIPT)
census = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = census
SPEC.loader.exec_module(census)

GUIDE_EMIT = (ROOT / "src/tools/core/guide_emit.rs").read_text(encoding="utf-8")


def whole(topic: str, body: str = "body") -> str:
    return (
        f"<!-- auto-injected get_guide('{topic}') — first call this session. Do NOT re-call. -->\n"
        f"{body}\n"
        f"<!-- end auto-injected get_guide('{topic}') -->"
    )


def section(topic: str, heading: str, body: str = "body") -> str:
    return (
        f"<!-- auto-injected get_guide('{topic}') § {heading} — first call this session. -->\n"
        f"{body}\n"
        f"<!-- end auto-injected get_guide('{topic}') § {heading} -->"
    )


def preamble(topic: str, body: str = "intro") -> str:
    return (
        f"<!-- auto-injected get_guide('{topic}') preamble — no section declares this call. -->\n"
        f"{body}\n"
        f"<!-- end auto-injected get_guide('{topic}') preamble -->"
    )


def pointer(topic: str) -> str:
    return f"<!-- get_guide('{topic}') NOT auto-injected: 20000 bytes is over the cap. Call it. -->"


def assistant_call(ts, call_id, name, inp):
    return {
        "type": "assistant",
        "timestamp": ts,
        "message": {"content": [{"type": "tool_use", "id": call_id, "name": name, "input": inp}]},
    }


def user_result(ts, call_id, text):
    return {
        "type": "user",
        "timestamp": ts,
        "message": {
            "content": [{"type": "tool_result", "tool_use_id": call_id, "content": text}]
        },
    }


def user_result_items(ts, call_id, items):
    """A tool_result whose content is a LIST of text items, as the server emits a delivery."""
    return {
        "type": "user",
        "timestamp": ts,
        "message": {
            "content": [
                {
                    "type": "tool_result",
                    "tool_use_id": call_id,
                    "content": [{"type": "text", "text": t} for t in items],
                }
            ]
        },
    }



def write_transcript(path: Path, rows: list) -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("\n".join(json.dumps(r) for r in rows) + "\n", encoding="utf-8")
    return path


class CensusTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.sub = self.root / "proj" / "sess-1" / "subagents" / "agent-a1.jsonl"

    def tearDown(self):
        self.tmp.cleanup()

    def events(self, rows, path=None):
        p = write_transcript(path or self.sub, rows)
        return census.census_transcript(p, profile="p")

    def test_a_section_block_is_one_row_with_its_real_byte_size(self):
        block = section("librarian", "Filter Syntax", "naïve — body")
        rows = [
            assistant_call("2026-10-20T10:00:00Z", "t1", "mcp__codescout__doc", {"action": "find"}),
            user_result("2026-10-20T10:00:01Z", "t1", "result text\n" + block + "\ntrailing note after the block"),
        ]
        ev = self.events(rows)
        self.assertEqual(len(ev), 1)
        self.assertEqual(ev[0]["form"], "section")
        self.assertEqual(ev[0]["topic"], "librarian")
        self.assertEqual(ev[0]["heading"], "Filter Syntax")
        self.assertEqual(ev[0]["bytes"], len(block.encode("utf-8")))
        self.assertEqual(ev[0]["kind"], "subagent")

    def test_the_triggering_call_is_named(self):
        rows = [
            assistant_call("2026-10-20T10:00:00Z", "t1", "mcp__codescout__doc", {"action": "update"}),
            user_result("2026-10-20T10:00:01Z", "t1", section("librarian", "Artifact Model")),
        ]
        ev = self.events(rows)[0]
        self.assertEqual(ev["trigger_tool"], "mcp__codescout__doc")
        self.assertEqual(ev["trigger_action"], "update")

    def test_whole_preamble_and_pointer_forms_are_told_apart(self):
        rows = [
            assistant_call("2026-10-20T10:00:00Z", "t1", "mcp__codescout__symbols", {}),
            user_result(
                "2026-10-20T10:00:01Z",
                "t1",
                whole("symbol-navigation") + "\n" + preamble("librarian") + "\n" + pointer("workspace-state"),
            ),
        ]
        forms = {(e["topic"], e["form"]) for e in self.events(rows)}
        self.assertEqual(
            forms,
            {("symbol-navigation", "whole"), ("librarian", "preamble"), ("workspace-state", "pointer")},
        )

    def test_two_blocks_in_one_result_are_not_paired_across_each_other(self):
        a = section("librarian", "Artifact Model", "a" * 10)
        b = section("tracker-conventions", "Querying with the librarian", "b" * 30)
        rows = [
            assistant_call("2026-10-20T10:00:00Z", "t1", "mcp__codescout__doc", {"action": "find"}),
            user_result("2026-10-20T10:00:01Z", "t1", a + "\n" + b),
        ]
        by_topic = {e["topic"]: e["bytes"] for e in self.events(rows)}
        self.assertEqual(by_topic["librarian"], len(a.encode()))
        self.assertEqual(by_topic["tracker-conventions"], len(b.encode()))

    def test_a_heading_that_contains_an_em_dash_is_kept_whole(self):
        h = "docs/trackers/ — Backing Store, Not a Docs Folder"
        rows = [
            assistant_call("2026-10-20T10:00:00Z", "t1", "mcp__codescout__doc", {"action": "get"}),
            user_result("2026-10-20T10:00:01Z", "t1", section("librarian", h)),
        ]
        self.assertEqual(self.events(rows)[0]["heading"], h)

    def test_what_the_session_did_next_is_counted(self):
        rows = [
            assistant_call("2026-10-20T10:00:00Z", "t1", "mcp__codescout__doc", {"action": "find"}),
            user_result("2026-10-20T10:00:01Z", "t1", section("librarian", "Artifact Model")),
            assistant_call("2026-10-20T10:00:02Z", "t2", "mcp__codescout__grep", {}),
            user_result("2026-10-20T10:00:03Z", "t2", "ok"),
            assistant_call("2026-10-20T10:00:04Z", "t3", "mcp__codescout__symbols", {}),
            user_result("2026-10-20T10:00:05Z", "t3", "ok"),
            assistant_call("2026-10-20T10:00:06Z", "t4", "mcp__codescout__doc", {"action": "get"}),
            user_result("2026-10-20T10:00:07Z", "t4", "ok"),
            assistant_call("2026-10-20T10:00:08Z", "t5", "Bash", {}),
            user_result("2026-10-20T10:00:09Z", "t5", "ok"),
        ]
        ev = self.events(rows)[0]
        self.assertEqual(ev["calls_after"], 4)
        self.assertEqual(ev["next_tools"], ["mcp__codescout__grep", "mcp__codescout__symbols", "mcp__codescout__doc"])
        self.assertEqual(ev["total_calls"], 5)

    def test_a_marker_in_assistant_text_is_a_discussion_not_a_delivery(self):
        rows = [
            {
                "type": "assistant",
                "timestamp": "2026-10-20T10:00:00Z",
                "message": {"content": [{"type": "text", "text": section("librarian", "Artifact Model")}]},
            }
        ]
        self.assertEqual(self.events(rows), [])

    def test_a_main_transcript_is_kind_main(self):
        main = self.root / "proj" / "sess-9.jsonl"
        rows = [
            assistant_call("2026-10-20T10:00:00Z", "t1", "mcp__codescout__doc", {"action": "find"}),
            user_result("2026-10-20T10:00:01Z", "t1", section("librarian", "Artifact Model")),
        ]
        ev = self.events(rows, path=main)[0]
        self.assertEqual(ev["kind"], "main")
        self.assertEqual(ev["session_id"], "sess-9")

    def test_a_subagent_row_carries_the_parent_session_and_agent_id(self):
        rows = [
            assistant_call("2026-10-20T10:00:00Z", "t1", "mcp__codescout__doc", {"action": "find"}),
            user_result("2026-10-20T10:00:01Z", "t1", section("librarian", "Artifact Model")),
        ]
        ev = self.events(rows)[0]
        self.assertEqual(ev["session_id"], "sess-1")
        self.assertEqual(ev["agent_id"], "a1")

    def test_session_start_comes_from_the_first_timestamp(self):
        rows = [
            assistant_call("2026-09-01T10:00:00Z", "t1", "mcp__codescout__doc", {"action": "find"}),
            user_result("2026-10-20T10:00:01Z", "t1", section("librarian", "Artifact Model")),
        ]
        self.assertEqual(self.events(rows)[0]["session_start"], "2026-09-01")

    def test_the_window_keeps_sessions_by_start_date_not_by_mtime(self):
        old = write_transcript(
            self.root / "proj" / "sess-old" / "subagents" / "agent-o.jsonl",
            [
                assistant_call("2026-08-01T10:00:00Z", "t1", "mcp__codescout__doc", {"action": "find"}),
                user_result("2026-08-01T10:00:01Z", "t1", section("librarian", "Artifact Model")),
            ],
        )
        new = write_transcript(
            self.root / "proj" / "sess-new" / "subagents" / "agent-n.jsonl",
            [
                assistant_call("2026-10-20T10:00:00Z", "t1", "mcp__codescout__doc", {"action": "find"}),
                user_result("2026-10-20T10:00:01Z", "t1", section("librarian", "Artifact Model")),
            ],
        )
        rows = census.census_paths([("p", old), ("p", new)], since="2026-10-08")
        self.assertEqual({r["session_id"] for r in rows}, {"sess-new"})

    def test_a_topic_name_that_is_not_real_is_ignored(self):
        rows = [
            assistant_call("2026-10-20T10:00:00Z", "t1", "mcp__codescout__doc", {"action": "find"}),
            user_result("2026-10-20T10:00:01Z", "t1", section("{topic}", "x")),
        ]
        self.assertEqual(self.events(rows), [])

    def test_a_block_in_its_own_content_item_is_a_delivery(self):
        rows = [
            assistant_call("2026-10-20T10:00:00Z", "t1", "mcp__codescout__doc", {"action": "find"}),
            user_result_items(
                "2026-10-20T10:00:01Z",
                "t1",
                ['{"ok": true}', "\n" + section("librarian", "Artifact Model")],
            ),
        ]
        self.assertTrue(self.events(rows)[0]["own_block"])

    def test_a_marker_quoted_inside_a_result_is_not_a_delivery(self):
        quoted = "grep output:\n" + section("librarian", "Artifact Model")
        rows = [
            assistant_call("2026-10-20T10:00:00Z", "t1", "mcp__codescout__run_command", {}),
            user_result_items("2026-10-20T10:00:01Z", "t1", [quoted]),
        ]
        self.assertFalse(self.events(rows)[0]["own_block"])

    def test_a_plain_string_result_cannot_prove_a_delivery(self):
        rows = [
            assistant_call("2026-10-20T10:00:00Z", "t1", "mcp__codescout__doc", {"action": "find"}),
            user_result("2026-10-20T10:00:01Z", "t1", section("librarian", "Artifact Model")),
        ]
        self.assertFalse(self.events(rows)[0]["own_block"])

    def test_summary_counts_only_own_block_rows_as_deliveries(self):
        rows = [
            {"kind": "subagent", "form": "section", "topic": "librarian", "bytes": 100,
             "session_id": "s", "agent_id": "a", "own_block": True},
            {"kind": "subagent", "form": "section", "topic": "librarian", "bytes": 900,
             "session_id": "s", "agent_id": "b", "own_block": False},
        ]
        text = census.summarize(rows)
        self.assertNotIn("1,000", text)
        self.assertIn("quoted or unproven: 1 rows, 900 bytes", text)


    def test_a_session_copied_into_two_profiles_counts_once(self):
        rows = [
            assistant_call("2026-10-20T10:00:00Z", "t1", "mcp__codescout__doc", {"action": "find"}),
            user_result_items(
                "2026-10-20T10:00:01Z", "t1", ["\n" + section("librarian", "Artifact Model")]
            ),
        ]
        a = write_transcript(self.root / "a" / "proj" / "sess-1" / "subagents" / "agent-a1.jsonl", rows)
        b = write_transcript(self.root / "b" / "proj" / "sess-1" / "subagents" / "agent-a1.jsonl", rows)
        kept, dropped = census.dedupe_copies(
            census.collect_rows([("p1", a), ("p2", b)], since="2026-10-08")
        )
        self.assertEqual(len(kept), 1)
        self.assertEqual(dropped, 1)

    def test_two_real_deliveries_in_one_transcript_are_both_kept(self):
        block = "\n" + section("librarian", "Artifact Model")
        rows = [
            assistant_call("2026-10-20T10:00:00Z", "t1", "mcp__codescout__doc", {"action": "find"}),
            user_result_items("2026-10-20T10:00:01Z", "t1", [block]),
            assistant_call("2026-10-20T10:05:00Z", "t2", "mcp__codescout__doc", {"action": "find"}),
            user_result_items("2026-10-20T10:05:01Z", "t2", [block]),
        ]
        p = write_transcript(self.sub, rows)
        kept, dropped = census.dedupe_copies(census.collect_rows([("p", p)], since="2026-10-08"))
        self.assertEqual((len(kept), dropped), (2, 0))


    def test_the_delivered_text_can_be_asked_for(self):
        blk = section("librarian", "Filter Syntax", "naïve — body")
        got = census._deliveries_in("\n" + blk + "\ntrailing", True, with_text=True)
        self.assertEqual(got[0]["text"], blk)
        self.assertNotIn("text", census._deliveries_in("\n" + blk, True)[0])


    def test_markers_match_the_emitter(self):
        for needle in (
            "<!-- auto-injected get_guide('{topic}') — first call this session",
            "<!-- end auto-injected get_guide('{topic}') -->",
            "<!-- auto-injected get_guide('{topic}') § {} — first call this",
            "<!-- end auto-injected get_guide('{topic}') § {} -->",
            "<!-- auto-injected get_guide('{topic}') preamble — no section",
            "<!-- end auto-injected get_guide('{topic}') preamble -->",
            "<!-- get_guide('{topic}') NOT auto-injected:",
        ):
            self.assertIn(needle, GUIDE_EMIT, f"emitter wording moved: {needle!r}")


if __name__ == "__main__":
    unittest.main()
