import json, sys, collections
sample = json.load(open(sys.argv[1]))
need = collections.defaultdict(list)
for r in sample:
    need[r["idx"][0]].append(r["idx"][1])
rounds_of = {}
for l in open("out/all.jsonl"):
    r = json.loads(l)
    if r["f"] in need:
        rounds_of.setdefault(r["f"], []).append(r)

def build(evs):
    rounds = []; pend = True
    for r in evs:
        if r["k"] == "bound":
            pend = True; continue
        if rounds and rounds[-1]["mid"] == r["mid"] and not pend:
            rounds[-1]["calls"].append(r)
        else:
            rounds.append({"mid": r["mid"], "calls": [r], "bound": pend})
        pend = False
    return rounds

def fmt(c, n_in, n_res):
    s = c["name"].replace("mcp__codescout__", "") + " " + c["inp"][:n_in].replace("\n", " ")
    if n_res:
        s += "\n      RESULT: " + (c["res"] or "")[:n_res].replace("\n", " ")
    return s

cache = {f: build(e) for f, e in rounds_of.items()}
for n, r in enumerate(sample, 1):
    f, k = r["idx"]
    a, b = cache[f][k], cache[f][k + 1]
    print(f"=== PAIR {n}")
    for c in a["calls"]:
        print("  FIRST:", fmt(c, 450, 450))
    for c in b["calls"]:
        print("  NEXT :", fmt(c, 450, 0))
