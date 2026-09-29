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
        for label, first in (("json", '{"exit_code": 7, "stdout": "'), ("line", "Exit code 7\n")):
            with self.subTest(form=label):
                s = Seq()
                s.user("go")
                # 5,000 chars: the only exit code is at the very start, far outside the last 1,500
                body = first + "y" * 5000
                (t,) = s.asst("m1", "Run.", tools=[("run_command", {"command": "big"})])
                s.result(t, body)
                s.asst("m2", "The decision.")
                p = self.build(s, "m2")
                self.assertIn("RESULT [exit 7] " + "y" * 1500 + "\n", p.text + "\n")
                self.assertNotIn(first, p.text)  # the head that held the code is really outside the kept tail

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


class Cap(PacketCase):
    def _ctx_packet(self, ctx_texts, unit_text="Go."):
        s = Seq()
        s.user("go")
        for i, t in enumerate(ctx_texts):
            s.asst(f"c{i}", t)
        s.asst("m9", unit_text)
        return self.build(s, "m9")

    def test_twenty_thousand_cap_drops_oldest_context_first(self):
        # six 4,000-char messages: ~24,000 in total. Dropping one leaves ~20,000+ (over), two leaves
        # ~16,000, so exactly the two oldest must go; margins are hundreds of chars either side.
        texts = [f"MARK{i}" + "x" * 3995 for i in range(1, 7)]
        p = self._ctx_packet(texts)
        self.assertEqual(p.n_context, 4)
        self.assertLessEqual(len(p.text), 20000)
        self.assertNotIn("MARK1", p.text)
        self.assertNotIn("MARK2", p.text)
        for kept in ("MARK3", "MARK4", "MARK5", "MARK6"):
            self.assertIn(kept, p.text)
        self.assertIn(f"### {MINUS}4\nMARK3", p.text)  # the survivors are relabelled from -4

    def test_twenty_thousand_boundary_is_inclusive(self):
        base = self._ctx_packet(["c" * 100])
        pad = 100 + 20000 - len(base.text)  # exactly 20,000 chars in total with this pad
        at = self._ctx_packet(["c" * pad])
        self.assertEqual(len(at.text), 20000)
        self.assertEqual(at.n_context, 1)  # 20,000 is not over the cap
        over = self._ctx_packet(["c" * (pad + 1)])
        self.assertEqual(over.n_context, 0)  # 20,001 is
        self.assertNotIn("cccccccccc", over.text)
        self.assertIn("(no earlier assistant messages)", over.text)

    def test_the_unit_is_never_dropped_and_its_last_20000_chars_are_kept(self):
        unit = "Q" + "u" * 24998 + "E"  # 25,000 chars; "Q" appears nowhere else in a packet
        p = self._ctx_packet(["CTX-MARKER text"], unit_text=unit)
        self.assertEqual(p.n_context, 0)  # the context went first
        self.assertNotIn("CTX-MARKER", p.text)
        msg = self.section(p.text, "The message")
        self.assertEqual(msg.strip(), TRIM_MARKER + "\n" + unit[-20000:])
        self.assertNotIn("Q", p.text)

    def test_unit_of_exactly_20000_is_not_trimmed_and_20001_is(self):
        exact = "Q" + "u" * 19998 + "E"  # 20,000
        p = self._ctx_packet(["CTX-MARKER text"], unit_text=exact)
        self.assertNotIn(TRIM_MARKER, p.text)
        self.assertIn(exact, p.text)
        self.assertEqual(p.n_context, 0)  # still over the cap in total: context dropped
        p2 = self._ctx_packet(["CTX-MARKER text"], unit_text="Q" + exact)  # 20,001
        self.assertIn(TRIM_MARKER + "\n" + exact, p2.text)


class Blinding(PacketCase):
    def test_no_ids_or_timestamps_in_text(self):
        s = Seq()
        s.user("go")
        (t,) = s.asst("msg_01ABCdefGHI", "Run.", tools=[("run_command", {"command": "x"})])
        # a result that quotes ids/timestamps from the transcript's own world (paths, logs)
        s.result(t, f"path /home/x/{SID}.jsonl at 2026-09-20T10:00:01.123Z "
                    "id msg_01XYZabc tool toolu_01QRSdef uuid 123e4567-e89b-12d3-a456-426614174000")
        s.asst("msg_01DECISION", "The decision.")
        p = self.build(s, "msg_01DECISION")
        for leaked in s.ids() | {SID, "msg_01XYZabc", "toolu_01QRSdef", "123e4567-e89b-12d3-a456-426614174000",
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
        for shown in (f"op <uuid>", "ctx text <uuid>", "ctx args <uuid>", "unit text <uuid>", "unit args <uuid>"):
            self.assertIn(shown, p.text)

    def test_the_dispatch_prompt_is_blinded(self):
        top = Seq()
        top.asst("t1", "top.")
        sub = Seq()
        sub.user(f"dispatch {SID} at 2026-09-20T10:00:01Z")  # the hand-back path is a separate code site
        sub.asst("h1", "All done.", stop="end_turn")
        p = self.build(top, "h1", subagents={"w": sub}, kind="handback")
        self.assertEqual(self.section(p.text, "Dispatch prompt").strip(), "dispatch <uuid> at <timestamp>")


class Tokens(PacketCase):
    def _with_result(self, content):
        s = Seq()
        s.user("go")
        (t,) = s.asst("m1", "Run.", tools=[("run_command", {"command": "x"})])
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


class ExitCode(unittest.TestCase):
    def test_json_form(self):
        self.assertEqual(packet.exit_code('{"exit_code": 101, "x": 1}'), 101)
        self.assertEqual(packet.exit_code('{"exit_code": 0}'), 0)
        self.assertEqual(packet.exit_code('{"exit_code": -1}'), -1)
        self.assertIsNone(packet.exit_code('{"exit_code": null}'))

    def test_line_form(self):
        self.assertEqual(packet.exit_code("Exit code 2"), 2)
        self.assertEqual(packet.exit_code("abc\nExit code -1\nmore"), -1)  # a line in the middle (multiline)

    def test_near_misses_are_none(self):
        for text in ("", "Exit code", "not Exit code 2", "Exit code 2 failed", "exit code 2", "exit_code 2"):
            with self.subTest(text=text):
                self.assertIsNone(packet.exit_code(text))


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


if __name__ == "__main__":
    unittest.main()
