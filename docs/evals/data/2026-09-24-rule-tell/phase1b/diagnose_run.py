"""Diagnostics on one trained L2 run, for the phase-1b design (phase-1 pre-registration,
§ Diagnostics after the stop). Reads val only; T, the T-syn sets and the gate texts are never read.

Reports, for the run's selected checkpoint:
- validation loss per epoch and the selected epoch (from the run's own log);
- val AUC over each row's own labelled cell, pooled over rules (raw logits) and per rule;
- cross-rule firing on val: each head, at its precision threshold, on the target units of
  OTHER rules' val rows (unlabelled cells), beside its firing on its own rule's negatives.

CONTROL: on the phase-1 checkpoint this must reproduce the committed
`stage4/qwen-cross-rule-firing.txt` total (2614/6773); otherwise its other numbers are withheld.
"""
import argparse
import json
import math
import sys
from pathlib import Path

import torch
from sklearn.metrics import roc_auc_score

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "stage3"))
import train_arm as ta  # noqa: E402


def sigmoid(x: float) -> float:
    try:
        return 1 / (1 + math.exp(-x))
    except OverflowError:
        return 0.0


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--run-dir", type=Path, required=True)
    ap.add_argument("--arm", default="qwen", choices=sorted(ta.ARMS))
    ap.add_argument("--device", default="cuda:0")
    ap.add_argument("--out", type=Path, required=True, help="JSON result path")
    args = ap.parse_args()

    run = args.run_dir
    cal = json.loads((run / "calibration.json").read_text())
    menu, temps = cal["menu"], cal["temperatures"]
    thr = json.loads((run / "thresholds.json").read_text())
    log = [json.loads(line) for line in (run / "log.jsonl").read_text().splitlines()]
    start = next(e for e in log if e["event"] == "start")
    recipe = start.get("recipe", "phase1")   # the run's own encoding and feature handling

    model = ta.Arm(args.arm, len(menu), args.device, recipe)
    state = torch.load(run / "best.pt")
    got = model.load_state_dict(state, strict=False)
    absent = sorted(set(state) - set(model.state_dict()))
    if got.unexpected_keys or absent:
        raise SystemExit(f"checkpoint does not fit: unexpected {got.unexpected_keys[:3]} absent {absent[:3]}")
    model.eval()
    ri = {r: i for i, r in enumerate(menu)}

    val = ta.load_rows("val")
    own = []                                   # (rule, label, z)
    own_neg = {r: [0, 0] for r in menu}
    other = {r: [0, 0] for r in menu}
    with torch.no_grad():
        for row in val:
            z = model.unit_logits(ta.segment(row["text"]))[row["target"]].cpu().tolist()
            own.append((row["rule"], row["label"], z[ri[row["rule"]]]))
            for h in menu:
                fire = sigmoid(z[ri[h]] / temps[h]["T"]) >= thr[h]["precision_t"]
                if h == row["rule"]:
                    if row["label"] == 0:
                        own_neg[h][0] += fire
                        own_neg[h][1] += 1
                else:
                    other[h][0] += fire
                    other[h][1] += 1

    per_rule_auc = {}
    for r in menu:
        ys = [y for rr, y, _ in own if rr == r]
        zs = [z for rr, _, z in own if rr == r]
        per_rule_auc[r] = roc_auc_score(ys, zs)
    result = {
        "run_dir": str(run),
        "recipe": recipe,
        "seed": start.get("seed"),
        "permute_labels": start.get("permute_labels", False),
        "val_loss_by_epoch": [[e["epoch"], e["val_loss"]] for e in log if e["event"] == "val"],
        "selected": next(({"epoch": e["epoch"], "val_loss": e["val_loss"]} for e in log
                          if e["event"] == "selected"), None),
        "pooled_val_auc": roc_auc_score([y for _, y, _ in own], [z for _, _, z in own]),
        "per_rule_val_auc": per_rule_auc,
        "own_negatives_fired": own_neg,
        "other_rule_cells_fired": other,
        "other_rule_total": [sum(v[0] for v in other.values()), sum(v[1] for v in other.values())],
    }
    epochs = [e for e in log if e["event"] == "epoch"]
    result["final_epoch_train_loss"] = epochs[-1]["train_loss"] if epochs else None
    args.out.write_text(json.dumps(result, indent=1))
    o = result["other_rule_total"]
    print(f"{run}: recipe {recipe} seed {result['seed']} permute {result['permute_labels']}")
    print(f"  val loss by epoch {result['val_loss_by_epoch']}  selected {result['selected']}")
    print(f"  pooled val AUC {result['pooled_val_auc']:.3f}  per-rule min/median/max "
          f"{min(per_rule_auc.values()):.3f}/{sorted(per_rule_auc.values())[len(menu) // 2]:.3f}/"
          f"{max(per_rule_auc.values()):.3f}")
    print(f"  cross-rule firing on val: {o[0]}/{o[1]} ({o[0] / o[1]:.2f})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
