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
    def test_a_non_string_action_is_not_a_catalog_write_and_does_not_raise(self):
        # A list/dict `action` is unhashable: `inp.get("action") in CATALOG_WRITE_ACTIONS` (a set)
        # raised TypeError and _frame_units refused the WHOLE frame. Only a str can be a write.
        for action in (["update"], {"update": 1}):
            with self.subTest(action=action):
                kw = dict(tool_uses=[("mcp__codescout__doc", {"action": action})])
                self.assertEqual(sampler.classify(_msg("m", **kw)), ("routine", ()))
        # positive control: the str form of the same input still is a catalog write
        kw = dict(tool_uses=[("mcp__codescout__doc", {"action": "update"})])
        self.assertEqual(sampler.classify(_msg("m", **kw)), ("substantive", ("catalog_write",)))



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


class Constants(unittest.TestCase):
    def test_constants_equal_the_brief_verbatim(self):
        # Literals copied from the task brief. Deleting a member or editing a pattern reds here.
        self.assertEqual(sampler.CLAIM_PATTERNS, {
            "completion": r"\b(?:done|fixed|passes|passing|verified|confirmed|committed|green|completed?|resolved|implemented|landed|merged|pushed|shipped|works now|now works|all set)\b",
            "test_result": r"\b\d[\d,]*\s+(?:passed|failed|passing|failing|tests? pass(?:ed)?|tests? fail(?:ed)?|ignored|skipped)\b|\b0\s+(?:failed|failures|errors|warnings)\b|test result:\s*ok|\ball\s+(?:tests?\s+)?(?:pass|green)\b|\b\d+\s*/\s*\d+\s+(?:pass|tests?)\b|\bexit(?:ed)?(?: code)?\s*[=:]?\s*0\b",
            "count_noun": r"\b\d[\d,]*\s+(?:[A-Za-z][A-Za-z-]*\s+){0,2}?(?:files?|tests?|lines?|messages?|sessions?|entries|rows?|commits?|bugs?|findings?|hits?|matches|occurrences?|functions?|symbols?|tools?|items?|calls?|errors?|instances?|cases?|sites?|callers?|references?|turns?|bytes|chars|characters|tokens|docs?|artifacts?|trackers?|crates?|modules?|failures?|warnings?|assertions?|mutations?|packets?|samples?|strata|percent|%)\b",
            "absence": r"\b(?:no|none|never|nothing|zero|nowhere|nobody|neither)\b|\bnot found\b|\bno such\b|\bnot (?:present|exist|there|used|reached|called|set)\b|\b(?:doesn't|does not|don't|do not|didn't|did not|isn't|is not|aren't|are not|cannot|can't|won't|will not)\s+(?:exist|appear|contain|occur|match|find|show|carry|reach|fire|hold|have)\b|\bwithout any\b",
        })
        self.assertEqual(sampler.EDIT_TOOLS, {"edit_file", "edit_code", "create_file", "Write", "Edit",
                                              "MultiEdit", "NotebookEdit", "edit_markdown"})
        self.assertEqual(sampler.DISPATCH_TOOLS, {"Agent", "Task"})
        self.assertEqual(sampler.SHELL_TOOLS, {"run_command", "Bash"})
        self.assertEqual(sampler.CONSEQ_CMD,
                         r"\bgit\s+(commit|push|reset|rebase|checkout|stash|clean)\b|\brm\s+-|\bcargo\s+rb\b|\brb\.sh\b")
        self.assertEqual(sampler.CATALOG_TOOLS, {"doc", "memory"})
        self.assertEqual(sampler.CATALOG_WRITE_ACTIONS, {
            "create", "update", "move", "delete", "graft", "link", "append_entry", "update_entry",
            "rekey_prefix", "event_create", "augment", "write", "remember", "forget"})
        self.assertEqual(sampler.REASONS, ("claim_marker", "end_turn", "edit", "git_or_rm_or_release",
                                           "dispatch", "catalog_write", "handback"))

    def test_each_claim_pattern_has_a_text_only_it_matches(self):
        # Each text matches exactly ONE of the four patterns (checked here against all four), so
        # dropping that pattern from the classifier flips it to routine while the others stay
        # substantive. "12 tests passed" cannot do this: test_result and count_noun both match it.
        import re
        cases = {
            "completion": "Done.",  # only completion: no digits, none of the absence words
            "test_result": "exit code 0",  # only test_result: no noun follows the digit
            "count_noun": "3 files",  # only count_noun: "files" is not a test-result word
            "absence": "nothing changed",  # only absence: "changed" is not a completion word
        }
        for name, text in cases.items():
            with self.subTest(pattern=name):
                hit = {k for k, p in sampler.CLAIM_PATTERNS.items() if re.search(p, text, re.IGNORECASE)}
                self.assertEqual(hit, {name})
                self.assertEqual(sampler.classify(_msg("m", text=text)), ("substantive", ("claim_marker",)))

    def test_claim_matching_is_case_insensitive(self):
        # "DONE." matches the completion pattern only when re.IGNORECASE is applied.
        self.assertEqual(sampler.classify(_msg("m", text="DONE.")), ("substantive", ("claim_marker",)))


class FixtureShape(unittest.TestCase):
    def test_sibling_entries_of_one_message_get_distinct_tool_use_ids(self):
        # Task 2 matches tool_results by tool_use_id; a split message (one mid, one tool per entry)
        # needs a distinct id per entry. ids are f"{entry uuid}-tu{i}".
        e1 = fx.assistant("u1", _ts(1), "mX", tool_uses=[("Read", {})])
        e2 = fx.assistant("u2", _ts(2), "mX", tool_uses=[("Read", {})])
        ids = [b["id"] for e in (e1, e2) for b in e["message"]["content"] if b["type"] == "tool_use"]
        self.assertEqual(ids, ["u1-tu0", "u2-tu0"])

    def test_subagent_entries_are_written_sidechain_and_top_level_are_not(self):
        import json
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            # sidechain=False is set explicitly on the subagent entry: the writer must OVERWRITE it.
            sub = [fx.assistant("s1", _ts(1), "sm", text=NEUTRAL, sidechain=False)]
            fx.build_corpus(root, [_session("s1", "p", [fx.assistant("t1", _ts(1), "tm", text=NEUTRAL)],
                                            subagents={"w": sub})])
            top = json.loads(next(root.glob("transcripts/*/proj/s1.jsonl")).read_text().splitlines()[0])
            sub_e = json.loads(next(root.glob("transcripts/*/proj/s1/subagents/w.jsonl")).read_text().splitlines()[0])
            self.assertIs(sub_e["isSidechain"], True)
            self.assertIs(top["isSidechain"], False)



def _unit(i, stratum):
    # Every field a sort key might be mistaken for DISAGREES with case_key order: copy_id repeats in
    # groups of 5 (ties fall back to input order), message_id counts DOWN, first_entry_index and
    # decision_ts are scrambled. If they all rose together, sorting by any of them would pass.
    cid = f"p/s{i // 5:03d}"
    mid = f"m{99 - i:03d}"
    return sampler.Unit(case_key=f"{cid}|t.jsonl|{mid}", copy_id=cid, transcript="t.jsonl",
                        message_id=mid, kind="top", stratum=stratum,
                        reasons=("claim_marker",) if stratum == "substantive" else (),
                        first_entry_index=(i * 37) % 101, decision_ts=_ts((i * 7) % 60))


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

    def test_golden_draw_for_a_fixed_seed(self):
        # Hard-coded literal computed once from the shipped algorithm. The re-implementation test
        # above shares the algorithm's assumptions; this one cannot drift with them. Units are in
        # generation order, and every non-case_key field disagrees with case_key order (see _unit).
        got = [u.case_key for u in sampler.draw(self.units, self.sizes, seed=5)]
        self.assertEqual(got, [
            "p/s003|t.jsonl|m084", "p/s001|t.jsonl|m093", "p/s004|t.jsonl|m078",
            "p/s002|t.jsonl|m086", "p/s005|t.jsonl|m070",
            "p/s009|t.jsonl|m051", "p/s006|t.jsonl|m065", "p/s008|t.jsonl|m059",
            "p/s007|t.jsonl|m062",
        ])


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
