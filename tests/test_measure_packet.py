"""Task 2 of the labelled sample: the packet an operator reads to label one assistant message.

Run: ~/work/claude/prompt-engineering/.venv/bin/python -m pytest tests/test_measure_packet.py -v

All transcripts here are SYNTHETIC (tests/measure_corpus_fixture.py); nothing reads a real corpus.
Every load-bearing fixture detail is annotated on its own line with what breaks if it goes.
"""
import hashlib
import importlib.util
import pathlib
import sys
import tempfile
import unittest

REPO_ROOT = pathlib.Path(__file__).resolve().parents[1]
MEASURE = REPO_ROOT / "scripts" / "measure"
sys.path.insert(0, str(MEASURE))
sys.path.insert(0, str(REPO_ROOT / "tests"))


def _load(name):
    spec = importlib.util.spec_from_file_location(f"measure_{name}", MEASURE / f"{name}.py")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


sampler = _load("sampler")
packet = _load("packet")
import measure_corpus_fixture as fx  # noqa: E402

MINUS = "−"  # the sign the context labels use: "−6 … −1"
MARKER = "AFTER-DECISION-MARKER"
JUDGED = "The message (the one you judge)"  # the judged section's title; the bare `The message` is gone
TRIM_MARKER = "[… the earlier part of this message is not shown]"
# UUID-shaped on purpose: a leaked session id is only scrubbed when it has the shape of one.
SID = "5b1f0c2e-1111-4222-8333-444455556666"


class Seq:
    """One transcript's entries with unique uuids and ISO timestamps, so id-leak checks have
    something distinctive to look for."""

    def __init__(self):
        self.entries = []
        self.n = 0

    def _ids(self):
        self.n += 1
        return f"aaaaaaaa-0000-4000-8000-{self.n:012d}", f"2026-09-20T10:{self.n // 60:02d}:{self.n % 60:02d}Z"

    def user(self, text, **extra):
        uuid, ts = self._ids()
        e = fx.user_prompt(uuid, ts, text)
        e.update(extra)
        self.entries.append(e)

    def asst(self, mid, text=None, tools=(), stop=None):
        """Append one assistant entry; return its tool_use ids in order."""
        uuid, ts = self._ids()
        self.entries.append(fx.assistant(uuid, ts, mid, text=text, tool_uses=tools, stop=stop))
        return [f"{uuid}-tu{i}" for i in range(len(tools))]

    def result(self, tool_id, content, is_error=False):
        uuid, ts = self._ids()
        self.entries.append(fx.tool_result(uuid, ts, tool_id, content, is_error=is_error))

    def ids(self):
        out = set()
        for e in self.entries:
            out.update([e["uuid"], e["timestamp"], (e.get("message") or {}).get("id")])
        out.discard(None)
        return out


class PacketCase(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.root = pathlib.Path(self._tmp.name)

    def build(self, top, mid, subagents=None, kind="top"):
        """Write one session and build the packet for the unit whose message id is `mid`."""
        sessions = [{"sid": SID, "profile": "work", "slug": "proj", "entries": top.entries,
                     "subagents": {k: v.entries for k, v in (subagents or {}).items()}}]
        # rebuild the corpus in a fresh subdirectory each call so a test may call build() twice
        root = self.root / f"c{len(list(self.root.iterdir()))}"
        fx.build_corpus(root, sessions)
        units = sampler.frame(root, excluded_sids=set())
        (unit,) = [u for u in units if u.message_id == mid and u.kind == kind]
        p = packet.build_packet(root, unit, "case-1")
        # every packet scenario in this file is also built through a cache and must be byte-identical
        q = packet.build_packet(root, unit, "case-1", cache={})
        self.assertEqual((q.text, q.sha256, q.n_context), (p.text, p.sha256, p.n_context))
        return p

    def section(self, text, title):
        """The body of `## title` up to the next `## ` heading."""
        head = f"## {title}\n"
        self.assertIn(head, text)
        body = text.split(head, 1)[1]
        return body.split("\n## ", 1)[0]


class DecisionPoint(PacketCase):
    def test_nothing_after_the_decision_appears(self):
        s = Seq()
        s.user("please fix foo")
        (t1,) = s.asst("m1", "Running the tests.", tools=[("run_command", {"command": "cargo test"})])
        s.result(t1, '{"exit_code": 101, "stdout": "test foo ... FAILED"}')
        (ut,) = s.asst("m2", "Fixing foo.", tools=[("edit_file", {"path": "a.rs"})])
        s.result(ut, f"{MARKER} edited a.rs")  # the unit's own tool result: never shown
        s.asst("m3", f"{MARKER} next message")  # the next assistant message: never shown
        s.user(f"{MARKER} later operator message")  # a later operator message: never shown
        p = self.build(s, "m2")
        self.assertNotIn(MARKER, p.text)
        # non-vacuous: the packet does carry the decision point and what came before it
        self.assertIn("Running the tests.", p.text)
        self.assertIn("ABOUT TO RUN:\n- edit_file({\"path\": \"a.rs\"})", p.text)

    def test_a_context_result_that_arrives_after_the_decision_is_not_shown(self):
        s = Seq()
        s.user("go")
        (t0,) = s.asst("m0", "Started a run.", tools=[("run_command", {"command": "slow"})])
        s.asst("m1", "The decision.")  # the unit
        s.result(t0, f"{MARKER} late result")  # m0's result arrives AFTER m1 began: must not leak
        p = self.build(s, "m1")
        self.assertNotIn(MARKER, p.text)
        self.assertIn("Started a run.", p.text)  # m0 is still context; only its late result is withheld

    def test_the_first_result_for_a_tool_use_id_wins_before_and_after_the_decision(self):
        # (built through build(), which also builds via a cache and requires identical bytes)
        s = Seq()
        s.user("go")
        (t0, t1) = s.asst("m0", "Two calls.", tools=[("run_command", {"command": "a"}), ("run_command", {"command": "b"})])
        s.result(t0, "FIRST-A")
        s.result(t0, "SECOND-A")  # a repeated id before the decision: the first block wins
        s.result(t1, "FIRST-B")
        s.asst("m1", "The decision.")
        s.result(t1, "LATE-B")  # after the decision: never shown, and it must not displace FIRST-B
        p = self.build(s, "m1")
        self.assertIn("RESULT FIRST-A", p.text)
        self.assertIn("RESULT FIRST-B", p.text)
        self.assertNotIn("SECOND-A", p.text)
        self.assertNotIn("LATE-B", p.text)


    def test_a_tool_result_with_an_unhashable_id_is_skipped_on_both_paths(self):
        # build() also builds through a cache and requires identical bytes; the cached index covers the WHOLE
        # transcript, so a malformed block AFTER the unit used to raise there while the plain path never saw it
        for where in ("after", "before"):
            with self.subTest(where=where):
                s = Seq()
                s.user("go")
                (t0,) = s.asst("m0", "Ran.", tools=[("run_command", {"command": "a"})])
                if where == "before":
                    s.result(["not", "hashable"], "MALFORMED-EARLY")
                s.result(t0, "GOOD-RESULT")
                s.asst("m1", "The decision.")
                if where == "after":
                    s.result({"also": "unhashable"}, "MALFORMED-LATE")
                p = self.build(s, "m1")
                self.assertIn("RESULT GOOD-RESULT", p.text)
                self.assertNotIn("MALFORMED", p.text)


    def test_a_context_message_piece_after_the_decision_is_not_shown(self):
        s = Seq()
        s.user("go")
        s.asst("m0", "early piece")
        s.asst("m1", "The decision.")  # the unit begins here
        s.asst("m0", f"{MARKER} late piece of m0")  # same message id as context m0, but AFTER the unit began
        p = self.build(s, "m1")
        self.assertNotIn(MARKER, p.text)
        self.assertIn("early piece", p.text)

    def test_pending_action_includes_sibling_entry_tool_calls(self):
        s = Seq()
        s.user("go")
        (ta,) = s.asst("m1", "Part one.", tools=[("run_command", {"command": "ls-A"})])  # unit, entry 1
        s.result(ta, f"{MARKER} result of A")  # between the unit's two entries: must not appear
        s.asst("m1", "Part two.", tools=[("run_command", {"command": "ls-B"})])  # unit, entry 2 (same mid)
        p = self.build(s, "m1")
        msg = self.section(p.text, JUDGED)
        self.assertIn("Part one.\nPart two.", msg)
        after = msg.split("ABOUT TO RUN:", 1)[1]
        self.assertIn('run_command({"command": "ls-A"})', after)
        self.assertIn('run_command({"command": "ls-B"})', after)
        self.assertNotIn(MARKER, p.text)


class ToolOnlyMessage(PacketCase):
    """Bug 05fe7d98e6827714: a judged message with no prose showed only `ABOUT TO RUN:`, which reads as text that
    failed to render, and the judged section was an ordinary `## The message` heading that a long context
    message could be mistaken for."""

    JUDGED = "The message (the one you judge)"
    NOTE = "(no text; the message is only the tool call(s) below)"

    def _tool_only(self, **kw):
        s = Seq()
        s.user("go")
        s.asst("c1", "A long earlier message that is context.", tools=[("run_command", {"command": "ls"})])
        s.asst("m1", None, tools=[("run_command", {"command": "cargo test"})])
        return self.build(s, "m1", **kw)

    def test_a_tool_only_message_says_it_has_no_text(self):
        body = self.section(self._tool_only().text, self.JUDGED)
        self.assertEqual(body.strip(), self.NOTE + '\n\nABOUT TO RUN:\n- run_command({"command": "cargo test"})')

    def test_a_message_with_text_carries_no_no_text_line(self):
        s = Seq()
        s.user("go")
        s.asst("m1", "Running it.", tools=[("run_command", {"command": "cargo test"})])
        p = self.build(s, "m1")
        self.assertNotIn("(no text", p.text)
        # non-vacuous: the same packet does carry the judged section and its calls
        self.assertIn("Running it.\n\nABOUT TO RUN:", self.section(p.text, self.JUDGED))

    def test_the_no_text_line_is_not_placed_in_a_context_message(self):
        # the context message c1 has text; a context message with NO text keeps its old shape (no note)
        s = Seq()
        s.user("go")
        s.asst("c1", None, tools=[("run_command", {"command": "ls"})])
        s.asst("m1", "The decision.")
        p = self.build(s, "m1")
        self.assertNotIn("(no text", p.text)

    def test_the_judged_heading_appears_exactly_once_and_no_context_block_borrows_it(self):
        p = self._tool_only()
        self.assertEqual(p.text.count(f"## {self.JUDGED}\n"), 1)
        self.assertEqual(p.text.count("## The message"), 1)  # the old bare heading is gone, not duplicated
        ctx = self.section(p.text, "Context")
        self.assertNotIn("The message", ctx)

    def test_a_message_with_neither_text_nor_calls_keeps_the_plain_placeholder(self):
        s = Seq()
        s.user("go")
        s.asst("m1", None)
        body = self.section(self.build(s, "m1").text, self.JUDGED)
        self.assertEqual(body.strip(), "(no text)")



class CutMarkers(PacketCase):
    """Bug f32b7d248a833167: a tool result (last 1,500 chars), a tool call's arguments (first 300) and the operator
    or dispatch message (first 1,500) were cut with no marker, so a partial value read as the whole. Per site,
    BOTH directions: a value at the limit is unmarked and one char over is marked (the absence assertion alone is
    monotone under a marker that never fires; the presence alone under one that always does)."""

    def _result_text(self, content, tool="Read"):
        s = Seq()
        s.user("go")
        (t,) = s.asst("m1", "Run.", tools=[(tool, {"file_path": "x"})])
        s.result(t, content)
        s.asst("m2", "The decision.")
        return self.build(s, "m2").text

    def test_a_result_at_the_limit_is_unmarked_and_one_over_says_what_was_dropped(self):
        at = self._result_text("h" * 1500)
        self.assertIn("RESULT " + "h" * 1500 + "\n", at)
        self.assertNotIn("not shown]", at)
        over = self._result_text("HEAD" + "h" * 1497)  # 1,501 chars: the first is dropped
        self.assertIn("RESULT [… 1 earlier character not shown]\n" + "EAD" + "h" * 1497 + "\n", over)

    def test_the_result_marker_counts_the_dropped_characters_with_separators(self):
        text = self._result_text("S" * 2500 + "t" * 1500)  # 4,000 chars: 2,500 dropped
        self.assertIn("RESULT [… 2,500 earlier characters not shown]\n" + "t" * 1500 + "\n", text)

    def test_the_result_marker_follows_the_exit_and_error_prefix_and_leads_the_tail(self):
        text = self._result_text("Exit code 7\n" + "y" * 5000, tool="Bash")
        self.assertIn("RESULT [exit 7] [… 3,512 earlier characters not shown]\n" + "y" * 1500 + "\n", text)

    def test_an_argument_string_at_the_limit_is_unmarked_and_one_over_is_marked(self):
        def calls_line(cmd_len):
            s = Seq()
            s.user("go")
            s.asst("m1", "Go.", tools=[("run_command", {"command": "c" * cmd_len})])
            return self.section(self.build(s, "m1").text, JUDGED)
        at = calls_line(285)  # args JSON = 15 + 285 = 300 chars: kept whole
        self.assertIn("c" * 285 + '"})', at)
        self.assertNotIn("not shown]", at)
        over = calls_line(286)  # 301 chars: only the closing brace is dropped
        self.assertIn("c" * 286 + '"[… 1 more character not shown])', over)

    def test_a_context_call_argument_cut_is_marked_too(self):
        s = Seq()
        s.user("go")
        s.asst("c1", "Ctx.", tools=[("run_command", {"command": "c" * 585})])  # args JSON 600: 300 dropped
        s.asst("m1", "The decision.")
        text = self.build(s, "m1").text
        self.assertIn("c" * 287 + "[… 300 more characters not shown])", text)

    def test_the_operator_message_at_the_limit_is_unmarked_and_one_over_is_marked(self):
        def operator(text):
            s = Seq()
            s.user(text)
            s.asst("m1", "The decision.")
            return self.section(self.build(s, "m1").text, "Operator's last message")
        at = operator("o" * 1500)
        self.assertEqual(at.strip(), "o" * 1500)
        over = operator("o" * 1500 + "XY")
        self.assertEqual(over.strip(), "o" * 1500 + "\n[… 2 more characters not shown]")

    def test_the_dispatch_prompt_cut_is_marked(self):
        top = Seq()
        top.asst("t1", "top.")
        sub = Seq()
        sub.user("d" * 1500 + "Z")  # the hand-back path reads the FIRST user text: 1,501 chars
        sub.asst("h1", "All done.", stop="end_turn")
        p = self.build(top, "h1", subagents={"w": sub}, kind="handback")
        self.assertEqual(self.section(p.text, "Dispatch prompt").strip(), "d" * 1500 + "\n[… 1 more character not shown]")

    def test_the_cut_markers_are_counted_inside_the_cap(self):
        s = Seq()
        s.user("o" * 1502)
        for i in range(6):
            (t,) = s.asst(f"c{i}", f"Ctx {i}.", tools=[("run_command", {"command": "c" * 585})])
            s.result(t, "r" * 4000)
        s.asst("m1", "The decision.")
        p = self.build(s, "m1")
        self.assertLessEqual(len(p.text), 20000)
        self.assertEqual(p.chars, len(p.text))
        self.assertIn("[… 2 more characters not shown]", p.text)  # non-vacuous: the markers are really in this packet



class OperatorSection(PacketCase):
    def _packet_with_operator_text(self, text):
        s = Seq()
        s.user(text)
        s.asst("m1", "The decision.")
        return self.build(s, "m1")

    def test_operator_message_skips_compaction_and_wrappers(self):
        s = Seq()
        s.user("OLDER-OPERATOR-TEXT")
        s.user("REAL-OPERATOR-TEXT")  # the one expected: the last genuine operator entry before the unit
        s.user("COMPACTION-SUMMARY-TEXT", isCompactSummary=True)  # later than the real one: skipped
        s.user("<command-name>/foo</command-name> WRAPPER-TEXT")  # later still: a slash-command wrapper, skipped
        s.asst("m1", "The decision.")
        s.user("LATER-OPERATOR-TEXT")  # after the decision: never shown
        p = self.build(s, "m1")
        op = self.section(p.text, "Operator's last message")
        self.assertEqual(op.strip(), "REAL-OPERATOR-TEXT")
        for absent in ("OLDER-OPERATOR-TEXT", "COMPACTION-SUMMARY-TEXT", "WRAPPER-TEXT", "LATER-OPERATOR-TEXT"):
            self.assertNotIn(absent, p.text)
        self.assertNotIn("Dispatch prompt", p.text)  # a top-level unit has the operator title only

    def test_operator_text_cut_at_1500_both_sides(self):
        at = self._packet_with_operator_text("S" + "m" * 1498 + "E")  # exactly 1,500: kept whole
        self.assertIn("S" + "m" * 1498 + "E", at.text)
        over = self._packet_with_operator_text("S" + "m" * 1499 + "E")  # 1,501: the last char is cut
        self.assertIn("S" + "m" * 1499, over.text)
        self.assertNotIn("S" + "m" * 1499 + "E", over.text)

    def test_no_operator_message_before_the_unit(self):
        s = Seq()
        s.asst("m1", "The decision.")  # nothing precedes it
        s.user("LATER-OPERATOR-TEXT")  # only an operator message AFTER the unit exists
        p = self.build(s, "m1")
        self.assertEqual(self.section(p.text, "Operator's last message").strip(), "(none before this point)")
        self.assertNotIn("LATER-OPERATOR-TEXT", p.text)


class ContextSection(PacketCase):
    def test_a_prior_tool_result_appears_with_its_exit_code(self):
        # Regression test of the new rule for bug 61f699816f5ee2fd: the old judge context carried
        # no tool output, so a failing `cargo test` was invisible to the judge.
        s = Seq()
        s.user("fix it")
        (t,) = s.asst("m1", "Running the tests.", tools=[("run_command", {"command": "cargo test"})])
        s.result(t, '{"exit_code": 101, "stdout": "test foo ... FAILED"}')
        s.asst("m2", "It passes now.")
        p = self.build(s, "m2")
        self.assertIn('RESULT [exit 101] {"exit_code": 101, "stdout": "test foo ... FAILED"}', p.text)

    def test_exit_zero_is_shown(self):
        s = Seq()
        s.user("go")
        (t,) = s.asst("m1", "Run.", tools=[("run_command", {"command": "true"})])
        s.result(t, '{"exit_code": 0}')  # 0 is falsy: a truthiness test would drop the prefix
        s.asst("m2", "The decision.")
        self.assertIn("RESULT [exit 0] ", self.build(s, "m2").text)

    def test_exit_code_outside_the_kept_tail_is_still_shown(self):
        # (form, tool, result). 5,000 y's: the only exit code is at the very start, far outside the last
        # 1,500. The JSON form must be VALID JSON as a whole (a top-level object), or it is not parsed.
        cases = (("json", "run_command", '{"exit_code": 7, "stdout": "' + "y" * 5000 + '"}',
                  "RESULT [exit 7] [… 3,530 earlier characters not shown]\n" + "y" * 1498 + '"}'),
                 ("line", "Bash", "Exit code 7\n" + "y" * 5000,
                  "RESULT [exit 7] [… 3,512 earlier characters not shown]\n" + "y" * 1500))
        for label, tool, body, expected in cases:
            with self.subTest(form=label):
                s = Seq()
                s.user("go")
                (t,) = s.asst("m1", "Run.", tools=[(tool, {"command": "big"})])
                s.result(t, body)
                s.asst("m2", "The decision.")
                p = self.build(s, "m2")
                self.assertIn(expected + "\n", p.text)
                self.assertNotIn("exit_code", p.text)  # the head that held the code is really cut away
                self.assertNotIn("Exit code 7", p.text)

    def test_is_error_flag_is_shown(self):
        s = Seq()
        s.user("go")
        (t,) = s.asst("m1", "Run.", tools=[("run_command", {"command": "bad"})])
        s.result(t, "boom", is_error=True)  # no exit code in the text: only the flag can produce the prefix
        s.asst("m2", "The decision.")
        self.assertIn("RESULT [is_error] boom", self.build(s, "m2").text)

    def test_result_tail_kept_and_args_cut(self):
        s = Seq()
        s.user("go")
        # tool input JSON is '{"command": "' + c*N + '"}' = N + 15 chars.
        at_300 = {"command": "c" * 285}   # 300 chars: kept whole
        over_300 = {"command": "d" * 286}  # 301 chars: only the closing brace is cut
        (t1, t2, t3, t4) = s.asst("m1", "Run.", tools=[("run_command", at_300), ("run_command", over_300),
                                                        ("run_command", {"command": "r1"}),
                                                        ("run_command", {"command": "r2"})])
        s.result(t3, "A" + "b" * 1500)  # 1,501 chars: the first char is cut
        s.result(t4, "A" + "b" * 1499)  # 1,500 chars: kept whole, first char included
        s.asst("m2", "The decision.")
        text = self.build(s, "m2").text
        self.assertIn('CALL run_command({"command": "' + "c" * 285 + '"})\n', text)
        self.assertIn('CALL run_command({"command": "' + "d" * 286 + '"[… 1 more character not shown])\n', text)
        self.assertIn("RESULT [… 1 earlier character not shown]\n" + "b" * 1500 + "\n", text)
        self.assertNotIn("A" + "b" * 1500, text)
        self.assertIn("RESULT A" + "b" * 1499 + "\n", text)

    def test_last_six_context_messages_labelled_oldest_first(self):
        s = Seq()
        s.user("go")
        for i in range(1, 9):  # 8 candidates: the two oldest must fall away
            s.asst(f"c{i}", f"ctx-{i}")
        s.asst("m9", "The decision.")
        p = self.build(s, "m9")
        self.assertEqual(p.n_context, 6)
        ctx = self.section(p.text, "Context")
        self.assertIn(f"### {MINUS}6\nctx-3\n", ctx)
        self.assertIn(f"### {MINUS}1\nctx-8\n", ctx)
        self.assertNotIn("ctx-1", ctx)
        self.assertNotIn("ctx-2", ctx)
        self.assertLess(ctx.index("ctx-3"), ctx.index("ctx-8"))

    def test_fewer_than_six_context_messages_are_labelled_from_minus_n(self):
        s = Seq()
        s.user("go")
        s.asst("c1", "ctx-1")
        s.asst("c2", "ctx-2")
        s.asst("m3", "The decision.")
        p = self.build(s, "m3")
        self.assertEqual(p.n_context, 2)
        self.assertIn(f"### {MINUS}2\nctx-1\n", p.text)
        self.assertNotIn(f"{MINUS}3", p.text)
class ExitPrefix(PacketCase):
    """`[exit N]` is reported only for a SHELL tool's result, from the result's own top-level JSON or
    its own first line; a result that merely quotes such text is not reporting its exit."""

    def _result_line(self, tool, content, is_error=False):
        s = Seq()
        s.user("go")
        (t,) = s.asst("m1", "Run.", tools=[(tool, {"command": "x"})])
        s.result(t, content, is_error=is_error)
        s.asst("m2", "The decision.")
        text = self.build(s, "m2").text
        (line,) = [l for l in text.splitlines() if l.startswith("RESULT")]
        return line

    def test_a_non_shell_result_quoting_an_exit_code_gets_no_prefix(self):
        quoted = '     12\ts.result(t, \'{"exit_code": 101, "stdout": "ok"}\')'
        for tool in ("Read", "grep", "mcp__codescout__read_file"):
            with self.subTest(tool=tool):
                self.assertNotIn("[exit", self._result_line(tool, '{"exit_code": 101}'))
                self.assertNotIn("[exit", self._result_line(tool, quoted))
                self.assertNotIn("[exit", self._result_line(tool, "Exit code 2"))

    def test_shell_tool_names_are_matched_after_the_last_double_underscore(self):
        for tool in ("run_command", "Bash", "mcp__codescout__run_command"):
            with self.subTest(tool=tool):
                self.assertEqual(self._result_line(tool, '{"exit_code": 4}'), 'RESULT [exit 4] {"exit_code": 4}')

    def test_first_line_beats_a_later_json_line_and_top_level_json_beats_text(self):
        # a Bash-style result: the harness's own first line says 2; the later JSON is program output
        self.assertEqual(self._result_line("Bash", 'Exit code 2\n{"exit_code": 0}').split(" ")[:3],
                         ["RESULT", "[exit", "2]"])
        # a run_command-style result: one top-level object; its exit_code is the answer
        self.assertIn("[exit 4] ", self._result_line("run_command", '{"exit_code": 4, "stdout": "Exit code 9"}'))

    def test_an_embedded_json_object_is_not_a_top_level_object(self):
        self.assertNotIn("[exit", self._result_line("run_command", 'note {"exit_code": 5} and more'))
        self.assertIn("[exit 3] ", self._result_line("run_command", 'Exit code 3\nnote {"exit_code": 5}'))

    def test_combined_prefix_order_is_exit_then_is_error(self):
        self.assertEqual(self._result_line("run_command", '{"exit_code": 2}', is_error=True),
                         'RESULT [exit 2] [is_error] {"exit_code": 2}')

    def test_the_compact_run_command_summary_is_not_an_exit_code(self):
        # `cat log` output that begins like format_run_command's summary must not become [exit 3]:
        # that form never reaches a transcript result as a first line, so recognising it can only mis-tag
        self.assertNotIn("[exit", self._result_line("Bash", "✗ exit 3 · 2 FAILED  (query @cmd_ab12)"))
        self.assertNotIn("[exit", self._result_line("run_command", "✓ exit 0 · 5 lines"))


class Handback(PacketCase):
    def test_handback_uses_dispatch_prompt_and_subagent_context(self):
        top = Seq()
        top.user("TOP-LEVEL-OPERATOR-TEXT")  # must not appear: a hand-back packet has no operator section
        top.asst("t1", "top-level assistant text")
        sub = Seq()
        sub.user("S" + "m" * 1499 + "E")  # the dispatch prompt, 1,501 chars: the last char is cut
        (a,) = sub.asst("h1", "Looking around.", tools=[("run_command", {"command": "ls"})])
        sub.result(a, '{"exit_code": 1, "stdout": "boom"}')
        sub.asst("h2", "All done, 3 tests passed.", stop="end_turn")  # the hand-back: last text-bearing message
        p = self.build(top, "h2", subagents={"w": sub}, kind="handback")
        self.assertEqual(self.section(p.text, "Dispatch prompt").strip(), "S" + "m" * 1499 + "\n[… 1 more character not shown]")
        self.assertNotIn("Operator's last message", p.text)
        self.assertNotIn("TOP-LEVEL-OPERATOR-TEXT", p.text)
        self.assertNotIn("top-level assistant text", p.text)
        self.assertIn("Looking around.", p.text)
        self.assertIn('RESULT [exit 1] {"exit_code": 1, "stdout": "boom"}', p.text)
        self.assertIn("All done, 3 tests passed.", self.section(p.text, JUDGED))

    def test_dispatch_prompt_is_the_first_user_text_and_nothing_later(self):
        top = Seq()
        top.asst("t1", "top.")
        sub = Seq()
        sub.user("DISPATCH-ONE")  # the expected prompt: the FIRST user text
        (a,) = sub.asst("h0", None, tools=[("Read", {"file_path": "x"})])
        sub.result(a, "file body")
        sub.user("MID-USER-TEXT")  # a second user text BEFORE the hand-back: must not become the prompt
        sub.asst("h1", "Hand-back text.", stop="end_turn")
        sub.user("LATER-USER-TEXT")  # AFTER the hand-back: must not appear anywhere
        p = self.build(top, "h1", subagents={"w": sub}, kind="handback")
        self.assertEqual(self.section(p.text, "Dispatch prompt").strip(), "DISPATCH-ONE")
        self.assertNotIn("MID-USER-TEXT", p.text)
        self.assertNotIn("LATER-USER-TEXT", p.text)

    def test_no_user_text_before_the_handback_means_none_not_a_later_one(self):
        top = Seq()
        top.asst("t1", "top.")
        sub = Seq()
        sub.asst("h1", "Hand-back text.", stop="end_turn")
        sub.user("LATER-USER-TEXT")  # the only user text is after the unit
        p = self.build(top, "h1", subagents={"w": sub}, kind="handback")
        self.assertEqual(self.section(p.text, "Dispatch prompt").strip(), "(none before this point)")
        self.assertNotIn("LATER-USER-TEXT", p.text)


class Cap(PacketCase):
    """The packet is at most 20,000 chars in TOTAL. Hard-coded budgets (derived by hand from the
    section skeleton, not by the code under test):
      operator "go", no context:        "## Operator's last message\\n\\ngo\\n\\n## Context\\n\\n(no earlier
                                         assistant messages)\\n\\n## The message (the one you judge)\\n\\n" + body + "\\n" -> body budget 19886
      operator of 1,500 chars, ditto:   body budget 18388
      trim marker:                      49 chars"""

    def _packet(self, ctx_texts=(), unit_text="Go.", operator="go", tools=()):
        s = Seq()
        s.user(operator)
        for i, t in enumerate(ctx_texts):
            s.asst(f"c{i}", t)
        s.asst("m9", unit_text, tools=tools)
        return self.build(s, "m9")

    def test_twenty_thousand_cap_drops_oldest_context_first(self):
        # six 4,000-char messages: ~24,000 in total. Dropping one leaves ~20,000+ (over), two leaves
        # ~16,000, so exactly the two oldest must go; margins are hundreds of chars either side.
        texts = [f"MARK{i}" + "x" * 3995 for i in range(1, 7)]
        p = self._packet(texts)
        self.assertEqual(p.n_context, 4)
        self.assertLessEqual(len(p.text), 20000)
        self.assertNotIn("MARK1", p.text)
        self.assertNotIn("MARK2", p.text)
        for kept in ("MARK3", "MARK4", "MARK5", "MARK6"):
            self.assertIn(kept, p.text)
        self.assertIn(f"### {MINUS}4\nMARK3", p.text)  # the survivors are relabelled from -4

    def test_twenty_thousand_boundary_for_context_is_inclusive(self):
        base = self._packet(["c" * 100])
        pad = 100 + 20000 - len(base.text)  # exactly 20,000 chars in total with this pad
        at = self._packet(["c" * pad])
        self.assertEqual(len(at.text), 20000)
        self.assertEqual(at.n_context, 1)  # 20,000 is not over the cap
        over = self._packet(["c" * (pad + 1)])
        self.assertEqual(over.n_context, 0)  # 20,001 is
        self.assertNotIn("cccccccccc", over.text)
        self.assertIn("(no earlier assistant messages)", over.text)
        self.assertEqual(over.text.count(TRIM_MARKER), 0)  # dropping the context was enough: the unit is whole

    def test_the_unit_alone_at_the_body_budget_is_whole_and_one_over_is_trimmed(self):
        for label, operator, budget in (("operator 1500", "o" * 1500, 18388), ("operator go", "go", 19886)):
            with self.subTest(case=label):
                unit = "Q" + "u" * (budget - 2) + "E"  # `budget` chars; "Q" appears nowhere else in a packet
                at = self._packet(["CTX-MARKER " + "c" * 100], unit_text=unit, operator=operator)
                self.assertEqual(len(at.text), 20000)
                self.assertEqual(at.chars, 20000)
                self.assertNotIn(TRIM_MARKER, at.text)
                self.assertIn(unit, at.text)
                self.assertEqual(at.n_context, 0)  # the context went first
                self.assertNotIn("CTX-MARKER", at.text)
                over = self._packet(["CTX-MARKER " + "c" * 100], unit_text=unit + "F", operator=operator)  # budget + 1
                self.assertEqual(len(over.text), 20000)  # the marker is inside the budget
                self.assertEqual(over.chars, len(over.text))
                self.assertEqual(over.text.count(TRIM_MARKER), 1)
                self.assertNotIn("Q", over.text)  # 51 chars fewer of the text: its head is gone
                self.assertTrue(over.text.endswith("EF\n"))  # the END of the message is what is kept

    def test_a_unit_over_25000_is_trimmed_to_exactly_the_remaining_budget(self):
        unit = "Q" + "u" * 24998 + "E"
        p = self._packet(unit_text=unit)
        self.assertEqual(len(p.text), 20000)
        self.assertEqual(p.chars, 20000)
        self.assertEqual(self.section(p.text, JUDGED).strip(), TRIM_MARKER + "\n" + unit[-(19886 - 49 - 1):])

    def _calls(self, n):
        # each pending call renders as "- run_command(" + args JSON of exactly 300 chars (`cmd-NNN-` + 277 z, uncut: a
        # longer one would gain a cut marker and change every line length) + ")" = 315 chars
        return [("run_command", {"command": f"cmd-{i:03d}-" + "z" * 277}) for i in range(n)]

    def test_a_unit_with_100_tool_calls_stays_within_20000_and_says_what_it_left_out(self):
        p = self._packet(unit_text="Go.", tools=self._calls(100))
        self.assertLessEqual(len(p.text), 20000)
        self.assertEqual(p.chars, len(p.text))
        self.assertIn("Go.\n\nABOUT TO RUN:\n- run_command(", p.text)
        self.assertIn("cmd-061-", p.text)  # 62 calls fit beside "Go." (block 19,638 chars <= room 19,881)
        self.assertNotIn("cmd-062-", p.text)
        self.assertIn("[… 38 more tool calls not shown]", p.text)
        self.assertNotIn(TRIM_MARKER, p.text)  # the text was short enough to stay whole

    def test_the_call_list_boundary_is_exact_on_both_sides(self):
        # 62 calls need block = 14 + 316*62 + 30 + 2 = 19,638 chars. With a 266-char text the room is
        # 19,886 - 246 - 2 = 19,638 exactly: fits; with 247 the room is one short: only 61 fit.
        fits = self._packet(unit_text="t" * 246, tools=self._calls(100))
        self.assertEqual(len(fits.text), 20000)
        self.assertIn("cmd-061-", fits.text)
        self.assertNotIn("cmd-062-", fits.text)
        short = self._packet(unit_text="t" * 247, tools=self._calls(100))
        self.assertLessEqual(len(short.text), 20000)
        self.assertIn("cmd-060-", short.text)
        self.assertNotIn("cmd-061-", short.text)
        self.assertIn("[… 39 more tool calls not shown]", short.text)

    def test_a_huge_text_and_huge_call_list_are_both_trimmed_within_budget(self):
        p = self._packet(unit_text="Q" + "u" * 14999 + "E", tools=self._calls(100))
        self.assertLessEqual(len(p.text), 20000)
        self.assertEqual(p.chars, len(p.text))
        self.assertIn(TRIM_MARKER + "\n", p.text)
        self.assertIn("more tool calls not shown]", p.text)
        self.assertNotIn("Q", p.text)
        self.assertIn("uuE\n\nABOUT TO RUN:\n- run_command(", p.text)

    def test_three_tool_calls_that_fit_are_untouched(self):
        p = self._packet(unit_text="Go.", tools=self._calls(3))
        self.assertNotIn("not shown]", p.text)
        for i in range(3):
            self.assertIn(f"cmd-{i:03d}-", p.text)


class Blinding(PacketCase):
    # Real API shapes: "msg_01" / "toolu_01" + >= 20 alphanumerics.
    MSG_A = "msg_01ABCdefGHIjklMNOpqrSTuv"
    MSG_B = "msg_01XYZabcdefghijklmnopqrst"
    TOOLU = "toolu_01QRSdefghijklmnopqrstuv"

    def test_no_ids_or_timestamps_in_text(self):
        s = Seq()
        s.user("go")
        (t,) = s.asst(self.MSG_A, "Run.", tools=[("run_command", {"command": "x"})])
        # a result that quotes ids/timestamps from the transcript's own world (paths, logs)
        s.result(t, f"path /home/x/{SID}.jsonl at 2026-09-20T10:00:01.123Z "
                    f"id {self.MSG_B} tool {self.TOOLU} uuid 123e4567-e89b-12d3-a456-426614174000")
        s.asst("msg_01DECISIONdecisionDECISION", "The decision.")
        p = self.build(s, "msg_01DECISIONdecisionDECISION")
        ids = s.ids()
        self.assertGreaterEqual(len(ids), 8)  # non-vacuous: the fixture really carries distinct uuids/timestamps/mids
        for leaked in ids | {SID, self.MSG_B, self.TOOLU, "123e4567-e89b-12d3-a456-426614174000",
                             "2026-09-20T10:00:01.123Z", "2026-09-20"}:
            self.assertNotIn(leaked, p.text)
        self.assertIn("RESULT path /home/x/<uuid>.jsonl at <timestamp> id <id> tool <id> uuid <uuid>", p.text)

    def test_every_place_transcript_text_enters_the_packet_is_blinded(self):
        # One distinct uuid per entry point: the operator message, a context message's text, a context
        # tool call's args, the unit's text and the unit's own pending-call args. Each is a separate
        # code site, so each needs its own probe; a shared one would let one site's scrub cover the rest.
        ids = {n: f"0000000{n}-aaaa-4bbb-8ccc-dddddddddddd" for n in range(1, 6)}
        s = Seq()
        s.user(f"op {ids[1]}")
        s.asst("c1", f"ctx text {ids[2]}", tools=[("run_command", {"command": f"ctx args {ids[3]}"})])
        s.asst("m2", f"unit text {ids[4]}", tools=[("run_command", {"command": f"unit args {ids[5]}"})])
        p = self.build(s, "m2")
        for n, uid in ids.items():
            with self.subTest(site=n):
                self.assertNotIn(uid, p.text)
        for own in s.ids():  # the fixture's own entry uuids, timestamps and message ids never surface either
            self.assertNotIn(own, p.text)
        for shown in ("op <uuid>", "ctx text <uuid>", "ctx args <uuid>", "unit text <uuid>", "unit args <uuid>"):
            self.assertIn(shown, p.text)

    def test_the_dispatch_prompt_is_blinded(self):
        top = Seq()
        top.asst("t1", "top.")
        sub = Seq()
        sub.user(f"dispatch {SID} at 2026-09-20T10:00:01Z")  # the hand-back path is a separate code site
        sub.asst("h1", "All done.", stop="end_turn")
        p = self.build(top, "h1", subagents={"w": sub}, kind="handback")
        self.assertEqual(self.section(p.text, "Dispatch prompt").strip(), "dispatch <uuid> at <timestamp>")

    def _result_body(self, content):
        s = Seq()
        s.user("go")
        (t,) = s.asst("m1", "Run.", tools=[("Read", {"file_path": "x"})])
        s.result(t, content)
        s.asst("m2", "The decision.")
        return self.build(s, "m2").text.split("RESULT ", 1)[1].split("\n\n## " + JUDGED, 1)[0]

    def test_words_shaped_like_api_ids_are_left_intact(self):
        text = "let msg_count = 3;\nfn toolu_helper() {}\nmsg_01short and toolu_01short"  # no 20 chars after `_01`
        self.assertEqual(self._result_body(text), text)
    def test_a_bare_calendar_date_is_kept_and_only_a_date_with_a_time_is_blinded(self):
        s = Seq()
        s.user("see notes-2026-09-20.md dated 2026-09-20 and stamped 2026-09-20 10:11:12 but 2026-09-20 10:11 stays")
        s.asst("m1", "Done.")
        p = self.build(s, "m1")
        op = self.section(p.text, "Operator's last message")
        self.assertEqual(op.strip(), "see notes-2026-09-20.md dated 2026-09-20 and stamped <timestamp> "
                                     "but 2026-09-20 10:11 stays")  # HH:MM without seconds is not a timestamp here
        self.assertIn("BARE calendar date is kept everywhere", packet.__doc__)  # the docstring says what the code does


    def test_api_id_needs_exactly_twenty_characters_after_the_01_prefix(self):
        nineteen, twenty = "01" + "a" * 19, "01" + "a" * 20
        self.assertEqual(self._result_body(f"x msg_{nineteen} y toolu_{nineteen}"), f"x msg_{nineteen} y toolu_{nineteen}")
        self.assertEqual(self._result_body(f"x msg_{twenty} y toolu_{twenty}"), "x <id> y <id>")


    def test_datetimes_are_blinded_in_iso_and_space_separated_forms(self):
        body = self._result_body("a 2026-09-20T10:00:01+02:00 b 2026-09-20 10:00:01 c "
                                 "2026-09-20 10:00:01.250 +0200 d 2026-09-20 10:00:01Z e")
        self.assertEqual(body, "a <timestamp> b <timestamp> c <timestamp> d <timestamp> e")

    def test_dates_without_a_time_and_in_file_names_stay(self):
        text = "see docs/issues/2026-09-20-the-bug.md and 2026-09-20 only, or 2026-09-20 at noon"
        self.assertEqual(self._result_body(text), text)
    # Bug 8829a4aade8bef7f: each pattern began with `\b`, which does not exist between two word characters,
    # so a value glued to a letter, digit or `_` passed through unmasked. One test per pattern: a glued value
    # is masked (the narrowing direction) and a look-alike is left alone (the widening direction, which the
    # masked cases alone cannot catch: a pattern widened to eat any digit run would satisfy them).

    def test_a_timestamp_glued_to_a_word_character_is_blinded_and_a_lookalike_is_not(self):
        self.assertEqual(self._result_body("run2026-09-30T12:00:00Z"), "run<timestamp>")
        self.assertEqual(self._result_body("id_2026-09-30 12:00:00"), "id_<timestamp>")
        self.assertEqual(self._result_body("2026-09-30T12:00:00Zabc"), "<timestamp>abc")
        self.assertEqual(self._result_body("2026-09-30 12:00:00x"), "<timestamp>x")
        for lookalike in ("12026-09-30 12:00:00",   # a fifth digit before the year: not a calendar date
                          "2026-09-30 12:00:001"):  # a third digit of seconds: not HH:MM:SS
            with self.subTest(lookalike=lookalike):
                self.assertEqual(self._result_body(lookalike), lookalike)

    def test_a_uuid_glued_to_a_word_character_is_blinded_and_a_lookalike_is_not(self):
        u = "11111111-2222-3333-4444-555555555555"
        self.assertEqual(self._result_body(f"x{u}"), "x<uuid>")
        self.assertEqual(self._result_body(f"sid_{u}"), "sid_<uuid>")
        self.assertEqual(self._result_body(f"{u}x"), "<uuid>x")
        for lookalike in (f"a{u}",                                   # hex glued on: a longer hex run, not this uuid
                          f"{u}a",                                   # same on the right
                          "11111111-2222-3333-4444-55555555555g"):   # a non-hex char inside the last group
            with self.subTest(lookalike=lookalike):
                self.assertEqual(self._result_body(lookalike), lookalike)

    def test_an_api_id_glued_to_a_word_character_is_blinded_and_a_lookalike_is_not(self):
        body = "01" + "A" * 20
        self.assertEqual(self._result_body(f"idmsg_{body}"), "id<id>")
        self.assertEqual(self._result_body(f"x_toolu_{body}"), "x_<id>")
        self.assertEqual(self._result_body(f"msg_{body}_tail"), "<id>_tail")
        lookalike = "id msg_01" + "A" * 19  # nineteen after `_01`: the length bound still holds without `\b`
        self.assertEqual(self._result_body(lookalike), lookalike)


class Tokens(PacketCase):
    TOK = "ghp_" + "Z" * 36  # 40 chars, a token-shaped string

    def _with_result(self, content, tool="run_command"):
        s = Seq()
        s.user("go")
        (t,) = s.asst("m1", "Run.", tools=[(tool, {"command": "x"})])
        s.result(t, content)
        s.asst("m2", "The decision.")
        return self.build(s, "m2")

    def test_token_shaped_string_refuses(self):
        tok = "ghp_" + "A" * 36
        with self.assertRaises(packet.TokenFound) as cm:
            self._with_result(f"token {tok} leaked")
        self.assertNotIn("A" * 36, str(cm.exception))  # the refusal must not echo the secret
        with self.assertRaises(packet.TokenFound):
            self._with_result("t github_pat_" + "B" * 50)

    def test_token_boundaries(self):
        self._with_result("ghp_" + "A" * 35)  # one short: not a token
        self._with_result("github_pat_" + "B" * 49)  # one short: not a token
        with self.assertRaises(packet.TokenFound):
            self._with_result("ghp_" + "A" * 36)

    def test_token_in_the_pending_action_refuses(self):
        s = Seq()
        s.user("go")
        s.asst("m1", "Leak.", tools=[("run_command", {"command": "echo ghp_" + "C" * 36})])
        with self.assertRaises(packet.TokenFound):
            self.build(s, "m1")

    def test_token_hits_counts_matches(self):
        self.assertEqual(packet.token_hits("none here"), 0)
        self.assertEqual(packet.token_hits("ghp_" + "A" * 36 + " and ghs_" + "b" * 36), 2)

    # -- a token half-cut by a cut must still refuse: the final text no longer matches the pattern,
    # -- but the secret's body would be rendered. Each case is built so every OTHER guard admits it.

    def test_token_cut_at_its_start_by_the_result_tail_refuses(self):
        # 3000 y's, token at 3000..3040, 1462 w's: the last 1,500 chars start at 3002, inside the token
        with self.assertRaises(packet.TokenFound):
            self._with_result("y" * 3000 + self.TOK + "w" * 1462)

    def test_token_cut_at_its_end_by_the_args_cut_refuses(self):
        # args JSON is '{"command": "' (13 chars) + cmd: the token starts at 294 and the cut is at 300
        cmd = "c" * 281 + self.TOK
        s = Seq()
        s.user("go")
        s.asst("m1", "Leak.", tools=[("run_command", {"command": cmd})])
        with self.assertRaises(packet.TokenFound):
            self.build(s, "m1")

    def test_token_cut_at_the_operator_boundary_refuses(self):
        s = Seq()
        s.user("o" * 1498 + self.TOK)  # 1,500 keeps two chars of the token
        s.asst("m1", "The decision.")
        with self.assertRaises(packet.TokenFound):
            self.build(s, "m1")

    def test_token_cut_at_the_dispatch_boundary_refuses(self):
        top = Seq()
        top.asst("t1", "top.")
        sub = Seq()
        sub.user("o" * 1498 + self.TOK)
        sub.asst("h1", "All done.", stop="end_turn")
        with self.assertRaises(packet.TokenFound):
            self.build(top, "h1", subagents={"w": sub}, kind="handback")

    def test_token_straddling_the_unit_trim_boundary_refuses(self):
        # no context, operator "go": the body budget is 19,886 and the trim keeps the last 19,836 chars,
        # which start at 25,000 - 19,836 = 5,164: the token at 5,160..5,200 is cut at its start
        s = Seq()
        s.user("go")
        s.asst("m1", "u" * 5160 + self.TOK + "u" * (25000 - 5200))
        with self.assertRaises(packet.TokenFound):
            self.build(s, "m1")

    def test_token_wholly_in_a_dropped_head_does_not_refuse(self):
        p = self._with_result(self.TOK + "y" * 4000)  # the tail cut drops the whole token
        self.assertNotIn("ghp_", p.text)
        self.assertIn("RESULT [… 2,540 earlier characters not shown]\n" + "y" * 1500 + "\n", p.text)
        cmd = "c" * 300 + self.TOK  # the args cut drops the whole token
        s = Seq()
        s.user("go")
        s.asst("m1", "Leak.", tools=[("run_command", {"command": cmd})])
        self.assertNotIn("ghp_", self.build(s, "m1").text)
        s = Seq()
        s.user("o" * 1500 + self.TOK)  # the operator cut drops the whole token
        s.asst("m1", "The decision.")
        self.assertNotIn("ghp_", self.build(s, "m1").text)
        s = Seq()  # the unit trim drops the whole token (in the first 100 of 25,000 chars)
        s.user("go")
        s.asst("m1", "u" * 100 + self.TOK + "u" * (25000 - 140))
        p = self.build(s, "m1")
        self.assertNotIn("ghp_", p.text)
        self.assertEqual(p.text.count(TRIM_MARKER), 1)

    def test_token_in_a_context_message_dropped_by_the_cap_does_not_refuse(self):
        s = Seq()
        s.user("go")
        s.asst("c1", "MARK1" + self.TOK + "x" * 3900)  # wholly inside the oldest message, which the cap drops
        (t,) = s.asst("c2", "MARK2" + "x" * 3990, tools=[("run_command", {"command": "x"})])
        s.result(t, "y" * 3000 + self.TOK + "w" * 1462)  # a straddling tail in the second-oldest, also dropped
        for i in range(3, 7):
            s.asst(f"c{i}", f"MARK{i}" + "x" * 3995)
        s.asst("m9", "The decision.")
        p = self.build(s, "m9")
        self.assertEqual(p.n_context, 4)
        self.assertNotIn("MARK1", p.text)
        self.assertNotIn("MARK2", p.text)
        self.assertNotIn("ghp_", p.text)


class ExitCode(unittest.TestCase):
    def test_json_form(self):
        self.assertEqual(packet.exit_code('{"exit_code": 101, "x": 1}'), 101)
        self.assertEqual(packet.exit_code('{"exit_code": 0}'), 0)
        self.assertEqual(packet.exit_code('{"exit_code": -1}'), -1)
        self.assertEqual(packet.exit_code('{\n  "exit_code": 4,\n  "stdout": "x"\n}'), 4)
        self.assertIsNone(packet.exit_code('{"exit_code": null}'))
        self.assertIsNone(packet.exit_code('{"exit_code": true}'))  # a bool is not an exit code
        self.assertIsNone(packet.exit_code('[{"exit_code": 4}]'))  # not a top-level object
        self.assertIsNone(packet.exit_code('note {"exit_code": 5} more'))  # embedded, not the whole text

    def test_line_form_is_the_first_line_only(self):
        self.assertEqual(packet.exit_code("Exit code 2"), 2)
        self.assertEqual(packet.exit_code("Exit code -1\nmore"), -1)
        self.assertEqual(packet.exit_code("Exit code 2\r\nmore"), 2)
        self.assertIsNone(packet.exit_code("abc\nExit code 2"))  # a later line is program output

    def test_compact_summary_form_is_not_recognised(self):
        for text in ("✗ exit 101 · 3 passed · 1 FAILED  (query @cmd_ab12)", "✓ exit 0  (query @cmd_x)",
                     "✗ exit 2 · 14 lines", "✗ timed out"):
            with self.subTest(text=text):
                self.assertIsNone(packet.exit_code(text))

    def test_near_misses_are_none(self):
        for text in ("", "Exit code", "not Exit code 2", "Exit code 2 failed", "exit code 2", "exit_code 2"):
            with self.subTest(text=text):
                self.assertIsNone(packet.exit_code(text))

    def test_first_line_beats_a_later_json_line(self):
        self.assertEqual(packet.exit_code('Exit code 2\n{"exit_code": 0}'), 2)


class Identity(PacketCase):
    def test_same_inputs_same_sha256(self):
        s = Seq()
        s.user("go")
        s.asst("m1", "Text.")
        a, b = self.build(s, "m1"), self.build(s, "m1")
        self.assertEqual(a.text, b.text)
        self.assertEqual(a.sha256, b.sha256)
        self.assertEqual(a.sha256, hashlib.sha256(a.text.encode("utf-8")).hexdigest())
        self.assertEqual(a.chars, len(a.text))
        self.assertEqual(a.token_hits, 0)
        self.assertEqual(a.case_id, "case-1")

    def test_case_id_is_the_seeded_hash_of_the_case_key(self):
        u = sampler.Unit(case_key="c1|transcripts/x.jsonl|m1", copy_id="c1", transcript="transcripts/x.jsonl",
                         message_id="m1", kind="top", stratum="routine", reasons=(), first_entry_index=0,
                         decision_ts="")
        self.assertEqual(packet.case_id_for(u, 7), "4064aa8179")  # literal: sha256("7|c1|transcripts/x.jsonl|m1")[:10]
        self.assertEqual(packet.case_id_for(u, 8), "3e7bfd435c")


class ExactText(PacketCase):
    def test_exact_packet_text_for_a_small_case(self):
        s = Seq()
        s.user("fix it")
        (t,) = s.asst("m1", "Run tests.", tools=[("run_command", {"command": "cargo test"})])
        s.result(t, "Exit code 101\nfailed")
        s.asst("m2", "Done.", tools=[("edit_file", {"path": "a"})])
        expected = (
            "## Operator's last message\n\nfix it\n\n"
            f"## Context\n\n### {MINUS}1\nRun tests.\n"
            'CALL run_command({"command": "cargo test"})\n'
            "RESULT [exit 101] Exit code 101\nfailed\n\n"
            "## The message (the one you judge)\n\nDone.\n\nABOUT TO RUN:\n"
            '- edit_file({"path": "a"})\n'
        )
        self.assertEqual(self.build(s, "m2").text, expected)
class TokenEdges(PacketCase):
    """Overlap edges and the places a token can hide that no cut touches."""
    TOK = "ghp_" + "Z" * 36  # 40 chars

    def _built(self, ctx_result=None, op=None, ctx_text=None, unit_text="The decision.", unit_tools=()):
        s = Seq()
        s.user(op if op is not None else "go")
        if ctx_result is not None or ctx_text is not None:
            (t,) = s.asst("c1", ctx_text, tools=[("run_command", {"command": "x"})])
            if ctx_result is not None:
                s.result(t, ctx_result)
        s.asst("m9", unit_text, tools=unit_tools)
        return self.build(s, "m9")

    def test_result_tail_overlap_edges(self):
        # tail = last 1,500 chars. TOK + 1,500 y's: the tail starts at 40 = the token's end: no overlap.
        p = self._built(ctx_result=self.TOK + "y" * 1500)
        self.assertNotIn("ghp_", p.text)
        # TOK + 1,499 y's: the tail starts at 39 and keeps the token's last char: overlap by one char
        with self.assertRaises(packet.TokenFound):
            self._built(ctx_result=self.TOK + "y" * 1499)

    def test_operator_head_overlap_edges(self):
        p = self._built(op="o" * 1500 + self.TOK)  # the token starts exactly where the cut is: no overlap
        self.assertNotIn("ghp_", p.text)
        with self.assertRaises(packet.TokenFound):
            self._built(op="o" * 1499 + self.TOK)  # one char of the token is kept

    def test_args_head_overlap_edges(self):
        # args JSON is '{"command": "' (13 chars) + cmd + '"}': the token starts at 13 + len(prefix)
        s = Seq()
        s.user("go")
        s.asst("m1", "Leak.", tools=[("run_command", {"command": "c" * 287 + self.TOK})])  # token starts at 300
        self.assertNotIn("ghp_", self.build(s, "m1").text)
        s = Seq()
        s.user("go")
        s.asst("m1", "Leak.", tools=[("run_command", {"command": "c" * 286 + self.TOK})])  # starts at 299
        with self.assertRaises(packet.TokenFound):
            self.build(s, "m1")
    def test_a_context_call_whose_args_cut_lands_inside_a_token_refuses(self):
        s = Seq()
        s.user("go")
        s.asst("c1", "Leak.", tools=[("run_command", {"command": "c" * 281 + self.TOK})])  # args cut inside the token
        s.asst("m9", "The decision.")
        with self.assertRaises(packet.TokenFound):
            self.build(s, "m9")


    def test_a_token_in_uncut_assistant_text_refuses(self):
        # no cut touches these two: only the scan of the final text can see them
        with self.assertRaises(packet.TokenFound):
            self._built(ctx_text="see " + self.TOK)
        with self.assertRaises(packet.TokenFound):
            self._built(unit_text="see " + self.TOK)

    def test_a_call_straddling_a_token_refuses_only_when_the_trim_keeps_it(self):
        big = [("run_command", {"command": f"cmd-{i:03d}-" + "z" * 277}) for i in range(100)]
        straddle = ("run_command", {"command": "c" * 281 + self.TOK})  # the args cut lands inside the token
        kept = big[:5] + [straddle] + big[6:]  # call 5 is shown (62 fit)
        with self.assertRaises(packet.TokenFound):
            self._built(unit_text="Go.", unit_tools=kept)
        dropped = big[:99] + [straddle]  # call 99 is left out of the list
        p = self._built(unit_text="Go.", unit_tools=dropped)
        self.assertNotIn("ghp_", p.text)
        self.assertIn("more tool calls not shown]", p.text)

    def test_a_call_straddling_a_token_refuses_when_only_the_text_is_trimmed(self):
        straddle = ("run_command", {"command": "c" * 281 + self.TOK})
        with self.assertRaises(packet.TokenFound):
            self._built(unit_text="u" * 25000, unit_tools=[straddle])


class UnitTrim(PacketCase):
    def test_a_huge_text_with_a_few_calls_keeps_the_calls_whole_and_fits_exactly(self):
        s = Seq()
        s.user("go")
        s.asst("m1", "Q" + "u" * 24998 + "E", tools=[("run_command", {"command": f"cmd-{i}"}) for i in range(3)])
        p = self.build(s, "m1")
        self.assertEqual(len(p.text), 20000)  # text trimmed by exactly what the calls and marker cost
        self.assertEqual(p.text.count(TRIM_MARKER), 1)
        for i in range(3):
            self.assertIn(f'- run_command({{"command": "cmd-{i}"}})', p.text)
        self.assertNotIn("not shown]", p.text.replace(TRIM_MARKER, ""))
        self.assertNotIn("Q", p.text)
    def test_the_text_trim_never_overflows_when_the_room_equals_the_marker(self):
        # 62 full calls (315-char lines) + one 229-char line: the call block is 19,835 chars, so the room
        # left for a trimmed text is 19,886 - 19,835 - 2 = 49 = len(marker): no room for any text beside it
        calls = [("run_command", {"command": f"cmd-{i:03d}-" + "z" * 277}) for i in range(62)]
        calls.append(("run_command", {"command": "y" * 199}))  # args JSON 214 chars -> line 229
        s = Seq()
        s.user("go")
        s.asst("m1", "Q" + "u" * 24998 + "E", tools=calls)
        p = self.build(s, "m1")
        self.assertLessEqual(len(p.text), 20000)
        self.assertEqual(p.chars, len(p.text))

    def test_a_huge_text_beside_a_huge_call_list_keeps_half_the_budget_for_the_text(self):
        calls = [("run_command", {"command": f"cmd-{i:03d}-" + "z" * 277}) for i in range(100)]
        s = Seq()
        s.user("go")
        s.asst("m1", "Q" + "u" * 14999 + "E", tools=calls)
        body = self.section(self.build(s, "m1").text, JUDGED)
        head = body.lstrip("\n").split("\n\nABOUT TO RUN:", 1)[0]
        self.assertEqual(len(head), 9943)  # half of the 19,886 budget, marker (49) and newline (1) included
        self.assertTrue(head.startswith(TRIM_MARKER + "\n") and head.endswith("uE"))
    def _many_calls_then_sized(self, sized_cmd_len, total=103):
        # 62 full calls (315-char lines), then ONE call of a chosen size at index 62, then filler calls
        calls = [("run_command", {"command": f"cmd-{i:03d}-" + "z" * 277}) for i in range(62)]
        calls.append(("run_command", {"command": "s" * sized_cmd_len}))  # line = 14 + (15 + len) + 1 chars
        calls += [("run_command", {"command": f"cmd-{i:03d}-" + "z" * 277}) for i in range(63, total)]
        return calls

    def test_a_tool_only_message_reaches_exactly_20000_and_never_more(self):
        # text=None: the message is only tool calls, the common shape. Room = the whole body budget
        # 19,886 (operator "go", no context) less the no-text note (53 chars) and its "\n\n" (2) = 19,831.
        # Block for 63 shown =
        # 14 + 62*315 + line + 32 (marker "[… 40 more tool calls not shown]") + 63 newlines = 19,639 + line;
        # a 192-char line fills 19,831 exactly, a 193-char line is one over.
        for cmd_len, shown, left, exact in ((162, True, 40, True), (163, False, 41, False)):
            with self.subTest(cmd_len=cmd_len):
                s = Seq()
                s.user("go")
                s.asst("m1", None, tools=self._many_calls_then_sized(cmd_len))
                p = self.build(s, "m1")
                self.assertIn(f"## {JUDGED}\n\n{ToolOnlyMessage.NOTE}\n\nABOUT TO RUN:\n- run_command(", p.text)  # the note, no stray head
                self.assertEqual(p.chars, len(p.text))
                self.assertLessEqual(len(p.text), 20000)
                if exact:
                    self.assertEqual(len(p.text), 20000)
                self.assertEqual('run_command({"command": "' + "s" * cmd_len + '"})' in p.text, shown)
                self.assertIn(f"[… {left} more tool calls not shown]", p.text)
    def test_a_tool_only_message_drops_only_its_note_when_only_the_note_does_not_fit(self):
        # 63 calls, no text: block = 14 + 62*315 + line + 62 newlines = 19,606 + line. The body budget is 19,886, the
        # note costs 53 + 2, so a block over 19,831 fits ALONE but not beside the note. Both sizes below fit
        # alone; 197 (line 227, block 19,833) leaves 51 spare, MORE than the trim marker (49) -- the size at which
        # an unguarded trim branch would print a trim marker over EMPTY text -- and 220 (line 250) leaves 28.
        for cmd_len in (197, 220):
            with self.subTest(cmd_len=cmd_len):
                s = Seq()
                s.user("go")
                s.asst("m1", None, tools=self._many_calls_then_sized(cmd_len, total=63))
                p = self.build(s, "m1")
                body = self.section(p.text, JUDGED)
                self.assertTrue(body.startswith("\nABOUT TO RUN:\n") or body.startswith("ABOUT TO RUN:\n"), body[:80])
                self.assertNotIn(ToolOnlyMessage.NOTE, p.text)   # the note yielded ...
                self.assertNotIn(TRIM_MARKER, p.text)            # ... nothing was "trimmed" from empty text ...
                self.assertNotIn("not shown]", p.text)           # ... and every call is still listed
                self.assertIn("s" * cmd_len, p.text)
                self.assertIn("cmd-061-", p.text)


    def test_a_token_straddling_the_half_trim_boundary_refuses_beside_a_call_list(self):
        # 15,000-char text beside 100 calls: the text keeps its last half - 49 - 1 = 9,893 chars, i.e.
        # from offset 5,107. A token at 5,080..5,120 is cut at its start.
        tok = "ghp_" + "Z" * 36
        s = Seq()
        s.user("go")
        s.asst("m1", "u" * 5080 + tok + "u" * (15000 - 5120), tools=self._many_calls_then_sized(237, total=100))
        with self.assertRaises(packet.TokenFound):
            self.build(s, "m1")

    def test_a_token_wholly_before_the_half_trim_boundary_renders_beside_a_call_list(self):
        tok = "ghp_" + "Z" * 36
        s = Seq()
        s.user("go")  # token at 5,067..5,107: ends exactly where the kept range begins
        s.asst("m1", "u" * 5067 + tok + "u" * (15000 - 5107), tools=self._many_calls_then_sized(237, total=100))
        p = self.build(s, "m1")
        self.assertNotIn("ghp_", p.text)
        self.assertEqual(p.text.count(TRIM_MARKER), 1)


class Surrogates(PacketCase):
    """A lone surrogate (Claude Code writes the JSON escape `\\ud83d` alone when a truncated output splits
    an emoji) used to reach `text.encode("utf-8")` and raise UnicodeEncodeError, aborting a whole render.
    Every piece that can reach the packet is cleaned in `_blind` (results, args, operator/dispatch text,
    unit text, context text) or in `_call` (tool names); a lone surrogate becomes U+FFFD, a valid pair stays."""

    LONE = "\ud83d"  # the high half of an emoji, alone: what a truncated output leaves behind
    FFFD = "�"

    def assertClean(self, p):
        self.assertIn(self.FFFD, p.text)
        # positive control on the scan itself: an unclean string WOULD be caught by this predicate
        self.assertTrue(any("\ud800" <= c <= "\udfff" for c in "x" + self.LONE))
        self.assertFalse(any("\ud800" <= c <= "\udfff" for c in p.text))
        p.text.encode("utf-8")  # raises UnicodeEncodeError on a surrogate
        self.assertEqual(p.sha256, hashlib.sha256(p.text.encode("utf-8")).hexdigest())
        self.assertEqual(p.chars, len(p.text))
        self.assertLessEqual(p.chars, 20000)

    def test_a_lone_surrogate_in_a_kept_result_builds_with_exact_text_and_sha(self):
        s = Seq()
        s.user("go")
        (t,) = s.asst("m1", "Run.", tools=[("run_command", {"command": "ls"})])
        s.result(t, "before " + self.LONE + " after")
        s.asst("m2", "Done.")
        p = self.build(s, "m2")
        self.assertEqual(
            p.text,
            "## Operator's last message\n\ngo\n\n"
            f"## Context\n\n### {MINUS}1\nRun.\n"
            'CALL run_command({"command": "ls"})\n'
            "RESULT before � after\n\n"
            "## The message (the one you judge)\n\nDone.\n")
        self.assertEqual(p.sha256, "9922c0eafa8bff2b765019f061025cc5626eb24ce4547c94edadc226e800213c")
        self.assertClean(p)

    def test_a_lone_surrogate_in_args_the_operator_message_and_the_unit_text_each_builds(self):
        def with_args():
            s = Seq()
            s.user("go")
            s.asst("m1", "Run.", tools=[("run_command", {"command": "echo " + self.LONE})])
            s.asst("m2", "Done.")
            return s

        def with_operator():
            s = Seq()
            s.user("fix " + self.LONE + " it")
            s.asst("m2", "Done.")
            return s

        def with_unit_text():
            s = Seq()
            s.user("go")
            s.asst("m2", "Done " + self.LONE + ".")
            return s

        def with_unit_args():
            s = Seq()
            s.user("go")
            s.asst("m2", "Done.", tools=[("edit_file", {"path": "a" + self.LONE})])
            return s

        for name, make, needle in [
            ("args", with_args, 'echo �"'),
            ("operator", with_operator, "fix � it"),
            ("unit_text", with_unit_text, "Done �."),
            ("unit_args", with_unit_args, 'a�"'),
        ]:
            with self.subTest(piece=name):
                p = self.build(make(), "m2")
                self.assertIn(needle, p.text)
                self.assertClean(p)

    def test_a_lone_surrogate_in_a_context_message_text_builds(self):
        s = Seq()
        s.user("go")
        s.asst("m1", "Ran " + self.LONE + ".")
        s.asst("m2", "Done.")
        p = self.build(s, "m2")
        self.assertIn("### " + MINUS + "1\nRan �.", p.text)
        self.assertClean(p)


    def test_a_lone_surrogate_in_a_tool_name_builds(self):
        s = Seq()
        s.user("go")
        (t,) = s.asst("m1", "Run.", tools=[("run" + self.LONE + "_cmd", {"command": "ls"})])
        s.result(t, "ok")
        s.asst("m2", "Done.", tools=[("edit" + self.LONE, {"path": "a"})])
        p = self.build(s, "m2")
        self.assertIn('CALL run�_cmd({"command": "ls"})', p.text)
        self.assertIn('- edit�({"path": "a"})', p.text)
        self.assertClean(p)

    def test_a_lone_surrogate_in_an_over_long_unit_still_fits_the_budget(self):
        # 25,000 chars of unit text with the surrogate inside the KEPT tail: offsets are unchanged
        # (one char for one char), so the trim is exactly the same as without the surrogate.
        s = Seq()
        s.user("go")
        s.asst("m1", "u" * 20000 + self.LONE + "u" * 4999)
        p = self.build(s, "m1")
        self.assertEqual(len(p.text), 20000)
        self.assertClean(p)

    def test_a_lone_low_surrogate_and_an_inverted_pair_are_both_cleaned(self):
        # the low half alone (a cut that kept only the tail of the emoji), and low-then-high (not a pair)
        for name, raw in [("low", "a\ude00b"), ("inverted", "a\ude00\ud83db")]:
            with self.subTest(case=name):
                s = Seq()
                s.user("go")
                (t,) = s.asst("m1", "Run.", tools=[("run_command", {"command": "ls"})])
                s.result(t, raw)
                s.asst("m2", "Done.")
                p = self.build(s, "m2")
                self.assertIn("RESULT a" + self.FFFD * (raw.count("\ude00") + raw.count("\ud83d")) + "b", p.text)
                self.assertClean(p)


    def test_a_valid_surrogate_pair_is_preserved_unchanged(self):
        emoji = "\U0001f600"  # one non-BMP char; on the wire it is the pair 😀
        s = Seq()
        s.user("fix " + emoji)
        (t,) = s.asst("m1", "Run " + emoji, tools=[("run_command", {"command": "echo " + emoji})])
        s.result(t, "ok " + emoji)
        s.asst("m2", "Done " + emoji + ".", tools=[("edit_file", {"path": "a" + emoji})])
        p = self.build(s, "m2")
        self.assertNotIn(self.FFFD, p.text)
        self.assertEqual(p.text.count(emoji), 6)
        self.assertEqual(
            p.text,
            f"## Operator's last message\n\nfix {emoji}\n\n"
            f"## Context\n\n### {MINUS}1\nRun {emoji}\n"
            f'CALL run_command({{"command": "echo {emoji}"}})\n'
            f"RESULT ok {emoji}\n\n"
            f"## The message (the one you judge)\n\nDone {emoji}.\n\nABOUT TO RUN:\n"
            f'- edit_file({{"path": "a{emoji}"}})\n')
        self.assertEqual(p.sha256, hashlib.sha256(p.text.encode("utf-8")).hexdigest())






if __name__ == "__main__":
    unittest.main()
