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
    def test_a_fork_is_reported_as_a_relation_and_its_original_kept(self):
        # R28: forks are no longer excluded — see relations()/attribute_entries().
        # Sid naming is deliberately a-orig/z-fork rather than orig-sid/fork-sid: lexically
        # "a-orig" < "z-fork" is the SAME direction as which one is chronologically earlier
        # (the real original) — the OPPOSITE of the old orig-sid/fork-sid naming (where
        # "fork-sid" < "orig-sid" lexically). Flipping it means a mutation that substitutes
        # sid/positional order for the real timestamp comparison produces a DIFFERENT
        # (wrong) answer here, instead of coincidentally matching.
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            proj = _session_dir(corpus_dir, "00-.claude-kat", "p")

            shared = [_entry(f"u{i}", f"2026-09-20T10:00:0{i}Z", "a-orig") for i in range(5)]
            # Two more entries with the SAME uuid on both branches, extending the common
            # prefix past FORK_PREFIX_LEN (5) — the real divergence is at index 7. If the
            # divergence-scan loop were removed (defaulting to divergence==FORK_PREFIX_LEN),
            # this would compare the wrong pair of entries and fail.
            common_tail = [
                _entry("c5", "2026-09-20T10:00:05Z", "a-orig"),
                _entry("c6", "2026-09-20T10:00:06Z", "a-orig"),
            ]
            # The original continues forward in real time right after the common prefix.
            orig_lines = shared + common_tail + [
                _entry("u7-orig", "2026-09-20T10:00:07Z", "a-orig", content="continue original work")
            ]
            fork_shared = [_entry(f"u{i}", f"2026-09-20T10:00:0{i}Z", "z-fork") for i in range(5)]
            fork_common_tail = [
                _entry("c5", "2026-09-20T10:00:05Z", "z-fork"),
                _entry("c6", "2026-09-20T10:00:06Z", "z-fork"),
            ]
            # The fork was replayed later — same shared prefix, but its own sessionId and a
            # later-timestamped divergent entry.
            fork_lines = fork_shared + fork_common_tail + [
                _entry("u7-fork", "2026-09-21T09:00:00Z", "z-fork", content="branch from the replay")
            ]

            _write_lines(proj / "a-orig.jsonl", orig_lines)
            _write_lines(proj / "z-fork.jsonl", fork_lines)

            sess = transcripts.sessions(corpus_dir)
            self.assertEqual({s.sid for s in sess}, {"a-orig", "z-fork"})

            excl = transcripts.exclusions(sess, set())
            rels = transcripts.relations(sess)
            kept_key = ".claude-kat/a-orig"
            fork_key = ".claude-kat/z-fork"

            # R28(i): forks are never an exclusion reason anymore.
            self.assertEqual(excl, {})

            # R28(ii): the relationship is reported instead, oriented correctly.
            self.assertEqual(rels.get(fork_key), "fork-of:" + kept_key)
            self.assertNotIn(kept_key, rels)

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
            self.assertEqual(transcripts.relations(sess), {})


class AttributeEntries(unittest.TestCase):
    def test_a_fork_pairs_shared_prefix_goes_to_the_longer_transcript(self):
        # R28(iii): a fork pair diverging right at FORK_PREFIX_LEN — the shared prefix is
        # attributed to whichever copy has MORE total operator-message uuids, and each
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
        # Both cases above put the longer copy in .claude-kat (lexically LATER than
        # .claude-sdd) — a "keep the later profile" mutation would pass both undetected.
        # Here the longer copy is in .claude (lexically EARLIER than .claude-sdd), so such a
        # mutation picks the wrong (shorter) copy as keeper.
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


if __name__ == "__main__":
    unittest.main()
