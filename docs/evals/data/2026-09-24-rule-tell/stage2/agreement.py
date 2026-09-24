"""Agreement between two label sets over a sample: collapsed-label kappa, binary kappa, exact-rule agreement, every disagreement.

    python3 agreement.py <labels_a.jsonl> <labels_b.jsonl>   (a = the sample's labeller, b = the agent labels)

Binary is the label that decides what enters T: a rule ("violation") against every dropped label
(not-a-violation, not-a-pair, unsure).
"""
import collections, json, sys


def collapse(l):
    return l if l in ("not-a-violation", "not-a-pair", "unsure") else "violation"


def binary(l):
    return "violation" if collapse(l) == "violation" else "dropped"


def load(p):
    return {r["id"]: r for r in map(json.loads, open(p).read().splitlines()) if r}


def kappa(pairs):
    n = len(pairs)
    po = sum(x == y for x, y in pairs) / n
    ca, cb = collections.Counter(x for x, _ in pairs), collections.Counter(y for _, y in pairs)
    pe = sum(ca[k] * cb[k] for k in set(ca) | set(cb)) / n ** 2
    return po, (po - pe) / (1 - pe) if pe < 1 else float("nan"), pe, ca, cb


a, b = load(sys.argv[1]), load(sys.argv[2])
ids = sorted(a)
assert set(ids) <= b.keys(), "sample ids missing from the agent labels"
n = len(ids)
po, k, pe, ca, cb = kappa([(collapse(a[i]["label"]), collapse(b[i]["label"])) for i in ids])
print(f"n={n}  collapsed raw agreement {po:.3f}  kappa {k:.3f}  (chance {pe:.3f})")
print("marginals a:", dict(ca), " b:", dict(cb))
po, k, pe, ca, cb = kappa([(binary(a[i]["label"]), binary(b[i]["label"])) for i in ids])
print(f"n={n}  binary raw agreement {po:.3f}  kappa {k:.3f}  (chance {pe:.3f})")
print("binary marginals a:", dict(ca), " b:", dict(cb))
both = [i for i in ids if collapse(a[i]["label"]) == collapse(b[i]["label"]) == "violation"]
exact = sum(a[i]["label"] == b[i]["label"] for i in both)
print(f"both call a violation: {len(both)}; same rule: {exact}")
print("\ndisagreements (id: a | b):")
for i in ids:
    if a[i]["label"] != b[i]["label"]:
        print(f"  {i}: {a[i]['label']} | {b[i]['label']}  -- b: {b[i].get('reason', '')[:110]}")
