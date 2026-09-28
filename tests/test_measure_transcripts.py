"""Stage 1a: sessions, exclusions, fork detection over a frozen corpus.

Run: ~/work/claude/prompt-engineering/.venv/bin/python -m pytest tests/test_measure_transcripts.py -v
"""
import importlib.util
import json
import pathlib
import sys
import tempfile
import unittest

REPO_ROOT = pathlib.Path(__file__).resolve().parents[1]
MEASURE = REPO_ROOT / "scripts" / "measure"
sys.path.insert(0, str(MEASURE))


def _load(name):
    spec = importlib.util.spec_from_file_location(f"measure_{name}", MEASURE / f"{name}.py")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


transcripts = _load("transcripts")


def _write_lines(path, dicts):
    path.parent.mkdir(parents=True, exist_ok=True)
    with open(path, "w") as f:
        for d in dicts:
            f.write(json.dumps(d) + "\n")


def _entry(uuid, ts, sid, entrypoint="cli", type_="user", content="hello",
           is_meta=None, is_compact=None):
    e = {
        "type": type_,
        "uuid": uuid,
        "timestamp": ts,
        "sessionId": sid,
        "entrypoint": entrypoint,
        "cwd": "/x",
        "gitBranch": "main",
        "message": {"content": content},
    }
    if is_meta is not None:
        e["isMeta"] = is_meta
    if is_compact is not None:
        e["isCompactSummary"] = is_compact
    return e


class OperatorMessages(unittest.TestCase):
    def test_tool_results_meta_and_command_wrappers_are_not_operator_messages(self):
        # Review Focus 3: exactly one real prompt survives among a tool result, an isMeta
        # injection, a command-wrapped string and a compaction summary.
        tool_result = _entry(
            "u1", "2026-09-20T10:00:00Z", "s1",
            content=[{"tool_use_id": "t1", "type": "tool_result", "content": "ok"}],
        )
        is_meta = _entry(
            "u2", "2026-09-20T10:00:01Z", "s1",
            content=[{"type": "text", "text": "Base directory for this skill: ..."}],
            is_meta=True,
        )
        command_wrapped = _entry(
            "u3", "2026-09-20T10:00:02Z", "s1",
            content="<command-name>/model</command-name>\n<command-message>model</command-message>",
        )
        compaction_summary = _entry(
            "u4", "2026-09-20T10:00:03Z", "s1",
            content="This session is being continued from a previous conversation...",
            is_compact=True,
        )
        real_prompt = _entry("u5", "2026-09-20T10:00:04Z", "s1", content="please fix the bug")

        entries = [tool_result, is_meta, command_wrapped, compaction_summary, real_prompt]
        got = transcripts.operator_messages(entries)
        self.assertEqual(got, [real_prompt])

    def test_local_command_stdout_and_caveat_wrappers_are_excluded_too(self):
        stdout_wrapped = _entry(
            "u1", "2026-09-20T10:00:00Z", "s1",
            content="<local-command-stdout>some output</local-command-stdout>",
        )
        caveat_wrapped = _entry(
            "u2", "2026-09-20T10:00:01Z", "s1",
            content="<local-command-caveat>Caveat: ...</local-command-caveat>",
        )
        real_prompt = _entry("u3", "2026-09-20T10:00:02Z", "s1", content="do the thing")
        got = transcripts.operator_messages([stdout_wrapped, caveat_wrapped, real_prompt])
        self.assertEqual(got, [real_prompt])

    def test_non_user_entries_are_ignored(self):
        assistant = _entry("u1", "2026-09-20T10:00:00Z", "s1", type_="assistant", content="hi")
        real_prompt = _entry("u2", "2026-09-20T10:00:01Z", "s1", content="hello there")
        got = transcripts.operator_messages([assistant, real_prompt])
        self.assertEqual(got, [real_prompt])

    def test_an_interrupt_marker_is_not_an_operator_message(self):
        # R25: both known literals, in both observed content shapes (plain string, and a
        # list leading with a {"type": "text"} item) — none of these are operator prompts.
        marker_string_plain = _entry(
            "u1", "2026-09-20T10:00:00Z", "s1", content="[Request interrupted by user]",
        )
        marker_string_tool = _entry(
            "u2", "2026-09-20T10:00:01Z", "s1",
            content="[Request interrupted by user for tool use]",
        )
        marker_array_plain = _entry(
            "u3", "2026-09-20T10:00:02Z", "s1",
            content=[{"type": "text", "text": "[Request interrupted by user]"}],
        )
        marker_array_tool = _entry(
            "u4", "2026-09-20T10:00:03Z", "s1",
            content=[{"type": "text", "text": "[Request interrupted by user for tool use]"}],
        )
        # Whitespace around the marker (strip()-equality, not raw equality) is still a marker.
        marker_padded = _entry(
            "u5", "2026-09-20T10:00:04Z", "s1", content="  [Request interrupted by user]  \n",
        )
        real_prompt = _entry("u6", "2026-09-20T10:00:05Z", "s1", content="please fix the bug")

        entries = [
            marker_string_plain, marker_string_tool, marker_array_plain, marker_array_tool,
            marker_padded, real_prompt,
        ]
        got = transcripts.operator_messages(entries)
        self.assertEqual(got, [real_prompt])

    def test_a_prompt_quoting_the_marker_is_still_a_prompt(self):
        # R25: exact equality only, never substring — a prompt that merely quotes the
        # marker text stays a real prompt.
        quoting_prompt = _entry(
            "u1", "2026-09-20T10:00:00Z", "s1",
            content="why did you print [Request interrupted by user]?",
        )
        got = transcripts.operator_messages([quoting_prompt])
        self.assertEqual(got, [quoting_prompt])
        self.assertEqual(transcripts.operator_interrupts([quoting_prompt]), [])

    def test_task_notifications_are_excluded_three_shapes_plus_a_kept_prompt(self):
        # R26: 1,409 of 4,528 real operator_messages() entries are harness task-
        # notifications. Three shapes, each independently excluded, plus a real prompt
        # carrying neither field (and not matching the text-prefix fallback) stays kept.
        prompt_source_system = _entry(
            "u1", "2026-09-20T10:00:00Z", "s1", content="some task update",
        )
        prompt_source_system["promptSource"] = "system"

        origin_kind = _entry(
            "u2", "2026-09-20T10:00:01Z", "s1",
            content="2 background agents were stopped by the user: ...",
        )
        origin_kind["origin"] = {"kind": "task-notification"}

        fallback_prefix = _entry(
            "u3", "2026-09-20T10:00:02Z", "s1",
            content="<task-notification>older-shaped notification</task-notification>",
        )

        real_prompt = _entry("u4", "2026-09-20T10:00:03Z", "s1", content="please review this")

        entries = [prompt_source_system, origin_kind, fallback_prefix, real_prompt]
        got = transcripts.operator_messages(entries)
        self.assertEqual(got, [real_prompt])

    def test_the_real_command_wrapper_order_is_excluded(self):
        # R27: the real skill-command wrapper order is <command-message> FIRST, then
        # <command-name> — 66 real entries in this exact order were missed by a
        # <command-name>-only startswith check.
        wrapped = _entry(
            "u1", "2026-09-20T10:00:00Z", "s1",
            content="<command-message>model</command-message>\n<command-name>/model</command-name>",
        )
        got = transcripts.operator_messages([wrapped])
        self.assertEqual(got, [])

    def test_a_prompt_mentioning_command_name_mid_sentence_stays_a_prompt(self):
        # Guards against over-exclusion: only text that STARTS WITH a wrapper tag is
        # excluded — mentioning <command-name> mid-sentence does not.
        mentioning = _entry(
            "u1", "2026-09-20T10:00:00Z", "s1",
            content="can you explain what <command-name> means in the transcript format?",
        )
        got = transcripts.operator_messages([mentioning])
        self.assertEqual(got, [mentioning])

    def test_bare_slash_commands_are_excluded(self):
        # R29: a bare slash command with no arguments (202 "/compact" in the real corpus)
        # is harness scaffolding, not an operator prompt.
        compact = _entry("u1", "2026-09-20T10:00:00Z", "s1", content="/compact")
        clear = _entry("u2", "2026-09-20T10:00:01Z", "s1", content="/clear")
        got = transcripts.operator_messages([compact, clear])
        self.assertEqual(got, [])

    def test_a_slash_command_with_arguments_is_kept(self):
        # Guards against over-exclusion: BARE_SLASH_COMMAND_RE is anchored full-string, so
        # a real prompt that happens to start with a slash-command-shaped token followed by
        # more words is not a bare command.
        prompt = _entry(
            "u1", "2026-09-20T10:00:00Z", "s1", content="/review this function please",
        )
        got = transcripts.operator_messages([prompt])
        self.assertEqual(got, [prompt])


class OperatorInterrupts(unittest.TestCase):
    def test_operator_interrupts_returns_exactly_the_markers(self):
        real_prompt = _entry("u1", "2026-09-20T10:00:00Z", "s1", content="do the thing")
        marker_string = _entry(
            "u2", "2026-09-20T10:00:01Z", "s1", content="[Request interrupted by user]",
        )
        marker_array = _entry(
            "u3", "2026-09-20T10:00:02Z", "s1",
            content=[{"type": "text", "text": "[Request interrupted by user for tool use]"}],
        )
        quoting_prompt = _entry(
            "u4", "2026-09-20T10:00:03Z", "s1",
            content="the marker text is [Request interrupted by user], see?",
        )

        entries = [real_prompt, marker_string, marker_array, quoting_prompt]
        got = transcripts.operator_interrupts(entries)
        self.assertEqual(got, [marker_string, marker_array])
    def test_a_sidechain_marker_is_still_an_interrupt(self):
        # R132: isSidechain is deliberately NOT a filter here -- every subagent entry is a
        # sidechain, and the harness writes the operator's Esc into whichever chain was running.
        # LOAD-BEARING: exact marker text, no origin, not isMeta -- only an isSidechain filter
        # could refuse it.
        m = _entry("u1", "2026-09-20T10:00:00Z", "s1", content="[Request interrupted by user]")
        m["isSidechain"] = True
        self.assertEqual(transcripts.operator_interrupts([m]), [m])

    def test_only_a_human_or_an_absent_origin_marker_is_an_interrupt(self):
        # R132 (R104's positive identification): an origin dict naming anyone but the human is
        # no interrupt -- a peer, or a kind the corpus lacks; an ABSENT origin is allowed (no
        # measured marker carries one). LOAD-BEARING: the fixtures differ ONLY in origin.
        cases = (("peer", False), ("system", False), ("human", True), (None, True))
        for kind, is_interrupt in cases:
            with self.subTest(origin_kind=kind):
                e = _entry("u1", "2026-09-20T10:00:00Z", "s1",
                           content="[Request interrupted by user]")
                if kind is not None:
                    e["origin"] = {"kind": kind}
                self.assertEqual(transcripts.operator_interrupts([e]), [e] if is_interrupt else [])


class ReadJsonl(unittest.TestCase):
    def test_a_truncated_last_line_is_skipped_and_counted(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = pathlib.Path(tmp) / "session.jsonl"
            with open(path, "w") as f:
                f.write(json.dumps({"type": "user", "uuid": "a"}) + "\n")
                f.write(json.dumps({"type": "assistant", "uuid": "b"}) + "\n")
                # Truncated: no closing brace, no trailing newline — a process killed mid-write.
                f.write('{"type": "user", "uuid": "c"')
            entries, skipped = transcripts.read_jsonl(path)
            self.assertEqual(len(entries), 2)
            self.assertEqual(skipped, 1)

    def test_a_well_formed_file_has_zero_skipped(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = pathlib.Path(tmp) / "session.jsonl"
            with open(path, "w") as f:
                f.write(json.dumps({"type": "user", "uuid": "a"}) + "\n")
                f.write(json.dumps({"type": "assistant", "uuid": "b"}) + "\n")
            entries, skipped = transcripts.read_jsonl(path)
            self.assertEqual(len(entries), 2)
            self.assertEqual(skipped, 0)


def _make_corpus(tmp_path):
    corpus_dir = tmp_path / "corpus"
    (corpus_dir / "manifest.json").parent.mkdir(parents=True, exist_ok=True)
    manifest = {
        "corpus_id": "c-test", "created_utc": "2026-09-26T00:00:00Z",
        "bounds": {}, "files": {}, "counts": {}, "versions": {}, "repos": {}, "exclusions": [],
    }
    (corpus_dir / "manifest.json").write_text(json.dumps(manifest, indent=2, sort_keys=True))
    return corpus_dir


def _session_dir(corpus_dir, profile_dir, project_slug):
    return corpus_dir / "transcripts" / profile_dir / project_slug


class ForkDetection(unittest.TestCase):
    def _assert_fork_orientation(self, orig_sid, fork_sid, divergence_idx):
        # R30: shared helper for TWIN fixtures — (A) the true original's sid sorts
        # lexically EARLIER than the fork's, (B) it sorts LATER — so a mutation that
        # substitutes sid order, glob/insertion order, or first_ts for the real
        # timestamp-based orientation fails on at least one twin instead of coincidentally
        # matching both. See the two callers below for exactly what each twin catches.
        #
        # Layout (both branches, both twins — divergence_idx DIFFERS between the twins, 6
        # and 8, so a hardcoded or skipped-scan divergence point cannot coincidentally land
        # on the true divergence entry for both):
        #   indices 0-4: identical uuid AND timestamp on both branches (the shared prefix,
        #     length == FORK_PREFIX_LEN) — what makes the two sessions group as a fork pair.
        #   indices 5..divergence_idx-1 (filler): the SAME uuid per index on both branches,
        #     but DELIBERATELY REVERSED timestamps — orig's is LATER than fork's at every
        #     filler index. A mutation that hardcodes or misidentifies the divergence index
        #     so it lands in this range reads a reversed-order pair and gets the orientation
        #     backwards.
        #   index == divergence_idx: the TRUE divergence — uuids differ between branches,
        #     with CORRECT timestamps (orig earlier) that correctly identify the "-orig"
        #     branch as the real original.
        #   one tail entry immediately after divergence_idx: uuids differ between branches
        #     again, but timestamps here are REVERSED relative to the true verdict (orig
        #     LATER). Skipping the forward scan entirely (divergence_idx falls back to
        #     common_len, so `_ts_at` reads tl[-1]) reads THIS entry instead of the true
        #     divergence one and gets the orientation backwards — this is what makes the
        #     scan itself, not just its target index, load-bearing.
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            proj = _session_dir(corpus_dir, "00-.claude-kat", "p")

            def branch(sid, is_orig):
                lines = [_entry(f"u{i}", f"2026-09-20T10:00:0{i}Z", sid) for i in range(5)]
                for i in range(transcripts.FORK_PREFIX_LEN, divergence_idx):
                    ts = (
                        f"2026-09-20T11:00:{i:02d}Z"
                        if is_orig
                        else f"2026-09-20T10:30:{i:02d}Z"
                    )
                    lines.append(_entry(f"c{i}", ts, sid))
                div_ts = "2026-09-20T09:00:00Z" if is_orig else "2026-09-20T09:00:05Z"
                lines.append(_entry(f"d-{sid}", div_ts, sid))
                tail_ts = "2026-09-21T09:00:00Z" if is_orig else "2026-09-21T08:00:00Z"
                lines.append(_entry(f"tail-{sid}", tail_ts, sid))
                return lines

            orig_lines = branch(orig_sid, True)
            fork_lines = branch(fork_sid, False)

            _write_lines(proj / f"{orig_sid}.jsonl", orig_lines)
            _write_lines(proj / f"{fork_sid}.jsonl", fork_lines)

            sess = transcripts.sessions(corpus_dir)
            self.assertEqual({s.sid for s in sess}, {orig_sid, fork_sid})

            excl = transcripts.exclusions(sess, set())
            rels = transcripts.relations(sess, excl)
            kept_key = f".claude-kat/{orig_sid}"
            fork_key = f".claude-kat/{fork_sid}"

            # R28(i): forks are never an exclusion reason.
            self.assertEqual(excl, {})
            # R28(ii)/R30: the real original — the branch whose true divergence-point entry
            # has the earlier timestamp — is oriented correctly regardless of which sid sorts
            # lexically earlier or which file glob-sorts first.
            self.assertEqual(rels.get(fork_key), "fork-of:" + kept_key)
            self.assertNotIn(kept_key, rels)

    def test_fork_orientation_twin_a_original_sorts_lexically_earlier(self):
        # a-orig < z-fork, and a-orig.jsonl also glob-sorts first. min(sid), first_ts
        # (tied, so it falls to sid), and distinct[0] (glob/insertion order) all
        # coincidentally agree with the correct answer here — this twin alone would not
        # catch them. max(sid) IS caught here: it would wrongly pick z-fork as the
        # original. divergence_idx=6 is this twin's TRUE divergence point, so a hardcoded
        # divergence_idx=FORK_PREFIX_LEN(=5) reads the reversed-timestamp filler entry at
        # index 5 and is caught here too.
        self._assert_fork_orientation("a-orig", "z-fork", divergence_idx=6)

    def test_fork_orientation_twin_b_original_sorts_lexically_later(self):
        # z-orig is the true original but sorts LEXICALLY LATER than a-fork, and
        # a-fork.jsonl also glob-sorts first — so min(sid), first_ts-tied-to-sid, and
        # distinct[0] (glob/insertion order) all wrongly pick a-fork here. max(sid)
        # coincidentally agrees with the correct answer on this twin. This twin's TRUE
        # divergence point is 8, two past twin A's (6) — so a hardcoded divergence_idx=6
        # lands in THIS twin's reversed-timestamp filler range (indices 5-7) rather than
        # coincidentally hitting the true divergence entry, and a deleted forward scan
        # (divergence_idx falls back to common_len, reading tl[-1]) reads the reversed-
        # timestamp tail entry on both twins regardless of where the true divergence sits.
        # Combined with twin A, every named mutation (min(sid), max(sid), distinct[0],
        # first_ts, divergence_idx=FORK_PREFIX_LEN, divergence_idx hardcoded to a fixed
        # value, and the forward scan deleted outright) fails at least one twin, while the
        # real forward-scan-then-timestamp logic passes both.
        self._assert_fork_orientation("z-orig", "a-fork", divergence_idx=8)

    def test_no_false_fork_when_first_uuids_differ(self):
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            proj = _session_dir(corpus_dir, "00-.claude-kat", "p")
            a = [_entry(f"a{i}", f"2026-09-20T10:00:0{i}Z", "sid-a") for i in range(5)]
            b = [_entry(f"b{i}", f"2026-09-20T10:00:0{i}Z", "sid-b") for i in range(5)]
            _write_lines(proj / "sid-a.jsonl", a)
            _write_lines(proj / "sid-b.jsonl", b)
            sess = transcripts.sessions(corpus_dir)
            excl = transcripts.exclusions(sess, set())
            self.assertEqual(excl, {})
            # R28: no false relation either — first 5 uuids differ, so this isn't a fork pair.
            self.assertEqual(transcripts.relations(sess, excl), {})


class AttributeEntries(unittest.TestCase):
    def test_a_fork_pairs_shared_prefix_goes_to_the_longer_transcript(self):
        # R28(iii): a fork pair diverging right at FORK_PREFIX_LEN — the shared prefix is
        # attributed to whichever copy has MORE total entry uuids (R31: every entry, not just
        # operator messages), and each
        # branch's unique tail stays with its own copy. None lost, none double-counted.
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            proj = _session_dir(corpus_dir, "00-.claude-kat", "p")

            shared = [
                _entry(f"u{i}", f"2026-09-20T10:00:0{i}Z", "orig-a", content=f"shared prompt {i}")
                for i in range(5)
            ]
            a_only = [
                _entry("u5-a", "2026-09-20T10:00:05Z", "orig-a", content="a tail 1"),
                _entry("u6-a", "2026-09-20T10:00:06Z", "orig-a", content="a tail 2"),
            ]
            b_shared = [
                _entry(f"u{i}", f"2026-09-20T10:00:0{i}Z", "fork-b", content=f"shared prompt {i}")
                for i in range(5)
            ]
            b_only = [
                _entry("u5-b", "2026-09-21T09:00:00Z", "fork-b", content="b tail 1"),
                _entry("u6-b", "2026-09-21T09:00:01Z", "fork-b", content="b tail 2"),
                _entry("u7-b", "2026-09-21T09:00:02Z", "fork-b", content="b tail 3"),
            ]
            _write_lines(proj / "orig-a.jsonl", shared + a_only)
            _write_lines(proj / "fork-b.jsonl", b_shared + b_only)

            sess = transcripts.sessions(corpus_dir)
            attrib = transcripts.attribute_entries(sess, {})

            cid_a = ".claude-kat/orig-a"
            cid_b = ".claude-kat/fork-b"
            # orig-a has 7 total uuids, fork-b has 8 — fork-b is longer, so it owns the
            # shared prefix.
            for i in range(5):
                self.assertEqual(attrib[f"u{i}"], cid_b)
            self.assertEqual(attrib["u5-a"], cid_a)
            self.assertEqual(attrib["u6-a"], cid_a)
            self.assertEqual(attrib["u5-b"], cid_b)
            self.assertEqual(attrib["u6-b"], cid_b)
            self.assertEqual(attrib["u7-b"], cid_b)
            self.assertEqual(len(attrib), 10)

    def test_a_near_duplicate_pair_shaped_like_the_real_corpus_counts_shared_prompts_once(self):
        # Shaped like the real .claude/dcf4beb1 vs .claude/66523284 pair: a deep shared run
        # (beyond FORK_PREFIX_LEN) with a small unique tail on EACH side. Every operator
        # prompt is attributed exactly once; none is lost.
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            proj = _session_dir(corpus_dir, "00-.claude", "p")

            shared = [
                _entry(f"s{i}", f"2026-09-20T10:00:{i:02d}Z", "small-orig", content=f"shared {i}")
                for i in range(14)
            ]
            small_only = [
                _entry(f"small-tail-{i}", f"2026-09-20T10:00:{14 + i:02d}Z", "small-orig",
                       content=f"small tail {i}")
                for i in range(3)
            ]
            big_shared = [
                _entry(f"s{i}", f"2026-09-20T10:00:{i:02d}Z", "big-fork", content=f"shared {i}")
                for i in range(14)
            ]
            big_only = [
                _entry(f"big-tail-{i}", f"2026-09-21T09:00:{i:02d}Z", "big-fork",
                       content=f"big tail {i}")
                for i in range(5)
            ]
            _write_lines(proj / "small-orig.jsonl", shared + small_only)
            _write_lines(proj / "big-fork.jsonl", big_shared + big_only)

            sess = transcripts.sessions(corpus_dir)
            attrib = transcripts.attribute_entries(sess, {})

            cid_small = ".claude/small-orig"
            cid_big = ".claude/big-fork"
            # big-fork (14 + 5 = 19 uuids) outsizes small-orig (14 + 3 = 17), so it owns the
            # 14-entry shared run; each side's own tail stays with it.
            for i in range(14):
                self.assertEqual(attrib[f"s{i}"], cid_big)
            for i in range(3):
                self.assertEqual(attrib[f"small-tail-{i}"], cid_small)
            for i in range(5):
                self.assertEqual(attrib[f"big-tail-{i}"], cid_big)
            self.assertEqual(len(attrib), 14 + 3 + 5)

    def test_a_pure_copy_under_a_new_sid_keeps_zero_entries(self):
        # A wholly different sessionId (not an R22 same-sid case) whose transcript is a
        # byte-for-byte copy of another's — same uuids, same timestamps, zero unique content
        # of its own. Tied uuid COUNT and tied first_ts against the original, so the
        # tie-break falls to copy_id: "a-original" sorts before "z-purecopy", so the copy
        # claims none of the shared uuids.
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            proj = _session_dir(corpus_dir, "00-.claude-kat", "p")

            original_lines = [
                _entry(f"p{i}", f"2026-09-20T10:00:0{i}Z", "a-original", content=f"prompt {i}")
                for i in range(5)
            ]
            copy_lines = [
                _entry(f"p{i}", f"2026-09-20T10:00:0{i}Z", "z-purecopy", content=f"prompt {i}")
                for i in range(5)
            ]
            _write_lines(proj / "a-original.jsonl", original_lines)
            _write_lines(proj / "z-purecopy.jsonl", copy_lines)

            sess = transcripts.sessions(corpus_dir)
            attrib = transcripts.attribute_entries(sess, {})

            cid_original = ".claude-kat/a-original"
            cid_copy = ".claude-kat/z-purecopy"
            for i in range(5):
                self.assertEqual(attrib[f"p{i}"], cid_original)
            copy_owned = [u for u, owner in attrib.items() if owner == cid_copy]
            self.assertEqual(copy_owned, [])
            self.assertEqual(len(attrib), 5)

    def test_attribution_covers_every_entry_type_not_just_operator_messages(self):
        # R31/R37: attribute_entries() must map EVERY entry uuid -- not only operator-message
        # (type=="user", real-prompt) uuids -- because the downstream audit samples ASSISTANT
        # messages. b-orig and a-copy share one real prompt ("sp0", tied first_ts) and each has
        # only that one operator message, so an operator-count-only comparison ties 1-vs-1 and
        # falls to the copy_id tie-break ("a-copy" < "b-orig" lexically) -- wrongly handing the
        # shared prompt to a-copy. Counting every entry breaks the tie correctly: b-orig has 7
        # total uuids, a-copy has 5, so b-orig wins.
        #
        # R37: probes/task5-fix3-types.txt enumerates the real corpus and finds exactly 4
        # uuid-bearing top-level `type` values (assistant, attachment, system, user) and, within
        # type=="user", exactly 4 sub-kinds by (isMeta, isCompactSummary, tool_result_shaped) --
        # 7 distinct kinds total. Every kind gets one uuid here, unique to a single copy, so a
        # scope restricted to operator-messages-plus-assistant (or missing any other single
        # kind) drops that uuid from the map entirely -- a bare KeyError on lookup below, not a
        # mere mis-attribution.
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            proj = _session_dir(corpus_dir, "00-.claude-kat", "p")

            ts0 = "2026-09-20T10:00:00Z"
            b_lines = [
                _entry("sp0", ts0, "b-orig", content="shared prompt"),
                _entry("a1", "2026-09-20T10:00:01Z", "b-orig", type_="assistant"),
                _entry("a2", "2026-09-20T10:00:02Z", "b-orig", type_="assistant"),
                _entry("t1", "2026-09-20T10:00:03Z", "b-orig", type_="assistant"),
                _entry("attach-b", "2026-09-20T10:00:04Z", "b-orig", type_="attachment"),
                _entry("sys-b", "2026-09-20T10:00:05Z", "b-orig", type_="system"),
                _entry("meta-b", "2026-09-20T10:00:06Z", "b-orig", type_="user", is_meta=True),
            ]
            a_lines = [
                _entry("sp0", ts0, "a-copy", content="shared prompt"),
                _entry("a3", "2026-09-20T10:00:01Z", "a-copy", type_="assistant"),
                _entry("t2", "2026-09-20T10:00:02Z", "a-copy", type_="assistant"),
                _entry(
                    "tr-a",
                    "2026-09-20T10:00:03Z",
                    "a-copy",
                    type_="user",
                    content=[{"type": "tool_result", "content": "result text"}],
                ),
                _entry(
                    "compact-a", "2026-09-20T10:00:04Z", "a-copy", type_="user", is_compact=True
                ),
            ]
            _write_lines(proj / "b-orig.jsonl", b_lines)
            _write_lines(proj / "a-copy.jsonl", a_lines)

            sess = transcripts.sessions(corpus_dir)
            attrib = transcripts.attribute_entries(sess, {})

            cid_b = ".claude-kat/b-orig"
            cid_a = ".claude-kat/a-copy"
            # Every non-prompt uuid must be in the map at all, whatever its entry `type` --
            # assistant, attachment, system, and each type=="user" sub-kind (isMeta,
            # isCompactSummary, tool_result-shaped).
            for u in ("a1", "a2", "t1", "attach-b", "sys-b", "meta-b"):
                self.assertEqual(attrib[u], cid_b)
            for u in ("a3", "t2", "tr-a", "compact-a"):
                self.assertEqual(attrib[u], cid_a)
            # b-orig has 7 total uuids, a-copy has 5 -- b-orig wins the shared prompt.
            self.assertEqual(attrib["sp0"], cid_b)
            self.assertEqual(len(attrib), 11)

    def test_attribution_most_count_wins_when_longer_copy_sorts_lexically_earlier(self):
        # Twin A (non-discriminating alone -- see the twin below): the copy with MORE total
        # uuids also sorts lexically earlier as a copy_id, so a mutation that dropped the
        # count and sorted by copy_id (or first_ts) alone would pass this one too.
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            proj = _session_dir(corpus_dir, "00-.claude-kat", "p")

            ts0 = "2026-09-20T10:00:00Z"
            long_lines = [
                _entry("sh0", ts0, "a-long", content="shared"),
                _entry("u1", "2026-09-20T10:00:01Z", "a-long", type_="assistant"),
                _entry("u2", "2026-09-20T10:00:02Z", "a-long", type_="assistant"),
                _entry("u3", "2026-09-20T10:00:03Z", "a-long", type_="assistant"),
            ]
            short_lines = [
                _entry("sh0", ts0, "z-short", content="shared"),
                _entry("w1", "2026-09-20T10:00:01Z", "z-short", type_="assistant"),
            ]
            _write_lines(proj / "a-long.jsonl", long_lines)
            _write_lines(proj / "z-short.jsonl", short_lines)

            sess = transcripts.sessions(corpus_dir)
            attrib = transcripts.attribute_entries(sess, {})

            self.assertEqual(attrib["sh0"], ".claude-kat/a-long")

    def test_attribution_most_count_wins_when_longer_copy_sorts_lexically_later(self):
        # Twin B -- the discriminating half: the copy with MORE total uuids sorts lexically
        # LATER as a copy_id, and tied first_ts, so a mutation that sorts by copy_id or
        # first_ts instead of (-count, ...) picks the wrong (shorter) copy here even though
        # it passed the twin above.
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            proj = _session_dir(corpus_dir, "00-.claude-kat", "p")

            ts0 = "2026-09-20T10:00:00Z"
            short_lines = [
                _entry("sh0", ts0, "a-short", content="shared"),
                _entry("w1", "2026-09-20T10:00:01Z", "a-short", type_="assistant"),
            ]
            long_lines = [
                _entry("sh0", ts0, "z-long", content="shared"),
                _entry("u1", "2026-09-20T10:00:01Z", "z-long", type_="assistant"),
                _entry("u2", "2026-09-20T10:00:02Z", "z-long", type_="assistant"),
                _entry("u3", "2026-09-20T10:00:03Z", "z-long", type_="assistant"),
            ]
            _write_lines(proj / "a-short.jsonl", short_lines)
            _write_lines(proj / "z-long.jsonl", long_lines)

            sess = transcripts.sessions(corpus_dir)
            attrib = transcripts.attribute_entries(sess, {})

            self.assertEqual(attrib["sh0"], ".claude-kat/z-long")

    def test_an_excluded_copy_never_wins_an_attribution_whatever_the_exclusion_reason(self):
        # R31 x R28 x R32: an excluded transcript must never own a uuid, whatever the reason it
        # was excluded and however it compares to the keeper. The R22 prefix-copy fixture this
        # test used to carry gave the excluded copy a strict SUBSET of the keeper's uuids, so it
        # never outranked the keeper whether or not the exclusion filter existed at all (R31's
        # excluded-copy half was vacuous for R22 copies). One case per exclusion kind below is
        # built so the excluded copy WOULD win (or at least own something) without the filter.
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            sdd_proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")
            kat_proj = _session_dir(corpus_dir, "01-.claude-kat", "p")

            # --- Case 1: R22 prefix copy (duplicate-prefix-of:), tied on distinct-uuid count ---
            # P (.claude-kat) has 5 lines / 5 distinct uuids. K (.claude-sdd) repeats one of
            # those same uuids on a 6th line, so it has 6 lines but the SAME 5 distinct uuids --
            # a genuine tie in attribute_entries()'s own count-based ranking. _sid_keepers()
            # picks K anyway (RAW line count, 6 > 5, is what R22/R32 rank on, not distinct
            # count), so K is the keeper and P is excluded as duplicate-prefix-of:K. Without the
            # exclusion filter, P's copy_id (".claude-kat/...") sorts lexically before K's
            # (".claude-sdd/..."), and a tied first_ts (both start with the same p0 entry) falls
            # through to that copy_id tie-break -- so P would WIN every shared uuid.
            p_lines = [_entry(f"p{i}", f"2026-09-20T10:00:0{i}Z", "dup-p1") for i in range(5)]
            k_lines = p_lines + [_entry("p0", "2026-09-20T10:00:05Z", "dup-p1")]
            _write_lines(kat_proj / "dup-p1.jsonl", p_lines)
            _write_lines(sdd_proj / "dup-p1.jsonl", k_lines)

            # --- Case 2: R22 divergent duplicate (divergent-duplicate-of:), unique tail uuids ---
            # K2 (.claude-sdd) has 6 lines/uuids; D (.claude-kat) shares K2's first 3 entries
            # then diverges -- its last 2 uuids (d3, d4) exist NOWHERE else in the corpus. K2 is
            # longer (6 > 5 lines) so it is the keeper and D is excluded as
            # divergent-duplicate-of:K2. d3/d4 being ABSENT from the attribution map once D is
            # excluded is R22's documented cost (an excluded copy's exclusive entries are never
            # audited), not a bug -- annotated here rather than asserted as a coincidence.
            k2_lines = [_entry(f"k{i}", f"2026-09-20T11:00:0{i}Z", "dup-y") for i in range(6)]
            d_lines = k2_lines[:3] + [
                _entry("d3", "2026-09-20T11:00:03Z", "dup-y"),
                _entry("d4", "2026-09-20T11:00:04Z", "dup-y"),
            ]
            _write_lines(sdd_proj / "dup-y.jsonl", k2_lines)
            _write_lines(kat_proj / "dup-y.jsonl", d_lines)

            # --- Case 3: sdk-cli (Stage B), kept from the original test -- already kills ---
            s_lines = [
                _entry(
                    f"s{i}", f"2026-09-21T09:00:{i:02d}Z", "sdk-session", entrypoint="sdk-cli"
                )
                for i in range(10)
            ]
            _write_lines(kat_proj / "sdk-session.jsonl", s_lines)

            sess = transcripts.sessions(corpus_dir)
            excl = transcripts.exclusions(sess, set())
            attrib = transcripts.attribute_entries(sess, excl)

            cid_k = ".claude-sdd/dup-p1"
            cid_p = ".claude-kat/dup-p1"
            cid_k2 = ".claude-sdd/dup-y"
            cid_d = ".claude-kat/dup-y"

            self.assertEqual(excl.get(cid_p), f"duplicate-prefix-of:{cid_k}")
            self.assertEqual(excl.get(cid_d), f"divergent-duplicate-of:{cid_k2}")
            self.assertNotIn(cid_k, excl)
            self.assertNotIn(cid_k2, excl)

            # Case 1: P owns NOTHING -- every shared uuid goes to K, the actual keeper.
            for u in ("p0", "p1", "p2", "p3", "p4"):
                self.assertEqual(attrib[u], cid_k)

            # Case 2: D's unique tail uuids are simply absent; the shared prefix goes to K2.
            for u in ("d3", "d4"):
                self.assertNotIn(u, attrib)
            for u in ("k0", "k1", "k2", "k3", "k4", "k5"):
                self.assertEqual(attrib[u], cid_k2)

            # Case 3: sdk-cli, excluded, owns nothing however many entries it has.
            for i in range(10):
                self.assertNotIn(f"s{i}", attrib)

            # P's 0 + D's 0 + S's 0 + K's 5 + K2's 6 == 11 total owned uuids.
            self.assertEqual(len(attrib), 11)


class SpecExclusions(unittest.TestCase):
    def test_sdk_cli_and_scratchpad_sessions_are_excluded_with_reasons(self):
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))

            interactive_proj = _session_dir(corpus_dir, "00-.claude-kat", "p")
            _write_lines(
                interactive_proj / "interactive-sid.jsonl",
                [_entry("i0", "2026-09-20T10:00:00Z", "interactive-sid", entrypoint="cli")],
            )

            headless_proj = _session_dir(corpus_dir, "00-.claude-kat", "p")
            _write_lines(
                headless_proj / "headless-sid.jsonl",
                [_entry("h0", "2026-09-20T10:00:00Z", "headless-sid", entrypoint="sdk-cli")],
            )

            scratch_proj = _session_dir(
                corpus_dir, "01-.claude-kat",
                "-tmp-claude-1000--home-marius-work-claude-codescout-abc-scratchpad-toy2",
            )
            _write_lines(
                scratch_proj / "scratch-sid.jsonl",
                [_entry("sc0", "2026-09-20T10:00:00Z", "scratch-sid", entrypoint="cli")],
            )

            sess = transcripts.sessions(corpus_dir)
            excl = transcripts.exclusions(sess, {"3c5b02df-b6ce-45f5-9d03-1194e38465c0"})

            self.assertEqual(excl.get(".claude-kat/headless-sid"), "sdk-cli")
            self.assertEqual(excl.get(".claude-kat/scratch-sid"), "scratchpad-project")
            self.assertNotIn(".claude-kat/interactive-sid", excl)

    def test_excluded_by_spec_sid_is_marked_regardless_of_entrypoint(self):
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            proj = _session_dir(corpus_dir, "00-.claude-kat", "p")
            _write_lines(
                proj / "3c5b02df-b6ce-45f5-9d03-1194e38465c0.jsonl",
                [_entry("z0", "2026-09-20T10:00:00Z", "3c5b02df-b6ce-45f5-9d03-1194e38465c0")],
            )
            sess = transcripts.sessions(corpus_dir)
            excl = transcripts.exclusions(sess, {"3c5b02df-b6ce-45f5-9d03-1194e38465c0"})
            self.assertEqual(
                excl.get(".claude-kat/3c5b02df-b6ce-45f5-9d03-1194e38465c0"), "excluded-by-spec"
            )


class SameSessionAcrossProfiles(unittest.TestCase):
    def test_a_prefix_copy_is_excluded_and_the_superset_copy_is_kept(self):
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            sdd_proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")
            kat_proj = _session_dir(corpus_dir, "01-.claude-kat", "p")

            shared = [_entry(f"u{i}", f"2026-09-20T10:00:0{i}Z", "dup-sid") for i in range(5)]
            kat_lines = shared + [_entry("u5", "2026-09-20T10:00:05Z", "dup-sid")]

            _write_lines(sdd_proj / "dup-sid.jsonl", shared)
            _write_lines(kat_proj / "dup-sid.jsonl", kat_lines)

            sess = transcripts.sessions(corpus_dir)
            excl = transcripts.exclusions(sess, set())

            self.assertEqual(
                excl.get(".claude-sdd/dup-sid"), "duplicate-prefix-of:.claude-kat/dup-sid"
            )
            self.assertNotIn(".claude-kat/dup-sid", excl)

    def test_a_divergent_duplicate_keeps_the_longer_copy(self):
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            sdd_proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")
            kat_proj = _session_dir(corpus_dir, "01-.claude-kat", "p")

            base = [_entry(f"u{i}", f"2026-09-20T10:00:0{i}Z", "div-sid") for i in range(5)]
            sdd_lines = base + [_entry("sdd-only", "2026-09-20T10:00:05Z", "div-sid")]
            kat_lines = base + [_entry("kat-only-1", "2026-09-20T10:00:05Z", "div-sid"),
                                 _entry("kat-only-2", "2026-09-20T10:00:06Z", "div-sid")]

            _write_lines(sdd_proj / "div-sid.jsonl", sdd_lines)
            _write_lines(kat_proj / "div-sid.jsonl", kat_lines)

            sess = transcripts.sessions(corpus_dir)
            excl = transcripts.exclusions(sess, set())

            self.assertEqual(
                excl.get(".claude-sdd/div-sid"), "divergent-duplicate-of:.claude-kat/div-sid"
            )
            self.assertNotIn(".claude-kat/div-sid", excl)

    def test_the_longer_copy_can_be_in_the_lexically_earlier_profile(self):
        # R32 report correction: the two cases above always process the longer copy SECOND
        # -- .claude-kat sits under "01-", .claude-sdd under "00-" -- so a mutation that
        # keeps whichever copy is iterated LAST, rather than genuinely comparing timeline
        # length, would pass both undetected. Here the longer copy sits in .claude ("00-",
        # processed FIRST) and the shorter one in .claude-sdd ("01-", processed SECOND), so
        # such a mutation now picks the wrong (shorter) copy as keeper.
        #
        # This does NOT exercise _sid_keepers()'s copy_id lexical tie-break: the two copies
        # here differ in LENGTH (7 vs 6 entries), so the (-len, copy_id) sort key never
        # reaches the copy_id component. As copy_id strings, ".claude/X" actually sorts
        # AFTER ".claude-sdd/X" (after the shared ".claude" prefix, "/" 0x2F > "-" 0x2D) --
        # the opposite of bare profile-name order -- but that fact plays no role here; the
        # real tie-break is exercised by
        # test_relations_never_names_an_excluded_copy_as_the_fork_target below.
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            early_proj = _session_dir(corpus_dir, "00-.claude", "p")
            late_proj = _session_dir(corpus_dir, "01-.claude-sdd", "p")

            base = [_entry(f"u{i}", f"2026-09-20T10:00:0{i}Z", "cross-sid") for i in range(5)]
            early_lines = base + [
                _entry("early-only-1", "2026-09-20T10:00:05Z", "cross-sid"),
                _entry("early-only-2", "2026-09-20T10:00:06Z", "cross-sid"),
            ]
            late_lines = base + [_entry("late-only", "2026-09-20T10:00:05Z", "cross-sid")]

            _write_lines(early_proj / "cross-sid.jsonl", early_lines)
            _write_lines(late_proj / "cross-sid.jsonl", late_lines)

            sess = transcripts.sessions(corpus_dir)
            excl = transcripts.exclusions(sess, set())

            self.assertEqual(
                excl.get(".claude-sdd/cross-sid"), "divergent-duplicate-of:.claude/cross-sid"
            )
            self.assertNotIn(".claude/cross-sid", excl)

    def test_relations_never_names_an_excluded_copy_as_the_fork_target(self):
        # R32: exclusions() Stage D and relations() must pick the SAME keeper for a tied-
        # length R22 duplicate, via the shared _sid_keepers() function -- otherwise Stage D
        # can exclude one profile's copy while relations() still points a fork's "original"
        # at that very excluded copy_id. dup-x is byte-identical (tied length, 5 entries) in
        # both .claude-sdd (numeric prefix "00-", processed first) and .claude-kat ("01-",
        # processed second); fork-y shares dup-x's first FORK_PREFIX_LEN uuids/timestamps and
        # diverges after. _sid_keepers()'s copy_id tie-break picks .claude-kat/dup-x as
        # keeper ('k' < 's'), even though it is processed SECOND -- the opposite of Stage
        # D's old independent sort, which (stable, no tie-break) kept whichever copy was
        # encountered FIRST, i.e. .claude-sdd/dup-x -- reproducing the reviewer's exact
        # "Stage D excludes .claude-kat/X, relations() still names .claude-kat/X as the fork
        # target" scenario.
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            sdd_proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")
            kat_proj = _session_dir(corpus_dir, "01-.claude-kat", "p")

            dup = [_entry(f"u{i}", f"2026-09-20T10:00:0{i}Z", "dup-x") for i in range(5)]
            fork = dup + [_entry("f5", "2026-09-21T09:00:00Z", "fork-y")]

            _write_lines(sdd_proj / "dup-x.jsonl", dup)
            _write_lines(kat_proj / "dup-x.jsonl", dup)
            _write_lines(kat_proj / "fork-y.jsonl", fork)

            sess = transcripts.sessions(corpus_dir)
            excl = transcripts.exclusions(sess, set())
            rels = transcripts.relations(sess, excl)

            self.assertEqual(
                excl.get(".claude-sdd/dup-x"), "duplicate-prefix-of:.claude-kat/dup-x"
            )
            self.assertNotIn(".claude-kat/dup-x", excl)

            fork_rel = rels.get(".claude-kat/fork-y")
            self.assertEqual(fork_rel, "fork-of:.claude-kat/dup-x")
            fork_target = fork_rel.split("fork-of:", 1)[1]
            self.assertNotIn(fork_target, excl)

    def test_relations_names_the_surviving_copy_when_the_forks_target_is_excluded_elsewhere(
        self,
    ):
        # R36: this time the target sid's copy is dropped by Stage B (sdk-cli), a DIFFERENT
        # exclusion reason than the R22 test above -- so Stage D's dup-check never even runs
        # on this sid (only one copy of it remains once Stage B has excluded the other).
        # Two profile copies of sid "dup-x" carry byte-identical content: .claude-kat's is
        # marked entrypoint="sdk-cli" (excluded at Stage B), .claude-sdd's is plain "cli"
        # (survives). fork-y shares dup-x's first FORK_PREFIX_LEN uuids and diverges after.
        # relations() must name the SURVIVING .claude-sdd copy as the fork's original, never
        # the excluded .claude-kat one -- proving the fallback representative-selection
        # (non_excluded, not sessions_list) works for a non-R22 exclusion reason too.
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            sdd_proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")
            kat_proj = _session_dir(corpus_dir, "01-.claude-kat", "p")

            dup = [_entry(f"u{i}", f"2026-09-20T10:00:0{i}Z", "dup-x") for i in range(5)]
            dup_sdk = [
                _entry(f"u{i}", f"2026-09-20T10:00:0{i}Z", "dup-x", entrypoint="sdk-cli")
                for i in range(5)
            ]
            fork = dup + [_entry("f5", "2026-09-21T09:00:00Z", "fork-y")]

            _write_lines(sdd_proj / "dup-x.jsonl", dup)
            _write_lines(kat_proj / "dup-x.jsonl", dup_sdk)
            _write_lines(kat_proj / "fork-y.jsonl", fork)

            sess = transcripts.sessions(corpus_dir)
            excl = transcripts.exclusions(sess, set())
            rels = transcripts.relations(sess, excl)

            self.assertEqual(excl.get(".claude-kat/dup-x"), "sdk-cli")
            self.assertNotIn(".claude-sdd/dup-x", excl)

            self.assertEqual(rels.get(".claude-kat/fork-y"), "fork-of:.claude-sdd/dup-x")

    def test_an_excluded_copy_that_would_be_a_fork_gets_no_relation_entry(self):
        # R36: the FORK side of the pair is excluded this time (scratchpad-project, Stage C),
        # not the original. Even though its first FORK_PREFIX_LEN uuids match dup-z's and it
        # would otherwise be named "fork-of:", an excluded copy must own no relation entry at
        # all -- it must not appear as a key in the returned map, whatever it would have
        # pointed at.
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            proj = _session_dir(corpus_dir, "00-.claude-kat", "p")
            scratch_proj = _session_dir(corpus_dir, "00-.claude-kat", "scratchpad-p")

            base = [_entry(f"u{i}", f"2026-09-20T10:00:0{i}Z", "dup-z") for i in range(5)]
            fork = base + [_entry("f5", "2026-09-21T09:00:00Z", "fork-scratch")]

            _write_lines(proj / "dup-z.jsonl", base)
            _write_lines(scratch_proj / "fork-scratch.jsonl", fork)

            sess = transcripts.sessions(corpus_dir)
            excl = transcripts.exclusions(sess, set())
            rels = transcripts.relations(sess, excl)

            fork_cid = ".claude-kat/fork-scratch"
            self.assertEqual(excl.get(fork_cid), "scratchpad-project")
            self.assertNotIn(fork_cid, rels)


class WriteExclusions(unittest.TestCase):
    def test_write_exclusions_updates_manifest_preserving_other_keys(self):
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            before = json.loads((corpus_dir / "manifest.json").read_text())

            transcripts.write_exclusions(
                corpus_dir, {".claude-kat/b": "sdk-cli", ".claude-kat/a": "scratchpad-project"}
            )
            manifest = json.loads((corpus_dir / "manifest.json").read_text())

            # R1(update): EVERY pre-existing key survives write_exclusions unchanged, not
            # just corpus_id — "exclusions" is the one key write_exclusions is meant to change.
            for key, value in before.items():
                if key == "exclusions":
                    continue
                self.assertEqual(manifest.get(key), value, f"key {key!r} was not preserved")

            self.assertEqual(
                manifest["exclusions"],
                [
                    {"sid": ".claude-kat/a", "reason": "scratchpad-project"},
                    {"sid": ".claude-kat/b", "reason": "sdk-cli"},
                ],
            )

    def test_read_exclusions_is_the_inverse_of_write_exclusions(self):
        # R106: the manifest form observability reads back.
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            excl = {".claude-kat/b": "sdk-cli", ".claude-sdd/a": "fork-of-excluded:.claude-sdd/x"}
            transcripts.write_exclusions(corpus_dir, excl)
            self.assertEqual(transcripts.read_exclusions(corpus_dir), excl)


# --- Task 14 (spec Amendment 7) -----------------------------------------------------------


def _queued(uuid, ts, text, mode="prompt", origin_kind="human", entry_origin=None):
    """A real-shaped `queued_command` attachment (R103). origin_kind=None writes NO attachment
    origin at all; entry_origin sets an ENTRY-level origin, which no real attachment carries."""
    att = {"type": "queued_command", "commandMode": mode, "prompt": text}
    if origin_kind is not None:
        att["origin"] = {"kind": origin_kind}
    e = {"type": "attachment", "uuid": uuid, "timestamp": ts, "sessionId": "s1",
         "entrypoint": "cli", "attachment": att}
    if entry_origin is not None:
        e["origin"] = {"kind": entry_origin}
    return e


def _with_origin(entry, kind):
    entry["origin"] = {"kind": kind}
    return entry


class QueuedOperatorMessages(unittest.TestCase):
    """R103: a message the operator typed while the agent ran is a queued_command attachment."""

    def test_a_peer_queued_command_stays_out(self):
        peer = _queued("q1", "2026-09-20T10:00:00Z", "a peer's message", origin_kind="peer")
        self.assertEqual(transcripts.operator_messages([peer]), [])

    def test_a_task_notification_queued_command_stays_out(self):
        # Both measured task-notification shapes: no origin, and origin task-notification. The
        # human-origin one also stays out: commandMode is what makes it a notification.
        for origin_kind in (None, "task-notification", "human"):
            with self.subTest(origin_kind=origin_kind):
                note = _queued("q1", "2026-09-20T10:00:00Z", "a background task finished",
                               mode="task-notification", origin_kind=origin_kind)
                self.assertEqual(transcripts.operator_messages([note]), [])

    def test_an_origin_less_prompt_mode_queued_command_stays_out(self):
        bare = _queued("q1", "2026-09-20T10:00:00Z", "no origin at all", origin_kind=None)
        self.assertEqual(transcripts.operator_messages([bare]), [])

    def test_an_is_meta_or_sidechain_queued_command_stays_out(self):
        # The ledger's R103 "not isMeta", and R104's "a sidechain entry is never the operator":
        # each flag alone keeps an otherwise-admitted human prompt-mode attachment out.
        for flag in ("isMeta", "isSidechain"):
            with self.subTest(flag=flag):
                q = _queued("q1", "2026-09-20T10:00:00Z", "an injected-looking message")
                q[flag] = True
                self.assertEqual(transcripts.operator_messages([q]), [])

    def test_a_human_queued_command_enters_once_with_its_prompt_as_text(self):
        human = _queued("q1", "2026-09-20T10:00:00Z", "stop, that is the wrong file")
        got = transcripts.operator_messages([human])
        self.assertEqual(got, [human])
        self.assertEqual(transcripts._message_text(human), "stop, that is the wrong file")

    def test_the_attachments_own_origin_decides_never_an_entry_level_one(self):
        # LOAD-BEARING: each fixture's two origins disagree. Reading the entry-level origin
        # admits the first and drops the second; reading the attachment's does the opposite.
        entry_human_att_peer = _queued("q1", "2026-09-20T10:00:00Z", "peer text",
                                       origin_kind="peer", entry_origin="human")
        entry_peer_att_human = _queued("q2", "2026-09-20T10:00:01Z", "operator text",
                                       origin_kind="human", entry_origin="peer")
        got = transcripts.operator_messages([entry_human_att_peer, entry_peer_att_human])
        self.assertEqual(got, [entry_peer_att_human])
    def test_an_attachment_origin_kind_the_corpus_does_not_contain_stays_out(self):
        # I-2 (R103 site): POSITIVE identification, so a kind no fixture or corpus attachment
        # carries stays out. LOAD-BEARING: prompt mode, a str prompt, no isMeta/isSidechain --
        # every other guard admits it, so only the attachment-origin check can refuse it, and a
        # peer-only denylist there admits it.
        for kind in ("system", "bot"):
            with self.subTest(kind=kind):
                q = _queued("q1", "2026-09-20T10:00:00Z", "a machine's message", origin_kind=kind)
                self.assertEqual(transcripts.operator_messages([q]), [])

    def test_a_queued_message_equal_to_a_kept_prompt_counts_once_each(self):
        # R130: no dedupe -- the queued message and the kept prompt are TWO operator messages,
        # one per channel, whichever comes first. LOAD-BEARING: the texts are byte-equal, so a
        # restored dedupe (normalized or not, later-only or two-way) drops the queued one.
        queued = _queued("q1", "2026-09-20T10:00:00Z", "use the other file")
        later = _entry("u2", "2026-09-20T10:00:05Z", "s1", content="use the other file")
        earlier = _entry("u0", "2026-09-20T09:59:55Z", "s1", content="use the other file")
        for order, entries in (("queued, then an equal kept prompt", [queued, later]),
                               ("an equal kept prompt, then queued", [earlier, queued])):
            with self.subTest(order=order):
                self.assertEqual(transcripts.operator_messages(entries), entries)

    def test_file_order_is_kept_across_both_kinds(self):
        p1 = _entry("u1", "2026-09-20T10:00:00Z", "s1", content="first")
        q = _queued("q2", "2026-09-20T10:00:01Z", "second, typed mid-turn")
        p3 = _entry("u3", "2026-09-20T10:00:02Z", "s1", content="third")
        self.assertEqual(transcripts.operator_messages([p1, q, p3]), [p1, q, p3])


class PositiveIdentification(unittest.TestCase):
    """R104: an entry that carries an origin counts only if origin.kind == "human"."""

    def test_a_synthetic_peer_origin_user_entry_stays_out_and_a_human_one_is_kept(self):
        peer = _with_origin(_entry("u1", "2026-09-20T10:00:00Z", "s1", content="hello"), "peer")
        human = _with_origin(_entry("u2", "2026-09-20T10:00:01Z", "s1", content="hello"), "human")
        self.assertEqual(transcripts.operator_messages([peer, human]), [human])
    def test_an_origin_kind_the_corpus_does_not_contain_stays_out(self):
        # I-2 (R104 site): POSITIVE identification, so a kind no fixture or corpus entry carries
        # stays out. LOAD-BEARING: plain text (no fallback tag, no marker, no wrapper), no
        # isMeta/isSidechain/promptSource -- every other guard admits it, so only the origin check
        # can refuse it, and a peer-only denylist there admits it.
        for kind in ("system", "bot"):
            with self.subTest(kind=kind):
                e = _with_origin(_entry("u1", "2026-09-20T10:00:00Z", "s1", content="hello"), kind)
                self.assertEqual(transcripts.operator_messages([e]), [])

    def test_each_fallback_tag_excludes_an_origin_less_entry(self):
        for tag in (
            "<cross-session-message from=\"x\">", "<teammate-message>", "<agent-message>",
            "<bash-input>", "<bash-stdout>", "<bash-stderr>", "<local-command-stderr>",
            "<system-reminder>",
        ):
            with self.subTest(tag=tag):
                e = _entry("u1", "2026-09-20T10:00:00Z", "s1", content=f"{tag}body")
                self.assertEqual(transcripts.operator_messages([e]), [])

    def test_the_fallback_tags_do_not_apply_to_a_human_origin_entry(self):
        # The tags are the FALLBACK for origin-less entries; where the harness names the human,
        # that positive identification decides.
        e = _with_origin(
            _entry("u1", "2026-09-20T10:00:00Z", "s1", content="<system-reminder> quoted"), "human"
        )
        self.assertEqual(transcripts.operator_messages([e]), [e])

    def test_an_is_sidechain_user_entry_is_never_an_operator_message(self):
        # The old-layout top-level agent-<id>.jsonl opens with the parent's brief, isSidechain.
        brief = _entry("u1", "2026-09-20T10:00:00Z", "agent-1", content="do the subtask")
        brief["isSidechain"] = True
        real = _entry("u2", "2026-09-20T10:00:01Z", "agent-1", content="a real prompt")
        real["isSidechain"] = False
        self.assertEqual(transcripts.operator_messages([brief, real]), [real])


class OperatorRejections(unittest.TestCase):
    """R105: tool-rejection feedback carries the harness marker `the user said:`."""

    _REJECTED = (
        "The user doesn't want to proceed with this tool use. The tool use was rejected. "
        "To tell you how to proceed, the user said:\n  edit the test file instead  "
    )

    def test_a_marker_in_a_tool_result_yields_one_with_the_text_after_it(self):
        for shape, content in (
            ("str", self._REJECTED),
            ("list", [{"type": "text", "text": self._REJECTED}]),
        ):
            with self.subTest(shape=shape):
                e = _entry("u1", "2026-09-20T10:00:00Z", "s1", content=[
                    {"type": "tool_result", "tool_use_id": "t1", "is_error": True,
                     "content": content},
                ])
                self.assertEqual(
                    transcripts.operator_rejections([e]),
                    [{"uuid": "u1", "ts": "2026-09-20T10:00:00Z",
                      "text": "edit the test file instead"}],
                )

    def test_a_tool_result_without_the_marker_yields_none(self):
        # LOAD-BEARING: is_error True, so the R127 guard admits the block and only the marker
        # check can refuse it (without it, a "marker not required" mutant would survive here).
        e = _entry("u1", "2026-09-20T10:00:00Z", "s1", content=[
            {"type": "tool_result", "tool_use_id": "t1", "is_error": True,
             "content": "ok, the file was written"},
        ])
        self.assertEqual(transcripts.operator_rejections([e]), [])

    def test_a_marker_inside_a_text_block_yields_none(self):
        # A marker in a plain text block (text in `text`, no `content`). LOAD-BEARING and unreal:
        # is_error True, so the R127 guard admits the block -- what refuses it is the type guard
        # AND _tool_result_text (no `content`), so this pins a text-reading branch (I3c), NOT
        # the type guard alone; test_a_non_tool_result_block_carrying_content... pins that.
        e = _entry("u1", "2026-09-20T10:00:00Z", "s1", content=[
            {"type": "text", "is_error": True, "text": "earlier the user said: use the other file"},
        ])
        self.assertEqual(transcripts.operator_rejections([e]), [])

    def test_a_non_tool_result_block_carrying_content_and_the_marker_yields_none(self):
        # m-2: the TYPE guard. LOAD-BEARING and unreal: is_error True and a str `content` with
        # the marker, so the R127 guard and _tool_result_text both admit it -- only the
        # block-type check can refuse it.
        e = _entry("u1", "2026-09-20T10:00:00Z", "s1", content=[
            {"type": "text", "is_error": True, "content": self._REJECTED},
        ])
        self.assertEqual(transcripts.operator_rejections([e]), [])

    def test_a_marker_bearing_tool_result_whose_is_error_is_not_true_yields_none(self):
        # R127: is_error must be True. LOAD-BEARING: a real-shaped marker-bearing tool_result, so
        # the type guard and the marker check both admit it -- only is_error can refuse it. The
        # truthy string is unreal and pins "== True", not mere truthiness.
        for label, flag in (("is_error False", {"is_error": False}), ("is_error absent", {}),
                            ("is_error the truthy string 'true'", {"is_error": "true"})):
            with self.subTest(label):
                block = {"type": "tool_result", "tool_use_id": "t1", "content": self._REJECTED}
                block.update(flag)
                e = _entry("u1", "2026-09-20T10:00:00Z", "s1", content=[block])
                self.assertEqual(transcripts.operator_rejections([e]), [])

    def test_a_rejection_is_not_an_operator_message(self):
        e = _entry("u1", "2026-09-20T10:00:00Z", "s1", content=[
            {"type": "tool_result", "tool_use_id": "t1", "content": self._REJECTED},
        ])
        self.assertEqual(transcripts.operator_messages([e]), [])


class ReadJsonlTornUtf8(unittest.TestCase):
    """R113: a torn multi-byte sequence must not raise; lines with U+FFFD are counted."""

    def test_a_torn_multibyte_line_does_not_raise_and_is_counted(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = pathlib.Path(tmp) / "session.jsonl"
            path.write_bytes(
                b'{"type": "user", "uuid": "a"}\n'
                # "caf" + the first byte of a 2-byte sequence: still valid JSON once replaced.
                b'{"type": "user", "uuid": "b", "t": "caf\xc3"}\n'
                # A process killed mid-character: torn UTF-8 AND torn JSON.
                b'{"type": "user", "uuid": "c", "t": "\xe2\x82'
            )
            stats = {}
            try:
                entries, skipped = transcripts.read_jsonl(path, stats=stats)
            except UnicodeDecodeError as e:  # the defect is the raise: report it as a failure
                self.fail(f"read_jsonl raised on a torn UTF-8 line: {e}")
            self.assertEqual([e["uuid"] for e in entries], ["a", "b"])
            self.assertEqual(entries[1]["t"], "caf�")
            self.assertEqual(skipped, 1)
            self.assertEqual(stats, {"skipped": 1, "replaced_lines": 2})

    def test_stats_accumulate_across_calls_and_a_clean_file_adds_zero(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = pathlib.Path(tmp) / "session.jsonl"
            path.write_bytes(b'{"type": "user", "uuid": "a", "t": "\xc3"}\n')
            clean = pathlib.Path(tmp) / "clean.jsonl"
            clean.write_bytes(b'{"type": "user", "uuid": "b"}\n')
            stats = {}
            try:
                transcripts.read_jsonl(path, stats=stats)
                transcripts.read_jsonl(path, stats=stats)
                transcripts.read_jsonl(clean, stats=stats)
            except UnicodeDecodeError as e:  # the defect is the raise: report it as a failure
                self.fail(f"read_jsonl raised on a torn UTF-8 line: {e}")
            self.assertEqual(stats, {"skipped": 0, "replaced_lines": 2})


def _timeline(sid, prefix, n_own, t0=0):
    """`prefix` shared uuids then n_own uuids unique to `sid`, one second apart."""
    uuids = list(prefix) + [f"{sid}-own{i}" for i in range(n_own)]
    return [_entry(u, f"2026-09-20T10:00:{t0 + i:02d}Z", sid) for i, u in enumerate(uuids)]


class ForkOfExcluded(unittest.TestCase):
    """R106: a copy sharing a FORK_PREFIX_LEN uuid prefix with an excluded-by-spec copy is
    excluded too, reason fork-of-excluded:<that copy>."""

    SPEC = "3c5b02df-b6ce-45f5-9d03-1194e38465c0"

    def test_a_fork_of_a_spec_excluded_session_is_excluded_with_its_reason(self):
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")
            shared = [f"sh{i}" for i in range(transcripts.FORK_PREFIX_LEN)]
            _write_lines(proj / f"{self.SPEC}.jsonl", _timeline(self.SPEC, shared, 2))
            _write_lines(proj / "fork-sid.jsonl", _timeline("fork-sid", shared, 3, t0=20))
            _write_lines(proj / "other-sid.jsonl", _timeline("other-sid", [], 6))
            excl = transcripts.exclusions(transcripts.sessions(corpus_dir), {self.SPEC})
            self.assertEqual(excl[f".claude-sdd/{self.SPEC}"], "excluded-by-spec")
            self.assertEqual(
                excl.get(".claude-sdd/fork-sid"), f"fork-of-excluded:.claude-sdd/{self.SPEC}"
            )
            self.assertNotIn(".claude-sdd/other-sid", excl)

    def test_a_shorter_shared_prefix_is_not_a_fork(self):
        # LOAD-BEARING: fork-sid shares FORK_PREFIX_LEN - 1 uuids, then diverges -- one short.
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")
            shared = [f"sh{i}" for i in range(transcripts.FORK_PREFIX_LEN - 1)]
            _write_lines(proj / f"{self.SPEC}.jsonl", _timeline(self.SPEC, shared, 3))
            _write_lines(proj / "fork-sid.jsonl", _timeline("fork-sid", shared, 3, t0=20))
            excl = transcripts.exclusions(transcripts.sessions(corpus_dir), {self.SPEC})
            self.assertNotIn(".claude-sdd/fork-sid", excl)

    def test_a_fork_of_an_sdk_cli_session_is_not_propagated(self):
        # Propagation is seeded by the spec exclusion only: a fork of a headless session keeps
        # R28's reporting-only status.
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")
            shared = [f"sh{i}" for i in range(transcripts.FORK_PREFIX_LEN)]
            headless = _timeline("headless", shared, 2)
            for e in headless:
                e["entrypoint"] = "sdk-cli"
            _write_lines(proj / "headless.jsonl", headless)
            _write_lines(proj / "fork-sid.jsonl", _timeline("fork-sid", shared, 3, t0=20))
            excl = transcripts.exclusions(transcripts.sessions(corpus_dir), {self.SPEC})
            self.assertEqual(excl[".claude-sdd/headless"], "sdk-cli")
            self.assertNotIn(".claude-sdd/fork-sid", excl)


class EntrypointChanged(unittest.TestCase):
    """R107: a kept session whose first and last entrypoint differ is flagged and counted."""

    def test_sessions_flags_a_changed_entrypoint_and_the_count_covers_kept_copies_only(self):
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")
            _write_lines(proj / "changed.jsonl", [
                _entry("c0", "2026-09-20T10:00:00Z", "changed", entrypoint="cli"),
                _entry("c1", "2026-09-20T10:00:01Z", "changed", entrypoint="sdk-ts"),
            ])
            _write_lines(proj / "same.jsonl", [
                _entry("s0", "2026-09-20T10:00:00Z", "same", entrypoint="cli"),
                _entry("s1", "2026-09-20T10:00:01Z", "same", entrypoint="cli"),
            ])
            # Excluded (sdk-cli first) although its entrypoint changes: not counted.
            _write_lines(proj / "headless.jsonl", [
                _entry("h0", "2026-09-20T10:00:00Z", "headless", entrypoint="sdk-cli"),
                _entry("h1", "2026-09-20T10:00:01Z", "headless", entrypoint="cli"),
            ])
            sess = {s.sid: s for s in transcripts.sessions(corpus_dir)}
            self.assertTrue(sess["changed"].entrypoint_changed)
            self.assertEqual(
                (sess["changed"].entrypoint, sess["changed"].last_entrypoint), ("cli", "sdk-ts")
            )
            self.assertFalse(sess["same"].entrypoint_changed)
            self.assertTrue(sess["headless"].entrypoint_changed)
            sessions_list = list(sess.values())
            excl = transcripts.exclusions(sessions_list, set())
            self.assertEqual(excl[".claude-sdd/headless"], "sdk-cli")
            self.assertEqual(transcripts.entrypoint_changed_count(sessions_list, excl), 1)


if __name__ == "__main__":
    unittest.main()
