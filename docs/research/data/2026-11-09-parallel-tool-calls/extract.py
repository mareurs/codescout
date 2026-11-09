"""Stage 1: pull ordered tool calls out of transcripts. Read-only on the transcripts.

Output: one JSON line per record in OUT/<n>.jsonl, record kinds:
  {"k":"call", f, sid, i, mid, tu, name, inp, tr, res, err}
  {"k":"bound", f, i, ts, why}      real user prompt or compaction boundary
"""
import json, os, sys, glob, hashlib
from multiprocessing import Pool

HOME = os.path.expanduser("~")
PROFILES = [".claude", ".claude-sdd", ".claude-kat", ".claude-stef"]
SINCE = "2026-10-01"
OUT = sys.argv[1]
INP_CAP = 3000
RES_CAP = 4000


def files():
    import subprocess
    out = []
    for p in PROFILES:
        d = f"{HOME}/{p}/projects/-home-marius-work-claude-codescout"
        r = subprocess.run(["find", d, "-name", "*.jsonl", "-newermt", SINCE],
                           capture_output=True, text=True).stdout.split("\n")
        out += [x for x in r if x]
    return sorted(out)


def text_of(content):
    if isinstance(content, str):
        return content
    s = []
    for c in content or []:
        if isinstance(c, dict):
            if c.get("type") == "text":
                s.append(c.get("text", ""))
            elif c.get("type") == "tool_reference":
                s.append(str(c))
    return "\n".join(s)


def work(path):
    kind = "sub" if "/subagents/" in path else "main"
    fid = hashlib.sha1(path.encode()).hexdigest()[:10]
    calls = {}      # tool_use_id -> record
    order = []
    recs = []
    i = 0
    seen_ids = set()
    try:
        fh = open(path, encoding="utf-8", errors="replace")
    except OSError:
        return fid, path, kind, 0, []
    for line in fh:
        if '"type":"assistant"' not in line and '"type":"user"' not in line:
            continue
        if line.startswith('{"type":"api-request'):
            continue
        try:
            o = json.loads(line)
        except Exception:
            continue
        t = o.get("type")
        ts = o.get("timestamp")
        if t == "assistant":
            m = o.get("message") or {}
            for b in m.get("content") or []:
                if b.get("type") == "tool_use":
                    tid = b.get("id")
                    if tid in seen_ids:
                        continue
                    seen_ids.add(tid)
                    inp = json.dumps(b.get("input"), ensure_ascii=False)
                    rec = {"k": "call", "f": fid, "kind": kind, "i": i, "mid": m.get("id"), "id": tid,
                           "tu": ts, "name": b.get("name"), "inp": inp[:INP_CAP],
                           "inp_len": len(inp), "tr": None, "res": None, "err": False}
                    calls[tid] = rec
                    recs.append(rec)
                    i += 1
        elif t == "user":
            c = (o.get("message") or {}).get("content")
            has_res = False
            if isinstance(c, list):
                for b in c:
                    if b.get("type") == "tool_result":
                        has_res = True
                        r = calls.get(b.get("tool_use_id"))
                        if r is not None and r["tr"] is None:
                            r["tr"] = ts
                            r["res"] = text_of(b.get("content"))[:RES_CAP]
                            r["err"] = bool(b.get("is_error"))
            if not has_res and not o.get("isMeta"):
                txt = text_of(c)
                why = "compact" if "continued from a previous conversation" in txt[:400] else "prompt"
                recs.append({"k": "bound", "f": fid, "kind": kind, "i": i, "ts": ts, "why": why})
    return fid, path, kind, i, recs


if __name__ == "__main__":
    os.makedirs(OUT, exist_ok=True)
    fl = files()
    print("files", len(fl), file=sys.stderr)
    idx = []
    with Pool(48) as pool, open(f"{OUT}/all.jsonl", "w") as w:
        for fid, path, kind, n, recs in pool.imap_unordered(work, fl, chunksize=2):
            idx.append({"f": fid, "path": path, "kind": kind, "calls": n})
            for r in recs:
                w.write(json.dumps(r, ensure_ascii=False) + "\n")
    json.dump(idx, open(f"{OUT}/index.json", "w"))
    print("done", sum(x["calls"] for x in idx), "calls", file=sys.stderr)
