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
    try:
        fh = open(path, encoding="utf-8")
    except OSError:
        # A returned path that does not resolve from here (a linked worktree, a
        # since-deleted file) is a diagnostic fact, not a crash. Before this the
        # scorer only opened paths it had already matched against `expect_path`;
        # recording every result means opening paths chosen by the RETRIEVER,
        # which is a wider and less trustworthy set.
        return None
    with fh:
        for i, text in enumerate(fh, 1):
            if i > line:
                break
            m = ENTRY.match(text)
            if m:
                tok = m.group(1)
    return tok

def defines_entry(path, token):
    """True iff `path` carries a heading that DEFINES `token`.

    The suite's ground truth decays silently, and its decay is shaped exactly
    like a retrieval failure: when `expect_path` no longer defines
    `expect_entry`, the scorer below reports `rank: None` -- the same value a
    genuine miss produces. A stale case is therefore not merely uncounted, it is
    counted AGAINST retrieval, and the score comes out lower for a reason that
    has nothing to do with the thing being measured.

    Measured 2026-09-04: AE-9 expected IC-16 in `docs/trackers/issue-clusters.md`,
    which had since been split into `docs/trackers/issue-clusters/IC-16-*.md`. That
    case could not have scored under any retrieval quality whatever, and had been
    contributing a guaranteed -1 to every number recorded after the split, all of
    them read as retrieval. Nothing here could have said so: `search_live` covers
    a dead search path and the returncode check covers a dead binary, and both
    were healthy. The GROUND TRUTH was the unchecked input.
    """
    try:
        with open(path, encoding="utf-8") as fh:
            for text in fh:
                m = ENTRY.match(text)
                if m and m.group(1) == token:
                    return True
    except OSError:
        return False
    return False

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--suite", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--bin", default="target/debug/codescout")
    # Default 5, NOT the product's 50. Deliberate, and the two are different
    # kinds of number: 5 makes the benchmark strict and keeps every figure in
    # `docs/trackers/retrieval-benchmark.md` comparable back to the first
    # baseline, while `find.rs`'s `default_limit() = 50` is what a caller
    # actually gets. Until this flag existed the harness hardcoded 5 and the
    # SHIPPED configuration had never been measured at all -- BL-72 was even
    # written as though the 5 were a product constant to be raised (F-115).
    # Pass `--limit 50` to measure the deployment; keep 5 to compare history.
    ap.add_argument("--limit", type=int, default=5,
                    help="page size passed to `doc find --limit` (default 5: the "
                         "strict measurement setting, not the product's 50)")
    args = ap.parse_args()

    suite = json.load(open(args.suite))
    cases, hits, file_hits, rr = [], 0, 0, 0.0
    any_results = False
    unscorable = []
    stale = []
    for c in suite["cases"]:
        # Check the GROUND TRUTH before the retriever is charged for it. A case
        # whose expected entry is no longer defined at its expected path cannot
        # score however good retrieval is, and its `rank: None` is byte-identical
        # to a real miss. See `defines_entry`.
        scorable = defines_entry(c["expect_path"], c["expect_entry"])
        if not scorable:
            unscorable.append(c["id"])
        proc = subprocess.run(
            # `doc`, not `artifact`: the subcommand was renamed and this line was
            # the only thing still calling the old name. A renamed subcommand
            # exits 2 with "unrecognized subcommand" on STDERR and prints nothing
            # to stdout, so `proc.stdout or "{}"` below turns it into an empty
            # item list -- i.e. a clean `hits@5 0/12` that reads as a retrieval
            # verdict. Caught 2026-09-03 only by the `any_results` guard at the
            # bottom; the returncode check just below now catches it at case 1
            # with the real error instead of at case 12 with a summary.
            [args.bin, "doc", "find", "--semantic", c["query"],
             "--limit", str(args.limit), "--json"],
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
        file_rank = None
        file_entry = None
        top = []
        for i, r in enumerate(results, 1):
            path = r.get("rel_path") or r.get("abs_path", "")
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
            ent = entry_at(path, line) if line is not None else None
            # THE INDEX'S OWN ANSWER, which this harness spent two sessions not
            # reading. `matched.entry_token` is computed when the chunk is BUILT,
            # in body coordinates; `entry_at` above re-derives it from the file on
            # disk using the PUBLISHED line. They can only disagree if the two
            # coordinate systems have drifted apart -- which makes their
            # disagreement a free detector, and it was sitting unused in the
            # response the whole time.
            #
            # Measured 2026-09-04, and the cause is NOT staleness (a forced
            # re-walk of all 1,471 artifacts left it in place, and got worse):
            # 378 of 3,729 entry-bearing chunks publish a `start_line` that
            # precedes the heading whose token they correctly carry, by a
            # PER-FILE constant -- -2 on all 218 chunks of bug-fix-session-log.md,
            # -1 on all 71 of open-issue-work-queue.md, with zero chunks landing
            # on a heading in either. That is the open bug
            # `docs/issues/2026-09-02-chunk-line-ranges-are-body-relative-but-published-as-file-lines.md`,
            # only PARTIALLY fixed: short by 1-2 rather than by the whole
            # frontmatter, which is why it stopped being visible.
            #
            # Why the harness cannot be trusted without this: AE-1 read as `hit`
            # in one run and `wrong_entry` in the next with the retrieval result
            # BYTE-IDENTICAL -- same rank 1, same line 7996, resolving to `W-80`
            # instead of `W-81`. Nothing errored and the corpus counts were
            # identical, so the natural reading was a retrieval regression.
            indexed_ent = (r.get("matched") or {}).get("entry_token")
            if indexed_ent is not None and ent is not None and indexed_ent != ent:
                stale.append(f"{c['id']}:{path.rsplit('/', 1)[-1]}@{line} "
                             f"indexed={indexed_ent} on-disk={ent}")
            # Record EVERY result, not only a matching one. Before this the loop
            # kept nothing but `rank`, and `None` was the same value for "the
            # target file never came back" and "the target file ranked 1 and the
            # chunk that matched belongs to no entry at all" -- findings whose
            # fixes point in opposite directions (retrieval vs page policy).
            # Separating them cost a session of hand-run queries on 2026-09-03.
            top.append({"rank": i, "path": path, "line": line,
                        "entry": ent, "indexed_entry": indexed_ent})
            if not path.endswith(c["expect_path"]):
                continue
            if file_rank is None:
                file_rank, file_entry = i, ent
            # Pre-change the tool returns artifacts, not chunks: a path match with
            # no line range cannot prove the ENTRY was found, so it is not a hit.
            if rank is None and line is not None and ent == c["expect_entry"]:
                rank = i
        if rank:
            hits += 1
            rr += 1.0 / rank
        if file_rank:
            file_hits += 1
        # Five outcomes, not two. `unscorable` outranks the rest: when the entry
        # is not defined at the path, a file-level hit says nothing about whether
        # retrieval could have found the entry.
        #
        # `preamble` vs `wrong_entry` is the split that a single "the right file,
        # the wrong place in it" bucket hides, and the first run of this harness
        # mislabelled all three of its own cases before the `entry` field made it
        # visible. A chunk whose line maps to NO entry token landed in the file's
        # index/preamble -- a section that summarises every entry and therefore
        # out-scores each specific one on any query about that file, which with
        # `max_per_artifact=1` evicts the real answer (BL-72). A chunk that maps
        # to a DIFFERENT entry is ordinary intra-file ranking loss. The first is
        # a page-policy defect, the second a retrieval one; one number cannot ask
        # for both fixes.
        klass = ("unscorable" if not scorable
                 else "hit" if rank
                 else ("preamble" if file_entry is None else "wrong_entry")
                 if file_rank
                 else "wrong_file")
        cases.append({**c, "rank": rank, "file_rank": file_rank,
                      "file_entry": file_entry,
                      "miss_class": klass, "top": top})

    out = {"suite": suite["suite"], "n": len(cases),
           # `limit` is recorded because the two figures below are meaningless
           # without it, and RENAMED off `hits_at_5` for the same reason: that
           # key baked a constant into a name while the value stopped being
           # computed at 5 the moment `--limit` existed. A number labelled with
           # a parameter it no longer uses is the defect this suite keeps
           # finding in other people's code (F-115).
           "limit": args.limit,
           "hits_at_limit": hits, "file_hits_at_limit": file_hits,
           "unscorable": unscorable,
           "stale_index": stale,
           "mrr": round(rr / max(len(cases), 1), 4),
           "search_live": any_results, "cases": cases}
    json.dump(out, open(args.out, "w"), indent=2)
    by = {}
    for c in cases:
        by[c["miss_class"]] = by.get(c["miss_class"], 0) + 1
    print(f"{out['suite']}: hits@{args.limit} {hits}/{len(cases)}  "
          f"file-hits@{args.limit} {file_hits}/{len(cases)}  MRR {out['mrr']}  "
          f"search_live={any_results}")
    # The class breakdown goes to STDOUT beside the score, not into the warning
    # below, because the number is what gets copied into a tracker and a warning
    # on stderr is not part of what gets copied.
    print("  " + "  ".join(f"{k}={v}" for k, v in sorted(by.items())))

    if unscorable:
        print(
            "WARNING: %d of %d cases are UNSCORABLE -- their expected entry is no\n"
            "         longer defined at their expected path, so they cannot score\n"
            "         under any retrieval quality: %s\n"
            "         hits@N is therefore capped at %d/%d. Repoint or retire them\n"
            "         before quoting the score." % (
                len(unscorable), len(cases), ", ".join(unscorable),
                len(cases) - len(unscorable), len(cases)),
            file=sys.stderr)

    if stale:
        # Louder than `unscorable`, because this one moves a number that already
        # looks fine. An unscorable case is at least visible as a class in the
        # stdout breakdown; a mis-anchored one is scored normally, in whichever
        # direction the drift happens to fall, and reads as retrieval.
        print(
            "WARNING: %d result(s) carry an entry token the PUBLISHED LINE does not\n"
            "         resolve to. The token is right and the line is short -- the\n"
            "         two are computed in different coordinate systems and have\n"
            "         drifted apart:\n"
            "           %s\n"
            "         This is the open bug docs/issues/2026-09-02-chunk-line-ranges-\n"
            "         are-body-relative-but-published-as-file-lines.md, only PARTIALLY\n"
            "         fixed -- short by a per-file 1-5 rather than by the whole\n"
            "         frontmatter. Reindexing does NOT clear it (a forced re-walk of\n"
            "         1,471 artifacts left it in place); the arithmetic is wrong, not\n"
            "         stale. Any case touching those files is scored against a target\n"
            "         that moved for a reason unrelated to retrieval."
            % (len(stale), "\n           ".join(stale[:8])),
            file=sys.stderr)

    if not any_results:
        print(
            "FATAL: every query returned zero items — this is a dead search path "
            "(bad --bin, crashing subprocess, unreachable embedder), not a "
            "retrieval finding. hits 0/N here is NOT a baseline. Check "
            f"`{args.bin} doc find --semantic '<query>' --limit {args.limit} --json` by hand.",
            file=sys.stderr)
        return 1
    return 0

if __name__ == "__main__":
    sys.exit(main())
