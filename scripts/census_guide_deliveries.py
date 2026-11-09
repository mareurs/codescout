#!/usr/bin/env python3
"""One row per guide delivery in a Claude Code transcript.

Why this exists: docs/issues/2026-08-27-guide-topics-are-atomic-nodes-in-an-unmodelled-graph.md.
scripts/probe_guide_section_use.py scores a topic as if it were delivered whole, so its
"never engaged" share is an upper bound. This census counts what was delivered: each injected
block (a whole topic, one section, a preamble, a subagent's pointer body, or an oversize pointer) with its real byte size,
the call that triggered it, and what the session did next.

It measures delivery, not use. Whether a later action depended on a delivered section is a
judgement about the transcript. This script prepares the rows that judgement reads.

Reads transcripts only. Writes JSON lines to stdout or --out, and a summary to stderr.
The window is capped at 30 days at the operator's instruction (--allow-older lifts the cap).
"""

from __future__ import annotations

import argparse
import collections
import datetime as dt
import importlib.util
import json
import os
import re
import sys
from pathlib import Path

_HERE = Path(__file__).resolve().parent


def _load_probe():
    spec = importlib.util.spec_from_file_location(
        "probe_guide_section_use", _HERE / "probe_guide_section_use.py"
    )
    mod = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = mod
    spec.loader.exec_module(mod)
    return mod


probe = _load_probe()
TOPICS = set(probe.TOPICS)

# The four opening forms the emitter writes (src/tools/core/guide_emit.rs). The heading part
# may not cross `-->` or `<!--`, and is lazy so a heading containing ` — ` is kept whole.
OPEN = re.compile(
    r"<!-- auto-injected get_guide\('([a-z][a-z-]*)'\)"
    r"(?: § ((?:(?!-->|<!--)[^\n])*?) — first call"
    r"| — first call"
    r"| subagent — first call"
    r"| preamble — no section)"
)
POINTER = re.compile(r"<!-- get_guide\('([a-z][a-z-]*)'\) NOT auto-injected:[^\n]*?-->")

MAX_WINDOW_DAYS = 30


def _result_items(block: dict) -> tuple[list[str], bool]:
    """The text items of one tool_result, and whether item boundaries are known.

    The server emits each delivery as its own content item, so a list result lets us tell a
    delivery from a marker quoted inside another item's text. A plain-string result has no item
    boundaries, so nothing in it can be proven a delivery.
    """
    content = block.get("content")
    if isinstance(content, str):
        return [content], False
    if isinstance(content, list):
        return [x.get("text", "") for x in content if isinstance(x, dict)], True
    return [], False


def _deliveries_in(text: str, provable: bool = True, with_text: bool = False) -> list[dict]:
    """Every block in one result item, in order, each with the bytes it really occupies.

    `own_block` is true only when item boundaries are known and the block opens the item. A
    marker in the middle of an item is a quotation (a grep, a cat, a doc), not a delivery.
    `with_text` adds the block's own text, for the layer-2 reader packets.
    """
    found: list[tuple[int, dict]] = []
    for m in OPEN.finditer(text):
        topic = m.group(1)
        if topic not in TOPICS:
            continue
        heading = m.group(2)
        if heading is not None:
            form, close = "section", f"<!-- end auto-injected get_guide('{topic}') § {heading} -->"
        elif "preamble" in m.group(0):
            form, close = "preamble", f"<!-- end auto-injected get_guide('{topic}') preamble -->"
        elif m.group(0).endswith("') subagent — first call"):
            form, close = "subagent", f"<!-- end auto-injected get_guide('{topic}') subagent -->"
        else:
            form, close = "whole", f"<!-- end auto-injected get_guide('{topic}') -->"
        at = text.find(close, m.end())
        closed = at != -1
        end = at + len(close) if closed else len(text)
        d = {
            "topic": topic,
            "form": form,
            "heading": heading,
            "bytes": len(text[m.start() : end].encode("utf-8")),
            "closed": closed,
            "own_block": provable and not text[: m.start()].strip(),
        }
        if with_text:
            d["text"] = text[m.start() : end]
        found.append((m.start(), d))
    for m in POINTER.finditer(text):
        if m.group(1) in TOPICS:
            d = {
                "topic": m.group(1),
                "form": "pointer",
                "heading": None,
                "bytes": len(m.group(0).encode("utf-8")),
                "closed": True,
                "own_block": provable and not text[: m.start()].strip(),
            }
            if with_text:
                d["text"] = m.group(0)
            found.append((m.start(), d))
    return [d for _, d in sorted(found, key=lambda x: x[0])]


def census_transcript(path: Path, profile: str = "?") -> list[dict]:
    """Rows for one transcript file. Empty when it holds no delivery."""
    try:
        raw = path.read_bytes()
    except OSError:
        return []
    if b"auto-injected" not in raw:  # ASCII, so it survives a —-escaped file too
        return []

    is_sub = "/subagents/" in str(path)
    calls: dict[str, tuple[str, dict]] = {}
    tool_lines: list[tuple[int, str]] = []
    pending: list[dict] = []
    session_start: str | None = None

    for idx, line in enumerate(raw.splitlines()):
        if not line.strip():
            continue
        try:
            d = json.loads(line)
        except (json.JSONDecodeError, UnicodeDecodeError):
            continue
        ts = d.get("timestamp")
        if session_start is None and ts:
            session_start = str(ts)[:10]
        typ = d.get("type")
        if typ == "assistant":
            for b in probe.blocks(d.get("message")):
                if isinstance(b, dict) and b.get("type") == "tool_use":
                    name = b.get("name", "")
                    inp = b.get("input") if isinstance(b.get("input"), dict) else {}
                    calls[b.get("id", "")] = (name, inp)
                    tool_lines.append((idx, name))
        elif typ == "user":
            for b in probe.blocks(d.get("message")):
                if not (isinstance(b, dict) and b.get("type") == "tool_result"):
                    continue
                name, inp = calls.get(b.get("tool_use_id", ""), ("", {}))
                items, provable = _result_items(b)
                for dv in (d for it in items for d in _deliveries_in(it, provable)):
                    action = inp.get("action")
                    pending.append(
                        {
                            **dv,
                            "line": idx,
                            "ts": ts,
                            "trigger_tool": name or None,
                            "trigger_action": action if isinstance(action, str) else None,
                            "trigger_input": json.dumps(
                                probe.strip_prose(inp), ensure_ascii=False
                            )[:300],
                        }
                    )

    if not pending:
        return []
    if is_sub:
        session_id = path.parent.parent.name
        stem = path.stem
        agent_id = stem[len("agent-") :] if stem.startswith("agent-") else stem
        project = path.parent.parent.parent.name
    else:
        session_id, agent_id, project = path.stem, None, path.parent.name
    total = len(tool_lines)
    rows = []
    for ev in pending:
        after = [n for (i, n) in tool_lines if i > ev["line"]]
        rows.append(
            {
                "profile": profile,
                "project": project,
                "kind": "subagent" if is_sub else "main",
                "session_id": session_id,
                "agent_id": agent_id,
                "session_start": session_start or "?",
                "path": str(path),
                **ev,
                "calls_after": len(after),
                "next_tools": after[:3],
                "total_calls": total,
            }
        )
    return rows


def collect_rows(paths: list[tuple[str, Path]], since: str) -> list[dict]:
    """Rows for every transcript whose SESSION started on or after `since` (YYYY-MM-DD)."""
    floor = dt.datetime.strptime(since, "%Y-%m-%d").timestamp()
    out: list[dict] = []
    for profile, p in paths:
        try:
            if os.path.getmtime(p) < floor:  # cheap reject: a session cannot end before it starts
                continue
        except OSError:
            continue
        out.extend(r for r in census_transcript(p, profile) if r["session_start"] >= since)
    return out


def dedupe_copies(rows: list[dict]) -> tuple[list[dict], int]:
    """Drop rows that are the same event seen in another copy of the same transcript.

    One session is often present in two or three profile directories (it was resumed or copied
    across them). Measured 2026-11-07: 316 of 1,298 subagent transcripts, 23% of subagent bytes.
    Two real deliveries in one transcript differ in `ts`, so they are both kept.
    """
    seen: set = set()
    kept: list[dict] = []
    for r in rows:
        key = (
            r["session_id"], r["agent_id"], r["ts"], r["topic"], r["heading"],
            r["form"], r["bytes"], r["trigger_tool"],
        )
        if key in seen:
            continue
        seen.add(key)
        kept.append(r)
    return kept, len(rows) - len(kept)


def census_paths(paths: list[tuple[str, Path]], since: str) -> list[dict]:
    """Rows for every transcript started on or after `since`, each event counted once."""
    return dedupe_copies(collect_rows(paths, since))[0]



def summarize(rows: list[dict]) -> str:
    """Deliveries by kind, form and topic. Quoted or unproven rows are counted apart."""
    delivered = [r for r in rows if r.get("own_block")]
    other = [r for r in rows if not r.get("own_block")]
    by = collections.defaultdict(lambda: [0, 0])
    sessions: dict[str, set] = collections.defaultdict(set)
    for r in delivered:
        k = (r["kind"], r["form"], r["topic"])
        by[k][0] += 1
        by[k][1] += r["bytes"]
        sessions[r["kind"]].add((r["session_id"], r["agent_id"]))
    lines = [f"delivered rows: {len(delivered)}"]
    for kind in ("main", "subagent"):
        lines.append(f"{kind}: {len(sessions[kind])} transcripts with a delivery")
    lines.append("kind      form      topic                         rows       bytes")
    for (kind, form, topic), (n, b) in sorted(by.items(), key=lambda kv: (kv[0][0], -kv[1][1])):
        lines.append(f"{kind:<9} {form:<9} {topic:<28} {n:>6} {b:>11,}")
    lines.append(
        f"quoted or unproven: {len(other)} rows, {sum(r['bytes'] for r in other)} bytes"
    )
    return "\n".join(lines)


def main() -> int:
    today = dt.date.today()
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--since", default=(today - dt.timedelta(days=MAX_WINDOW_DAYS)).isoformat())
    ap.add_argument("--allow-older", action="store_true", help="lift the 30-day cap")
    ap.add_argument("--profiles", nargs="*", default=list(probe.PROFILES))
    ap.add_argument("--roots", nargs="*", default=None)
    ap.add_argument("--kind", choices=("main", "subagent", "all"), default="all")
    ap.add_argument("--out", default=None, help="write JSON lines here instead of stdout")
    args = ap.parse_args()

    earliest = today - dt.timedelta(days=MAX_WINDOW_DAYS)
    if dt.date.fromisoformat(args.since) < earliest and not args.allow_older:
        print(
            f"refusing --since {args.since}: the window is capped at {MAX_WINDOW_DAYS} days "
            f"(earliest {earliest}). Pass --allow-older to lift the cap.",
            file=sys.stderr,
        )
        return 2

    paths = probe.iter_transcripts(args.profiles, args.roots)
    rows, dropped = dedupe_copies(collect_rows(paths, args.since))
    if args.kind != "all":
        rows = [r for r in rows if r["kind"] == args.kind]
    text = "\n".join(json.dumps(r, ensure_ascii=False) for r in rows)
    if args.out:
        Path(args.out).write_text(text + ("\n" if text else ""), encoding="utf-8")
    elif text:
        print(text)
    print(f"window: sessions started on or after {args.since}; transcripts scanned: {len(paths)}; duplicate copies dropped: {dropped}", file=sys.stderr)
    print(summarize(rows), file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
