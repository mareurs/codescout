#!/usr/bin/env python3
"""Before-and-after check for the progressive-disclosure split (bug 7579b32b1cd2362f).

Question: did agents misuse overflow buffers more often once an overflow stopped delivering the
whole guide and delivered only the sections for the overflowing tool?

Instrument: `usage.db` `tool_calls`. Exposure is `SUM(overflowed)`, the calls whose result was
buffered. Events are rows whose `err_family` is one of MISUSE_FAMILIES, the families the removed
guide text teaches around: a bad `json_path`, `json_path` on a raw-text handle, a handle that no
longer exists. Rate = events per 1,000 overflowed calls.

Cohorts are by BUILD, not by date: many builds (peer worktrees, old sessions) write to the same
database at the same time, so a date cutoff would mix them. A row is "after" when its
`codescout_sha` has the split commit as an ancestor, "before" when it does not, "unknown" when git
does not know the sha. Dirty builds are kept: uncommitted changes do not remove a committed commit.

What this cannot show: it is not randomised. Other changes ship in the same builds, and the base
rate is small (about 95 events in 30 days, against about 6,800 overflowed calls, when this was
written), so only a large rise is detectable in a week. A flat result is "no rise detected", never
"no effect". Misuse of the `.err` suffix has no error family and is not counted.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import math
import sqlite3
import subprocess
import sys
from pathlib import Path
from typing import Callable

ROOT = Path(__file__).resolve().parents[1]

DEFAULT_SPLIT_COMMIT = "281ff16b3"

MISUSE_FAMILIES = (
    "json_path_key_miss",
    "json_path_unsupported",
    "json_path_wrong_buffer_kind",
    "json_path_shape_mismatch",
    "buffer_ref_expired",
)

# Pre-registered decision rule. Written down before the data exists.
MIN_AFTER_EXPOSURE = 1000  # overflowed calls in the "after" cohort before any verdict
ALARM_RATE_RATIO = 2.0
ALARM_P = 0.05
ALARM_MIN_EVENTS = 10


def classify_builds(shas, is_ancestor: Callable[[str], bool | None]) -> dict[str, str]:
    """Map each build sha to 'after', 'before' or 'unknown' by whether it contains the split."""
    out = {}
    for sha in shas:
        got = is_ancestor(sha)
        out[sha] = "unknown" if got is None else ("after" if got else "before")
    return out


def per_build_counts(conn: sqlite3.Connection, since: str, agent: str = "all") -> list[dict]:
    """Per build: overflowed calls (exposure) and misuse events, by family."""
    clause = {"all": "", "parent": " AND agent_id IS NULL", "subagent": " AND agent_id IS NOT NULL"}[agent]
    rows = conn.execute(
        "SELECT codescout_sha, err_family, COUNT(*), COALESCE(SUM(overflowed), 0) FROM tool_calls"
        f" WHERE called_at >= ? AND codescout_sha IS NOT NULL{clause}"
        " GROUP BY codescout_sha, err_family",
        (since,),
    ).fetchall()
    builds: dict[str, dict] = {}
    for sha, family, n, over in rows:
        b = builds.setdefault(sha, {"sha": sha, "overflowed": 0, "misuse": 0, "by_family": {}})
        b["overflowed"] += over
        if family in MISUSE_FAMILIES:
            b["misuse"] += n
            b["by_family"][family] = b["by_family"].get(family, 0) + n
    return sorted(builds.values(), key=lambda b: b["sha"])


def cohort_totals(builds: list[dict], classification: dict[str, str]) -> dict[str, dict]:
    totals = {c: {"overflowed": 0, "misuse": 0, "by_family": {}, "builds": 0} for c in ("before", "after", "unknown")}
    for b in builds:
        t = totals[classification[b["sha"]]]
        t["overflowed"] += b["overflowed"]
        t["misuse"] += b["misuse"]
        t["builds"] += 1
        for fam, n in b["by_family"].items():
            t["by_family"][fam] = t["by_family"].get(fam, 0) + n
    return totals


def rate_per_1000(events: int, exposure: int) -> float | None:
    return None if exposure <= 0 else 1000.0 * events / exposure


def rate_ratio_p(a_events: int, a_exposure: float, b_events: int, b_exposure: float) -> float:
    """Two-sided exact p-value that two Poisson rates are equal (conditional binomial test).

    Given n = a + b events, under equal rates a ~ Binomial(n, Ea / (Ea + Eb)). The p-value sums
    every outcome that is no more probable than the one observed.
    """
    n = a_events + b_events
    if n == 0:
        return 1.0
    p0 = a_exposure / (a_exposure + b_exposure)

    def log_pmf(k: int) -> float:
        return (
            math.lgamma(n + 1) - math.lgamma(k + 1) - math.lgamma(n - k + 1)
            + k * math.log(p0) + (n - k) * math.log(1.0 - p0)
        )

    observed = log_pmf(a_events)
    total = sum(math.exp(log_pmf(k)) for k in range(n + 1) if log_pmf(k) <= observed + 1e-9)
    return min(1.0, total)


def verdict(before: dict, after: dict) -> dict:
    """Apply the pre-registered rule to the two cohorts."""
    if after["overflowed"] < MIN_AFTER_EXPOSURE or before["overflowed"] <= 0:
        return {
            "status": "not-enough-data",
            "note": f"the after cohort has {after['overflowed']:,} overflowed calls; the rule needs at least "
            f"{MIN_AFTER_EXPOSURE:,} (and a non-empty before cohort) before it says anything",
        }
    r_before = rate_per_1000(before["misuse"], before["overflowed"])
    r_after = rate_per_1000(after["misuse"], after["overflowed"])
    p = rate_ratio_p(after["misuse"], after["overflowed"], before["misuse"], before["overflowed"])
    ratio = (r_after / r_before) if r_before else math.inf if r_after else 1.0
    base = {"rate_before": r_before, "rate_after": r_after, "ratio": ratio, "p": p}
    if ratio >= ALARM_RATE_RATIO and p < ALARM_P and after["misuse"] >= ALARM_MIN_EVENTS:
        return {**base, "status": "alarm",
                "note": f"misuse rose {ratio:.1f}-fold (p={p:.3f}); look at the families and consider restoring "
                "the sections that teach around them"}
    return {**base, "status": "no-change-detected",
            "note": "no rise detected. This cannot rule out a modest rise: at the baseline rate a week of data "
            "holds only about 20 events, enough to see roughly a doubling, not a 30% change"}


def git_is_ancestor(split: str, sha: str, repo: Path = ROOT) -> bool | None:
    r = subprocess.run(["git", "-C", str(repo), "merge-base", "--is-ancestor", split, sha],
                       capture_output=True, text=True)
    return {0: True, 1: False}.get(r.returncode)


def run(db: Path, split: str, since: str, agent: str, is_ancestor: Callable[[str], bool | None]) -> dict:
    conn = sqlite3.connect(f"file:{db}?mode=ro", uri=True)
    try:
        builds = per_build_counts(conn, since, agent)
    finally:
        conn.close()
    classification = classify_builds([b["sha"] for b in builds], is_ancestor)
    cohorts = cohort_totals(builds, classification)
    return {"split_commit": split, "since": since, "agent": agent, "cohorts": cohorts,
            "verdict": verdict(cohorts["before"], cohorts["after"])}


def format_report(report: dict) -> str:
    lines = [f"split commit {report['split_commit']}, rows since {report['since']}, agent={report['agent']}"]
    for name in ("before", "after", "unknown"):
        c = report["cohorts"][name]
        rate = rate_per_1000(c["misuse"], c["overflowed"])
        shown = "n/a" if rate is None else f"{rate:.1f}"
        lines.append(f"  {name:8s} builds={c['builds']:4d}  overflowed={c['overflowed']:7,d}  "
                     f"misuse={c['misuse']:4d}  per 1,000 overflowed={shown}")
        for fam, n in sorted(c["by_family"].items(), key=lambda kv: -kv[1]):
            lines.append(f"             {fam}: {n}")
    v = report["verdict"]
    lines.append(f"verdict: {v['status']} — {v['note']}")
    return "\n".join(lines)


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--db", type=Path, default=ROOT / ".codescout/usage.db")
    ap.add_argument("--split-commit", default=DEFAULT_SPLIT_COMMIT)
    ap.add_argument("--since", default=(dt.datetime.now(dt.timezone.utc) - dt.timedelta(days=30)).strftime("%Y-%m-%d"),
                    help="UTC date; `called_at` is UTC (default: 30 days ago)")
    ap.add_argument("--agent", choices=("all", "parent", "subagent"), default="all")
    ap.add_argument("--json", action="store_true")
    args = ap.parse_args(argv)
    report = run(args.db, args.split_commit, args.since, args.agent,
                 lambda sha: git_is_ancestor(args.split_commit, sha))
    print(json.dumps(report, indent=2) if args.json else format_report(report))
    return 0


if __name__ == "__main__":
    sys.exit(main())
