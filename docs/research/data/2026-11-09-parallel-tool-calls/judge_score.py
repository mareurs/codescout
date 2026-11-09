import json, collections, math
key = json.load(open("judge_key.json"))
V = {}
for j in range(1, 5):
    for o in json.load(open(f"judge_out_{j}.json")):
        V[(j - 1) * 30 + o["n"]] = o
assert len(V) == 120, len(V)
def wilson(k, n, z=1.96):
    if not n: return (0, 0)
    p = k / n; d = 1 + z * z / n
    c = (p + z * z / (2 * n)) / d; h = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / d
    return (c - h, c + h)
for heur in ("indep", "dep"):
    rows = [(k, V[k["n"]]) for k in key if k["heur"] == heur]
    ok = sum(1 for k, v in rows if v["verdict"] == "SAME_MESSAGE_OK")
    uns = sum(1 for k, v in rows if "UNSURE" in v["reason"].upper())
    lo, hi = wilson(ok, len(rows))
    print(f"heuristic={heur}: n={len(rows)} judged SAME_MESSAGE_OK={ok} ({100*ok/len(rows):.0f}%, 95% CI {100*lo:.0f}-{100*hi:.0f}%) unsure={uns}")
rows = [(k, V[k["n"]]) for k in key if k["heur"] == "indep"]
print("\nby (a,b) class, heuristic-independent:")
by = collections.defaultdict(lambda: [0, 0])
for k, v in rows:
    c = ("RO" if k["a"] == ["RO"] else "other") + "->" + ("RO" if k["b"] == ["RO"] else "other")
    by[c][1] += 1; by[c][0] += v["verdict"] == "SAME_MESSAGE_OK"
for c, (o, n) in sorted(by.items()): print(f"  {c:12} {o}/{n} = {100*o/n:.0f}%")
print("\nby heuristic dep reason:")
by = collections.defaultdict(lambda: [0, 0])
for k, v in [(k, V[k["n"]]) for k in key if k["heur"] == "dep"]:
    w = (k["why"] or "").split(":")[0]
    by[w][1] += 1; by[w][0] += v["verdict"] == "SAME_MESSAGE_OK"
for c, (o, n) in by.items(): print(f"  {c:8} judged OK anyway {o}/{n}")
# calibrated estimate
P = json.load(open("pairs.json"))
real = [r for r in P if r["cat"] in ("dep", "indep")]
ni = sum(r["cat"] == "indep" for r in real); nd = len(real) - ni
pi = sum(1 for k, v in rows if v["verdict"] == "SAME_MESSAGE_OK") / len(rows)
drows = [(k, V[k["n"]]) for k in key if k["heur"] == "dep"]
pd = sum(1 for k, v in drows if v["verdict"] == "SAME_MESSAGE_OK") / len(drows)
est = (ni * pi + nd * pd) / len(real)
print(f"\ncalibrated share of adjacent pairs collapsible: {100*est:.1f}%  (indep {ni}*{pi:.2f} + dep sample {nd}*{pd:.2f}; dep sample covers only failsig/data reasons)")
print("print examples of judged-OK among heuristic-indep, by median M:")
import statistics
mm = [k["M"] for k, v in rows if v["verdict"] == "SAME_MESSAGE_OK"]
print("  median M of judged-OK pairs: %.1fs  n=%d" % (statistics.median(mm), len(mm)))
