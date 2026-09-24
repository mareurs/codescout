"""Draw T from the admitted Stage 2 rows by the registered procedure, and publish its per-rule counts.

    python3 draw_t.py

Registered (phase-1 local-classifier pre-registration, Stage 2 labelling amendment):
  1. Connected components over document groups, joining any two groups that share an 8-token
     shingle. Shingles and the pair set are the miner's own (mine_pairs.shingles over each
     group's positives and twins), and the pair count must reproduce the published 25.
  2. Each component goes to T with probability 0.3 under random.Random(20260927).
  3. T's per-rule positive counts are published; a rule under 10 is T-underpowered.

Not fixed by the registration, fixed here before the draw and label-blind: components meet
the generator in order of their alphabetically first doc group. The draw reads no label; the
labels are joined only afterwards, to count.
"""
import collections, importlib.util, itertools, json, pathlib, random, sys

HERE = pathlib.Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("mine_pairs", HERE / "mine_pairs.py")
mp = importlib.util.module_from_spec(spec); spec.loader.exec_module(mp)

DROPPED = ("not-a-violation", "not-a-pair", "unsure")


def main() -> int:
    rows = [json.loads(l) for l in (HERE / "mined-candidates.jsonl").read_text().splitlines()]
    by_group = collections.defaultdict(set)
    for r in rows:
        by_group[r["doc_group"]] |= mp.shingles(r["positive"]) | mp.shingles(r["twin"] or "")
    owners = collections.defaultdict(set)
    for g, ss in by_group.items():
        for s in ss:
            owners[s].add(g)
    pairs = {p for gs in owners.values() if len(gs) > 1 for p in itertools.combinations(sorted(gs), 2)}
    assert len(pairs) == 25, f"pair set drifted from the published 25: {len(pairs)}"

    parent = {g: g for g in by_group}
    def find(g):
        while parent[g] != g:
            parent[g] = parent[parent[g]]; g = parent[g]
        return g
    for a, b in pairs:
        parent[find(a)] = find(b)
    comps = collections.defaultdict(list)
    for g in by_group:
        comps[find(g)].append(g)
    ordered = sorted((sorted(m) for m in comps.values()), key=lambda m: m[0])
    rng = random.Random(20260927)
    t_groups = {g for m in ordered if rng.random() < 0.3 for g in m}

    labels = {r["id"]: r for r in map(json.loads, (HERE / "agent-labels.jsonl").read_text().splitlines())}
    split = {i: ("T" if r["doc_group"] in t_groups else "rest") for i, r in enumerate(rows)}
    (HERE / "t-split.jsonl").write_text("".join(
        json.dumps({"id": i, "doc_group": rows[i]["doc_group"], "split": s}) + "\n" for i, s in split.items()))

    pos = collections.defaultdict(collections.Counter)
    for i, s in split.items():
        l = labels[i]["label"]
        if l not in DROPPED:
            pos[l][s] += 1
    print(f"doc groups {len(by_group)}, shingle pairs {len(pairs)}, components {len(ordered)} "
          f"(largest {max(map(len, ordered))} groups)")
    print(f"T: {sum(1 for m in ordered if m[0] in t_groups)} components, {len(t_groups)} groups, "
          f"{sum(s == 'T' for s in split.values())} of {len(rows)} rows")
    print(f"admitted positives (primary label): T {sum(c['T'] for c in pos.values())}, "
          f"rest {sum(c['rest'] for c in pos.values())}")
    print(f"\n{'rule':22} {'T':>3} {'rest':>5}  T-powered (>= 10)?")
    for rule, c in sorted(pos.items(), key=lambda x: (-x[1]['T'], -x[1]['rest'], x[0])):
        print(f"{rule:22} {c['T']:>3} {c['rest']:>5}  {'yes' if c['T'] >= 10 else 'no -- T-underpowered'}")
    print(f"\nrules with any admitted positive: {len(pos)} of 22; T-powered: {sum(c['T'] >= 10 for c in pos.values())}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
