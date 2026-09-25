"""Size the synthetic top-up from MEASURED survival after every filter (top-up amendment). No model calls.

    python3 plan_topup.py            # writes topup-plan.json and topup-plan.txt

Per rule r, on the round-1 training data:
  current  S_r = train-fold survivors: synthetic pairs that passed construction, sit in a kept
             audit cell, are not quarantined, and survive the final filter; plus admitted mined
             rows (train fold) that survive it. `contradiction`'s cell is counted AS IF KEPT,
             because its v1 drop is superseded by the relational re-audit, which decides it.
  yield    y_r = round-1 construction pass rate (ok / 80).
  audit    a_r = 1 - disagreements / audited in r's round-1 training cell; for `contradiction`
             the v1 question is void, so the source rate is used instead.
  fold     f   = the measured train-fold share of round-1 training seeds.
  filter   s   = the measured share of train-fold synthetic items surviving the final filter.
  seeds_r  = min(CAP, ceil(MARGIN * max(0, 50 - S_r) / (y_r * f * a_r * s))).

The final filter (correction 2 and amendment 3), applied here as it will be at freeze:
  1. a training-side item (mined: its four fields; synthetic: positive and substituted
     paragraphs) sharing an 8-token shingle with any held-out input is dropped. Held-out inputs:
     mine_pairs.held_out(), every mined T row's four fields, every T-syn pair's two paragraphs
     (both generators), and every drawn S seed.
  2. a train-fold item sharing a shingle with a validation or calibration item is counted as
     lost to train (conservative: amendment 3 moves the smaller group, which can only lower
     the train count or leave it).
"""
import collections, json, math, pathlib, sys

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import generate_synthetic as gs          # noqa: E402

TARGET, MARGIN, CAP = 50, 1.3, 300
DROPPED = ("not-a-violation", "not-a-pair", "unsure")
SYN = HERE / "synthetic"


def load(p):
    return [json.loads(l) for l in pathlib.Path(p).read_text().splitlines() if l.strip()]


def paras(p):
    return (p["paragraph"], p["paragraph"].replace(p["violating_sentence"], p["fixed_sentence"], 1))


def main() -> int:
    mp = gs.mp
    rules = sorted(mp.sel.RULES)
    rows = load(HERE / "mined-candidates.jsonl")
    split = {r["id"]: r["split"] for r in load(HERE / "t-split.jsonl")}
    lab = {r["id"]: r["label"] for r in load(HERE / "agent-labels.jsonl")}
    fold_of = {r["group"]: r["fold"] for r in load(HERE / "fold-assignment.jsonl")}
    manifest = load(HERE / "seed-manifest.jsonl")
    train = load(SYN / "train/pairs.jsonl")
    tsyn = load(SYN / "tsyn-in/pairs.jsonl") + load(SYN / "tsyn-cross/pairs.jsonl")
    audit = load(SYN / "audit/audit.jsonl")
    cells = json.loads((SYN / "audit/decisions.json").read_text())
    quarantine = {a["pair_id"] for a in audit if a["disagree"] and a["rule"] != "contradiction"}

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

    items = []                                   # (rule, fold, kind, shingles)
    lost = collections.Counter()
    for p in train:
        key = f"{p['generator']}|training|{p['rule']}"
        if not p["ok"] or (cells["cells"][key]["drop"] and p["rule"] != "contradiction"):
            continue
        if p["pair_id"] in quarantine:
            lost["quarantined"] += 1; continue
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
    cur = collections.Counter(); syn_train = [0, 0]
    for rule, f, kind, sh in items:
        if f != "train":
            continue
        if kind == "synthetic":
            syn_train[1] += 1
        if sh & other:
            lost[f"cross-fold collision ({kind})"] += 1; continue
        if kind == "synthetic":
            syn_train[0] += 1
        cur[rule] += 1
    s = syn_train[0] / syn_train[1]
    held_syn = lost["held-out collision (synthetic)"]
    s *= 1 - held_syn / (held_syn + syn_train[1])

    seeds_r1 = [m for m in manifest if m["use"] and m["use"].startswith("train:")]
    f_train = sum(m["fold"] == "train" for m in seeds_r1) / len(seeds_r1)
    src_rate = cells["sources"]["claude:claude-sonnet-5"]["rate"]
    plan, lines = {}, []
    for r in rules:
        ok = sum(p["ok"] for p in train if p["rule"] == r)
        y = ok / sum(p["rule"] == r for p in train)
        c = cells["cells"][f"claude:claude-sonnet-5|training|{r}"]
        a = (1 - src_rate) if r == "contradiction" else 1 - c["disagree"] / c["audited"]
        need = max(0, TARGET - cur[r])
        seeds = min(CAP, math.ceil(MARGIN * need / (y * f_train * a * s))) if need else 0
        plan[r] = {"current": cur[r], "need": need, "yield": round(y, 3), "audit": round(a, 3), "seeds": seeds}
        lines.append(f"  {r:22} current {cur[r]:3}  need {need:3}  yield {y:.2f}  audit {a:.2f}  -> seeds {seeds}")
    head = [f"fold share f = {f_train:.3f}; filter survival s = {s:.3f}; source audit rate {src_rate}",
            "losses: " + json.dumps(dict(lost)),
            f"seeds requested: {sum(v['seeds'] for v in plan.values())} (cap {CAP}/rule, margin {MARGIN})"]
    (HERE / "topup-plan.json").write_text(json.dumps({"f": f_train, "s": s, "losses": lost, "rules": plan}, indent=1))
    (HERE / "topup-plan.txt").write_text("\n".join(head + lines) + "\n")
    print("\n".join(head + lines))
    return 0


if __name__ == "__main__":
    sys.exit(main())
