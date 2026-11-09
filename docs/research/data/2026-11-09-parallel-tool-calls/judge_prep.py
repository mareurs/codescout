import json, random, collections
random.seed(23)
P = json.load(open("pairs.json"))
ind = [r for r in P if r["cat"] == "indep"]
dep = [r for r in P if r["cat"] == "dep"]
# stratify indep by (a_cls,b_cls) head classes proportional-ish but cap per stratum
by = collections.defaultdict(list)
for r in ind:
    by[(",".join(r["a_cls"]), ",".join(r["b_cls"]))].append(r)
pick = []
for k, v in sorted(by.items(), key=lambda kv: -len(kv[1])):
    n = max(1, round(90 * len(v) / len(ind)))
    random.shuffle(v); pick += v[:n]
random.shuffle(pick); pick = pick[:90]
random.shuffle(dep)
dsel = [r for r in dep if (r["why"] or "").split(":")[0] in ("failsig", "data")][:30]
allp = pick + dsel
random.shuffle(allp)
key = []
for i, r in enumerate(allp):
    key.append({"n": i + 1, "idx": r["idx"], "heur": r["cat"], "why": r.get("why"), "kind": r["kind"],
                "a": r["a_cls"], "b": r["b_cls"], "M": r["M"]})
json.dump(key, open("judge_key.json", "w"))
for j in range(4):
    json.dump([dict(r, **{"idx": r["idx"]}) for r in allp[j * 30:(j + 1) * 30]], open(f"judge_in_{j+1}.json", "w"))
print(len(pick), len(dsel))
