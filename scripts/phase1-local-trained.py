#!/usr/bin/env python3
"""Local route, Stage 4: a trained Stage-3 arm (L1-MBERT or L2-QWEN) as the per-rule judge.

Registered in docs/evals/phase1-local-classifier-preregistration.md (Stage 3, Stage 4, and the
Stage-4 execution amendment). Like scripts/phase1-local-l0.py it reuses
scripts/phase1-span-selector.py's gate and span gate and swaps `judge_rule`. Two further
substitutions, both disclosed in the amendment:
  - `sel.JUDGED` = the 14-rule local menu. The gate's own code reports a positive for a
    Haiku-only rule as n/a, never as a pass or a failure.
  - `sel.SPAN_GATE` keeps only menu rules (`span-cannot` is `cannot_happen`, Haiku-only).

Per (text, rule): the text's `segment()` units are scored once (cached per text). Candidate
units are those at least `sel.MIN_SPAN` long, the claims `verify_span` can accept (as L0).
P(rule) = max over candidates of sigmoid(z / T_rule); the rule fires when P >= its val-fold
threshold. The claim is the argmax unit, verbatim; `verify_span` runs as an invariant.

Run:
  ~/work/claude/jevk5/.venv/bin/python -u scripts/phase1-local-trained.py --arm qwen --gate --log x.jsonl
  ~/work/claude/rule-tell-rocm/.venv/bin/python -u scripts/phase1-local-trained.py --arm mbert --parity
"""

import argparse
import importlib.util
import json
import math
import pathlib
import sys

import torch

HERE = pathlib.Path(__file__).resolve().parent
_spec = importlib.util.spec_from_file_location("selector", HERE / "phase1-span-selector.py")
sel = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(sel)

STAGE3 = HERE.parent / "docs/evals/data/2026-09-24-rule-tell/stage3"
sys.path.insert(0, str(STAGE3))
import train_arm as ta  # noqa: E402

RUNS = pathlib.Path.home() / "work/claude/rule-tell-runs/stage3"
CHECKPOINT_SHA = {   # recorded in the Stage 3 results section
    "mbert": "8b9de74e756a5edeeb076084a45590b183cedf5cf7a9e7bfe20a6da16c6d44a1",
    "qwen": "db18339dbeb2f92f70aa6f46da7539e83baa3a975dcf513170510c643bda2bc4",
}

_model = None
_temps: dict = {}
_thr: dict = {}
_threshold_key = "precision_t"
_rule_idx: dict[str, int] = {}
_cache: dict[str, tuple[list[str], list[int], list[list[float]]]] = {}
_log: list[dict] = []


def sigmoid(x: float) -> float:
    """train_arm's own formula (`1 / (1 + exp(-z/T))`), guarded only where it would overflow."""
    try:
        return 1 / (1 + math.exp(-x))
    except OverflowError:
        return 0.0


def load_arm(arm: str, device: str):
    import hashlib
    ckpt = RUNS / arm / "best.pt"
    h = hashlib.sha256(ckpt.read_bytes()).hexdigest()
    if h != CHECKPOINT_SHA[arm]:
        raise SystemExit(f"checkpoint {ckpt} sha256 {h} is not the recorded one")
    res = json.loads((STAGE3 / "results" / arm / "calibration.json").read_text())
    menu = res["menu"]
    model = ta.Arm(arm, len(menu), device)
    state = torch.load(ckpt)
    got = model.load_state_dict(state, strict=False)
    missing_saved = sorted(set(state) - set(model.state_dict()))
    if got.unexpected_keys or missing_saved:
        raise SystemExit(f"checkpoint does not fit the arm: unexpected {got.unexpected_keys[:3]} "
                         f"absent {missing_saved[:3]}")
    model.eval()
    thr = json.loads((STAGE3 / "results" / arm / "thresholds.json").read_text())
    return model, menu, res["temperatures"], thr


@torch.no_grad()
def scored(text: str):
    if text not in _cache:
        units = ta.segment(text)
        cand = [i for i, u in enumerate(units) if len(u) >= sel.MIN_SPAN]
        z = _model.unit_logits(units).cpu().tolist() if units else []
        _cache[text] = (units, cand, z)
    return _cache[text]


def judge_rule(text: str, rule: str, retries: int = 1) -> dict:
    if rule not in _temps:
        raise ValueError(f"{rule} is not on the local menu")   # JUDGED = menu; cannot be reached
    units, cand, z = scored(text)
    j = _rule_idx[rule]   # the checkpoint's own head order, from calibration.json's menu
    T, t = _temps[rule]["T"], _thr[rule][_threshold_key]
    if not cand:
        _log.append({"rule": rule, "p": None, "verdict": "NO"})
        return {"rule": rule, "verdict": "NO"}
    ps = [sigmoid(z[i][j] / T) for i in cand]
    k = max(range(len(ps)), key=lambda i: ps[i])
    p = ps[k]
    row = {"rule": rule, "p": p, "threshold": t}
    if p < t:
        _log.append({**row, "verdict": "NO"})
        return {"rule": rule, "verdict": "NO"}
    claim = units[cand[k]]
    span = sel.verify_span(claim, text)
    if not span:  # a segmenter unit of the text; a failure is a bug, raised, never downgraded
        raise ValueError(f"argmax unit failed verify_span: {claim[:80]!r}")
    _log.append({**row, "verdict": "YES", "claim": span})
    return {"rule": rule, "verdict": "YES", "claim": span}


def parity(arm: str, menu: list[str]) -> int:
    """Recompute every val logit from the loaded checkpoint and compare with the committed
    fold-logits.json. Proves the checkpoint load (and so which epoch calibration used) and
    measures backend drift. Reads only val, which Stage 3 already consumed."""
    committed = json.loads((STAGE3 / "results" / arm / "fold-logits.json").read_text())["val"]
    rows = {r["id"]: r for r in ta.load_rows("val")}
    ri = {r: i for i, r in enumerate(menu)}
    diffs, flips = [], 0
    for c in committed:
        r = rows[c["id"]]
        units = ta.segment(r["text"])
        with torch.no_grad():
            z = _model.unit_logits(units)[r["target"], ri[r["rule"]]].item()
        diffs.append(abs(z - c["z"]))
        T, t = _temps[r["rule"]]["T"], _thr[r["rule"]]["precision_t"]
        flips += (sigmoid(z / T) >= t) != (sigmoid(c["z"] / T) >= t)
    print(f"parity {arm} on {torch.cuda.get_device_name()} ({'rocm' if torch.version.hip else 'cuda'}): "
          f"{len(diffs)} val cells, max |dz| {max(diffs):.3e}, mean {sum(diffs)/len(diffs):.3e}, "
          f"decisions flipped at the precision threshold: {flips}")
    return 0


def main() -> int:
    global _model, _temps, _thr, _threshold_key, _rule_idx
    ap = argparse.ArgumentParser()
    ap.add_argument("--arm", choices=sorted(ta.ARMS), required=True)
    ap.add_argument("--device", default="cuda:0")
    ap.add_argument("--threshold", choices=["precision", "recall90"], default="precision",
                    help="precision: L1/L2 standalone (registered); recall90: C1's first stage")
    mode = ap.add_mutually_exclusive_group(required=True)
    mode.add_argument("--gate", action="store_true")
    mode.add_argument("--span-gate", action="store_true")
    mode.add_argument("--parity", action="store_true")
    mode.add_argument("--check-determinism", action="store_true")
    ap.add_argument("--runs", type=int, default=1)
    ap.add_argument("--log", help="write every (rule, probability, verdict) row as JSONL")
    args = ap.parse_args()
    args.pool = 1
    _threshold_key = "precision_t" if args.threshold == "precision" else "recall90_t"

    _model, menu, _temps, _thr = load_arm(args.arm, args.device)
    _rule_idx = {r: i for i, r in enumerate(menu)}
    sel.JUDGED = list(menu)
    sel.SPAN_GATE = [s for s in sel.SPAN_GATE if s[1] in menu]
    sel.judge_rule = judge_rule
    backend = "rocm" if torch.version.hip else "cuda"
    print(f"judge: trained {args.arm} on {torch.cuda.get_device_name(args.device)} ({backend}), "
          f"{len(menu)} menu rules, threshold {args.threshold}", flush=True)

    if args.parity:
        return parity(args.arm, menu)
    if args.check_determinism:
        cid, text = "neutral", ("The build script copies the assets into dist/. It then runs "
                                "the bundler with the production flag and writes a manifest.")
        with torch.no_grad():
            a = _model.unit_logits(ta.segment(text)).cpu()
            b = _model.unit_logits(ta.segment(text)).cpu()
        drift = (a - b).abs().max().item()
        print(f"determinism on {cid}: max |dz| over {a.numel()} cells = {drift:.2e}")
        return 0 if drift < 1e-4 else 1

    rc = sel.gate(args) if args.gate else sel.span_gate(args)
    if args.log:
        with open(args.log, "w") as fh:
            fh.writelines(json.dumps(r) + "\n" for r in _log)
    return rc


if __name__ == "__main__":
    sys.exit(main())
