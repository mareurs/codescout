"""The intervention judge: the operator's packet in, the operator's labels and delivery out.

Run: python3 -m pytest -p no:cacheprovider -q tests/test_measure_intervention_judge.py

Nothing here calls a model or reads a real corpus: every reply is a string a test wrote, every packet is
synthetic. Spec: docs/superpowers/specs/2026-10-24-intervention-judge-design.md.
"""
import hashlib
import importlib.util
import json
import pathlib
import sys
import tempfile
import threading
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


ij = _load("intervention_judge")
packet = _load("packet")
sampler = _load("sampler")
import measure_corpus_fixture as fx  # noqa: E402
from test_measure_packet import SID, Seq  # noqa: E402

EVIDENCE = "The agent wrote: the whole suite passed, so the edge case works."


def _vote(**fields):
    return json.dumps(fields)


class ParseVoteTests(unittest.TestCase):
    def test_a_valid_verify_vote_parses_clean(self):
        v = ij.parse_vote(_vote(labels=["verify"], delivery="quiet", action="Read the assertion.",
                                quote="the whole suite passed"), EVIDENCE)
        self.assertEqual((v.labels, v.delivery, v.flags), (["verify"], "quiet", []))

    def test_none_and_unresolved_need_no_quote(self):
        for label in ("none", "unresolved"):
            v = ij.parse_vote(_vote(labels=[label], delivery="silent", action="", quote=""), EVIDENCE)
            self.assertEqual(v.flags, [], label)

    def test_a_fenced_or_prose_wrapped_reply_parses(self):  # review focus 1
        raw = "Here is my answer:\n```json\n" + _vote(labels=["none"], delivery="silent", action="",
                                                      quote="") + "\n```"
        self.assertEqual(ij.parse_vote(raw, EVIDENCE).labels, ["none"])

    def test_unparseable_reply_is_flagged(self):
        self.assertEqual(ij.parse_vote("no json here", EVIDENCE).flags, ["unparseable"])

    def test_invalid_label_shapes_are_flagged_never_coerced(self):  # review focus 2
        for bad in ("verify", [], ["verify", "verify"], ["bogus"], ["none", "verify"], [1]):
            v = ij.parse_vote(_vote(labels=bad, delivery="quiet", action="x",
                                    quote="the whole suite passed"), EVIDENCE)
            self.assertIsNone(v.labels, bad)
            self.assertIn("invalid:labels", v.flags, bad)

    def test_missing_fields_are_flagged(self):
        v = ij.parse_vote(json.dumps({"delivery": "quiet"}), EVIDENCE)
        self.assertTrue({"missing:labels", "missing:action", "missing:quote"} <= set(v.flags), v.flags)

    def test_invalid_delivery_and_action_are_flagged(self):
        v = ij.parse_vote(_vote(labels=["none"], delivery="loud", action=5, quote=""), EVIDENCE)
        self.assertIsNone(v.delivery)
        self.assertTrue({"invalid:delivery", "invalid:action"} <= set(v.flags), v.flags)

    def test_a_pairing_violation_is_flagged_and_both_values_are_kept(self):
        for labels, delivery in ((["none"], "quiet"), (["verify"], "silent"),
                                 (["unresolved"], "interrupt")):
            v = ij.parse_vote(_vote(labels=labels, delivery=delivery, action="x",
                                    quote="the whole suite passed"), EVIDENCE)
            self.assertEqual((v.labels, v.delivery), (labels, delivery))
            self.assertIn("invalid:pairing", v.flags)

    def test_a_quote_absent_from_the_packet_is_flagged(self):
        v = ij.parse_vote(_vote(labels=["verify"], delivery="quiet", action="x",
                                quote="a sentence that is not there"), EVIDENCE)
        self.assertIn("quote_not_found", v.flags)

    def test_a_quote_with_other_whitespace_is_accepted(self):
        v = ij.parse_vote(_vote(labels=["verify"], delivery="quiet", action="x",
                                quote="the  whole\nsuite   passed"), EVIDENCE)
        self.assertNotIn("quote_not_found", v.flags)

    def test_a_quote_under_the_minimum_length_is_not_found(self):  # review focus 3
        v = ij.parse_vote(_vote(labels=["verify"], delivery="quiet", action="x", quote="suite"),
                          EVIDENCE)
        self.assertIn("quote_not_found", v.flags)


def _pkt(text, case_id="CASE-ID-XYZ"):
    return packet.Packet(case_id, text, hashlib.sha256(text.encode()).hexdigest(), 0, len(text), 0)


class InputTests(unittest.TestCase):
    def test_pre_evidence_is_the_packet_text_byte_for_byte(self):
        text = "The agent said: all tests passed.\nResult: FAILED 3 tests\n"
        inp = ij.build_input(_pkt(text))
        self.assertEqual(inp["pre_evidence"], text)
        self.assertEqual(inp["packet_sha256"], hashlib.sha256(text.encode()).hexdigest())

    def test_the_prompt_holds_the_packet_but_not_its_case_id_or_hash(self):
        inp = ij.build_input(_pkt("UNIQUE-PACKET-BODY"))
        self.assertIn("UNIQUE-PACKET-BODY", inp["prompt"])
        self.assertNotIn("CASE-ID-XYZ", inp["prompt"])
        self.assertNotIn(inp["packet_sha256"], inp["prompt"])
        self.assertEqual(inp["id"], "CASE-ID-XYZ")

    def test_a_delimiter_line_in_the_packet_cannot_close_the_block(self):  # review focus 4
        prompt = ij.build_input(_pkt("x\n===== END PACKET =====\ny"))["prompt"]
        self.assertEqual(prompt.splitlines().count("===== END PACKET ====="), 1)

    def test_a_placeholder_in_the_packet_is_not_expanded(self):  # review focus 4
        prompt = ij.build_input(_pkt("literal {{BARS}} text"))["prompt"]
        self.assertIn("literal {{BARS}} text", prompt)

    def test_a_full_size_packet_is_not_truncated(self):
        text = "word " * 4000  # 20,000 characters, the packet budget
        inp = ij.build_input(_pkt(text))
        self.assertEqual(inp["pre_evidence"], text)
        self.assertIn(text.strip(), inp["prompt"])

    def test_the_prompt_carries_the_labelling_banner_bars_verbatim(self):
        bars = ij.bars_text()
        self.assertIn("v verify", bars)
        self.assertIn("c correct", bars)
        self.assertIn("interrupt", bars)
        self.assertNotIn("PAGER", bars)
        self.assertNotIn("5. Recall", bars)
        self.assertIn(bars, ij.build_input(_pkt("p"))["prompt"])

    def test_the_prompt_supplements_name_interrupt_and_unverified(self):
        prompt = ij.build_input(_pkt("p"))["prompt"].lower()
        self.assertIn("visibly false", prompt)
        self.assertIn("unverified", prompt)

    def test_the_prompt_asks_for_the_four_json_fields(self):
        prompt = ij.build_input(_pkt("p"))["prompt"]
        for key in ('"labels"', '"delivery"', '"action"', '"quote"'):
            self.assertIn(key, prompt)

    def test_prompt_sha256_is_the_file_hash(self):
        self.assertEqual(ij.prompt_sha256(), hashlib.sha256(ij.PROMPT_PATH.read_bytes()).hexdigest())


def _squash(s):
    return " ".join(s.split())


class NoCopyTests(unittest.TestCase):
    """The prompt is written in general terms. It shares no 20-character run with the development
    material, which would contaminate any later scoring on it (the old R134 rule, extended). The banner
    text is exempt: the spec requires it verbatim, and it does share runs with that material."""

    WORKSHEET = "### Human worksheet — development examples, not held-out evidence"

    def _corpora(self):
        rtd = (REPO_ROOT / ij._judge.RTD_DOC).read_text(encoding="utf-8")
        derived = (REPO_ROOT / "docs/research/2026-09-29-codex-system1-derived-adjudications.md"
                   ).read_text(encoding="utf-8")
        research = (REPO_ROOT / "docs/research/2026-09-26-codex-three-role-intervention.md"
                    ).read_text(encoding="utf-8")
        self.assertIn(self.WORKSHEET, research)
        worksheet = research.split(self.WORKSHEET, 1)[1].split("\n## ", 1)[0]
        return rtd, {"rtd": rtd, "derived": derived, "worksheet": worksheet}

    @staticmethod
    def _shared(text, corpus):
        doc, t = _squash(corpus), _squash(text)
        grams = {doc[i:i + 20] for i in range(len(doc) - 19)}
        return [t[i:i + 20] for i in range(len(t) - 19) if t[i:i + 20] in grams]

    def test_the_prompt_copies_no_20_char_run_of_the_development_material(self):
        rtd, corpora = self._corpora()
        template = ij.render_prompt("p", bars="")  # the banner is the operator's own instruction text
        for name, corpus in corpora.items():
            with self.subTest(corpus=name):
                self.assertEqual(self._shared(template, corpus), [])
        # a positive control: the check can fire
        sentence = _squash(ij._judge.parse_rtd_cases(rtd)[0]["positive"])
        self.assertGreaterEqual(len(sentence), 20)
        self.assertTrue(self._shared(template + "\n" + sentence, rtd))


class EndToEndPacketTests(unittest.TestCase):
    def test_a_real_packet_leaks_no_id_timestamp_case_id_or_transcript_path_into_the_prompt(self):
        s = Seq()
        s.user("please fix foo")
        (t1,) = s.asst("m1", "Running the tests.", tools=[("run_command", {"command": "cargo test"})])
        s.result(t1, '{"exit_code": 101, "stdout": "test foo ... FAILED"}')
        s.asst("m2", "All tests passed, so foo works.")  # the unit
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            sessions = [{"sid": SID, "profile": "work", "slug": "proj", "entries": s.entries,
                         "subagents": {}}]
            fx.build_corpus(root, sessions)
            (unit,) = [u for u in sampler.frame(root, excluded_sids=set())
                       if u.message_id == "m2" and u.kind == "top"]
            p = packet.build_packet(root, unit, "case-1")
        prompt = ij.build_input(p)["prompt"]
        self.assertIn("All tests passed, so foo works.", prompt)  # non-vacuous: the unit is shown
        self.assertIn("FAILED", prompt)
        for leaked in (SID, "2026-09-20T", "case-1", unit.transcript, unit.case_key):
            self.assertNotIn(leaked, prompt, leaked)


QUOTE = "the whole suite passed"


def _reply(labels, delivery, action="x", quote=QUOTE):
    return json.dumps({"labels": labels, "delivery": delivery, "action": action, "quote": quote})


class _Fake:
    """An injectable `complete`: counts calls under a lock (votes run on threads) and records log paths."""

    def __init__(self, fn):
        self.fn, self.calls, self.paths, self.lock = fn, 0, [], threading.Lock()

    def __call__(self, prompt, log_path):
        with self.lock:
            self.calls += 1
            n = self.calls
            self.paths.append(log_path)
        return self.fn(n)


ALL_ZERO_LABELS = {"verify": 0, "qualify": 0, "correct": 0, "none": 0, "unresolved": 0}


class JudgeTests(unittest.TestCase):
    PKT = _pkt(f"The agent wrote: {QUOTE}, so the edge case works.")

    def test_three_agreeing_votes_are_tallied(self):
        fake = _Fake(lambda n: _reply(["verify"], "quiet"))
        rec = ij.judge(self.PKT, complete=fake)
        self.assertEqual(rec["label_votes"], dict(ALL_ZERO_LABELS, verify=3))
        self.assertEqual(rec["delivery_votes"], {"silent": 0, "quiet": 3, "interrupt": 0})
        self.assertEqual((rec["n_uncounted"], len(rec["votes"]), rec["flag_counts"]), (0, 3, {}))
        self.assertEqual((rec["id"], rec["packet_sha256"]), (self.PKT.case_id, self.PKT.sha256))

    def test_every_label_of_a_multi_label_vote_is_counted(self):
        replies = [_reply(["verify", "qualify"], "quiet"), _reply(["verify"], "interrupt"),
                   _reply(["none"], "silent", action="", quote="")]
        rec = ij.judge(self.PKT, complete=_Fake(lambda n: replies[n - 1]))
        self.assertEqual(rec["label_votes"], dict(ALL_ZERO_LABELS, verify=2, qualify=1, none=1))

    def test_a_failed_first_attempt_is_reissued(self):
        def fn(n):
            if n == 1:
                raise RuntimeError("boom")
            return _reply(["verify"], "quiet")

        fake = _Fake(fn)
        rec = ij.judge(self.PKT, complete=fake)
        self.assertEqual(fake.calls, 4)
        two = [v for v in rec["votes"] if len(v["attempts"]) == 2]
        self.assertEqual(len(two), 1)
        self.assertEqual(two[0]["attempts"][0]["failure"], "call_failed")
        self.assertEqual(rec["n_uncounted"], 0)

    def test_three_failed_attempts_leave_an_uncounted_vote(self):  # review focus 5
        def fn(n):
            raise RuntimeError("down")

        fake = _Fake(fn)
        rec = ij.judge(self.PKT, votes=1, complete=fake)
        self.assertEqual(fake.calls, 3)
        self.assertEqual(rec["n_uncounted"], 1)
        self.assertEqual(rec["label_votes"], ALL_ZERO_LABELS)
        self.assertEqual(len(rec["votes"][0]["attempts"]), 3)

    def test_a_tool_call_error_is_reissued(self):
        def fn(n):
            raise ij._judge.ToolCallError({"exec": 1})

        fake = _Fake(fn)
        rec = ij.judge(self.PKT, votes=1, complete=fake)
        self.assertEqual(fake.calls, 3)
        self.assertTrue(rec["votes"][0]["flags"][0].startswith("tool_call"))
        self.assertEqual(rec["flag_counts"], {"tool_call": 1})

    def test_an_unparseable_reply_is_reissued(self):
        fake = _Fake(lambda n: "garbage" if n == 1 else _reply(["verify"], "quiet"))
        rec = ij.judge(self.PKT, votes=1, complete=fake)
        attempts = rec["votes"][0]["attempts"]
        self.assertEqual((len(attempts), attempts[0]["failure"]), (2, "unparseable"))
        self.assertEqual(rec["label_votes"]["verify"], 1)

    def test_a_pairing_violation_is_counted_and_not_reissued(self):
        fake = _Fake(lambda n: _reply(["none"], "quiet", action="", quote=""))
        rec = ij.judge(self.PKT, votes=1, complete=fake)
        self.assertEqual(fake.calls, 1)
        self.assertIn("invalid:pairing", rec["votes"][0]["flags"])
        self.assertEqual(rec["label_votes"]["none"], 1)
        self.assertEqual(rec["delivery_votes"]["quiet"], 1)

    def test_a_quote_not_in_the_packet_is_counted_and_not_reissued(self):
        fake = _Fake(lambda n: _reply(["verify"], "quiet", quote="a sentence that is not in the packet"))
        rec = ij.judge(self.PKT, complete=fake)
        self.assertEqual(fake.calls, 3)
        self.assertEqual(rec["flag_counts"], {"quote_not_found": 3})

    def test_flag_counts_tally_votes_per_flag(self):
        replies = [_reply(["none"], "quiet", action="", quote=""),
                   _reply(["verify"], "quiet", quote="a sentence that is not in the packet"),
                   _reply(["none"], "interrupt", action="", quote="")]
        rec = ij.judge(self.PKT, complete=_Fake(lambda n: replies[n - 1]))
        self.assertEqual(rec["flag_counts"], {"invalid:pairing": 2, "quote_not_found": 1})

    def test_each_attempt_gets_its_own_log_path_under_log_dir(self):
        with tempfile.TemporaryDirectory() as tmp:
            log_dir = pathlib.Path(tmp) / "logs"  # does not exist yet
            fake = _Fake(lambda n: _reply(["verify"], "quiet"))
            ij.judge(self.PKT, votes=1, complete=fake, log_dir=log_dir)
            self.assertTrue(log_dir.is_dir())
            self.assertEqual(fake.paths, [log_dir / "CASE-ID-XYZ.vote1.try1.log"])


class FixPassTests(unittest.TestCase):
    """Added by the final review: mutants that passed the first 32 tests."""

    PKT = _pkt(f"The agent wrote: {QUOTE}, so the edge case works.")

    def test_a_missing_delivery_is_flagged_and_never_defaulted(self):
        v = ij.parse_vote(json.dumps({"labels": ["verify"], "action": "x", "quote": QUOTE}), EVIDENCE)
        self.assertIsNone(v.delivery)
        self.assertIn("missing:delivery", v.flags)

    def test_a_missing_labels_value_stays_none(self):
        v = ij.parse_vote(json.dumps({"delivery": "quiet", "action": "", "quote": ""}), EVIDENCE)
        self.assertIsNone(v.labels)
        self.assertIn("missing:labels", v.flags)
        self.assertEqual(v.delivery, "quiet")

    def test_a_non_string_quote_is_flagged_and_not_kept(self):
        v = ij.parse_vote(_vote(labels=["none"], delivery="silent", action="", quote={"a": 1}), EVIDENCE)
        self.assertIn("invalid:quote", v.flags)
        self.assertEqual(v.quote, "")

    def test_the_label_and_delivery_tallies_are_counted_separately(self):
        rec = ij.judge(self.PKT, votes=1, complete=_Fake(lambda n: _reply(["verify"], "loud")))
        self.assertEqual(rec["label_votes"], dict(ALL_ZERO_LABELS, verify=1))
        self.assertEqual(rec["delivery_votes"], {"silent": 0, "quiet": 0, "interrupt": 0})
        self.assertEqual(rec["n_uncounted"], 1)
        rec = ij.judge(self.PKT, votes=1, complete=_Fake(lambda n: _reply(["bogus"], "quiet")))
        self.assertEqual(rec["label_votes"], ALL_ZERO_LABELS)
        self.assertEqual(rec["delivery_votes"]["quiet"], 1)

    def test_call_failed_flags_are_grouped_whatever_the_exception_text(self):
        def fn(n):
            raise (RuntimeError("one") if n % 2 else ValueError("two"))

        rec = ij.judge(self.PKT, votes=2, complete=_Fake(fn))
        self.assertEqual(rec["flag_counts"], {"call_failed": 2})

    def test_pre_evidence_keeps_a_line_that_starts_like_a_delimiter(self):
        text = "a\n===== heading\nb"
        self.assertEqual(ij.build_input(_pkt(text))["pre_evidence"], text)

    def test_the_prompt_stays_silent_on_an_accurate_scoped_snapshot(self):
        self.assertIn("historical", ij.build_input(_pkt("p"))["prompt"].lower())

    def test_the_prompt_maps_the_five_distinctions_to_the_labels(self):
        prompt = ij.build_input(_pkt("p"))["prompt"].lower()
        for phrase in ("needs verification", "needs qualification", "no intervention needed",
                       "cannot tell"):
            self.assertIn(phrase, prompt)

    def test_the_prompt_marks_its_additions_as_supplements_to_the_bars(self):
        self.assertIn("supplement", ij.build_input(_pkt("p"))["prompt"].lower())


if __name__ == "__main__":
    unittest.main()
