"""Stage 1c: the observability map (Task 13, fix round 2).

Spec A1.6 requires this map before any Codex-judge call: for every outcome and every
causal-chain link, a label (measurable now / needs adjudication / unobservable) plus the
coverage number from the events database that justifies it, or the reason none exists.

Consumes Task 6's events.db (scripts/measure/join.py), Task 5's transcript helpers
(scripts/measure/transcripts.py) and Task 4's frozen-corpus manifest (scripts/measure/archive.py);
none of the three is modified here. Produces:
- earliest_kept_top_level_ts(corpus_dir) -> str and finalize_bounds(corpus_dir) -> dict (R71):
  the pipeline, not a scratch driver, owns the two windows. Task 12 calls finalize_bounds right
  after archive.freeze and before join.build_events (whose commits table reads the bounds).
- coverage(events_db, corpus_dir) -> dict: every count the map needs, per DECISION window and
  RETAINED window (R62), plus the A1.6 appendix breakdowns.
- render_map(coverage, manifest, code_version=None) -> str: the WHOLE document (R60, R68). The
  committed map file must be byte-identical to this function's output; it is never hand-edited.

Fix round 2 (rulings R68-R71):
- R70, strict reads: every read of a coverage, events_meta, manifest or closed-set key (turn kind,
  join method, delivery source) is a subscript, never `.get(..., default)`. A missing key RAISES;
  only a PRESENT 0 renders 0. A closed-set value the build emits but this module does not declare
  also raises, so a schema change cannot silently drop rows from a table. (A raw transcript
  entry's optional fields -- uuid, timestamp, type -- are source data, not keys this module
  owns, and are the only `.get` reads left.)
- R71, windows: every membership test is the one half-open predicate `_in_window`, [start, end).
  NULL and unparseable ts rows are skipped and counted, per table and per cause, and rendered.
- R69, deliveries count DELIVERED ITEMS, not table rows (`_is_delivered_item`).
- R68, the header is rendered from data plus one fixed rule sentence (PROVISIONAL_SENTENCE).

Fix round 3 (rulings R75-R77): finalize_bounds ends both windows at floor(created_utc) + 1 s
(R75); coverage() also raises on an events-DB row before retained.start (R76) and on a
joins-by-method row count that disagrees with events_meta (R77).

Run: ~/work/claude/prompt-engineering/.venv/bin/python -m pytest tests/test_measure_observability.py -v
"""
import collections
import json
import pathlib
import sqlite3
import subprocess
import sys
from datetime import timedelta

_THIS_DIR = pathlib.Path(__file__).resolve().parent
if str(_THIS_DIR) not in sys.path:
    sys.path.insert(0, str(_THIS_DIR))
import join  # noqa: E402
import transcripts  # noqa: E402

# R68: the rendering code's version is read from the repo that holds this scripts/measure dir.
_REPO_ROOT = _THIS_DIR.parents[1]

# Empirically confirmed (2026-09-27, against the real corpus): the subagent-launching tool is
# literally named "Agent" in transcripts -- not "Task". 95/95 sampled files matched "Agent";
# none matched "Task".
AGENT_TOOL_NAME = "Agent"

# R63: the transfer delivery filter is ONE named constant. A usage_output_json-derived delivery
# carries a marker name in engine_or_hook ("operator-rule" / "get_guide" -- see
# join._deliveries_from_usage_row); a usage_deliveries_json-derived one carries a literal
# engine id from src/engines/mod.rs ("operator-rules" / "guide-sections" / "session-opener").
# Both write into the same deliveries.engine_or_hook column. "craft-skills" is a real engine id
# in src/engines/mod.rs but is deliberately EXCLUDED here -- its own test suite says it "must
# stay registered while it remains uncoordinated", i.e. it is not yet a transfer-carrying path.
TRANSFER_DELIVERY_MARKERS = frozenset(
    {"operator-rule", "get_guide", "operator-rules", "guide-sections", "session-opener"}
)

# R70: the closed value sets of join.py's schema (R46 and join._entry_kind). Every one is
# zero-filled up front so an absent value is a PRESENT 0; a value outside the set raises.
TURN_KINDS = (
    "prompt", "interrupt", "delegation", "assistant_text", "assistant_thinking", "tool_use",
    "tool_result", "meta",
)
JOIN_METHODS = ("exact", "heuristic", "none", "not_codescout")

# R69: what ONE delivered item is, per deliveries.source. Measured on the 2026-09-27 scratch
# snapshot before this was written (probes/task13-fix2-real.txt): join.py emits, for each
# usage_deliveries_json engine record, one row per ledger key (key set, sha256 NULL) AND one row
# per block (key NULL, sha256 set) -- 35 + 35 rows for 35 deliveries, every record carrying as
# many blocks as keys. usage_output_json and transcript_hook had zero NULL-key rows; by
# construction a transcript_hook row is one attachment whatever its key, so it always counts.
DELIVERY_UNITS = {
    "usage_output_json": "one per operator-rule or get_guide marker match in output_json",
    "usage_deliveries_json": (
        "one per ledger key of a deliveries_json engine record; its block rows (key NULL) are "
        "digests of the same deliveries and are not counted"
    ),
    "transcript_hook": "one per hook injection (a hook_success or hook_additional_context row)",
}
DELIVERY_SOURCES = tuple(sorted(DELIVERY_UNITS))
DELIVERY_UNIT_PHRASE = (
    "delivered items: one per output_json marker match, one per deliveries_json ledger key "
    "(block rows not counted), one per transcript hook injection"
)

# A1.2: the decision window is the last 7 days of the retained window.
DECISION_WINDOW_DAYS = 7

# R68: the ONE fixed sentence the header carries, true of every build.
PROVISIONAL_SENTENCE = (
    "A map from any corpus other than the Task 12 freeze is provisional; Task 12 regenerates "
    "this file from the frozen real corpus."
)

_LINKS = ("opportunity", "signal/request", "delivery/action", "observed use", "checked outcome")
_OUTCOMES = ("mistakes", "context", "transfer", "background-worker")
_TS_TABLES = ("turns", "tool_events", "deliveries")

# R58 (controller ruling): the map's BASE labels, as a table -- not scattered ifs. `field`
# names a key into coverage()["fields"]; a cell with field=None has no computable aggregate
# and its `basis` text stands in as the reason none exists -- always a full reason sentence,
# never a bare judge-field name. A base label of "measurable now" whose field's DECISION-BEARING
# window count is 0 is downgraded to "needs adjudication" by _resolve_cell() below (R62) --
# never encode that downgrade here.
_BASE_TABLE = {
    ("mistakes", "opportunity"): {
        "label": "measurable now",
        "basis": "decision points = assistant_text turns",
        "field": "assistant_text_turns",
    },
    ("mistakes", "signal/request"): {
        "label": "needs adjudication",
        "basis": "prompt + interrupt rows are the candidate population; whether each is a "
                 "correction is judged",
        "field": "prompt_interrupt_turns",
    },
    ("mistakes", "delivery/action"): {
        "label": "needs adjudication",
        "basis": "deliveries before the decision exist, but their relevance to the mistake is "
                 "judged, not counted",
        "field": None,
    },
    ("mistakes", "observed use"): {
        "label": "needs adjudication",
        "basis": "whether the assistant's next action aligned with the correction is judged "
                 "(Amendment 2(c))",
        "field": None,
    },
    ("mistakes", "checked outcome"): {
        "label": "needs adjudication",
        "basis": "the Codex judge, plus the operator's 25-item spot-check, decide this",
        "field": None,
    },
    ("context", "opportunity"): {
        "label": "measurable now",
        "basis": "assistant_text turns",
        "field": "assistant_text_turns",
    },
    ("context", "signal/request"): {
        "label": "measurable now",
        "basis": "tool_use blocks (tool_events total)",
        "field": "tool_events_total",
    },
    ("context", "delivery/action"): {
        "label": "measurable now",
        "basis": f"deliveries by source, counted as {DELIVERY_UNIT_PHRASE}",
        "field": "deliveries_total",
    },
    ("context", "observed use"): {
        "label": "needs adjudication",
        "basis": "whether the delivered context was used in the assistant's next action is "
                 "judged",
        "field": None,
    },
    ("context", "checked outcome"): {
        "label": "needs adjudication",
        "basis": "the judge, plus the spot-check, decide this",
        "field": None,
    },
    ("transfer", "opportunity"): {
        "label": "needs adjudication",
        "basis": "lesson applicability is judged",
        "field": None,
    },
    ("transfer", "signal/request"): {
        "label": "needs adjudication",
        "basis": "the lesson inventory is Task 7, not yet built, so there is no candidate "
                 "population to count",
        "field": None,
    },
    ("transfer", "delivery/action"): {
        "label": "measurable now",
        "basis": "deliveries whose engine_or_hook names a transfer-carrying engine "
                 "(TRANSFER_DELIVERY_MARKERS: operator-rule, get_guide, operator-rules, "
                 f"guide-sections, session-opener), counted as {DELIVERY_UNIT_PHRASE}",
        "field": "transfer_filtered_deliveries",
    },
    ("transfer", "observed use"): {
        "label": "needs adjudication",
        "basis": "whether a transferred lesson was applied or missed is judged",
        "field": None,
    },
    ("transfer", "checked outcome"): {
        "label": "needs adjudication",
        "basis": "the judge, plus the spot-check, decide this",
        "field": None,
    },
    ("background-worker", "opportunity"): {
        "label": "measurable now",
        "basis": "tool calls (the A1.5 task-family split is not computed in this map)",
        "field": "tool_events_total",
    },
    ("background-worker", "signal/request"): {
        "label": "measurable now",
        "basis": "Agent tool_uses",
        "field": "agent_tool_uses",
    },
    ("background-worker", "delivery/action"): {
        "label": "measurable now",
        "basis": "subagent turns (agent_path set)",
        "field": "subagent_turns",
    },
    ("background-worker", "observed use"): {
        "label": "needs adjudication",
        "basis": "whether the parent used the subagent's result is judged",
        "field": None,
    },
    ("background-worker", "checked outcome"): {
        "label": "unobservable",
        "basis": "there is no counterfactual; A1.5 measures opportunity size, not delegability "
                 "or saving",
        "field": None,
    },
}

# Rediscovery (A1.4) is its own row under transfer, always "needs adjudication" -- never
# subject to the zero-coverage downgrade (it carries no field to downgrade).
_REDISCOVERY_SPEC = {
    "label": "needs adjudication",
    "basis": "always: it needs semantic matching (A1.4)",
    "field": None,
}


def _parse_ts(raw):
    """(datetime, None) for a parseable ts; (None, "null") for a NULL or empty one; (None,
    "unparseable") for anything join.utc cannot read. Never raises (R71)."""
    if raw is None or raw == "":
        return None, "null"
    try:
        return join.utc(raw), None
    except (ValueError, TypeError, AttributeError):
        return None, "unparseable"


def _in_window(dt, window):
    """R71: the ONE window-membership predicate, half-open [start, end)."""
    start, end = window
    return start <= dt < end


def _iso_z(dt):
    """Canonical "YYYY-MM-DDTHH:MM:SS[.mmm]Z" for a UTC datetime."""
    frac = f".{dt.microsecond // 1000:03d}" if dt.microsecond else ""
    return dt.strftime("%Y-%m-%dT%H:%M:%S") + frac + "Z"


def _iso_ms_z(dt):
    """The events DB's canonical ts form (join._fmt_ts): "YYYY-MM-DDTHH:MM:SS.mmmZ"."""
    return dt.strftime("%Y-%m-%dT%H:%M:%S.") + f"{dt.microsecond // 1000:03d}Z"


def _windows_from_bounds(bounds):
    """{"retained": (start, end), "decision": (start, end)} from manifest.bounds, strictly: a
    missing key raises KeyError, and an unset (None/empty) bound raises ValueError."""
    windows = {}
    for window_name in ("retained", "decision"):
        w = bounds[window_name]
        start_raw, end_raw = w["start_utc"], w["end_utc"]
        if not start_raw or not end_raw:
            raise ValueError(
                f"manifest.bounds.{window_name} is not finalized ({w!r}); run "
                "observability.finalize_bounds(corpus_dir) after archive.freeze."
            )
        windows[window_name] = (join.utc(start_raw), join.utc(end_raw))
    return windows


def _earliest_kept_top_level(sessions_list, excl):
    """R71: the earliest ts over every entry of every KEPT session's top-level transcript (the
    sessions transcripts.exclusions() does not exclude). Returns {"ts": the raw ts string of
    that entry, "entries_total": every kept top-level entry scanned, "entries_without_ts": those
    whose ts field is absent, null or empty, "entries_without_ts_by_type": {entry type: count}
    over those, "entries_unparseable_ts": those whose ts join.utc cannot read}. Entries without a
    parseable ts are skipped and counted, never fed to min(). "ts" is None when no kept
    top-level entry has a parseable ts. The single implementation behind both
    earliest_kept_top_level_ts() and coverage()."""
    best_dt = None
    best_raw = None
    total = 0
    without_ts_by_type = collections.Counter()
    unparseable = 0
    for s in sessions_list:
        if transcripts.copy_id(s) in excl:
            continue
        entries, _skipped = transcripts.read_jsonl(s.path)
        for e in entries:
            total += 1
            raw = e.get("timestamp")
            dt, why = _parse_ts(raw)
            if why == "null":
                without_ts_by_type[str(e.get("type"))] += 1
                continue
            if why == "unparseable":
                unparseable += 1
                continue
            if best_dt is None or dt < best_dt:
                best_dt, best_raw = dt, raw
    return {
        "ts": best_raw,
        "entries_total": total,
        "entries_without_ts": sum(without_ts_by_type.values()),
        "entries_without_ts_by_type": dict(without_ts_by_type),
        "entries_unparseable_ts": unparseable,
    }


def earliest_kept_top_level_ts(corpus_dir):
    """R71: the earliest top-level entry ts over the sessions KEPT by
    transcripts.exclusions(transcripts.sessions(corpus_dir), join.SPEC_EXCLUDED_SIDS). RAISES
    ValueError if no kept top-level entry carries a parseable ts: the window has no start."""
    sessions_list = transcripts.sessions(corpus_dir)
    excl = transcripts.exclusions(sessions_list, join.SPEC_EXCLUDED_SIDS)
    ts = _earliest_kept_top_level(sessions_list, excl)["ts"]
    if ts is None:
        raise ValueError(
            "no kept top-level transcript entry carries a parseable ts, so the retained window "
            "has no start."
        )
    return ts


def finalize_bounds(corpus_dir):
    """R71/R75: set manifest.bounds from data and rewrite corpus_dir/manifest.json. With
    end = floor(manifest.created_utc) + 1 s: retained = [earliest_kept_top_level_ts, end) and
    decision = [end - 7 days, end). Task 12 calls this right after archive.freeze. Returns the
    bounds.

    The guarantee, and why the + 1 s: archive.freeze takes created_utc AFTER copying every file,
    but archive._format_iso floors it to the second. A row written during the freeze's final
    second is copied yet can carry a ts >= created_utc (reproduced 5/5 by the round-2 reviewer's
    probe_floor.py). Every copied ts is earlier than the instant created_utc was read, which is
    earlier than floor(created_utc) + 1 s -- provided the writers' clock is the freezing host's.
    RAISES ValueError if the earliest kept entry is not before that end (an empty window)."""
    corpus_dir = pathlib.Path(corpus_dir)
    manifest_path = corpus_dir / "manifest.json"
    manifest = json.loads(manifest_path.read_text())
    created = manifest["created_utc"]
    end_dt = join.utc(created).replace(microsecond=0) + timedelta(seconds=1)
    end = _iso_z(end_dt)
    start = earliest_kept_top_level_ts(corpus_dir)
    if not join.utc(start) < end_dt:
        raise ValueError(
            f"finalize_bounds: the earliest kept top-level entry ({start}) is not before the "
            f"window end ({end}, the freeze instant {created} floored + 1 s), so the retained "
            "window would be empty."
        )
    bounds = {
        "retained": {"start_utc": start, "end_utc": end},
        "decision": {
            "start_utc": _iso_z(end_dt - timedelta(days=DECISION_WINDOW_DAYS)),
            "end_utc": end,
        },
    }
    manifest["bounds"] = bounds
    manifest_path.write_text(json.dumps(manifest, indent=2, sort_keys=True))
    return bounds


def _exclusions_by_reason(excl):
    """Group transcripts.exclusions()'s {copy_id: reason} by reason CLASS: a dynamic reason
    like "divergent-duplicate-of:<keeper>" groups under "divergent-duplicate-of", not as its
    own singleton key per keeper.
    """
    counts = collections.Counter()
    for reason in excl.values():
        base = reason.split(":", 1)[0]
        counts[base] += 1
    return dict(counts)


def _excluded_by_spec_detail(sessions_list, excl):
    """Minor ruling: explain "excluded-by-spec: N" data-wise -- which bare sid(s) recur across
    which profiles, not a bare count.
    """
    by_cid = {transcripts.copy_id(s): s for s in sessions_list}
    by_bare_sid = {}
    for cid, reason in excl.items():
        if reason.split(":", 1)[0] != "excluded-by-spec":
            continue
        by_bare_sid.setdefault(by_cid[cid].sid, []).append(cid)
    if not by_bare_sid:
        return "none"
    parts = [
        f"sid {bare_sid} present in {len(cids)} profile(s): {', '.join(sorted(cids))}"
        for bare_sid, cids in sorted(by_bare_sid.items())
    ]
    return "; ".join(parts)


def _divergent_duplicate_unowned(sessions_list, excl, attribution):
    """R57/Amendment 3: for each divergent-duplicate-of: copy, the uuids in its OWN top-level
    transcript that no kept transcript owns (attribute_entries() gives {uuid: owning copy_id}
    over every kept session; a uuid absent from that map is unowned). This is the documented
    cost of collapsing a divergent duplicate onto its keeper -- the diverged tail is dropped.
    """
    owned = set(attribution.keys())
    by_cid = {transcripts.copy_id(s): s for s in sessions_list}
    result = {}
    for cid, reason in excl.items():
        if not reason.startswith("divergent-duplicate-of:"):
            continue
        entries, _skipped = transcripts.read_jsonl(by_cid[cid].path)
        uuids = {e.get("uuid") for e in entries if e.get("uuid") is not None}
        unowned = sorted(uuids - owned)
        if unowned:
            result[cid] = unowned
    return result


def _kept_sessions_with_zero_turns(sessions_list, excl, conn, attribution):
    """R64: kept sessions (by cid) with zero rows in turns, plus a DATA-DERIVED explanation:
    the owned FRACTION of its own top-level uuids, per owning copy ("N of M uuids owned by
    <copy>", from attribute_entries() over the kept population), plus "R of M owned by no kept
    copy" for the rest. Never a hardcoded cid, never an inferred cause.

    Keys by transcripts.copy_id(s) (a str), never by the Session object itself: Session is a
    plain @dataclass with mutable list fields and no frozen=True/unsafe_hash=True, so Python
    sets __hash__ = None on it.
    """
    by_cid = {
        transcripts.copy_id(s): s
        for s in sessions_list
        if transcripts.copy_id(s) not in excl
    }
    turn_cids = {row["sid"] for row in conn.execute("SELECT DISTINCT sid FROM turns")}
    zero_turn = sorted(cid for cid in by_cid if cid not in turn_cids)
    if not zero_turn:
        return {"count": 0, "cids": [], "detail": "none"}

    explanations = []
    for cid in zero_turn:
        entries, _skipped = transcripts.read_jsonl(by_cid[cid].path)
        uuids = {e.get("uuid") for e in entries if e.get("uuid") is not None}
        total = len(uuids)
        owners = collections.Counter()
        unowned = 0
        for u in uuids:
            if u in attribution:
                owners[attribution[u]] += 1
            else:
                unowned += 1
        parts = [
            f"{n} of {total} uuids owned by {owner}" for owner, n in sorted(owners.items())
        ]
        if unowned or not parts:
            parts.append(f"{unowned} of {total} uuids owned by no kept copy")
        explanations.append(f"{cid}: " + ", ".join(parts))
    return {"count": len(zero_turn), "cids": zero_turn, "detail": "; ".join(explanations)}


def _sessions_per_project(sessions_list, excl, conn, windows):
    """Sessions per project (the project dir under transcripts/NN-<profile>/), split by
    window (retained, decision). A kept session is in a window if any of its turns' ts falls
    inside it, by the half-open _in_window (R71). build_events applies no bounds filter to
    turns, so this membership check is coverage()'s own responsibility. A turn with a NULL or
    unparseable ts is skipped here; coverage() counts it once, in skipped_ts["turns"].
    Requires R64's subset check first: every turns.sid is a kept cid.
    """
    kept = [s for s in sessions_list if transcripts.copy_id(s) not in excl]
    ts_by_cid = {transcripts.copy_id(s): [] for s in kept}
    for row in conn.execute("SELECT sid, ts FROM turns"):
        dt, why = _parse_ts(row["ts"])
        if why is not None:
            continue
        ts_by_cid[row["sid"]].append(dt)

    result = {}
    for s in kept:
        cid = transcripts.copy_id(s)
        entry = result.setdefault(s.path.parent.name, {"retained": 0, "decision": 0})
        for window_name, window in windows.items():
            if any(_in_window(dt, window) for dt in ts_by_cid[cid]):
                entry[window_name] += 1
    return result


def _zero_flat_window():
    return {"decision": 0, "retained": 0}


def _zero_pop_window():
    return {
        "decision": {"top-level": 0, "subagent": 0, "all": 0},
        "retained": {"top-level": 0, "subagent": 0, "all": 0},
    }


def _is_delivered_item(source, key):
    """R69: whether one deliveries row IS a delivered item (see DELIVERY_UNITS). RAISES
    ValueError for a source this module does not declare."""
    if source == "usage_deliveries_json":
        return key is not None
    if source in ("usage_output_json", "transcript_hook"):
        return True
    raise ValueError(
        f"deliveries.source {source!r} is not one of {DELIVERY_SOURCES}; its delivered-item "
        "unit is undefined, so it cannot be counted."
    )


def _window_counts(conn, windows):
    """One pass each over turns, tool_events and deliveries. Buckets every row into the
    DECISION window and the RETAINED window independently (by _in_window), tracks population
    (top-level / subagent / all) for turns and the observed ts span, and computes the whole-DB
    breakdowns: turns by kind, tool_events by join_method (overall and by UTC day), the first
    exact join, and deliveries by source (table rows and delivered items). A row with a NULL or
    unparseable ts is counted in the whole-DB breakdowns, skipped from every window, day and
    span, and counted in skipped_ts[table][cause] (R71). A turn kind, join method or delivery
    source outside its declared set RAISES ValueError (R70).
    """
    turns_windows = {kind: _zero_pop_window() for kind in TURN_KINDS}
    turns_by_kind = {kind: 0 for kind in TURN_KINDS}
    tool_events = {"total": _zero_flat_window(), AGENT_TOOL_NAME: _zero_flat_window()}
    tool_events_by_method = {method: 0 for method in JOIN_METHODS}
    tool_events_by_day = {}
    first_exact = None
    deliveries = {
        "total": _zero_flat_window(),
        "transfer_filtered": _zero_flat_window(),
        "with_tool_use_id": _zero_flat_window(),
    }
    deliveries_by_source = {source: {"rows": 0, "items": 0} for source in DELIVERY_SOURCES}
    skipped_ts = {table: {"null": 0, "unparseable": 0} for table in _TS_TABLES}
    span = {"min": None, "max": None}

    def _bump_span(dt):
        if span["min"] is None or dt < span["min"]:
            span["min"] = dt
        if span["max"] is None or dt > span["max"]:
            span["max"] = dt

    for row in conn.execute("SELECT agent_path, kind, ts FROM turns"):
        kind = row["kind"]
        if kind not in turns_by_kind:
            raise ValueError(f"turns.kind {kind!r} is not one of {TURN_KINDS}")
        turns_by_kind[kind] += 1
        dt, why = _parse_ts(row["ts"])
        if why is not None:
            skipped_ts["turns"][why] += 1
            continue
        _bump_span(dt)
        pop = "subagent" if row["agent_path"] else "top-level"
        bucket = turns_windows[kind]
        for window_name, window in windows.items():
            if _in_window(dt, window):
                bucket[window_name][pop] += 1
                bucket[window_name]["all"] += 1

    for row in conn.execute("SELECT name, ts, join_method FROM tool_events"):
        method = row["join_method"]
        if method not in tool_events_by_method:
            raise ValueError(f"tool_events.join_method {method!r} is not one of {JOIN_METHODS}")
        tool_events_by_method[method] += 1
        dt, why = _parse_ts(row["ts"])
        if why is not None:
            skipped_ts["tool_events"][why] += 1
            continue
        _bump_span(dt)
        day = tool_events_by_day.setdefault(
            dt.date().isoformat(), {m: 0 for m in JOIN_METHODS}
        )
        day[method] += 1
        if method == "exact" and (first_exact is None or dt < first_exact[0]):
            first_exact = (dt, row["ts"])
        for window_name, window in windows.items():
            if _in_window(dt, window):
                tool_events["total"][window_name] += 1
                if row["name"] == AGENT_TOOL_NAME:
                    tool_events[AGENT_TOOL_NAME][window_name] += 1

    for row in conn.execute(
        "SELECT source, engine_or_hook, key, tool_use_id, ts FROM deliveries"
    ):
        is_item = _is_delivered_item(row["source"], row["key"])
        per_source = deliveries_by_source[row["source"]]
        per_source["rows"] += 1
        if is_item:
            per_source["items"] += 1
        dt, why = _parse_ts(row["ts"])
        if why is not None:
            skipped_ts["deliveries"][why] += 1
            continue
        _bump_span(dt)
        if not is_item:
            continue
        for window_name, window in windows.items():
            if _in_window(dt, window):
                deliveries["total"][window_name] += 1
                if row["engine_or_hook"] in TRANSFER_DELIVERY_MARKERS:
                    deliveries["transfer_filtered"][window_name] += 1
                if row["tool_use_id"]:
                    deliveries["with_tool_use_id"][window_name] += 1

    return {
        "turns": turns_windows,
        "turns_by_kind": turns_by_kind,
        "tool_events": tool_events,
        "tool_events_by_method": tool_events_by_method,
        "tool_events_by_day": tool_events_by_day,
        "first_exact_tool_event_ts": first_exact[1] if first_exact is not None else None,
        "deliveries": deliveries,
        "deliveries_by_source": deliveries_by_source,
        "data_min": span["min"],
        "data_max": span["max"],
        "skipped_ts": skipped_ts,
    }


def _field_window_count(win, field):
    """Returns (decision_count, retained_count, population_label) for a known coverage field
    name. RAISES KeyError for anything else -- R60: a MISSING or typo'd coverage field must
    raise, never silently read as 0. Every lookup below is a subscript (R70).
    """
    if field is None:
        return (None, None, "n/a")
    if field == "assistant_text_turns":
        b = win["turns"]["assistant_text"]
        return (b["decision"]["all"], b["retained"]["all"], "all")
    if field == "prompt_interrupt_turns":
        p = win["turns"]["prompt"]
        i = win["turns"]["interrupt"]
        dec = p["decision"]["all"] + i["decision"]["all"]
        ret = p["retained"]["all"] + i["retained"]["all"]
        return (dec, ret, "all")
    if field == "tool_events_total":
        t = win["tool_events"]["total"]
        return (t["decision"], t["retained"], "all")
    if field == "deliveries_total":
        d = win["deliveries"]["total"]
        return (d["decision"], d["retained"], "all")
    if field == "transfer_filtered_deliveries":
        d = win["deliveries"]["transfer_filtered"]
        return (d["decision"], d["retained"], "all")
    if field == "agent_tool_uses":
        t = win["tool_events"][AGENT_TOOL_NAME]
        return (t["decision"], t["retained"], "all")
    if field == "delegation_turns":
        b = win["turns"]["delegation"]
        return (b["decision"]["all"], b["retained"]["all"], "all")
    if field == "subagent_turns":
        dec = sum(b["decision"]["subagent"] for b in win["turns"].values())
        ret = sum(b["retained"]["subagent"] for b in win["turns"].values())
        return (dec, ret, "subagent")
    raise KeyError(
        f"coverage(): {field!r} has no window-count dispatcher branch -- R60: a missing or "
        "typo'd coverage field name must raise, never silently read as 0"
    )


def _top_level_subset(win, field):
    """The top-level-only subset of a field's count, for cells whose primary number is "all"
    (R62's population line) but whose basis also cites a top-level breakdown. Returns None for
    a field with no meaningful top-level/subagent split.
    """
    if field == "assistant_text_turns":
        b = win["turns"]["assistant_text"]
        return (b["decision"]["top-level"], b["retained"]["top-level"])
    if field == "prompt_interrupt_turns":
        p = win["turns"]["prompt"]
        i = win["turns"]["interrupt"]
        return (
            p["decision"]["top-level"] + i["decision"]["top-level"],
            p["retained"]["top-level"] + i["retained"]["top-level"],
        )
    return None


def _all_declared_fields():
    """Every field name _BASE_TABLE actually references, plus delegation_turns (R66's always-
    computed secondary number). Deriving this FROM _BASE_TABLE rather than hardcoding a second
    list means a typo'd or renamed field in the table is validated (and raises) the moment
    coverage() runs -- there is no way for the two to drift apart.
    """
    fields = {spec["field"] for spec in _BASE_TABLE.values() if spec["field"] is not None}
    fields.add("delegation_turns")
    return sorted(fields)


def coverage(events_db, corpus_dir):
    """R57/R60-R71. Reads corpus_dir/manifest.json and events_db (opened read-only, closed on
    every exit path including a raise) and returns every coverage number the map needs:
    per-field DECISION-window and RETAINED-window counts (R62), the A1.6 appendix breakdowns
    render_map renders (R60), and the raw events_meta counters. Every events_meta and manifest
    read is a subscript: a missing counter raises KeyError (R70).

    RAISES ValueError if:
    - the exclusions recomputed here (transcripts.exclusions() over join.SPEC_EXCLUDED_SIDS,
      exactly as build_events would with its own default) disagree with events_meta's
      persisted kept/excluded counts, or turns.sid is not a subset of the recomputed kept cids
      (R64; the reviewer's swap fixture trips it). The excluded cids and the kept cids partition
      the sessions, so the subset check also rules out an excluded cid in turns.sid;
    - manifest.bounds is unset, or retained.start_utc is later than the earliest kept top-level
      entry, or any observed ts is at or after retained.end_utc (R61, half-open per R71), or any
      events-DB row's ts is before retained.start_utc (R76);
    - a tool_events join_method row count disagrees with events_meta's tool_events_<method>
      counter (R77);
    - a turn kind, join method or delivery source is outside its declared set (R70).

    RAISES KeyError if a _BASE_TABLE cell names a field with no window-count dispatcher branch
    (R60).
    """
    corpus_dir = pathlib.Path(corpus_dir)
    manifest = json.loads((corpus_dir / "manifest.json").read_text())

    db_uri = pathlib.Path(events_db).resolve().as_uri() + "?mode=ro"
    conn = sqlite3.connect(db_uri, uri=True)
    try:
        conn.row_factory = sqlite3.Row

        meta = {
            row["key"]: row["value"]
            for row in conn.execute("SELECT key, value FROM events_meta")
        }

        sessions_list = transcripts.sessions(corpus_dir)
        excl = transcripts.exclusions(sessions_list, join.SPEC_EXCLUDED_SIDS)
        recomputed_kept = len(sessions_list) - len(excl)
        recomputed_excluded = len(excl)
        persisted_kept = int(meta["sessions_kept"])
        persisted_excluded = int(meta["sessions_excluded"])
        if recomputed_kept != persisted_kept or recomputed_excluded != persisted_excluded:
            raise ValueError(
                "exclusions mismatch: transcripts.exclusions(sessions(corpus_dir), "
                f"join.SPEC_EXCLUDED_SIDS) recomputes kept={recomputed_kept} "
                f"excluded={recomputed_excluded}, but events_meta records "
                f"sessions_kept={persisted_kept} sessions_excluded={persisted_excluded} -- "
                "events.db was built with a different excluded_sids set than coverage() "
                "assumes, so every count below would be measuring the wrong corpus."
            )

        kept_cids = {
            transcripts.copy_id(s) for s in sessions_list if transcripts.copy_id(s) not in excl
        }
        turn_sids = {row["sid"] for row in conn.execute("SELECT DISTINCT sid FROM turns")}
        if not turn_sids.issubset(kept_cids):
            raise ValueError(
                "R64: turns.sid contains cid(s) not in the recomputed kept set: "
                f"{sorted(turn_sids - kept_cids)} -- events.db was built with a different "
                "excluded_sids set than coverage() assumes."
            )

        bounds = manifest["bounds"]
        windows = _windows_from_bounds(bounds)
        win = _window_counts(conn, windows)

        earliest = _earliest_kept_top_level(sessions_list, excl)
        retained_start, retained_end = windows["retained"]
        earliest_dt = join.utc(earliest["ts"]) if earliest["ts"] is not None else None
        if earliest_dt is not None and retained_start > earliest_dt:
            raise ValueError(
                "R61: manifest.bounds.retained.start_utc "
                f"({retained_start.isoformat()}) is later than the earliest kept top-level "
                f"entry ({earliest_dt.isoformat()}) -- the retained window must start at or "
                "before the oldest kept transcript."
            )
        if win["data_max"] is not None and win["data_max"] >= retained_end:
            raise ValueError(
                f"R61: an observed ts ({win['data_max'].isoformat()}) is at or after "
                f"manifest.bounds.retained.end_utc ({retained_end.isoformat()}) -- the "
                "half-open retained window must cover every observed ts."
            )
        # R76 / Amendment 6(a): the other half of "coverage raises when data falls outside the
        # window" -- an events-DB row (a subagent turn, a usage-row delivery) can predate every
        # top-level entry, which the R61 start check above cannot see.
        if win["data_min"] is not None and win["data_min"] < retained_start:
            raise ValueError(
                f"R76: an events-DB row's ts ({win['data_min'].isoformat()}) is before "
                f"manifest.bounds.retained.start_utc ({retained_start.isoformat()}) -- the "
                "retained window must cover every observed ts."
            )
        # R77: the joins-by-method table's row counts must equal build_events' own counters.
        for method in JOIN_METHODS:
            persisted = int(meta[f"tool_events_{method}"])
            if win["tool_events_by_method"][method] != persisted:
                raise ValueError(
                    f"R77: tool_events has {win['tool_events_by_method'][method]} rows with "
                    f"join_method {method!r}, but events_meta.tool_events_{method} records "
                    f"{persisted} -- the table and the build's counters disagree."
                )

        tool_events_total_count = int(meta["tool_events_total"])
        heuristic_total = int(meta["tool_events_heuristic"])
        heuristic_via_called_at = int(meta["heuristic_via_called_at"])
        hook_success_only = int(meta["hook_success_only"])
        hook_success_twins_dropped = int(meta["hook_success_twins_dropped"])
        # R67 / Amendment 5(f): N is build_events' own counter, read, never recomputed.
        usage_rows_unmapped = int(meta["deliveries_unmapped_session"])

        attribution = transcripts.attribute_entries(sessions_list, excl)
        divergent_duplicate_unowned = _divergent_duplicate_unowned(
            sessions_list, excl, attribution
        )
        kept_sessions_with_zero_turns = _kept_sessions_with_zero_turns(
            sessions_list, excl, conn, attribution
        )
        sessions_per_project = _sessions_per_project(sessions_list, excl, conn, windows)
        exclusions_by_reason = _exclusions_by_reason(excl)
        excluded_by_spec_detail = _excluded_by_spec_detail(sessions_list, excl)

        # R60(b)/R67: usage rows are read once, straight from join's own loader, so M (rows
        # read), K (of the unmapped, those from the spec-excluded session; no build counter
        # exists for it) and the Amendment 4(a) count agree with what build_events saw.
        usage_rows = join._load_usage_rows(corpus_dir)
        kept_bare_sids = {s.sid for s in sessions_list if transcripts.copy_id(s) not in excl}
        usage_rows_unmapped_spec_excluded = sum(
            1 for r in usage_rows
            if r["cc_session_id"] not in kept_bare_sids
            and r["cc_session_id"] in join.SPEC_EXCLUDED_SIDS
        )
        usage_rows_with_tool_use_id = sum(1 for r in usage_rows if r["tool_use_id"])
        latency_row_keys = {r["_row_key"] for r in usage_rows if r["latency_ms"] is not None}

        joined_tool_events = (
            win["tool_events_by_method"]["exact"] + win["tool_events_by_method"]["heuristic"]
        )
        joined_with_latency_ms = 0
        for row in conn.execute(
            "SELECT usage_row_id FROM tool_events WHERE join_method IN ('exact', 'heuristic')"
        ):
            if row["usage_row_id"] in latency_row_keys:
                joined_with_latency_ms += 1

        field_results = {}
        for field in _all_declared_fields():
            dec, ret, pop = _field_window_count(win, field)
            field_results[field] = {"decision": dec, "retained": ret, "population": pop}

        at_top = _top_level_subset(win, "assistant_text_turns")
        pi_top = _top_level_subset(win, "prompt_interrupt_turns")
        deleg = field_results["delegation_turns"]
        d_tid = win["deliveries"]["with_tool_use_id"]
        d_all = win["deliveries"]["total"]

        # R70: one entry per rendered cell, None where the cell has no extra -- read with a
        # subscript, so a mistyped cell key raises.
        cell_extra = {key: None for key in _BASE_TABLE}
        cell_extra[("transfer", "rediscovery")] = None
        cell_extra[("mistakes", "opportunity")] = (
            f"of which top-level -- decision: {at_top[0]}, retained: {at_top[1]}"
        )
        cell_extra[("context", "opportunity")] = (
            f"of which top-level -- decision: {at_top[0]}, retained: {at_top[1]}"
        )
        cell_extra[("mistakes", "signal/request")] = (
            f"of which top-level -- decision: {pi_top[0]}, retained: {pi_top[1]}"
        )
        cell_extra[("context", "delivery/action")] = (
            "delivered items carrying a tool_use_id -- decision: "
            f"{d_tid['decision']} of {d_all['decision']}; retained: "
            f"{d_tid['retained']} of {d_all['retained']}"
        )
        cell_extra[("background-worker", "signal/request")] = (
            "delegation turns (separate, never summed) -- decision: "
            f"{deleg['decision']}, retained: {deleg['retained']}"
        )

        return {
            "corpus_id": manifest["corpus_id"],
            "manifest_bounds": bounds,
            "meta": dict(meta),
            "earliest_kept_top_level": earliest,
            "data_min_ts": _iso_ms_z(win["data_min"]) if win["data_min"] else None,
            "data_max_ts": _iso_ms_z(win["data_max"]) if win["data_max"] else None,
            "skipped_ts": win["skipped_ts"],
            "tool_events_by_method": win["tool_events_by_method"],
            "tool_events_by_day": win["tool_events_by_day"],
            "first_exact_tool_event_ts": win["first_exact_tool_event_ts"],
            "heuristic_total": heuristic_total,
            "heuristic_via_called_at": heuristic_via_called_at,
            "heuristic_rest": heuristic_total - heuristic_via_called_at,
            "joined_tool_events": joined_tool_events,
            "joined_with_latency_ms": joined_with_latency_ms,
            "tool_events_total_count": tool_events_total_count,
            "deliveries_by_source": win["deliveries_by_source"],
            "hook_success_only": hook_success_only,
            "hook_success_twins_dropped": hook_success_twins_dropped,
            "usage_rows_total": len(usage_rows),
            "usage_rows_unmapped": usage_rows_unmapped,
            "usage_rows_unmapped_spec_excluded": usage_rows_unmapped_spec_excluded,
            "usage_rows_with_tool_use_id": usage_rows_with_tool_use_id,
            "sessions_per_project": sessions_per_project,
            "exclusions_by_reason": exclusions_by_reason,
            "excluded_by_spec_detail": excluded_by_spec_detail,
            "turns_by_kind": win["turns_by_kind"],
            "divergent_duplicate_unowned": divergent_duplicate_unowned,
            "kept_sessions_with_zero_turns": kept_sessions_with_zero_turns,
            "fields": field_results,
            "cell_extra": cell_extra,
        }
    finally:
        conn.close()


def _resolve_cell(cov, outcome, link, spec):
    label = spec["label"]
    basis = spec["basis"]
    field = spec["field"]

    if field is None:
        dec_number = None
        ret_number = None
        population = "n/a"
    else:
        entry = cov["fields"][field]
        dec_number = entry["decision"]
        ret_number = entry["retained"]
        population = entry["population"]

    extra = cov["cell_extra"][(outcome, link)]
    if extra is not None:
        basis = f"{basis} ({extra})"

    # R62: downgrade-on-zero reads the outcome's DECISION-bearing window -- decision for
    # mistakes, retained for the other three.
    decision_bearing_window = "decision" if outcome == "mistakes" else "retained"
    decision_bearing_number = dec_number if decision_bearing_window == "decision" else ret_number

    if label == "measurable now" and not decision_bearing_number:
        label = "needs adjudication"
        basis = (
            f"{basis} -- downgraded: the {decision_bearing_window}-window coverage number "
            "for this link is 0, so nothing is measurable now"
        )

    return {
        "outcome": outcome,
        "link": link,
        "label": label,
        "basis": basis,
        "decision_number": dec_number,
        "retained_number": ret_number,
        "population": population,
    }


def build_cells(cov):
    """Every (outcome, link) cell, base-labelled from _BASE_TABLE and resolved (zero-coverage
    downgrade applied) against `cov`, plus the one extra rediscovery row under transfer.
    """
    cells = []
    for outcome in _OUTCOMES:
        for link in _LINKS:
            cells.append(_resolve_cell(cov, outcome, link, _BASE_TABLE[(outcome, link)]))
    cells.append(_resolve_cell(cov, "transfer", "rediscovery", _REDISCOVERY_SPEC))
    return cells


def rendering_code_version(repo=None):
    """R68: the RENDERING code's version -- {"head": `git -C <repo> rev-parse HEAD`, "dirty":
    whether `git status --porcelain -- scripts/measure` is non-empty}, read at render time.
    RAISES RuntimeError if git cannot answer: a map must never render an unknown version."""
    repo = pathlib.Path(repo) if repo is not None else _REPO_ROOT
    head = subprocess.run(
        ["git", "-C", str(repo), "rev-parse", "HEAD"],
        capture_output=True, text=True, timeout=30,
    )
    status = subprocess.run(
        ["git", "-C", str(repo), "status", "--porcelain", "--", "scripts/measure"],
        capture_output=True, text=True, timeout=30,
    )
    if head.returncode != 0 or not head.stdout.strip() or status.returncode != 0:
        raise RuntimeError(
            f"rendering_code_version: git could not report HEAD and status for {repo}: "
            f"{head.stderr.strip()} {status.stderr.strip()}"
        )
    return {"head": head.stdout.strip(), "dirty": bool(status.stdout.strip())}


def _sources_from_manifest(manifest):
    """R68: the frozen sources, derived from the manifest's `files` keys in archive.freeze's
    layout: {"transcript_dirs": {(profile dir, project dir): {"top_level": n, "subagent": m}},
    "usage_db_files": k}. A key in any other layout RAISES ValueError."""
    per_dir = {}
    usage_db_files = 0
    for rel in manifest["files"]:
        parts = rel.split("/")
        if parts[0] == "transcripts" and len(parts) == 4:
            kind = "top_level"
        elif parts[0] == "transcripts" and len(parts) == 6 and parts[4] == "subagents":
            kind = "subagent"
        elif parts[0] == "usage_dbs" and len(parts) == 2:
            usage_db_files += 1
            continue
        else:
            raise ValueError(f"manifest files key {rel!r} matches no archive.freeze layout")
        entry = per_dir.setdefault((parts[1], parts[2]), {"top_level": 0, "subagent": 0})
        entry[kind] += 1
    return {"transcript_dirs": per_dir, "usage_db_files": usage_db_files}


def _table(lines, header, rows):
    lines.append("| " + " | ".join(header) + " |")
    lines.append("|" + "---|" * len(header))
    for row in rows:
        lines.append("| " + " | ".join(str(c) for c in row) + " |")


def render_map(coverage, manifest, code_version=None):
    """R60/R68: renders the WHOLE document -- the title, the fixed provisional sentence,
    provenance, windows, the four label tables plus the rediscovery row, and every A1.6 appendix
    breakdown. The committed map file must be byte-identical to this function's output for the
    build it describes; there is no hand-editing step after it. Prose here is only (a) a
    data-derived statement, (b) the fixed Amendment 4(a) citation, or (c) PROVISIONAL_SENTENCE --
    never an inferred cause. Every read is a subscript (R70).

    `code_version` is {"head", "dirty"}; None reads it with rendering_code_version() now.
    RAISES ValueError if `coverage` was computed from a different manifest's corpus or bounds.
    """
    if (coverage["corpus_id"] != manifest["corpus_id"]
            or coverage["manifest_bounds"] != manifest["bounds"]):
        raise ValueError(
            "render_map: coverage was computed for corpus "
            f"{coverage['corpus_id']!r} with bounds {coverage['manifest_bounds']!r}, but the "
            f"manifest is {manifest['corpus_id']!r} with bounds {manifest['bounds']!r}."
        )
    if code_version is None:
        code_version = rendering_code_version()

    lines = []
    lines.append(f"# Observability map -- {coverage['corpus_id']}")
    lines.append("")
    lines.append(PROVISIONAL_SENTENCE)
    lines.append("")

    lines.append("## Provenance")
    lines.append("")
    dirty = (
        "scripts/measure has uncommitted changes" if code_version["dirty"]
        else "scripts/measure clean"
    )
    earliest = coverage["earliest_kept_top_level"]
    sources = _sources_from_manifest(manifest)
    repos = manifest["repos"]
    repo_bits = "; ".join(f"{name} at {sha}" for name, sha in sorted(repos.items())) or "none"
    lines.append(f"- corpus_id: {coverage['corpus_id']}")
    lines.append(f"- freeze instant (manifest created_utc): {manifest['created_utc']}")
    lines.append(
        f"- rendering code: git HEAD {code_version['head']} at render time; {dirty}"
    )
    lines.append(f"- repos at freeze (manifest repos): {repo_bits}")
    earliest_ts = earliest["ts"] if earliest["ts"] is not None else "none, no kept entry has one"
    lines.append(
        f"- earliest kept top-level entry ts: {earliest_ts} (the minimum over the kept top-level "
        "entries that carry a parseable ts)"
    )
    by_type = earliest["entries_without_ts_by_type"]
    histogram = ", ".join(
        f"{t} {by_type[t]}" for t in sorted(by_type, key=lambda t: (-by_type[t], t))
    ) or "none"
    lines.append(
        f"- {earliest['entries_without_ts']} of {earliest['entries_total']} kept top-level "
        "entries carry no ts field (absent, null or empty) and "
        f"{earliest['entries_unparseable_ts']} carry an unparseable ts; the no-ts entries by "
        f"entry type: {histogram}"
    )
    lines.append(
        f"- usage DBs: {sources['usage_db_files']} file(s) in the manifest's files, holding "
        f"{manifest['counts']['usage_rows']} usage rows (manifest counts)"
    )
    lines.append("")
    lines.append("Transcript sources, from the manifest's files:")
    lines.append("")
    dirs = sources["transcript_dirs"]
    if dirs:
        _table(
            lines,
            ("profile dir", "project dir", "top-level transcripts", "subagent transcripts"),
            [
                (profile, project, dirs[(profile, project)]["top_level"],
                 dirs[(profile, project)]["subagent"])
                for profile, project in sorted(dirs)
            ],
        )
    else:
        lines.append("none")
    lines.append("")

    lines.append("## Windows")
    lines.append("")
    lines.append("Both windows are half-open, [start, end).")
    lines.append("")
    bounds = coverage["manifest_bounds"]
    for window_name in ("retained", "decision"):
        w = bounds[window_name]
        lines.append(f"- {window_name}: {w['start_utc']} to {w['end_utc']}")
    lines.append("")
    if coverage["data_min_ts"] is None:
        lines.append("Data span observed across turns, tool_events and deliveries: no row "
                     "carries a parseable ts.")
    else:
        lines.append(
            "Data span observed across turns, tool_events and deliveries: "
            f"{coverage['data_min_ts']} to {coverage['data_max_ts']}."
        )
    lines.append("")
    skipped = coverage["skipped_ts"]
    for cause, phrase in (("null", "a NULL or empty ts"), ("unparseable", "an unparseable ts")):
        lines.append(
            f"Rows skipped from every window, day and span for {phrase} -- "
            + ", ".join(f"{table}: {skipped[table][cause]}" for table in _TS_TABLES)
            + "."
        )
    lines.append("")

    cells = build_cells(coverage)
    by_outcome = {outcome: [] for outcome in _OUTCOMES}
    for cell in cells:
        by_outcome[cell["outcome"]].append(cell)

    for outcome in _OUTCOMES:
        lines.append(f"## {outcome}")
        lines.append("")
        lines.append("| link | label | decision | retained | population | basis |")
        lines.append("|---|---|---|---|---|---|")
        for cell in by_outcome[outcome]:
            dec = "n/a" if cell["decision_number"] is None else str(cell["decision_number"])
            ret = "n/a" if cell["retained_number"] is None else str(cell["retained_number"])
            lines.append(
                f"| {cell['link']} | {cell['label']} | {dec} | {ret} | {cell['population']} "
                f"| {cell['basis']} |"
            )
        lines.append("")

    lines.append("## Appendix")
    lines.append("")

    whole_db = (
        "Window: retained, as the whole events DB (rows skipped above for their ts included)."
    )

    lines.append("### A1.6 -- joins by method (overall)")
    lines.append("")
    lines.append(whole_db)
    lines.append("")
    by_method = coverage["tool_events_by_method"]
    _table(lines, ("method", "count"), [(m, by_method[m]) for m in JOIN_METHODS])
    lines.append("")
    first_exact = coverage["first_exact_tool_event_ts"]
    lines.append(
        "Amendment 4(a): tool_use_id was NULL on every usage row until 6f6349ca; it appears "
        "per session from that session's /mcp. Usage rows with a non-NULL tool_use_id: "
        f"{coverage['usage_rows_with_tool_use_id']}. First exact join ts (the transcript "
        f"tool_use ts): {first_exact if first_exact is not None else 'none, no exact join'}."
    )
    lines.append("")
    lines.append(
        f"Heuristic joins: {coverage['heuristic_total']} total, of which "
        f"{coverage['heuristic_via_called_at']} matched via called_at (started_at NULL) and "
        f"{coverage['heuristic_rest']} matched via started_at directly."
    )
    lines.append("")
    lines.append(
        "A1.5's latency share is measurable only for joined calls: "
        f"{coverage['joined_tool_events']} of {coverage['tool_events_total_count']} "
        "tool_events are joined (exact + heuristic), and "
        f"{coverage['joined_with_latency_ms']} of those joined rows carry latency_ms on their "
        "usage row."
    )
    lines.append("")

    lines.append("### A1.6 -- joins by method (by day)")
    lines.append("")
    skipped_te = skipped["tool_events"]["null"] + skipped["tool_events"]["unparseable"]
    lines.append(
        "Window: retained, by UTC day of the tool_use ts; the "
        f"{skipped_te} tool_events rows with a NULL or unparseable ts appear in the overall "
        "table only."
    )
    lines.append("")
    by_day = coverage["tool_events_by_day"]
    if by_day:
        _table(
            lines,
            ("day",) + JOIN_METHODS,
            [(day,) + tuple(by_day[day][m] for m in JOIN_METHODS) for day in sorted(by_day)],
        )
    else:
        lines.append("none")
    lines.append("")

    lines.append("### A1.6 -- delivery coverage by source")
    lines.append("")
    lines.append(whole_db)
    lines.append("")
    by_source = coverage["deliveries_by_source"]
    _table(
        lines,
        ("source", "delivered items", "table rows", "unit"),
        [
            (s, by_source[s]["items"], by_source[s]["rows"], DELIVERY_UNITS[s])
            for s in DELIVERY_SOURCES
        ],
    )
    lines.append("")
    lines.append(
        f"hook_success_only: {coverage['hook_success_only']}; "
        f"hook_success_twins_dropped: {coverage['hook_success_twins_dropped']}."
    )
    lines.append("")
    lines.append(
        "usage rows with no kept session: "
        f"{coverage['usage_rows_unmapped']} of {coverage['usage_rows_total']} usage rows read "
        f"(of which {coverage['usage_rows_unmapped_spec_excluded']} from the spec-excluded "
        "session)."
    )
    lines.append("")

    lines.append("### A1.6 -- sessions per project and window")
    lines.append("")
    lines.append(
        "Window: both, as columns; a kept session is in a window if any of its turns' ts is."
    )
    lines.append("")
    spp = coverage["sessions_per_project"]
    _table(
        lines,
        ("project", "retained", "decision"),
        [(p, spp[p]["retained"], spp[p]["decision"]) for p in sorted(spp)],
    )
    lines.append("")

    lines.append("### A1.6 -- exclusions by reason")
    lines.append("")
    lines.append("Window: retained, as every session in the corpus.")
    lines.append("")
    ebr = coverage["exclusions_by_reason"]
    _table(lines, ("reason", "count"), [(r, ebr[r]) for r in sorted(ebr)])
    lines.append("")
    lines.append(f"excluded-by-spec, data-wise: {coverage['excluded_by_spec_detail']}")
    lines.append("")

    lines.append("### A1.6 -- turns by kind")
    lines.append("")
    lines.append(whole_db)
    lines.append("")
    tbk = coverage["turns_by_kind"]
    _table(lines, ("kind", "count"), [(k, tbk[k]) for k in sorted(tbk)])
    lines.append("")

    lines.append("### A1.6 -- divergent-duplicate unowned uuids")
    lines.append("")
    lines.append("Window: retained, as every divergent-duplicate copy in the corpus.")
    lines.append("")
    dd = coverage["divergent_duplicate_unowned"]
    if dd:
        _table(lines, ("copy", "unowned uuid count"), [(c, len(dd[c])) for c in sorted(dd)])
    else:
        lines.append("none")
    lines.append("")

    lines.append("### A1.6 -- kept sessions with zero turns")
    lines.append("")
    lines.append(whole_db)
    lines.append("")
    zt = coverage["kept_sessions_with_zero_turns"]
    lines.append(f"count: {zt['count']}.")
    lines.append("")
    lines.append(zt["detail"])
    lines.append("")

    return "\n".join(lines)
