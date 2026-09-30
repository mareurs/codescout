"""Measure bug 2bac7e0a (diagnose_run's `own_negatives_fired` is counted on the threshold fold).

Offline, from each Stage 1 run's fold-logits.json, calibration.json and thresholds.json. No GPU, no torch.

CONTROL, run first: the validation-fold own-negative count recomputed here must equal the committed
diagnose output (stage1/<run>.json `own_negatives_fired`) for every rule of every run. If it does not,
nothing below is reported.
"""
import json
import math
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
RUNS = Path.home() / "work/claude/rule-tell-runs/phase1b-s1"      # as stage1_summary.py
RECIPES = ("s1-r1", "s1-r2")
SEEDS = ("20260935", "20260937", "20260940")
NAMES = [f"{r}-{s}" for r in RECIPES for s in SEEDS]


def sig(x: float) -> float:
    try:
        return 1 / (1 + math.exp(-x))
    except OverflowError:
        return 0.0


def load(name: str) -> dict:
    d = RUNS / name
    cal = json.loads((d / "calibration.json").read_text())
    return dict(menu=cal["menu"], T=cal["temperatures"],
                thr=json.loads((d / "thresholds.json").read_text()),
                fl=json.loads((d / "fold-logits.json").read_text()),
                committed=json.loads((HERE / "stage1" / f"{name}.json").read_text()))


def fired(D: dict, fold: str, rule: str) -> tuple[int, int, set]:
    """(fired, negatives, fired ids): `rule`'s own negatives on `fold` at its val-chosen precision_t."""
    T, t = D["T"][rule]["T"], D["thr"][rule]["precision_t"]
    neg = [r for r in D["fl"][fold] if r["rule"] == rule and r["label"] == 0]
    ids = {r["id"] for r in neg if sig(r["z"] / T) >= t}
    return len(ids), len(neg), ids


def main() -> int:
    data = {n: load(n) for n in NAMES}
    menu = data[NAMES[0]]["menu"]

    bad = 0
    for n, D in data.items():
        for rule in menu:
            f, m, _ = fired(D, "val", rule)
            if [f, m] != D["committed"]["own_negatives_fired"][rule]:
                bad += 1
    print(f"CONTROL: {len(NAMES)} runs x {len(menu)} rules, val counts differing from the committed "
          f"diagnose output: {bad}")
    if bad:
        return 1

    cells = [(n, r) for n in NAMES for r in menu]
    same_as_fp = sum(fired(data[n], "val", r)[0] == data[n]["thr"][r]["val_fp"] for n, r in cells)
    floor9 = sum(data[n]["thr"][r]["val_fp"] == data[n]["thr"][r]["val_tp"] // 9 for n, r in cells)
    print(f"\nval count == thresholds.json val_fp:        {same_as_fp}/{len(cells)} run-rule cells")
    print(f"val_fp == floor(val_tp / 9) (PREC_TARGET .9): {floor9}/{len(cells)} run-rule cells")

    print("\nper seed, r1 against r2 on the val fold:")
    for s in SEEDS:
        a, b = data[f"s1-r1-{s}"], data[f"s1-r2-{s}"]
        eq = anyf = idd = 0
        for rule in menu:
            fa, _, ia = fired(a, "val", rule)
            fb, _, ib = fired(b, "val", rule)
            eq += fa == fb
            if fa or fb:
                anyf += 1
                idd += ia != ib
        print(f"  {s}: equal counts on {eq}/{len(menu)} rules; of {anyf} rules with any fire, ids differ on {idd}")

    print("\npooled own-negative firing per run (same threshold, same T; only the fold differs):")
    pooled = {}
    for n, D in data.items():
        v = [fired(D, "val", r) for r in menu]
        c = [fired(D, "cal", r) for r in menu]
        pooled[n] = (sum(x[0] for x in v), sum(x[1] for x in v), sum(x[0] for x in c), sum(x[1] for x in c))
        vf, vn, cf, cn = pooled[n]
        print(f"  {n}: val {vf}/{vn}   cal {cf}/{cn}")
    for label, i in (("val", 0), ("cal", 2)):
        xs = [pooled[n][i] for n in NAMES]
        print(f"  {label} range over the six runs: {min(xs)}-{max(xs)}")
    r1 = [pooled[f"s1-r1-{s}"][2] for s in SEEDS]
    r2 = [pooled[f"s1-r2-{s}"][2] for s in SEEDS]
    print(f"  cal, r1 across seeds {r1} (range {max(r1) - min(r1)}); r1 minus r2 within a seed "
          f"{[a - b for a, b in zip(r1, r2)]}")

    per_rule_v = per_rule_c = 0
    for rule in menu:
        per_rule_v += len({fired(data[n], "val", rule)[0] for n in NAMES}) > 1
        per_rule_c += len({fired(data[n], "cal", rule)[0] for n in NAMES}) > 1
    print(f"\nrules whose count is not the same in all six runs: val {per_rule_v}/{len(menu)}, "
          f"cal {per_rule_c}/{len(menu)}")
    D = data[NAMES[0]]
    negs = {r: D["T"][r]["cal_n"] - D["T"][r]["cal_pos"] for r in menu}
    print(f"cal negatives per rule: min {min(negs.values())}, max {max(negs.values())}")
    v, c = {r["id"] for r in D["fl"]["val"]}, {r["id"] for r in D["fl"]["cal"]}
    print(f"val and cal fold ids in common: {len(v & c)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
