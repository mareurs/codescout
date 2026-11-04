"""Observational follow-through of in-band hints, from .codescout/usage.db (read-only).

For each delivered hint of three classes, look at the next K calls of the same session+agent:
  verify callers (edit_code):  strict = a references/call_graph call whose input names the symbol;
                               loose  = also a grep/symbols call naming it. Baseline: edit_code calls WITHOUT that hint.
  buffer pointer (@tool_*):    followed = a later call whose input names the handle.
  job started (@bg_*):         followed = a later call whose input names the handle.
This is observational: hint and no-hint calls differ in more than the hint. It gives the in-band rate only.
"""
import collections
import json
import re
import sqlite3
import sys

DB = sys.argv[1]
K = {"callers": 8, "buffer": 6, "job": 15}
con = sqlite3.connect(f"file:{DB}?mode=ro", uri=True)
rows = con.execute(
    "select id, tool_name, coalesce(cc_session_id, session_id, ''), coalesce(agent_id,''), input_json, output_json "
    "from tool_calls order by id").fetchall()

groups = collections.defaultdict(list)
for r in rows:
    groups[(r[2], r[3])].append(r)

def env_of(oj):
    try:
        blocks = json.loads(oj) if oj else []
        for b in blocks:
            t = b.get("text") if isinstance(b, dict) else None
            if isinstance(t, str):
                try:
                    e = json.loads(t)
                except ValueError:
                    continue
                if isinstance(e, dict):
                    return e
    except ValueError:
        pass
    return {}

def inp(ij):
    try:
        d = json.loads(ij) if ij else {}
        return d if isinstance(d, dict) else {}
    except ValueError:
        return {}

res = collections.Counter()
for key, calls in groups.items():
    for i, (cid, tool, sess, agent, ij, oj) in enumerate(calls):
        if not oj:
            continue
        env = env_of(oj)
        hint = env.get("hint") if isinstance(env.get("hint"), str) else ""
        later = calls[i + 1:]
        if tool == "edit_code":
            sym = (inp(ij).get("symbol") or "").split("/")[-1]
            if not sym or inp(ij).get("action") not in ("replace", "insert", "remove", "rename"):
                continue
            hinted = hint.startswith("verify callers:")
            strict = loose = False
            for (_, t2, _, _, ij2, _) in later[:K["callers"]]:
                if sym in (ij2 or ""):
                    if t2 in ("references", "call_graph"):
                        strict = True
                    if t2 in ("references", "call_graph", "grep", "symbols"):
                        loose = True
            b = "callers/hinted" if hinted else "callers/unhinted"
            res[(b, "n")] += 1
            res[(b, "strict")] += strict
            res[(b, "loose")] += loose
        m = re.search(r"@tool_[0-9a-f]+", hint) if hint.startswith("read_file(") else None
        if m:
            h = m.group(0)
            f = any(h in (x[4] or "") for x in later[:K["buffer"]])
            res[("buffer", "n")] += 1
            res[("buffer", "followed")] += f
        m = re.search(r"@bg_[0-9a-f]+", hint) if hint.startswith("Started; outcome") else None
        if m:
            h = m.group(0)
            f = any(h in (x[4] or "") for x in later[:K["job"]])
            res[("job", "n")] += 1
            res[("job", "followed")] += f
            res[("job", "has_later_calls")] += bool(later)

for k in sorted(res):
    print(k, res[k])
