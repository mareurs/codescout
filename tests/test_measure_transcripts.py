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
    def test_a_fork_is_excluded_and_its_original_kept(self):
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            proj = _session_dir(corpus_dir, "00-.claude-kat", "p")

            shared = [
                _entry(f"u{i}", f"2026-09-20T10:00:0{i}Z", "orig-sid")
                for i in range(5)
            ]
            # The original continues forward in real time right after the shared prefix.
            orig_lines = shared + [_entry("u5-orig", "2026-09-20T10:00:05Z", "orig-sid",
                                           content="continue original work")]
            # The fork was replayed later — same first 5 uuids, but its own sessionId and a
            # later-timestamped divergent entry.
            fork_shared = [
                _entry(f"u{i}", f"2026-09-20T10:00:0{i}Z", "fork-sid")
                for i in range(5)
            ]
            fork_lines = fork_shared + [_entry("u5-fork", "2026-09-21T09:00:00Z", "fork-sid",
                                                content="branch from the replay")]

            _write_lines(proj / "orig-sid.jsonl", orig_lines)
            _write_lines(proj / "fork-sid.jsonl", fork_lines)

            sess = transcripts.sessions(corpus_dir)
            self.assertEqual({s.sid for s in sess}, {"orig-sid", "fork-sid"})

            excl = transcripts.exclusions(sess, set())
            kept_key = "[.]claude-kat/orig-sid".replace("[.]", ".")
            fork_key = ".claude-kat/fork-sid"
            self.assertNotIn(kept_key, excl)
            self.assertEqual(excl.get(fork_key), "fork-of:orig-sid")

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


class WriteExclusions(unittest.TestCase):
    def test_write_exclusions_updates_manifest_preserving_other_keys(self):
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            transcripts.write_exclusions(
                corpus_dir, {".claude-kat/b": "sdk-cli", ".claude-kat/a": "scratchpad-project"}
            )
            manifest = json.loads((corpus_dir / "manifest.json").read_text())
            self.assertEqual(manifest["corpus_id"], "c-test")
            self.assertEqual(
                manifest["exclusions"],
                [
                    {"sid": ".claude-kat/a", "reason": "scratchpad-project"},
                    {"sid": ".claude-kat/b", "reason": "sdk-cli"},
                ],
            )


if __name__ == "__main__":
    unittest.main()
