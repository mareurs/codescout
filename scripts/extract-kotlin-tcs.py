#!/usr/bin/env python3
"""Mine real (query, expected_files) pairs from a project's usage.db.

For each semantic_search call, collect the file paths touched by
read_file / edit_file / symbols / edit_code calls within 300s, in the
same session, by the same cc_session_id. The set of those paths is the
behavioral ground truth.

Output: JSON list of {id, query, expected_files[], when, session_id}.
"""
import argparse
import json
import os
import sqlite3
import sys
from datetime import datetime, timedelta

WINDOW_SECS = 300
MIN_EXPECTED = 1
TARGET_TOOLS = ("read_file", "edit_file", "symbols", "edit_code", "read_markdown", "edit_markdown")


def parse_ts(s: str) -> datetime:
    # SQLite datetime('now') => "YYYY-MM-DD HH:MM:SS"
    return datetime.strptime(s, "%Y-%m-%d %H:%M:%S")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("db", help="path to usage.db")
    ap.add_argument("-o", "--out", required=True, help="output JSON path")
    ap.add_argument("--max-tcs", type=int, default=30)
    args = ap.parse_args()

    if not os.path.exists(args.db):
        print(f"DB not found: {args.db}", file=sys.stderr)
        sys.exit(2)

    conn = sqlite3.connect(args.db)
    conn.row_factory = sqlite3.Row
    cur = conn.cursor()

    cur.execute(
        """
        SELECT id, called_at, session_id, cc_session_id, input_json
        FROM tool_calls
        WHERE tool_name = 'semantic_search'
          AND input_json IS NOT NULL
          AND outcome = 'success'
        ORDER BY called_at DESC
        """
    )
    sem_rows = cur.fetchall()

    follow_q = (
        "SELECT tool_name, called_at, input_json, output_json "
        "FROM tool_calls "
        "WHERE tool_name IN ({}) "
        "  AND session_id = ? "
        "  AND called_at > ? AND called_at <= ? "
        "  AND input_json IS NOT NULL".format(
            ",".join(f"'{t}'" for t in TARGET_TOOLS)
        )
    )

    tcs = []
    for r in sem_rows:
        try:
            qbody = json.loads(r["input_json"])
        except json.JSONDecodeError:
            continue
        query = qbody.get("query")
        if not query or len(query) < 5:
            continue
        try:
            t0 = parse_ts(r["called_at"])
        except ValueError:
            continue
        t1 = (t0 + timedelta(seconds=WINDOW_SECS)).strftime("%Y-%m-%d %H:%M:%S")
        cur.execute(follow_q, (r["session_id"], r["called_at"], t1))
        paths = []
        for f in cur.fetchall():
            try:
                ib = json.loads(f["input_json"])
            except json.JSONDecodeError:
                continue
            p = ib.get("path") or ib.get("file_path")
            if p and not p.startswith("/tmp") and not p.endswith(".lock"):
                paths.append(p)
        # dedupe preserving order
        seen = set()
        uniq = []
        for p in paths:
            if p not in seen:
                seen.add(p)
                uniq.append(p)
        if len(uniq) < MIN_EXPECTED:
            continue
        tcs.append(
            {
                "id": f"K-{len(tcs)+1:02d}",
                "query": query.strip(),
                "expected_files": uniq[:5],
                "when": r["called_at"],
                "session_id": r["session_id"],
            }
        )
        if len(tcs) >= args.max_tcs:
            break

    with open(args.out, "w") as f:
        json.dump(tcs, f, indent=2)

    print(f"wrote {len(tcs)} TCs to {args.out}", file=sys.stderr)
    for t in tcs[:5]:
        print(f"  {t['id']}: {t['query'][:60]}... -> {len(t['expected_files'])} files")


if __name__ == "__main__":
    main()
