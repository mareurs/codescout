"""Stage 1b: the events database and joins (Task 6).

Run: ~/work/claude/prompt-engineering/.venv/bin/python -m pytest tests/test_measure_join.py -v
"""
import hashlib
import importlib.util
import json
import os
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


def _attachment_entry(uuid, ts, sid, hook_name="SessionStart", content="shared hook text"):
    return {
        "type": "attachment",
        "uuid": uuid,
        "timestamp": ts,
        "sessionId": sid,
        "attachment": {
            "type": "hook_additional_context",
            "hookName": hook_name,
            "hookEvent": hook_name,
            "content": content,
            "toolUseID": None,
        },
    }


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
        "tool_events_not_codescout": 0,
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
class TokensWrittenOncePerMessageId(unittest.TestCase):
    def test_a_second_line_sharing_a_message_id_is_written_with_zero_tokens(self):
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")

            usage = {"input_tokens": 10, "output_tokens": 20, "cache_creation_input_tokens": 5}
            entries = [
                _entry("u0", "2026-09-20T10:00:00Z", "sid1"),
                {
                    "type": "assistant",
                    "uuid": "a1",
                    "timestamp": "2026-09-20T10:00:01Z",
                    "sessionId": "sid1",
                    "entrypoint": "cli",
                    "message": {"id": "m1", "usage": usage,
                                "content": [{"type": "text", "text": "part one"}]},
                },
                {
                    "type": "assistant",
                    "uuid": "a2",
                    "timestamp": "2026-09-20T10:00:02Z",
                    "sessionId": "sid1",
                    "entrypoint": "cli",
                    "message": {"id": "m1", "usage": usage,
                                "content": [{"type": "tool_use", "id": "t1", "name": "Read",
                                             "input": {}}]},
                },
            ]
            _write_lines(proj / "sid1.jsonl", entries)

            events_db = pathlib.Path(tmp) / "events.db"
            join.build_events(corpus_dir, events_db, excluded_sids=set())

            turns = _table_rows(sqlite3.connect(str(events_db)), "turns")
            by_uuid = {r["uuid"]: r for r in turns}

            # _write_turn must consume _compute_tokens' once-per-message-id dedup via
            # tokens_by_uuid, not re-derive tokens from each line's own usage dict -- a1 (the
            # first line to report message.id "m1") gets the full count and a2 (same id) gets 0.
            self.assertEqual(by_uuid["a1"]["tokens"], 35)
            self.assertEqual(by_uuid["a2"]["tokens"], 0)


class OperatorRuleMarkerDelivery(unittest.TestCase):
    def test_the_operator_rule_marker_parses_to_a_delivery_row(self):
        # Literal text retrieved from a live usage.db row (tool_calls id=143310, 2026-09-26).
        block1 = (
            '{\n  "status": "ok",\n  "wrote_to": "/home/user/work/example",\n'
            '  "abs_path": "/home/user/.claude/settings.json"\n}'
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
        self.assertEqual(d["ts"], "2026-09-26T05:22:48.812Z")
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
            counts = join.build_events(corpus_dir, events_db, excluded_sids=set())

            self.assertEqual(counts["sessions_total"], 3)
            self.assertEqual(
                counts["sessions_kept"] + counts["sessions_excluded"], counts["sessions_total"]
            )

            # I10: name the SPECIFIC kept/excluded copy_ids rather than the old
            # `kept_cids | set(excl.keys()) == all_cids` tautology, which a session
            # mis-classified into the wrong side cannot fail (it just moves sides and the
            # union is unchanged either way).
            sess_list = transcripts.sessions(corpus_dir)
            excl = transcripts.exclusions(sess_list, set())
            self.assertEqual(
                excl,
                {
                    ".claude-sdd/sid-sdk": "sdk-cli",
                    ".claude-sdd/sid-scratch": "scratchpad-project",
                },
            )

            turns = _table_rows(sqlite3.connect(str(events_db)), "turns")
            sids_in_turns = {r["sid"] for r in turns}
            self.assertEqual(sids_in_turns, {".claude-sdd/sid-kept"})


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
class SubagentSiblingEdgeCases(unittest.TestCase):
    def test_a_same_sid_copy_excluded_for_a_non_dedup_reason_is_not_a_sibling(self):
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            sdd_proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")
            kat_proj = _session_dir(corpus_dir, "01-.claude-kat", "p")

            # sdd is excluded for "sdk-cli" -- NOT as a same-sid duplicate of kat -- so it must
            # not be treated as kat's R40 sibling merely for sharing kat's sid.
            _write_lines(sdd_proj / "shared-sid.jsonl", [
                _entry("d1", "2026-09-20T10:00:00Z", "shared-sid", entrypoint="sdk-cli"),
            ])
            _write_lines(kat_proj / "shared-sid.jsonl", [
                _entry("k1", "2026-09-20T10:00:00Z", "shared-sid"),
            ])

            # sdd's UNIQUE subagent file must NOT be unioned into kat's turns.
            _write_lines(sdd_proj / "shared-sid" / "subagents" / "sub-x.jsonl", [
                _entry("sx1", "2026-09-20T11:00:00Z", "shared-sid"),
            ])

            events_db = pathlib.Path(tmp) / "events.db"
            join.build_events(corpus_dir, events_db, excluded_sids=set())

            turns = _table_rows(sqlite3.connect(str(events_db)), "turns")
            sub_uuids = {r["uuid"] for r in turns if r["agent_path"]}
            self.assertEqual(sub_uuids, set())

    def test_a_divergent_duplicate_sibling_is_still_unioned(self):
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            sdd_proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")
            kat_proj = _session_dir(corpus_dir, "01-.claude-kat", "p")

            # kat: 5-entry timeline -> the longer copy, so it is the keeper.
            kat_lines = [_entry(f"u{i}", f"2026-09-20T10:00:0{i}Z", "div-sid") for i in range(5)]
            # sdd shares kat's first 3 entries but its 4th DIVERGES (different uuid/ts) rather
            # than simply stopping early, so its full timeline is not a PREFIX of kat's --
            # exclusions() labels it "divergent-duplicate-of:", not "duplicate-prefix-of:".
            sdd_lines = kat_lines[:3] + [_entry("vX", "2026-09-20T10:00:09Z", "div-sid")]
            _write_lines(kat_proj / "div-sid.jsonl", kat_lines)
            _write_lines(sdd_proj / "div-sid.jsonl", sdd_lines)

            # sdd's UNIQUE subagent file -- a divergent duplicate must still be unioned in (R40).
            _write_lines(sdd_proj / "div-sid" / "subagents" / "sub-d.jsonl", [
                _entry("sd1", "2026-09-20T11:00:00Z", "div-sid"),
            ])

            events_db = pathlib.Path(tmp) / "events.db"
            counts = join.build_events(corpus_dir, events_db, excluded_sids=set())

            turns = _table_rows(sqlite3.connect(str(events_db)), "turns")
            sub_uuids = {r["uuid"] for r in turns if r["agent_path"]}
            self.assertIn("sd1", sub_uuids)
            self.assertEqual(counts["subagent_files_unioned"], 1)

    def test_the_keepers_own_subagent_file_wins_a_name_collision(self):
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            sdd_proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")
            kat_proj = _session_dir(corpus_dir, "01-.claude-kat", "p")

            # kat is the superset copy -> keeper; sdd is duplicate-prefix-of: kat (same
            # construction as SubagentUnion's own fixture).
            shared = [_entry(f"u{i}", f"2026-09-20T10:00:0{i}Z", "coll-sid") for i in range(5)]
            kat_lines = shared + [_entry("u5", "2026-09-20T10:00:05Z", "coll-sid")]
            _write_lines(sdd_proj / "coll-sid.jsonl", shared)
            _write_lines(kat_proj / "coll-sid.jsonl", kat_lines)

            # Both keeper and sibling carry a subagent file of the SAME NAME but different
            # content -- the keeper's own file must win.
            _write_lines(kat_proj / "coll-sid" / "subagents" / "sub-same.jsonl", [
                _entry("keeper-only", "2026-09-20T11:00:00Z", "coll-sid"),
            ])
            _write_lines(sdd_proj / "coll-sid" / "subagents" / "sub-same.jsonl", [
                _entry("sibling-only", "2026-09-20T11:00:00Z", "coll-sid"),
            ])

            events_db = pathlib.Path(tmp) / "events.db"
            join.build_events(corpus_dir, events_db, excluded_sids=set())

            turns = _table_rows(sqlite3.connect(str(events_db)), "turns")
            sub_uuids = {r["uuid"] for r in turns if r["agent_path"]}
            self.assertIn("keeper-only", sub_uuids)
            self.assertNotIn("sibling-only", sub_uuids)


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

        join._join_tool_events(conn, candidates, usage_rows, counts, {}, {})
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

        join._join_tool_events(conn, candidates, usage_rows, counts, {}, {})
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

        join._join_tool_events(conn, candidates, usage_rows, counts, {}, {})
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

        join._join_tool_events(conn, candidates, usage_rows, counts, {}, {})
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

        join._join_tool_events(conn, candidates, usage_rows, counts, {}, {})
        conn.commit()
        rows = _table_rows(conn, "tool_events")

        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]["join_method"], "none")
    def test_time_window_boundary_is_inclusive_at_exactly_120_seconds(self):
        conn = sqlite3.connect(":memory:")
        join._create_schema(conn)
        counts = _fresh_counts()

        candidates = [_candidate(
            "p/sid1", "sid1", "mcp__codescout__grep", {"pattern": "x"}, _iso(BASE),
        )]
        usage_rows = [
            _usage_row("db:1", "grep", "sid1", json.dumps({"pattern": "x"}),
                       _sql_ts(BASE + timedelta(seconds=join.JOIN_WINDOW_SECONDS))),
        ]

        join._join_tool_events(conn, candidates, usage_rows, counts, {}, {})
        conn.commit()
        rows = _table_rows(conn, "tool_events")

        # R42 window: a candidate exactly JOIN_WINDOW_SECONDS away is still IN the window --
        # only `diff > JOIN_WINDOW_SECONDS` excludes. `>=` at this boundary would wrongly
        # exclude it and fall through to "none".
        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]["join_method"], "heuristic")
        self.assertEqual(rows[0]["usage_row_id"], "db:1")

    def test_a_null_usage_agent_id_does_not_gate_matching(self):
        conn = sqlite3.connect(":memory:")
        join._create_schema(conn)
        counts = _fresh_counts()

        candidates = [_candidate(
            "p/sid1", "sid1", "mcp__codescout__grep", {"pattern": "x"},
            _iso(BASE), agent_id="agentX",
        )]
        # usage.db's agent_id is NULL -- the hard key applies only when non-NULL, so this row
        # must still be eligible for a candidate that DOES carry an agent_id.
        usage_rows = [
            _usage_row("db:1", "grep", "sid1", json.dumps({"pattern": "x"}),
                       _sql_ts(BASE), agent_id=None),
        ]

        join._join_tool_events(conn, candidates, usage_rows, counts, {}, {})
        conn.commit()
        rows = _table_rows(conn, "tool_events")

        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]["join_method"], "heuristic")
        self.assertEqual(rows[0]["usage_row_id"], "db:1")


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

        join._join_tool_events(conn, candidates, usage_rows, counts, {}, {})
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

        join._join_tool_events(conn, candidates, [], counts, {}, {})
        conn.commit()
        rows = _table_rows(conn, "tool_events")

        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]["join_method"], "not_codescout")
        self.assertEqual(counts["tool_events_none"], 0)
        self.assertEqual(counts["tool_events_not_codescout"], 1)
        self.assertEqual(counts["tool_events_total"], 1)
class ExactJoinClaimsItsRow(unittest.TestCase):
    def test_a_second_exact_candidate_does_not_reclaim_an_already_exact_matched_row(self):
        conn = sqlite3.connect(":memory:")
        join._create_schema(conn)
        counts = _fresh_counts()

        # Both candidates share the SAME tool_use_id, which maps to exactly ONE usage row.
        # The first must claim db:1 as exact; the second must NOT re-match it as exact too.
        candidates = [
            _candidate("p/sid1", "sid1", "mcp__codescout__grep", {"pattern": "x"},
                       _iso(BASE), tool_use_id="tu1"),
            _candidate("p/sid1", "sid1", "mcp__codescout__grep", {"pattern": "y"},
                       _iso(BASE + timedelta(seconds=1)), tool_use_id="tu1"),
        ]
        usage_rows = [
            _usage_row("db:1", "grep", "sid1", json.dumps({"pattern": "x"}),
                       _sql_ts(BASE), tool_use_id="tu1"),
        ]

        join._join_tool_events(conn, candidates, usage_rows, counts, {}, {})
        conn.commit()
        rows = _table_rows(conn, "tool_events")

        exact_rows = [r for r in rows if r["join_method"] == "exact"]
        self.assertEqual(len(exact_rows), 1)
        self.assertEqual(counts["tool_events_exact"], 1)


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
class RetainedBoundsSinceIsApplied(unittest.TestCase):
    def test_a_start_bound_excludes_a_commit_before_it(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo = pathlib.Path(tmp) / "repo"
            repo.mkdir(parents=True)

            def run(*args, env=None):
                subprocess.run(
                    ["git", *args], cwd=str(repo), check=True, capture_output=True,
                    text=True, env=env,
                )

            run("init", "-q")
            run("config", "user.name", "t")
            run("config", "user.email", "t@t")

            base_env = dict(os.environ)
            (repo / "a.txt").write_text("1")
            run("add", "a.txt")
            env1 = dict(base_env, GIT_AUTHOR_DATE="2026-01-01T00:00:00Z",
                        GIT_COMMITTER_DATE="2026-01-01T00:00:00Z")
            run("commit", "-q", "-m", "early commit", env=env1)

            (repo / "b.txt").write_text("1")
            run("add", "b.txt")
            env2 = dict(base_env, GIT_AUTHOR_DATE="2026-06-01T00:00:00Z",
                        GIT_COMMITTER_DATE="2026-06-01T00:00:00Z")
            run("commit", "-q", "-m", "late commit", env=env2)

            head = subprocess.run(
                ["git", "rev-parse", "HEAD"], cwd=str(repo), check=True,
                capture_output=True, text=True,
            ).stdout.strip()

            # R41: --since must actually reach git log, or both commits come back.
            commits = join._run_git_log(repo, head, "2026-03-01T00:00:00Z", None)

            self.assertEqual(len(commits), 1)
            self.assertEqual(commits[0]["subject"], "late commit")


class KindClassificationUnit(unittest.TestCase):
    def test_precedence_interrupt_over_prompt_over_delegation(self):
        entry = _entry("u1", "2026-09-20T10:00:00Z", "sid1")
        self.assertEqual(join._entry_kind(entry, True, True, True), "interrupt")
        self.assertEqual(join._entry_kind(entry, True, False, True), "prompt")
        self.assertEqual(join._entry_kind(entry, False, False, True), "delegation")

    def test_tool_result_and_meta_fall_through_to_structural_kinds(self):
        tool_result_entry = _entry(
            "u2", "2026-09-20T10:00:01Z", "sid1",
            content=[{"type": "tool_result", "content": "ok"}],
        )
        self.assertEqual(join._entry_kind(tool_result_entry, False, False, False), "tool_result")

        meta_entry = _entry("u3", "2026-09-20T10:00:02Z", "sid1", is_meta=True)
        self.assertEqual(join._entry_kind(meta_entry, False, False, False), "meta")

    def test_assistant_tool_use_thinking_and_text(self):
        tool_use_entry = {"type": "assistant", "message": {"content": [
            {"type": "tool_use", "id": "t1", "name": "Read", "input": {}}]}}
        self.assertEqual(join._entry_kind(tool_use_entry, False, False, False), "tool_use")

        thinking_entry = {"type": "assistant", "message": {"content": [
            {"type": "thinking", "thinking": "hmm"}]}}
        self.assertIsNone(transcripts._message_text(thinking_entry))
        self.assertEqual(
            join._entry_kind(thinking_entry, False, False, False), "assistant_thinking"
        )

        text_entry = {"type": "assistant", "message": {"content": [
            {"type": "text", "text": "hello"}]}}
        self.assertEqual(join._entry_kind(text_entry, False, False, False), "assistant_text")


class AttributionUnit(unittest.TestCase):
    def test_top_level_never_produces_delegation(self):
        entries = [
            _entry("u1", "2026-09-20T10:00:00Z", "sid1", content="do the thing"),
            _entry("u2", "2026-09-20T10:00:01Z", "sid1", content="[Request interrupted by user]"),
        ]
        prompt_uuids, interrupt_uuids, delegation_uuids = join._classify_uuids(entries, False)
        self.assertEqual(prompt_uuids, {"u1"})
        self.assertEqual(interrupt_uuids, {"u2"})
        self.assertEqual(delegation_uuids, set())

    def test_subagent_never_produces_prompt(self):
        entries = [
            _entry("u1", "2026-09-20T10:00:00Z", "sid1", content="the parent's brief"),
            _entry("u2", "2026-09-20T10:00:01Z", "sid1", content="[Request interrupted by user]"),
            _entry("u3", "2026-09-20T10:00:02Z", "sid1",
                   content=[{"type": "tool_result", "content": "ok"}]),
            _entry("u4", "2026-09-20T10:00:03Z", "sid1", is_meta=True),
        ]
        prompt_uuids, interrupt_uuids, delegation_uuids = join._classify_uuids(entries, True)
        self.assertEqual(prompt_uuids, set())
        self.assertEqual(interrupt_uuids, {"u2"})
        self.assertEqual(delegation_uuids, {"u1"})


class SubagentClassification(unittest.TestCase):
    def test_subagent_entries_never_yield_prompt_rows(self):
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")

            _write_lines(proj / "sid1.jsonl", [
                _entry("top1", "2026-09-20T10:00:00Z", "sid1", content="a real top-level prompt"),
            ])
            _write_lines(proj / "sid1" / "subagents" / "sub.jsonl", [
                _entry("sub1", "2026-09-20T10:01:00Z", "sid1", content="the parent's brief"),
                _entry("sub2", "2026-09-20T10:01:01Z", "sid1",
                       content="[Request interrupted by user]"),
                _entry("sub3", "2026-09-20T10:01:02Z", "sid1",
                       content=[{"type": "tool_result", "content": "ok"}]),
            ])

            events_db = pathlib.Path(tmp) / "events.db"
            join.build_events(corpus_dir, events_db, excluded_sids=set())

            turns = _table_rows(sqlite3.connect(str(events_db)), "turns")
            by_uuid = {r["uuid"]: r for r in turns}

            self.assertEqual(by_uuid["top1"]["kind"], "prompt")
            self.assertEqual(by_uuid["sub1"]["kind"], "delegation")
            self.assertEqual(by_uuid["sub2"]["kind"], "interrupt")
            self.assertEqual(by_uuid["sub3"]["kind"], "tool_result")

            subagent_rows = [r for r in turns if r["agent_path"]]
            self.assertEqual(sum(1 for r in subagent_rows if r["kind"] == "prompt"), 0)


class SpecExclusionDefault(unittest.TestCase):
    def test_default_build_excludes_the_spec_session_explicit_set_keeps_it(self):
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")
            spec_sid = next(iter(join.SPEC_EXCLUDED_SIDS))
            _write_lines(proj / f"{spec_sid}.jsonl", [
                _entry("k1", "2026-09-20T10:00:00Z", spec_sid),
            ])

            events_db = pathlib.Path(tmp) / "events-default.db"
            join.build_events(corpus_dir, events_db)
            turns = _table_rows(sqlite3.connect(str(events_db)), "turns")
            self.assertEqual(len(turns), 0)

            events_db2 = pathlib.Path(tmp) / "events-explicit.db"
            join.build_events(corpus_dir, events_db2, excluded_sids=set())
            turns2 = _table_rows(sqlite3.connect(str(events_db2)), "turns")
            self.assertEqual({r["uuid"] for r in turns2}, {"k1"})


class ForkGlobalUuidGate(unittest.TestCase):
    def test_fork_shared_attachment_and_subagent_uuids_are_each_written_once(self):
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")

            def _shared_top_level_entries(sid):
                return [
                    _attachment_entry("att1", "2026-09-20T10:00:00Z", sid),
                    _entry("u2", "2026-09-20T10:00:01Z", sid),
                    _entry("u3", "2026-09-20T10:00:02Z", sid),
                    _entry("u4", "2026-09-20T10:00:03Z", sid),
                    _entry("u5", "2026-09-20T10:00:04Z", sid),
                ]

            _write_lines(proj / "fork-a.jsonl", _shared_top_level_entries("fork-a"))
            _write_lines(proj / "fork-b.jsonl", _shared_top_level_entries("fork-b"))

            sub_entries = [
                _entry("subu1", "2026-09-20T10:05:00Z", "fork-a"),
                _entry("subu2", "2026-09-20T10:05:01Z", "fork-a"),
            ]
            _write_lines(proj / "fork-a" / "subagents" / "sub.jsonl", sub_entries)
            _write_lines(proj / "fork-b" / "subagents" / "sub.jsonl", sub_entries)

            events_db = pathlib.Path(tmp) / "events.db"
            counts = join.build_events(corpus_dir, events_db, excluded_sids=set())

            conn = sqlite3.connect(str(events_db))
            turns = _table_rows(conn, "turns")
            deliveries = _table_rows(conn, "deliveries")

            # I4: every uuid appears exactly once in turns. att1 is an attachment -- it never
            # gets a turns row at all (R45: attachments emit a delivery, not a turn), so it is
            # deliberately absent here and checked via hook_deliveries below instead.
            # deliveries/tool_events carry no uuid column at all, so uniqueness there is
            # checked by content (source + one row) below, matching the reviewer's own
            # probe_live5 T1/T4 methodology.
            uuid_counts = {}
            for r in turns:
                uuid_counts[r["uuid"]] = uuid_counts.get(r["uuid"], 0) + 1
            self.assertEqual({u: n for u, n in uuid_counts.items() if n > 1}, {})
            self.assertEqual(
                {r["uuid"] for r in turns},
                {"u2", "u3", "u4", "u5", "subu1", "subu2"},
            )

            hook_deliveries = [d for d in deliveries if d["source"] == "transcript_hook"]
            self.assertEqual(len(hook_deliveries), 1)

            self.assertEqual(counts["top_level_attribution_skipped"], 5)
            self.assertEqual(counts["subagent_duplicate_uuids_skipped"], 2)


class ForkPrefixHeuristicJoin(unittest.TestCase):
    def test_a_fork_candidate_joins_via_the_forked_from_targets_bare_sid(self):
        conn = sqlite3.connect(":memory:")
        join._create_schema(conn)
        counts = _fresh_counts()

        candidates = [_candidate(
            "p/fork-b", "fork-b", "mcp__codescout__grep", {"pattern": "x"}, _iso(BASE),
        )]
        # The usage.db row was recorded under fork-a's bare sid -- before the fork split off
        # fork-b, per R49's own rationale ("a fork's shared prefix was recorded under the
        # original").
        usage_rows = [
            _usage_row("db:1", "grep", "fork-a", json.dumps({"pattern": "x"}), _sql_ts(BASE)),
        ]
        relations_map = {"p/fork-b": "fork-of:p/fork-a"}
        cid_to_bare_sid = {"p/fork-a": "fork-a", "p/fork-b": "fork-b"}

        join._join_tool_events(
            conn, candidates, usage_rows, counts, relations_map, cid_to_bare_sid,
        )
        conn.commit()
        rows = _table_rows(conn, "tool_events")

        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]["join_method"], "heuristic")
        self.assertEqual(rows[0]["usage_row_id"], "db:1")

    def test_a_non_fork_candidate_does_not_admit_an_unrelated_bare_sid(self):
        conn = sqlite3.connect(":memory:")
        join._create_schema(conn)
        counts = _fresh_counts()

        candidates = [_candidate(
            "p/plain", "plain", "mcp__codescout__grep", {"pattern": "x"}, _iso(BASE),
        )]
        usage_rows = [
            _usage_row("db:1", "grep", "fork-a", json.dumps({"pattern": "x"}), _sql_ts(BASE)),
        ]
        # No relation recorded for "p/plain" -- it must not reach fork-a's bucket.
        join._join_tool_events(conn, candidates, usage_rows, counts, {}, {"p/fork-a": "fork-a"})
        conn.commit()
        rows = _table_rows(conn, "tool_events")

        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]["join_method"], "none")


class RealFormatHeuristicJoin(unittest.TestCase):
    """Fix round 2: a build_events-level test using REAL-shaped values end to end, rather than
    the hand-built _candidate()/_usage_row() dicts every heuristic-join test above uses (which
    call _join_tool_events directly and never exercise _tool_use_candidates_from_entry, _fmt_ts,
    or _load_usage_rows). A real UUID-shaped sessionId, an ISO '...Z' transcript timestamp with
    milliseconds, an 'mcp__codescout__<tool>' name, a present agentId, and a usage.db row whose
    started_at is real usage.db shape -- 'YYYY-MM-DD HH:MM:SS.mmm', no zone -- exercise the full
    transcript-parse + usage.db-load + join pipeline the coordinator named as the suspect
    surface (candidate keying/bucketing, R50's ts normalization, R49's fork admission) that the
    round-1 50-test suite + 36 killed mutants never covered with real-format fixtures.

    Measured fix round 2 (2026-09-27): this test PASSES against HEAD (1e32b228) unmodified --
    see probes/task6-fix2-green.txt. It is not a regression test for a real bug (the root-cause
    probe in probes/task6-fix2-rootcause.txt found none); it is new coverage for a code path a
    real-corpus reconciliation showed was already correct. Its discriminating power is
    demonstrated by mutant M38 in probes/task6-fix2-mutants.txt (swap the bucket key's bare_sid
    for cid in _join_tool_events), which this test kills -- see probes/task6-fix2-red.txt.
    """

    REAL_SID = "8f3a1c2d-4e5b-4a6c-9d7e-1a2b3c4d5e6f"

    def test_a_real_shaped_tool_use_joins_heuristically_through_build_events(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = pathlib.Path(tmp)
            corpus_dir = _make_corpus(tmp_path)
            proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")

            tool_input = {"pattern": "TODO", "glob": "*.rs"}
            entries = [
                _entry("u1", "2026-09-26T10:00:00.000Z", self.REAL_SID),
                {
                    "type": "assistant",
                    "uuid": "a1",
                    "timestamp": "2026-09-26T10:00:01.500Z",
                    "sessionId": self.REAL_SID,
                    "agentId": "agent-real-1",
                    "entrypoint": "cli",
                    "message": {
                        "id": "m1",
                        "content": [
                            {"type": "tool_use", "id": "toolu_01AbC",
                             "name": "mcp__codescout__grep", "input": tool_input},
                        ],
                    },
                },
            ]
            _write_lines(proj / f"{self.REAL_SID}.jsonl", entries)

            usage_dbs_dir = corpus_dir / "usage_dbs"
            usage_dbs_dir.mkdir(parents=True)
            db_path = usage_dbs_dir / "u1.db"
            conn = sqlite3.connect(str(db_path))
            conn.execute(
                "CREATE TABLE tool_calls (id INTEGER PRIMARY KEY, tool_name TEXT, "
                "called_at TEXT, started_at TEXT, cc_session_id TEXT, agent_id TEXT, "
                "input_json TEXT, output_json TEXT, deliveries_json TEXT, tool_use_id TEXT)"
            )
            # started_at: real usage.db shape -- "YYYY-MM-DD HH:MM:SS.mmm", no zone -- ~20ms
            # after the transcript-side tool_use timestamp above, well inside the 120s window.
            conn.execute(
                "INSERT INTO tool_calls (tool_name, called_at, started_at, cc_session_id, "
                "agent_id, input_json, output_json, deliveries_json, tool_use_id) VALUES "
                "('grep', '2026-09-26 10:00:01.520', '2026-09-26 10:00:01.520', ?, "
                "'agent-real-1', ?, NULL, NULL, NULL)",
                (self.REAL_SID, json.dumps(tool_input)),
            )
            conn.commit()
            conn.close()

            events_db = tmp_path / "events.db"
            counts = join.build_events(corpus_dir, events_db, excluded_sids=set())

            tool_events = _table_rows(sqlite3.connect(str(events_db)), "tool_events")
            grep_events = [r for r in tool_events if r["name"] == "mcp__codescout__grep"]
            self.assertEqual(len(grep_events), 1)
            self.assertEqual(grep_events[0]["join_method"], "heuristic")
            self.assertEqual(counts["tool_events_heuristic"], 1)
            self.assertEqual(counts["tool_events_exact"], 0)
            self.assertEqual(counts["tool_events_none"], 0)



class EventsMetaPersistence(unittest.TestCase):
    def test_every_build_counter_is_persisted_to_events_meta(self):
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")
            _write_lines(proj / "sid1.jsonl", [
                _entry("k1", "2026-09-20T10:00:00Z", "sid1"),
            ])

            events_db = pathlib.Path(tmp) / "events.db"
            counts = join.build_events(corpus_dir, events_db, excluded_sids=set())

            conn = sqlite3.connect(str(events_db))
            meta_rows = _table_rows(conn, "events_meta")
            meta = {r["key"]: r["value"] for r in meta_rows}

            for key, value in counts.items():
                self.assertIn(key, meta)
                self.assertEqual(str(meta[key]), str(value))


class NoSilentRerun(unittest.TestCase):
    def test_build_events_refuses_an_existing_db_path(self):
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")
            _write_lines(proj / "sid1.jsonl", [
                _entry("k1", "2026-09-20T10:00:00Z", "sid1"),
            ])

            events_db = pathlib.Path(tmp) / "events.db"
            join.build_events(corpus_dir, events_db, excluded_sids=set())

            with self.assertRaises(FileExistsError):
                join.build_events(corpus_dir, events_db, excluded_sids=set())

            # I9: the refused re-run attempt must not have doubled any rows.
            turns = _table_rows(sqlite3.connect(str(events_db)), "turns")
            self.assertEqual(len(turns), 1)


class SubagentHookDeliveries(unittest.TestCase):
    def test_a_subagent_attachment_entry_emits_a_delivery_row_under_the_keeper(self):
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")

            _write_lines(proj / "sid1.jsonl", [
                _entry("u1", "2026-09-20T10:00:00Z", "sid1"),
            ])
            _write_lines(proj / "sid1" / "subagents" / "sub-a.jsonl", [
                _attachment_entry("hu1", "2026-09-20T11:00:00Z", "sid1"),
            ])

            events_db = pathlib.Path(tmp) / "events.db"
            join.build_events(corpus_dir, events_db, excluded_sids=set())

            deliveries = _table_rows(sqlite3.connect(str(events_db)), "deliveries")
            hook_deliveries = [d for d in deliveries if d["source"] == "transcript_hook"]
            self.assertEqual(len(hook_deliveries), 1)
            self.assertEqual(hook_deliveries[0]["engine_or_hook"], "SessionStart")

class ThinkingOnlyLines(unittest.TestCase):
    def test_a_thinking_only_assistant_line_is_kind_assistant_thinking_with_null_text(self):
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")

            entries = [
                _entry("u1", "2026-09-20T10:00:00Z", "sid1"),
                {
                    "type": "assistant",
                    "uuid": "a1",
                    "timestamp": "2026-09-20T10:00:01Z",
                    "sessionId": "sid1",
                    "entrypoint": "cli",
                    "message": {"content": [
                        {"type": "thinking", "thinking": "secret reasoning"}
                    ]},
                },
            ]
            _write_lines(proj / "sid1.jsonl", entries)

            events_db = pathlib.Path(tmp) / "events.db"
            join.build_events(corpus_dir, events_db, excluded_sids=set())

            turns = _table_rows(sqlite3.connect(str(events_db)), "turns")
            by_uuid = {r["uuid"]: r for r in turns}

            self.assertEqual(by_uuid["a1"]["kind"], "assistant_thinking")
            self.assertIsNone(by_uuid["a1"]["text"])
            for r in turns:
                self.assertNotIn("secret reasoning", r["text"] or "")


class TsNormalization(unittest.TestCase):
    def test_fmt_ts_normalizes_all_three_source_shapes_to_the_same_form(self):
        self.assertEqual(join._fmt_ts("2026-09-26 05:22:48.812"), "2026-09-26T05:22:48.812Z")
        self.assertEqual(join._fmt_ts("2026-09-26T05:22:48.812Z"), "2026-09-26T05:22:48.812Z")
        self.assertEqual(join._fmt_ts("2026-09-26T07:22:48+02:00"), "2026-09-26T05:22:48.000Z")

    def test_fmt_ts_passes_through_falsy_and_unparseable_input_unchanged(self):
        self.assertIsNone(join._fmt_ts(None))
        self.assertEqual(join._fmt_ts(""), "")
        self.assertEqual(join._fmt_ts("not-a-timestamp"), "not-a-timestamp")


class CommitsGitFailure(unittest.TestCase):
    def test_run_git_log_returns_none_on_a_git_failure(self):
        with tempfile.TemporaryDirectory() as tmp:
            not_a_repo = pathlib.Path(tmp) / "not-a-repo"
            not_a_repo.mkdir()

            commits = join._run_git_log(not_a_repo, "deadbeef", None, None)
            self.assertIsNone(commits)

    def test_build_events_counts_the_git_failure_separately_from_zero_commits(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = pathlib.Path(tmp)
            not_a_repo = tmp_path / "not-a-repo"
            not_a_repo.mkdir()

            corpus_dir = _make_corpus(tmp_path, repos={"broken-repo": "deadbeef"})
            proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")
            _write_lines(proj / "sid1.jsonl", [_entry("k1", "2026-09-20T10:00:00Z", "sid1")])

            events_db = tmp_path / "events.db"
            counts = join.build_events(
                corpus_dir, events_db, repo_paths={"broken-repo": not_a_repo},
                excluded_sids=set(),
            )

            self.assertEqual(counts["commits_git_failures"], 1)
            self.assertEqual(counts["commits_total"], 0)
            self.assertEqual(counts["commits_repos_missing"], 0)


class UnmappedUsageSession(unittest.TestCase):
    def test_a_usage_row_for_an_unmapped_session_is_dropped_and_counted(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = pathlib.Path(tmp)
            corpus_dir = _make_corpus(tmp_path)
            proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")
            _write_lines(proj / "sid1.jsonl", [_entry("k1", "2026-09-20T10:00:00Z", "sid1")])

            usage_dbs_dir = corpus_dir / "usage_dbs"
            usage_dbs_dir.mkdir(parents=True)
            db_path = usage_dbs_dir / "u1.db"
            conn = sqlite3.connect(str(db_path))
            conn.execute(
                "CREATE TABLE tool_calls (id INTEGER PRIMARY KEY, tool_name TEXT, "
                "called_at TEXT, started_at TEXT, cc_session_id TEXT, agent_id TEXT, "
                "input_json TEXT, output_json TEXT, deliveries_json TEXT, tool_use_id TEXT)"
            )
            conn.execute(
                "INSERT INTO tool_calls (tool_name, called_at, started_at, cc_session_id, "
                "input_json, output_json, deliveries_json, tool_use_id) VALUES "
                "('grep', ?, ?, 'not-a-kept-session', '{}', NULL, NULL, NULL)",
                (_sql_ts(BASE), _sql_ts(BASE)),
            )
            conn.commit()
            conn.close()

            events_db = tmp_path / "events.db"
            counts = join.build_events(corpus_dir, events_db, excluded_sids=set())

            self.assertEqual(counts["deliveries_unmapped_session"], 1)
            deliveries = _table_rows(sqlite3.connect(str(events_db)), "deliveries")
            self.assertEqual(len(deliveries), 0)


class DeliveriesJsonHandling(unittest.TestCase):
    def test_empty_list_deliveries_json_yields_zero_rows_and_never_falls_back(self):
        row = {
            "tool_name": "edit_file",
            "called_at": _sql_ts(BASE),
            "cc_session_id": "sid1",
            "output_json": json.dumps([{"type": "text", "text": (
                "<!-- operator-rule OP-9 -- would be a hit if output_json were scanned -->"
            )}]),
            "deliveries_json": "[]",
            "tool_use_id": None,
        }
        self.assertEqual(join._deliveries_from_usage_row(row), [])

    def test_real_deliveries_json_is_used_over_output_json(self):
        row = {
            "tool_name": "doc",
            "called_at": _sql_ts(BASE),
            "cc_session_id": "sid1",
            "output_json": json.dumps([{"type": "text", "text": (
                "<!-- operator-rule OP-1 -- must be ignored, deliveries_json wins -->"
            )}]),
            "deliveries_json": json.dumps([
                {"ledger_keys": ["T-9"], "blocks": [{"sha256": "abc123", "bytes": 42}]},
            ]),
            "tool_use_id": None,
        }
        deliveries = join._deliveries_from_usage_row(row)
        sources = {d["source"] for d in deliveries}
        self.assertEqual(sources, {"usage_deliveries_json"})
        keys = [d["key"] for d in deliveries if d["key"] is not None]
        self.assertEqual(keys, ["T-9"])
        shas = [d["sha256"] for d in deliveries if d["sha256"] is not None]
        self.assertEqual(shas, ["abc123"])
    def test_empty_string_deliveries_json_yields_zero_rows_and_never_falls_back(self):
        # R50(d): gated on presence (`is not None`), never truthiness -- an empty STRING is a
        # falsy-but-present value distinct from '[]' (a present, valid, empty list) and from
        # NULL (truly absent). Either non-NULL form must fail closed rather than fall back to
        # output_json.
        row = {
            "tool_name": "get_guide",
            "called_at": _sql_ts(BASE),
            "cc_session_id": "sid1",
            "output_json": json.dumps([{"type": "text", "text": (
                "<!-- auto-injected get_guide('librarian') -->\n"
                "body\n"
                "<!-- end auto-injected get_guide('librarian') -->"
            )}]),
            "deliveries_json": "",
            "tool_use_id": None,
        }
        self.assertEqual(join._deliveries_from_usage_row(row), [])


class GuideMarkerAnchoring(unittest.TestCase):
    def test_a_real_open_and_close_block_yields_one_delivery_not_two(self):
        block = (
            "<!-- auto-injected get_guide('librarian') -->\n"
            "some injected guidance text here\n"
            "<!-- end auto-injected get_guide('librarian') -->"
        )
        row = {
            "tool_name": "get_guide",
            "called_at": _sql_ts(BASE),
            "cc_session_id": "sid1",
            "output_json": json.dumps([{"type": "text", "text": block}]),
            "deliveries_json": None,
            "tool_use_id": None,
        }
        deliveries = join._deliveries_from_usage_row(row)
        self.assertEqual(len(deliveries), 1)
        self.assertEqual(deliveries[0]["engine_or_hook"], "get_guide")
        self.assertEqual(deliveries[0]["key"], "librarian")

    def test_a_marker_quoted_mid_line_is_not_counted(self):
        block = "grep hit: text before <!-- auto-injected get_guide('librarian') --> after"
        row = {
            "tool_name": "grep",
            "called_at": _sql_ts(BASE),
            "cc_session_id": "sid1",
            "output_json": json.dumps([{"type": "text", "text": block}]),
            "deliveries_json": None,
            "tool_use_id": None,
        }
        deliveries = join._deliveries_from_usage_row(row)
        self.assertEqual(deliveries, [])

    def test_the_closing_marker_alone_is_not_counted(self):
        block = "<!-- end auto-injected get_guide('librarian') -->"
        row = {
            "tool_name": "get_guide",
            "called_at": _sql_ts(BASE),
            "cc_session_id": "sid1",
            "output_json": json.dumps([{"type": "text", "text": block}]),
            "deliveries_json": None,
            "tool_use_id": None,
        }
        deliveries = join._deliveries_from_usage_row(row)
        self.assertEqual(deliveries, [])

    def test_an_operator_rule_open_and_close_block_yields_one_delivery_not_two(self):
        block = (
            "<!-- operator-rule OP-7 -- some note -->\n"
            "operator rule body text\n"
            "<!-- end operator-rule OP-7 -->"
        )
        row = {
            "tool_name": "edit_file",
            "called_at": _sql_ts(BASE),
            "cc_session_id": "sid1",
            "output_json": json.dumps([{"type": "text", "text": block}]),
            "deliveries_json": None,
            "tool_use_id": None,
        }
        deliveries = join._deliveries_from_usage_row(row)
        self.assertEqual(len(deliveries), 1)
        self.assertEqual(deliveries[0]["engine_or_hook"], "operator-rule")
        self.assertEqual(deliveries[0]["key"], "OP-7")


if __name__ == "__main__":
    unittest.main()
