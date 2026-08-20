#!/usr/bin/env python3
"""Measure entry-grain citation attribution precision over tracker ledgers.

WHAT THIS ANSWERS
    When a `PREFIX-N` citation appears at line L of a ledger, which ENTRY does it
    belong to? Two candidate rules are compared:

      naive     the definition with the greatest line <= L
      bounded   that definition, but only if L is still inside its section, where a
                section ends at the next heading of the SAME OR HIGHER level

    The headline number is the fraction of attributed citations on which the two
    rules AGREE. It is consumed by the entry-graph backfill, by buffer-slice read
    attribution, and by grep-hit read attribution, so its error rate is load-bearing
    in three places.

WHAT EACH PREDICATE LITERALLY COUNTS
    citations       occurrences of \\b[A-Z]{1,3}-\\d+\\b on a non-fenced, non-frontmatter
                    line. NOT distinct tokens, NOT resolvable citations, NOT edges. A
                    token repeated three times on one line counts three times, which is
                    what a per-occurrence attribution rule needs.
    definitions     lines matching `#{1,6} <TOKEN> <dash> <text>` — link_scan's def_re
                    shape. A table row defines nothing, by design.
    orphan          a citation with no preceding definition in its file. These are
                    index-table rows and preambles; BOTH rules attribute them to
                    nothing, and that is correct.
    naive_wrong     a citation the naive rule attributes to an entry whose section has
                    already ended. This is the error population.
    precision       agree / (agree + naive_wrong). NOT ground truth — see BLIND SPOTS.

BLIND SPOTS — read these before quoting a number
    * `precision` is AGREEMENT BETWEEN TWO ALGORITHMS with `bounded` assumed correct.
      It is not a labelled set. Use --ground-truth to print disagreement rows with
      their context and check them by hand; as of 2026-08-20 only n=1 had been
      hand-verified (bounded was right).
    * Fence detection is a naive ``` / ~~~ toggle. An UNBALANCED fence anywhere in a
      file silently inverts the in/out state for every line after it — the exact
      "heuristic whose assumption the data violates" failure CLAUDE.md's Measurement
      rule describes. --fence-audit reports files with an odd fence count.
    * Scope is ledgers under docs/trackers/ that DECLARE `entry_prefix:` in
      frontmatter, excluding archive/. Specs, plans, bug files and ADRs also carry
      PREFIX-N tokens and are NOT measured here. A number from this probe says
      nothing about them.
    * It reads files directly from disk, bypassing the librarian read guard. That is
      deliberate for a probe, and it is itself evidence that run_command is an
      unguarded read path for ledgers.
    * The calibration set is frozen from one `link_scan` run. If extract.rs's grammar
      changes, re-derive it before trusting --calibrate.

USAGE
    python3 scripts/probe_entry_attribution.py [--calibrate] [--ground-truth N]
                                               [--fence-audit] [--json] [--root PATH]
"""
from __future__ import annotations

import argparse
import collections
import json
import os
import re
import sqlite3
import subprocess
import sys

DEFAULT_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CATALOG = os.path.expanduser("~/.local/share/librarian/catalog.db")

# Mirrors src/librarian/tools/link_scan/extract.rs. Keep in sync, and re-run
# --calibrate after any change there.
TOK = re.compile(r"\b([A-Z]{1,3}-\d+)\b")
DEF = re.compile(r"^(#{1,6})\s+([A-Z]{1,3}-\d+)\s+[—–-]\s+\S")
HEAD = re.compile(r"^(#{1,6})\s+(.*)$")

# Frozen from `librarian(action="link_scan", write=false)` on 2026-08-20: the dangling
# EntryToken citations whose src_id resolves to a readable file. Calibration target.
CALIBRATION = [
    ("01291679a5ee4707", "CAP-4", 41),
    ("f2ecdd76a6189efb", "TU-2", 811),
    ("5696563f06b2c222", "L-08", 326),
    ("5696563f06b2c222", "TU-11", 332),
    ("03464a8808345846", "PV-12", 1152),
    ("03464a8808345846", "PV-3", 1153),
    ("03464a8808345846", "PV-20", 1154),
    ("03464a8808345846", "PV-1", 1155),
    ("03464a8808345846", "PV-9", 1155),
    ("2dd9d90bc83f9f49", "T-13", 489),
    ("e6697aea0ee3ea37", "L-6", 388),
]


def parse(path):
    """-> (headings, definitions, citations, n_lines, fence_toggles).

    `fence_toggles` is returned so the caller can detect an unbalanced fence, which
    would corrupt every classification after it rather than raising.
    """
    with open(path, encoding="utf-8") as fh:
        lines = fh.read().split("\n")
    heads, defs, cites = [], [], []
    fenced = in_fm = False
    toggles = 0
    for i, line in enumerate(lines, 1):
        s = line.strip()
        if i == 1 and s == "---":
            in_fm = True
            continue
        if in_fm:
            if s == "---":
                in_fm = False
            continue
        if s.startswith("```") or s.startswith("~~~"):
            fenced = not fenced
            toggles += 1
            continue
        if fenced:
            continue
        h = HEAD.match(line)
        if h:
            heads.append((len(h.group(1)), i, h.group(2)))
        m = DEF.match(line)
        own = None
        if m:
            own = m.group(2)
            defs.append((own, i, len(m.group(1))))
        for tok in TOK.findall(line):
            # A defining heading defines its own token and does not self-cite it;
            # any OTHER token on that line is a citation (extract.rs pins both).
            if own is not None and tok == own:
                own = None
                continue
            cites.append((tok, i))
    return heads, defs, cites, len(lines), toggles


def section_bounds(heads, defs, n_lines):
    """entry -> (start_line, end_line); end is the line before the next same-or-higher heading."""
    out = {}
    for tok, dline, lvl in defs:
        end = n_lines
        for hlvl, hline, _txt in heads:
            if hline > dline and hlvl <= lvl:
                end = hline - 1
                break
        out[(tok, dline)] = (dline, end)
    return out


def naive_owner(defs, line):
    owner = None
    for tok, dline, _lvl in defs:
        if dline <= line:
            owner = (tok, dline)
        else:
            break
    return owner


def find_ledgers(root):
    """Ledgers = files under docs/trackers/ declaring entry_prefix, excluding archive/."""
    res = subprocess.run(
        ["grep", "-rl", "--include=*.md", "-E", "^entry_prefix:",
         os.path.join(root, "docs", "trackers")],
        capture_output=True, text=True,
    )
    return sorted(p for p in res.stdout.split() if "/archive/" not in p)


def run_calibration(root):
    """Compare this extractor against link_scan's own output on the overlap.

    CLAUDE.md Measurement clause 4: a ratio near 1 licenses extending a hand-built
    instrument to the part the known-good one does not cover.
    """
    try:
        con = sqlite3.connect(f"file:{CATALOG}?mode=ro", uri=True)
        id2path = dict(con.execute(
            "SELECT id, abs_path FROM artifact WHERE abs_path LIKE ?", (root + "/%",)))
    except sqlite3.Error as exc:
        return {"error": f"catalog unreadable: {exc}"}
    hit = miss = skipped = 0
    misses = []
    for aid, tok, line in CALIBRATION:
        path = id2path.get(aid)
        if not path or not os.path.exists(path):
            skipped += 1
            continue
        _h, _d, cites, _n, _t = parse(path)
        if (tok, line) in cites:
            hit += 1
        else:
            miss += 1
            misses.append({"file": os.path.basename(path), "token": tok, "line": line})
    return {"hit": hit, "miss": miss, "skipped": skipped, "misses": misses}


def main(argv=None):
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--root", default=DEFAULT_ROOT, help="repo root (default: this repo)")
    ap.add_argument("--calibrate", action="store_true",
                    help="check this extractor against link_scan's output first")
    ap.add_argument("--ground-truth", type=int, metavar="N", default=0,
                    help="print N disagreement rows for hand-checking (precision is "
                         "algorithm agreement, not a labelled set)")
    ap.add_argument("--fence-audit", action="store_true",
                    help="report files with an odd ``` count — their classification is "
                         "corrupt from the unbalanced fence onward")
    ap.add_argument("--json", action="store_true", help="machine-readable output")
    args = ap.parse_args(argv)

    out = {}
    if args.calibrate:
        out["calibration"] = run_calibration(args.root)

    ledgers = find_ledgers(args.root)
    counts = collections.Counter()
    per_file, disagreements, odd_fences = [], [], []

    for path in ledgers:
        heads, defs, cites, n_lines, toggles = parse(path)
        if toggles % 2:
            odd_fences.append(os.path.basename(path))
        bounds = section_bounds(heads, defs, n_lines)
        f = collections.Counter()
        for tok, line in cites:
            counts["citations"] += 1
            f["citations"] += 1
            owner = naive_owner(defs, line)
            if owner is None:
                counts["orphan"] += 1
                f["orphan"] += 1
                continue
            lo, hi = bounds[owner]
            if lo <= line <= hi:
                counts["agree"] += 1
                f["agree"] += 1
            else:
                counts["naive_wrong"] += 1
                f["naive_wrong"] += 1
                disagreements.append({
                    "file": os.path.basename(path), "line": line, "token": tok,
                    "naive_owner": owner[0], "owner_def_line": owner[1],
                    "owner_section_ends": hi,
                })
        per_file.append({"file": os.path.basename(path), "definitions": len(defs),
                         **dict(f)})

    attributed = counts["agree"] + counts["naive_wrong"]
    out["counts"] = dict(counts)
    out["counts"]["attributed"] = attributed
    out["precision_naive"] = (counts["agree"] / attributed) if attributed else None
    out["ledgers"] = len(ledgers)
    out["per_file"] = per_file
    out["odd_fence_files"] = odd_fences

    if args.json:
        if args.ground_truth:
            out["disagreements"] = disagreements[: args.ground_truth]
        print(json.dumps(out, indent=2))
        return 0

    if args.calibrate:
        c = out["calibration"]
        if "error" in c:
            print(f"CALIBRATION UNAVAILABLE: {c['error']}\n")
        else:
            print(f"=== CALIBRATION vs link_scan (n={len(CALIBRATION)}) ===")
            print(f"  hit={c['hit']} miss={c['miss']} skipped={c['skipped']}")
            for m in c["misses"]:
                print(f"  MISS {m['file']} {m['token']} L{m['line']}")
            if c["miss"]:
                print("  ratio is NOT 1 — do not extend this extractor until it is\n")
            else:
                print("  ratio 1.0 — extension licensed\n")

    print(f"=== ATTRIBUTION over {len(ledgers)} ledgers ===")
    print(f"  {'ledger':<50} {'defs':>5} {'cites':>7} {'orphan':>7} {'wrong':>6}")
    for r in per_file:
        print(f"  {r['file'][:50]:<50} {r['definitions']:>5} {r.get('citations', 0):>7} "
              f"{r.get('orphan', 0):>7} {r.get('naive_wrong', 0):>6}")
    print(f"\n  citations ............... {counts['citations']}")
    print(f"  above first definition .. {counts['orphan']}  (both rules: unattributed)")
    print(f"  agree ................... {counts['agree']}")
    print(f"  naive attributes outside  {counts['naive_wrong']}")
    if attributed:
        print(f"\n  naive precision ......... {counts['agree']}/{attributed} = "
              f"{100.0 * counts['agree'] / attributed:.1f}%")
        print("  (agreement between two rules, NOT ground truth — see --ground-truth)")

    if args.fence_audit or odd_fences:
        print(f"\n=== FENCE AUDIT ===")
        if odd_fences:
            print("  ODD fence count — classification is corrupt after the unbalanced fence:")
            for f in odd_fences:
                print(f"    {f}")
        else:
            print("  all files balanced")

    if args.ground_truth:
        print(f"\n=== DISAGREEMENTS for hand-checking (first {args.ground_truth}) ===")
        for d in disagreements[: args.ground_truth]:
            print(f"  {d['file'][:44]:<44} L{d['line']:<6} cites {d['token']:<9} "
                  f"naive->{d['naive_owner']} (def L{d['owner_def_line']}, "
                  f"section ends L{d['owner_section_ends']})")
        by_file = collections.Counter(d["file"] for d in disagreements)
        print("\n  concentration:")
        for f, n in by_file.most_common():
            print(f"    {f[:50]:<50} {n:>4}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
