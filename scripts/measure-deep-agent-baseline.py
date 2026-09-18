#!/usr/bin/env python3
"""Read-only aggregate baseline for a codescout usage.db window.

The script opens SQLite using mode=ro, starts one read transaction, and freezes
max(tool_calls.id) in that snapshot. It persists aggregates only: it never
exports inputs, outputs, paths, sessions, or error text.
"""
import argparse
import json
import math
import os
import sqlite3
from collections import Counter, defaultdict
from datetime import datetime, timezone, timedelta


def percentile(values, p):
    if not values:
        return None
    values = sorted(values)
    k = (len(values) - 1) * p
    lo, hi = math.floor(k), math.ceil(k)
    if lo == hi:
        return values[lo]
    return values[lo] + (values[hi] - values[lo]) * (k - lo)


def rate(n, d):
    return None if d == 0 else n / d


def where_window(start, end, max_id):
    return ("called_at >= ? AND called_at < ? AND id <= ?", (start, end, max_id))


def summarize(rows):
    calls = len(rows)
    outcomes = Counter(r["outcome"] for r in rows)
    errors = sum(r["outcome"] != "success" for r in rows)
    recoverable = outcomes["recoverable_error"]
    overflows = sum(bool(r["overflowed"]) for r in rows)
    latencies = [r["latency_ms"] for r in rows]
    tools = defaultdict(list)
    for r in rows:
        tools[r["tool_name"]].append(r)
    tool_rows = []
    for name, group in sorted(tools.items(), key=lambda x: (-len(x[1]), x[0])):
        ls = [r["latency_ms"] for r in group]
        tool_rows.append({
            "tool": name, "calls": len(group),
            "non_success_calls": sum(r["outcome"] != "success" for r in group),
            "overflowed_calls": sum(bool(r["overflowed"]) for r in group),
            "p50_latency_ms": percentile(ls, .50),
            "p95_latency_ms": percentile(ls, .95),
        })
    return {
        "calls": calls,
        "outcomes": dict(sorted(outcomes.items())),
        "non_success_calls": errors,
        "non_success_rate": rate(errors, calls),
        "recoverable_error_calls": recoverable,
        "recoverable_error_rate": rate(recoverable, calls),
        "overflowed_calls": overflows,
        "overflowed_rate": rate(overflows, calls),
        "latency_ms": {
            "p50": percentile(latencies, .50),
            "p95": percentile(latencies, .95),
            "p99": percentile(latencies, .99),
        },
        "input_json_present": sum(bool(r["input_json_present"]) for r in rows),
        "output_json_present": sum(bool(r["output_json_present"]) for r in rows),
        "session_id_present": sum(bool(r["session_id"]) for r in rows),
        "cc_session_id_present": sum(bool(r["cc_session_id"]) for r in rows),
        "distinct_nonempty_session_id": len({r["session_id"] for r in rows if r["session_id"]}),
        "distinct_nonempty_cc_session_id": len({r["cc_session_id"] for r in rows if r["cc_session_id"]}),
        "distinct_nonempty_project_root": len({r["project_root"] for r in rows if r["project_root"]}),
        "by_tool": tool_rows,
    }


def sequence_proxy(rows):
    """Immediate same-principal, same-error-family pairs; not task success/recovery."""
    ordered = sorted(
        (r for r in rows if r["cc_session_id"]),
        key=lambda r: (r["cc_session_id"], r["called_at"], r["id"]),
    )
    prev = {}
    comparable = repeated = 0
    by_family = Counter()
    for r in ordered:
        key = r["cc_session_id"]
        earlier = prev.get(key)
        if earlier is not None:
            comparable += 1
            if (r["err_family"] and r["err_family"] == earlier["err_family"]
                    and r["outcome"] != "success" and earlier["outcome"] != "success"):
                repeated += 1
                by_family[r["err_family"]] += 1
        prev[key] = r
    return {
        "definition": "adjacent rows ordered by (cc_session_id, called_at, id), both non-success with equal non-null err_family",
        "comparable_adjacent_pairs": comparable,
        "repeated_family_pairs": repeated,
        "repeated_family_pair_rate": rate(repeated, comparable),
        "by_err_family": [
            {"err_family": fam, "pairs": count}
            for fam, count in by_family.most_common()
        ],
        "warning": "This is a sequence proxy only. It is not task success, user recovery, or causal evidence; cc_session_id may blend parent/subagent activity.",
    }


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--db", required=True)
    ap.add_argument("--start", required=True, help="UTC inclusive calendar date, YYYY-MM-DD")
    ap.add_argument("--end", required=True, help="UTC exclusive calendar date, YYYY-MM-DD")
    ap.add_argument("--output", required=True)
    ap.add_argument("--max-id", type=int, help="optional inclusive frozen tool_calls.id cutoff")
    args = ap.parse_args()
    try:
        start_day = datetime.strptime(args.start, "%Y-%m-%d").date()
        end_day = datetime.strptime(args.end, "%Y-%m-%d").date()
    except ValueError:
        raise SystemExit("--start and --end must be UTC ISO calendar dates: YYYY-MM-DD")
    if start_day.isoformat() != args.start or end_day.isoformat() != args.end:
        raise SystemExit("--start and --end must use zero-padded UTC ISO dates: YYYY-MM-DD")
    if start_day >= end_day:
        raise SystemExit("--start must be earlier than --end")
    if os.path.realpath(args.output) == os.path.realpath(args.db):
        raise SystemExit("--output must not be the database path")
    uri = "file:" + args.db + "?mode=ro"
    con = sqlite3.connect(uri, uri=True, isolation_level=None)
    con.row_factory = sqlite3.Row
    con.execute("BEGIN")
    schema = con.execute("PRAGMA table_info(tool_calls)").fetchall()
    columns = [r["name"] for r in schema]
    required = {"id", "tool_name", "called_at", "latency_ms", "outcome", "overflowed",
                "input_json", "output_json", "session_id", "cc_session_id", "err_family",
                "project_root", "codescout_sha", "codescout_dirty"}
    missing = sorted(required - set(columns))
    if missing:
        raise SystemExit("required columns absent: " + ", ".join(missing))
    observed_max_id = con.execute("SELECT COALESCE(MAX(id), 0) FROM tool_calls").fetchone()[0]
    max_id = args.max_id if args.max_id is not None else observed_max_id
    if max_id < 0 or max_id > observed_max_id:
        raise SystemExit("--max-id must be within the database's observed id range")
    snapshot_utc = datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")
    where, params = where_window(args.start, args.end, max_id)
    selected = con.execute(
        "SELECT id, tool_name, called_at, latency_ms, outcome, overflowed, "
        "(input_json IS NOT NULL) AS input_json_present, "
        "(output_json IS NOT NULL) AS output_json_present, "
        "session_id, cc_session_id, err_family, project_root "
        "FROM tool_calls WHERE " + where + " ORDER BY id", params).fetchall()
    selected = [dict(r) for r in selected]
    direct = dict(con.execute(
        "SELECT COUNT(*) AS calls, "
        "COALESCE(SUM(outcome <> 'success'), 0) AS non_success_calls, "
        "COALESCE(SUM(outcome = 'recoverable_error'), 0) AS recoverable_error_calls, "
        "COALESCE(SUM(overflowed <> 0), 0) AS overflowed_calls, "
        "COALESCE(SUM(input_json IS NOT NULL), 0) AS input_json_present, "
        "COALESCE(SUM(output_json IS NOT NULL), 0) AS output_json_present, "
        "MIN(called_at) AS first_called_at_utc, MAX(called_at) AS last_called_at_utc, "
        "COUNT(DISTINCT NULLIF(codescout_sha, '')) AS distinct_nonempty_codescout_sha, "
        "COALESCE(SUM(codescout_dirty = 1), 0) AS codescout_dirty_true, "
        "COALESCE(SUM(codescout_dirty = 0), 0) AS codescout_dirty_false, "
        "COALESCE(SUM(codescout_dirty IS NULL), 0) AS codescout_dirty_null "
        "FROM tool_calls WHERE " + where, params).fetchone())
    direct_by_tool = [dict(r) for r in con.execute(
        "SELECT tool_name, COUNT(*) AS calls FROM tool_calls WHERE " + where
        + " GROUP BY tool_name ORDER BY tool_name", params).fetchall()]
    summary = summarize(selected)
    selected_by_tool_total = sum(x["calls"] for x in summary["by_tool"])
    direct_by_tool_total = sum(x["calls"] for x in direct_by_tool)
    headline_keys = ["calls", "non_success_calls", "recoverable_error_calls",
                     "overflowed_calls", "input_json_present", "output_json_present"]
    calibration = {
        "direct_sql_aggregate": direct,
        "direct_sql_by_tool": {"groups": len(direct_by_tool), "calls_sum": direct_by_tool_total},
        "matches_projection_summary": all(direct[k] == summary[k] for k in headline_keys),
        "matches_by_tool_calls_sum": direct_by_tool_total == selected_by_tool_total == summary["calls"],
    }
    if not calibration["matches_projection_summary"] or not calibration["matches_by_tool_calls_sum"]:
        raise RuntimeError("aggregate calibration mismatch; JSON was not written")
    summary["coverage_utc"] = {
        "first_called_at": direct["first_called_at_utc"],
        "last_called_at": direct["last_called_at_utc"],
    }
    summary["codescout_build_composition"] = {
        "distinct_nonempty_sha_count": direct["distinct_nonempty_codescout_sha"],
        "dirty_true_calls": direct["codescout_dirty_true"],
        "dirty_false_calls": direct["codescout_dirty_false"],
        "dirty_null_calls": direct["codescout_dirty_null"],
    }
    last7_start = ((end_day - timedelta(days=7)).isoformat()
                   if (end_day - start_day).days == 14 else None)
    out = {
        "method": {
            "db_open": "sqlite URI mode=ro; one BEGIN read transaction",
            "window_utc": {"start_inclusive": args.start, "end_exclusive": args.end},
            "snapshot_utc": snapshot_utc,
            "max_tool_calls_id_in_snapshot": max_id,
            "max_id_source": "argument" if args.max_id is not None else "max(id) observed in transaction",
            "observed_max_tool_calls_id": observed_max_id,
            "row_filter": "called_at >= start AND called_at < end AND id <= snapshot max id",
            "raw_sensitive_content_persisted": False,
            "scope": "all tool_calls rows in this one usage.db matching the UTC window and id cutoff; not all machine usage and not a single project root",
        },
        "schema_columns": columns,
        "window": summary,
        "sequence_proxy": sequence_proxy(selected),
        "calibration": calibration,
    }
    if last7_start:
        first = [r for r in selected if r["called_at"] < last7_start]
        last = [r for r in selected if r["called_at"] >= last7_start]
        out["comparison_last7_vs_preceding7"] = {
            "preceding7_utc": {"start_inclusive": args.start, "end_exclusive": last7_start, "summary": summarize(first)},
            "last7_utc": {"start_inclusive": last7_start, "end_exclusive": args.end, "summary": summarize(last)},
        }
    con.execute("COMMIT")
    with open(args.output, "w", encoding="utf-8") as f:
        json.dump(out, f, indent=2, sort_keys=True)
        f.write("\n")


if __name__ == "__main__":
    main()
