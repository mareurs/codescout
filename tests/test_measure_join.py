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


def _attachment_entry(uuid, ts, sid, hook_name="SessionStart", content="shared hook text",
                       tool_use_id=None):
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
            "toolUseID": tool_use_id,
        },
    }
def _hook_success_entry(uuid, ts, sid, hook_name="SessionStart", content="shared hook text",
                         tool_use_id=None):
    # R53: the `hook_success` twin of `_attachment_entry` -- real-shaped, the injected text
    # arrives via `stdout`'s JSON `hookSpecificOutput.additionalContext`, not `content`.
    return {
        "type": "attachment",
        "uuid": uuid,
        "timestamp": ts,
        "sessionId": sid,
        "attachment": {
            "type": "hook_success",
            "hookName": hook_name,
            "hookEvent": hook_name,
            "stdout": json.dumps({"hookSpecificOutput": {"additionalContext": content}}),
            "toolUseID": tool_use_id,
        },
    }


# --- R56 real-shaped hook fixtures ---------------------------------------------------------
# Key sets and value forms copied from real attachments on the 2026-09-27 snapshot (texts
# sanitized and shortened). A hook_success carries every key below. A hac carries only
# type/content/hookName/toolUseID/hookEvent, and its `content` is ALWAYS a list, with 1 element
# (Pre/PostToolUse, SubagentStart, some SessionStart) or 2 (a merged SessionStart hac).


def _sha(text):
    return hashlib.sha256(text.encode()).hexdigest()


def _real_hook_success(uuid, ts, sid, event, hook_name, tool_use_id, text, *, plain=False,
                       agent_id=None):
    """`plain=False`: a JSON-stdout hook, which carries the text in stdout's
    hookSpecificOutput.additionalContext, with `content` "". `plain=True`: a plain-stdout hook
    (buddy's run.mjs), which carries the text verbatim in `content`, with stdout adding a
    trailing newline. `text=None`: a hook that printed `{}` and injected nothing.
    """
    if text is None:
        content, stdout = "", "{}"
    elif plain:
        content, stdout = text, text + "\n"
    else:
        content = ""
        stdout = json.dumps({"hookSpecificOutput": {"hookEventName": event,
                                                    "additionalContext": text}})
    e = {
        "type": "attachment", "uuid": uuid, "timestamp": ts, "sessionId": sid,
        "isSidechain": agent_id is not None,
        "attachment": {
            "type": "hook_success", "hookName": hook_name, "toolUseID": tool_use_id,
            "hookEvent": event, "content": content, "stdout": stdout, "stderr": "",
            "exitCode": 0, "command": "node ${CLAUDE_PLUGIN_ROOT}/hooks/hook.mjs",
            "durationMs": 57,
        },
    }
    if agent_id is not None:
        e["agentId"] = agent_id
    return e


def _real_hac(uuid, ts, sid, event, hook_name, tool_use_id, elements, *, agent_id=None):
    e = {
        "type": "attachment", "uuid": uuid, "timestamp": ts, "sessionId": sid,
        "isSidechain": agent_id is not None,
        "attachment": {
            "type": "hook_additional_context", "content": list(elements),
            "hookName": hook_name, "toolUseID": tool_use_id, "hookEvent": event,
        },
    }
    if agent_id is not None:
        e["agentId"] = agent_id
    return e


SUPERPOWERS_TEXT = (
    "<EXTREMELY_IMPORTANT>\nYou have superpowers.\n\n**Below is the full content of your "
    "'superpowers:using-superpowers' skill.**\n</EXTREMELY_IMPORTANT>"
)
CS_SESSION_TEXT = "PROJECT BOOTSTRAP: As your FIRST codescout action, call\nworkspace(activate)."
CS_MEMORIES_TEXT = "codescout MEMORIES: architecture conventions gotchas -- read the matching ones."
SUBAGENT_BOOTSTRAP_TEXT = (
    "PROJECT BOOTSTRAP: workspace(action=\"activate\", path=\"/repo\") is your FIRST\n"
    "codescout action, before Phase 0 below."
)
BUDDY_RELOADED_TEXT = (
    "<!-- buddy:reloaded sid=s-1 from=s-0 source=compact -->\n\n"
    "Reloaded from compact -- and ONLY these, nothing else: reconnaissance."
)
UPS_TEXT = (
    "→ skill `codescout-companion:reconnaissance` already loaded this session (seen 2×) "
    "— do not re-invoke; its instructions are still in context."
)
CS_HINT_TEXT = "[cs-hint] Use `read_file` or `find_symbol` — Bash on source files is blocked."


def _hook_rows(events_db):
    rows = _table_rows(sqlite3.connect(str(events_db)), "deliveries")
    return [d for d in rows if d["source"] == "transcript_hook"]


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
               agent_id=None, tool_use_id=None, called_at=None, latency_ms=None):
    # R54: `called_at` defaults to `started_at` (today's shape); a caller testing the
    # started_at-NULL fallback passes started_at=None and a distinct called_at explicitly.
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
        "called_at": called_at if called_at is not None else started_at,
        "latency_ms": latency_ms,
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
    # R110: Session-Id is git's trailer parser, which reads only the LAST paragraph as trailers
    # -- so the extra body line sits above it (a non-trailer line after it would leave git no
    # trailer block; 0 such commits in this repo's history, measured 2026-09-28).
    run("commit", "-q", "-m", "fix: tweak a and add b\n\nBody-Line-2\n\nSession-Id: sess-456")
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


class TurnsInsertIsPlain(unittest.TestCase):
    """The R45 uuid gate upstream is what keeps `turns` at one row per (sid, uuid). If it ever
    lets a uuid through twice, the second write must RAISE, not vanish behind `INSERT OR
    IGNORE` while the `turns` counter still counts it.
    """

    def test_a_second_turns_write_of_one_uuid_raises_integrity_error(self):
        conn = sqlite3.connect(":memory:")
        join._create_schema(conn)
        counts = {}
        entry = _entry("u-dup", "2026-09-20T10:00:00Z", "sid1")

        join._write_turn(conn, counts, ".claude/sid1", None, entry, "prompt", {})
        with self.assertRaises(sqlite3.IntegrityError):
            join._write_turn(conn, counts, ".claude/sid1", None, entry, "prompt", {})


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
class CalledAtFallback(unittest.TestCase):
    """R54: 81% of usage rows have `started_at` NULL (every row before 2026-09-20 19:17:34
    UTC). The heuristic then falls back to `called_at` -- measured completion time -- minus
    `latency_ms`, so it can still find a candidate that a `started_at`-only heuristic would
    miss entirely.
    """

    def test_a_null_started_at_with_called_at_and_latency_ms_joins_heuristically(self):
        conn = sqlite3.connect(":memory:")
        join._create_schema(conn)
        counts = _fresh_counts()

        candidates = [_candidate(
            "p/sid1", "sid1", "mcp__codescout__grep", {"pattern": "x"}, _iso(BASE),
        )]
        # called_at is 3s after the transcript ts; latency_ms=2500 pulls the adjusted time to
        # 0.5s after it -- well inside JOIN_WINDOW_SECONDS. started_at=None means this row can
        # only join through the called_at fallback.
        usage_rows = [
            _usage_row("db:1", "grep", "sid1", json.dumps({"pattern": "x"}), started_at=None,
                       called_at=_sql_ts(BASE + timedelta(seconds=3)), latency_ms=2500),
        ]

        join._join_tool_events(conn, candidates, usage_rows, counts, {}, {})
        conn.commit()
        rows = _table_rows(conn, "tool_events")

        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]["join_method"], "heuristic")
        self.assertEqual(rows[0]["usage_row_id"], "db:1")
        self.assertEqual(counts["heuristic_via_called_at"], 1)

    def test_a_null_started_at_with_called_at_200_seconds_later_does_not_join(self):
        conn = sqlite3.connect(":memory:")
        join._create_schema(conn)
        counts = _fresh_counts()

        candidates = [_candidate(
            "p/sid1", "sid1", "mcp__codescout__grep", {"pattern": "x"}, _iso(BASE),
        )]
        usage_rows = [
            _usage_row("db:1", "grep", "sid1", json.dumps({"pattern": "x"}), started_at=None,
                       called_at=_sql_ts(BASE + timedelta(seconds=200)), latency_ms=2500),
        ]

        join._join_tool_events(conn, candidates, usage_rows, counts, {}, {})
        conn.commit()
        rows = _table_rows(conn, "tool_events")

        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]["join_method"], "none")
        self.assertEqual(counts.get("heuristic_via_called_at", 0), 0)

    def test_the_called_at_fallback_subtracts_latency_before_the_120_second_window_check(self):
        conn = sqlite3.connect(":memory:")
        join._create_schema(conn)
        counts = _fresh_counts()

        candidates = [_candidate(
            "p/sid1", "sid1", "mcp__codescout__grep", {"pattern": "x"}, _iso(BASE),
        )]
        # Edge case that distinguishes "subtract latency_ms" from "ignore latency_ms": raw
        # called_at is 122s after the transcript ts (OUTSIDE the 120s window), but
        # latency_ms=3000 pulls the adjusted time to 119s after it (INSIDE the window). A
        # fallback that ignores latency_ms would see 122s and reject this candidate.
        usage_rows = [
            _usage_row("db:1", "grep", "sid1", json.dumps({"pattern": "x"}), started_at=None,
                       called_at=_sql_ts(BASE + timedelta(seconds=122)), latency_ms=3000),
        ]

        join._join_tool_events(conn, candidates, usage_rows, counts, {}, {})
        conn.commit()
        rows = _table_rows(conn, "tool_events")

        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]["join_method"], "heuristic")
        self.assertEqual(rows[0]["usage_row_id"], "db:1")
        self.assertEqual(counts["heuristic_via_called_at"], 1)


    def test_a_started_at_keyed_heuristic_join_leaves_heuristic_via_called_at_at_zero(self):
        conn = sqlite3.connect(":memory:")
        join._create_schema(conn)
        counts = _fresh_counts()

        candidates = [_candidate(
            "p/sid1", "sid1", "mcp__codescout__grep", {"pattern": "x"}, _iso(BASE),
        )]
        # started_at is present, so the time key is started_at and the called_at fallback is
        # never consulted. load-bearing: called_at and latency_ms are set anyway, so a counter
        # that fires for every heuristic join, fallback or not, has something to fire on.
        usage_rows = [
            _usage_row("db:1", "grep", "sid1", json.dumps({"pattern": "x"}),
                       started_at=_sql_ts(BASE + timedelta(seconds=1)),
                       called_at=_sql_ts(BASE + timedelta(seconds=3)), latency_ms=2000),
        ]

        join._join_tool_events(conn, candidates, usage_rows, counts, {}, {})
        conn.commit()
        rows = _table_rows(conn, "tool_events")

        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]["join_method"], "heuristic")
        self.assertEqual(counts.get("heuristic_via_called_at", 0), 0)


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
        prompt_uuids, interrupt_uuids, delegation_uuids, rejections = join._classify_uuids(
            entries, False
        )
        self.assertEqual(prompt_uuids, {"u1"})
        self.assertEqual(interrupt_uuids, {"u2"})
        self.assertEqual(delegation_uuids, set())
        self.assertEqual(rejections, {})

    def test_subagent_never_produces_prompt(self):
        entries = [
            _entry("u1", "2026-09-20T10:00:00Z", "sid1", content="the parent's brief"),
            _entry("u2", "2026-09-20T10:00:01Z", "sid1", content="[Request interrupted by user]"),
            _entry("u3", "2026-09-20T10:00:02Z", "sid1",
                   content=[{"type": "tool_result", "content": "ok"}]),
            _entry("u4", "2026-09-20T10:00:03Z", "sid1", is_meta=True),
            # R105 is top-level only: a rejection inside a subagent file is no rejection.
            # LOAD-BEARING: is_error True (R127), so the fixture IS a rejection at top level and
            # only the subagent path can refuse it.
            _entry("u5", "2026-09-20T10:00:04Z", "sid1",
                   content=[{"type": "tool_result", "is_error": True, "content": _REJECTED_TEXT}]),
        ]
        prompt_uuids, interrupt_uuids, delegation_uuids, rejections = join._classify_uuids(
            entries, True
        )
        self.assertEqual(prompt_uuids, set())
        self.assertEqual(interrupt_uuids, {"u2"})
        self.assertEqual(delegation_uuids, {"u1"})
        self.assertEqual(rejections, {})

    def test_top_level_rejections_map_uuid_to_feedback(self):
        entries = [
            _entry("u1", "2026-09-20T10:00:00Z", "sid1",
                   content=[{"type": "tool_result", "is_error": True, "content": _REJECTED_TEXT}]),
            _entry("u2", "2026-09-20T10:00:01Z", "sid1",
                   content=[{"type": "tool_result", "content": "ok"}]),
        ]
        _p, _i, _d, rejections = join._classify_uuids(entries, False)
        self.assertEqual(rejections, {"u1": "edit the test instead"})
    def test_a_sidechain_interrupt_marker_stays_interrupt_on_both_paths(self):
        # R132: isSidechain is not an interrupt filter -- every subagent entry is a sidechain,
        # and the marker is the operator's Esc in whichever chain was running. LOAD-BEARING:
        # isSidechain True on an exact marker with no origin, so only a sidechain filter in
        # operator_interrupts could turn it into delegation (subagent) or drop it (top-level).
        m = _entry("u1", "2026-09-20T10:00:00Z", "sid1", content="[Request interrupted by user]")
        m["isSidechain"] = True
        for is_subagent in (False, True):
            with self.subTest(is_subagent=is_subagent):
                _p, interrupt_uuids, delegation_uuids, _r = join._classify_uuids([m], is_subagent)
                self.assertEqual((interrupt_uuids, delegation_uuids), ({"u1"}, set()))


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
            spec_sid = sorted(join.SPEC_EXCLUDED_SIDS)[0]
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
class SubagentOwnershipOrder(unittest.TestCase):
    """X06: `_order_like_attribution`'s global first-writer-wins order decides who owns a
    subagent uuid shared across two UNRELATED kept sessions (different sids -- no exclusion
    mechanism touches either copy), and must agree with what `attribute_entries` independently
    computes for the same sessions' shared TOP-LEVEL uuid -- `_process_subagent_union`'s
    seen_uuids gate is only correct when the caller visits kept sessions in that exact order
    (R45). Before this test, nothing asserted WHICH copy_id won a shared subagent uuid, and
    nothing cross-checked the two orderings against each other; reversing
    `_order_like_attribution`'s sort flips 380 real subagent-uuid owners on the live corpus with
    no test noticing (reviewer mutation X06).
    """

    def test_the_higher_uuid_count_session_owns_the_shared_uuid_and_agrees_with_attribute_entries(self):
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")

            # sess-hi has 3 distinct top-level uuids (incl. the shared one), sess-lo has 2 --
            # both `_order_like_attribution` and `attribute_entries` sort candidates by
            # (-uuid_count, first_ts, copy_id). load-bearing: sess-hi has the higher count but
            # the LATER first_ts (09:30 vs 09:00), so count and time disagree. An order that
            # drops the count dimension, leaving (first_ts, copy_id), puts sess-lo first and
            # drifts from attribute_entries -- which this fixture must detect (mutant D1).
            _write_lines(proj / "sess-hi.jsonl", [
                _entry("shared-top-1", "2026-09-20T09:30:00Z", "sess-hi"),
                _entry("hi2", "2026-09-20T09:30:01Z", "sess-hi"),
                _entry("hi3", "2026-09-20T09:30:02Z", "sess-hi"),
            ])
            _write_lines(proj / "sess-lo.jsonl", [
                _entry("shared-top-1", "2026-09-20T09:00:00Z", "sess-lo"),
                _entry("lo2", "2026-09-20T09:00:01Z", "sess-lo"),
            ])

            shared_sub_entries = [_entry("shared-sub-1", "2026-09-20T10:00:00Z", "sess-hi")]
            _write_lines(proj / "sess-hi" / "subagents" / "sub.jsonl", shared_sub_entries)
            _write_lines(proj / "sess-lo" / "subagents" / "sub.jsonl", shared_sub_entries)

            events_db = pathlib.Path(tmp) / "events.db"
            join.build_events(corpus_dir, events_db, excluded_sids=set())

            conn = sqlite3.connect(str(events_db))
            turns = _table_rows(conn, "turns")

            top_owner = [r for r in turns if r["uuid"] == "shared-top-1"]
            sub_owner = [r for r in turns if r["uuid"] == "shared-sub-1"]
            self.assertEqual(len(top_owner), 1)
            self.assertEqual(len(sub_owner), 1)
            self.assertEqual(top_owner[0]["sid"], ".claude-sdd/sess-hi")
            self.assertEqual(sub_owner[0]["sid"], ".claude-sdd/sess-hi")

            # Cross-check: `_order_like_attribution`'s order must agree with what
            # `attribute_entries` independently decides for the same shared top-level uuid on
            # the same sessions -- this is the check that catches the two orderings drifting
            # apart from each other, since `_process_subagent_union` relies on the caller
            # visiting kept sessions in `_order_like_attribution`'s order for its global
            # seen_uuids gate to match attribute_entries' own top-level priority.
            sessions_list = transcripts.sessions(corpus_dir)
            excl = transcripts.exclusions(sessions_list, set())
            ordered_cids = [
                transcripts.copy_id(s) for s in join._order_like_attribution(sessions_list, excl)
            ]
            attribution = transcripts.attribute_entries(sessions_list, excl)
            self.assertEqual(ordered_cids[0], ".claude-sdd/sess-hi")
            self.assertEqual(attribution["shared-top-1"], ordered_cids[0])
class SubagentAttachmentUnionGate(unittest.TestCase):
    """X05: `_process_subagent_union`'s `seen_uuids` gate must be checked BEFORE the
    attachment-type branch, not after -- otherwise a subagent ATTACHMENT whose uuid is shared
    across the global union (the same physical hook-injection record copied into two kept
    sessions' subagent directories, as a fork pair's shared subagent file is) is emitted once
    PER KEEPER instead of once total, since the attachment path would bypass the very dedup
    gate that plain entries are correctly subject to. Calls `_process_subagent_union` directly
    (bypassing `build_events`'s later `_dedup_hook_deliveries` pass, which under R56 never
    collapses two hook_additional_context rows anyway) -- the two duplicate rows this test
    constructs share a uuid, so only the seen_uuids gate can prevent the second emission. Reviewer's own mutation of this ordering double-emitted 12 real subagent
    deliverable attachments on the live corpus with no test noticing.
    """

    def test_a_subagent_attachment_shared_across_the_global_union_is_emitted_once(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = pathlib.Path(tmp)
            sub_dir_a = tmp_path / "fork-a" / "subagents"
            sub_dir_b = tmp_path / "fork-b" / "subagents"
            # Same uuid, same content -- a literal copy, as a fork pair's subagent file is --
            # but each keeper's own call passes its OWN tool_use_id, so the two rows this
            # emits (if the gate fails to block the second) differ in tool_use_id too, and only
            # the seen_uuids gate stands between one row and two.
            shared_att_a = [_attachment_entry(
                "sub_att1", "2026-09-21T10:05:00Z", "fork-a", tool_use_id="tu-a",
            )]
            shared_att_b = [_attachment_entry(
                "sub_att1", "2026-09-21T10:05:00Z", "fork-a", tool_use_id="tu-b",
            )]
            _write_lines(sub_dir_a / "sub.jsonl", shared_att_a)
            _write_lines(sub_dir_b / "sub.jsonl", shared_att_b)

            keeper_a = transcripts.Session(
                sid="fork-a", path=tmp_path / "fork-a.jsonl", profile=".claude-sdd",
                entrypoint="cli", subagent_paths=[sub_dir_a / "sub.jsonl"], first_ts="t1",
            )
            keeper_b = transcripts.Session(
                sid="fork-b", path=tmp_path / "fork-b.jsonl", profile=".claude-sdd",
                entrypoint="cli", subagent_paths=[sub_dir_b / "sub.jsonl"], first_ts="t2",
            )
            sessions_list = [keeper_a, keeper_b]
            excl = {}
            conn = sqlite3.connect(":memory:")
            join._create_schema(conn)
            counts = _fresh_counts()
            seen_uuids = set()

            _, deliveries_a = join._process_subagent_union(
                conn, sessions_list, excl, keeper_a, "fork-a", tmp_path, counts, seen_uuids,
            )
            _, deliveries_b = join._process_subagent_union(
                conn, sessions_list, excl, keeper_b, "fork-b", tmp_path, counts, seen_uuids,
            )

            all_deliveries = deliveries_a + deliveries_b
            self.assertEqual(len(all_deliveries), 1)
            self.assertEqual(counts.get("subagent_duplicate_uuids_skipped"), 1)






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
    demonstrated by mutant M18 in probes/task6-fix2-mutants.txt (swap the bucket key's bare_sid
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
                "input_json TEXT, output_json TEXT, deliveries_json TEXT, tool_use_id TEXT, "
                "latency_ms INTEGER)"
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
            # started_at is non-NULL here, so this join never used the R54 called_at fallback.
            self.assertEqual(counts["heuristic_via_called_at"], 0)



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


class HookDeliveryDedup(unittest.TestCase):
    """R53, as narrowed by R56: a `hook_success`/`hook_additional_context` twin yields ONE row
    (the hac), and a lone `hook_success` falls back to itself and is counted in
    `hook_success_only`. These two are the Pre/PostToolUse shape, where both sides do share a
    toolUseID; `HookTwinRule` covers the shapes where they do not.
    """

    def test_a_hook_success_and_hook_additional_context_twin_yields_one_row(self):
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")

            _write_lines(proj / "sid1.jsonl", [
                _entry("u1", "2026-09-20T10:00:00Z", "sid1"),
                _hook_success_entry("hs1", "2026-09-20T11:00:00Z", "sid1",
                                     hook_name="PostToolUse", content="same injected text",
                                     tool_use_id="toolu_1"),
                _attachment_entry("hc1", "2026-09-20T11:00:00Z", "sid1",
                                   hook_name="PostToolUse", content="same injected text",
                                   tool_use_id="toolu_1"),
            ])

            events_db = pathlib.Path(tmp) / "events.db"
            counts = join.build_events(corpus_dir, events_db, excluded_sids=set())

            deliveries = _table_rows(sqlite3.connect(str(events_db)), "deliveries")
            hook_deliveries = [d for d in deliveries if d["source"] == "transcript_hook"]
            self.assertEqual(len(hook_deliveries), 1)
            self.assertEqual(hook_deliveries[0]["engine_or_hook"], "PostToolUse")
            self.assertEqual(counts.get("hook_success_only", 0), 0)

    def test_a_lone_hook_success_yields_one_row_and_is_counted_as_hook_success_only(self):
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")

            _write_lines(proj / "sid1.jsonl", [
                _entry("u1", "2026-09-20T10:00:00Z", "sid1"),
                _hook_success_entry("hs1", "2026-09-20T11:00:00Z", "sid1",
                                     hook_name="PostToolUse", content="only copy",
                                     tool_use_id="toolu_2"),
            ])

            events_db = pathlib.Path(tmp) / "events.db"
            counts = join.build_events(corpus_dir, events_db, excluded_sids=set())

            deliveries = _table_rows(sqlite3.connect(str(events_db)), "deliveries")
            hook_deliveries = [d for d in deliveries if d["source"] == "transcript_hook"]
            self.assertEqual(len(hook_deliveries), 1)
            self.assertEqual(counts["hook_success_only"], 1)


class HookTwinRule(unittest.TestCase):
    """R56 (supersedes R53's key): a `hook_success` row is DROPPED as a twin iff a
    `hook_additional_context` (hac) row exists in the SAME session copy, the SAME transcript file
    and the SAME hook event, whose text EQUALS the hook_success text or has an ELEMENT equal to
    it, within |dts| <= 5 s. toolUseID is not part of the key, two hac rows are never collapsed,
    and nothing pairs across sessions or files.

    Each fixture copies a real shape from the 2026-09-27 snapshot (the file and lines are cited
    on the test), sanitized. The timestamps keep the real sub-second offsets between twins, so a
    window narrowed to 0 s fails on them.
    """

    SID_A = "19e0e253-6b26-4a74-a201-000000000001"
    SID_B = "2cb44cd3-8673-4604-a8ac-000000000002"

    def _build(self, files):
        """`files` maps a path under one project dir to its entries. Returns (hook rows, counts)."""
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        corpus_dir = _make_corpus(pathlib.Path(tmp.name))
        proj = _session_dir(corpus_dir, "00-.claude", "p")
        for rel, entries in files.items():
            _write_lines(proj / rel, entries)
        events_db = pathlib.Path(tmp.name) / "events.db"
        counts = join.build_events(corpus_dir, events_db, excluded_sids=set())
        return _hook_rows(events_db), counts

    def test_a_subagentstart_twin_under_a_different_tool_use_id_yields_one_row(self):
        # Real shape: 00-.claude/.../19e0e253-.../subagents/agent-a17b789c8283dda21.jsonl L2-L4.
        # All 909 SubagentStart twins on the snapshot carry a different uuid on each side.
        sid, agent = self.SID_A, "a17b789c8283dda21"
        rows, counts = self._build({
            f"{sid}.jsonl": [_entry("u1", "2026-09-02T09:30:00.000Z", sid)],
            f"{sid}/subagents/agent-{agent}.jsonl": [
                # agent-guide-snapshot.mjs printed `{}`: it injected nothing, so it has no row.
                _real_hook_success("s2", "2026-09-02T09:30:20.016Z", sid, "SubagentStart",
                                   "SubagentStart:general-purpose",
                                   "f6064fde-1c3a-43ce-9181-9474a695d62a", None, agent_id=agent),
                _real_hook_success("s3", "2026-09-02T09:30:20.021Z", sid, "SubagentStart",
                                   "SubagentStart:general-purpose",
                                   # load-bearing: NOT the hac's toolUseID (kills a tuid key)
                                   "f6064fde-1c3a-43ce-9181-9474a695d62a",
                                   SUBAGENT_BOOTSTRAP_TEXT, agent_id=agent),
                # load-bearing: 1 ms after s3, not 0 (kills a 0 s window)
                _real_hac("s4", "2026-09-02T09:30:20.022Z", sid, "SubagentStart", "SubagentStart",
                          "b92acfec-8c2b-44c4-82a5-f0bedb5a9797", [SUBAGENT_BOOTSTRAP_TEXT],
                          agent_id=agent),
            ],
        })
        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]["engine_or_hook"], "SubagentStart")  # the hac's hookName
        self.assertEqual(rows[0]["tool_use_id"], "b92acfec-8c2b-44c4-82a5-f0bedb5a9797")
        self.assertEqual(counts["hook_success_twins_dropped"], 1)
        self.assertEqual(counts["hook_success_only"], 0)

    def test_a_merged_two_string_sessionstart_hac_absorbs_both_hook_success_twins(self):
        # Real shape: 00-.claude/.../08a2785b-...jsonl L4-L6 (startup). Each hook_success text is
        # ONE ELEMENT of the merged hac, never the joined whole.
        sid, tuid = self.SID_A, "4cc21e92-88ee-4506-a178-6224eff03119"
        rows, counts = self._build({f"{sid}.jsonl": [
            _entry("u1", "2026-09-03T23:02:50.000Z", sid),
            # 325 ms before the hac
            _real_hook_success("h1", "2026-09-03T23:02:54.906Z", sid, "SessionStart",
                               "SessionStart:startup", tuid, SUPERPOWERS_TEXT),
            # 91 ms before the hac
            _real_hook_success("h2", "2026-09-03T23:02:55.140Z", sid, "SessionStart",
                               "SessionStart:startup", tuid, CS_SESSION_TEXT),
            # load-bearing: 2 elements, each equal to one hook_success text (kills
            # per-element matching disabled)
            _real_hac("h3", "2026-09-03T23:02:55.231Z", sid, "SessionStart", "SessionStart",
                      "SessionStart", [SUPERPOWERS_TEXT, CS_SESSION_TEXT]),
        ]})
        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]["sha256"], _sha(SUPERPOWERS_TEXT + "\n\n" + CS_SESSION_TEXT))
        self.assertEqual(counts["hook_success_twins_dropped"], 2)
        self.assertEqual(counts["hook_success_only"], 0)

    def test_a_sessionstart_hac_under_the_literal_tool_use_id_sessionstart_is_a_twin(self):
        # Real shape: 00-.claude/.../2cb44cd3-...jsonl L2388-L2389 (resume), 67 ms apart. All 666
        # SessionStart hacs on the snapshot carry the toolUseID "SessionStart".
        sid = self.SID_A
        rows, counts = self._build({f"{sid}.jsonl": [
            _entry("u1", "2026-09-02T01:52:39.303Z", sid),
            _real_hook_success("r1", "2026-09-02T01:52:39.387Z", sid, "SessionStart",
                               "SessionStart:resume", "7388e5d7-efc6-4480-8fc5-49ff8a7c3596",
                               CS_MEMORIES_TEXT),
            _real_hac("r2", "2026-09-02T01:52:39.454Z", sid, "SessionStart", "SessionStart",
                      "SessionStart", [CS_MEMORIES_TEXT]),
        ]})
        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]["tool_use_id"], "SessionStart")
        self.assertEqual(counts["hook_success_twins_dropped"], 1)
        self.assertEqual(counts["hook_success_only"], 0)

    def test_a_compact_plain_stdout_hook_success_with_no_hac_is_kept(self):
        # Real shape: 00-.claude/.../19e0e253-...jsonl L1127-L1130 (compact). Three hooks ran; the
        # merged hac carries only the two JSON-stdout hooks' texts. Buddy's plain-stdout text
        # never gets a hac (on the snapshot, no hac within 10 s carries even its first 120
        # characters), yet it sits 1 ms from a same-event hac, so only the TEXT key keeps it.
        sid, tuid = self.SID_A, "07b8849a-e40d-4e4d-b135-15451bc6c5c5"
        rows, counts = self._build({f"{sid}.jsonl": [
            _entry("u1", "2026-09-02T06:46:06.000Z", sid),
            _real_hook_success("c1", "2026-09-02T06:46:06.907Z", sid, "SessionStart",
                               "SessionStart:compact", tuid, SUPERPOWERS_TEXT),
            _real_hook_success("c2", "2026-09-02T06:46:06.959Z", sid, "SessionStart",
                               "SessionStart:compact", tuid, CS_MEMORIES_TEXT),
            # load-bearing: plain stdout, text in `content`, and no element of c4 equals it
            _real_hook_success("c3", "2026-09-02T06:46:07.173Z", sid, "SessionStart",
                               "SessionStart:compact", tuid, BUDDY_RELOADED_TEXT, plain=True),
            _real_hac("c4", "2026-09-02T06:46:07.174Z", sid, "SessionStart", "SessionStart",
                      "SessionStart", [SUPERPOWERS_TEXT, CS_MEMORIES_TEXT]),
        ]})
        buddy = [r for r in rows if r["sha256"] == _sha(BUDDY_RELOADED_TEXT)]
        self.assertEqual(len(buddy), 1)
        self.assertEqual(buddy[0]["engine_or_hook"], "SessionStart:compact")
        self.assertEqual(len(rows), 2)  # the buddy hook_success and the merged hac
        self.assertEqual(counts["hook_success_only"], 1)
        self.assertEqual(counts["hook_success_twins_dropped"], 2)

    def test_a_userpromptsubmit_plain_stdout_hook_success_is_kept(self):
        # Real shape: 00-.claude/.../bf44ba81-...jsonl L2506-L2507. No UserPromptSubmit hook on
        # the snapshot has a hac.
        sid = self.SID_A
        rows, counts = self._build({f"{sid}.jsonl": [
            _entry("p1", "2026-09-01T10:57:03.675Z", sid),
            _real_hook_success("p2", "2026-09-01T10:57:03.885Z", sid, "UserPromptSubmit",
                               "UserPromptSubmit", "a058aaa6-b893-41fe-a26c-92a311f29283",
                               UPS_TEXT, plain=True),
        ]})
        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]["engine_or_hook"], "UserPromptSubmit")
        self.assertEqual(rows[0]["sha256"], _sha(UPS_TEXT))
        self.assertEqual(counts["hook_success_only"], 1)
        self.assertEqual(counts.get("hook_success_twins_dropped", 0), 0)

    def test_byte_identical_sessionstart_text_in_two_sessions_is_never_paired_across_them(self):
        # Session A has the real hs/hac pair (shape: 2cb44cd3 L2388-L2389). Session B, starting
        # 1 s later, carries the same text as a hook_success with no hac of its own (the fallback
        # class). B's row must survive: a key without the session lets A's hac absorb it.
        a, b = self.SID_A, self.SID_B
        rows, counts = self._build({
            f"{a}.jsonl": [
                _entry("a0", "2026-09-02T01:52:30.000Z", a),
                _real_hook_success("a1", "2026-09-02T01:52:39.387Z", a, "SessionStart",
                                   "SessionStart:resume", "7388e5d7-efc6-4480-8fc5-49ff8a7c3596",
                                   CS_MEMORIES_TEXT),
                _real_hac("a2", "2026-09-02T01:52:39.454Z", a, "SessionStart", "SessionStart",
                          "SessionStart", [CS_MEMORIES_TEXT]),
            ],
            f"{b}.jsonl": [
                _entry("b0", "2026-09-02T01:52:31.000Z", b),
                # load-bearing: within 5 s of a2, same event, same text; no hac in session B
                _real_hook_success("b1", "2026-09-02T01:52:40.454Z", b, "SessionStart",
                                   "SessionStart:resume", "2dd3d6ba-f0d9-4bf1-a91a-ce0606d7460d",
                                   CS_MEMORIES_TEXT),
            ],
        })
        self.assertEqual(
            sorted(r["sid"] for r in rows), sorted([f".claude/{a}", f".claude/{b}"])
        )
        self.assertEqual(counts["hook_success_only"], 1)
        self.assertEqual(counts["hook_success_twins_dropped"], 1)

    def test_identical_sessionstart_hacs_of_two_sessions_are_never_collapsed(self):
        # The R53 defect itself: every SessionStart hac carries the toolUseID "SessionStart", so
        # R53's (toolUseID, event, sha256) key merged byte-identical hacs of DIFFERENT sessions
        # into one row, deleting 333 real deliveries on the fix-round-3 re-review's snapshot. The
        # two sessions here even share the timestamp, so no time key could separate them.
        a, b = self.SID_A, self.SID_B
        files = {}
        for sid, pfx in ((a, "a"), (b, "b")):
            files[f"{sid}.jsonl"] = [
                _entry(f"{pfx}0", "2026-09-02T01:52:30.000Z", sid),
                _real_hook_success(f"{pfx}1", "2026-09-02T01:52:39.387Z", sid, "SessionStart",
                                   "SessionStart:resume", f"{pfx}-tuid", CS_MEMORIES_TEXT),
                _real_hac(f"{pfx}2", "2026-09-02T01:52:39.454Z", sid, "SessionStart",
                          "SessionStart", "SessionStart", [CS_MEMORIES_TEXT]),
            ]
        rows, counts = self._build(files)
        self.assertEqual(
            sorted(r["sid"] for r in rows), sorted([f".claude/{a}", f".claude/{b}"])
        )
        self.assertTrue(all(r["tool_use_id"] == "SessionStart" for r in rows))
        self.assertEqual(counts["hook_success_twins_dropped"], 2)

    def test_the_same_text_in_two_files_of_one_session_is_never_paired_across_them(self):
        # SessionStart hooks occur in subagent files too (680 hook_success, 242 hac on the
        # snapshot). The top-level file's hook_success has no hac of its own; the subagent file
        # holds a hac with the same text 600 ms later. A key without the file would pair them.
        sid, agent = self.SID_A, "a24915499b197900d"
        rows, counts = self._build({
            f"{sid}.jsonl": [
                _entry("u1", "2026-09-02T11:53:30.000Z", sid),
                _real_hook_success("t1", "2026-09-02T11:53:35.361Z", sid, "SessionStart",
                                   "SessionStart:startup", "top-tuid", SUPERPOWERS_TEXT),
            ],
            f"{sid}/subagents/agent-{agent}.jsonl": [
                _real_hook_success("t2", "2026-09-02T11:53:35.861Z", sid, "SessionStart",
                                   "SessionStart:startup", "sub-tuid", SUPERPOWERS_TEXT,
                                   agent_id=agent),
                # load-bearing: same session, event and text as t1, 600 ms from it, other file
                _real_hac("t3", "2026-09-02T11:53:35.961Z", sid, "SessionStart", "SessionStart",
                          "SessionStart", [SUPERPOWERS_TEXT], agent_id=agent),
            ],
        })
        self.assertEqual(sorted(r["tool_use_id"] for r in rows), ["SessionStart", "top-tuid"])
        self.assertEqual(counts["hook_success_only"], 1)
        self.assertEqual(counts["hook_success_twins_dropped"], 1)

    def test_a_same_text_hac_six_seconds_away_is_not_a_twin(self):
        # Real shape: 00-.claude/.../774ba049-.../subagents/agent-a3ba615808d91a57d.jsonl. The
        # subagent's first SubagentStart has its twin; a later SubagentStart in the same file
        # has no hac, and the earlier hac carries its text (62.6 s away there). Here the gap is
        # 6 s, just outside the window; every real twin on the snapshot is within 1.713 s.
        sid, agent = self.SID_A, "a3ba615808d91a57d"
        rows, counts = self._build({
            f"{sid}.jsonl": [_entry("u1", "2026-09-24T07:39:00.000Z", sid)],
            f"{sid}/subagents/agent-{agent}.jsonl": [
                _real_hook_success("w1", "2026-09-24T07:39:12.049Z", sid, "SubagentStart",
                                   "SubagentStart:general-purpose", "tuid-1",
                                   SUBAGENT_BOOTSTRAP_TEXT, agent_id=agent),
                _real_hac("w2", "2026-09-24T07:39:12.050Z", sid, "SubagentStart", "SubagentStart",
                          "tuid-2", [SUBAGENT_BOOTSTRAP_TEXT], agent_id=agent),
                # load-bearing: exactly 6.000 s after w2 (kills an unbounded window)
                _real_hook_success("w3", "2026-09-24T07:39:18.050Z", sid, "SubagentStart",
                                   "SubagentStart:general-purpose", "tuid-3",
                                   SUBAGENT_BOOTSTRAP_TEXT, agent_id=agent),
            ],
        })
        self.assertEqual(sorted(r["tool_use_id"] for r in rows), ["tuid-2", "tuid-3"])
        self.assertEqual(counts["hook_success_twins_dropped"], 1)
        self.assertEqual(counts["hook_success_only"], 1)

    def test_a_same_text_hac_of_another_hook_event_is_not_a_twin(self):
        # Constructed from two real shapes: no text crosses hook events on the snapshot today,
        # so this case guards the ruling's event key rather than a measured collision. A lone
        # UserPromptSubmit hook_success (bf44ba81 L2507 shape) and a PostToolUse twin pair
        # (ffb95976 L121-L122 shape) share one text, 500 ms apart.
        sid = self.SID_A
        rows, counts = self._build({f"{sid}.jsonl": [
            _entry("e0", "2026-09-02T08:34:47.000Z", sid),
            _real_hook_success("e1", "2026-09-02T08:34:47.343Z", sid, "UserPromptSubmit",
                               "UserPromptSubmit", "ups-tuid", CS_HINT_TEXT, plain=True),
            _real_hook_success("e2", "2026-09-02T08:34:47.843Z", sid, "PostToolUse",
                               "PostToolUse:Bash", "toolu_01HpGPXXtbLsM3VX6cfRyDkg", CS_HINT_TEXT),
            # load-bearing: same text as e1, 500 ms away, but a different hook event
            _real_hac("e3", "2026-09-02T08:34:47.843Z", sid, "PostToolUse", "PostToolUse:Bash",
                      "toolu_01HpGPXXtbLsM3VX6cfRyDkg", [CS_HINT_TEXT]),
        ]})
        self.assertEqual(sorted(r["tool_use_id"] for r in rows),
                         ["toolu_01HpGPXXtbLsM3VX6cfRyDkg", "ups-tuid"])
        self.assertEqual(counts["hook_success_only"], 1)
        self.assertEqual(counts["hook_success_twins_dropped"], 1)

    def test_two_hacs_with_equal_text_in_one_file_are_both_kept(self):
        # Real shape: 00-.claude/.../ffb95976-...jsonl L121-L126. Two Bash calls 1.197 s apart
        # drew the same [cs-hint] text, each with its own hs/hac twin: two injections, so two
        # rows. A hac-vs-hac collapse would leave one.
        sid = self.SID_A
        a, b = "toolu_01HpGPXXtbLsM3VX6cfRyDkg", "toolu_01WbCnhBuWyNfDijrfZviCzt"
        rows, counts = self._build({f"{sid}.jsonl": [
            _entry("q0", "2026-09-02T08:34:40.000Z", sid),
            _real_hook_success("q1", "2026-09-02T08:34:47.843Z", sid, "PostToolUse",
                               "PostToolUse:Bash", a, CS_HINT_TEXT),
            _real_hac("q2", "2026-09-02T08:34:47.843Z", sid, "PostToolUse", "PostToolUse:Bash",
                      a, [CS_HINT_TEXT]),
            _real_hook_success("q3", "2026-09-02T08:34:49.040Z", sid, "PostToolUse",
                               "PostToolUse:Bash", b, CS_HINT_TEXT),
            # load-bearing: equal text to q2, 1.197 s after it, same file and event
            _real_hac("q4", "2026-09-02T08:34:49.040Z", sid, "PostToolUse", "PostToolUse:Bash",
                      b, [CS_HINT_TEXT]),
        ]})
        self.assertEqual(sorted(r["tool_use_id"] for r in rows), sorted([a, b]))
        self.assertEqual(counts["hook_success_twins_dropped"], 2)
        self.assertEqual(counts["hook_success_only"], 0)

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
                "input_json TEXT, output_json TEXT, deliveries_json TEXT, tool_use_id TEXT, "
                "latency_ms INTEGER)"
            )
            # R55/X07: a real, non-empty deliveries_json -- the pre-fix mutant (removing the
            # `continue` after the deliveries_unmapped_session increment) would call
            # _deliveries_from_usage_row(row) on this and emit 2 rows (one ledger_key row,
            # one blocks row) under the bare/NULL sid; with a NULL deliveries_json neither
            # version could ever fail len(deliveries) == 0.
            unmapped_deliveries_json = json.dumps(
                [{"engine": "get_guide", "ledger_keys": ["T-9"],
                  "blocks": [{"sha256": "abc123", "bytes": 42}]}]
            )
            conn.execute(
                "INSERT INTO tool_calls (tool_name, called_at, started_at, cc_session_id, "
                "input_json, output_json, deliveries_json, tool_use_id) VALUES "
                "('grep', ?, ?, 'not-a-kept-session', '{}', NULL, ?, NULL)",
                (_sql_ts(BASE), _sql_ts(BASE), unmapped_deliveries_json),
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
    def test_an_operator_rule_marker_quoted_mid_line_is_not_counted(self):
        # X09: OP_RULE_RE carries its own `^[ \t]*` line-start anchor, separate from
        # GUIDE_MARKER_RE's -- the test above only exercises the get_guide regex's anchor, so
        # a mutation that de-anchors OP_RULE_RE specifically (dropping the `^[ \t]*` prefix or
        # the MULTILINE flag) survived until this test existed. 3 real rows on the live corpus
        # carry a mid-line-quoted operator-rule marker that the mutant counts.
        block = "note: see <!-- operator-rule OP-7 -- some note --> for details"
        row = {
            "tool_name": "edit_file",
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


# --- Task 14 (spec Amendment 7) -----------------------------------------------------------

miner = _load("miner")

_REJECTED_TEXT = (
    "The user doesn't want to proceed with this tool use. The tool use was rejected. "
    "To tell you how to proceed, the user said:\n edit the test instead "
)


def _queued_entry(uuid, ts, sid, text, origin_kind="human", mode="prompt"):
    """A real-shaped R103 `queued_command` attachment (origin on the ATTACHMENT)."""
    return {
        "type": "attachment", "uuid": uuid, "timestamp": ts, "sessionId": sid,
        "attachment": {"type": "queued_command", "commandMode": mode, "prompt": text,
                       "origin": {"kind": origin_kind}},
    }


def _rejection_entry(uuid, ts, sid):
    return _entry(uuid, ts, sid, content=[
        {"type": "tool_result", "tool_use_id": "t-rej", "is_error": True,
         "content": _REJECTED_TEXT},
    ])


class QueuedPromptAndRejectionRows(unittest.TestCase):
    """R103/R105 recording: a queued human prompt is a `prompt` row under the attachment's uuid
    and ts; a rejection is ONE `rejection` row (it replaces tool_result); top-level only."""

    def _build(self, tmp):
        corpus_dir = _make_corpus(pathlib.Path(tmp))
        proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")
        _write_lines(proj / "sid1.jsonl", [
            _entry("p1", "2026-09-20T10:00:00Z", "sid1", content="fix the parser"),
            {"type": "assistant", "uuid": "a1", "timestamp": "2026-09-20T10:00:01Z",
             "sessionId": "sid1", "message": {"content": [{"type": "text", "text": "done"}]}},
            _queued_entry("q-human", "2026-09-20T10:00:02.5Z", "sid1", "no, the other parser"),
            _queued_entry("q-peer", "2026-09-20T10:00:03Z", "sid1", "a peer's note",
                          origin_kind="peer"),
            # R130: NOT deduped -- byte-equal to the later kept prompt p2, and still a prompt row
            # of its own: the queued message and the prompt are two operator messages.
            _queued_entry("q-equal", "2026-09-20T10:00:04Z", "sid1", "and run the tests"),
            _entry("p2", "2026-09-20T10:00:05Z", "sid1", content="and run the tests"),
            _rejection_entry("r1", "2026-09-20T10:00:06Z", "sid1"),
            _entry("tr1", "2026-09-20T10:00:07Z", "sid1",
                   content=[{"type": "tool_result", "tool_use_id": "t1", "content": "ok"}]),
        ])
        _write_lines(proj / "sid1" / "subagents" / "sub.jsonl", [
            _entry("s-brief", "2026-09-20T10:01:00Z", "sid1", content="the parent's brief"),
            _queued_entry("s-queued", "2026-09-20T10:01:01Z", "sid1", "typed at a subagent"),
            _rejection_entry("s-rej", "2026-09-20T10:01:02Z", "sid1"),
        ])
        events_db = pathlib.Path(tmp) / "events.db"
        counts = join.build_events(corpus_dir, events_db, excluded_sids=set())
        turns = _table_rows(sqlite3.connect(str(events_db)), "turns")
        return counts, turns

    def test_a_queued_human_prompt_is_a_prompt_row_under_the_attachments_uuid_and_ts(self):
        with tempfile.TemporaryDirectory() as tmp:
            counts, turns = self._build(tmp)
            by_uuid = {r["uuid"]: r for r in turns}
            self.assertIn("q-human", by_uuid)
            row = by_uuid["q-human"]
            self.assertEqual(
                (row["kind"], row["role"], row["text"], row["ts"], row["agent_path"]),
                ("prompt", "attachment", "no, the other parser", "2026-09-20T10:00:02.500Z", None),
            )
            # R130: q-equal counts too, beside the equal-text prompt p2 -- once each.
            self.assertEqual(
                (by_uuid["q-equal"]["kind"], by_uuid["q-equal"]["role"], by_uuid["q-equal"]["text"]),
                ("prompt", "attachment", "and run the tests"),
            )
            self.assertEqual(counts["prompts_queued"], 2)
            # The peer message is not the operator's own words: no row.
            self.assertNotIn("q-peer", by_uuid)
            self.assertEqual(
                sorted(r["uuid"] for r in turns if r["kind"] == "prompt"),
                ["p1", "p2", "q-equal", "q-human"],
            )

    def test_a_rejection_is_one_rejection_row_replacing_tool_result(self):
        with tempfile.TemporaryDirectory() as tmp:
            _counts, turns = self._build(tmp)
            rows = [r for r in turns if r["uuid"] == "r1"]
            self.assertEqual(len(rows), 1)
            self.assertEqual(
                (rows[0]["kind"], rows[0]["role"], rows[0]["text"], rows[0]["ts"]),
                ("rejection", "user", "edit the test instead", "2026-09-20T10:00:06.000Z"),
            )
            by_uuid = {r["uuid"]: r for r in turns}
            self.assertEqual(by_uuid["tr1"]["kind"], "tool_result")

    def test_subagent_entries_stay_delegation_and_tool_result(self):
        with tempfile.TemporaryDirectory() as tmp:
            _counts, turns = self._build(tmp)
            by_uuid = {r["uuid"]: r for r in turns}
            self.assertEqual(by_uuid["s-brief"]["kind"], "delegation")
            self.assertEqual(by_uuid["s-rej"]["kind"], "tool_result")
            self.assertNotIn("s-queued", by_uuid)
            self.assertEqual({r["kind"] for r in turns if r["agent_path"]},
                             {"delegation", "tool_result"})


class TopLevelInterruptKind(unittest.TestCase):
    """The top-level call site's interrupt argument: a top-level interrupt marker is an
    `interrupt` row, never the `meta` it falls through to without it."""

    def test_a_top_level_interrupt_marker_is_an_interrupt_row(self):
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")
            _write_lines(proj / "sid1.jsonl", [
                _entry("p1", "2026-09-20T10:00:00Z", "sid1", content="do the thing"),
                _entry("i1", "2026-09-20T10:00:01Z", "sid1",
                       content="[Request interrupted by user]"),
            ])
            events_db = pathlib.Path(tmp) / "events.db"
            join.build_events(corpus_dir, events_db, excluded_sids=set())
            by_uuid = {r["uuid"]: r for r in _table_rows(sqlite3.connect(str(events_db)), "turns")}
            self.assertEqual(by_uuid["i1"]["kind"], "interrupt")
            self.assertIsNone(by_uuid["i1"]["agent_path"])


class ExclusionsRecordedInManifest(unittest.TestCase):
    """R106/R1: build_events records the exclusion map, reasons included, in the manifest."""

    REVIEW = "82cff72e-0245-48cb-ab07-45a1c3d0d388"

    def test_the_spec_set_holds_both_measurement_sessions(self):
        self.assertEqual(
            join.SPEC_EXCLUDED_SIDS,
            frozenset({"3c5b02df-b6ce-45f5-9d03-1194e38465c0", self.REVIEW}),
        )

    def test_a_default_build_records_the_review_session_and_its_fork(self):
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")
            shared = [f"sh{i}" for i in range(transcripts.FORK_PREFIX_LEN)]
            _write_lines(proj / f"{self.REVIEW}.jsonl", [
                _entry(u, f"2026-09-20T10:00:0{i}Z", self.REVIEW) for i, u in enumerate(shared)
            ])
            _write_lines(proj / "fork-sid.jsonl", [
                _entry(u, f"2026-09-20T10:00:0{i}Z", "fork-sid") for i, u in enumerate(shared)
            ] + [_entry("fork-own", "2026-09-20T10:00:09Z", "fork-sid")])
            _write_lines(proj / "kept-sid.jsonl", [
                _entry("k1", "2026-09-20T10:00:00Z", "kept-sid", content="hello"),
            ])
            events_db = pathlib.Path(tmp) / "events.db"
            counts = join.build_events(corpus_dir, events_db)
            manifest = json.loads((corpus_dir / "manifest.json").read_text())
            self.assertEqual(manifest["exclusions"], [
                {"sid": ".claude-sdd/82cff72e-0245-48cb-ab07-45a1c3d0d388",
                 "reason": "excluded-by-spec"},
                {"sid": ".claude-sdd/fork-sid",
                 "reason": "fork-of-excluded:.claude-sdd/82cff72e-0245-48cb-ab07-45a1c3d0d388"},
            ])
            self.assertEqual(
                transcripts.read_exclusions(corpus_dir),
                transcripts.exclusions(transcripts.sessions(corpus_dir), join.SPEC_EXCLUDED_SIDS),
            )
            self.assertEqual((counts["sessions_kept"], counts["sessions_excluded"]), (1, 2))
            turn_sids = {r["sid"] for r in _table_rows(sqlite3.connect(str(events_db)), "turns")}
            self.assertEqual(turn_sids, {".claude-sdd/kept-sid"})


class NewBuildCounters(unittest.TestCase):
    """R107 entrypoint_changed and R113 decode_replaced_lines reach events_meta."""

    def test_entrypoint_changed_and_decode_replaced_lines_are_counted_and_persisted(self):
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")
            _write_lines(proj / "changed.jsonl", [
                _entry("c0", "2026-09-20T10:00:00Z", "changed", entrypoint="cli"),
                _entry("c1", "2026-09-20T10:00:01Z", "changed", entrypoint="sdk-ts"),
            ])
            _write_lines(proj / "same.jsonl", [
                _entry("s0", "2026-09-20T10:00:00Z", "same"),
            ])
            with open(proj / "same.jsonl", "ab") as f:
                f.write(b'{"type": "user", "uuid": "s1", "timestamp": "2026-09-20T10:00:01Z", '
                        b'"message": {"content": "caf\xc3"}}\n')
            events_db = pathlib.Path(tmp) / "events.db"
            try:
                counts = join.build_events(corpus_dir, events_db, excluded_sids=set())
            except UnicodeDecodeError as e:  # the defect is the raise: report it as a failure
                self.fail(f"build_events raised on a torn UTF-8 line: {e}")
            self.assertEqual(counts.get("entrypoint_changed"), 1)
            self.assertEqual(counts.get("decode_replaced_lines"), 1)
            conn = sqlite3.connect(str(events_db))
            meta = dict(conn.execute("SELECT key, value FROM events_meta").fetchall())
            conn.close()
            self.assertEqual((int(meta["entrypoint_changed"]), int(meta["decode_replaced_lines"])),
                             (1, 1))


def _git(repo, *args, env=None):
    return subprocess.run(
        ["git", *args], cwd=str(repo), check=True, capture_output=True, text=True, env=env,
    ).stdout.strip()


def _git_init(repo):
    repo.mkdir(parents=True, exist_ok=True)
    _git(repo, "init", "-q", "-b", "main")
    _git(repo, "config", "user.name", "t")
    _git(repo, "config", "user.email", "t@t")


def _dated_commit(repo, name, message, author, committer=None, allow_empty=False):
    """Commit `name` (a new file, unless allow_empty) with explicit author/committer dates."""
    env = dict(os.environ, GIT_AUTHOR_DATE=author, GIT_COMMITTER_DATE=committer or author)
    if allow_empty:
        _git(repo, "commit", "-q", "--allow-empty", "-m", message, env=env)
    else:
        (repo / name).write_text(name)
        _git(repo, "add", name)
        _git(repo, "commit", "-q", "-m", message, env=env)
    return _git(repo, "rev-parse", "HEAD")


def _merge_repo(repo):
    """main: a -> c, side: b; then a --no-ff merge of side carrying a Session-Id trailer.
    Returns the merge sha."""
    _git_init(repo)
    _dated_commit(repo, "a.txt", "feat: a\n\nSession-Id: S-A", "2026-06-01T00:00:00Z")
    _git(repo, "checkout", "-q", "-b", "side")
    _dated_commit(repo, "b.txt", "feat: b\n\nSession-Id: S-B", "2026-06-02T00:00:00Z")
    _git(repo, "checkout", "-q", "main")
    _dated_commit(repo, "c.txt", "feat: c\n\nSession-Id: S-C", "2026-06-03T00:00:00Z")
    env = dict(os.environ, GIT_AUTHOR_DATE="2026-06-04T00:00:00Z",
               GIT_COMMITTER_DATE="2026-06-04T00:00:00Z")
    _git(repo, "merge", "-q", "--no-ff", "side", "-m",
         "Merge branch side\n\nSome prose about the merge.\n\nSession-Id: S-MERGE", env=env)
    return _git(repo, "rev-parse", "HEAD")


class GitLogR110(unittest.TestCase):
    """R110 + bug 6708cab25f53b797: trailer-parser Session-Id, author dates, the window filtered
    in Python over the full walk, and the file list located by an explicit end marker."""

    def test_a_merge_with_a_trailer_has_its_session_id_and_no_files(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo = pathlib.Path(tmp) / "repo"
            merge_sha = _merge_repo(repo)
            by_sha = {c["sha"]: c for c in join._run_git_log(repo, merge_sha, None, None)}
            self.assertEqual(len(_git(repo, "rev-list", "--parents", "-n1", merge_sha).split()), 3)
            self.assertEqual(by_sha[merge_sha]["session_id"], "S-MERGE")
            # git log --name-only prints no file section for a merge: files is [], never the
            # trailer lines.
            self.assertEqual(by_sha[merge_sha]["files"], [])
            # The ordinary commits around it keep their own files and trailers.
            by_subject = {c["subject"]: c for c in by_sha.values()}
            self.assertEqual((by_subject["feat: c"]["session_id"], by_subject["feat: c"]["files"]),
                             ("S-C", ["c.txt"]))

    def test_a_prose_session_id_line_above_the_trailer_loses(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo = pathlib.Path(tmp) / "repo"
            _git_init(repo)
            sha = _dated_commit(
                repo, "a.txt",
                "docs: a change\n\nSession-Id: WRONG was the value a draft named.\n\n"
                "Session-Id: S-REAL",
                "2026-06-01T00:00:00Z",
            )
            (commit,) = join._run_git_log(repo, sha, None, None)
            self.assertEqual((commit["session_id"], commit["files"]), ("S-REAL", ["a.txt"]))

    def test_an_allow_empty_commit_has_no_files(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo = pathlib.Path(tmp) / "repo"
            _git_init(repo)
            _dated_commit(repo, "a.txt", "feat: a", "2026-06-01T00:00:00Z")
            sha = _dated_commit(repo, None, "chore: empty\n\nA body paragraph.\n\nSession-Id: S-E",
                                "2026-06-02T00:00:00Z", allow_empty=True)
            by_sha = {c["sha"]: c for c in join._run_git_log(repo, sha, None, None)}
            self.assertEqual((by_sha[sha]["session_id"], by_sha[sha]["files"]), ("S-E", []))

    def test_ts_is_the_author_date_not_the_committer_date(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo = pathlib.Path(tmp) / "repo"
            _git_init(repo)
            sha = _dated_commit(repo, "a.txt", "feat: a", "2026-06-01T10:00:00Z",
                                committer="2026-07-15T12:00:00Z")
            (commit,) = join._run_git_log(repo, sha, None, None)
            self.assertEqual(commit["ts"], "2026-06-01T10:00:00.000Z")

    def test_a_commit_behind_an_older_committer_date_is_still_found(self):
        # git's --since stops the walk at B (committer date restamped to before the window), so
        # A -- in the window by BOTH dates -- is never reached. A Python filter on the author
        # date over the full walk finds all three.
        with tempfile.TemporaryDirectory() as tmp:
            repo = pathlib.Path(tmp) / "repo"
            _git_init(repo)
            _dated_commit(repo, "a.txt", "A", "2026-06-01T00:00:00Z")
            _dated_commit(repo, "b.txt", "B", "2026-06-02T00:00:00Z",
                          committer="2026-01-01T00:00:00Z")
            head = _dated_commit(repo, "c.txt", "C", "2026-06-03T00:00:00Z")
            commits = join._run_git_log(repo, head, "2026-03-01T00:00:00Z", "2026-12-01T00:00:00Z")
            self.assertEqual(sorted(c["subject"] for c in commits), ["A", "B", "C"])

    def test_the_window_is_half_open_on_the_author_date(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo = pathlib.Path(tmp) / "repo"
            _git_init(repo)
            _dated_commit(repo, "a.txt", "at-start", "2026-06-01T00:00:00Z")
            _dated_commit(repo, "b.txt", "inside", "2026-06-02T00:00:00Z")
            head = _dated_commit(repo, "c.txt", "at-end", "2026-06-03T00:00:00Z",
                                 committer="2026-06-01T12:00:00Z")
            commits = join._run_git_log(repo, head, "2026-06-01T00:00:00Z", "2026-06-03T00:00:00Z")
            self.assertEqual(sorted(c["subject"] for c in commits), ["at-start", "inside"])

    def test_a_non_utf8_commit_message_is_decoded_with_replacement(self):
        # R113: a raw latin-1 byte in a message must not abort the build with UnicodeDecodeError.
        # `git commit` itself re-encodes a non-UTF-8 message (it assumes latin-1), so the
        # commit object is written directly -- the shape another tool or an old git can leave.
        with tempfile.TemporaryDirectory() as tmp:
            repo = pathlib.Path(tmp) / "repo"
            _git_init(repo)
            (repo / "a.txt").write_text("a")
            _git(repo, "add", "a.txt")
            tree = _git(repo, "write-tree")
            obj = (f"tree {tree}\nauthor t <t@t> 1780272000 +0000\n"
                   "committer t <t@t> 1780272000 +0000\n\n").encode()
            obj += b"feat: caf\xe9 in latin-1\n\nSession-Id: S-L1\n"
            sha = subprocess.run(
                ["git", "hash-object", "-t", "commit", "-w", "--stdin"], cwd=str(repo),
                input=obj, capture_output=True, check=True,
            ).stdout.decode().strip()
            try:
                (commit,) = join._run_git_log(repo, sha, None, None)
            except UnicodeDecodeError as e:  # the defect is the raise: report it as a failure
                self.fail(f"_run_git_log raised on a non-UTF-8 message: {e}")
            self.assertEqual(commit["subject"], "feat: caf� in latin-1")
            self.assertEqual((commit["session_id"], commit["files"]), ("S-L1", ["a.txt"]))
            self.assertEqual(commit["ts"], "2026-06-01T00:00:00.000Z")


class OneSessionIdDefinition(unittest.TestCase):
    """10b: join._run_git_log's session_id EQUALS miner.session_id_of -- one Session-Id
    definition across the two modules (R110), on the two shapes that used to split them."""

    def test_join_and_miner_agree_on_a_prose_paragraph_above_the_trailer(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo = pathlib.Path(tmp) / "repo"
            _git_init(repo)
            sha = _dated_commit(
                repo, "a.txt",
                "docs: a change\n\nSession-Id: WRONG was the value a draft named.\n\n"
                "Session-Id: S-REAL",
                "2026-06-01T00:00:00Z",
            )
            (commit,) = join._run_git_log(repo, sha, None, None)
            self.assertEqual(commit["session_id"], miner.session_id_of(repo, sha))
            self.assertEqual(commit["session_id"], "S-REAL")

    def test_join_and_miner_agree_on_a_two_parent_merge(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo = pathlib.Path(tmp) / "repo"
            merge_sha = _merge_repo(repo)
            by_sha = {c["sha"]: c for c in join._run_git_log(repo, merge_sha, None, None)}
            for sha, commit in by_sha.items():
                self.assertEqual(commit["session_id"], miner.session_id_of(repo, sha), sha)
            self.assertEqual(by_sha[merge_sha]["session_id"], "S-MERGE")
    def test_join_and_miner_agree_on_two_session_id_trailers_and_the_first_wins(self):
        # m-5: two Session-Id values in ONE trailer block. Both modules take the FIRST
        # non-empty value (git prints the values in message order). LOAD-BEARING: the two
        # values differ, so a last-value reader in either module disagrees.
        with tempfile.TemporaryDirectory() as tmp:
            repo = pathlib.Path(tmp) / "repo"
            _git_init(repo)
            sha = _dated_commit(
                repo, "a.txt", "feat: two trailers\n\nSession-Id: S-FIRST\nSession-Id: S-SECOND",
                "2026-06-01T00:00:00Z",
            )
            (commit,) = join._run_git_log(repo, sha, None, None)
            self.assertEqual(commit["session_id"], "S-FIRST")
            self.assertEqual(miner.session_id_of(repo, sha), "S-FIRST")


if __name__ == "__main__":
    unittest.main()
