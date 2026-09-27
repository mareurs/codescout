"""Stage 1c: the observability map (Task 13).

Spec A1.6 requires this map before any Codex-judge call: for every outcome and every
causal-chain link, a label (measurable now / needs adjudication / unobservable) plus the
coverage number from the events database that justifies it, or the reason none exists.

Consumes Task 6's events.db (scripts/measure/join.py, do not modify) and Task 5's transcript
helpers (scripts/measure/transcripts.py, do not modify). Produces two things:
- coverage(events_db, corpus_dir) -> dict: the raw counts.
- render_map(coverage) -> str: the markdown table, one section per outcome.

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

_LINKS = ("opportunity", "signal/request", "delivery/action", "observed use", "checked outcome")
_OUTCOMES = ("mistakes", "context", "transfer", "background-worker")

# R58 (controller ruling): the map's BASE labels, as a table -- not scattered ifs. `field`
# names a key into coverage()["fields"]; a cell with field=None has no computable aggregate
# and its `basis` text stands in as the reason none exists. A base label of "measurable now"
# whose field's coverage number is 0 is downgraded to "needs adjudication" by
# _resolve_cell() below -- never encode that downgrade here.
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
        "basis": "deliveries before the decision; their relevance is judged",
        "field": None,
    },
    ("mistakes", "observed use"): {
        "label": "needs adjudication",
        "basis": "next_action_aligned (Amendment 2(c))",
        "field": None,
    },
    ("mistakes", "checked outcome"): {
        "label": "needs adjudication",
        "basis": "the Codex judge, plus the operator's 25-item spot-check",
        "field": None,
    },
    ("context", "opportunity"): {
        "label": "measurable now",
        "basis": "assistant_text turns",
        "field": "assistant_text_turns",
    },
    ("context", "signal/request"): {
        "label": "measurable now",
        "basis": "tool_use requests",
        "field": "tool_use_turns",
    },
    ("context", "delivery/action"): {
        "label": "measurable now",
        "basis": "deliveries by source, with join coverage",
        "field": "deliveries_total",
    },
    ("context", "observed use"): {
        "label": "needs adjudication",
        "basis": "evidence_used",
        "field": None,
    },
    ("context", "checked outcome"): {
        "label": "needs adjudication",
        "basis": "the judge, plus the spot-check",
        "field": None,
    },
    ("transfer", "opportunity"): {
        "label": "needs adjudication",
        "basis": "lesson applicability is judged",
        "field": None,
    },
    ("transfer", "signal/request"): {
        "label": "needs adjudication",
        "basis": "the lesson inventory is Task 7, not yet built; state that reason",
        "field": None,
    },
    ("transfer", "delivery/action"): {
        "label": "measurable now",
        "basis": "operator-rule (OP-N) and get_guide deliveries",
        "field": "operator_rule_and_guide_deliveries",
    },
    ("transfer", "observed use"): {
        "label": "needs adjudication",
        "basis": "applied / missed",
        "field": None,
    },
    ("transfer", "checked outcome"): {
        "label": "needs adjudication",
        "basis": "the judge, plus the spot-check",
        "field": None,
    },
    ("background-worker", "opportunity"): {
        "label": "measurable now",
        "basis": "tool calls by A1.5 task family",
        "field": "tool_events_total",
    },
    ("background-worker", "signal/request"): {
        "label": "measurable now",
        "basis": "Agent tool_uses plus delegation turns",
        "field": "agent_tool_uses_plus_delegation_turns",
    },
    ("background-worker", "delivery/action"): {
        "label": "measurable now",
        "basis": "subagent turns (agent_path set)",
        "field": "subagent_turns",
    },
    ("background-worker", "observed use"): {
        "label": "needs adjudication",
        "basis": "did the parent use the result",
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


def _sessions_per_project(sessions_list, excl, conn, manifest):
    """Sessions per project (the project dir under transcripts/NN-<profile>/), split by
    window (retained, decision). A session is in a window if any of its turns' ts falls
    inside that window's [start_utc, end_utc] -- build_events applies no bounds filter to
    turns itself, so this membership check is coverage()'s own responsibility.
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

    result = {}
    for s in kept:
        cid = transcripts.copy_id(s)
        project = s.path.parent.name
        entry = result.setdefault(project, {"retained": 0, "decision": 0})
        ts_list = ts_by_sid.get(cid, [])
        for window_name, (start, end) in windows.items():
            if start is None or end is None:
                continue
            if any(start <= join.utc(ts) <= end for ts in ts_list):
                entry[window_name] += 1
    return result


def coverage(events_db, corpus_dir):
    """R57 signature. Reads corpus_dir/manifest.json and events_db, and returns a dict of
    every coverage number the map needs, plus the raw events_meta counters.

    RAISES a ValueError if the exclusions recomputed here (via transcripts.exclusions() over
    join.SPEC_EXCLUDED_SIDS, exactly as build_events would with its own default) disagree
    with the kept/excluded counts events.db actually persisted -- meaning the build used a
    different excluded_sids set than this function always assumes.
    """
    corpus_dir = pathlib.Path(corpus_dir)
    manifest = json.loads((corpus_dir / "manifest.json").read_text())

    conn = sqlite3.connect(str(events_db))
    conn.row_factory = sqlite3.Row

    meta = {row["key"]: row["value"] for row in conn.execute("SELECT key, value FROM events_meta")}

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
            f"excluded={recomputed_excluded}, but events_meta on events_db records "
            f"sessions_kept={persisted_kept} sessions_excluded={persisted_excluded} -- "
            "events.db was built with a different excluded_sids set than coverage() assumes, "
            "so every count below would be measuring the wrong corpus."
        )

    tool_events_by_method = {}
    for row in conn.execute("SELECT join_method, COUNT(*) c FROM tool_events GROUP BY join_method"):
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
    divergent_duplicate_unowned = _divergent_duplicate_unowned(sessions_list, excl, attribution)

    sessions_per_project = _sessions_per_project(sessions_list, excl, conn, manifest)
    exclusions_by_reason = _exclusions_by_reason(excl)

    agent_tool_uses = conn.execute(
        "SELECT COUNT(*) c FROM tool_events WHERE name = ?", (AGENT_TOOL_NAME,)
    ).fetchone()["c"]
    delegation_turns = turns_by_kind.get("delegation", 0)
    subagent_turns = conn.execute(
        "SELECT COUNT(*) c FROM turns WHERE agent_path IS NOT NULL AND agent_path != ''"
    ).fetchone()["c"]
    operator_rule_and_guide_deliveries = conn.execute(
        "SELECT COUNT(*) c FROM deliveries WHERE engine_or_hook IN ('operator-rule', 'get_guide')"
    ).fetchone()["c"]

    tool_events_total = int(meta.get("tool_events_total", sum(tool_events_by_method.values())))
    deliveries_total = int(meta.get("deliveries_total", sum(deliveries_by_source.values())))

    fields = {
        "assistant_text_turns": turns_by_kind.get("assistant_text", 0),
        "prompt_interrupt_turns": turns_by_kind.get("prompt", 0) + turns_by_kind.get("interrupt", 0),
        "tool_use_turns": turns_by_kind.get("tool_use", 0),
        "deliveries_total": deliveries_total,
        "operator_rule_and_guide_deliveries": operator_rule_and_guide_deliveries,
        "tool_events_total": tool_events_total,
        "agent_tool_uses_plus_delegation_turns": agent_tool_uses + delegation_turns,
        "subagent_turns": subagent_turns,
    }

    conn.close()

    return {
        "corpus_id": manifest.get("corpus_id"),
        "meta": dict(meta),
        "tool_events_by_method": tool_events_by_method,
        "tool_events_by_day": tool_events_by_day,
        "first_exact_tool_event_ts": first_exact_tool_event_ts,
        "heuristic_via_called_at": heuristic_via_called_at,
        "heuristic_rest": heuristic_total - heuristic_via_called_at,
        "deliveries_by_source": deliveries_by_source,
        "hook_success_only": int(meta.get("hook_success_only", 0)),
        "hook_success_twins_dropped": int(meta.get("hook_success_twins_dropped", 0)),
        "deliveries_unmapped_session": int(meta.get("deliveries_unmapped_session", 0)),
        "sessions_per_project": sessions_per_project,
        "exclusions_by_reason": exclusions_by_reason,
        "turns_by_kind": turns_by_kind,
        "divergent_duplicate_unowned": divergent_duplicate_unowned,
        "fields": fields,
    }


def _resolve_cell(cov, outcome, link, spec):
    label = spec["label"]
    basis = spec["basis"]
    field = spec.get("field")
    number = cov["fields"].get(field) if field else None
    if label == "measurable now" and not number:
        label = "needs adjudication"
        basis = basis + " -- downgraded: the coverage number for this link is 0, so nothing " \
                         "is measurable now"
    return {"outcome": outcome, "link": link, "label": label, "basis": basis, "number": number}


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


def render_map(coverage):
    """R58: for each outcome and chain link, one of measurable now / needs adjudication /
    unobservable, with the coverage number that justifies it, or the reason none exists.
    """
    cells = build_cells(coverage)
    by_outcome = {}
    for cell in cells:
        by_outcome.setdefault(cell["outcome"], []).append(cell)

    lines = []
    for outcome in _OUTCOMES:
        lines.append(f"## {outcome}")
        lines.append("")
        lines.append("| link | label | coverage | basis |")
        lines.append("|---|---|---|---|")
        for cell in by_outcome.get(outcome, []):
            number_text = "n/a" if cell["number"] is None else str(cell["number"])
            lines.append(f"| {cell['link']} | {cell['label']} | {number_text} | {cell['basis']} |")
        lines.append("")
    return "\n".join(lines)
