#!/usr/bin/env python3
"""Chunk coordinate drift, split by repo root.

Companion to chunk-coord-drift.py.  The reindex that motivated this ran with
scope="project" (codescout only), so the catalog now holds a natural control:
codescout artifacts were rebuilt, every other repo's were not.  If the defect
is stale chunk rows, drift should be ~0 inside the reindexed root and unchanged
outside it.  If the defect is offset arithmetic, both should drift alike.

Also prints the two files the bug file named, by full path, whether or not they
drift -- an absent name in a top-N list is not evidence, since the list is
truncated.
"""
import collections
import os
import re
import sqlite3

DB = os.path.expanduser("~/.local/share/librarian/catalog.db")
CODESCOUT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
NAMED = ("bug-fix-session-log.md", "open-issue-work-queue.md")
HEAD = re.compile(r"^(#{1,6})\s+([A-Z]{1,3}-\d+)\s+[—–-]\s+(.+)$")


def defining_lines(path):
    out, fence = {}, False
    try:
        with open(path, encoding="utf-8") as fh:
            for i, text in enumerate(fh, 1):
                s = text.lstrip()
                if s.startswith("```") or s.startswith("~~~"):
                    fence = not fence
                    continue
                if fence:
                    continue
                m = HEAD.match(text.rstrip("\n"))
                if m and m.group(2) not in out:
                    out[m.group(2)] = i
    except OSError:
        return None
    return out


def root_of(path):
    return "codescout" if path.startswith(CODESCOUT + os.sep) else "OTHER-REPOS"


def main():
    con = sqlite3.connect("file:%s?mode=ro" % DB, uri=True)
    rows = con.execute(
        "select a.abs_path, c.entry_token, c.start_line "
        "from artifact_chunk c join artifact a on a.id = c.artifact_id "
        "where c.entry_token is not null"
    ).fetchall()

    by_file = collections.defaultdict(list)
    for p, tok, line in rows:
        by_file[p].append((tok, line))

    stats = collections.defaultdict(lambda: {"resolvable": 0, "drift": 0, "files": set()})
    named_report = []
    for path, items in by_file.items():
        heads = defining_lines(path)
        if heads is None:
            continue
        r = root_of(path)
        d_here = res_here = 0
        for tok, line in items:
            h = heads.get(tok)
            if h is None or line is None:
                continue
            res_here += 1
            if line < h:
                d_here += 1
        stats[r]["resolvable"] += res_here
        stats[r]["drift"] += d_here
        if d_here:
            stats[r]["files"].add(os.path.basename(path))
        if os.path.basename(path) in NAMED:
            named_report.append((path, res_here, d_here))

    print("DRIFT BY ROOT  (codescout was reindexed with reembed=true; others were not)")
    for r in ("codescout", "OTHER-REPOS"):
        s = stats[r]
        if not s["resolvable"]:
            print("  %-12s no resolvable rows" % r)
            continue
        print(
            "  %-12s drift %4d of %5d resolvable (%.2f%%)   across %d file(s)"
            % (r, s["drift"], s["resolvable"], 100.0 * s["drift"] / s["resolvable"], len(s["files"]))
        )

    print("")
    print("THE TWO FILES THE BUG FILE NAMED (reported whether or not they drift):")
    if not named_report:
        print("  neither file has any entry-bearing chunk row -- that is itself a finding")
    for path, res, d in sorted(named_report):
        print("  %-70s resolvable=%4d drift=%d" % (path.replace(CODESCOUT + "/", ""), res, d))


main()
