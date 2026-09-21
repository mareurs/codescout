#!/usr/bin/env python3
"""Measure predicate candidates for the deep-agent rung-1a observer phase.

READ-ONLY. Opens `.codescout/usage.db` with `mode=ro` in a single deferred read
transaction against a frozen `max_id` cutoff, so every figure describes one
snapshot even while sessions keep writing.

WHAT THIS ANSWERS (pre-registered; see the research artifact for results):
  Q1 base rates and denominator
  Q2 reader thrashing      — repeat reads of one path within a session
  Q3 suspicious zeros      — grep calls returning nothing
  Q4 overflow economics    — what a session does on the call AFTER an overflow
  Q5 sequence shape        — tool bigrams within a session

WHAT IT DELIBERATELY DOES NOT DO:
  * It never projects `input_json` / `output_json` CONTENT. Structural fields
    (a path, a pattern) are read to compute aggregates and are hashed or
    classified before they leave this process. No path, pattern or code
    fragment reaches stdout. The 2026-09-18 baseline set this rule; it holds
    here because the same columns hold real source.
  * It never writes to usage.db.

THREE THINGS THAT WOULD MAKE A READER MIS-TRUST THE OUTPUT, stated because
each one returns a plausible number rather than an error:

  1. TOOL NAMES CUT OVER ON 2026-09-03 and the retained corpus spans it.
     `read_markdown`->`read_file`, `edit_markdown`->`edit_file`,
     `artifact*`->`doc`. A per-tool count that does not union the aliases is a
     floor that reads like a total — `read_file` is understated by 16.3% and
     `doc` by 42% without the union. `--no-alias` reproduces the wrong figure
     on purpose so the gap can be measured rather than asserted.

  2. SESSION KEY. `session_id` is per-PROCESS and correct. `cc_session_id` is
     per-conversation and wrong on ~31% of rows
     (docs/issues/2026-08-20-telemetry-session-id-frozen-while-the-ledger-re-keys-per-call.md).
     This script keys on `session_id`. NOTE the 2026-09-18 baseline sequenced
     on `cc_session_id`; its sequence figures and these are not comparable.

  3. ORDERING IS BY COMPLETION, AT ONE-SECOND RESOLUTION. `called_at` is
     written after the call returns, and `id` is assigned at INSERT, so both
     order by completion, not by start. Q4 and Q5 are sequence questions and
     inherit that. The script reports `adjacent_pairs_sharing_a_second` so the
     reader can size the ambiguity instead of trusting the sort.
     `started_at` (the fix in 1dd363eb) is NOT in the live schema yet — the
     running binary predates it — so no better ordering is available today.

POSITIVE CONTROLS (PROBES.md rule 4: the predicate you supplied from memory is
the one that fails silently). Every extractor below is made to rank or find a
case whose answer is already known, and the control result is printed. The
zero-match control exists because `LIKE '%0 matches%'` also matches
"10 matches" and "20 matches" — a wrong answer with no zero in it.

Usage:
    python3 scripts/probe-predicate-candidates.py [--db PATH] [--json] [--no-alias]
"""

from __future__ import annotations

import argparse
import json
import sqlite3
import sys
from collections import Counter, defaultdict
from pathlib import Path

# Tool renames shipped 2026-09-02/03. The retained 30-day corpus spans the
# cutover, so every per-tool figure must union these or silently under-report.
TOOL_ALIASES = {
    "read_markdown": "read_file",
    "edit_markdown": "edit_file",
    "artifact": "doc",
    "artifact_event": "doc",
    "artifact_augment": "doc",
    "artifact_refresh": "doc",
}

# A read whose argument is an @ref handle is a BUFFER read, not a file read.
# Counting it as a repeat visit to a path would invent thrashing that did not
# happen — the caller is paging a result it already holds.
REF_PREFIXES = ("@cmd_", "@tool_", "@file_", "@ack_", "@bg_")

# Arguments that narrow a read to part of a file. A second read of one path
# carrying one of these is PAGING, which is the designed interaction, not
# thrashing. Separating the two is the whole point of Q2.
NARROWING_KEYS = frozenset(
    {"start_line", "end_line", "heading", "headings", "json_path",
     "toml_key", "offset", "limit", "occurrence", "output_id"}
)


def canon(tool: str) -> str:
    return TOOL_ALIASES.get(tool, tool)


def jload(s):
    if not s:
        return None
    try:
        return json.loads(s)
    except (ValueError, TypeError):
        return None


def read_target(inp) -> str | None:
    """The path a read call names, or None if it is not a plain file read.

    Returns None for @ref buffer reads deliberately: see REF_PREFIXES.
    """
    if not isinstance(inp, dict):
        return None
    p = inp.get("path") or inp.get("file_path") or inp.get("file")
    if not isinstance(p, str) or not p:
        return None
    if p.startswith(REF_PREFIXES):
        return None
    return p


def is_narrowed(inp) -> bool:
    return isinstance(inp, dict) and any(k in inp for k in NARROWING_KEYS)


def grep_zero(out_json: str | None) -> bool:
    """True when a grep returned no matches.

    EXACT prefix, not a substring. `LIKE '%0 matches%'` also matches
    "10 matches" / "20 matches" / "100 matches" — the control below measures
    exactly how many rows that mistake would have added.
    """
    if not out_json:
        return False
    blocks = jload(out_json)
    if not isinstance(blocks, list) or not blocks:
        return False
    first = blocks[0]
    if not isinstance(first, dict):
        return False
    text = first.get("text")
    return isinstance(text, str) and text.startswith("0 matches")


def pattern_is_pathish(inp) -> bool:
    """Does the grep pattern look like it is reaching for a path?

    The ADR this would mechanise (docs/adrs/2026-08-27-negative-results-name-
    their-scope.md) is about a zero whose SCOPE is suspect. A pattern carrying
    a separator or glob metacharacter is the cheap tell.
    """
    if not isinstance(inp, dict):
        return False
    pat = inp.get("pattern")
    if not isinstance(pat, str):
        return False
    return "/" in pat or "*" in pat


def load(db: Path, no_alias: bool):
    uri = f"file:{db}?mode=ro"
    conn = sqlite3.connect(uri, uri=True)
    conn.row_factory = sqlite3.Row
    cur = conn.cursor()
    cur.execute("BEGIN DEFERRED")
    # Frozen cutoff: everything below describes rows <= max_id at this instant.
    max_id = cur.execute("SELECT MAX(id) FROM tool_calls").fetchone()[0]
    cols = {r[1] for r in cur.execute("PRAGMA table_info(tool_calls)")}
    rows = cur.execute(
        "SELECT id, tool_name, called_at, latency_ms, outcome, overflowed, "
        "       session_id, project_root, input_json, output_json "
        "FROM tool_calls WHERE id <= ? ORDER BY session_id, called_at, id",
        (max_id,),
    ).fetchall()
    out = []
    for r in rows:
        d = dict(r)
        d["tool"] = d["tool_name"] if no_alias else canon(d["tool_name"])
        out.append(d)
    span = cur.execute(
        "SELECT MIN(called_at), MAX(called_at) FROM tool_calls WHERE id <= ?", (max_id,)
    ).fetchone()
    # Calibration: the aggregate projection above must agree with direct SQL.
    direct = cur.execute(
        "SELECT COUNT(*), SUM(overflowed), SUM(outcome!='success'), "
        "       COUNT(input_json), COUNT(output_json) "
        "FROM tool_calls WHERE id <= ?", (max_id,)
    ).fetchone()
    conn.rollback()
    conn.close()
    return out, max_id, span, cols, direct


def q1(rows, cols, span, max_id, direct):
    per = Counter(r["tool"] for r in rows)
    roots = Counter(r["project_root"] or "(null)" for r in rows)
    return {
        "frozen_max_id": max_id,
        "covered_utc_first": span[0],
        "covered_utc_last": span[1],
        "total_rows": len(rows),
        "distinct_session_id": len({r["session_id"] for r in rows}),
        "distinct_project_root": len(roots),
        "all_roots_are_codescout": all(
            "codescout" in (k or "") for k in roots if k != "(null)"
        ),
        "schema_has_started_at": "started_at" in cols,
        "schema_has_agent_id": "agent_id" in cols,
        "rows_with_subsecond_called_at": sum(1 for r in rows if "." in r["called_at"]),
        "input_json_present": sum(1 for r in rows if r["input_json"]),
        "output_json_present": sum(1 for r in rows if r["output_json"]),
        "overflowed": sum(r["overflowed"] for r in rows),
        "nonsuccess": sum(1 for r in rows if r["outcome"] != "success"),
        "per_tool": dict(per.most_common()),
        "calibration_projection_matches_sql": [
            len(rows) == direct[0],
            sum(r["overflowed"] for r in rows) == (direct[1] or 0),
            sum(1 for r in rows if r["outcome"] != "success") == (direct[2] or 0),
            sum(1 for r in rows if r["input_json"]) == direct[3],
            sum(1 for r in rows if r["output_json"]) == direct[4],
        ],
    }


def q2(rows):
    """Repeat reads of one path within one session.

    Reported three ways, because they answer three different questions and a
    single 'repeat rate' would silently pick one:
      * any_repeat      — the path was seen before in this session, at all
      * narrowed_repeat — ...and this call narrows (paging: the DESIGNED path)
      * identical_args  — ...and the arguments are byte-identical

    identical_args is a CANDIDATE population, not a redundancy verdict: this
    loop skips every non-read_file row, so an intervening edit_file/edit_code on
    the same path is invisible and a correct re-read of CHANGED content lands
    here too. Argument equality is not result equality.
    docs/issues/2026-09-20-predicate-probe-overstates-retrieval-and-redundancy.md
    """
    seen: dict[tuple, int] = defaultdict(int)
    seen_args: dict[tuple, int] = defaultdict(int)
    total = first = any_rep = narrowed = identical = 0
    depth = Counter()
    for r in rows:
        if r["tool"] != "read_file":
            continue
        inp = jload(r["input_json"])
        tgt = read_target(inp)
        if tgt is None:
            continue
        total += 1
        k = (r["session_id"], tgt)
        ak = (r["session_id"], tgt, json.dumps(inp, sort_keys=True))
        if seen[k] == 0:
            first += 1
        else:
            any_rep += 1
            if is_narrowed(inp):
                narrowed += 1
            if seen_args[ak] > 0:
                identical += 1
        seen[k] += 1
        seen_args[ak] += 1
    for v in seen.values():
        depth[min(v, 10)] += 1
    return {
        "read_file_calls_naming_a_path": total,
        "first_visits": first,
        "any_repeat": any_rep,
        "any_repeat_pct": round(100.0 * any_rep / total, 2) if total else 0.0,
        "repeat_that_is_narrowing": narrowed,
        "repeat_that_is_narrowing_pct": round(100.0 * narrowed / any_rep, 2) if any_rep else 0.0,
        "repeat_with_identical_args": identical,
        "repeat_with_identical_args_pct": round(100.0 * identical / total, 2) if total else 0.0,
        "distinct_session_path_pairs": len(seen),
        "visits_per_pair_distribution_capped_at_10": dict(sorted(depth.items())),
    }


def q3(rows):
    greps = [r for r in rows if r["tool"] == "grep"]
    with_out = [r for r in greps if r["output_json"]]
    zero = [r for r in with_out if grep_zero(r["output_json"])]
    zero_pathish = sum(1 for r in zero if pattern_is_pathish(jload(r["input_json"])))
    nonzero_pathish = sum(
        1 for r in with_out
        if not grep_zero(r["output_json"]) and pattern_is_pathish(jload(r["input_json"]))
    )
    # POSITIVE CONTROL: the naive substring test must over-count, and by a
    # measurable amount. If these are equal the control has not fired and the
    # exact-prefix claim is unproven, not confirmed.
    naive = sum(1 for r in with_out if "0 matches" in (r["output_json"] or ""))
    return {
        "grep_calls": len(greps),
        "grep_with_output": len(with_out),
        "zero_match": len(zero),
        "zero_match_pct_of_with_output": round(100.0 * len(zero) / len(with_out), 2) if with_out else 0.0,
        "zero_match_pattern_pathish": zero_pathish,
        "zero_match_pathish_pct": round(100.0 * zero_pathish / len(zero), 2) if zero else 0.0,
        "nonzero_match_pattern_pathish": nonzero_pathish,
        "nonzero_pathish_base_rate_pct": round(
            100.0 * nonzero_pathish / (len(with_out) - len(zero)), 2
        ) if (len(with_out) - len(zero)) else 0.0,
        "control_naive_substring_count": naive,
        "control_naive_overcounts_by": naive - len(zero),
        "control_fired": naive > len(zero),
    }


def classify_next(prev, nxt):
    """Classify the ONE call recorded after `prev`. Not a retrieval test.

    Two limits, running in OPPOSITE directions, so the result bounds an
    eventual-retrieval rate in neither:
      * horizon — only seq[i+1] is examined, so a buffer read two or more calls
        later is invisible (retrieval UNDERcounted);
      * linkage — 'queried_the_buffer' fires on any REF_PREFIXES string in the
        next input; the handle `prev` actually emitted is never compared, so
        reading an UNRELATED buffer counts (retrieval OVERcounted).
    Any retrieval claim needs handle matching over a declared horizon.
    docs/issues/2026-09-20-predicate-probe-overstates-retrieval-and-redundancy.md
    """
    if nxt is None:
        return "session_ended"
    ni = jload(nxt["input_json"])
    blob = json.dumps(ni) if ni is not None else ""
    if any(p in blob for p in REF_PREFIXES):
        return "queried_the_buffer"
    if nxt["tool"] != prev["tool"]:
        return "switched_tool"
    pi = jload(prev["input_json"])
    if isinstance(ni, dict) and isinstance(pi, dict):
        if json.dumps(ni, sort_keys=True) == json.dumps(pi, sort_keys=True):
            return "reran_identical"
        if is_narrowed(ni) and not is_narrowed(pi):
            return "same_tool_narrowed"
    return "same_tool_other_args"


def q4(rows):
    by_sess = defaultdict(list)
    for r in rows:
        by_sess[r["session_id"]].append(r)
    out = Counter()
    ties = adj = 0
    for seq in by_sess.values():
        for i, r in enumerate(seq):
            nxt = seq[i + 1] if i + 1 < len(seq) else None
            if nxt is not None:
                adj += 1
                if nxt["called_at"] == r["called_at"]:
                    ties += 1
            if r["overflowed"]:
                out[classify_next(r, nxt)] += 1
    tot = sum(out.values())
    return {
        "overflowed_calls": tot,
        "next_call_classification": dict(out.most_common()),
        "next_call_pct": {k: round(100.0 * v / tot, 2) for k, v in out.most_common()} if tot else {},
        "adjacent_pairs": adj,
        "adjacent_pairs_sharing_a_second": ties,
        "tie_pct": round(100.0 * ties / adj, 2) if adj else 0.0,
    }


def q5(rows, top=20):
    by_sess = defaultdict(list)
    for r in rows:
        by_sess[r["session_id"]].append(r)
    bi = Counter()
    for seq in by_sess.values():
        for a, b in zip(seq, seq[1:]):
            bi[(a["tool"], b["tool"])] += 1
    tot = sum(bi.values())
    return {
        "total_bigrams": tot,
        "top": [
            {"from": a, "to": b, "n": n, "pct": round(100.0 * n / tot, 2)}
            for (a, b), n in bi.most_common(top)
        ],
        "self_transition_pct": round(
            100.0 * sum(n for (a, b), n in bi.items() if a == b) / tot, 2
        ) if tot else 0.0,
    }


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--db", type=Path, default=Path(".codescout/usage.db"))
    ap.add_argument("--json", action="store_true")
    ap.add_argument("--no-alias", action="store_true",
                    help="do NOT union the 2026-09-03 tool renames — reproduces the "
                         "under-reported per-tool figures on purpose")
    a = ap.parse_args()
    if not a.db.exists():
        print(f"no such db: {a.db}", file=sys.stderr)
        return 2
    rows, max_id, span, cols, direct = load(a.db, a.no_alias)
    rep = {
        "db": str(a.db),
        "alias_union_applied": not a.no_alias,
        "session_key": "session_id",
        "q1_base_rates": q1(rows, cols, span, max_id, direct),
        "q2_reader_thrashing": q2(rows),
        "q3_suspicious_zeros": q3(rows),
        "q4_overflow_economics": q4(rows),
        "q5_sequence_shape": q5(rows),
    }
    if a.json:
        print(json.dumps(rep, indent=2))
        return 0
    print(json.dumps(rep, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
