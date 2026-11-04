"""Read-only census of the hints codescout delivered, from .codescout/usage.db (tool_calls.output_json).

Counts, per tool and per hint template: envelope fields `hint`, `_guide_hint`, `next`; auto-injected guide
blocks (a later content block that opens with `<!-- auto-injected get_guide`); and outcome/error hints.
Writes census.json next to this file. Never writes to the database.
"""
import collections
import json
import re
import sqlite3
import sys
from pathlib import Path

DB = sys.argv[1]
OUT = Path(sys.argv[2])
FIELDS = ("hint", "_guide_hint", "next")

def norm(s):
    s = re.sub(r"@[a-z]+_[0-9a-f]+(\.err)?", "@REF", s)
    s = re.sub(r"\$[\w.\[\]*]+", "$JP", s)
    s = re.sub(r'"[^"]{0,90}"', '"X"', s)
    s = re.sub(r"'[^']{0,60}'", "'X'", s)
    s = re.sub(r"[\w./-]+\.(rs|md|py|toml|json|mjs|sh|jsonl|kt|ts|tsx|js)\b", "PATH", s)
    s = re.sub(r"\b[0-9a-f]{8,}\b", "HEX", s)
    s = re.sub(r"\d+", "N", s)
    s = re.sub(r"`[^`]{1,60}`", "`X`", s)
    return s[:150]

con = sqlite3.connect(f"file:{DB}?mode=ro", uri=True)
cur = con.execute("select tool_name, outcome, err_family, error_msg, output_json from tool_calls")
calls = collections.Counter()
with_field = collections.defaultdict(collections.Counter)      # tool -> field -> calls
templ = collections.defaultdict(collections.Counter)           # field -> template -> n
templ_tools = collections.defaultdict(lambda: collections.defaultdict(set))
guide_blocks = collections.Counter()                           # tool -> calls with an auto-injected guide block
guide_topics = collections.Counter()
errors = collections.Counter()
err_with_hint = collections.Counter()
unparsed = 0

for tool, outcome, fam, emsg, oj in cur:
    calls[tool] += 1
    if outcome and outcome != "ok":
        errors[(tool, outcome)] += 1
        if emsg and ("hint" in emsg.lower() or "try " in emsg.lower() or "use " in emsg.lower()):
            err_with_hint[(tool, outcome)] += 1
    if not oj:
        continue
    try:
        blocks = json.loads(oj)
    except ValueError:
        unparsed += 1
        continue
    if not isinstance(blocks, list):
        continue
    seen = set()
    for i, b in enumerate(blocks):
        text = b.get("text") if isinstance(b, dict) else None
        if not isinstance(text, str):
            continue
        if i > 0 and text.lstrip().startswith("<!-- auto-injected get_guide"):
            m = re.search(r"get_guide\('([^']+)'", text)
            guide_topics[m.group(1) if m else "?"] += 1
            if "guide" not in seen:
                guide_blocks[tool] += 1
                seen.add("guide")
        try:
            env = json.loads(text)
        except ValueError:
            continue
        if not isinstance(env, dict):
            continue
        for f in FIELDS:
            v = env.get(f)
            if isinstance(v, str) and v:
                if f not in seen:
                    with_field[tool][f] += 1
                    seen.add(f)
                t = norm(v)
                templ[f][t] += 1
                templ_tools[f][t].add(tool)

res = {
    "calls": sum(calls.values()),
    "unparsed_output_json": unparsed,
    "per_tool": {t: {"calls": calls[t], **dict(with_field[t]), "guide_block": guide_blocks[t]} for t in calls},
    "guide_topics": guide_topics.most_common(),
    "errors": [[t, o, n, err_with_hint[(t, o)]] for (t, o), n in errors.most_common()],
    "templates": {f: [[t, n, sorted(templ_tools[f][t])] for t, n in templ[f].most_common(40)] for f in FIELDS},
    "distinct_templates": {f: len(templ[f]) for f in FIELDS},
}
OUT.write_text(json.dumps(res, indent=1, ensure_ascii=False))
print("calls", res["calls"], "unparsed", unparsed)
for f in FIELDS:
    print(f, "distinct templates", res["distinct_templates"][f], "deliveries", sum(templ[f].values()))
print("guide blocks", sum(guide_blocks.values()), "topics", len(guide_topics))
