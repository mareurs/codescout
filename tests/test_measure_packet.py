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
        return packet.build_packet(root, unit, "case-1")

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
        msg = self.section(p.text, "The message")
        self.assertIn("Part one.\nPart two.", msg)
        after = msg.split("ABOUT TO RUN:", 1)[1]
        self.assertIn('run_command({"command": "ls-A"})', after)
        self.assertIn('run_command({"command": "ls-B"})', after)
        self.assertNotIn(MARKER, p.text)


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
                  "RESULT [exit 7] " + "y" * 1498 + '"}'),
                 ("line", "Bash", "Exit code 7\n" + "y" * 5000, "RESULT [exit 7] " + "y" * 1500))
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
        self.assertIn('CALL run_command({"command": "' + "d" * 286 + '")\n', text)
        self.assertIn("RESULT " + "b" * 1500 + "\n", text)
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

    def test_the_compact_summary_line_on_the_first_line_is_recognised(self):
        # format from src/tools/run_command/output.rs format_run_command: "{✓|✗} exit N · ..."
        self.assertIn("RESULT [exit 101] ", self._result_line("run_command", "✗ exit 101 · 3 passed · 1 FAILED  (query @cmd_ab12)"))
        self.assertNotIn("[exit", self._result_line("Read", "✗ exit 101 · 3 passed"))


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
        self.assertEqual(self.section(p.text, "Dispatch prompt").strip(), "S" + "m" * 1499)
        self.assertNotIn("Operator's last message", p.text)
        self.assertNotIn("TOP-LEVEL-OPERATOR-TEXT", p.text)
        self.assertNotIn("top-level assistant text", p.text)
        self.assertIn("Looking around.", p.text)
        self.assertIn('RESULT [exit 1] {"exit_code": 1, "stdout": "boom"}', p.text)
        self.assertIn("All done, 3 tests passed.", self.section(p.text, "The message"))

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
                                         assistant messages)\\n\\n## The message\\n\\n" + body + "\\n" -> body budget 19906
      operator of 1,500 chars, ditto:   body budget 18408
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
        for label, operator, budget in (("operator 1500", "o" * 1500, 18408), ("operator go", "go", 19906)):
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
        self.assertEqual(self.section(p.text, "The message").strip(), TRIM_MARKER + "\n" + unit[-(19906 - 49 - 1):])

    def _calls(self, n):
        # each pending call renders as "- run_command(" + 300 chars of args + ")" = 315 chars
        return [("run_command", {"command": f"cmd-{i:03d}-" + "z" * 300}) for i in range(n)]

    def test_a_unit_with_100_tool_calls_stays_within_20000_and_says_what_it_left_out(self):
        p = self._packet(unit_text="Go.", tools=self._calls(100))
        self.assertLessEqual(len(p.text), 20000)
        self.assertEqual(p.chars, len(p.text))
        self.assertIn("Go.\n\nABOUT TO RUN:\n- run_command(", p.text)
        self.assertIn("cmd-061-", p.text)  # 62 calls fit beside "Go." (block 19,638 chars <= room 19,901)
        self.assertNotIn("cmd-062-", p.text)
        self.assertIn("[… 38 more tool calls not shown]", p.text)
        self.assertNotIn(TRIM_MARKER, p.text)  # the text was short enough to stay whole

    def test_the_call_list_boundary_is_exact_on_both_sides(self):
        # 62 calls need block = 14 + 316*62 + 30 + 2 = 19,638 chars. With a 266-char text the room is
        # 19,906 - 266 - 2 = 19,638 exactly: fits; with 267 the room is one short: only 61 fit.
        fits = self._packet(unit_text="t" * 266, tools=self._calls(100))
        self.assertEqual(len(fits.text), 20000)
        self.assertIn("cmd-061-", fits.text)
        self.assertNotIn("cmd-062-", fits.text)
        short = self._packet(unit_text="t" * 267, tools=self._calls(100))
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
        return self.build(s, "m2").text.split("RESULT ", 1)[1].split("\n\n## The message", 1)[0]

    def test_words_shaped_like_api_ids_are_left_intact(self):
        text = "let msg_count = 3;\nfn toolu_helper() {}\nmsg_01short and toolu_01short"  # no 20 chars after `_01`
        self.assertEqual(self._result_body(text), text)
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
        # no context, operator "go": the body budget is 19,906 and the trim keeps the last 19,856 chars,
        # which start at 25,000 - 19,856 = 5,144: the token at 5,140..5,180 is cut at its start
        s = Seq()
        s.user("go")
        s.asst("m1", "u" * 5140 + self.TOK + "u" * (25000 - 5180))
        with self.assertRaises(packet.TokenFound):
            self.build(s, "m1")

    def test_token_wholly_in_a_dropped_head_does_not_refuse(self):
        p = self._with_result(self.TOK + "y" * 4000)  # the tail cut drops the whole token
        self.assertNotIn("ghp_", p.text)
        self.assertIn("RESULT " + "y" * 1500 + "\n", p.text)
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

    def test_compact_summary_form_is_the_first_line_only(self):
        # format_run_command in src/tools/run_command/output.rs
        self.assertEqual(packet.exit_code("✗ exit 101 · 3 passed · 1 FAILED  (query @cmd_ab12)"), 101)
        self.assertEqual(packet.exit_code("✓ exit 0  (query @cmd_x)"), 0)
        self.assertEqual(packet.exit_code("✗ exit 2 · 14 lines"), 2)
        for text in ("✗ timed out", "… running  (query @cmd_x)", "  ✗ exit 3", "ok\n✗ exit 3"):
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
            "## The message\n\nDone.\n\nABOUT TO RUN:\n"
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
        big = [("run_command", {"command": f"cmd-{i:03d}-" + "z" * 300}) for i in range(100)]
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
        # 62 full calls (315-char lines) + one 249-char line: the call block is 19,855 chars, so the room
        # left for a trimmed text is 19,906 - 19,855 - 2 = 49 = len(marker): no room for any text beside it
        calls = [("run_command", {"command": f"cmd-{i:03d}-" + "z" * 300}) for i in range(62)]
        calls.append(("run_command", {"command": "y" * 219}))  # args JSON 234 chars -> line 249
        s = Seq()
        s.user("go")
        s.asst("m1", "Q" + "u" * 24998 + "E", tools=calls)
        p = self.build(s, "m1")
        self.assertLessEqual(len(p.text), 20000)
        self.assertEqual(p.chars, len(p.text))

    def test_a_huge_text_beside_a_huge_call_list_keeps_half_the_budget_for_the_text(self):
        calls = [("run_command", {"command": f"cmd-{i:03d}-" + "z" * 300}) for i in range(100)]
        s = Seq()
        s.user("go")
        s.asst("m1", "Q" + "u" * 14999 + "E", tools=calls)
        body = self.section(self.build(s, "m1").text, "The message")
        head = body.lstrip("\n").split("\n\nABOUT TO RUN:", 1)[0]
        self.assertEqual(len(head), 9953)  # half of the 19,906 budget, marker (49) and newline (1) included
        self.assertTrue(head.startswith(TRIM_MARKER + "\n") and head.endswith("uE"))





if __name__ == "__main__":
    unittest.main()
