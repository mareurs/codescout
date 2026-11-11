#!/usr/bin/env python3
"""Extract Opus review subagents from Claude Code transcripts.

Read-only. Walks every profile's codescout project dir, finds subagent
transcripts whose meta.json says model=opus and whose description looks like a
review, and writes one JSON record per review: the brief the reviewer got and
the report it handed back.

Usage:
  python3 -I extract_reviews.py OUT_DIR [--limit N] [--model opus]

Writes OUT_DIR/reviews.jsonl (one record per review) and OUT_DIR/index.tsv.
The corpus holds transcript text: keep it in the scratchpad, never commit it.
"""
import glob
import json
import os
import re
import sys

PROJECT = "-home-marius-work-claude-codescout"
PROFILES = ["~/.claude", "~/.claude-sdd", "~/.claude-kat", "~/.claude-stef"]
REVIEW_RE = re.compile(r"review", re.I)


def text_of(content):
    if isinstance(content, str):
        return content
    out = []
    for b in content or []:
        if isinstance(b, dict) and b.get("type") == "text":
            out.append(b.get("text", ""))
    return "\n".join(out)


def parse_agent(path):
    brief, report, ts_first, ts_last, tools = None, None, None, None, 0
    last_text = None
    with open(path, encoding="utf-8", errors="replace") as fh:
        for line in fh:
            try:
                r = json.loads(line)
            except ValueError:
                continue
            ts = r.get("timestamp")
            if ts:
                ts_first = ts_first or ts
                ts_last = ts
            t = r.get("type")
            msg = r.get("message") or {}
            if t == "user" and brief is None:
                brief = text_of(msg.get("content"))
            elif t == "assistant":
                for b in msg.get("content") or []:
                    if b.get("type") == "tool_use":
                        tools += 1
                        if b.get("name") == "SubagentHandback":
                            report = (b.get("input") or {}).get("message")
                    elif b.get("type") == "text" and b.get("text", "").strip():
                        last_text = b["text"]
    return {
        "brief": brief or "",
        "report": report or last_text or "",
        "report_via": "handback" if report else "last_text",
        "ts_first": ts_first,
        "ts_last": ts_last,
        "tool_calls": tools,
    }


def main():
    args = sys.argv[1:]
    if not args:
        sys.exit(__doc__)
    out_dir = args[0]
    limit = None
    model = "opus"
    if "--limit" in args:
        limit = int(args[args.index("--limit") + 1])
    if "--model" in args:
        model = args[args.index("--model") + 1]
    os.makedirs(out_dir, exist_ok=True)
    recs = []
    for prof in PROFILES:
        base = os.path.expanduser(f"{prof}/projects/{PROJECT}")
        for meta_path in glob.glob(f"{base}/*/subagents/*.meta.json"):
            try:
                meta = json.load(open(meta_path))
            except ValueError:
                continue
            if meta.get("model") != model:
                continue
            if not REVIEW_RE.search(meta.get("description", "")):
                continue
            jsonl = meta_path[: -len(".meta.json")] + ".jsonl"
            if not os.path.exists(jsonl):
                continue
            rec = parse_agent(jsonl)
            rec.update(
                {
                    "agent_id": os.path.basename(jsonl)[len("agent-") : -len(".jsonl")],
                    "profile": prof,
                    "session": meta_path.split("/")[-3],
                    "description": meta.get("description", ""),
                    "finished": meta.get("finished"),
                }
            )
            recs.append(rec)
    recs.sort(key=lambda r: r["ts_first"] or "")
    if limit:
        recs = recs[:limit]
    with open(os.path.join(out_dir, "reviews.jsonl"), "w") as fh:
        for r in recs:
            fh.write(json.dumps(r) + "\n")
    with open(os.path.join(out_dir, "index.tsv"), "w") as fh:
        fh.write("agent_id\tprofile\tsession\tts_first\treport_chars\tvia\tdescription\n")
        for r in recs:
            fh.write(
                "\t".join(
                    [
                        r["agent_id"],
                        r["profile"],
                        r["session"],
                        r["ts_first"] or "",
                        str(len(r["report"])),
                        r["report_via"],
                        r["description"],
                    ]
                )
                + "\n"
            )
    empty = sum(1 for r in recs if not r["report"])
    print(f"{len(recs)} reviews, {empty} with no report")


if __name__ == "__main__":
    main()
