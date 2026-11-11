#!/usr/bin/env python3
"""Split reviews.jsonl into shards of readable text files for miner subagents.

Usage:
  python3 -I make_shards.py CORPUS_DIR SHARD_DIR --shards K --per-shard N [--seed S] [--exclude FILE]

Sampling is stratified: reviews are grouped by calendar month of ts_first and
drawn round-robin, so one busy week does not fill every shard. Each review is
written as SHARD_DIR/shard-<k>/<agent_id>.md with a short brief excerpt and the
full report. SHARD_DIR/manifest.json lists which agent ids went to which shard.
--exclude takes a manifest.json of an earlier run; those ids are skipped.
"""
import collections
import json
import os
import random
import sys

BRIEF_CHARS = 2500


def main():
    a = sys.argv[1:]
    corpus, shard_dir = a[0], a[1]

    def opt(name, default):
        return a[a.index(name) + 1] if name in a else default

    k = int(opt("--shards", 8))
    n = int(opt("--per-shard", 10))
    seed = int(opt("--seed", 1))
    skip = set()
    if "--exclude" in a:
        old = json.load(open(opt("--exclude", "")))
        for ids in old.values():
            skip.update(ids)
    recs = [json.loads(l) for l in open(os.path.join(corpus, "reviews.jsonl"))]
    recs = [r for r in recs if r["report"] and len(r["report"]) > 500 and r["agent_id"] not in skip]
    by_month = collections.defaultdict(list)
    for r in recs:
        by_month[(r["ts_first"] or "")[:7]].append(r)
    rnd = random.Random(seed)
    for v in by_month.values():
        rnd.shuffle(v)
    order = []
    months = sorted(by_month)
    while any(by_month[m] for m in months) and len(order) < k * n:
        for m in months:
            if by_month[m]:
                order.append(by_month[m].pop())
    manifest = {}
    for i in range(k):
        chunk = order[i::k][:n]
        d = os.path.join(shard_dir, f"shard-{i + 1}")
        os.makedirs(d, exist_ok=True)
        manifest[f"shard-{i + 1}"] = [r["agent_id"] for r in chunk]
        for r in chunk:
            with open(os.path.join(d, r["agent_id"] + ".md"), "w") as fh:
                fh.write(
                    f"# review {r['agent_id']}\n"
                    f"- description: {r['description']}\n"
                    f"- when: {r['ts_first']}\n"
                    f"- session: {r['session']}\n"
                    f"- reviewer tool calls: {r['tool_calls']}\n\n"
                    f"## Brief the reviewer received (first {BRIEF_CHARS} chars)\n\n"
                    f"{r['brief'][:BRIEF_CHARS]}\n\n"
                    f"## Report the reviewer handed back\n\n{r['report']}\n"
                )
    json.dump(manifest, open(os.path.join(shard_dir, "manifest.json"), "w"), indent=1)
    print({s: len(v) for s, v in manifest.items()}, "months:", {m: len(by_month[m]) for m in months})


if __name__ == "__main__":
    main()
