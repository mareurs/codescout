"""Stage 1b: the events database and joins.

Task 5 (transcripts.py) finds sessions, exclusions and per-entry attribution over a frozen
corpus. This module builds a SQLite events database joining those transcripts to codescout's
own usage.db, for later stages (correction miner, observability map, readout) to read.

Run: ~/work/claude/prompt-engineering/.venv/bin/python -m pytest tests/test_measure_join.py -v
"""
import hashlib
import json
import pathlib
import re
import sqlite3
import subprocess
import sys
from datetime import datetime, timezone

_THIS_DIR = pathlib.Path(__file__).resolve().parent
if str(_THIS_DIR) not in sys.path:
    sys.path.insert(0, str(_THIS_DIR))
import transcripts  # noqa: E402

# R4: usage.db stores tool input already stripped of this key; the transcript side still
# carries it, so the heuristic argument-equality check must strip it before comparing.
AGENT_ID_STRIP_KEY = "dev.codescout.mcp/agentId"

# The two marker shapes `output_json` injections carry (brief: "Parse output_json injections
# from their markers"). Grounded against a real row (usage.db id=143310, 2026-09-26):
#   <!-- operator-rule OP-4 — delivered once this session for this call shape; ... -->
# and against docs/superpowers guide injections:
#   <!-- auto-injected get_guide('project-activation-bootstrap') — first call this session ... -->
OP_RULE_RE = re.compile(r"<!--\s*operator-rule\s+(OP-\d+)")
GUIDE_MARKER_RE = re.compile(r"auto-injected get_guide\('([^']+)'\)")

MCP_TOOL_PREFIX = "mcp__codescout__"
JOIN_WINDOW_SECONDS = 120


def utc(ts):
    """Parse both source timestamp formats into an aware UTC datetime.

    usage.db's called_at/started_at: "YYYY-MM-DD HH:MM:SS[.ffffff]" (space separator, no
    zone marker, but real UTC — verified against a live server start). Transcript
    timestamps: ISO "YYYY-MM-DDTHH:MM:SS[.ffffff]Z". Normalizing the separator and stripping
    a trailing Z lets one strptime shape handle both.
    """
    s = ts.strip()
    if s.endswith("Z"):
        s = s[:-1]
    s = s.replace("T", " ", 1)
    if "." in s:
        dt = datetime.strptime(s, "%Y-%m-%d %H:%M:%S.%f")
    else:
        dt = datetime.strptime(s, "%Y-%m-%d %H:%M:%S")
    return dt.replace(tzinfo=timezone.utc)


def _create_schema(conn):
    conn.executescript(
        """
        CREATE TABLE IF NOT EXISTS turns (
            sid TEXT NOT NULL,
            agent_path TEXT,
            uuid TEXT NOT NULL,
            ts TEXT,
            role TEXT,
            kind TEXT,
            text TEXT,
            message_id TEXT,
            tokens INTEGER,
            PRIMARY KEY (sid, uuid)
        );
        CREATE TABLE IF NOT EXISTS tool_events (
            sid TEXT,
            tool_use_id TEXT,
            ts TEXT,
            name TEXT,
            input_json TEXT,
            usage_row_id TEXT,
            join_method TEXT
        );
        CREATE TABLE IF NOT EXISTS deliveries (
            sid TEXT,
            ts TEXT,
            source TEXT,
            engine_or_hook TEXT,
            key TEXT,
            sha256 TEXT,
            bytes INTEGER,
            tool_use_id TEXT
        );
        CREATE TABLE IF NOT EXISTS commits (
            repo TEXT,
            sha TEXT,
            ts TEXT,
            session_id TEXT,
            subject TEXT,
            files_json TEXT
        );
        """
    )
    conn.commit()


def _compute_tokens(entries):
    """Return {uuid: tokens} for one transcript file's entries.

    `tokens` is input + output + cache_creation from message.usage, counted ONCE per
    message.id: one completion spans several JSONL lines carrying the same usage dict (e.g.
    a text block and a tool_use block written as separate lines). The first entry (in file
    order) to report a given message.id gets the full count; later entries sharing that
    message.id get 0 — so summing `tokens` over rows equals summing over distinct
    message.ids, rather than multiplying by the number of lines that share one.
    """
    tokens_by_uuid = {}
    seen_message_ids = set()
    for e in entries:
        uuid = e.get("uuid")
        if uuid is None:
            continue
        message = e.get("message")
        if not isinstance(message, dict):
            continue
        usage = message.get("usage")
        if not isinstance(usage, dict):
            continue
        mid = message.get("id")
        if mid is not None and mid in seen_message_ids:
            tokens_by_uuid[uuid] = 0
            continue
        if mid is not None:
            seen_message_ids.add(mid)
        tokens_by_uuid[uuid] = (
            (usage.get("input_tokens") or 0)
            + (usage.get("output_tokens") or 0)
            + (usage.get("cache_creation_input_tokens") or 0)
        )
    return tokens_by_uuid


def _entry_kind(entry, is_prompt, is_interrupt):
    """R25/R35: kind is one of prompt|interrupt|assistant_text|tool_use|tool_result|meta."""
    if is_interrupt:
        return "interrupt"
    if is_prompt:
        return "prompt"
    etype = entry.get("type")
    message = entry.get("message")
    content = message.get("content") if isinstance(message, dict) else None
    if etype == "assistant":
        if isinstance(content, list) and any(
            isinstance(b, dict) and b.get("type") == "tool_use" for b in content
        ):
            return "tool_use"
        return "assistant_text"
    if etype == "user":
        if (
            isinstance(content, list)
            and content
            and isinstance(content[0], dict)
            and content[0].get("type") == "tool_result"
        ):
            return "tool_result"
        return "meta"
    # system, or anything else not otherwise classified (attachment is handled separately,
    # never reaching this function).
    return "meta"


def _tool_use_blocks(entry):
    message = entry.get("message")
    content = message.get("content") if isinstance(message, dict) else None
    if not isinstance(content, list):
        return []
    return [b for b in content if isinstance(b, dict) and b.get("type") == "tool_use"]


def _coerce_attachment_text(text):
    """Normalize hook attachment text before hashing it.

    Measured on the real corpus (task 6 real-data positive control): for
    `hook_success`/`hook_additional_context` attachments, `rendered`/`content` is sometimes a
    list of 1-2 strings rather than a single string -- multiple additionalContext injections
    delivered together, never empty and never containing a non-string element for these two
    attachment types. A list holding anything else (seen only under other attachment types
    this function's type guard already excludes, e.g. `task_reminder`'s list of todo dicts)
    is treated as absent rather than crashing on `.encode()`.
    """
    if isinstance(text, str):
        return text
    if isinstance(text, list) and text and all(isinstance(x, str) for x in text):
        return "\n\n".join(text)
    return None


def _deliveries_from_attachment(entry, cid):
    """Amendment 2(a): hook executions appear as `attachment` entries. `hook_success` marks
    a hook ran (exit code/duration, toolUseID); injected text is recorded as
    `hook_additional_context`. A hook emitting nothing (e.g. a Stop hook with no output)
    records no content, and produces no delivery row here.
    """
    att = entry.get("attachment")
    if not isinstance(att, dict):
        return []
    atype = att.get("type")
    if atype not in ("hook_success", "hook_additional_context"):
        return []

    text = _coerce_attachment_text(att.get("rendered") or att.get("content") or None)
    if not text:
        stdout = att.get("stdout")
        if isinstance(stdout, str) and stdout.strip():
            try:
                parsed = json.loads(stdout)
            except (json.JSONDecodeError, TypeError):
                parsed = None
            if isinstance(parsed, dict):
                hso = parsed.get("hookSpecificOutput")
                if isinstance(hso, dict):
                    text = hso.get("additionalContext") or None
    if not text:
        return []

    key = att.get("hookEvent") or att.get("hookName")
    return [
        {
            "sid": cid,
            "ts": entry.get("timestamp"),
            "source": "transcript_hook",
            "engine_or_hook": att.get("hookName"),
            "key": key,
            "sha256": hashlib.sha256(text.encode()).hexdigest(),
            "bytes": len(text.encode()),
            "tool_use_id": att.get("toolUseID"),
        }
    ]


def _deliveries_from_usage_row(row):
    """Brief: parse output_json injections from their markers; use deliveries_json wherever
    it is non-NULL. Amendment 2(b): deliveries_json is one entry per claiming engine —
    {engine, ledger_keys[], blocks[{sha256,bytes}], hint} — with ledger_keys and blocks NOT
    positionally paired, so a key-row and a block-row are emitted separately here rather
    than inventing a pairing the source data does not make.
    """
    out = []
    dj = row.get("deliveries_json")
    parsed = None
    if dj:
        try:
            parsed = json.loads(dj)
        except (json.JSONDecodeError, TypeError):
            parsed = None
    if parsed:
        for rec in parsed:
            if not isinstance(rec, dict):
                continue
            engine = rec.get("engine")
            for key in rec.get("ledger_keys") or []:
                out.append(
                    {
                        "ts": row.get("called_at"),
                        "source": "usage_deliveries_json",
                        "engine_or_hook": engine,
                        "key": key,
                        "sha256": None,
                        "bytes": None,
                        "tool_use_id": row.get("tool_use_id"),
                    }
                )
            for block in rec.get("blocks") or []:
                if not isinstance(block, dict):
                    continue
                out.append(
                    {
                        "ts": row.get("called_at"),
                        "source": "usage_deliveries_json",
                        "engine_or_hook": engine,
                        "key": None,
                        "sha256": block.get("sha256"),
                        "bytes": block.get("bytes"),
                        "tool_use_id": row.get("tool_use_id"),
                    }
                )
        return out

    # Fallback: deliveries_json is NULL/empty — scan output_json's text blocks for markers.
    oj = row.get("output_json")
    if not oj:
        return out
    try:
        blocks = json.loads(oj)
    except (json.JSONDecodeError, TypeError):
        return out
    if not isinstance(blocks, list):
        return out
    for b in blocks:
        if not isinstance(b, dict):
            continue
        text = b.get("text")
        if not isinstance(text, str):
            continue
        digest = None
        nbytes = None
        for m in OP_RULE_RE.finditer(text):
            if digest is None:
                digest = hashlib.sha256(text.encode()).hexdigest()
                nbytes = len(text.encode())
            out.append(
                {
                    "ts": row.get("called_at"),
                    "source": "usage_output_json",
                    "engine_or_hook": "operator-rule",
                    "key": m.group(1),
                    "sha256": digest,
                    "bytes": nbytes,
                    "tool_use_id": row.get("tool_use_id"),
                }
            )
        for m in GUIDE_MARKER_RE.finditer(text):
            if digest is None:
                digest = hashlib.sha256(text.encode()).hexdigest()
                nbytes = len(text.encode())
            out.append(
                {
                    "ts": row.get("called_at"),
                    "source": "usage_output_json",
                    "engine_or_hook": "get_guide",
                    "key": m.group(1),
                    "sha256": digest,
                    "bytes": nbytes,
                    "tool_use_id": row.get("tool_use_id"),
                }
            )
    return out


def _rel_to_corpus(path, corpus_dir):
    try:
        return str(pathlib.Path(path).relative_to(corpus_dir))
    except ValueError:
        return str(path)


def _write_turn(conn, counts, cid, agent_path, entry, kind, tokens_by_uuid):
    uuid = entry.get("uuid")
    ts = entry.get("timestamp")
    text = transcripts._message_text(entry)
    message = entry.get("message") if isinstance(entry.get("message"), dict) else {}
    message_id = message.get("id")
    tokens = tokens_by_uuid.get(uuid, 0)
    conn.execute(
        "INSERT OR IGNORE INTO turns (sid, agent_path, uuid, ts, role, kind, text, "
        "message_id, tokens) VALUES (?,?,?,?,?,?,?,?,?)",
        (cid, agent_path, uuid, ts, entry.get("type"), kind, text, message_id, tokens),
    )
    counts["turns"] = counts.get("turns", 0) + 1


def _tool_use_candidates_from_entry(entry, cid, bare_sid, ts):
    out = []
    for block in _tool_use_blocks(entry):
        tinput = block.get("input") if isinstance(block.get("input"), dict) else {}
        out.append(
            {
                "cid": cid,
                "bare_sid": bare_sid,
                "agent_id": entry.get("agentId"),
                "tool_use_id": block.get("id"),
                "ts": ts,
                "name": block.get("name") or "",
                "input": tinput,
            }
        )
    return out


def _process_top_level_session(conn, session, cid, attribution, counts):
    """R28/R31/R35: a top-level entry is written to turns ONLY under its attribute_entries
    owner, so no uuid appears twice across a fork/duplicate pair.
    """
    entries, skipped = transcripts.read_jsonl(session.path)
    counts["parse_errors_skipped"] = counts.get("parse_errors_skipped", 0) + skipped
    prompt_uuids = {
        e.get("uuid") for e in transcripts.operator_messages(entries) if e.get("uuid") is not None
    }
    interrupt_uuids = {
        e.get("uuid") for e in transcripts.operator_interrupts(entries) if e.get("uuid") is not None
    }
    tokens_by_uuid = _compute_tokens(entries)

    tool_use_candidates = []
    delivery_rows = []
    for e in entries:
        uuid = e.get("uuid")
        if uuid is None:
            continue
        if e.get("type") == "attachment":
            delivery_rows.extend(_deliveries_from_attachment(e, cid))
            continue
        if attribution.get(uuid) != cid:
            continue
        kind = _entry_kind(e, uuid in prompt_uuids, uuid in interrupt_uuids)
        _write_turn(conn, counts, cid, None, e, kind, tokens_by_uuid)
        if kind == "tool_use":
            tool_use_candidates.extend(
                _tool_use_candidates_from_entry(e, cid, session.sid, e.get("timestamp"))
            )
    return tool_use_candidates, delivery_rows


def _subagent_siblings(sessions_list, excl, keeper, keeper_cid):
    """R40: the keeper's sibling copies of the SAME sid — those excluded specifically as
    duplicate-prefix-of:/divergent-duplicate-of: the keeper. A copy of the same sid excluded
    for sdk-cli/scratchpad-project/excluded-by-spec contributes nothing (its own reason
    already says so), so it is not a sibling for this purpose.
    """
    siblings = []
    for s in sessions_list:
        if s.sid != keeper.sid:
            continue
        c = transcripts.copy_id(s)
        if c == keeper_cid:
            continue
        reason = excl.get(c, "")
        if reason.startswith("duplicate-prefix-of:") or reason.startswith(
            "divergent-duplicate-of:"
        ):
            siblings.append(s)
    siblings.sort(key=transcripts.copy_id)
    return siblings


def _process_subagent_union(conn, sessions_list, excl, keeper, keeper_cid, corpus_dir, counts):
    """R40: for each KEPT session, its subagent files are the union over every copy of that
    sid — the keeper plus its duplicate-prefix-of:/divergent-duplicate-of: copies, with the
    keeper's own file winning a filename collision. Entries are deduplicated by uuid across
    all unioned files.
    """
    siblings = _subagent_siblings(sessions_list, excl, keeper, keeper_cid)

    by_name = {}
    for p in keeper.subagent_paths:
        by_name.setdefault(p.name, p)
    keeper_names = set(by_name.keys())
    for sib in siblings:
        for p in sib.subagent_paths:
            by_name.setdefault(p.name, p)

    counts["subagent_files_unioned"] = counts.get("subagent_files_unioned", 0) + len(by_name)

    tool_use_candidates = []
    seen_uuids = set()
    for name, path in sorted(by_name.items()):
        agent_path = _rel_to_corpus(path, corpus_dir)
        entries, skipped = transcripts.read_jsonl(path)
        counts["parse_errors_skipped"] = counts.get("parse_errors_skipped", 0) + skipped
        prompt_uuids = {
            e.get("uuid")
            for e in transcripts.operator_messages(entries)
            if e.get("uuid") is not None
        }
        interrupt_uuids = {
            e.get("uuid")
            for e in transcripts.operator_interrupts(entries)
            if e.get("uuid") is not None
        }
        tokens_by_uuid = _compute_tokens(entries)
        recovered_source = name not in keeper_names

        for e in entries:
            uuid = e.get("uuid")
            if uuid is None:
                continue
            if uuid in seen_uuids:
                counts["subagent_duplicate_uuids_skipped"] = (
                    counts.get("subagent_duplicate_uuids_skipped", 0) + 1
                )
                continue
            seen_uuids.add(uuid)
            if e.get("type") == "attachment":
                # Subagent hook deliveries are rare in practice; skip rather than guess.
                continue
            kind = _entry_kind(e, uuid in prompt_uuids, uuid in interrupt_uuids)
            _write_turn(conn, counts, keeper_cid, agent_path, e, kind, tokens_by_uuid)
            if recovered_source:
                counts["subagent_entries_recovered_from_duplicates"] = (
                    counts.get("subagent_entries_recovered_from_duplicates", 0) + 1
                )
            if kind == "tool_use":
                tool_use_candidates.extend(
                    _tool_use_candidates_from_entry(e, keeper_cid, keeper.sid, e.get("timestamp"))
                )
    return tool_use_candidates


def _find_usage_dbs(corpus_dir):
    d = pathlib.Path(corpus_dir) / "usage_dbs"
    if not d.is_dir():
        return []
    return sorted(d.glob("*.db"))


def _load_usage_rows(corpus_dir):
    rows = []
    for db_path in _find_usage_dbs(corpus_dir):
        conn = sqlite3.connect(str(db_path))
        conn.row_factory = sqlite3.Row
        try:
            cur = conn.execute(
                "SELECT id, tool_name, called_at, started_at, cc_session_id, agent_id, "
                "input_json, output_json, deliveries_json, tool_use_id FROM tool_calls"
            )
            for row in cur:
                d = dict(row)
                d["_row_key"] = f"{db_path.name}:{d['id']}"
                rows.append(d)
        finally:
            conn.close()
    return rows


def _join_tool_events(conn, tool_use_candidates, usage_rows, counts):
    """R42: the heuristic join key. The exact join (Task 1) is on tool_use_id — usage.db
    recorded that column NULL on every row until a fix deployed 2026-09-26 (bug
    6e14221db0c20de9), so on the real corpus nearly everything falls through to heuristic.
    That is expected, not a defect.
    """
    usage_by_tool_use_id = {}
    bucket = {}
    for row in usage_rows:
        tid = row.get("tool_use_id")
        if tid:
            usage_by_tool_use_id.setdefault(tid, []).append(row)
        bucket.setdefault((row.get("cc_session_id"), row.get("tool_name")), []).append(row)

    claimed = set()

    def _insert(c, usage_row_id, join_method):
        conn.execute(
            "INSERT INTO tool_events (sid, tool_use_id, ts, name, input_json, usage_row_id, "
            "join_method) VALUES (?,?,?,?,?,?,?)",
            (c["cid"], c["tool_use_id"], c["ts"], c["name"], json.dumps(c["input"]), usage_row_id, join_method),
        )
        counts["tool_events_total"] = counts.get("tool_events_total", 0) + 1
        counts[f"tool_events_{join_method}"] = counts.get(f"tool_events_{join_method}", 0) + 1

    codescout_candidates = []
    for c in tool_use_candidates:
        if not c["name"].startswith(MCP_TOOL_PREFIX):
            _insert(c, None, "none")
            counts["tool_events_none_not_codescout"] = (
                counts.get("tool_events_none_not_codescout", 0) + 1
            )
            # Undo the generic "none" bucket increment above: this row is not a join
            # failure, it is a tool that was never a join candidate at all.
            counts["tool_events_none"] -= 1
        else:
            codescout_candidates.append(c)

    remaining = []
    for c in codescout_candidates:
        tid = c["tool_use_id"]
        matched = None
        if tid:
            for row in usage_by_tool_use_id.get(tid, []):
                if row["_row_key"] not in claimed:
                    matched = row
                    break
        if matched is not None:
            claimed.add(matched["_row_key"])
            _insert(c, matched["_row_key"], "exact")
        else:
            remaining.append(c)

    def _sort_key(c):
        try:
            dt = utc(c["ts"]) if c["ts"] else datetime.min.replace(tzinfo=timezone.utc)
        except ValueError:
            dt = datetime.min.replace(tzinfo=timezone.utc)
        return (dt, c["tool_use_id"] or "")

    remaining.sort(key=_sort_key)

    for c in remaining:
        stripped_name = c["name"][len(MCP_TOOL_PREFIX):]
        candidates = bucket.get((c["bare_sid"], stripped_name), [])
        transcript_input = dict(c["input"])
        transcript_input.pop(AGENT_ID_STRIP_KEY, None)  # R4

        try:
            c_dt = utc(c["ts"]) if c["ts"] else None
        except ValueError:
            c_dt = None

        best = None
        best_diff = None
        for row in candidates:
            if row["_row_key"] in claimed:
                continue
            usage_agent_id = row.get("agent_id")
            if usage_agent_id:  # hard key only when non-NULL
                if c["agent_id"] != usage_agent_id:
                    continue
            usage_input_raw = row.get("input_json")
            try:
                usage_input = json.loads(usage_input_raw) if usage_input_raw else {}
            except (json.JSONDecodeError, TypeError):
                continue
            if usage_input != transcript_input:
                continue
            started_at = row.get("started_at")
            if not started_at or c_dt is None:
                continue
            try:
                row_dt = utc(started_at)
            except ValueError:
                continue
            diff = abs((row_dt - c_dt).total_seconds())
            if diff > JOIN_WINDOW_SECONDS:
                continue
            if best is None or diff < best_diff:
                best = row
                best_diff = diff

        if best is not None:
            claimed.add(best["_row_key"])
            _insert(c, best["_row_key"], "heuristic")
        else:
            _insert(c, None, "none")


_GIT_FIELD_SEP = "\x1f"
# Leading marker, not a trailing one -- see _run_git_log's docstring for why.
_GIT_RECORD_SEP = "\x00"


def _run_git_log(repo_path, sha, start_utc, end_utc):
    """R41: `git log <sha> --name-only`, parsed into one dict per commit.

    The record separator goes at the FRONT of each commit's format string, not the back.
    `--name-only`'s file list is appended by git AFTER the whole pretty-printed commit
    (subject + body included) and BEFORE the next commit's formatted output begins -- so a
    separator placed after %B would split each commit's own header/body away from its own
    file list, which then lands at the START of the following split chunk instead. A leading
    marker keeps one commit's entire block (header, body, blank line, file list) together
    between one marker and the next. Measured empirically against a real two-commit fixture
    repo (2026-09-26) before trusting this -- the trailing-separator version was wrong.
    """
    fmt = f"%x00%H{_GIT_FIELD_SEP}%cI{_GIT_FIELD_SEP}%s{_GIT_FIELD_SEP}%B"
    cmd = ["git", "-C", str(repo_path), "log", sha, "--name-only", f"--pretty=format:{fmt}"]
    if start_utc:
        cmd.append(f"--since={start_utc}")
    if end_utc:
        cmd.append(f"--until={end_utc}")
    proc = subprocess.run(cmd, capture_output=True, text=True, check=False)
    if proc.returncode != 0:
        return []

    commits = []
    for rec in proc.stdout.split(_GIT_RECORD_SEP):
        if not rec.strip():
            continue
        parts = rec.split(_GIT_FIELD_SEP, 3)
        if len(parts) < 4:
            continue
        chash, cdate, subject, rest = parts

        # %B's own trailing newline plus --name-only's forced blank-line separator leave a
        # variable number of trailing newlines depending on whether this is the last commit
        # in the stream (no next marker to absorb it against); rstrip normalizes that before
        # the last-blank-line split.
        rest = rest.rstrip("\n")
        lines = rest.split("\n")
        last_blank = -1
        for i, ln in enumerate(lines):
            if ln == "":
                last_blank = i
        if last_blank >= 0:
            body_lines = lines[:last_blank]
            files = [ln for ln in lines[last_blank + 1 :] if ln.strip()]
        else:
            # No blank line at all: a single-line message and (per the known limitation
            # below) indistinguishable from a zero-file commit either way.
            body_lines = lines
            files = []

        # Known limitation, not fixed here: a commit that touches NO files but carries a
        # multi-paragraph body is indistinguishable from one whose last paragraph IS the
        # file list, since --name-only omits the file section entirely rather than leaving
        # it empty. Every real commit in this repo's convention touches at least one
        # tracked file, so this is out of scope rather than silently mis-parsed in practice.
        m = re.search(r"^Session-Id:\s*(\S+)", "\n".join(body_lines), re.MULTILINE)
        session_id = m.group(1) if m else None

        commits.append(
            {"sha": chash, "ts": cdate, "subject": subject, "session_id": session_id, "files": files}
        )
    return commits


def _build_commits(conn, manifest, repo_paths, counts):
    """R41: for each repo named in the manifest, run `git log <sha>` limited to the
    manifest's retained bounds; parse the Session-Id: trailer and the --name-only file list.
    A manifest repo absent from repo_paths is skipped and counted, never an error.
    """
    repos = manifest.get("repos", {}) or {}
    bounds = ((manifest.get("bounds") or {}).get("retained")) or {}
    start_utc = bounds.get("start_utc")
    end_utc = bounds.get("end_utc")
    repo_paths = repo_paths or {}

    for name, sha in repos.items():
        path = repo_paths.get(name)
        if not path:
            counts["commits_repos_missing"] = counts.get("commits_repos_missing", 0) + 1
            continue
        for commit in _run_git_log(path, sha, start_utc, end_utc):
            conn.execute(
                "INSERT INTO commits (repo, sha, ts, session_id, subject, files_json) "
                "VALUES (?,?,?,?,?,?)",
                (
                    name,
                    commit["sha"],
                    commit["ts"],
                    commit["session_id"],
                    commit["subject"],
                    json.dumps(commit["files"]),
                ),
            )
            counts["commits_total"] = counts.get("commits_total", 0) + 1


def build_events(corpus_dir, events_db, repo_paths=None):
    """Build the events database for a frozen corpus. Returns a dict of row counts."""
    corpus_dir = pathlib.Path(corpus_dir)
    events_db = pathlib.Path(events_db)
    repo_paths = {k: pathlib.Path(v) for k, v in (repo_paths or {}).items()}

    manifest = json.loads((corpus_dir / "manifest.json").read_text())

    # manifest["exclusions"] is write-only from build_events' point of view: archive.freeze()
    # always emits an empty list, and its one known writer (transcripts.write_exclusions())
    # produces copy_id-keyed {"sid", "reason"} records — output shape, not the bare-sid
    # force-exclude SET transcripts.exclusions() takes as input. No spec-level force-exclude
    # source is named anywhere in Task 5/6's interfaces, so build_events recomputes exclusions
    # fresh from sessions_list rather than reading (or misreading) that manifest field.
    sessions_list = transcripts.sessions(corpus_dir)
    excl = transcripts.exclusions(sessions_list, set())
    attribution = transcripts.attribute_entries(sessions_list, excl)
    kept_sessions = [s for s in sessions_list if transcripts.copy_id(s) not in excl]

    conn = sqlite3.connect(str(events_db))
    _create_schema(conn)

    counts = {
        "sessions_total": len(sessions_list),
        "sessions_kept": len(kept_sessions),
        "sessions_excluded": len(excl),
        "turns": 0,
        "parse_errors_skipped": 0,
        "tool_events_total": 0,
        "tool_events_exact": 0,
        "tool_events_heuristic": 0,
        "tool_events_none": 0,
        "tool_events_none_not_codescout": 0,
        "deliveries_total": 0,
        "commits_total": 0,
        "commits_repos_missing": 0,
        "subagent_files_unioned": 0,
        "subagent_entries_recovered_from_duplicates": 0,
        "subagent_duplicate_uuids_skipped": 0,
    }

    all_tool_use_candidates = []
    delivery_rows = []

    for session in kept_sessions:
        cid = transcripts.copy_id(session)
        tool_use_candidates, session_deliveries = _process_top_level_session(
            conn, session, cid, attribution, counts
        )
        all_tool_use_candidates.extend(tool_use_candidates)
        delivery_rows.extend(session_deliveries)

        all_tool_use_candidates.extend(
            _process_subagent_union(conn, sessions_list, excl, session, cid, corpus_dir, counts)
        )

    bare_to_cid = {s.sid: transcripts.copy_id(s) for s in kept_sessions}
    usage_rows = _load_usage_rows(corpus_dir)
    for row in usage_rows:
        for d in _deliveries_from_usage_row(row):
            d["sid"] = bare_to_cid.get(row.get("cc_session_id"), row.get("cc_session_id"))
            delivery_rows.append(d)

    for d in delivery_rows:
        conn.execute(
            "INSERT INTO deliveries (sid, ts, source, engine_or_hook, key, sha256, bytes, "
            "tool_use_id) VALUES (?,?,?,?,?,?,?,?)",
            (
                d.get("sid"),
                d.get("ts"),
                d.get("source"),
                d.get("engine_or_hook"),
                d.get("key"),
                d.get("sha256"),
                d.get("bytes"),
                d.get("tool_use_id"),
            ),
        )
        counts["deliveries_total"] += 1

    _join_tool_events(conn, all_tool_use_candidates, usage_rows, counts)
    _build_commits(conn, manifest, repo_paths, counts)

    conn.commit()
    conn.close()
    return counts
