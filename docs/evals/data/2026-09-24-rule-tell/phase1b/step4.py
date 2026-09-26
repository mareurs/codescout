"""Phase 1b Stage 2, Step 4 for one run: calibration, thresholds and the pre-gate check over each
admitted head's own cells plus its admitted cross cells, then Stage 2's per-run measurements.
Input: score_run.py's output, which carries the admission file it was scored with. train_arm's
temperature fit and threshold rule are reused unchanged, so only the cell set differs from Stage 1.

Per admitted head H (a head Step 1 did not admit has already left the local menu):
- cells: H's own cells (rows whose rule is H: frozen rows, and arm NC's counterexample rows,
  which are negatives), plus H's admitted cross cells, each a negative.
- calibration: one temperature on cal over those cells, bounds [0.25, 10], a fit on a bound reported.
- thresholds: on val over the same cells. Precision: the smallest t with precision >= 0.9 and at
  least 1 true positive, else max F0.5. Recall: the largest t with own-positive recall >= 0.9
  (cross cells and counterexamples hold no positive, so recall over all cells is own-positive).
- the pre-gate check, on cal, at the precision threshold: cross firing <= 5% of H's cal cross
  cells, and own-positive recall >= 0.5. A head failing either leaves the local menu, reported
  with its counts. A head with no cal cross cell or no cal positive cannot be checked and leaves.
- saturation, reported and never applied: the cal cells that fire at t although their raw logit is
  below the raw cutoff t stands for, the only decisions float saturation lets T change.
Then the final menu, and whether the gate can detect anything on it: at least 2 of its 3 menu
positives, and at least 1 of the span gate's 2 menu texts, must still apply.

Measurements (Stage 2 § Measured per run), none of which feeds a choice:
- own-cell val AUC over frozen rows, pooled (Stage 1's learned criterion, >= 0.80) and per head;
- all-cells val AUC over own plus admitted cross cells, pooled and per head;
- on cal, at each head's precision threshold: firing on its own frozen negatives, and on its
  counterexample rows. Cal, because val set the thresholds.
All AUCs are over raw logits, as in Stage 1.

Run:  ~/work/claude/jevk5/.venv/bin/python step4.py --scored <score_run output> --out <step4.json>
      ~/work/claude/jevk5/.venv/bin/python step4.py --common <step4.json> ... --out <common.json>
"""
import argparse
import hashlib
import json
import math
import sys
from fractions import Fraction
from pathlib import Path

from sklearn.metrics import roc_auc_score

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "stage3"))
import train_arm as ta  # noqa: E402

CROSS_FIRING_MAX = Fraction(1, 20)     # the pre-gate check: at most 5% of the cal cross cells fire
RECALL_MIN = Fraction(1, 2)            # and at least half the cal own positives do
LEARNED_AUC = 0.80                     # Stage 1's registered criterion
# The gate texts that are positives for a local-menu rule: phase1-rule-selection.py's GATE_CASES
# and phase1-span-selector.py's EXTRA_GATE and SPAN_GATE. tests/test_phase1b_step4.py holds these
# to the gate code, so a gate text added or renamed there reds rather than drifting.
GATE_POSITIVES = {"semicolon": "d_semicolon", "sessionid": "d_sessionid", "member": "member_vs_population"}
SPAN_POSITIVES = {"span-sessionid": "d_sessionid", "span-semicolon": "d_semicolon"}
GATE_MIN, SPAN_MIN = 2, 1


def sig(x: float) -> float:        # phase 1's formula, guarded only where it would overflow
    try:
        return 1 / (1 + math.exp(-x))
    except OverflowError:
        return 0.0


def head_cells(scored: dict, fold: str, head: str) -> list[tuple[float, int, str]]:
    """(z, label, kind) for a head's cells on one fold. kind: "own" (a frozen row of the head's
    rule), "counterexample" (arm NC's rows, label 0), or "cross" (an admitted cross cell, label 0).
    A row is a counterexample exactly when its source says so, the test train_arm.cross_cells uses:
    frozen rows carry source "synthetic" or "mined"."""
    own = [(r["z"], r["label"], "counterexample" if r["source"] == "counterexample" else "own")
           for r in scored[fold] if r["rule"] == head]
    return own + [(c["z"], 0, "cross") for c in scored[f"{fold}_cross"] if c["head"] == head]


def fired(cells: list, kind: str, T: float, t: float, label: int | None = None) -> tuple[int, int]:
    """(cells of this kind, and label if given, that fire at p >= t; how many there are)."""
    sel = [z for z, y, k in cells if k == kind and (label is None or y == label)]
    return sum(sig(z / T) >= t for z in sel), len(sel)


def pregate(cal_cells: list, T: float, t: float) -> dict:
    cf, cn = fired(cal_cells, "cross", T, t)
    ph, pn = fired(cal_cells, "own", T, t, label=1)
    reasons = []
    if cn == 0:
        reasons.append("no cal cross cell")
    elif Fraction(cf, cn) > CROSS_FIRING_MAX:
        reasons.append(f"cross firing {cf}/{cn} > 5%")
    if pn == 0:
        reasons.append("no cal positive")
    elif Fraction(ph, pn) < RECALL_MIN:
        reasons.append(f"own-positive recall {ph}/{pn} < 0.5")
    return dict(cross_fired=cf, cross_n=cn, pos_hit=ph, pos_n=pn, passed=not reasons, reasons=reasons)


def saturation_ties(val_cells: list, cal_cells: list, T: float, t: float) -> dict:
    """Decisions the temperature changed. In exact arithmetic the calibrated threshold t is a raw
    cutoff z_t, the smallest val logit that fires, and T cancels from every decision. In floats,
    sigmoid(x) is exactly 1.0 for x above about 36.8, so at T = 0.25 every logit above about 9.2
    ties. Floats merge values and never reorder them, so the only flip is a cal cell that fires
    although its raw logit is below z_t; counted per pre-gate bound. Zero means the check read raw
    logits alone. Reported, never applied: the registered rule is the calibrated one."""
    cutoff = min(z for z, _, _ in val_cells if sig(z / T) >= t)
    below = [(y, k) for z, y, k in cal_cells if sig(z / T) >= t and z < cutoff]
    return dict(raw_cutoff=cutoff, cross=sum(k == "cross" for _, k in below),
                own_pos=sum(k == "own" and y == 1 for y, k in below))


def gate_ability(menu: list[str]) -> dict:
    gate = [cid for cid, h in GATE_POSITIVES.items() if h in menu]
    span = [cid for cid, h in SPAN_POSITIVES.items() if h in menu]
    return dict(gate_positives_applying=gate, span_texts_applying=span,
                gate_able=len(gate) >= GATE_MIN and len(span) >= SPAN_MIN)


def auc(pairs: list[tuple[float, int]]) -> float | None:
    """sklearn's roc_auc_score, Stage 1's instrument; None when a class is absent."""
    ys = [y for _, y in pairs]
    if len(set(ys)) < 2:
        return None
    return float(roc_auc_score(ys, [z for z, _ in pairs]))


def measure(scored: dict, heads: list[str], temps: dict, thr: dict) -> dict:
    own = {h: [(r["z"], r["label"]) for r in scored["val"] if r["rule"] == h and r["source"] != "counterexample"]
           for h in scored["menu"]}
    allc = {h: [(z, y) for z, y, _ in head_cells(scored, "val", h)] for h in heads}
    pooled_own = auc([p for v in own.values() for p in v])
    out = dict(
        own_cell_val_auc=dict(pooled=pooled_own, per_head={h: auc(v) for h, v in own.items()}),
        learned=pooled_own is not None and pooled_own >= LEARNED_AUC,
        all_cells_val_auc=dict(pooled=auc([p for v in allc.values() for p in v]),
                               per_head={h: auc(v) for h, v in allc.items()}),
        cal_own_negatives_fired={}, cal_counterexamples_fired={})
    for h in heads:
        cells = head_cells(scored, "cal", h)
        T, t = temps[h]["T"], thr[h]["precision_t"]
        out["cal_own_negatives_fired"][h] = list(fired(cells, "own", T, t, label=0))
        out["cal_counterexamples_fired"][h] = list(fired(cells, "counterexample", T, t))
    return out


def step4(scored: dict) -> dict:
    menu, admitted = scored["menu"], set(scored["cross"]["admitted"])
    heads = [h for h in menu if h in admitted]
    temps, thr, pre, sat = {}, {}, {}, {}
    for h in heads:
        cal_cells, val_cells = head_cells(scored, "cal", h), head_cells(scored, "val", h)
        T, at_bound = ta.fit_temperature([z for z, _, _ in cal_cells], [y for _, y, _ in cal_cells])
        temps[h] = dict(T=T, at_bound=at_bound, cal_n=len(cal_cells), cal_pos=sum(y for _, y, _ in cal_cells),
                        cal_cross=sum(k == "cross" for _, _, k in cal_cells),
                        cal_counterexamples=sum(k == "counterexample" for _, _, k in cal_cells))
        thr[h] = dict(ta.thresholds([sig(z / T) for z, _, _ in val_cells], [y for _, y, _ in val_cells]),
                      val_cross=sum(k == "cross" for _, _, k in val_cells),
                      val_counterexamples=sum(k == "counterexample" for _, _, k in val_cells))
        pre[h] = pregate(cal_cells, T, thr[h]["precision_t"])
        sat[h] = saturation_ties(val_cells, cal_cells, T, thr[h]["precision_t"])
    final = [h for h in heads if pre[h]["passed"]]
    return dict(
        run_dir=scored["run_dir"], arm=scored["arm"], recipe=scored["recipe"], seed=scored["seed"],
        checkpoint_sha256=scored["checkpoint_sha256"], cross=scored["cross"], extra_rows=scored["extra_rows"],
        menu=menu, final_menu=final,
        removed=dict(step1=[h for h in menu if h not in admitted],
                     step4=[dict(head=h, **pre[h]) for h in heads if not pre[h]["passed"]]),
        gate=gate_ability(final),
        temperatures=temps, thresholds=thr, pregate=pre, saturation=sat,
        measured=measure(scored, heads, temps, thr))


def common_menu(results: list[dict]) -> dict:
    """The heads on every run's final menu, in head order: the only menu a claim about the cause
    is read on, because a different menu alone can change a gate result."""
    menu = results[0]["menu"]
    if any(r["menu"] != menu for r in results):
        raise SystemExit("runs disagree on the head order")
    common = [h for h in menu if all(h in r["final_menu"] for r in results)]
    return dict(runs=[dict(run_dir=r["run_dir"], final_menu=r["final_menu"]) for r in results],
                common_menu=common, gate=gate_ability(common))


def main() -> int:
    ap = argparse.ArgumentParser()
    mode = ap.add_mutually_exclusive_group(required=True)
    mode.add_argument("--scored", type=Path, help="one run: score_run.py's output")
    mode.add_argument("--common", type=Path, nargs="+", help="several runs' step4.json")
    ap.add_argument("--out", type=Path, required=True)
    args = ap.parse_args()
    if args.common:
        res = common_menu([json.loads(p.read_text()) for p in args.common])
        print(f"common menu ({len(res['common_menu'])}): {res['common_menu']}  gate-able {res['gate']['gate_able']}")
    else:
        raw = args.scored.read_bytes()
        res = dict(step4(json.loads(raw)), input=dict(file=str(args.scored), sha256=hashlib.sha256(raw).hexdigest()))
        m = res["measured"]
        f4 = lambda x: "n/a" if x is None else f"{x:.4f}"  # noqa: E731
        print(f"{res['run_dir']}: final menu {len(res['final_menu'])}/{len(res['menu'])}; "
              f"removed at step 4 {[r['head'] for r in res['removed']['step4']]}; gate-able {res['gate']['gate_able']}")
        print(f"  own-cell val AUC {f4(m['own_cell_val_auc']['pooled'])} (learned {m['learned']}); "
              f"all-cells val AUC {f4(m['all_cells_val_auc']['pooled'])}")
        flips = {h: s for h, s in res["saturation"].items() if s["cross"] or s["own_pos"]}
        if flips:
            print(f"  SATURATION: cal cells firing below the raw cutoff, by head: {flips}")
    args.out.write_text(json.dumps(res, indent=1))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
