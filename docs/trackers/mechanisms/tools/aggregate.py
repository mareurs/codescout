#!/usr/bin/env python3
"""Verify every miner shard and aggregate the verified findings.

Usage: python3 -I aggregate.py OUT_DIR SHARD_DIR RESULT_JSON

Reads OUT_DIR/shard-N.json, checks each finding's quote against
SHARD_DIR/shard-N/<agent_id>.md, drops findings whose quote is not found
(they are counted, not trusted), and writes RESULT_JSON with the verified
findings plus counts by category, mechanical, implementer_preventable,
rereview, verdict. A finding keeps `verified: true` only if its quote matched.
"""
import collections
import json
import os
import re
import sys

CATS = {"spec-gap", "test-weak", "edge-case", "wrong-behavior", "scope-creep",
        "dead-or-unwired", "doc-drift", "error-handling",
        "concurrency-or-ordering", "naming-or-structure", "report-inaccuracy",
        "other"}


def norm(s):
    s = re.sub(r"[*_]+", "", s)
    return re.sub(r"\s+", " ", s).strip()


def main():
    out_dir, shard_dir, result = sys.argv[1:4]
    kept, dropped, per_shard = [], [], {}
    verdicts = collections.Counter()
    reviews_seen = 0
    for fn in sorted(os.listdir(out_dir)):
        if not (fn.startswith("shard-") and fn.endswith(".json")):
            continue
        shard = fn[:-5]
        try:
            data = json.load(open(os.path.join(out_dir, fn)))
        except ValueError as e:
            per_shard[shard] = f"INVALID JSON: {e}"
            continue
        n_ok = n_bad = 0
        for rv in data.get("reviews", []):
            reviews_seen += 1
            verdicts[rv.get("verdict", "?")] += 1
            path = os.path.join(shard_dir, shard, rv["agent_id"] + ".md")
            src = norm(open(path).read()) if os.path.exists(path) else None
            for f in rv.get("findings", []):
                f = dict(f, agent_id=rv["agent_id"], description=rv.get("description"),
                         shard=shard)
                ok = src is not None and norm(f.get("evidence", "")) in src
                cat_ok = f.get("category") in CATS
                f["verified"] = ok and cat_ok
                (kept if f["verified"] else dropped).append(f)
                n_ok += f["verified"]
                n_bad += not f["verified"]
        per_shard[shard] = {"verified": n_ok, "dropped": n_bad,
                            "reviews": len(data.get("reviews", []))}
    def cnt(key, rows=kept):
        return dict(collections.Counter(str(r.get(key)) for r in rows).most_common())
    summary = {
        "reviews": reviews_seen,
        "findings_verified": len(kept),
        "findings_dropped": len(dropped),
        "per_shard": per_shard,
        "verdict": dict(verdicts),
        "by_category": cnt("category"),
        "by_mechanical": cnt("mechanical"),
        "by_implementer_preventable": cnt("implementer_preventable"),
        "by_rereview": cnt("rereview"),
        "by_severity": cnt("severity"),
    }
    json.dump({"summary": summary, "findings": kept, "dropped": dropped},
              open(result, "w"), indent=1)
    print(json.dumps(summary, indent=1))


if __name__ == "__main__":
    main()
