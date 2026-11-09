"""Stage 2b: overlap rate over time for read-only pairs (to separate server/harness change from tool behaviour)."""
import json, sqlite3, collections, datetime

db = sqlite3.connect("file:/home/marius/work/claude/codescout/.codescout/usage.db?mode=ro", uri=True)
U = {}
for tid, name, st, lat, sha in db.execute(
        "select tool_use_id, tool_name, started_at, latency_ms, codescout_sha from tool_calls "
        "where tool_use_id is not null and started_at is not null"):
    try:
        t0 = datetime.datetime.fromisoformat(st.replace("Z", "")).replace(
            tzinfo=datetime.timezone.utc).timestamp()
    except Exception:
        continue
    U[tid] = (name, t0, t0 + lat / 1000.0, st[:10], sha)

rounds = collections.defaultdict(list)
for l in open("out/all.jsonl"):
    r = json.loads(l)
    if r["k"] == "call" and r["id"] in U:
        rounds[(r["f"], r["mid"])].append(r)

want = {("grep", "grep"), ("symbols", "symbols"), ("read_file", "read_file"), ("grep", "symbols"),
        ("symbols", "grep"), ("grep", "read_file")}
by_day = collections.defaultdict(lambda: collections.defaultdict(lambda: [0, 0]))
by_sha = collections.defaultdict(lambda: [0, 0])
for calls in rounds.values():
    js = sorted(((c["name"].replace("mcp__codescout__", ""), *U[c["id"]][1:]) for c in calls),
                key=lambda x: x[1])
    for a, b in zip(js, js[1:]):
        k = (a[0], b[0])
        if k not in want:
            continue
        ov = b[1] < a[2] - 0.05
        by_day[b[3]][k][0 if ov else 1] += 1
        if k == ("grep", "grep"):
            by_sha[b[4]][0 if ov else 1] += 1
days = sorted(by_day)
print("day         grep>grep   sym>sym   read>read   grep>sym   sym>grep   grep>read   (overlap/serial)")
for d in days:
    row = by_day[d]
    f = lambda k: "%d/%d" % tuple(row[k]) if k in row else "-"
    print(d, f(("grep","grep")).rjust(9), f(("symbols","symbols")).rjust(9), f(("read_file","read_file")).rjust(10),
          f(("grep","symbols")).rjust(9), f(("symbols","grep")).rjust(9), f(("grep","read_file")).rjust(10))
print("\ngrep>grep by codescout_sha (top):")
for s, (o, ss) in sorted(by_sha.items(), key=lambda kv: -sum(kv[1]))[:12]:
    print(s, o, ss)
