#!/usr/bin/env python3
"""Why the clusters ledger has no commit-path check for a BACKTICKED `n=`, re-derivable.

`no_class_field_states_a_bare_n` refuses a bare `n=` on a `**Members:**` / `**Promotes to:**`
line. The backtick is the escape, and it is the only signal separating a live claim from a
quotation -- which means a live count written in backticks passes the gate and decays exactly
like the stored count the gate was inverted to remove
(`docs/issues/archive/2026-09-02-the-no-stored-count-gate-is-defeated-by-house-style.md`).

The obvious repair is a diff-scoped hook check: refuse a backticked `n=` that is on a gated line
in the INDEX and was not on that line at HEAD. Existing quotations are untouched forever because
their lines do not change. It looks free.

THIS SCRIPT IS WHY IT IS NOT, and it exists so the refusal is a measurement a reader can re-run
rather than a verdict they must take on trust. `--replay` walks every commit that ever touched
the ledger and reports what that check would have refused. Run it before rebuilding the check.

TWO NUMBERS, TWO EPISTEMIC KINDS, and conflating them is the trap this script is shaped to
avoid. The hit count is DERIVED and exact. Whether a given hit was a live claim (a true refusal)
or a quotation (a wrong one) is an ADJUDICATION -- it turns on whether the author meant to assert
the number or to mention it, which no parser reaches. So `--replay` prints the surrounding text
and refuses to classify: the reader classifies, and the bug file records what one reader got.

`--agreement` reports the one discriminator that IS computable. A backticked `n=7` on a class the
corpus says holds 24 is visibly a quotation -- no reader mistakes it for current. A backticked
`n=24` on a class holding 24 reads as today's count whether or not it is one, and goes silently
wrong on the class's next member. Agreement is therefore the hazard state, and it is a small
fraction of the quotations rather than most of them, which is what makes it an advisory rather
than a re-print of the ledger.

BOTH MODES PRINT THEIR DENOMINATOR, and that is not decoration. Every reading here is an
EXISTENCE report over a parse, so a parser that silently matched nothing would return a clean
empty list -- indistinguishable from a healthy ledger. A `0 of 0` is visibly broken where a bare
`0` is reassuring. There is no test behind this script; the denominator is the vacuity guard.

IT RE-DERIVES NOTHING. The span semantics come from `bare_n_values` in
`scripts/pre-commit-ledger-counts.py`, which is pinned against the Rust in
`tests/issue_clusters.rs`. `backticked_values` below is its exact complement and the comment on
it says so -- if the two ever have to move, they move together.
"""

from __future__ import annotations

import argparse
import importlib.util
import os
import pathlib
import subprocess
import sys

_HERE = pathlib.Path(__file__).resolve().parent
_spec = importlib.util.spec_from_file_location("gate", _HERE / "pre-commit-ledger-counts.py")
G = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(G)

# The two judgement fields `no_class_field_states_a_bare_n` gates. Kept as a literal rather than
# imported because the gate holds it inline in `parse_bare_n_claims`; if that list grows, this
# one must too, and a reader comparing them should see both.
FIELDS = ("**Members:**", "**Promotes to:**")

# The commit that INVERTED the gate -- before it a bare `n=` was legal, so demoting a superseded
# live count to a backticked quotation was the prescribed act and shows up here as a hit that
# could never recur. Everything at or before this commit is a different regime; only what comes
# after predicts what the check would refuse going forward.
INVERSION = "1b3ac36b"


def _git(*args: str) -> str:
    r = subprocess.run(["git", *args], capture_output=True, text=True)
    return r.stdout if r.returncode == 0 else ""


def backticked_values(rest: str) -> list[tuple[int, int]]:
    """[(value, offset)] for every `n=<digits>` INSIDE a complete backtick span.

    The exact complement of `bare_n_values` in `scripts/pre-commit-ledger-counts.py`: same span
    scan, opposite membership test. SPAN, not adjacency -- a backticked PHRASE wrapping an `n=`
    counts as quoted, which is the shape an adjacency check shipped wrong at `cd17a58c`.
    Complete pairs only, so a DANGLING backtick opens nothing and its tail falls on the bare
    side. That asymmetry is inherited deliberately: it makes this script UNDER-report, which is
    the safe direction for something whose whole output is a prompt to re-read.
    """
    spans: list[tuple[int, int]] = []
    open_at: int | None = None
    for i, ch in enumerate(rest):
        if ch == "`":
            if open_at is None:
                open_at = i
            else:
                spans.append((open_at, i))
                open_at = None

    out: list[tuple[int, int]] = []
    i = 0
    while True:
        at = rest.find("n=", i)
        if at < 0:
            return out
        digits = ""
        for ch in rest[at + 2 :]:
            if not ch.isdigit():
                break
            digits += ch
        if digits and any(a < at < b for a, b in spans):
            out.append((int(digits), at))
        i = at + 2


def gated_lines(ledger: str, valid: set[str]) -> dict[tuple[str, str], str]:
    """(slug, field) -> that field's line, minus its prefix.

    Slug resets at every `## IC-` heading and is not consumed on use, matching
    `parse_bare_n_claims`: a section stating a figure while declaring no slug must be DROPPED,
    never filed under whichever slug happened to be set last.
    """
    out: dict[tuple[str, str], str] = {}
    cur: str | None = None
    for line in ledger.splitlines():
        if line.startswith("## IC-"):
            cur = None
        elif line.startswith("**Slug:**"):
            cur = G.backticked_cluster_slug(line[len("**Slug:**") :])
        elif cur is not None and cur in valid:
            for prefix in FIELDS:
                if line.startswith(prefix):
                    out[(cur, prefix)] = line[len(prefix) :]
    return out


def ledger_at(sha: str) -> str | None:
    """Index file + every class file as of `sha`. None when the Index is absent from that tree."""
    paths = [
        p
        for p in _git("ls-tree", "-r", "--name-only", sha, "docs/trackers/").split("\n")
        if p == G.LEDGER or (p.startswith("docs/trackers/issue-clusters/") and p.endswith(".md"))
    ]
    if G.LEDGER not in paths:
        return None
    return "\n".join(_git("show", f"{sha}:{p}") for p in paths)


_CACHE: dict[str, dict[tuple[str, str], str]] = {}


def snapshot(sha: str) -> dict[tuple[str, str], str]:
    if sha not in _CACHE:
        led = ledger_at(sha)
        _CACHE[sha] = {} if led is None else gated_lines(led, G.valid_slugs(led))
    return _CACHE[sha]


def replay(context: int, all_history: bool) -> int:
    head = _git("rev-parse", "--short", "HEAD").strip()
    log = _git(
        "log", "--format=%H %s", "--", G.LEDGER, "docs/trackers/issue-clusters/"
    ).strip().split("\n")
    log = [ln for ln in log if ln]

    print(f"replaying the proposed backticked-`n=` check over {len(log)} commits")
    print(f"tree: HEAD={head}   regime: {'ALL history' if all_history else f'after {INVERSION}'}\n")

    commits = 0
    tokens = 0
    for ln in log:
        sha, _, subject = ln.partition(" ")
        if sha.startswith(INVERSION) and not all_history:
            break  # newest-first, so this and everything below predate the inversion
        parents = _git("rev-list", "--parents", "-n", "1", sha).split()
        if len(parents) < 2:
            continue  # a root commit has no parent to diff the field against
        now, was = snapshot(sha), snapshot(parents[1])
        shown = False
        for (slug, field), text in sorted(now.items()):
            old = was.get((slug, field), "")
            fresh = {v for v, _ in backticked_values(text)} - {v for v, _ in backticked_values(old)}
            if not fresh:
                continue
            if not shown:
                commits += 1
                shown = True
                print(f"=== {sha[:8]}  {subject[:70]}")
            print(f"    cluster/{slug}  {field}")
            for val, off in backticked_values(text):
                if val not in fresh:
                    continue
                tokens += 1
                lo, hi = max(0, off - context), min(len(text), off + context)
                print(f"      n={val}: ...{text[lo:hi]}...")

    print(f"\n-- {commits} commit(s) refused, carrying {tokens} newly-appearing token(s)")
    if tokens == 0:
        print("-- a ZERO here is a broken parser as readily as a clean history: this ledger's")
        print("   own migration backticked dozens of figures, so `--all-history` must be >0.")
        return 1
    print("-- CLASSIFY THE TOKENS ABOVE YOURSELF. Whether each was a live claim (a correct")
    print("   refusal) or a quotation (a wrong one) turns on whether the author ASSERTED the")
    print("   number or MENTIONED it, which is not in the grammar. That is the whole finding:")
    print("   a check keyed on this token cannot make the distinction the token cannot carry.")
    return 0


def agreement(source: str) -> int:
    ledger = G.read_ledger("worktree")
    if ledger is None:
        raise SystemExit(f"cannot read {G.LEDGER}")
    valid = G.valid_slugs(ledger)
    counts = G.actual_counts(valid, source)

    total = 0
    hits: list[tuple[str, str, int, int]] = []
    for (slug, field), text in sorted(gated_lines(ledger, valid).items()):
        real = counts.get(slug, 0)
        for val, _ in backticked_values(text):
            total += 1
            if val == real:
                hits.append((slug, field, val, real))

    print(f"backticked figures on gated fields, source={source}\n")
    for slug, field, val, real in hits:
        print(f"  READS AS CURRENT  cluster/{slug}")
        print(f"                    {field} quotes `n={val}`, corpus holds {real}")
    print(f"\n-- {len(hits)} of {total} backticked figure(s) EQUAL today's derived count")
    if total == 0:
        print("-- 0 of 0 is a broken parser, not a clean ledger: the 2026-09-02 migration")
        print("   backticked every live figure it retired, so this denominator cannot be 0.")
        return 1
    print(f"-- the other {total - len(hits)} differ from it, so they are VISIBLY quotations")
    print()
    print("Each line above is right today and goes wrong on that class's next member, with")
    print("nothing to report it -- the gate skips backticked figures by design. Re-read the")
    print("sentence: if it asserts today's count, delete the number and cite")
    print("`scripts/probe-cluster-census.py`; if it quotes a retired one, it is fine and will")
    print("become visibly so. This is an advisory on a READ path and refuses nothing: a check")
    print("that refused here would be the one --replay measures and rejects.")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    mode = ap.add_mutually_exclusive_group()
    mode.add_argument(
        "--replay",
        action="store_true",
        help="walk history and print what the proposed commit-path check would have refused",
    )
    # Named rather than left implicit: this is the default mode, and a default with no spelling
    # cannot be cited. The bug file and docs/PROBES.md both name it, and a citation to a flag
    # that does not parse is a worse failure than a missing flag -- argparse exits 2 with
    # `unrecognized arguments`, which reads as the reader's mistake.
    mode.add_argument(
        "--agreement",
        action="store_true",
        help="report backticked figures that EQUAL today's derived count (default)",
    )
    ap.add_argument(
        "--all-history",
        action="store_true",
        help="with --replay, include the pre-inversion regime (the one-time migration commits)",
    )
    ap.add_argument("--context", type=int, default=150, help="chars either side of a token")
    ap.add_argument(
        "--source",
        choices=("index", "worktree", "head"),
        default="worktree",
        help="which tree to count members over for --agreement",
    )
    args = ap.parse_args()

    root = pathlib.Path(G._git("rev-parse", "--show-toplevel").strip())
    # chdir first: since the 2026-09-02 split `read_ledger` enumerates class files by
    # repo-relative path, and reading the Index alone returns a clean, plausible, empty census.
    os.chdir(root)

    if args.replay:
        return replay(args.context, args.all_history)
    return agreement(args.source)


if __name__ == "__main__":
    sys.exit(main())
