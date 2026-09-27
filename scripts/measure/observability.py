"""Stage 1c: the observability map (Task 13, fix round 1).

Spec A1.6 requires this map before any Codex-judge call: for every outcome and every
causal-chain link, a label (measurable now / needs adjudication / unobservable) plus the
coverage number from the events database that justifies it, or the reason none exists.

Consumes Task 6's events.db (scripts/measure/join.py, do not modify) and Task 5's transcript
helpers (scripts/measure/transcripts.py, do not modify). Produces two things:
- coverage(events_db, corpus_dir) -> dict: every count the map needs, per DECISION window and
  RETAINED window (R62), plus the A1.6 appendix breakdowns.
- render_map(coverage, manifest) -> str: the WHOLE document (R60) -- header, provenance,
  windows, the four label tables plus the rediscovery row, and every A1.6 breakdown. The
  committed map file must be byte-identical to this function's output; it is never hand-edited.

Fix round 1 (rulings R60-R67, ledger 2026-09-26-system1-base-rate-measurement): round 0's
committed map was hand-written prose that happened to describe render_map's numbers, not
render_map's actual output, so it published an inferred cause for the exact-join share, a
retained window that started after the corpus's own oldest entry, and a wrong cause for a
118-vs-119 gap that no test could have caught. This round makes render_map render everything,
makes a missing coverage field raise instead of silently reading 0, derives the retained window
from data, and reports both windows (with population) on every cell.

Run: ~/work/claude/prompt-engineering/.venv/bin/python -m pytest tests/test_measure_observability.py -v
"""
import json
import pathlib
import sqlite3
import sys

_THIS_DIR = pathlib.Path(__file__).resolve().parent
if str(_THIS_DIR) not in sys.path:
    sys.path.insert(0, str(_THIS_DIR))
import join  # noqa: E402
import transcripts  # noqa: E402

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

_LINKS = ("opportunity", "signal/request", "delivery/action", "observed use", "checked outcome")
_OUTCOMES = ("mistakes", "context", "transfer", "background-worker")

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
        "basis": "deliveries by source",
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
                 "guide-sections, session-opener)",
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
        "basis": "tool calls by A1.5 task family",
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


def _exclusions_by_reason(excl):
    """Group transcripts.exclusions()'s {copy_id: reason} by reason CLASS: a dynamic reason
    like "divergent-duplicate-of:<keeper>" groups under "divergent-duplicate-of", not as its
    own singleton key per keeper.
    """
    counts = {}
    for reason in excl.values():
        base = reason.split(":", 1)[0]
        counts[base] = counts.get(base, 0) + 1
    return counts


def _excluded_by_spec_detail(sessions_list, excl):
    """Minor ruling: explain "excluded-by-spec: N" data-wise -- which bare sid(s) recur across
    which profiles, not a bare count.
    """
    by_cid = {transcripts.copy_id(s): s for s in sessions_list}
    by_bare_sid = {}
    for cid, reason in excl.items():
        if reason.split(":", 1)[0] != "excluded-by-spec":
            continue
        s = by_cid.get(cid)
        if s is None:
            continue
        by_bare_sid.setdefault(s.sid, []).append(cid)
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
        session = by_cid.get(cid)
        if session is None:
            continue
        entries, _skipped = transcripts.read_jsonl(session.path)
        uuids = {e.get("uuid") for e in entries if e.get("uuid") is not None}
        unowned = sorted(uuids - owned)
        if unowned:
            result[cid] = unowned
    return result


def _kept_sessions_with_zero_turns(sessions_list, excl, conn, attribution):
    """R64: kept sessions (by cid) with zero rows in turns, plus a DATA-DERIVED explanation --
    never a hardcoded cid or corpus-specific cause. For each such cid, look up who owns its own
    transcript's uuids (attribute_entries() over the kept population): a zero-turn kept session
    whose uuids are all owned by a DIFFERENT copy is explained by that; one with no owner found
    anywhere gets an honest "no owning copy found" instead of a guessed cause. The old
    "none of whose turns fall inside either window" text named the wrong cause (it was
    vacuously true of every zero-turn session) and is not reproduced here.

    Keys by transcripts.copy_id(s) (a str), never by the Session object itself: Session is a
    plain @dataclass with mutable list fields (first_uuids, subagent_paths) and no
    frozen=True/unsafe_hash=True, so Python sets __hash__ = None on it -- using a Session as a
    dict key raises "TypeError: unhashable type: 'Session'" (confirmed against this file's own
    coverage() on 2026-09-27; every sibling helper in this module already keys by cid).
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
        session = by_cid.get(cid)
        owners = set()
        if session is not None:
            entries, _skipped = transcripts.read_jsonl(session.path)
            for e in entries:
                u = e.get("uuid")
                if u is not None and u in attribution:
                    owners.add(attribution[u])
        owners.discard(cid)
        if owners:
            explanations.append(f"{cid}: its uuids are owned by {', '.join(sorted(owners))}")
        else:
            explanations.append(f"{cid}: no owning copy found among kept sessions")
    return {"count": len(zero_turn), "cids": zero_turn, "detail": "; ".join(explanations)}


def _sessions_per_project(sessions_list, excl, conn, manifest):
    """Sessions per project (the project dir under transcripts/NN-<profile>/), split by
    window (retained, decision). A session is in a window if any of its turns' ts falls inside
    that window's [start_utc, end_utc] -- build_events applies no bounds filter to turns
    itself, so this membership check is coverage()'s own responsibility. A turn with a NULL or
    unparseable ts is skipped from this membership check (Minor ruling), never raised.
    """
    kept = [s for s in sessions_list if transcripts.copy_id(s) not in excl]

    bounds = manifest.get("bounds") or {}
    windows = {}
    for window_name in ("retained", "decision"):
        w = bounds.get(window_name) or {}
        start_raw, end_raw = w.get("start_utc"), w.get("end_utc")
        start = join.utc(start_raw) if start_raw else None
        end = join.utc(end_raw) if end_raw else None
        windows[window_name] = (start, end)

    ts_by_sid = {}
    for row in conn.execute("SELECT sid, ts FROM turns"):
        ts_by_sid.setdefault(row["sid"], []).append(row["ts"])

    def _parsed(ts_list):
        out = []
        for ts in ts_list:
            if not ts:
                continue
            try:
                out.append(join.utc(ts))
            except (ValueError, TypeError):
                continue
        return out

    result = {}
    for s in kept:
        cid = transcripts.copy_id(s)
        project = s.path.parent.name
        entry = result.setdefault(project, {"retained": 0, "decision": 0})
        dts = _parsed(ts_by_sid.get(cid, []))
        for window_name, (start, end) in windows.items():
            if start is None or end is None:
                continue
            if any(start <= dt <= end for dt in dts):
                entry[window_name] += 1
    return result


def _zero_flat_window():
    return {"decision": 0, "retained": 0}


def _zero_pop_window():
    return {
        "decision": {"top-level": 0, "subagent": 0, "all": 0},
        "retained": {"top-level": 0, "subagent": 0, "all": 0},
    }


def _window_counts(conn, windows):
    """One pass each over turns, tool_events and deliveries. Buckets every row into the
    DECISION window and the RETAINED window simultaneously (a row inside decision is also
    inside retained, since decision is a suffix of retained, but each is computed
    independently against `windows` rather than assumed), tracking population (top-level /
    subagent / all) for turns and the global observed ts span. A row with a NULL or
    unparseable ts is skipped from every window bucket but still counted via `skipped_ts`
    (Minor ruling) -- it never crashes coverage().
    """
    turns_by_kind = {}
    tool_events = {"total": _zero_flat_window(), AGENT_TOOL_NAME: _zero_flat_window()}
    deliveries = {
        "total": _zero_flat_window(),
        "transfer_filtered": _zero_flat_window(),
        "with_tool_use_id": _zero_flat_window(),
    }
    data_min = None
    data_max = None
    skipped_ts = 0

    def _parse(ts_raw):
        if not ts_raw:
            return None
        try:
            return join.utc(ts_raw)
        except (ValueError, TypeError):
            return None

    def _bump_span(dt):
        nonlocal data_min, data_max
        if data_min is None or dt < data_min:
            data_min = dt
        if data_max is None or dt > data_max:
            data_max = dt

    for row in conn.execute("SELECT agent_path, kind, ts FROM turns"):
        dt = _parse(row["ts"])
        if dt is None:
            skipped_ts += 1
            continue
        _bump_span(dt)
        pop = "subagent" if row["agent_path"] else "top-level"
        bucket = turns_by_kind.setdefault(row["kind"], _zero_pop_window())
        for window_name, (start, end) in windows.items():
            if start is not None and end is not None and start <= dt <= end:
                bucket[window_name][pop] += 1
                bucket[window_name]["all"] += 1

    for row in conn.execute("SELECT name, ts FROM tool_events"):
        dt = _parse(row["ts"])
        if dt is None:
            skipped_ts += 1
            continue
        _bump_span(dt)
        for window_name, (start, end) in windows.items():
            if start is not None and end is not None and start <= dt <= end:
                tool_events["total"][window_name] += 1
                if row["name"] == AGENT_TOOL_NAME:
                    tool_events[AGENT_TOOL_NAME][window_name] += 1

    for row in conn.execute("SELECT engine_or_hook, tool_use_id, ts FROM deliveries"):
        dt = _parse(row["ts"])
        if dt is None:
            skipped_ts += 1
            continue
        _bump_span(dt)
        for window_name, (start, end) in windows.items():
            if start is not None and end is not None and start <= dt <= end:
                deliveries["total"][window_name] += 1
                if row["engine_or_hook"] in TRANSFER_DELIVERY_MARKERS:
                    deliveries["transfer_filtered"][window_name] += 1
                if row["tool_use_id"]:
                    deliveries["with_tool_use_id"][window_name] += 1

    return {
        "turns": turns_by_kind,
        "tool_events": tool_events,
        "deliveries": deliveries,
        "data_min": data_min,
        "data_max": data_max,
        "skipped_ts": skipped_ts,
    }


def _field_window_count(win, field):
    """Returns (decision_count, retained_count, population_label) for a known coverage field
    name. RAISES KeyError for anything else -- R60: a MISSING or typo'd coverage field must
    raise, never silently read as 0.
    """
    if field is None:
        return (None, None, "n/a")
    if field == "assistant_text_turns":
        b = win["turns"].get("assistant_text", _zero_pop_window())
        return (b["decision"]["all"], b["retained"]["all"], "all")
    if field == "prompt_interrupt_turns":
        p = win["turns"].get("prompt", _zero_pop_window())
        i = win["turns"].get("interrupt", _zero_pop_window())
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
        b = win["turns"].get("delegation", _zero_pop_window())
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
    (R62's population line) but whose basis also cites a top-level breakdown (matching the
    reviewer's own citation shape: "assistant_text 4,375 (3,736 top-level)"). Returns None for
    a field with no meaningful top-level/subagent split.
    """
    if field == "assistant_text_turns":
        b = win["turns"].get("assistant_text", _zero_pop_window())
        return (b["decision"]["top-level"], b["retained"]["top-level"])
    if field == "prompt_interrupt_turns":
        p = win["turns"].get("prompt", _zero_pop_window())
        i = win["turns"].get("interrupt", _zero_pop_window())
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
    fields = {spec["field"] for spec in _BASE_TABLE.values() if spec.get("field")}
    fields.add("delegation_turns")
    return sorted(fields)


def coverage(events_db, corpus_dir):
    """R57/R60-R67 signature. Reads corpus_dir/manifest.json and events_db (opened read-only,
    closed on every exit path including a raise) and returns every coverage number the map
    needs: per-field DECISION-window and RETAINED-window counts (R62), the A1.6 appendix
    breakdowns render_map now also renders (R60), and the raw events_meta counters.

    RAISES ValueError if:
    - the exclusions recomputed here (transcripts.exclusions() over join.SPEC_EXCLUDED_SIDS,
      exactly as build_events would with its own default) disagree with events_meta's
      persisted kept/excluded counts; OR turns.sid is not a subset of the recomputed kept
      cids; OR the recomputed excluded cids intersect turns.sid (R64: three independent
      checks -- a build with a DIFFERENT excluded_sids set than coverage() assumes can trip
      any one of them even when the first two counts happen to coincide, which is exactly
      what the reviewer's swap fixture demonstrates);
    - manifest.bounds.retained.start_utc is later than the earliest surviving top-level entry,
      or any observed ts falls after retained.end_utc (R61).

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
        excluded_cids = set(excl.keys())
        turn_sids = {row["sid"] for row in conn.execute("SELECT DISTINCT sid FROM turns")}
        if not turn_sids.issubset(kept_cids):
            raise ValueError(
                "R64: turns.sid contains cid(s) not in the recomputed kept set: "
                f"{sorted(turn_sids - kept_cids)} -- events.db was built with a different "
                "excluded_sids set than coverage() assumes."
            )
        if excluded_cids & turn_sids:
            raise ValueError(
                "R64: the recomputed excluded cids intersect turns.sid: "
                f"{sorted(excluded_cids & turn_sids)} -- a session events.db actually kept "
                "(it has turns) is one coverage() recomputes as excluded; events.db was "
                "built with a different excluded_sids set than coverage() assumes."
            )

        bounds = manifest.get("bounds") or {}
        windows = {}
        for window_name in ("retained", "decision"):
            w = bounds.get(window_name) or {}
            start_raw, end_raw = w.get("start_utc"), w.get("end_utc")
            start = join.utc(start_raw) if start_raw else None
            end = join.utc(end_raw) if end_raw else None
            windows[window_name] = (start, end)

        win = _window_counts(conn, windows)

        kept_first_ts = [
            s.first_ts
            for s in sessions_list
            if transcripts.copy_id(s) not in excl and s.first_ts
        ]
        earliest_top_level_ts = (
            min(join.utc(t) for t in kept_first_ts) if kept_first_ts else None
        )

        retained_start, retained_end = windows["retained"]
        if earliest_top_level_ts is not None and retained_start is not None:
            if retained_start > earliest_top_level_ts:
                raise ValueError(
                    "R61: manifest.bounds.retained.start_utc "
                    f"({retained_start.isoformat()}) is later than the earliest surviving "
                    f"top-level entry ({earliest_top_level_ts.isoformat()}) -- the retained "
                    "window must start at or before the oldest kept transcript."
                )
        if win["data_max"] is not None and retained_end is not None:
            if win["data_max"] > retained_end:
                raise ValueError(
                    f"R61: an observed ts ({win['data_max'].isoformat()}) falls after "
                    f"manifest.bounds.retained.end_utc ({retained_end.isoformat()}) -- the "
                    "retained window must cover every observed ts."
                )

        tool_events_by_method = {}
        for row in conn.execute(
            "SELECT join_method, COUNT(*) c FROM tool_events GROUP BY join_method"
        ):
            tool_events_by_method[row["join_method"]] = row["c"]

        tool_events_by_day = {}
        for row in conn.execute(
            "SELECT substr(ts, 1, 10) AS day, join_method, COUNT(*) c FROM tool_events "
            "GROUP BY day, join_method ORDER BY day"
        ):
            tool_events_by_day.setdefault(row["day"], {})[row["join_method"]] = row["c"]

        first_exact_row = conn.execute(
            "SELECT MIN(ts) ts FROM tool_events WHERE join_method = 'exact'"
        ).fetchone()
        first_exact_tool_event_ts = first_exact_row["ts"] if first_exact_row else None

        heuristic_total = int(meta.get("tool_events_heuristic", 0))
        heuristic_via_called_at = int(meta.get("heuristic_via_called_at", 0))

        deliveries_by_source = {}
        for row in conn.execute("SELECT source, COUNT(*) c FROM deliveries GROUP BY source"):
            deliveries_by_source[row["source"]] = row["c"]

        turns_by_kind = {}
        for row in conn.execute("SELECT kind, COUNT(*) c FROM turns GROUP BY kind"):
            turns_by_kind[row["kind"]] = row["c"]

        attribution = transcripts.attribute_entries(sessions_list, excl)
        divergent_duplicate_unowned = _divergent_duplicate_unowned(
            sessions_list, excl, attribution
        )
        kept_sessions_with_zero_turns = _kept_sessions_with_zero_turns(
            sessions_list, excl, conn, attribution
        )

        sessions_per_project = _sessions_per_project(sessions_list, excl, conn, manifest)
        exclusions_by_reason = _exclusions_by_reason(excl)
        excluded_by_spec_detail = _excluded_by_spec_detail(sessions_list, excl)

        # R63/R67/R60(b): usage rows are read once, straight from join's own loader -- never
        # re-derived -- so "usage rows with no kept session", "of which K from the
        # spec-excluded session" and the exact-join citation count all agree with what
        # build_events actually saw.
        usage_rows = join._load_usage_rows(corpus_dir)
        kept_bare_sids = {s.sid for s in sessions_list if transcripts.copy_id(s) not in excl}
        usage_rows_total = len(usage_rows)
        usage_rows_unmapped = 0
        usage_rows_unmapped_spec_excluded = 0
        usage_rows_with_tool_use_id = 0
        for row in usage_rows:
            if row.get("tool_use_id"):
                usage_rows_with_tool_use_id += 1
            cc_sid = row.get("cc_session_id")
            if cc_sid not in kept_bare_sids:
                usage_rows_unmapped += 1
                if cc_sid in join.SPEC_EXCLUDED_SIDS:
                    usage_rows_unmapped_spec_excluded += 1

        field_results = {}
        for field in _all_declared_fields():
            dec, ret, pop = _field_window_count(win, field)
            field_results[field] = {"decision": dec, "retained": ret, "population": pop}

        at_top = _top_level_subset(win, "assistant_text_turns")
        pi_top = _top_level_subset(win, "prompt_interrupt_turns")
        deleg = field_results["delegation_turns"]

        joined_tool_events = (
            tool_events_by_method.get("exact", 0) + tool_events_by_method.get("heuristic", 0)
        )
        tool_events_total_count = int(
            meta.get("tool_events_total", sum(tool_events_by_method.values()))
        )
        deliveries_with_tid_dec = win["deliveries"]["with_tool_use_id"]["decision"]
        deliveries_with_tid_ret = win["deliveries"]["with_tool_use_id"]["retained"]
        deliveries_total_dec = win["deliveries"]["total"]["decision"]
        deliveries_total_ret = win["deliveries"]["total"]["retained"]

        cell_extra = {
            ("mistakes", "opportunity"): (
                f"of which top-level -- decision: {at_top[0]}, retained: {at_top[1]}"
            ),
            ("context", "opportunity"): (
                f"of which top-level -- decision: {at_top[0]}, retained: {at_top[1]}"
            ),
            ("mistakes", "signal/request"): (
                f"of which top-level -- decision: {pi_top[0]}, retained: {pi_top[1]}"
            ),
            ("context", "delivery/action"): (
                "share carrying a tool_use_id -- decision: "
                f"{deliveries_with_tid_dec} of {deliveries_total_dec}; retained: "
                f"{deliveries_with_tid_ret} of {deliveries_total_ret}"
            ),
            ("background-worker", "signal/request"): (
                "delegation turns (separate, never summed) -- decision: "
                f"{deleg['decision']}, retained: {deleg['retained']}"
            ),
        }

        return {
            "corpus_id": manifest.get("corpus_id"),
            "manifest_bounds": bounds,
            "meta": dict(meta),
            "data_min_ts": win["data_min"].isoformat() if win["data_min"] else None,
            "data_max_ts": win["data_max"].isoformat() if win["data_max"] else None,
            "skipped_ts_count": win["skipped_ts"],
            "tool_events_by_method": tool_events_by_method,
            "tool_events_by_day": tool_events_by_day,
            "first_exact_tool_event_ts": first_exact_tool_event_ts,
            "heuristic_via_called_at": heuristic_via_called_at,
            "heuristic_rest": heuristic_total - heuristic_via_called_at,
            "joined_tool_events": joined_tool_events,
            "tool_events_total_count": tool_events_total_count,
            "deliveries_by_source": deliveries_by_source,
            "hook_success_only": int(meta.get("hook_success_only", 0)),
            "hook_success_twins_dropped": int(meta.get("hook_success_twins_dropped", 0)),
            "usage_rows_total": usage_rows_total,
            "usage_rows_unmapped": usage_rows_unmapped,
            "usage_rows_unmapped_spec_excluded": usage_rows_unmapped_spec_excluded,
            "usage_rows_with_tool_use_id": usage_rows_with_tool_use_id,
            "sessions_per_project": sessions_per_project,
            "exclusions_by_reason": exclusions_by_reason,
            "excluded_by_spec_detail": excluded_by_spec_detail,
            "turns_by_kind": turns_by_kind,
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
    field = spec.get("field")

    if field is None:
        dec_number = None
        ret_number = None
        population = "n/a"
    else:
        entry = cov["fields"][field]
        dec_number = entry["decision"]
        ret_number = entry["retained"]
        population = entry["population"]

    extra = cov.get("cell_extra", {}).get((outcome, link))
    if extra:
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


def render_map(coverage, manifest):
    """R60: renders the WHOLE document -- header, provenance, windows, the four label tables
    plus the rediscovery row, and every A1.6 appendix breakdown (joins by method overall and
    by day, delivery coverage by source, sessions per project and window, exclusions by
    reason, turns by kind, divergent-duplicate unowned count, kept-sessions-with-zero-turns).
    The committed map file must be byte-identical to this function's output for the build it
    describes; there is no hand-editing step after it. Prose here is only (a) a data-derived
    statement or (b) the fixed Amendment 4(a) citation -- never an inferred cause.
    """
    lines = []
    lines.append(f"# Observability map -- {coverage.get('corpus_id')}")
    lines.append("")
    lines.append(f"Built: {manifest.get('created_utc')}.")
    repos = manifest.get("repos") or {}
    if repos:
        repo_bits = "; ".join(f"{name} at {sha}" for name, sha in sorted(repos.items()))
        lines.append(f"Repo HEAD at freeze time: {repo_bits}.")
    lines.append("")

    lines.append("## Windows")
    lines.append("")
    bounds = coverage.get("manifest_bounds") or {}
    for window_name in ("retained", "decision"):
        w = bounds.get(window_name) or {}
        lines.append(f"- {window_name}: {w.get('start_utc')} to {w.get('end_utc')}")
    lines.append("")
    lines.append(
        "Data span observed across turns, tool_events and deliveries: "
        f"{coverage.get('data_min_ts')} to {coverage.get('data_max_ts')} "
        f"({coverage.get('skipped_ts_count', 0)} rows skipped for a NULL or unparseable ts)."
    )
    lines.append("")

    cells = build_cells(coverage)
    by_outcome = {}
    for cell in cells:
        by_outcome.setdefault(cell["outcome"], []).append(cell)

    for outcome in _OUTCOMES:
        lines.append(f"## {outcome}")
        lines.append("")
        lines.append("| link | label | decision | retained | population | basis |")
        lines.append("|---|---|---|---|---|---|")
        for cell in by_outcome.get(outcome, []):
            dec = "n/a" if cell["decision_number"] is None else str(cell["decision_number"])
            ret = "n/a" if cell["retained_number"] is None else str(cell["retained_number"])
            lines.append(
                f"| {cell['link']} | {cell['label']} | {dec} | {ret} | {cell['population']} "
                f"| {cell['basis']} |"
            )
        lines.append("")

    lines.append("## Appendix")
    lines.append("")

    lines.append("### A1.6 -- joins by method (overall)")
    lines.append("")
    lines.append("| method | count |")
    lines.append("|---|---|")
    for method in sorted(coverage.get("tool_events_by_method", {})):
        lines.append(f"| {method} | {coverage['tool_events_by_method'][method]} |")
    lines.append("")
    lines.append(
        "Amendment 4(a): tool_use_id was NULL on every usage row until 6f6349ca; it appears "
        "per session from that session's /mcp. Usage rows with a non-NULL tool_use_id: "
        f"{coverage.get('usage_rows_with_tool_use_id', 0)}. First exact join ts (the "
        f"transcript tool_use ts): {coverage.get('first_exact_tool_event_ts')}."
    )
    lines.append("")
    lines.append(
        f"Heuristic joins: {coverage.get('tool_events_by_method', {}).get('heuristic', 0)} "
        f"total, of which {coverage.get('heuristic_via_called_at', 0)} matched via called_at "
        f"(started_at NULL) and {coverage.get('heuristic_rest', 0)} matched via started_at "
        "directly."
    )
    lines.append("")
    lines.append(
        "A1.5's latency share is measurable only for joined calls: "
        f"{coverage.get('joined_tool_events', 0)} of "
        f"{coverage.get('tool_events_total_count', 0)} tool_events are joined (exact + "
        "heuristic); latency_ms is only present on the usage rows behind those."
    )
    lines.append("")

    lines.append("### A1.6 -- joins by method (by day)")
    lines.append("")
    by_day = coverage.get("tool_events_by_day", {})
    all_methods = sorted({m for day in by_day.values() for m in day})
    if all_methods:
        lines.append("| day | " + " | ".join(all_methods) + " |")
        lines.append("|---|" + "---|" * len(all_methods))
        for day in sorted(by_day):
            row = by_day[day]
            lines.append(
                f"| {day} | " + " | ".join(str(row.get(m, 0)) for m in all_methods) + " |"
            )
    else:
        lines.append("(no tool_events)")
    lines.append("")

    lines.append("### A1.6 -- delivery coverage by source")
    lines.append("")
    lines.append("| source | count |")
    lines.append("|---|---|")
    for source in sorted(coverage.get("deliveries_by_source", {})):
        lines.append(f"| {source} | {coverage['deliveries_by_source'][source]} |")
    lines.append("")
    lines.append(
        f"hook_success_only: {coverage.get('hook_success_only', 0)}; "
        f"hook_success_twins_dropped: {coverage.get('hook_success_twins_dropped', 0)}."
    )
    lines.append("")
    lines.append(
        "usage rows with no kept session: "
        f"{coverage.get('usage_rows_unmapped', 0)} of {coverage.get('usage_rows_total', 0)} "
        f"usage rows read (of which {coverage.get('usage_rows_unmapped_spec_excluded', 0)} "
        "from the spec-excluded session)."
    )
    lines.append("")

    lines.append("### A1.6 -- sessions per project and window")
    lines.append("")
    lines.append("| project | retained | decision |")
    lines.append("|---|---|---|")
    for project in sorted(coverage.get("sessions_per_project", {})):
        entry = coverage["sessions_per_project"][project]
        lines.append(f"| {project} | {entry.get('retained', 0)} | {entry.get('decision', 0)} |")
    lines.append("")

    lines.append("### A1.6 -- exclusions by reason")
    lines.append("")
    lines.append("| reason | count |")
    lines.append("|---|---|")
    for reason in sorted(coverage.get("exclusions_by_reason", {})):
        lines.append(f"| {reason} | {coverage['exclusions_by_reason'][reason]} |")
    lines.append("")
    lines.append(f"excluded-by-spec, data-wise: {coverage.get('excluded_by_spec_detail', 'none')}")
    lines.append("")

    lines.append("### A1.6 -- turns by kind")
    lines.append("")
    lines.append("| kind | count |")
    lines.append("|---|---|")
    for kind in sorted(coverage.get("turns_by_kind", {})):
        lines.append(f"| {kind} | {coverage['turns_by_kind'][kind]} |")
    lines.append("")

    lines.append("### A1.6 -- divergent-duplicate unowned uuids")
    lines.append("")
    dd = coverage.get("divergent_duplicate_unowned", {})
    if dd:
        lines.append("| copy | unowned uuid count |")
        lines.append("|---|---|")
        for cid in sorted(dd):
            lines.append(f"| {cid} | {len(dd[cid])} |")
    else:
        lines.append("none")
    lines.append("")

    lines.append("### A1.6 -- kept sessions with zero turns")
    lines.append("")
    zt = coverage.get("kept_sessions_with_zero_turns") or {"count": 0, "detail": "none"}
    lines.append(f"count: {zt.get('count', 0)}.")
    lines.append("")
    lines.append(zt.get("detail", "none"))
    lines.append("")

    return "\n".join(lines)
