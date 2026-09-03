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
            # `doc`, not `artifact`: the subcommand was renamed and this line was
            # the only thing still calling the old name. A renamed subcommand
            # exits 2 with "unrecognized subcommand" on STDERR and prints nothing
            # to stdout, so `proc.stdout or "{}"` below turns it into an empty
            # item list -- i.e. a clean `hits@5 0/12` that reads as a retrieval
            # verdict. Caught 2026-09-03 only by the `any_results` guard at the
            # bottom; the returncode check just below now catches it at case 1
            # with the real error instead of at case 12 with a summary.
            [args.bin, "doc", "find", "--semantic", c["query"], "--limit", "5", "--json"],
            capture_output=True, text=True)
        if proc.returncode != 0:
            raise SystemExit(
                "FATAL: `%s doc find` exited %d on case %s. This is a harness or\n"
                "binary problem, not a retrieval result -- do NOT record a score.\n"
                "stderr: %s" % (args.bin, proc.returncode, c.get("id", "?"),
                                (proc.stderr or "").strip()[:400]))
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
            #
            # Read BOTH shapes. The baseline run scored against a top-level
            # `start_line`, which is where this harness expected the field; Task 10
            # shipped it under `matched` alongside `end_line`, `entry_token` and a
            # snippet. Reading only one shape makes the scorer blind rather than
            # conservative -- measured 2026-09-02, every case returned rank None
            # while the rank-1 hit for AE-1 was the correct file with
            # `matched.start_line: 7793`. `search_live` does not cover this: the
            # search path was alive, the FIELD was missing. Accepting both means a
            # future shape change moves the number for a real reason, not because
            # the scorer was re-pointed.
            line = r.get("start_line")
            if line is None:
                line = (r.get("matched") or {}).get("start_line")
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
