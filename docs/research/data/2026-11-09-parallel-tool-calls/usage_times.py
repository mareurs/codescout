"""Stage 2a: do calls issued in ONE assistant message overlap in time?
Source of truth for timing: usage.db (started_at + latency_ms per tool_use_id)."""
import json, sqlite3, collections, datetime

db = sqlite3.connect("file:/home/marius/work/claude/codescout/.codescout/usage.db?mode=ro", uri=True)
U = {}
for tid, name, st, lat in db.execute(
        "select tool_use_id, tool_name, started_at, latency_ms from tool_calls "
        "where tool_use_id is not null and started_at is not null"):
    try:
        t0 = datetime.datetime.fromisoformat(st.replace("Z", "")).replace(
            tzinfo=datetime.timezone.utc).timestamp()
    except Exception:
        continue
    U[tid] = (name, t0, t0 + lat / 1000.0)
print("usage rows joinable:", len(U))

rounds = collections.defaultdict(list)
joined = 0
total_calls = 0
for l in open("out/all.jsonl"):
    r = json.loads(l)
    if r["k"] != "call":
        continue
    total_calls += 1
    rounds[(r["f"], r["mid"])].append(r)
    if r["id"] in U:
        joined += 1
print("transcript calls:", total_calls, " joined to usage.db:", joined)

size = collections.Counter(len(v) for v in rounds.values())
print("round sizes:", sorted(size.items())[:12])
multi_rounds = sum(c for s, c in size.items() if s > 1)
print("rounds with >=2 calls:", multi_rounds, "of", len(rounds))

# overlap analysis on rounds with >=2 usage-joined calls
stats = collections.defaultdict(lambda: [0, 0])   # key -> [overlap, serial]
ex = []
for key, calls in rounds.items():
    js = [(c["name"], *U[c["id"]][1:]) for c in calls if c["id"] in U]
    if len(js) < 2:
        continue
    js.sort(key=lambda x: x[1])
    for a, b in zip(js, js[1:]):
        if a[2] - a[1] < 0.3:       # first call too short to tell serial from parallel
            continue
        overlap = b[1] < a[2] - 0.05      # b started before a ended (50 ms slack)
        k = (a[0].replace("mcp__codescout__", ""), b[0].replace("mcp__codescout__", ""))
        stats[k][0 if overlap else 1] += 1
        if len(ex) < 3 and not overlap:
            ex.append((k, round(b[1] - a[2], 3)))
print("\npair (first,second): overlapped / serial   [consecutive calls inside one message]")
for k, (o, s) in sorted(stats.items(), key=lambda kv: -(kv[1][0] + kv[1][1]))[:30]:
    print(f"{k[0]:>12} -> {k[1]:<12} {o:>5} / {s:<5}")
tot_o = sum(v[0] for v in stats.values()); tot_s = sum(v[1] for v in stats.values())
print("TOTAL overlapped", tot_o, "serial", tot_s)
