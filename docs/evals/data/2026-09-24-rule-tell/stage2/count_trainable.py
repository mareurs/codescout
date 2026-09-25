"""Trainable rules at freeze (top-up amendment, "after the top-up"). No model calls.

    python3 count_trainable.py       # writes trainable.json and trainable.txt

Train-fold positives per rule, by the registered procedure:
  - synthetic, round 1: construction-passing `train` pairs whose round-1 training cell was kept;
    for `contradiction`, the relational re-audit's cell decides instead of round 1's (void);
  - synthetic, round 2: construction-passing `topup` pairs whose round-2 training cell was kept;
  - quarantine: every audited pair with a disagreement, in all three audits, is excluded;
  - mined: admitted rows (a rule label) outside T;
  - the final filter, identical to plan_topup.py: held-out collisions drop an item, and a
    train-fold item sharing a shingle with a validation or calibration item is counted lost.
A rule reaching 50 is trainable; every other rule stays Haiku-only.
"""
import collections, json, pathlib, sys

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import generate_synthetic as gs          # noqa: E402

TARGET = 50
SYN = HERE / "synthetic"
DROPPED = ("not-a-violation", "not-a-pair", "unsure")


def load(p):
    return [json.loads(l) for l in pathlib.Path(p).read_text().splitlines() if l.strip()]


def paras(p):
    return (p["paragraph"], p["paragraph"].replace(p["violating_sentence"], p["fixed_sentence"], 1))


def main() -> int:
    mp = gs.mp
    rows = load(HERE / "mined-candidates.jsonl")
    split = {r["id"]: r["split"] for r in load(HERE / "t-split.jsonl")}
    lab = {r["id"]: r["label"] for r in load(HERE / "agent-labels.jsonl")}
    fold_of = {r["group"]: r["fold"] for r in load(HERE / "fold-assignment.jsonl")}
    manifest = load(HERE / "seed-manifest.jsonl")
    r1 = json.loads((SYN / "audit/decisions.json").read_text())["cells"]
    rel = json.loads((SYN / "audit-contradiction-relational/decisions.json").read_text())["cells"]
    r2 = json.loads((SYN / "audit-r2/decisions.json").read_text())["cells"]
    audits = load(SYN / "audit/audit.jsonl") + load(SYN / "audit-contradiction-relational/audit.jsonl") \
        + load(SYN / "audit-r2/audit.jsonl")
    # round 1's v1 contradiction verdicts are void: only the relational re-audit quarantines those pairs
    v1_contra = {a["pair_id"] for a in load(SYN / "audit/audit.jsonl") if a["rule"] == "contradiction"}
    rel_dis = {a["pair_id"] for a in load(SYN / "audit-contradiction-relational/audit.jsonl") if a["disagree"]}
    quarantine = {a["pair_id"] for a in audits if a["disagree"]} - (v1_contra - rel_dis)

    tsyn = load(SYN / "tsyn-in/pairs.jsonl") + load(SYN / "tsyn-cross/pairs.jsonl")
    H = set().union(*mp.held_out().values())
    for i, s in split.items():
        if s == "T":
            for f in ("positive", "twin", "context_before", "context_after"):
                H |= mp.shingles(rows[i].get(f) or "")
    for p in tsyn:
        if isinstance(p.get("paragraph"), str) and isinstance(p.get("violating_sentence"), str):
            for t in paras(p):
                H |= mp.shingles(t)
    for m in manifest:
        if m["use"] and m["use"].startswith("tsyn:"):
            H |= mp.shingles(m["text"])

    def kept(p, rnd):
        key = f"{p['generator']}|training|{p['rule']}"
        if rnd == 1 and p["rule"] == "contradiction":
            return not rel[key]["drop"]
        return not (r1 if rnd == 1 else r2)[key]["drop"]

    items, lost = [], collections.Counter()
    for rnd, f in ((1, "train/pairs.jsonl"), (2, "topup/pairs.jsonl")):
        for p in load(SYN / f):
            if not p["ok"]:
                continue
            if not kept(p, rnd):
                lost[f"round {rnd}: cell dropped"] += 1; continue
            if p["pair_id"] in quarantine:
                lost[f"round {rnd}: quarantined"] += 1; continue
            sh = mp.shingles(paras(p)[0]) | mp.shingles(paras(p)[1])
            if sh & H:
                lost["held-out collision (synthetic)"] += 1; continue
            items.append((p["rule"], p["fold"], "synthetic", sh))
    for i, r in enumerate(rows):
        if split[i] != "rest" or lab[i] in DROPPED or lab[i] not in mp.sel.RULES:
            continue
        sh = set().union(*(mp.shingles(r.get(f) or "") for f in ("positive", "twin", "context_before", "context_after")))
        if sh & H:
            lost["held-out collision (mined)"] += 1; continue
        items.append((lab[i], fold_of[r["doc_group"]], "mined", sh))

    other = set().union(*(sh for _, f, _, sh in items if f in ("val", "cal")))
    cur = collections.Counter(); src = collections.defaultdict(collections.Counter)
    for rule, f, kind, sh in items:
        if f != "train":
            continue
        if sh & other:
            lost[f"cross-fold collision ({kind})"] += 1; continue
        cur[rule] += 1; src[rule][kind] += 1
    rules = sorted(mp.sel.RULES)
    trainable = [r for r in rules if cur[r] >= TARGET]
    lines = [f"losses: {json.dumps(dict(lost))}",
             f"trainable (>= {TARGET} train-fold positives): {len(trainable)} of {len(rules)}: {trainable}"]
    lines += [f"  {r:22} {cur[r]:4}  (synthetic {src[r]['synthetic']}, mined {src[r]['mined']}) "
              f"{'TRAINABLE' if cur[r] >= TARGET else 'Haiku-only'}" for r in rules]
    (HERE / "trainable.json").write_text(json.dumps({"losses": lost, "counts": {r: cur[r] for r in rules},
                                                     "trainable": trainable}, indent=1))
    (HERE / "trainable.txt").write_text("\n".join(lines) + "\n")
    print("\n".join(lines))
    return 0


if __name__ == "__main__":
    sys.exit(main())
