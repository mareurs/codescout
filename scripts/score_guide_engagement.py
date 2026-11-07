#!/usr/bin/env python3
"""Layer 1 of the subagent guide measurement: did the taught behaviour follow a delivery?

Input is the census (scripts/census_guide_deliveries.py). For each delivered block the scorer
reads the next K tool calls of the same transcript and asks a mechanical, per-topic question:
did one of them do what the guide teaches?

This measures RELEVANCE, not causation. A zero is strong evidence that the block went unused.
A one is an upper bound: the agent may have done it anyway, since the iron laws are already in
the server instructions. The predicates are the author's choice and are validated only by layer 2,
a reader judging a sample. A topic with no honest predicate is reported as not scored.

Usage:
    python3 scripts/census_guide_deliveries.py --out census.jsonl
    python3 scripts/score_guide_engagement.py --census census.jsonl --comparison
"""

from __future__ import annotations

import argparse
import datetime as dt
import importlib.util
import json
import math
import os
import re
import sys
from pathlib import Path

_HERE = Path(__file__).resolve().parent


def _load(name: str):
    spec = importlib.util.spec_from_file_location(name, _HERE / f"{name}.py")
    mod = importlib.util.module_from_spec(spec)
    sys.modules[name] = mod
    spec.loader.exec_module(mod)
    return mod


census = _load("census_guide_deliveries")
probe = census.probe

HANDLE = re.compile(r"@(?:cmd|tool|file|bg|ack)_[0-9a-zA-Z]+")
SERVER = "mcp__codescout__"


def engaged(topic: str, name: str, inp: dict) -> bool | None:
    """Whether one tool call does what `topic`'s guide teaches. None: the topic has no predicate."""
    if topic == "progressive-disclosure":
        # The guide teaches reading a buffered result through its @ref handle, json_path,
        # the .err suffix and background jobs.
        if HANDLE.search(json.dumps(inp, ensure_ascii=False)):
            return True
        return "json_path" in inp or inp.get("run_in_background") is True
    if topic == "symbol-navigation":
        # The guide teaches name-paths with `/`, kind filters, bodies by name, and the
        # structured navigation tools in place of grep.
        if not name.startswith(SERVER):
            return False
        tool = name[len(SERVER) :]
        if tool in ("references", "call_graph", "symbol_at"):
            return True
        if tool == "symbols":
            nm = inp.get("name")
            return (isinstance(nm, str) and "/" in nm) or "kind" in inp or inp.get("include_body") is True
        return False
    return None  # e.g. project-activation-bootstrap: delivered on the first call, whatever it is


def wilson(k: int, n: int, z: float = 1.96) -> tuple[float, float]:
    """95% Wilson interval for k hits in n trials. (0, 0) for an empty sample."""
    if n == 0:
        return (0.0, 0.0)
    p = k / n
    d = 1 + z * z / n
    centre = (p + z * z / (2 * n)) / d
    half = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / d
    return (max(0.0, centre - half), min(1.0, centre + half))


def read_uses(path: Path) -> tuple[str | None, list[tuple[int, str, dict]]]:
    """(session start date, [(line, tool name, input)]) for one transcript."""
    try:
        raw = path.read_bytes()
    except OSError:
        return None, []
    start: str | None = None
    uses: list[tuple[int, str, dict]] = []
    for idx, line in enumerate(raw.splitlines()):
        if not line.strip():
            continue
        try:
            d = json.loads(line)
        except (json.JSONDecodeError, UnicodeDecodeError):
            continue
        if start is None and d.get("timestamp"):
            start = str(d["timestamp"])[:10]
        if d.get("type") == "assistant":
            for b in probe.blocks(d.get("message")):
                if isinstance(b, dict) and b.get("type") == "tool_use":
                    inp = b.get("input") if isinstance(b.get("input"), dict) else {}
                    uses.append((idx, b.get("name", ""), inp))
    return start, uses


def score_events(events: list[dict], windows: tuple[int, ...] = (10, 50)) -> list[dict]:
    """Add `engaged_k<K>` to each census row. Only calls AFTER the delivery's line are read."""
    by_path: dict[str, list[dict]] = {}
    for ev in events:
        by_path.setdefault(ev["path"], []).append(ev)
    out: list[dict] = []
    for path, evs in by_path.items():
        _, uses = read_uses(Path(path))
        for ev in evs:
            after = [u for u in uses if u[0] > ev["line"]]
            row = dict(ev)
            for k in windows:
                flags = [engaged(ev["topic"], n, i) for (_, n, i) in after[:k]]
                row[f"engaged_k{k}"] = None if ev["topic"] not in ("progressive-disclosure", "symbol-navigation") else any(f is True for f in flags)
            out.append(row)
    return out


def comparison_group(
    paths: list[tuple[str, Path]], since: str, windows: tuple[int, ...] = (10, 50)
) -> list[dict]:
    """Subagents that called `symbols` and never received symbol-navigation.

    NOT a clean control. Most such subagents were seeded with the parent's ledger or resumed, so
    the guide was already in their context. Read it as "what do agents do without a fresh
    delivery", not as an untreated arm.
    """
    floor = dt.datetime.strptime(since, "%Y-%m-%d").timestamp()
    seen: set = set()
    out: list[dict] = []
    for profile, p in paths:
        if "/subagents/" not in str(p):
            continue
        try:
            if os.path.getmtime(p) < floor:
                continue
        except OSError:
            continue
        key = (p.parent.parent.name, p.stem)
        if key in seen:
            continue
        start, uses = read_uses(p)
        if not start or start < since:
            continue
        first = next((u for u in uses if u[1] == SERVER + "symbols"), None)
        if first is None:
            continue
        if any(r["topic"] == "symbol-navigation" and r["own_block"] for r in census.census_transcript(p, profile)):
            continue
        seen.add(key)
        after = [u for u in uses if u[0] > first[0]]
        row = {"session_id": key[0], "agent_id": key[1], "path": str(p)}
        for k in windows:
            row[f"engaged_k{k}"] = any(engaged("symbol-navigation", n, i) is True for (_, n, i) in after[:k])
        out.append(row)
    return out


def _cell(rows: list[dict], k: int) -> str:
    flags = [r[f"engaged_k{k}"] for r in rows]
    n = len(flags)
    hits = sum(1 for f in flags if f)
    lo, hi = wilson(hits, n)
    return f"{hits}/{n} = {100 * hits / n:.0f}% [{100 * lo:.0f}-{100 * hi:.0f}]" if n else "0/0"


def summarize(rows: list[dict], windows: tuple[int, ...] = (10, 50), comparison: list[dict] | None = None) -> str:
    lines = ["rate = events with the taught behaviour in the next K calls / events, 95% Wilson interval in brackets"]
    for topic in sorted({r["topic"] for r in rows}):
        rs = [r for r in rows if r["topic"] == topic]
        first = f"engaged_k{windows[0]}"
        if all(r.get(first) is None for r in rs):
            lines.append(f"{topic:<30} n={len(rs):<5} not scored (no honest mechanical predicate)")
            continue
        cells = "   ".join(f"K={k}: {_cell(rs, k)}" for k in windows)
        lines.append(f"{topic:<30} n={len(rs):<5} {cells}")
    if comparison is not None:
        cells = "   ".join(f"K={k}: {_cell(comparison, k)}" for k in windows)
        lines.append(f"{'symbol-navigation, NO delivery':<30} n={len(comparison):<5} {cells}   (not a clean control)")
    return "\n".join(lines)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--census", required=True, help="JSON lines from census_guide_deliveries.py")
    ap.add_argument("--windows", nargs="*", type=int, default=[10, 50])
    ap.add_argument("--kind", choices=("main", "subagent"), default="subagent")
    ap.add_argument("--comparison", action="store_true", help="also score subagents that got no delivery")
    ap.add_argument("--since", default=None, help="window start for --comparison (default: census minimum)")
    ap.add_argument("--out", default=None, help="write scored rows here")
    args = ap.parse_args()

    events = [json.loads(l) for l in Path(args.census).read_text(encoding="utf-8").splitlines() if l.strip()]
    events = [e for e in events if e["own_block"] and e["kind"] == args.kind]
    windows = tuple(args.windows)
    scored = score_events(events, windows)
    comp = None
    if args.comparison:
        since = args.since or min(e["session_start"] for e in events)
        comp = comparison_group(probe.iter_transcripts(list(probe.PROFILES), None), since, windows)
    if args.out:
        Path(args.out).write_text("\n".join(json.dumps(r, ensure_ascii=False) for r in scored) + "\n", encoding="utf-8")
    print(summarize(scored, windows, comp))
    return 0


if __name__ == "__main__":
    sys.exit(main())
