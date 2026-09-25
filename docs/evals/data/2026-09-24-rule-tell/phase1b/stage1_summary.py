"""Phase-1b Stage 1 results: applies the registered definitions and decision rule to the six runs.

    ~/work/claude/jevk5/.venv/bin/python stage1_summary.py

Reads the six diagnose outputs in stage1/ and each run's own files (log.jsonl, calibration.json,
thresholds.json, fold-logits.json) under the run root. Reads no T, T-syn or gate text; the
registration's own definitions (pre-registration § Stage 1):
  learned  pooled own-cell val AUC >= 0.80
  failed   pooled val AUC in [0.45, 0.55], or final-epoch train loss >= 0.65
  weak     neither
  stable   all three seeds learned; if both recipes are stable, the higher worst-seed pooled
           val AUC is carried, a difference under 0.005 being a tie that goes to s1-r1.

Also reported, and NOT registered: own-negative firing on `cal` at each head's precision
threshold. The diagnose step counted own negatives on `val`, the fold that chose the threshold,
which fixes the count by construction (docs/issues/2026-09-25-diagnose-own-negative-firing-is-
counted-on-the-threshold-fold.md). `cal` did not choose the threshold, though it did fit T.

Writes stage1/summary.json and stage1/summary.txt.
"""
import json
import math
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
RUNS = Path.home() / "work/claude/rule-tell-runs/phase1b-s1"
RECIPES = ("s1-r1", "s1-r2")
SEEDS = (20260935, 20260937, 20260940)
LEARNED, NULL_BAND, FAIL_LOSS, TIE = 0.80, (0.45, 0.55), 0.65, 0.005
OVERSHOOT = 0.75                      # prediction 3: running train loss above this in epoch 0


def sigmoid(x: float) -> float:
    return 1 / (1 + math.exp(-x)) if x >= 0 else math.exp(x) / (1 + math.exp(x))


def wilson(k: int, n: int, z: float = 1.96) -> tuple[float, float]:
    p = k / n
    d = 1 + z * z / n
    c = (p + z * z / (2 * n)) / d
    h = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / d
    return max(0.0, c - h), min(1.0, c + h)


def classify(auc: float, final_loss: float) -> str:
    if auc >= LEARNED:
        return "learned"
    if NULL_BAND[0] <= auc <= NULL_BAND[1] or final_loss >= FAIL_LOSS:
        return "failed"
    return "weak"


def cal_own_negatives(run: Path) -> dict:
    temps = json.loads((run / "calibration.json").read_text())["temperatures"]
    thr = json.loads((run / "thresholds.json").read_text())
    cal = json.loads((run / "fold-logits.json").read_text())["cal"]
    per = {}
    for x in cal:
        h = x["rule"]
        fire = sigmoid(x["z"] / temps[h]["T"]) >= thr[h]["precision_t"]
        e = per.setdefault(h, {"neg_fired": 0, "neg": 0, "pos_hit": 0, "pos": 0})
        if x["label"] == 0:
            e["neg_fired"] += fire; e["neg"] += 1
        else:
            e["pos_hit"] += fire; e["pos"] += 1
    return per


def main() -> int:
    runs = {}
    for rec in RECIPES:
        for seed in SEEDS:
            name = f"{rec}-{seed}"
            d = json.loads((HERE / "stage1" / f"{name}.json").read_text())
            log = [json.loads(line) for line in (RUNS / name / "log.jsonl").read_text().splitlines()]
            ep0 = [e["train_loss"] for e in log if e["event"] == "progress" and e["epoch"] == 0]
            done = next(e for e in log if e["event"] == "done")
            aucs = sorted(d["per_rule_val_auc"].values())
            cal = cal_own_negatives(RUNS / name)
            runs[name] = {
                "recipe": rec, "seed": seed, "selected_epoch": d["selected"]["epoch"],
                "selected_val_loss": d["selected"]["val_loss"], "pooled_val_auc": d["pooled_val_auc"],
                "per_rule_auc_min": aucs[0], "per_rule_auc_upper_median": aucs[len(aucs) // 2],
                "final_epoch_train_loss": d["final_epoch_train_loss"],
                "epoch0_max_running_train_loss": max(ep0),
                "cross_rule_firing_val": d["other_rule_total"],
                "temps_at_bound": done.get("temps_at_bound", []),
                "cal_own_negatives_fired": [sum(v["neg_fired"] for v in cal.values()),
                                            sum(v["neg"] for v in cal.values())],
                "cal_own_positive_recall": [sum(v["pos_hit"] for v in cal.values()),
                                            sum(v["pos"] for v in cal.values())],
                "cal_per_rule": cal,
                "class": classify(d["pooled_val_auc"], d["final_epoch_train_loss"])}
    recipes = {}
    for rec in RECIPES:
        rs = [runs[f"{rec}-{s}"] for s in SEEDS]
        k = sum(r["class"] == "learned" for r in rs)
        f = sum(r["class"] == "failed" for r in rs)
        recipes[rec] = {"learned": k, "failed": f, "n": len(rs), "stable": k == len(rs),
                        "learned_wilson95": wilson(k, len(rs)), "failed_wilson95": wilson(f, len(rs)),
                        "worst_seed_auc": min(r["pooled_val_auc"] for r in rs)}
    stable = [r for r in RECIPES if recipes[r]["stable"]]
    if len(stable) == 2:
        a, b = (recipes[r]["worst_seed_auc"] for r in RECIPES)
        carried = "s1-r1" if abs(a - b) < TIE else max(RECIPES, key=lambda r: recipes[r]["worst_seed_auc"])
        why = f"both stable; worst-seed AUC {a:.4f} vs {b:.4f}" + (" (tie, to s1-r1)" if abs(a - b) < TIE else "")
    elif len(stable) == 1:
        carried, why = stable[0], "only stable recipe"
    else:
        carried, why = None, "neither stable: Stage 1 fails"
    out = {"runs": runs, "recipes": recipes, "carried": carried, "why": why}
    (HERE / "stage1" / "summary.json").write_text(json.dumps(out, indent=1))

    lines = [f"{'run':16s} {'ep':>2s} {'val loss':>8s} {'AUC':>6s} {'min':>6s} {'med':>6s} "
             f"{'ep0 max':>7s} {'final':>7s} {'cross-rule':>16s} {'cal own neg':>11s} {'cal recall':>10s}  class"]
    for name, r in runs.items():
        c, n_, p, q = *r["cross_rule_firing_val"], *r["cal_own_negatives_fired"]
        rh, rp = r["cal_own_positive_recall"]
        lines.append(f"{name:16s} {r['selected_epoch']:>2d} {r['selected_val_loss']:8.3f} {r['pooled_val_auc']:6.3f} "
                     f"{r['per_rule_auc_min']:6.3f} {r['per_rule_auc_upper_median']:6.3f} "
                     f"{r['epoch0_max_running_train_loss']:7.3f} {r['final_epoch_train_loss']:7.4f} "
                     f"{c:>5d}/{n_} ({c / n_:.2f}) {p:>4d}/{q:<5d} {rh:>4d}/{rp:<4d}  {r['class']}")
    for rec, v in recipes.items():
        lo, hi = v["learned_wilson95"]
        flo, fhi = v["failed_wilson95"]
        lines.append(f"{rec}: learned {v['learned']}/{v['n']} (Wilson 95% {lo:.2f}-{hi:.2f}); failed "
                     f"{v['failed']}/{v['n']} ({flo:.2f}-{fhi:.2f}); worst-seed AUC {v['worst_seed_auc']:.4f}; "
                     f"stable {v['stable']}")
    lines.append(f"carried forward: {carried} ({why})")
    (HERE / "stage1" / "summary.txt").write_text("\n".join(lines) + "\n")
    print("\n".join(lines))
    return 0


if __name__ == "__main__":
    sys.exit(main())
