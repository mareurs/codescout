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
from datetime import datetime, timedelta, timezone

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
# R50(e): anchored to the OPENING form at the start of a line (re.MULTILINE `^`), so the
# `\s*` between `<!--` and the literal token cannot skip over a closing marker's leading
# "end " -- `<!-- end auto-injected get_guide('X') -->` fails to match here for exactly
# that reason (verified against the real closing-marker shapes emitted by
# src/tools/core/guide_emit.rs), and a marker merely quoted mid-line (e.g. in a grep
# result) is never at the start of its own line either. operator-rule markers carry no
# closing form in this codebase (grepped: no "end operator-rule" emitter exists), but the
# same anchor is applied for symmetry and to exclude a mid-line quote of one.
OP_RULE_RE = re.compile(r"^[ \t]*<!--\s*operator-rule\s+(OP-\d+)", re.MULTILINE)
GUIDE_MARKER_RE = re.compile(r"^[ \t]*<!--\s*auto-injected get_guide\('([^']+)'\)", re.MULTILINE)

# R50(e) minor: the two marker-scanning loops in _deliveries_from_usage_row's output_json
# fallback were previously copy-pasted; this table drives one shared loop instead.
_OUTPUT_MARKER_PATTERNS = (("operator-rule", OP_RULE_RE), ("get_guide", GUIDE_MARKER_RE))

MCP_TOOL_PREFIX = "mcp__codescout__"
JOIN_WINDOW_SECONDS = 120

# R56: a hook_success and its hook_additional_context twin are at most this far apart. Measured
# on the 2026-09-27 snapshot: every twin pair falls within 1.713 s (Pre/PostToolUse and
# SubagentStart within 0.2 s), and the nearest same-text, same-file, same-event hac for a
# hook_success that is NOT a twin is 12.1 s away.
HOOK_TWIN_WINDOW_SECONDS = 5

# R44: the spec's own § Scope excludes its own design session from the census by default --
# "Excluded, each listed in the manifest with its reason: ... **this design session**
# (`3c5b02df`)" (docs/superpowers/specs/2026-09-26-system1-base-rate-measurement-design.md
# § Scope). R106 / Amendment 7(a)4: "sessions whose task was this measurement" also holds
# `82cff72e` (review work inside the decision window, self-disclosed). build_events records the
# resulting exclusions, with reasons, in the manifest (transcripts.write_exclusions), and
# transcripts.exclusions() propagates them to forks. A caller that needs no spec exclusion (e.g.
# a test fixture unrelated to the real corpus) passes excluded_sids=set() explicitly.
SPEC_EXCLUDED_SIDS = frozenset({
    "3c5b02df-b6ce-45f5-9d03-1194e38465c0",
    "82cff72e-0245-48cb-ab07-45a1c3d0d388",
})


def utc(ts):
    """Parse every source timestamp shape into an aware UTC datetime.

    Three shapes reach this function: usage.db's called_at/started_at ("YYYY-MM-DD
    HH:MM:SS[.ffffff]", space separator, no zone marker, but real UTC -- verified against a
    live server start); transcript timestamps (ISO "YYYY-MM-DDTHH:MM:SS[.ffffff]Z"); and,
    R50(a)/R110, git's `%aI` author dates -- ISO with a NUMERIC offset, e.g.
    "2026-09-14T10:00:00+02:00", which is never UTC by construction and so (unlike the
    other two shapes) is converted from its own offset rather than assumed UTC.
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
    # A numeric UTC offset in the last 6 characters (+HH:MM or -HH:MM) is git's %aI shape --
    # the space-separated usage.db shape never carries one, so this check discriminates the
    # two without needing to know which caller we're being asked by.
    if re.search(r"[+-]\d{2}:\d{2}$", s):
        dt = datetime.fromisoformat(s)
        return dt.astimezone(timezone.utc)
    s = s.replace("T", " ", 1)
    if "." in s:
        dt = datetime.strptime(s, "%Y-%m-%d %H:%M:%S.%f")
    else:
        dt = datetime.strptime(s, "%Y-%m-%d %H:%M:%S")
    return dt.replace(tzinfo=timezone.utc)


def _fmt_ts(ts):
    """R50(a): normalize any ts value through utc() into canonical ISO
    "YYYY-MM-DDTHH:MM:SS.mmmZ" (millisecond precision, always Z) before it is written to any
    `ts` column. A falsy input (None, "") passes through unchanged -- not every row carries a
    timestamp, and normalizing a missing value into a fake epoch would be worse than leaving
    it NULL. A value utc() cannot parse also passes through unchanged rather than aborting
    the whole build over one malformed timestamp -- no ruling calls for a hard failure here,
    and the real 137-transcript corpus is the thing this must survive running over.
    """
    if not ts:
        return ts
    try:
        dt = utc(ts)
    except (ValueError, TypeError):
        return ts
    return dt.strftime("%Y-%m-%dT%H:%M:%S.") + f"{dt.microsecond // 1000:03d}Z"


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
        CREATE TABLE IF NOT EXISTS events_meta (
            key TEXT PRIMARY KEY,
            value TEXT
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


def _entry_kind(entry, is_prompt, is_interrupt, is_delegation, is_rejection=False):
    """R43/R48: kind is one of
    prompt|interrupt|delegation|rejection|assistant_text|assistant_thinking|tool_use|
    tool_result|meta.

    is_prompt/is_interrupt/is_delegation/is_rejection come from `_classify_uuids`, computed once
    per file over ALL its entries (never derived from this single entry) -- interrupt outranks
    prompt, which outranks rejection, which outranks delegation, matching `_classify_uuids`' own
    precedence so the two can never disagree about the same uuid.

    R105: a rejection entry is a tool_result-shaped user entry; `rejection` REPLACES
    `tool_result` for it (R35: one row per uuid, so the entry gets exactly one kind).
    """
    if is_interrupt:
        return "interrupt"
    if is_prompt:
        return "prompt"
    if is_rejection:
        return "rejection"
    if is_delegation:
        return "delegation"
    etype = entry.get("type")
    message = entry.get("message")
    content = message.get("content") if isinstance(message, dict) else None
    if etype == "assistant":
        if isinstance(content, list) and any(
            isinstance(b, dict) and b.get("type") == "tool_use" for b in content
        ):
            return "tool_use"
        # R48: a thinking-only line has no extractable text -- `_message_text` only ever
        # returns non-None for a plain string or a first content block of type "text", so
        # None here means the entry's content is something else, and empirically (probe_7,
        # 82,677/82,677 over the real corpus) that something else is always a thinking
        # block. Never store thinking content anywhere: `text` stays NULL for this kind.
        if transcripts._message_text(entry) is None:
            return "assistant_thinking"
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
    # system, or anything else not otherwise classified (an attachment reaches this function
    # only as an R103 queued operator prompt, which is_prompt already returned).
    return "meta"


def _classify_uuids(entries, is_subagent):
    """R43: the ONE classification function both `_process_top_level_session` and
    `_process_subagent_union` call, replacing the copy-pasted logic previously at :384 and
    :458 (where C2 lived in the copy). Returns (prompt_uuids, interrupt_uuids,
    delegation_uuids, rejections), where `rejections` is {uuid: feedback text} (R105) --
    membership (`uuid in rejections`) is the classification, the value is the row's text.

    `operator_messages()`/`operator_interrupts()`/`operator_rejections()` apply to TOP-LEVEL
    entries only:
    - is_subagent=False (top-level): prompt_uuids from operator_messages() (R103: including a
      queued operator prompt's ATTACHMENT uuid), interrupt_uuids from operator_interrupts(),
      rejections from operator_rejections(), delegation_uuids always empty.
    - is_subagent=True (a subagent file): prompt_uuids and rejections are ALWAYS empty -- a
      subagent file contributes zero `prompt` rows by construction (C2's fix), and a rejection
      inside one stays `tool_result` (R105 is top-level only). interrupt_uuids is still
      computed from operator_interrupts() -- an exact interrupt marker stays `interrupt` even
      inside a subagent file. delegation_uuids is every `type == "user"` entry that is not
      already an interrupt uuid, not `isMeta`, not `isCompactSummary`, and not a
      tool_result-shaped user entry -- the parent model's brief, or a SendMessage
      continuation.
    """
    interrupt_uuids = {
        e.get("uuid") for e in transcripts.operator_interrupts(entries) if e.get("uuid") is not None
    }
    if not is_subagent:
        prompt_uuids = {
            e.get("uuid") for e in transcripts.operator_messages(entries) if e.get("uuid") is not None
        }
        rejections = {
            r["uuid"]: r["text"]
            for r in transcripts.operator_rejections(entries)
            if r["uuid"] is not None
        }
        return prompt_uuids, interrupt_uuids, set(), rejections

    delegation_uuids = set()
    for e in entries:
        uuid = e.get("uuid")
        if uuid is None or uuid in interrupt_uuids:
            continue
        if e.get("type") != "user":
            continue
        if e.get("isMeta") or e.get("isCompactSummary"):
            continue
        message = e.get("message")
        content = message.get("content") if isinstance(message, dict) else None
        if (
            isinstance(content, list)
            and content
            and isinstance(content[0], dict)
            and content[0].get("type") == "tool_result"
        ):
            continue
        delegation_uuids.add(uuid)
    return set(), interrupt_uuids, delegation_uuids, {}


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


def _deliveries_from_attachment(entry, cid, agent_path=None):
    """Amendment 2(a): hook executions appear as `attachment` entries. `hook_success` marks
    a hook ran (exit code/duration, toolUseID); injected text is recorded as
    `hook_additional_context`. A hook emitting nothing (e.g. a Stop hook with no output)
    records no content, and produces no delivery row here.

    R53/R56: Claude Code records some hook injections TWICE -- a `hook_success` attachment
    (whose stdout carries the context) and a `hook_additional_context` ("hac") attachment.
    The returned dict carries transient fields for the later twin pass
    (`_dedup_hook_deliveries`), all stripped before any row reaches the `deliveries` table:
    - `_atype`: the attachment's own `type`;
    - `_agent_path`: which transcript FILE the entry came from -- None for the top-level file,
      the subagent file's corpus-relative path otherwise (the same convention as
      `turns.agent_path`), so (sid, _agent_path) names exactly one file;
    - `_elem_sha256s`: the sha256 of each ELEMENT of a list-shaped `content`. A merged hac
      carries several hooks' texts as one list (measured: 609 SessionStart hacs with 2
      elements on the 2026-09-27 snapshot), and a hook_success twin's text equals ONE element,
      never the joined whole. A string-shaped or stdout-derived text is its own one element.
    """
    att = entry.get("attachment")
    if not isinstance(att, dict):
        return []
    atype = att.get("type")
    if atype not in ("hook_success", "hook_additional_context"):
        return []

    raw = att.get("rendered") or att.get("content") or None
    text = _coerce_attachment_text(raw)
    elements = raw if text and isinstance(raw, list) else None
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
    if elements is None:
        elements = [text]

    key = att.get("hookEvent") or att.get("hookName")
    return [
        {
            "sid": cid,
            "ts": _fmt_ts(entry.get("timestamp")),
            "source": "transcript_hook",
            "engine_or_hook": att.get("hookName"),
            "key": key,
            "sha256": hashlib.sha256(text.encode()).hexdigest(),
            "bytes": len(text.encode()),
            "tool_use_id": att.get("toolUseID"),
            "_atype": atype,
            "_agent_path": agent_path,
            "_elem_sha256s": [hashlib.sha256(x.encode()).hexdigest() for x in elements],
        }
    ]


def _deliveries_from_usage_row(row):
    """Brief: parse output_json injections from their markers; use deliveries_json wherever
    it is non-NULL. Amendment 2(b): deliveries_json is one entry per claiming engine --
    {engine, ledger_keys[], blocks[{sha256,bytes}], hint} -- with ledger_keys and blocks NOT
    positionally paired, so a key-row and a block-row are emitted separately here rather
    than inventing a pairing the source data does not make.

    R50(d): gated on `dj is not None` (field PRESENCE), never on truthiness -- a literal
    `'[]'` parses to an empty list and correctly yields zero rows here, and this branch
    never falls back to output_json once deliveries_json is present at all (even malformed
    or non-list JSON yields zero rows rather than reaching the fallback).
    """
    out = []
    dj = row.get("deliveries_json")
    ts = _fmt_ts(row.get("called_at"))
    if dj is not None:
        try:
            parsed = json.loads(dj)
        except (json.JSONDecodeError, TypeError):
            parsed = None
        if not isinstance(parsed, list):
            return out
        for rec in parsed:
            if not isinstance(rec, dict):
                continue
            engine = rec.get("engine")
            for key in rec.get("ledger_keys") or []:
                out.append(
                    {
                        "ts": ts,
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
                        "ts": ts,
                        "source": "usage_deliveries_json",
                        "engine_or_hook": engine,
                        "key": None,
                        "sha256": block.get("sha256"),
                        "bytes": block.get("bytes"),
                        "tool_use_id": row.get("tool_use_id"),
                    }
                )
        return out

    # Fallback: deliveries_json is NULL (field truly absent, not merely '[]') -- scan
    # output_json's text blocks for the two marker shapes via the shared pattern table,
    # R50(e) anchored to their OPENING form only.
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
        for engine_or_hook, pattern in _OUTPUT_MARKER_PATTERNS:
            for m in pattern.finditer(text):
                if digest is None:
                    digest = hashlib.sha256(text.encode()).hexdigest()
                    nbytes = len(text.encode())
                out.append(
                    {
                        "ts": ts,
                        "source": "usage_output_json",
                        "engine_or_hook": engine_or_hook,
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


def _write_turn(conn, counts, cid, agent_path, entry, kind, tokens_by_uuid, text=None):
    """One turns row. `text` defaults to transcripts._message_text(entry) -- for an R103 queued
    operator prompt that is its attachment.prompt; an R105 rejection passes its feedback text
    explicitly, since its entry's content is a tool_result. `role` is the entry's own `type`,
    so a queued prompt's row reads role 'attachment', kind 'prompt' -- which keeps the queued
    count derivable from the table (the readout discloses it, R103)."""
    uuid = entry.get("uuid")
    ts = _fmt_ts(entry.get("timestamp"))
    if text is None:
        text = transcripts._message_text(entry)
    message = entry.get("message") if isinstance(entry.get("message"), dict) else {}
    message_id = message.get("id")
    tokens = tokens_by_uuid.get(uuid, 0)
    # A plain INSERT, never OR IGNORE: the R45 uuid gate upstream is what guarantees one row
    # per (sid, uuid), so a second write means that gate failed. That must raise
    # (sqlite3.IntegrityError on the primary key), not be silently dropped while the
    # `turns` counter below still counts it.
    conn.execute(
        "INSERT INTO turns (sid, agent_path, uuid, ts, role, kind, text, "
        "message_id, tokens) VALUES (?,?,?,?,?,?,?,?,?)",
        (cid, agent_path, uuid, ts, entry.get("type"), kind, text, message_id, tokens),
    )
    counts["turns"] = counts.get("turns", 0) + 1


def _tool_use_candidates_from_entry(entry, cid, bare_sid, ts):
    out = []
    fmt_ts = _fmt_ts(ts)
    for block in _tool_use_blocks(entry):
        tinput = block.get("input") if isinstance(block.get("input"), dict) else {}
        out.append(
            {
                "cid": cid,
                "bare_sid": bare_sid,
                "agent_id": entry.get("agentId"),
                "tool_use_id": block.get("id"),
                "ts": fmt_ts,
                "name": block.get("name") or "",
                "input": tinput,
            }
        )
    return out


def _process_top_level_session(conn, session, cid, attribution, counts):
    """R28/R31/R35: a top-level entry is written to turns ONLY under its attribute_entries
    owner, so no uuid appears twice across a fork/duplicate pair. R45: the SAME attribution
    check also gates an attachment's delivery rows -- checked BEFORE a delivery is emitted,
    not after (the previous order let an attachment emit unconditionally, ahead of the
    attribution check). Every attribution miss increments the same skip counter.

    R103: an attachment that is a queued operator prompt (its uuid is in prompt_uuids) is a
    `prompt` turn, never a delivery; every other attachment stays a delivery source. R105: a
    rejection entry is written ONCE, as kind `rejection` with its feedback text.
    """
    read_stats = {}
    entries, skipped = transcripts.read_jsonl(session.path, stats=read_stats)
    counts["parse_errors_skipped"] = counts.get("parse_errors_skipped", 0) + skipped
    counts["decode_replaced_lines"] = (
        counts.get("decode_replaced_lines", 0) + read_stats["replaced_lines"]
    )
    prompt_uuids, interrupt_uuids, delegation_uuids, rejections = _classify_uuids(
        entries, is_subagent=False
    )
    tokens_by_uuid = _compute_tokens(entries)

    tool_use_candidates = []
    delivery_rows = []
    for e in entries:
        uuid = e.get("uuid")
        if uuid is None:
            continue
        if attribution.get(uuid) != cid:
            counts["top_level_attribution_skipped"] = (
                counts.get("top_level_attribution_skipped", 0) + 1
            )
            continue
        if e.get("type") == "attachment" and uuid not in prompt_uuids:
            delivery_rows.extend(_deliveries_from_attachment(e, cid))
            continue
        kind = _entry_kind(
            e, uuid in prompt_uuids, uuid in interrupt_uuids, uuid in delegation_uuids,
            uuid in rejections,
        )
        _write_turn(conn, counts, cid, None, e, kind, tokens_by_uuid, text=rejections.get(uuid))
        if e.get("type") == "attachment":
            counts["prompts_queued"] = counts.get("prompts_queued", 0) + 1
        if kind == "tool_use":
            tool_use_candidates.extend(
                _tool_use_candidates_from_entry(e, cid, session.sid, e.get("timestamp"))
            )
    return tool_use_candidates, delivery_rows


def _order_like_attribution(sessions_list, exclusions_map):
    """R45: subagent-file uuids are gated by a SINGLE global first-writer-wins set across ALL
    kept sessions (not reset per keeper), so its outcome depends on the ORDER kept sessions
    are visited in. `attribute_entries()` computes its 'most total uuids' owner by one global
    sort of candidate sessions -- (-count, first_ts, copy_id) -- but does not expose that
    order, only its resulting per-uuid dict. This function reproduces the identical order
    from the same inputs, so build_events' subagent pass visits sessions in the same
    priority attribute_entries used to decide TOP-LEVEL ownership.
    """
    candidates = [s for s in sessions_list if transcripts.copy_id(s) not in exclusions_map]
    counts = {}
    for s in candidates:
        entries, _skipped = transcripts.read_jsonl(s.path)
        counts[transcripts.copy_id(s)] = len(
            {e.get("uuid") for e in entries if e.get("uuid") is not None}
        )
    return sorted(
        candidates,
        key=lambda s: (-counts[transcripts.copy_id(s)], s.first_ts or "", transcripts.copy_id(s)),
    )



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


def _process_subagent_union(conn, sessions_list, excl, keeper, keeper_cid, corpus_dir, counts, seen_uuids):
    """R40: for each KEPT session, its subagent files are the union over every copy of that
    sid -- the keeper plus its duplicate-prefix-of:/divergent-duplicate-of: copies, with the
    keeper's own file winning a filename collision. R45: `seen_uuids` is a SINGLE set shared
    across ALL kept sessions' subagent passes -- passed in by the caller and mutated here,
    never reset per keeper -- so first-writer-wins is global rather than per keeper; the
    caller must iterate kept sessions in `_order_like_attribution`'s order for this to match
    attribute_entries' own priority. I6: a subagent attachment now emits its delivery rows
    (gated by the same global uuid set) instead of being silently dropped.
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
    delivery_rows = []
    for name, path in sorted(by_name.items()):
        agent_path = _rel_to_corpus(path, corpus_dir)
        read_stats = {}
        entries, skipped = transcripts.read_jsonl(path, stats=read_stats)
        counts["parse_errors_skipped"] = counts.get("parse_errors_skipped", 0) + skipped
        counts["decode_replaced_lines"] = (
            counts.get("decode_replaced_lines", 0) + read_stats["replaced_lines"]
        )
        prompt_uuids, interrupt_uuids, delegation_uuids, _rejections = _classify_uuids(
            entries, is_subagent=True
        )
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
                delivery_rows.extend(_deliveries_from_attachment(e, keeper_cid, agent_path))
                continue
            kind = _entry_kind(
                e, uuid in prompt_uuids, uuid in interrupt_uuids, uuid in delegation_uuids
            )
            _write_turn(conn, counts, keeper_cid, agent_path, e, kind, tokens_by_uuid)
            if recovered_source:
                counts["subagent_entries_recovered_from_duplicates"] = (
                    counts.get("subagent_entries_recovered_from_duplicates", 0) + 1
                )
            if kind == "tool_use":
                tool_use_candidates.extend(
                    _tool_use_candidates_from_entry(e, keeper_cid, keeper.sid, e.get("timestamp"))
                )
    return tool_use_candidates, delivery_rows


def _hook_twin_scope(d):
    """R56: the scope a hook_success and its hac twin must SHARE -- the same session copy
    (`sid`, the profile-qualified copy_id), the same transcript file (`_agent_path`: None for
    the top-level file, else the subagent file) and the same hook event (`key`). toolUseID is
    deliberately NOT part of it: measured on the 2026-09-27 snapshot, only Pre/PostToolUse
    twins share one. A SubagentStart twin carries a different uuid on each side (909 of 909),
    and every SessionStart hac carries the literal toolUseID "SessionStart" (666 of 666).
    """
    return (d.get("sid"), d.get("_agent_path"), d.get("key"))


def _is_hook_twin(hs, hac):
    """R56: True iff `hac` records the same injection as hook_success `hs`, given that the
    caller has already matched their `_hook_twin_scope`: the hs text EQUALS the hac text, or
    equals one ELEMENT of a merged multi-string hac, and |dts| <= HOOK_TWIN_WINDOW_SECONDS.
    A missing or unparseable ts is never a twin, since the window cannot be shown to hold.
    """
    if hs.get("sha256") != hac.get("sha256") and hs.get("sha256") not in hac.get("_elem_sha256s", ()):
        return False
    try:
        dt = abs((utc(hac["ts"]) - utc(hs["ts"])).total_seconds())
    except (KeyError, ValueError, TypeError, AttributeError):
        return False
    return dt <= HOOK_TWIN_WINDOW_SECONDS


def _dedup_hook_deliveries(delivery_rows, counts):
    """R56 (supersedes R53's key): among `transcript_hook` rows, a `hook_success` delivery is
    DROPPED as a twin iff some `hook_additional_context` row shares its `_hook_twin_scope` and
    `_is_hook_twin` holds; the drop is counted in `hook_success_twins_dropped`. Otherwise it is
    kept as the fallback record of that injection and counted in `hook_success_only` (measured:
    SessionStart:compact and UserPromptSubmit plain-stdout hooks never get a hac, and some
    SubagentStart hooks do not either).

    hac rows are ALWAYS kept: two hacs are never collapsed against each other, whatever their
    text or toolUseID, since each is its own injection. R53 keyed on (toolUseID, event, sha)
    with no session and no time, so all SessionStart hacs -- sharing toolUseID "SessionStart"
    -- were collapsed across sessions, deleting 333 real deliveries. Nothing here pairs across
    sessions or files. Non-`transcript_hook` rows (usage-row deliveries) pass through untouched.
    """
    hacs_by_scope = {}
    for d in delivery_rows:
        if d.get("source") == "transcript_hook" and d.get("_atype") == "hook_additional_context":
            hacs_by_scope.setdefault(_hook_twin_scope(d), []).append(d)

    out = []
    for d in delivery_rows:
        if d.get("source") != "transcript_hook":
            out.append(d)
            continue
        if d.get("_atype") == "hook_success":
            if any(_is_hook_twin(d, h) for h in hacs_by_scope.get(_hook_twin_scope(d), ())):
                counts["hook_success_twins_dropped"] = counts.get("hook_success_twins_dropped", 0) + 1
                continue
            counts["hook_success_only"] = counts.get("hook_success_only", 0) + 1
        out.append(d)

    for d in out:
        for transient in ("_atype", "_agent_path", "_elem_sha256s"):
            d.pop(transient, None)
    return out



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
                "input_json, output_json, deliveries_json, tool_use_id, latency_ms "
                "FROM tool_calls"
            )
            for row in cur:
                d = dict(row)
                d["_row_key"] = f"{db_path.name}:{d['id']}"
                rows.append(d)
        finally:
            conn.close()
    return rows


def _join_tool_events(conn, tool_use_candidates, usage_rows, counts, relations_map, cid_to_bare_sid):
    """R42/R46/R49: the heuristic join key, plus the exact/none/not_codescout split.

    R46: `join_method` is one of exact|heuristic|none|not_codescout -- `none` is a codescout
    call that failed to join, `not_codescout` is a tool_use that can never have a usage row.
    `not_codescout` rows are inserted directly under that join_method rather than falling
    through the generic "none" bucket and being undone afterward (removing the previous
    `-= 1` counter undo, which relied on `_insert`'s generic per-join_method counter never
    having been incremented for the wrong bucket in the first place).

    R49: when a candidate's owning copy is a fork (`relations_map` names it
    `fork-of:<target_cid>`), the heuristic bucket lookup also admits the fork TARGET's bare
    sid, because a fork's shared prefix was recorded in usage.db under the original session,
    not under the fork copy.
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
            _insert(c, None, "not_codescout")
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
        bucket_keys = [(c["bare_sid"], stripped_name)]
        fork_rel = relations_map.get(c["cid"])
        if fork_rel and fork_rel.startswith("fork-of:"):
            target_cid = fork_rel[len("fork-of:"):]
            target_bare_sid = cid_to_bare_sid.get(target_cid)
            if target_bare_sid is not None and target_bare_sid != c["bare_sid"]:
                bucket_keys.append((target_bare_sid, stripped_name))
        candidates = []
        for k in bucket_keys:
            candidates.extend(bucket.get(k, []))
        transcript_input = dict(c["input"])
        transcript_input.pop(AGENT_ID_STRIP_KEY, None)  # R4

        try:
            c_dt = utc(c["ts"]) if c["ts"] else None
        except ValueError:
            c_dt = None

        best = None
        best_diff = None
        best_via_called_at = None
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
            # R54: `started_at` is NULL on every usage row before 2026-09-20 19:17:34 UTC
            # (81% of the corpus), so the heuristic falls back to `called_at` -- measured
            # completion time -- minus `latency_ms` (called_at - started_at - latency_ms
            # averages ~11ms and is within 1s for 99.8% of rows carrying both). No
            # latency_ms leaves called_at un-adjusted rather than dropping the row.
            started_at = row.get("started_at")
            via_called_at = False
            time_key = started_at
            if not time_key:
                time_key = row.get("called_at")
                via_called_at = True
            if not time_key or c_dt is None:
                continue
            try:
                row_dt = utc(time_key)
            except ValueError:
                continue
            if via_called_at:
                latency_ms = row.get("latency_ms")
                if latency_ms:
                    row_dt = row_dt - timedelta(milliseconds=latency_ms)
            diff = abs((row_dt - c_dt).total_seconds())
            if diff > JOIN_WINDOW_SECONDS:
                continue
            if best is None or diff < best_diff:
                best = row
                best_diff = diff
                best_via_called_at = via_called_at

        if best is not None:
            claimed.add(best["_row_key"])
            _insert(c, best["_row_key"], "heuristic")
            if best_via_called_at:
                counts["heuristic_via_called_at"] = counts.get("heuristic_via_called_at", 0) + 1
        else:
            _insert(c, None, "none")


_GIT_FIELD_SEP = "\x1f"
# Leading marker, not a trailing one -- see _run_git_log's docstring for why.
_GIT_RECORD_SEP = "\x00"
# Bug 6708cab25f53b797: an explicit END-OF-MESSAGE marker written right after %B. --name-only's file
# list follows it, so the file list is located by this marker -- never by the last blank line,
# which for a merge (git prints no file list for one) is the start of the trailer block.
_GIT_BODY_END = "\x1e"


def _run_git_log(repo_path, sha, start_utc, end_utc):
    """R41/R50(b)/R110: `git log <sha> --name-only`, parsed into one dict per commit.

    The record separator goes at the FRONT of each commit's format string, not the back.
    `--name-only`'s file list is appended by git AFTER the whole pretty-printed commit
    (subject + body included) and BEFORE the next commit's formatted output begins -- so a
    separator placed after %B would split each commit's own header/body away from its own
    file list, which then lands at the START of the following split chunk instead. A leading
    marker keeps one commit's entire block (header, body, end marker, file list) together
    between one marker and the next. Measured empirically against a real two-commit fixture
    repo (2026-09-26) before trusting this -- the trailing-separator version was wrong.

    Bug 6708cab25f53b797: the file list is whatever follows the explicit end-of-message marker
    _GIT_BODY_END (written right after %B), so a commit git prints no file section for -- a
    merge (git log diffs a merge against nothing by default) or an --allow-empty commit -- has
    `files == []` and its trailer block stays in its message. A merge's `files` is therefore
    always [], not the files the merge brought in.

    R110 (one Session-Id definition with miner.session_id_of): `session_id` is git's own
    trailer parser, `%(trailers:key=Session-Id,valueonly)`, as a SEPARATE pretty-format field --
    its first non-empty line, stripped -- never a regex over %B, so a prose line that merely
    starts "Session-Id:" above the real trailer block loses. `ts` is the AUTHOR date %aI: a
    rebase (experiments is rebased after every ship) restamps committer dates, never author
    dates.

    R110: no `--since`/`--until`. git's --since stops walking at the first commit whose
    COMMITTER date is older than the bound, so a rebased history can hide in-window commits
    behind one restamped ancestor. The whole history from `sha` is walked and the window is
    filtered HERE, on the author date, half-open [start_utc, end_utc) like every other window
    (R71). A None bound is unbounded on that side.

    R113: stdout is decoded as UTF-8 with errors="replace", so a non-UTF-8 byte in a commit
    message cannot abort the build.

    R50(b): a git FAILURE (nonzero exit) returns `None`, distinct from `[]` -- a successful run
    that legitimately found zero commits in the retained window. The caller must not read a
    `None` as "no commits"; it increments `commits_git_failures` instead.
    """
    fmt = (
        f"%x00%H{_GIT_FIELD_SEP}%aI{_GIT_FIELD_SEP}%s{_GIT_FIELD_SEP}"
        f"%(trailers:key=Session-Id,valueonly){_GIT_FIELD_SEP}%B%x1e"
    )
    cmd = ["git", "-C", str(repo_path), "log", sha, "--name-only", f"--pretty=format:{fmt}"]
    proc = subprocess.run(
        cmd, capture_output=True, text=True, encoding="utf-8", errors="replace", check=False
    )
    if proc.returncode != 0:
        return None
    start_dt = utc(start_utc) if start_utc else None
    end_dt = utc(end_utc) if end_utc else None

    commits = []
    for rec in proc.stdout.split(_GIT_RECORD_SEP):
        if not rec.strip():
            continue
        # maxsplit 4: %B (the last field) may itself contain the field separator.
        parts = rec.split(_GIT_FIELD_SEP, 4)
        if len(parts) < 5:
            continue
        chash, adate, subject, trailer_values, rest = parts
        # rpartition: the LAST end marker, since git C-quotes any control byte in a file name
        # but a message body could in principle carry the marker byte itself.
        _body, marker, file_section = rest.rpartition(_GIT_BODY_END)
        if not marker:
            continue
        files = [ln for ln in file_section.split("\n") if ln.strip()]
        session_id = next(
            (ln.strip() for ln in trailer_values.splitlines() if ln.strip()), None
        )

        author_dt = utc(adate)
        if start_dt is not None and author_dt < start_dt:
            continue
        if end_dt is not None and not author_dt < end_dt:
            continue

        commits.append(
            {
                "sha": chash,
                "ts": _fmt_ts(adate),
                "subject": subject,
                "session_id": session_id,
                "files": files,
            }
        )
    return commits


def _build_commits(conn, manifest, repo_paths, counts):
    """R41/R50(b)/R110: for each repo named in the manifest, walk `git log <sha>` in full and keep
    the commits whose AUTHOR date lies in the manifest's retained bounds, half-open (the filter
    is _run_git_log's, in Python, never git's --since); read the Session-Id trailer with git's
    trailer parser and the --name-only file list after the explicit end-of-message marker.
    A manifest repo absent from repo_paths is skipped and counted, never an error.

    R50(b): `_run_git_log` returning `None` means the git invocation itself FAILED -- that is
    counted under `commits_git_failures` and produces no commit rows for that repo, rather than
    being read as "this repo has zero commits in the window" (which is what an empty list, `[]`,
    legitimately means).
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
        commits = _run_git_log(path, sha, start_utc, end_utc)
        if commits is None:
            counts["commits_git_failures"] = counts.get("commits_git_failures", 0) + 1
            continue
        for commit in commits:
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


def build_events(corpus_dir, events_db, repo_paths=None, excluded_sids=SPEC_EXCLUDED_SIDS):
    """Build the events database for a frozen corpus. Returns a dict of row counts.

    R44: `excluded_sids` defaults to `SPEC_EXCLUDED_SIDS` -- the spec's own § Scope excludes its
    own design session from the census by default. A caller that needs no spec exclusion (e.g. a
    test fixture unrelated to the real corpus) passes `excluded_sids=set()` explicitly.

    R47: refuses an existing `events_db` path outright (raises FileExistsError) rather than
    silently doubling every row on a re-run.
    """
    corpus_dir = pathlib.Path(corpus_dir)
    events_db = pathlib.Path(events_db)
    repo_paths = {k: pathlib.Path(v) for k, v in (repo_paths or {}).items()}

    if events_db.exists():
        raise FileExistsError(
            f"{events_db} already exists -- build_events refuses a silent re-run over an "
            "existing events database (R47). Delete it first if you mean to rebuild."
        )

    manifest = json.loads((corpus_dir / "manifest.json").read_text())

    # R44: the one spec-level force-exclude source is `excluded_sids` (SPEC_EXCLUDED_SIDS by
    # default), passed straight into transcripts.exclusions(), which also propagates it to forks
    # (R106). R106/R1: the resulting map, reasons included, is then RECORDED in the manifest via
    # transcripts.write_exclusions() -- the form observability.coverage() reads back
    # (transcripts.read_exclusions) and checks against its own recomputation. Any exclusions the
    # manifest carried before (archive.freeze writes []) are replaced, never read as input.
    sessions_list = transcripts.sessions(corpus_dir)
    excl = transcripts.exclusions(sessions_list, excluded_sids)
    transcripts.write_exclusions(corpus_dir, excl)
    attribution = transcripts.attribute_entries(sessions_list, excl)
    kept_sessions = [s for s in sessions_list if transcripts.copy_id(s) not in excl]
    relations_map = transcripts.relations(sessions_list, excl)
    cid_to_bare_sid = {transcripts.copy_id(s): s.sid for s in sessions_list}

    conn = sqlite3.connect(str(events_db))
    _create_schema(conn)

    counts = {
        "sessions_total": len(sessions_list),
        "sessions_kept": len(kept_sessions),
        "sessions_excluded": len(excl),
        # R107: kept sessions whose first and last entrypoint differ (class stays first-entry).
        "entrypoint_changed": transcripts.entrypoint_changed_count(sessions_list, excl),
        "turns": 0,
        # R103: queued operator prompts written as turns.kind='prompt' (role 'attachment').
        "prompts_queued": 0,
        "parse_errors_skipped": 0,
        # R113: transcript lines holding U+FFFD after read_jsonl's errors="replace" decode.
        "decode_replaced_lines": 0,
        "tool_events_total": 0,
        "tool_events_exact": 0,
        "tool_events_heuristic": 0,
        "tool_events_none": 0,
        "tool_events_not_codescout": 0,
        "deliveries_total": 0,
        "deliveries_unmapped_session": 0,
        "commits_total": 0,
        "commits_repos_missing": 0,
        "commits_git_failures": 0,
        "subagent_files_unioned": 0,
        "subagent_entries_recovered_from_duplicates": 0,
        "subagent_duplicate_uuids_skipped": 0,
        "top_level_attribution_skipped": 0,
        "hook_success_only": 0,
        "hook_success_twins_dropped": 0,
        "heuristic_via_called_at": 0,
    }

    all_tool_use_candidates = []
    delivery_rows = []

    # R45: subagent uuids are first-writer-wins across ALL kept sessions, gated by ONE set
    # shared across the whole loop (never reset per keeper); kept sessions are visited in
    # attribute_entries' own priority order so this pass agrees with top-level attribution about
    # which session "owns" a shared uuid first.
    seen_subagent_uuids = set()
    for session in _order_like_attribution(sessions_list, excl):
        cid = transcripts.copy_id(session)
        tool_use_candidates, session_deliveries = _process_top_level_session(
            conn, session, cid, attribution, counts
        )
        all_tool_use_candidates.extend(tool_use_candidates)
        delivery_rows.extend(session_deliveries)

        sub_tool_use_candidates, sub_delivery_rows = _process_subagent_union(
            conn, sessions_list, excl, session, cid, corpus_dir, counts, seen_subagent_uuids
        )
        all_tool_use_candidates.extend(sub_tool_use_candidates)
        delivery_rows.extend(sub_delivery_rows)

    # R56: drop each hook_success whose hook_additional_context twin (same session copy, file
    # and event; equal text or an equal element; |dts| <= 5 s) is present; hacs are all kept.
    delivery_rows = _dedup_hook_deliveries(delivery_rows, counts)

    # R50(c): a usage row whose session is out of the corpus or excluded (no kept session maps
    # to its bare cc_session_id) emits NOTHING and increments deliveries_unmapped_session --
    # never written under a bare or NULL sid.
    bare_to_cid = {s.sid: transcripts.copy_id(s) for s in kept_sessions}
    usage_rows = _load_usage_rows(corpus_dir)
    for row in usage_rows:
        cid = bare_to_cid.get(row.get("cc_session_id"))
        if cid is None:
            counts["deliveries_unmapped_session"] = counts.get("deliveries_unmapped_session", 0) + 1
            continue
        for d in _deliveries_from_usage_row(row):
            d["sid"] = cid
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

    _join_tool_events(conn, all_tool_use_candidates, usage_rows, counts, relations_map, cid_to_bare_sid)
    _build_commits(conn, manifest, repo_paths, counts)

    # R46: persist EVERY build counter into events_meta, where Task 13 reads them.
    for k, v in counts.items():
        conn.execute("INSERT OR REPLACE INTO events_meta (key, value) VALUES (?, ?)", (k, v))

    conn.commit()
    conn.close()
    return counts
