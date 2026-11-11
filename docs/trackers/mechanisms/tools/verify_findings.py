#!/usr/bin/env python3
"""Check miner output against its source files.

Usage: python3 -I verify_findings.py FINDINGS_JSON SHARD_DIR

For every finding, `evidence` must occur literally in the review file of its
agent_id (whitespace-normalised, backticks and markdown emphasis kept). Also
checks that category is in the closed set and that each review file in the
shard is covered. Prints a summary and every failing quote.
"""
import json
import os
import re
import sys

CATS = {"spec-gap", "test-weak", "edge-case", "wrong-behavior", "scope-creep",
        "dead-or-unwired", "doc-drift", "error-handling",
        "concurrency-or-ordering", "naming-or-structure", "report-inaccuracy",
        "other"}


def norm(s):
    # Markdown emphasis is formatting, not wording: miners strip it from quotes.
    s = re.sub(r"[*_]+", "", s)
    return re.sub(r"\s+", " ", s).strip()


def main():
    data = json.load(open(sys.argv[1]))
    shard = sys.argv[2]
    files = {f[:-3] for f in os.listdir(shard) if f.endswith(".md")}
    seen, n, bad_q, bad_c = set(), 0, [], []
    for rv in data["reviews"]:
        aid = rv["agent_id"]
        seen.add(aid)
        src = norm(open(os.path.join(shard, aid + ".md")).read()) if aid in files else None
        for f in rv["findings"]:
            n += 1
            if f.get("category") not in CATS:
                bad_c.append((aid, f.get("category")))
            if src is None or norm(f.get("evidence", "")) not in src:
                bad_q.append((aid, f.get("evidence")))
    print(f"reviews {len(seen)}/{len(files)} covered; findings {n}; "
          f"quotes not found {len(bad_q)}; bad categories {len(bad_c)}")
    for q in bad_q:
        print("QUOTE-MISS", q)
    for c in bad_c:
        print("BAD-CAT", c)
    print("missing:", sorted(files - seen))


if __name__ == "__main__":
    main()
