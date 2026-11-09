import json, collections, statistics, random
random.seed(11)
P = json.load(open("pairs.json")); S = json.load(open("segs.json"))
CAP = 120.0

def pct(a, b): return "%.1f%%" % (100.0 * a / b) if b else "-"
def q(xs, p):
    xs = sorted(xs); return xs[min(len(xs) - 1, int(p * len(xs)))] if xs else 0

print("adjacent round pairs:", len(P))
c = collections.Counter(r["cat"] for r in P)
print(dict(c))
real = [r for r in P if r["cat"] in ("dep", "indep")]
print("excl. barrier:", len(real), " independent:", c["indep"], pct(c["indep"], len(real)))
for kind in ("main", "sub"):
    rr = [r for r in real if r["kind"] == kind]
    ind = [r for r in rr if r["cat"] == "indep"]
    print(f"  {kind}: pairs {len(rr)} indep {len(ind)} {pct(len(ind), len(rr))}")

print("\nwhy dependent:")
w = collections.Counter((r["why"] or "").split(":")[0] for r in real if r["cat"] == "dep")
for k, v in w.most_common(): print("  ", k, v, pct(v, c["dep"]))

ind = [r for r in real if r["cat"] == "indep"]
M = [min(r["M"], CAP) for r in ind]
allM = [min(r["M"], CAP) for r in real]
print("\nmodel latency M between result and next call (s), capped at %ds" % CAP)
print("  independent: n=%d median %.1f mean %.1f p90 %.1f sum %.0fs (%.1f h)" % (
    len(M), statistics.median(M), statistics.mean(M), q(M, .9), sum(M), sum(M) / 3600))
print("  all pairs  : median %.1f mean %.1f   sum %.0fs (%.1f h)" % (
    statistics.median(allM), statistics.mean(allM), sum(allM), sum(allM) / 3600))
print("  share of all inter-round time that is an independent pair's M:", pct(sum(M), sum(allM)))

print("\nindependent pairs by class (a -> b):")
cc = collections.Counter((",".join(r["a_cls"]), ",".join(r["b_cls"])) for r in ind)
for k, v in cc.most_common(14):
    mm = [min(r["M"], CAP) for r in ind if (",".join(r["a_cls"]), ",".join(r["b_cls"])) == k]
    print("  %-18s -> %-18s %6d  %5s  median M %.1fs" % (k[0], k[1], v, pct(v, len(ind)), statistics.median(mm)))
print("  read-only -> read-only (also execute concurrently):", sum(r["ro_ro"] for r in ind), pct(sum(r["ro_ro"] for r in ind), len(ind)))
print("  next round irreversible (git commit/push/rm...):", sum(r["b_irrev"] for r in ind))

print("\nindependent pairs by tool (a -> b), top 20:")
tt = collections.Counter((r["a_names"][-1], r["b_names"][0]) for r in ind)
for k, v in tt.most_common(20): print("  %-14s -> %-14s %5d" % (k[0], k[1], v))

print("\nsingle-call rounds followed by single-call round, independent:",
      sum(1 for r in ind if r["a_n"] == 1 and r["b_n"] == 1))

print("\nsegment leveling (rounds with no user prompt between):")
A = sum(s["actual"] for s in S); I = sum(s["ideal"] for s in S)
print("  segments", len(S), " actual rounds", A, " ideal rounds", I, " removable", A - I, pct(A - I, A))
med = statistics.median(allM)
print("  removable rounds x median M (%.1fs) = %.0fs = %.1f h" % (
    statistics.median(M), (A - I) * statistics.median(M), (A - I) * statistics.median(M) / 3600))
wall = sum(min(s["wall"], 3600 * 3) for s in S)
print("  total segment wall time (each segment capped 3 h): %.0f h" % (wall / 3600))
print("  estimated saving as share of wall: %s" % pct((A - I) * statistics.median(M), wall))
for kind in ("main", "sub"):
    ss = [s for s in S if s["kind"] == kind]
    a = sum(s["actual"] for s in ss); i = sum(s["ideal"] for s in ss)
    wl = sum(min(s["wall"], 3600 * 3) for s in ss)
    print(f"  {kind}: segments {len(ss)} actual {a} ideal {i} removable {pct(a - i, a)}  wall {wl/3600:.0f} h  saving/wall {pct((a-i)*statistics.median(M), wl)}")

# sample for manual verification
random.shuffle(ind)
json.dump(ind[:60], open("indep_sample.json", "w"))
dep = [r for r in real if r["cat"] == "dep" and (r["why"] or "").startswith("data")]
random.shuffle(dep)
json.dump(dep[:40], open("dep_sample.json", "w"))
