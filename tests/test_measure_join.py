"""Stage 1b: the events database and joins (Task 6).

Run: ~/work/claude/prompt-engineering/.venv/bin/python -m pytest tests/test_measure_join.py -v
"""
import hashlib
import importlib.util
import json
import pathlib
import sqlite3
import subprocess
import sys
import tempfile
import unittest
from datetime import datetime, timedelta, timezone

REPO_ROOT = pathlib.Path(__file__).resolve().parents[1]
MEASURE = REPO_ROOT / "scripts" / "measure"
sys.path.insert(0, str(MEASURE))


def _load(name):
    spec = importlib.util.spec_from_file_location(f"measure_{name}", MEASURE / f"{name}.py")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


transcripts = _load("transcripts")
join = _load("join")

BASE = datetime(2026, 9, 26, 10, 0, 0, tzinfo=timezone.utc)


def _iso(dt):
    return dt.strftime("%Y-%m-%dT%H:%M:%SZ")


def _sql_ts(dt):
    return dt.strftime("%Y-%m-%d %H:%M:%S")


# --- transcripts.py-fixture-shape helpers, mirrored from tests/test_measure_transcripts.py ---


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


def _make_corpus(tmp_path, repos=None, bounds=None):
    corpus_dir = tmp_path / "corpus"
    (corpus_dir / "manifest.json").parent.mkdir(parents=True, exist_ok=True)
    manifest = {
        "corpus_id": "c-test", "created_utc": "2026-09-26T00:00:00Z",
        "bounds": bounds or {}, "files": {}, "counts": {}, "versions": {},
        "repos": repos or {}, "exclusions": [],
    }
    (corpus_dir / "manifest.json").write_text(json.dumps(manifest, indent=2, sort_keys=True))
    return corpus_dir


def _session_dir(corpus_dir, profile_dir, project_slug):
    return corpus_dir / "transcripts" / profile_dir / project_slug


# --- join.py-specific helpers ---


def _table_rows(conn, table):
    conn.row_factory = sqlite3.Row
    cur = conn.execute(f"SELECT * FROM {table}")
    return [dict(r) for r in cur]


def _fresh_counts():
    return {
        "tool_events_total": 0,
        "tool_events_exact": 0,
        "tool_events_heuristic": 0,
        "tool_events_none": 0,
        "tool_events_none_not_codescout": 0,
    }


def _candidate(cid, bare_sid, name, input_, ts, tool_use_id=None, agent_id=None):
    return {
        "cid": cid,
        "bare_sid": bare_sid,
        "agent_id": agent_id,
        "tool_use_id": tool_use_id,
        "ts": ts,
        "name": name,
        "input": input_,
    }


def _usage_row(row_key, tool_name, cc_session_id, input_json, started_at,
               agent_id=None, tool_use_id=None):
    return {
        "_row_key": row_key,
        "tool_name": tool_name,
        "cc_session_id": cc_session_id,
        "agent_id": agent_id,
        "input_json": input_json,
        "output_json": None,
        "deliveries_json": None,
        "tool_use_id": tool_use_id,
        "started_at": started_at,
        "called_at": started_at,
    }


def _make_git_repo(root):
    root.mkdir(parents=True, exist_ok=True)

    def run(*args):
        subprocess.run(
            ["git", *args], cwd=str(root), check=True, capture_output=True, text=True,
        )

    run("init", "-q")
    run("config", "user.name", "t")
    run("config", "user.email", "t@t")
    (root / "a.txt").write_text("1")
    run("add", "a.txt")
    run("commit", "-q", "-m", "feat: add a\n\nSession-Id: sess-123")
    (root / "a.txt").write_text("2")
    (root / "b.txt").write_text("1")
    run("add", "a.txt", "b.txt")
    run("commit", "-q", "-m", "fix: tweak a and add b\n\nSession-Id: sess-456\nBody-Line-2")
    head = subprocess.run(
        ["git", "rev-parse", "HEAD"], cwd=str(root), check=True, capture_output=True, text=True
    ).stdout.strip()
    return head


class UtcOrdering(unittest.TestCase):
    def test_utc_orders_mixed_formats_correctly(self):
        a = join.utc("2026-09-26 05:22:48")
        b = join.utc("2026-09-26T05:22:48.812Z")
        c = join.utc("2026-09-26 05:22:49.001")
        self.assertLess(a, b)
        self.assertLess(b, c)


class TokensOncePerMessageId(unittest.TestCase):
    def test_tokens_are_counted_once_per_message_id(self):
        usage = {"input_tokens": 10, "output_tokens": 20, "cache_creation_input_tokens": 5}
        entries = [
            {"uuid": "u1", "message": {"id": "m1", "usage": usage}},
            {"uuid": "u2", "message": {"id": "m1", "usage": usage}},
            {"uuid": "u3", "message": {"id": "m1", "usage": usage}},
        ]
        tokens = join._compute_tokens(entries)
        self.assertEqual(tokens["u1"], 35)
        self.assertEqual(tokens["u2"], 0)
        self.assertEqual(tokens["u3"], 0)
        self.assertEqual(sum(tokens.values()), 35)


class OperatorRuleMarkerDelivery(unittest.TestCase):
    def test_the_operator_rule_marker_parses_to_a_delivery_row(self):
        # Literal text retrieved from a live usage.db row (tool_calls id=143310, 2026-09-26).
        block1 = (
            '{\n  "status": "ok",\n  "wrote_to": "/home/marius/work/claude/codescout",\n'
            '  "abs_path": "/home/marius/.claude/settings.json"\n}'
        )
        block2 = (
            "<!-- operator-rule OP-4 — delivered once this session for this call shape; "
            "see docs/trackers/operator-rules.md -->\n"
            "Apply every Claude Code config change to all three profiles — "
            "`~/.claude`, `~/.claude-sdd`, `~/.claude-kat`."
        )
        row = {
            "tool_name": "edit_file",
            "called_at": "2026-09-26 05:22:48.812",
            "cc_session_id": "3c5b02df-b6ce-45f5-9d03-1194e38465c0",
            "output_json": json.dumps([{"type": "text", "text": block1},
                                        {"type": "text", "text": block2}]),
            "deliveries_json": None,
            "tool_use_id": None,
        }

        deliveries = join._deliveries_from_usage_row(row)

        self.assertEqual(len(deliveries), 1)
        d = deliveries[0]
        self.assertEqual(d["source"], "usage_output_json")
        self.assertEqual(d["engine_or_hook"], "operator-rule")
        self.assertEqual(d["key"], "OP-4")
        self.assertEqual(d["ts"], "2026-09-26 05:22:48.812")
        self.assertEqual(d["sha256"], hashlib.sha256(block2.encode()).hexdigest())
        self.assertEqual(d["bytes"], len(block2.encode()))


class HookAttachmentDelivery(unittest.TestCase):
    """Regression: real corpus hook_success/hook_additional_context attachments carry
    `content` as a list of 1-2 strings (measured on the task-6 real-data positive control,
    2026-09-26), not always a single string. The first cut of _deliveries_from_attachment
    called `.encode()` directly on whatever `content` held and crashed build_events on the
    real corpus with `AttributeError: 'list' object has no attribute 'encode'`.
    """

    def test_list_shaped_hook_content_is_joined_not_crashed_on(self):
        blocks = ["<EXTREMELY_IMPORTANT>\nfirst block\n", "[cs-hint] second block"]
        entry = {
            "timestamp": "2026-09-26T10:00:00Z",
            "attachment": {
                "type": "hook_additional_context",
                "hookName": "SessionStart",
                "hookEvent": "SessionStart",
                "content": blocks,
                "toolUseID": None,
            },
        }

        deliveries = join._deliveries_from_attachment(entry, "cid/sid1")

        self.assertEqual(len(deliveries), 1)
        d = deliveries[0]
        expected_text = "\n\n".join(blocks)
        self.assertEqual(d["source"], "transcript_hook")
        self.assertEqual(d["sha256"], hashlib.sha256(expected_text.encode()).hexdigest())
        self.assertEqual(d["bytes"], len(expected_text.encode()))

    def test_non_string_list_content_is_treated_as_absent(self):
        # Measured shape under attachment type "task_reminder" (a list of todo dicts) --
        # a different attachment type than hook_success/hook_additional_context, but the
        # coercion helper itself must not crash if it is ever reached with this shape.
        entry = {
            "timestamp": "2026-09-26T10:00:00Z",
            "attachment": {
                "type": "hook_additional_context",
                "hookName": "SomeHook",
                "content": [{"id": "1", "subject": "not text"}],
            },
        }

        deliveries = join._deliveries_from_attachment(entry, "cid/sid1")

        self.assertEqual(deliveries, [])



class SessionCompleteness(unittest.TestCase):
    def test_every_session_is_in_events_or_in_exclusions(self):
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")
            scratch_proj = _session_dir(corpus_dir, "00-.claude-sdd", "p-scratchpad-x")

            _write_lines(proj / "sid-kept.jsonl", [
                _entry("k1", "2026-09-20T10:00:00Z", "sid-kept"),
            ])
            _write_lines(proj / "sid-sdk.jsonl", [
                _entry("s1", "2026-09-20T10:00:00Z", "sid-sdk", entrypoint="sdk-cli"),
            ])
            _write_lines(scratch_proj / "sid-scratch.jsonl", [
                _entry("c1", "2026-09-20T10:00:00Z", "sid-scratch"),
            ])

            events_db = pathlib.Path(tmp) / "events.db"
            counts = join.build_events(corpus_dir, events_db)

            self.assertEqual(counts["sessions_total"], 3)
            self.assertEqual(
                counts["sessions_kept"] + counts["sessions_excluded"], counts["sessions_total"]
            )

            sess_list = transcripts.sessions(corpus_dir)
            excl = transcripts.exclusions(sess_list, set())
            all_cids = {transcripts.copy_id(s) for s in sess_list}
            kept_cids = all_cids - set(excl.keys())

            turns = _table_rows(sqlite3.connect(str(events_db)), "turns")
            sids_in_turns = {r["sid"] for r in turns}

            self.assertEqual(kept_cids | set(excl.keys()), all_cids)
            self.assertEqual(sids_in_turns, kept_cids)


class SubagentUnion(unittest.TestCase):
    def test_subagent_files_are_unioned_across_duplicate_copies(self):
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            sdd_proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")
            kat_proj = _session_dir(corpus_dir, "01-.claude-kat", "p")

            shared = [_entry(f"u{i}", f"2026-09-20T10:00:0{i}Z", "dup-sid") for i in range(5)]
            kat_lines = shared + [_entry("u5", "2026-09-20T10:00:05Z", "dup-sid")]
            _write_lines(sdd_proj / "dup-sid.jsonl", shared)
            _write_lines(kat_proj / "dup-sid.jsonl", kat_lines)

            # Keeper's own subagent file (kat is the superset copy -> keeper).
            _write_lines(kat_proj / "dup-sid" / "subagents" / "sub-a.jsonl", [
                _entry("ka1", "2026-09-20T11:00:00Z", "dup-sid"),
                _entry("ka2", "2026-09-20T11:00:01Z", "dup-sid"),
            ])
            # The excluded duplicate-prefix sibling's UNIQUE subagent file -- must be recovered.
            _write_lines(sdd_proj / "dup-sid" / "subagents" / "sub-b.jsonl", [
                _entry("sb1", "2026-09-20T11:05:00Z", "dup-sid"),
            ])

            events_db = pathlib.Path(tmp) / "events.db"
            counts = join.build_events(corpus_dir, events_db)

            self.assertEqual(counts["subagent_files_unioned"], 2)
            self.assertEqual(counts["subagent_entries_recovered_from_duplicates"], 1)

            turns = _table_rows(sqlite3.connect(str(events_db)), "turns")
            sub_turns = {r["uuid"]: r for r in turns if r["agent_path"]}
            self.assertEqual(set(sub_turns.keys()), {"ka1", "ka2", "sb1"})
            self.assertTrue(sub_turns["sb1"]["agent_path"].endswith("sub-b.jsonl"))
            self.assertEqual(sub_turns["ka1"]["sid"], ".claude-kat/dup-sid")
            self.assertEqual(sub_turns["sb1"]["sid"], ".claude-kat/dup-sid")


class JoinPrecedence(unittest.TestCase):
    def test_an_exact_join_wins_over_a_heuristic_candidate(self):
        conn = sqlite3.connect(":memory:")
        join._create_schema(conn)
        counts = _fresh_counts()

        candidates = [_candidate(
            "p/sid1", "sid1", "mcp__codescout__grep", {"pattern": "x"},
            _iso(BASE), tool_use_id="tu1",
        )]
        usage_rows = [
            _usage_row("db:1", "grep", "sid1", json.dumps({"pattern": "x"}),
                       _sql_ts(BASE), tool_use_id="tu1"),
            _usage_row("db:2", "grep", "sid1", json.dumps({"pattern": "x"}),
                       _sql_ts(BASE), tool_use_id=None),
        ]

        join._join_tool_events(conn, candidates, usage_rows, counts)
        conn.commit()
        rows = _table_rows(conn, "tool_events")

        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]["join_method"], "exact")
        self.assertEqual(rows[0]["usage_row_id"], "db:1")
        self.assertEqual(counts["tool_events_exact"], 1)
        self.assertEqual(counts["tool_events_heuristic"], 0)

    def test_agent_id_is_a_hard_key_when_present(self):
        conn = sqlite3.connect(":memory:")
        join._create_schema(conn)
        counts = _fresh_counts()

        candidates = [_candidate(
            "p/sid1", "sid1", "mcp__codescout__grep", {"pattern": "x"},
            _iso(BASE), agent_id="agentX",
        )]
        usage_rows = [
            _usage_row("db:1", "grep", "sid1", json.dumps({"pattern": "x"}),
                       _sql_ts(BASE), agent_id="agentY"),
            _usage_row("db:2", "grep", "sid1", json.dumps({"pattern": "x"}),
                       _sql_ts(BASE), agent_id="agentX"),
        ]

        join._join_tool_events(conn, candidates, usage_rows, counts)
        conn.commit()
        rows = _table_rows(conn, "tool_events")

        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]["join_method"], "heuristic")
        self.assertEqual(rows[0]["usage_row_id"], "db:2")

    def test_r4_strip_removes_agent_id_key_before_comparison(self):
        conn = sqlite3.connect(":memory:")
        join._create_schema(conn)
        counts = _fresh_counts()

        transcript_input = {"pattern": "x", join.AGENT_ID_STRIP_KEY: "agentX"}
        candidates = [_candidate(
            "p/sid1", "sid1", "mcp__codescout__grep", transcript_input, _iso(BASE),
        )]
        # usage.db stores the input already stripped of the agentId key (R4).
        usage_rows = [
            _usage_row("db:1", "grep", "sid1", json.dumps({"pattern": "x"}), _sql_ts(BASE)),
        ]

        join._join_tool_events(conn, candidates, usage_rows, counts)
        conn.commit()
        rows = _table_rows(conn, "tool_events")

        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]["join_method"], "heuristic")
        self.assertEqual(rows[0]["usage_row_id"], "db:1")

    def test_time_window_nearest_match_wins(self):
        conn = sqlite3.connect(":memory:")
        join._create_schema(conn)
        counts = _fresh_counts()

        candidates = [_candidate(
            "p/sid1", "sid1", "mcp__codescout__grep", {"pattern": "x"}, _iso(BASE),
        )]
        usage_rows = [
            _usage_row("db:far", "grep", "sid1", json.dumps({"pattern": "x"}),
                       _sql_ts(BASE + timedelta(seconds=50))),
            _usage_row("db:near", "grep", "sid1", json.dumps({"pattern": "x"}),
                       _sql_ts(BASE + timedelta(seconds=10))),
            _usage_row("db:outside", "grep", "sid1", json.dumps({"pattern": "x"}),
                       _sql_ts(BASE + timedelta(seconds=join.JOIN_WINDOW_SECONDS + 30))),
        ]

        join._join_tool_events(conn, candidates, usage_rows, counts)
        conn.commit()
        rows = _table_rows(conn, "tool_events")

        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]["join_method"], "heuristic")
        self.assertEqual(rows[0]["usage_row_id"], "db:near")

    def test_time_window_excludes_a_candidate_outside_120_seconds(self):
        conn = sqlite3.connect(":memory:")
        join._create_schema(conn)
        counts = _fresh_counts()

        candidates = [_candidate(
            "p/sid1", "sid1", "mcp__codescout__grep", {"pattern": "x"}, _iso(BASE),
        )]
        usage_rows = [
            _usage_row("db:far", "grep", "sid1", json.dumps({"pattern": "x"}),
                       _sql_ts(BASE + timedelta(seconds=join.JOIN_WINDOW_SECONDS + 30))),
        ]

        join._join_tool_events(conn, candidates, usage_rows, counts)
        conn.commit()
        rows = _table_rows(conn, "tool_events")

        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]["join_method"], "none")


class UsageRowJoinsAtMostOnce(unittest.TestCase):
    def test_each_usage_row_joins_at_most_once(self):
        conn = sqlite3.connect(":memory:")
        join._create_schema(conn)
        counts = _fresh_counts()

        candidates = [
            _candidate("p/sid1", "sid1", "mcp__codescout__grep", {"pattern": "x"},
                       _iso(BASE), tool_use_id="tuA"),
            _candidate("p/sid1", "sid1", "mcp__codescout__grep", {"pattern": "x"},
                       _iso(BASE), tool_use_id="tuB"),
        ]
        usage_rows = [
            _usage_row("db:1", "grep", "sid1", json.dumps({"pattern": "x"}), _sql_ts(BASE)),
        ]

        join._join_tool_events(conn, candidates, usage_rows, counts)
        conn.commit()
        rows = {r["tool_use_id"]: r for r in _table_rows(conn, "tool_events")}

        self.assertEqual(len(rows), 2)
        methods = sorted(r["join_method"] for r in rows.values())
        self.assertEqual(methods, ["heuristic", "none"])
        self.assertEqual(counts["tool_events_heuristic"], 1)
        claimed = [r for r in rows.values() if r["usage_row_id"] == "db:1"]
        self.assertEqual(len(claimed), 1)


class NoneNotCodescout(unittest.TestCase):
    def test_none_not_codescout_for_non_codescout_tool_names(self):
        conn = sqlite3.connect(":memory:")
        join._create_schema(conn)
        counts = _fresh_counts()

        candidates = [_candidate(
            "p/sid1", "sid1", "Agent", {"description": "do a thing"}, _iso(BASE),
        )]

        join._join_tool_events(conn, candidates, [], counts)
        conn.commit()
        rows = _table_rows(conn, "tool_events")

        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]["join_method"], "none")
        self.assertEqual(counts["tool_events_none"], 0)
        self.assertEqual(counts["tool_events_none_not_codescout"], 1)
        self.assertEqual(counts["tool_events_total"], 1)


class CommitsGitLog(unittest.TestCase):
    def test_run_git_log_parses_two_commits_with_trailers_and_file_lists(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo = pathlib.Path(tmp) / "repo"
            head = _make_git_repo(repo)

            commits = join._run_git_log(repo, head, None, None)

            self.assertEqual(len(commits), 2)
            by_subject = {c["subject"]: c for c in commits}
            first = by_subject["feat: add a"]
            second = by_subject["fix: tweak a and add b"]

            self.assertEqual(first["session_id"], "sess-123")
            self.assertEqual(first["files"], ["a.txt"])

            self.assertEqual(second["session_id"], "sess-456")
            self.assertEqual(sorted(second["files"]), ["a.txt", "b.txt"])


class CommitsIntegration(unittest.TestCase):
    def test_build_events_records_commits_and_counts_a_missing_repo(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = pathlib.Path(tmp)
            repo = tmp_path / "repo"
            head = _make_git_repo(repo)

            corpus_dir = _make_corpus(
                tmp_path, repos={"present-repo": head, "missing-repo": "deadbeef"}
            )
            events_db = tmp_path / "events.db"

            counts = join.build_events(corpus_dir, events_db, repo_paths={"present-repo": repo})

            self.assertEqual(counts["commits_total"], 2)
            self.assertEqual(counts["commits_repos_missing"], 1)

            commit_rows = _table_rows(sqlite3.connect(str(events_db)), "commits")
            self.assertEqual(len(commit_rows), 2)
            self.assertTrue(all(r["repo"] == "present-repo" for r in commit_rows))


if __name__ == "__main__":
    unittest.main()
