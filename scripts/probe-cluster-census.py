#!/usr/bin/env python3
"""Per-class membership counts for `docs/trackers/issue-clusters.md`, derived rather than stored.

WHY THIS EXISTS. The ledger used to store each class's count in three places -- the Index row's
`n` cell, `**Members:**`, and `**Promotes to:**` -- and filing any bug file forced an edit to all
three. One file is git's staging unit; 22 class records are the edit unit, so every filer held a
lock on all 22 classes and textually disjoint edits blocked each other
(`docs/issues/2026-09-02-one-ledger-file-serializes-every-class-edit.md`). The counts are exactly
derivable, so they are no longer stored. This is where you read them.

WHAT IT DOES NOT DO, and this is the part to read before quoting it.
The promotion threshold is **three or more instances spanning two or more subsystems**. This
probe derives the first half exactly and CANNOT derive the second at all. Verified 2026-09-02:
the only tag families on bug files are `cluster/` and `docs/`, and subsystem spread is
adjudicated by hand into prose (*"spread adjudicated 2026-09-01 -- 4 doc surfaces / 4
subsystems"*). So `n` is a measurement and `promotable` is NOT computed -- the `verdict` column
reports what a human already wrote in the ledger, and a class showing `n>=3` with no verdict is
a prompt to adjudicate, never a finding that it qualifies.

THE COUNT IS A FLOOR. Population is `git ls-files docs/issues` -- the INDEX -- so a peer's
untracked in-flight bug file is invisible by construction. That is deliberate and matches the
gate: gating on untracked files lets one session red another's build.

AND ON A SHARED CHECKOUT THE DEFAULT SOURCE IS NOT A COMMIT. `--source=worktree` reads files as
they sit on disk, which on this repo means WITH every other session's uncommitted edits folded
in -- a count over a tree that never existed at any commit and never will. Each session's own
tree is self-consistent, so nobody can see this from inside their own; it returns a plausible
number rather than an error. The probe therefore derives all three sources and prints a warning
naming any class where they disagree, rather than leaving the reader to know to ask. Raised by
codescout-00 (sessionId 953b5e77) from a live instance: a constant's slack was measured against a
tree containing another session's uncommitted 31 bytes, and the reading was correct for the tree
in front of the measurer and wrong about HEAD.

IT RE-DERIVES NOTHING. `valid_slugs`, `bug_files`, `cluster_tags` and `actual_counts` are
imported from `pre-commit-ledger-counts.py`, which is itself pinned against the Rust in
`tests/issue_clusters.rs` by `the_ledger_parsers_agree_on_a_fixture`. A second implementation
would be a second thing to hold in agreement; importing means this probe inherits that pinning
instead of needing its own. If the numbers here are wrong, the gate is wrong too -- they cannot
drift apart silently, which is the only property that makes an unpinned probe worth trusting.
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import pathlib
import re
import sys

_HERE = pathlib.Path(__file__).resolve().parent


def _load_gate():
    """Import the hook script as a module.

    It is hyphenated, so it is not importable by name; and it guards `main()` behind
    `if __name__ == "__main__"`, so importing it runs nothing.
    """
    path = _HERE / "pre-commit-ledger-counts.py"
    spec = importlib.util.spec_from_file_location("_ledger_counts", path)
    if spec is None or spec.loader is None:  # pragma: no cover - unreachable in-tree
        raise SystemExit(f"cannot load {path}")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


G = _load_gate()

# `**Promotes to:** ` and `**Mechanism status:** ` open their fields; the class's own heading
# opens the record. Nothing else is parsed -- this probe reports prose verbatim rather than
# interpreting it, because the fields it reads are human adjudications and summarising a
# judgement is how a probe starts inventing one.
_HEADING = re.compile(r"^## (IC-\d+) — (.*)$")


def records(ledger: str) -> dict[str, dict[str, str]]:
    """slug -> {id, title, promotes, mechanism}, for every class that declares a `**Slug:**`."""
    out: dict[str, dict[str, str]] = {}
    cur: dict[str, str] | None = None
    for line in ledger.splitlines():
        m = _HEADING.match(line)
        if m:
            cur = {"id": m.group(1), "title": m.group(2), "promotes": "", "mechanism": ""}
            continue
        if cur is None:
            continue
        if line.startswith("**Slug:**"):
            slug = G.backticked_cluster_slug(line[len("**Slug:**") :])
            # The template's placeholder declares a `cluster/<slug>` that is not a class; the
            # same lowercase-ASCII rule `valid_slugs` uses excludes it here.
            if slug and all(c.islower() and c.isascii() or c == "-" for c in slug):
                out[slug] = cur
        elif line.startswith("**Promotes to:**"):
            cur["promotes"] = line[len("**Promotes to:**") :].strip()
        elif line.startswith("**Mechanism status:**"):
            cur["mechanism"] = line[len("**Mechanism status:**") :].strip()
    return out


def _clip(s: str, width: int) -> str:
    s = " ".join(s.split())
    return s if len(s) <= width else s[: width - 1] + "…"


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument(
        "--source",
        choices=("index", "worktree", "head"),
        default="worktree",
        help="worktree mirrors the cargo gate; index mirrors the commit hook; "
        "head is the only one that names a tree that actually exists at a commit",
    )
    ap.add_argument("--json", action="store_true", help="machine-readable output")
    ap.add_argument(
        "--threshold", type=int, default=3, help="count bar for the prompt column (default 3)"
    )
    args = ap.parse_args()

    root = pathlib.Path(G._git("rev-parse", "--show-toplevel").strip())
    ledger = (root / G.LEDGER).read_text(encoding="utf-8")

    valid = G.valid_slugs(ledger)
    counts = G.actual_counts(valid, args.source)
    recs = records(ledger)

    rows = []
    for slug in sorted(valid, key=lambda s: (-counts.get(s, 0), s)):
        r = recs.get(slug, {})
        rows.append(
            {
                "id": r.get("id", "?"),
                "slug": slug,
                "n": counts.get(slug, 0),
                "verdict": r.get("promotes", ""),
                "mechanism": r.get("mechanism", ""),
            }
        )

    # Three sources, because on a shared checkout they are three different questions and the
    # difference is invisible from inside any one session's tree.
    divergence = {}
    others = {
        src: G.actual_counts(valid, src)
        for src in ("worktree", "index", "head")
        if src != args.source
    }
    for src, other in others.items():
        for slug in sorted(valid):
            a, b = counts.get(slug, 0), other.get(slug, 0)
            if a != b:
                divergence.setdefault(slug, {})[src] = b

    if args.json:
        print(
            json.dumps(
                {
                    "source": args.source,
                    "divergence": divergence,
                    "population": len(G.bug_files()),
                    "classes": len(rows),
                    "spread_derivable": False,
                    "rows": rows,
                },
                indent=2,
            )
        )
        return 0

    print(f"cluster census — {len(rows)} classes over {len(G.bug_files())} tracked bug files")
    print(f"source: {args.source} (a floor — untracked peer work is invisible)\n")
    print(f"{'ID':<7} {'n':>3}  {'slug':<44} verdict")
    print(f"{'-' * 7} {'-' * 3}  {'-' * 44} {'-' * 40}")
    for r in rows:
        print(f"{r['id']:<7} {r['n']:>3}  {r['slug']:<44} {_clip(r['verdict'], 40)}")

    unadjudicated = [
        r
        for r in rows
        if r["n"] >= args.threshold and r["verdict"].startswith("`not yet`")
    ]
    print(
        f"\n{len(unadjudicated)} class(es) at n>={args.threshold} whose recorded verdict is "
        f"`not yet`:"
    )
    for r in unadjudicated:
        print(f"  {r['id']:<7} n={r['n']:<3} {r['slug']}")
    if not divergence:
        print(
            "\nAll three sources agree on every count — which establishes LESS than it reads.\n"
            "  It rules out one failure (a count measured over a tree that exists at no commit)\n"
            "  and not the other: a COUPLED CHANGE SPLIT ACROSS THE COMMIT BOUNDARY leaves HEAD\n"
            "  internally inconsistent, and all three sources then agree on a number that is\n"
            "  wrong. Reported by codescout-00/cc (sessionId 953b5e77) from a live 90-minute\n"
            "  window: a budget raise landed while the bytes justifying it stayed uncommitted.\n"
            "  Agreement is evidence about the TREES, never about the change."
        )

    if divergence:
        print(
            f"\n{len(divergence)} class(es) count DIFFERENTLY under another source — this tree "
            f"is not a commit:"
        )
        for slug, other in sorted(divergence.items()):
            alts = ", ".join(f"{src}={n}" for src, n in sorted(other.items()))
            print(f"  {slug:<44} {args.source}={counts.get(slug, 0)}  ({alts})")
        print(
            "  A `worktree` count on a shared checkout folds in every session's UNCOMMITTED\n"
            "  edits, so it describes a tree that exists at no commit. Use --source=head for a\n"
            "  number you can publish; --source=index for what a commit would ship."
        )

    print(
        "\nThat is a PROMPT TO ADJUDICATE, not a promotable set. The threshold is "
        f"n>={args.threshold} across >=2 SUBSYSTEMS, and subsystem spread has no "
        "machine-readable\nsource in this corpus — it is adjudicated by hand into the "
        "`**Promotes to:**` prose. This probe\ncannot compute it and does not try."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
