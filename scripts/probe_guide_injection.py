#!/usr/bin/env python3
"""Measure codescout guide AUTO-INJECTION delivery across Claude Code transcripts.

Answers "how much guide text does the push path actually deliver, to whom, and how
much of it is duplicate" at corpus scale. The *use* half of that question is not
scriptable and was measured separately (see
docs/evals/2026-08-27-guide-injection-use.md).

Read the blind spots in docs/PROBES.md before quoting any number from this.
The short version, because two of them have already produced wrong figures:

  * Every injection carries TWO markers (opening + closing). Counting occurrences
    of `auto-injected get_guide(` double-counts. This counts openings only.
  * Guide files GROW. `tracker-conventions.md` went 10,377 -> 34,333 B in ten days.
    Looking up today's file size and applying it to a July injection overstated the
    corpus total by 1.17x. This measures the bytes actually between the markers.

Usage:
    python3 scripts/probe_guide_injection.py [--main-only] [--fix-date YYYY-MM-DD]
                                             [--profiles DIR [DIR ...]] [--json]
"""
from __future__ import annotations

import argparse
import collections
import json
import os
import statistics as st
import sys

OPEN_MARK = "<!-- auto-injected get_guide('"
END_MARK = "<!-- end auto-injected"

# The ten registered topics. Anything else matching the marker shape is a false
# positive -- `{topic}` and `{}` are the literal Rust format string from
# src/tools/core/types.rs, seen in transcripts where somebody read that source.
TOPICS = {
    "tracker-conventions", "librarian", "iron-laws-detail", "workspace-state",
    "librarian-runtime", "progressive-disclosure", "untrusted-content",
    "symbol-navigation", "project-activation-bootstrap", "error-handling",
}

DEFAULT_PROFILES = ["~/.claude", "~/.claude-sdd", "~/.claude-kat"]


def scan_transcript(path: str) -> dict | None:
    """One transcript -> census. Returns None if it has no assistant model line."""
    models: collections.Counter = collections.Counter()
    inj: list[tuple[str, int, str]] = []          # (topic, true_bytes, date)
    trig: collections.Counter = collections.Counter()
    uses: dict[str, str] = {}
    rejected = mcp_cmds = compactions = 0

    try:
        fh = open(path, encoding="utf-8", errors="replace")
    except OSError:
        return None

    with fh:
        for line in fh:
            # cheap pre-filters before the JSON parse
            if "<command-name>" in line and "mcp" in line:
                mcp_cmds += 1
            if '"compact_boundary"' in line:
                compactions += 1
            if '"tool_use"' not in line and OPEN_MARK not in line and '"model"' not in line:
                continue
            try:
                d = json.loads(line)
            except (ValueError, TypeError):
                continue

            msg = d.get("message") or {}
            if (m := msg.get("model")):
                models[m] += 1

            content = msg.get("content")
            if not isinstance(content, list):
                continue

            # remember tool_use ids so an injection can be attributed to its trigger
            for b in content:
                if isinstance(b, dict) and b.get("type") == "tool_use":
                    uses[b.get("id")] = b.get("name") or ""

            if OPEN_MARK not in line:
                continue

            ts = (d.get("timestamp") or "")[:10]
            for b in content:
                if not (isinstance(b, dict) and b.get("type") == "tool_result"):
                    continue
                cc = b.get("content")
                if isinstance(cc, str):
                    text = cc
                elif isinstance(cc, list):
                    text = "".join(x.get("text", "") for x in cc if isinstance(x, dict))
                else:
                    continue
                if OPEN_MARK not in text:
                    continue

                tool = uses.get(b.get("tool_use_id"), "UNPAIRED")
                # GUARD: a genuine injection rides a codescout MCP result and carries
                # exactly one opening marker. More than two means the tool_result is
                # quoting a transcript, not receiving an injection.
                if not tool.startswith("mcp__codescout__") or text.count(OPEN_MARK) > 2:
                    rejected += 1
                    continue

                for seg in text.split(OPEN_MARK)[1:]:
                    topic = seg.split("'")[0]
                    if topic not in TOPICS:
                        continue
                    end = seg.find(END_MARK)
                    if end < 0:            # truncated in the transcript; skip rather than guess
                        continue
                    inj.append((topic, end, ts))
                    trig[f"{topic}|{tool}"] += 1

    if not models:
        return None
    return {
        "path": path,
        "sid": os.path.basename(path),
        "dom": models.most_common(1)[0][0],
        "asst": sum(models.values()),
        "injections": inj,
        "triggers": dict(trig),
        "rejected": rejected,
        "mcp_cmds": mcp_cmds,
        "compactions": compactions,
        "is_subagent": "/subagents/" in path,
    }


def collect(profiles: list[str]) -> list[dict]:
    rows, walked = [], 0
    for prof in profiles:
        root = os.path.join(os.path.expanduser(prof), "projects")
        for dirpath, _, files in os.walk(root):
            for f in files:
                if not f.endswith(".jsonl"):
                    continue
                walked += 1
                if (r := scan_transcript(os.path.join(dirpath, f))):
                    rows.append(r)
    # Same session-id can exist under more than one profile (49 of 1703 at baseline).
    # Keep the largest copy so the corpus is not double-counted.
    best: dict[str, dict] = {}
    for r in rows:
        if r["sid"] not in best or len(r["injections"]) > len(best[r["sid"]]["injections"]):
            best[r["sid"]] = r
    print(f"# transcripts walked: {walked}  parsed: {len(rows)}  unique sessions: {len(best)}",
          file=sys.stderr)
    return [r for r in best.values() if r["injections"]]


def report(rows: list[dict], label: str) -> dict:
    total = repeat_bytes = 0
    per_topic: collections.Counter = collections.Counter()
    sessions: collections.Counter = collections.Counter()
    counts = []
    for r in rows:
        seen: collections.Counter = collections.Counter()
        for topic, nbytes, _ in r["injections"]:
            total += nbytes
            per_topic[topic] += nbytes
            seen[topic] += 1
            if seen[topic] > 1:
                repeat_bytes += nbytes
        for t in seen:
            sessions[t] += 1
        counts.append(len(r["injections"]))

    out = {
        "label": label, "sessions": len(rows), "total_bytes": total,
        "mean_bytes_per_session": total // len(rows) if rows else 0,
        "median_injections": st.median(counts) if counts else 0,
        "max_injections": max(counts) if counts else 0,
        "repeat_share": repeat_bytes / total if total else 0.0,
        "by_topic": {
            t: {"bytes": b, "share": b / total, "sessions": sessions[t],
                "session_share": sessions[t] / len(rows)}
            for t, b in per_topic.most_common()
        },
        "never_delivered": sorted(TOPICS - set(per_topic)),
    }
    print(f"\n### {label}   n={out['sessions']} sessions")
    print(f"    {total/1e6:.1f} MB delivered | mean {out['mean_bytes_per_session']:,} B/session"
          f" | median {out['median_injections']:.0f} injections (max {out['max_injections']})")
    print(f"    repeats: {out['repeat_share']*100:.0f}% of delivered bytes")
    for t, v in out["by_topic"].items():
        print(f"      {t:30}{v['session_share']*100:4.0f}% of sessions  "
              f"{v['bytes']/1e6:6.2f} MB  {v['share']*100:5.1f}% of bytes")
    if out["never_delivered"]:
        print(f"    NEVER auto-injected: {', '.join(out['never_delivered'])}")
    return out


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--profiles", nargs="+", default=DEFAULT_PROFILES)
    ap.add_argument("--main-only", action="store_true",
                    help="exclude /subagents/ transcripts (they share the parent's ledger)")
    ap.add_argument("--fix-date", default="2026-08-20",
                    help="split pre/post this date; default is the day after the "
                         "2026-08-19 activate-clears-the-ledger fix")
    ap.add_argument("--json", action="store_true")
    args = ap.parse_args()

    rows = collect(args.profiles)
    if args.main_only:
        rows = [r for r in rows if not r["is_subagent"]]

    def first_date(r):
        ds = [d for _, _, d in r["injections"] if d]
        return min(ds) if ds else ""

    main_rows = [r for r in rows if not r["is_subagent"]]
    out = {
        "all": report(rows, "ALL SESSIONS"),
        "main": report(main_rows, "MAIN SESSIONS ONLY"),
        "dev": report([r for r in main_rows if r["mcp_cmds"] > 0],
                      "DEV (ran /mcp -- rebuild loop; artifact, not usage)"),
        "normal": report([r for r in main_rows if r["mcp_cmds"] == 0],
                         "NORMAL (never ran /mcp)"),
        "clean": report([r for r in main_rows if r["mcp_cmds"] == 0 and r["compactions"] == 0],
                        "NORMAL + never compacted  <<< the real-usage figure"),
        "post_fix_clean": report(
            [r for r in main_rows if r["mcp_cmds"] == 0 and r["compactions"] == 0
             and first_date(r) >= args.fix_date],
            f"POST-{args.fix_date} + no /mcp + no compaction  <<< TODAY"),
    }

    trig: collections.Counter = collections.Counter()
    for r in rows:
        for k, v in r["triggers"].items():
            trig[k] += v
    out["triggers"] = dict(trig.most_common(12))
    print("\n### what triggers delivery (topic|tool)")
    for k, v in trig.most_common(12):
        print(f"    {v:6}  {k}")

    rejected = sum(r["rejected"] for r in rows)
    print(f"\n# guard rejected {rejected} marker-bearing tool_results "
          f"(quoted transcripts / non-codescout triggers)")
    if args.json:
        json.dump(out, sys.stdout, indent=1)
    return 0


if __name__ == "__main__":
    sys.exit(main())
