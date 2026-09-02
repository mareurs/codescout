#!/usr/bin/env python3
"""Score artifact-path retrieval against known entry tokens.

Distinct from run-tc-benchmark.py, which scores bench_<model>_code_chunks.
This one exercises artifact(find, semantic=) — the path with no instrument.

Positive control: a scorer that reports 0/N cannot be trusted on its own — a dead
embedder, a wrong --bin path, or a crashing subprocess all produce exactly the same
0/N as a real "the tool doesn't return line ranges yet" finding. So every run also
records `search_live`: true iff at least one query returned a non-empty `items`
array. If search_live is false, the 0/N is not a baseline — it's a broken harness,
and this script says so loudly and exits non-zero.
"""
import argparse, json, re, subprocess, sys

ENTRY = re.compile(r'^#{2,4}\s+([A-Z]{1,3}-\d+)\s+[—–-]\s')

def entry_at(path, line):
    """The PREFIX-N entry enclosing 1-indexed `line`, or None."""
    tok = None
    with open(path, encoding="utf-8") as fh:
        for i, text in enumerate(fh, 1):
            if i > line:
                break
            m = ENTRY.match(text)
            if m:
                tok = m.group(1)
    return tok

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--suite", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--bin", default="target/debug/codescout")
    args = ap.parse_args()

    suite = json.load(open(args.suite))
    cases, hits, rr = [], 0, 0.0
    any_results = False
    for c in suite["cases"]:
        proc = subprocess.run(
            [args.bin, "artifact", "find", "--semantic", c["query"], "--limit", "5", "--json"],
            capture_output=True, text=True)
        results = json.loads(proc.stdout or "{}").get("items", [])
        if results:
            any_results = True
        rank = None
        for i, r in enumerate(results, 1):
            path = r.get("rel_path") or r.get("abs_path", "")
            if not path.endswith(c["expect_path"]):
                continue
            # Pre-change the tool returns artifacts, not chunks: a path match with
            # no line range cannot prove the ENTRY was found, so it is not a hit.
            line = r.get("start_line")
            if line is not None and entry_at(path, line) == c["expect_entry"]:
                rank = i
                break
        if rank:
            hits += 1
            rr += 1.0 / rank
        cases.append({**c, "rank": rank})

    out = {"suite": suite["suite"], "n": len(cases),
           "hits_at_5": hits, "mrr": round(rr / max(len(cases), 1), 4),
           "search_live": any_results, "cases": cases}
    json.dump(out, open(args.out, "w"), indent=2)
    print(f"{out['suite']}: hits@5 {hits}/{len(cases)}  MRR {out['mrr']}  search_live={any_results}")

    if not any_results:
        print(
            "FATAL: every query returned zero items — this is a dead search path "
            "(bad --bin, crashing subprocess, unreachable embedder), not a "
            "retrieval finding. hits@5 0/N here is NOT a baseline. Check "
            f"`{args.bin} artifact find --semantic '<query>' --limit 5 --json` by hand.",
            file=sys.stderr)
        return 1
    return 0

if __name__ == "__main__":
    sys.exit(main())
