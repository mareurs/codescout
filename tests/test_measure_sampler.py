"""Task 1 of the labelled sample: frame, claim-and-action strata, seeded draw.

Run: ~/work/claude/prompt-engineering/.venv/bin/python -m pytest tests/test_measure_sampler.py -v

All transcripts here are SYNTHETIC (tests/measure_corpus_fixture.py); nothing reads a real corpus.
"""
import importlib.util
import pathlib
import random
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
join = sys.modules["join"] if "join" in sys.modules else _load("join")
import measure_corpus_fixture as fx  # noqa: E402

NEUTRAL = "Let me look at the module layout next."  # matches none of the four claim patterns


def _msg(mid, text=NEUTRAL, tool_uses=(), stop=None):
    return [fx.assistant(f"u-{mid}", "2026-09-20T10:00:00Z", mid, text=text, tool_uses=tool_uses, stop=stop)]


class Classify(unittest.TestCase):
    def test_each_reason_has_a_fixture_only_it_admits(self):
        # (reason, kwargs carrying the trigger, kwargs with the trigger removed, handback flag pair).
        # Each `with` case carries exactly one trigger on top of NEUTRAL text; the `without` twin
        # differs from it in that trigger alone. Breaking a reason's rule turns its own case red
        # (reasons == ()), and no other case admits this reason, so each guard is exercised alone.
        cases = [
            ("claim_marker",  # trigger: "12 tests passed" (test_result + count_noun); no stop, no tool
             dict(text="12 tests passed"), dict(text=NEUTRAL), False, False),
            ("end_turn",  # trigger: stop_reason end_turn on a neutral text message
             dict(stop="end_turn"), dict(stop="tool_use"), False, False),
            ("edit",  # trigger: mcp-prefixed edit_file tool_use (name matched after the last "__")
             dict(tool_uses=[("mcp__codescout__edit_file", {"path": "a.py"})]),
             dict(tool_uses=[("mcp__codescout__read_file", {"path": "a.py"})]), False, False),
            ("git_or_rm_or_release",  # trigger: Bash command "git commit"; twin is "git status"
             dict(tool_uses=[("Bash", {"command": "git commit -m x"})]),
             dict(tool_uses=[("Bash", {"command": "git status"})]), False, False),
            ("dispatch",  # trigger: an Agent tool_use
             dict(tool_uses=[("Agent", {"prompt": "x"})]),
             dict(tool_uses=[("mcp__codescout__symbols", {"name": "x"})]), False, False),
            ("catalog_write",  # trigger: doc action=update (a catalog WRITE); twin is action=find
             dict(tool_uses=[("mcp__codescout__doc", {"action": "update"})]),
             dict(tool_uses=[("mcp__codescout__doc", {"action": "find"})]), False, False),
            ("handback",  # trigger: the handback flag alone, on neutral text with no tool
             dict(), dict(), True, False),
        ]
        self.assertEqual([c[0] for c in cases], list(sampler.REASONS))
        for reason, with_kw, without_kw, hb_with, hb_without in cases:
            with self.subTest(reason=reason):
                stratum, reasons = sampler.classify(_msg("m", **with_kw), handback=hb_with)
                self.assertEqual((stratum, reasons), ("substantive", (reason,)))
                stratum, reasons = sampler.classify(_msg("m", **without_kw), handback=hb_without)
                self.assertEqual((stratum, reasons), ("routine", ()))

    def test_plain_tool_names_and_other_edit_tools_are_recognised(self):
        # "Write" (no mcp prefix) is in EDIT_TOOLS; a `memory` write is a catalog write; a `rm -`
        # shell command via run_command is consequential. Each is a separate rule branch.
        for kw, reason in [
            (dict(tool_uses=[("Write", {"file_path": "a"})]), "edit"),  # plain, unprefixed name
            (dict(tool_uses=[("mcp__codescout__memory", {"action": "remember"})]), "catalog_write"),
            (dict(tool_uses=[("mcp__codescout__run_command", {"command": "rm -rf x"})]),
             "git_or_rm_or_release"),
        ]:
            with self.subTest(reason=reason):
                self.assertEqual(sampler.classify(_msg("m", **kw)), ("substantive", (reason,)))

    def test_actions_and_commands_on_the_wrong_tool_are_not_triggers(self):
        # The widening direction: the same input on a tool outside the rule's tool set must stay
        # routine. Without these, dropping the tool-set half of a rule survives every positive case.
        for kw in [
            dict(tool_uses=[("mcp__codescout__workspace", {"action": "create"})]),  # catalog action, non-catalog tool
            dict(tool_uses=[("mcp__codescout__symbols", {"command": "git commit -m x"})]),  # conseq command, non-shell tool
        ]:
            self.assertEqual(sampler.classify(_msg("m", **kw)), ("routine", ()))


    def test_end_turn_needs_text_and_reads_any_entry_of_the_message(self):
        # end_turn with NO text is not a final message to the operator.
        self.assertEqual(sampler.classify(_msg("m", text=None, stop="end_turn")), ("routine", ()))
        # stop_reason may sit on a later entry than the text (a message spans several entries).
        entries = [
            fx.assistant("u1", "2026-09-20T10:00:00Z", "m", text=NEUTRAL, stop=None),
            fx.assistant("u2", "2026-09-20T10:00:01Z", "m", stop="end_turn"),
        ]
        self.assertEqual(sampler.classify(entries), ("substantive", ("end_turn",)))

    def test_short_claim_is_substantive_and_long_narration_alone_is_not(self):
        # Pins the spec's change from the length rule: a 400-char message with no marker and no
        # action is routine, while a 13-char one carrying a marker is substantive.
        self.assertEqual(sampler.classify(_msg("m", text="12 tests passed")),
                         ("substantive", ("claim_marker",)))
        long_text = ("Reading the surrounding module to see how the pieces fit together. " * 6)[:400]
        self.assertEqual(len(long_text), 400)
        self.assertEqual(sampler.classify(_msg("m", text=long_text)), ("routine", ()))

    def test_reasons_follow_the_REASONS_order(self):
        entries = _msg("m", text="all tests pass", stop="end_turn",
                       tool_uses=[("Agent", {"prompt": "x"}), ("Edit", {})])
        stratum, reasons = sampler.classify(entries, handback=True)
        self.assertEqual(stratum, "substantive")
        self.assertEqual(reasons, ("claim_marker", "end_turn", "edit", "dispatch", "handback"))


def _session(sid, profile, entries, subagents=None, slug="proj"):
    return {"sid": sid, "profile": profile, "slug": slug, "entries": entries,
            "subagents": subagents or {}}


def _ts(n):
    return f"2026-09-20T10:00:{n:02d}Z"


class Frame(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = pathlib.Path(self.tmp.name)

    def test_entries_sharing_a_message_id_form_one_unit(self):
        # Review Focus 1: a text entry and a tool_use entry with one message.id are ONE unit whose
        # reasons include the tool's reason; first_entry_index is the message's first entry.
        entries = [
            fx.user_prompt("a0", _ts(0), "go"),
            fx.assistant("a1", _ts(1), "mX", text=NEUTRAL),  # text half, same message.id as a2
            fx.assistant("a2", _ts(2), "mX", tool_uses=[("mcp__codescout__edit_file", {})]),  # tool half
            fx.assistant("a3", _ts(3), "mY", text=NEUTRAL),  # a different message: routine
        ]
        fx.build_corpus(self.root, [_session("s1", "p", entries)])
        units = sampler.frame(self.root, excluded_sids=set())
        by_id = {u.message_id: u for u in units}
        self.assertEqual(sorted(by_id), ["mX", "mY"])
        ux = by_id["mX"]
        self.assertEqual((ux.kind, ux.stratum, ux.reasons), ("top", "substantive", ("edit",)))
        self.assertEqual(ux.first_entry_index, 1)
        self.assertEqual(ux.decision_ts, _ts(1))
        self.assertEqual(ux.copy_id, "p/s1")
        self.assertEqual(ux.transcript, "transcripts/01-p/proj/s1.jsonl")
        self.assertEqual(ux.case_key, "p/s1|transcripts/01-p/proj/s1.jsonl|mX")
        self.assertEqual((by_id["mY"].stratum, by_id["mY"].first_entry_index), ("routine", 3))

    def test_synthetic_and_api_error_messages_are_not_units(self):
        # Review Focus 2. Each bad message carries a claim marker, so it would be substantive if
        # it leaked in; the only thing keeping it out is the exclusion.
        synthetic = fx.assistant("b1", _ts(1), "mSyn", text="12 tests passed", model="<synthetic>")
        api_error = fx.assistant("b2", _ts(2), "mErr", text="12 tests passed")
        api_error["isApiErrorMessage"] = True
        good = fx.assistant("b3", _ts(3), "mOk", text="12 tests passed")
        fx.build_corpus(self.root, [_session("s1", "p", [synthetic, api_error, good])])
        self.assertEqual([u.message_id for u in sampler.frame(self.root, excluded_sids=set())], ["mOk"])

    def test_sidechain_entries_are_not_top_level_units(self):
        side = fx.assistant("c1", _ts(1), "mSide", text="12 tests passed", sidechain=True)
        main = fx.assistant("c2", _ts(2), "mMain", text="12 tests passed")
        fx.build_corpus(self.root, [_session("s1", "p", [side, main])])
        self.assertEqual([u.message_id for u in sampler.frame(self.root, excluded_sids=set())], ["mMain"])

    def test_handback_is_last_assistant_message_with_text(self):
        # Review Focus 4. File "ends_tool_only": text message, then a tool-only message -> the text
        # one is the hand-back. File "no_text": only tool-only messages -> no unit.
        ends_tool_only = [
            fx.assistant("h1", _ts(1), "hEarly", text=NEUTRAL),  # earlier text message
            fx.assistant("h2", _ts(2), "hLater", text="report: found it"),  # THE hand-back
            fx.assistant("h3", _ts(3), "hTool", tool_uses=[("Read", {})]),  # tool-only tail, skipped
        ]
        no_text = [fx.assistant("n1", _ts(1), "nTool", tool_uses=[("Read", {})])]
        top = [fx.assistant("t1", _ts(1), "tTop", text=NEUTRAL)]
        fx.build_corpus(self.root, [_session("s1", "p", top,
                                             subagents={"ends_tool_only": ends_tool_only, "no_text": no_text})])
        units = sampler.frame(self.root, excluded_sids=set())
        hand = [u for u in units if u.kind == "handback"]
        self.assertEqual([u.message_id for u in hand], ["hLater"])
        self.assertEqual((hand[0].stratum, hand[0].reasons), ("substantive", ("handback",)))
        self.assertEqual(hand[0].transcript,
                         "transcripts/01-p/proj/s1/subagents/ends_tool_only.jsonl")
        self.assertEqual(hand[0].first_entry_index, 1)
        self.assertEqual([u.message_id for u in units if u.kind == "top"], ["tTop"])

    def test_handback_carries_its_other_reasons_too(self):
        sub = [fx.assistant("h1", _ts(1), "hA", text="12 tests passed", stop="end_turn")]
        fx.build_corpus(self.root, [_session("s1", "p", [], subagents={"w": sub})])
        (u,) = sampler.frame(self.root, excluded_sids=set())
        self.assertEqual(u.reasons, ("claim_marker", "end_turn", "handback"))

    def test_frame_holds_only_kept_copies(self):
        # Review Focus 5. "dup" exists in profiles a and b; a's timeline is a prefix of b's, so b
        # (longer) is the keeper. The spec-excluded sid yields nothing. Distinct uuid prefixes keep
        # the fork rule (shared first-5 uuids) from tying the three sessions together.
        spec_sid = sorted(join.SPEC_EXCLUDED_SIDS)[0]
        long_entries = [fx.assistant(f"d{i}", _ts(i), f"dm{i}", text="12 tests passed") for i in range(8)]
        prefix_entries = long_entries[:6]
        spec_entries = [fx.assistant(f"x{i}", _ts(i), f"xm{i}", text="12 tests passed") for i in range(3)]
        sub = [fx.assistant("k1", _ts(1), "kh", text=NEUTRAL)]
        fx.build_corpus(self.root, [
            _session("dup", "a", prefix_entries, subagents={"w": [fx.assistant("q1", _ts(1), "qh", text=NEUTRAL)]}),
            _session("dup", "b", long_entries, subagents={"w": sub}),
            _session(spec_sid, "a", spec_entries, slug="other",
                     subagents={"w": [fx.assistant("q2", _ts(1), "qh2", text=NEUTRAL)]}),
        ])
        units = sampler.frame(self.root, excluded_sids=join.SPEC_EXCLUDED_SIDS)
        self.assertEqual({u.copy_id for u in units}, {"b/dup"})
        self.assertEqual(sorted(u.message_id for u in units if u.kind == "top"),
                         sorted(f"dm{i}" for i in range(8)))
        self.assertEqual([u.message_id for u in units if u.kind == "handback"], ["kh"])
        # excluded_sids=None means the spec set, so the spec sid is still absent...
        self.assertEqual(sampler.frame(self.root), units)
        # ...while an explicit empty set admits it (proves the sid was excluded BY the set).
        self.assertIn(f"a/{spec_sid}", {u.copy_id for u in sampler.frame(self.root, excluded_sids=set())})


def _unit(i, stratum):
    cid = f"p/s{i:03d}"
    return sampler.Unit(case_key=f"{cid}|t.jsonl|m{i:03d}", copy_id=cid, transcript="t.jsonl",
                        message_id=f"m{i:03d}", kind="top", stratum=stratum,
                        reasons=("claim_marker",) if stratum == "substantive" else (),
                        first_entry_index=i, decision_ts=_ts(i % 60))


class Draw(unittest.TestCase):
    def setUp(self):
        self.units = [_unit(i, "substantive" if i < 30 else "routine") for i in range(50)]
        self.sizes = {"substantive": 5, "routine": 4}

    def test_draw_is_reproducible_and_disjoint_from_exclude(self):
        a = sampler.draw(self.units, self.sizes, seed=7)
        self.assertEqual(a, sampler.draw(self.units, self.sizes, seed=7))
        self.assertNotEqual([u.case_key for u in a],
                            [u.case_key for u in sampler.draw(self.units, self.sizes, seed=8)])
        self.assertEqual(sum(u.stratum == "substantive" for u in a), 5)
        self.assertEqual(sum(u.stratum == "routine" for u in a), 4)
        self.assertEqual(len({u.case_key for u in a}), 9)
        excl = {u.case_key for u in a}
        b = sampler.draw(self.units, self.sizes, seed=7, exclude=excl)
        self.assertEqual((len(b), {u.case_key for u in b} & excl), (9, set()))
        # a request of exactly the stratum's size is satisfiable (the bound is `>`, not `>=`)
        full = sampler.draw(self.units, {"substantive": 30, "routine": 20}, seed=3)
        self.assertEqual(len({u.case_key for u in full}), 50)

    def test_draw_order_is_input_order_independent_and_strata_ordered(self):
        # Pins: sort by case_key, one Random(seed), substantive before routine, exclude removed
        # BEFORE sampling. Units are passed reversed so an unsorted implementation diverges.
        exclude = {self.units[3].case_key, self.units[40].case_key}
        got = sampler.draw(list(reversed(self.units)), self.sizes, seed=11, exclude=exclude)
        rng = random.Random(11)
        subs = sorted((u for u in self.units if u.stratum == "substantive" and u.case_key not in exclude),
                      key=lambda u: u.case_key)
        rous = sorted((u for u in self.units if u.stratum == "routine" and u.case_key not in exclude),
                      key=lambda u: u.case_key)
        expected = rng.sample(subs, 5) + rng.sample(rous, 4)
        self.assertEqual(got, expected)

    def test_oversize_request_raises(self):
        with self.assertRaises(ValueError):
            sampler.draw(self.units, {"substantive": 31, "routine": 1}, seed=1)
        # the request fits the stratum only until exclude shrinks it
        with self.assertRaises(ValueError):
            sampler.draw(self.units, {"substantive": 30, "routine": 1}, seed=1,
                         exclude={self.units[0].case_key})
        with self.assertRaises(ValueError):
            sampler.draw(self.units, {"bogus": 1}, seed=1)

    def test_frame_counts_by_stratum_and_kind(self):
        import dataclasses
        units = [_unit(0, "substantive"), _unit(1, "substantive"), _unit(2, "routine"),
                 dataclasses.replace(_unit(3, "substantive"), kind="handback")]
        self.assertEqual(sampler.frame_counts(units),
                         {"substantive": {"top": 2, "handback": 1}, "routine": {"top": 1}})


if __name__ == "__main__":
    unittest.main()
