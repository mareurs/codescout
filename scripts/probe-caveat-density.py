#!/usr/bin/env python3
"""Declared-uncertainty density per defect class -- which classes are we least able to VERIFY.

WHY THIS EXISTS. `docs/trackers/issue-clusters.md` promotes a class on `n>=3` across `>=2`
subsystems. Neither bar reads CONFIDENCE. A class at n=22 where 60% of its members carry an
`unverified:` field saying what was never established is a different promotion candidate from
one at n=19 where 18% do, and until this probe nothing in the ledger expressed that. The
`unverified:` field already exists on every bug file, is already queryable, and was already
being written honestly -- it was simply never read in aggregate.

THE ASYMMETRY, AND IT IS THE WHOLE POINT. This measures DECLARED uncertainty. The field is
voluntary, so its absence is ambiguous by construction: `docs/issues/_TEMPLATE.md` says absence
means "nothing outstanding", and absence equally means "the author did not write one". The
instrument cannot separate those, and no larger corpus separates them either -- this is
CLAUDE.md's second Testing-Discipline law, a defect the RECORDING filters out rather than a
sampling problem. So the readings are one-directional:

    a HIGH rate IS evidence of doubt.
    a LOW rate is NOT evidence of confidence.

Never rank the bottom of this table. Rank the top, and treat the bottom as unmeasured. A class
whose authors do not write caveats is indistinguishable here from one with nothing to caveat.

TWO CONFOUNDS, BOTH MEASURED, BOTH CONTROLLED BY DEFAULT.

1. ERA. The field barely existed before 2026-07. Measured 2026-09-04 over the 672-file corpus:
   0% (2026-05), 0% (2026-06), 2% (2026-07), 26% (2026-08), 34% (2026-09). An uncontrolled
   cross-class comparison therefore measures CONVENTION ADOPTION and reports it as epistemics --
   newer classes look less certain because their members are newer. `--since` (default 2026-08)
   is the control. Run `--by-month` to see the curve rather than take this on trust.

2. THE `scripts/` TEST-HARNESS GAP. Many caveats read verbatim "No regression test -- `scripts/`
   has no test harness in this repo", which is a property of the SUBSYSTEM, not of the defect
   class. Measured in the same window: 43% for files citing `scripts/` against 25% for files
   that do not. A class concentrated there inherits the rate. `--exclude-harness-gap` drops those
   files; the default prints both columns so the reader sees whether a cell moved.

   Measured 2026-09-04, the class ranking SURVIVES both controls -- 9%-60% against a controlled
   baseline of 25%, with the high band being exactly the classes whose CLAIM is that some fact
   is unobservable from where the observer stands (selector-narrower-than-its-population,
   repro-env-diverges-from-gate-env, shared-resource-carries-no-owner,
   gate-keyed-on-unobservable-event) and the low band the classes whose defect is visible once
   you look (addressing-without-an-escape-hatch, declared-not-wired,
   accepted-parameter-silently-dropped, capped-result-presented-as-complete). A third
   alternative was tested and REJECTED: open-vs-terminal status is flat, 30% against 29%, so
   this is not "unfinished work carries more caveats".

IT RE-DERIVES NOTHING. `bug_files`, `frontmatter`, `cluster_tags` and `actual_counts` come from
`scripts/pre-commit-ledger-counts.py`, itself pinned against `tests/issue_clusters.rs`. This is
not a style preference: a first draft of this probe re-implemented the frontmatter reader and
under-reported five classes because it handled only the BLOCK YAML form, so every file writing
`tags: [a, b]` inline lost its tag; a second draft imported the gate correctly and then filtered
its output on `startswith("cluster/")`, which `cluster_tags` has already stripped, returning a
clean well-formatted 0 for every class. Neither raised an error. Both are now caught by
`_self_check` below, which fails LOUDLY rather than printing a plausible table -- and that check
is the reason to trust this file, not the prose above it.

POPULATION IS A FLOOR, and by the same construction as the census: `git ls-files docs/issues`,
so a peer's untracked in-flight bug file is invisible. Default source is the INDEX rather than
the worktree -- measured 2026-09-04, a peer's in-flight archive `doc(action="move")` left a file
tracked-but-deleted, invisible at its old path and untracked at its new one simultaneously.

ONE TAG PER FILE. The ledger's one-cluster-per-bug rule makes these rates a partition, not a
tally -- but a bug instantiating two classes is counted for one, and the ledger's own Index
records that the class which disappears is systematically the less-developed one.
"""

from __future__ import annotations

import argparse
import collections
import importlib.util
import json
import os
import pathlib
import re
import sys

_HERE = pathlib.Path(__file__).resolve().parent

# The caveat field, and the axes it is read against. `opened:` dates the file for the era
# control; a cited `scripts/` path flags the harness confound.
_CAVEAT = "unverified"
_SCALAR = re.compile(r"^([A-Za-z_][A-Za-z0-9_]*):(.*)$")
_PATH = re.compile(
    r"\b(?:src|crates|tests|benches|examples|scripts|\.github|docs|assets)"
    r"/[A-Za-z0-9_][A-Za-z0-9_./-]*\.[A-Za-z0-9]{1,5}\b"
)


def _load_gate():
    """Import the hyphenated hook script as a module; it guards main(), so this runs nothing."""
    path = _HERE / "pre-commit-ledger-counts.py"
    spec = importlib.util.spec_from_file_location("_ledger_counts", path)
    if spec is None or spec.loader is None:  # pragma: no cover - unreachable in-tree
        raise SystemExit(f"cannot load {path}")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


G = _load_gate()


def scalars(fm: str) -> dict[str, str]:
    """Top-level scalar keys of a frontmatter block.

    Deliberately NOT a YAML parser: it reads only unindented `key:` lines, so a block-sequence
    value registers as present-with-empty-value and a quoted scalar's continuation lines are
    ignored. Every field this probe reads (`unverified`, `opened`, `status`) is written on one
    unindented line throughout the corpus; if that stops being true this returns a wrong answer
    silently, which is why `_self_check` cross-checks the one field the gate also computes.
    """
    return {m.group(1): m.group(2).strip().strip("'\"") for m in map(_SCALAR.match, fm.splitlines()) if m}


def subsystem(path: str) -> str:
    parts = path.split("/")
    if parts[0] in ("src", "crates") and len(parts) > 2:
        return f"{parts[0]}/{parts[1]}"
    if parts[0] == "src":
        return f"src/{parts[1].removesuffix('.rs')}"
    if parts[0] == ".github":
        return "ci"
    if parts[0] == "docs":
        return "docs/issues" if len(parts) > 1 and parts[1] == "issues" else "docs"
    return parts[0]


def scan(source: str) -> list[dict]:
    rows = []
    for rel in G.bug_files():
        text = G.read(rel, source)
        if text is None:
            # Tracked but unreadable in this tree: a peer's in-flight move, or a deletion not
            # yet staged. Skipped rather than counted as caveat-free, which would bias the
            # rate DOWNWARD -- the direction that reads as reassurance.
            continue
        fm = G.frontmatter(text) or ""
        s = scalars(fm)
        tags = G.cluster_tags(fm)  # returns SLUGS; the `cluster/` prefix is already stripped
        subs = {subsystem(m) for m in _PATH.findall(text)}
        rows.append(
            {
                "path": rel,
                "cluster": tags[0] if tags else None,
                "caveat": _CAVEAT in s,
                "opened": s.get("opened", ""),
                "status": s.get("status", ""),
                "harness_gap": "scripts" in subs,
            }
        )
    return rows


def _self_check(rows: list[dict], source: str) -> None:
    """Fail loudly if this probe's membership disagrees with the gate's.

    Both of this script's development-time defects were silent wrong ANSWERS, not errors: one
    under-reported five classes, one reported zero for all of them. Neither is reachable now
    without tripping this. It is the only reason an unpinned probe is worth trusting.

    VERIFIED BY OBSERVED RED, 2026-09-04, at both guarded sites, mutating the production path
    rather than the test's inputs: re-filtering already-stripped slugs reds with `this probe 0,
    gate 19`; a block-only tag reader reds with `this probe 19, gate 24` -- the latter numerically
    reproducing draft one's actual readings.

    WHAT IT CANNOT CATCH, and this is not a caveat, it is the shape of the check. It compares
    this probe against the GATE, and the two SHARE `bug_files`, `frontmatter` and `cluster_tags`
    by construction -- that sharing is the point. So a defect inside a shared function is
    invisible here: both sides compute it wrong and agree, which at the point of use is
    indistinguishable from corroboration. Demonstrated the same day by mutating `G.cluster_tags`
    itself, which this check passed GREEN while every count was zero. The gate's own pinning
    against `tests/issue_clusters.rs` is what covers that layer; this covers only the layer above
    it. Two instruments agreeing is evidence only where their scopes differ.
    """
    mine = collections.Counter(r["cluster"] for r in rows if r["cluster"])
    theirs = G.actual_counts(G.valid_slugs(G.read_ledger("worktree")), source)
    bad = {s: (mine.get(s, 0), n) for s, n in theirs.items() if mine.get(s, 0) != n}
    if bad:
        lines = "\n".join(f"  {s}: this probe {a}, gate {b}" for s, (a, b) in sorted(bad.items()))
        raise SystemExit(
            "SELF-CHECK FAILED -- membership disagrees with pre-commit-ledger-counts.py.\n"
            f"{lines}\n"
            "Do not read the table below; it is computed over a different population than the\n"
            "gate enforces. Most likely cause: a frontmatter or tag reader drifted from the\n"
            "gate's. This probe must IMPORT those, never re-implement them."
        )


def rate(rows: list[dict]) -> tuple[int, int]:
    return sum(r["caveat"] for r in rows), len(rows)


def _fmt(u: int, n: int, min_n: int) -> str:
    return f"{u}/{n} = {100 * u / n:.0f}%" if n >= min_n else f"{u}/{n} (n<{min_n})"


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--source", choices=("index", "worktree", "head"), default="index")
    ap.add_argument("--since", default="2026-08", help="YYYY-MM era floor (default 2026-08)")
    ap.add_argument("--min-n", type=int, default=6, help="suppress rates below this n")
    ap.add_argument("--exclude-harness-gap", action="store_true")
    ap.add_argument("--by-month", action="store_true", help="print the era curve and exit")
    ap.add_argument("--json", action="store_true")
    args = ap.parse_args()

    os.chdir(pathlib.Path(G._git("rev-parse", "--show-toplevel").strip()))
    rows = scan(args.source)
    _self_check(rows, args.source)

    if args.by_month:
        by = collections.defaultdict(list)
        for r in rows:
            by[r["opened"][:7] or "(no opened:)"].append(r)
        print("— the ERA confound, derived rather than cited —")
        for m in sorted(by):
            u, n = rate(by[m])
            print(f"  {m:<12} {_fmt(u, n, 1)}")
        print("\nThis is why cross-class comparison is windowed. A class whose members are newer")
        print("looks less certain for reasons that are about the CONVENTION, not the defect.")
        return 0

    win = [r for r in rows if r["opened"][:7] >= args.since]
    ctl = [r for r in win if not r["harness_gap"]]
    use = ctl if args.exclude_harness_gap else win

    by_all = collections.defaultdict(list)
    by_ctl = collections.defaultdict(list)
    for r in win:
        by_all[r["cluster"] or "(untagged)"].append(r)
    for r in ctl:
        by_ctl[r["cluster"] or "(untagged)"].append(r)

    if args.json:
        print(json.dumps({
            "source": args.source, "since": args.since, "min_n": args.min_n,
            "baseline": dict(zip(("caveats", "n"), rate(use))),
            "harness_gap_confound": {
                "cites_scripts": dict(zip(("caveats", "n"), rate([r for r in win if r["harness_gap"]]))),
                "does_not": dict(zip(("caveats", "n"), rate(ctl))),
            },
            "classes": {
                k: {"windowed": rate(v), "harness_free": rate(by_ctl.get(k, []))}
                for k, v in by_all.items()
            },
        }, indent=2))
        return 0

    bu, bn = rate(use)
    print(f"caveat density — `{_CAVEAT}:` per defect class")
    print(f"source: {args.source} · window: opened >= {args.since} · n={len(use)} · "
          f"baseline {100 * bu / bn:.0f}%\n")

    gu, gn = rate([r for r in win if r["harness_gap"]])
    cu, cn = rate(ctl)
    print("— the harness-gap confound (a SUBSYSTEM property, not an epistemic one) —")
    print(f"  cites scripts/ : {_fmt(gu, gn, 1)}")
    print(f"  does not       : {_fmt(cu, cn, 1)}\n")

    print(f"  {'cluster':<44} {'windowed':>15} {'harness-free':>15}")
    ranked = sorted(by_all.items(), key=lambda kv: -(kv[1] and rate(kv[1])[0] / len(kv[1])))
    for slug, rs in ranked:
        u, n = rate(rs)
        if n < args.min_n:
            continue
        cu2, cn2 = rate(by_ctl.get(slug, []))
        print(f"  {slug:<44} {_fmt(u, n, args.min_n):>15} {_fmt(cu2, cn2, args.min_n):>15}")

    print(f"\n  ({sum(1 for _, rs in by_all.items() if len(rs) < args.min_n)} class(es) "
          f"suppressed at n<{args.min_n}; raise with --min-n)")
    print("\nREAD THE TOP, NOT THE BOTTOM. A high rate is evidence of doubt; a low rate is NOT")
    print("evidence of confidence — this counts DECLARED uncertainty, and a class whose authors")
    print("do not write the field is indistinguishable here from one with nothing to declare.")
    print("The bottom of this table is unmeasured, not clean.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
