#!/usr/bin/env python3
"""Corpus-wide chunk coordinate drift.

For every artifact_chunk row carrying an entry token T with a published
start_line L, locate the line H of the heading that DEFINES T in the file on
disk.  The defect under test is L < H: a chunk announcing a position ABOVE the
heading whose token it carries, which makes every consumer that re-derives the
entry from (path, line) resolve to the PREVIOUS entry.

Discriminator, stated BEFORE running:
  drift -> ~0  => the offset arithmetic is sound; the cause was STALE chunk
                  rows (a doc(update) stamps a fresh file hash without
                  rebuilding chunks, so an ordinary reindex skips the file).
  drift stays  => the arithmetic itself is wrong, independent of staleness.

Reports BOTH terms of every ratio, and the per-file delta SHAPE, because a
single constant per file is the signature of staleness while a mix of values
within one file is the signature of an arithmetic error.  A bare percentage
cannot tell those apart, and this population yields several defensible
numerators.
"""
import collections
import os
import re
import sqlite3

DB = os.path.expanduser("~/.local/share/librarian/catalog.db")
# Same grammar as the Rust entry-token module: ATX heading, PREFIX-N, dash, title.
HEAD = re.compile(r"^(#{1,6})\s+([A-Z]{1,3}-\d+)\s+[—–-]\s+(.+)$")


def defining_lines(path):
    """token -> 1-indexed line of its FIRST defining heading (fence-aware)."""
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


def main():
    con = sqlite3.connect("file:%s?mode=ro" % DB, uri=True)
    rows = con.execute(
        "select a.abs_path, c.entry_token, c.start_line "
        "from artifact_chunk c join artifact a on a.id = c.artifact_id "
        "where c.entry_token is not null"
    ).fetchall()

    total = len(rows)
    by_file = collections.defaultdict(list)
    for p, tok, line in rows:
        by_file[p].append((tok, line))

    resolvable = drift = unresolvable = 0
    deltas = collections.defaultdict(collections.Counter)
    for path, items in by_file.items():
        heads = defining_lines(path)
        if heads is None:
            unresolvable += len(items)
            continue
        for tok, line in items:
            h = heads.get(tok)
            if h is None or line is None:
                unresolvable += 1
                continue
            resolvable += 1
            if line < h:
                drift += 1
                deltas[os.path.basename(path)][line - h] += 1

    print("entry_bearing_chunks = %d" % total)
    print("  resolvable (token defined in its own file)  = %d" % resolvable)
    print("  unresolvable (token from scope, or no file) = %d" % unresolvable)
    if resolvable:
        print(
            "DRIFT (start_line < defining heading) = %d of %d resolvable (%.2f%%)"
            % (drift, resolvable, 100.0 * drift / resolvable)
        )
    else:
        print("DRIFT: no resolvable rows")

    if deltas:
        print("")
        print("per-file deltas (published_line - heading_line).")
        print("CONSTANT per file = staleness signature; MIXED = arithmetic signature.")
        ordered = sorted(deltas.items(), key=lambda kv: -sum(kv[1].values()))
        for f, c in ordered[:15]:
            shape = "CONSTANT" if len(c) == 1 else "MIXED(%d values)" % len(c)
            print("  %5d  %-56s %s %s" % (sum(c.values()), f[:56], shape, dict(c)))

    got = con.execute(
        "select count(*) from artifact_chunk where entry_part is not null"
    ).fetchone()[0]
    tot = con.execute("select count(*) from artifact_chunk").fetchone()[0]
    print("")
    print("entry_part populated = %d of %d chunks" % (got, tot))


main()
