#!/usr/bin/env python3
"""Analyse a scored rule-tell run: judge calibration, then precision.

Reads the corpus's labels STRUCTURALLY — each `### Passage <id>` block's
`- **for_prompt:**`, `- **why_it_resembles:**` and `- **never_corrected:**`
fields, anchored on line-start + key prefix. Never by keyword: prose and field
share a vocabulary here by construction (a passage quotes the word "withheld"
in ordinary use), and a keyword scan is the exact defect RTD-10 names.

Order of report is the order of dependence:

  1. CALIBRATION — the spread distribution on THIS corpus. cross-family-panel-
     calibration pins max_spread=0.25 in an empty valley at n=64, and its own
     refresh rule says thresholds do not transfer corpora. The gate is NOT
     refitted here after seeing the result; it is reported against.
  2. POSITIVE GATE — the five seeded positives must fire on their own prompt.
     The corpus states the consequence itself: "If any of the five draws a NO,
     the run is not measuring precision ... and the control results from that
     run are uninterpretable."
  3. PRECISION — only if 2 passes.
"""
from __future__ import annotations

import argparse
import collections
import json
import re
import sys

PASSAGE_RE = re.compile(r"^### Passage ([A-Z0-9-]+) — (.+)$")
FIELD_RE = re.compile(r"^- \*\*(for_prompt|why_it_resembles|never_corrected):\*\*\s*(.*)$")
DIAGONAL = {"CTL3": "RTD-3", "CTL8": "RTD-8", "CTL9": "RTD-9",
            "CTL10": "RTD-10", "CTLX": "contradiction"}


def load_labels(path: str) -> dict[str, dict]:
    labels: dict[str, dict] = {}
    cur = None
    for line in open(path):
        m = PASSAGE_RE.match(line)
        if m:
            pid = m.group(1)
            if pid == "<id>":          # the format exemplar, not a passage
                cur = None
                continue
            cur = labels.setdefault(pid, {"passage_id": pid})
            continue
        if cur is None:
            continue
        f = FIELD_RE.match(line)
        if f:
            cur[f.group(1)] = f.group(2).strip()
    for pid, d in labels.items():
        w = d.get("why_it_resembles", "")
        d["shape"] = ("near-miss" if w.startswith("near-miss")
                      else "full-shape" if w.startswith("full-shape") else "UNKNOWN")
        # A positive is marked in its own field, not by the word appearing anywhere.
        d["is_positive"] = d.get("never_corrected", "").startswith("withheld")
        d["for_prompt"] = d.get("for_prompt", "").strip("`* ")
    return labels


def hist(spreads: list[float], gate: float) -> None:
    buckets = collections.Counter(min(int(s / 0.05), 19) for s in spreads)
    for i in range(20):
        lo = i * 0.05
        c = buckets.get(i, 0)
        if c or lo < 0.5:
            mark = "   <-- gate" if abs(lo - gate) < 0.025 else ""
            print(f"  {lo:.2f}-{lo+0.05:.2f} | {'#'*min(c,60)}{' ' if c else ''}{c}{mark}")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--scored", required=True)
    ap.add_argument("--controls", required=True)
    ap.add_argument("--max-spread", type=float, default=0.25)
    args = ap.parse_args()

    labels = load_labels(args.controls)
    rows = [json.loads(l) for l in open(args.scored)]
    errors = [r for r in rows if "error" in r]
    ok = [r for r in rows if "error" not in r]

    print(f"=== RUN — {len(ok)}/{len(rows)} scored, {len(errors)} errored")
    cfgs = sorted({r.get("judges", "unknown") for r in ok})
    print(f"    judge configuration: {cfgs}"
          + ("   ⚠ PROVISIONAL — single family, no divergence signal"
             if cfgs != ["panel"] else ""))
    if errors:
        print("⚠ ERRORS PRESENT. Every rate below is over a PARTIAL batch; an "
              "errored\n  row and a NO are indistinguishable downstream. Read this "
              "before the numbers.")
        for e in errors[:5]:
            print(f"    {e.get('task_id')}: {e['error'][:120]}")

    unlabelled = {r["passage_id"] for r in ok} - set(labels)
    if unlabelled:
        print(f"⚠ {len(unlabelled)} scored passages have no label block: "
              f"{sorted(unlabelled)[:6]}")

    # ---- 1. CALIBRATION ----------------------------------------------------
    # A single-judge run makes every spread 0.0 BY CONSTRUCTION. Computing the
    # antimode over it would print "CLEAN VALLEY — gate transfers" — a verdict
    # indistinguishable from the one a real cross-family run with perfect
    # agreement produces. The distribution is monotone under the very thing the
    # section exists to detect, so the section refuses rather than softens.
    judge_cfg = {r.get("judges", "unknown") for r in ok}
    spreads = [r["spread"] for r in ok]
    g = args.max_spread
    print(f"\n=== 1. JUDGE CALIBRATION ON THIS CORPUS (n={len(spreads)}) ===")
    if judge_cfg != {"panel"}:
        print(f"  REFUSED — this run was scored by {sorted(judge_cfg)}, not the "
              f"cross-family panel.\n  Every spread is 0.0 because there is only one "
              f"judge, so a clean-valley verdict here\n  would be an artifact of the "
              f"configuration, not a measurement of the corpus.\n  Re-run with "
              f"--judges panel (needs a live GEMINI_API_KEY) to calibrate the gate.")
    else:
        zone = [s for s in spreads if 0.20 < s <= 0.30]
        s = sorted(spreads)
        p = lambda q: s[min(int(q * (len(s) - 1)), len(s) - 1)]  # noqa: E731
        print(f"  mean={sum(s)/len(s):.3f}  p50={p(.5):.3f}  p90={p(.9):.3f}  "
              f"p95={p(.95):.3f}  max={s[-1]:.3f}")
        print(f"  over gate {g}: {sum(1 for x in s if x > g)}/{len(s)} "
              f"({100*sum(1 for x in s if x > g)/len(s):.1f}%) — these are WITHHELD")
        hist(s, g)
        print(f"\n  ANTIMODE: rows in (0.20, 0.30] = {len(zone)} -> "
              f"{'CLEAN VALLEY — gate transfers' if not zone else 'gate sits ON data, NOT a clean valley for this corpus'}")

    withheld = {r["task_id"] for r in ok if r["diverged"]}
    graded = [r for r in ok if not r["diverged"]]

    # ---- 2. POSITIVE GATE --------------------------------------------------
    positives = {p for p, d in labels.items() if d["is_positive"]}
    print(f"\n=== 2. POSITIVE GATE — {len(positives)} seeded positives must fire ===")
    gate_ok = True
    for pid in sorted(positives):
        own = labels[pid]["for_prompt"]
        hit = [r for r in ok if r["passage_id"] == pid and r["prompt_id"] == own]
        if not hit:
            print(f"  {pid:<10} {own:<14} NOT SCORED")
            gate_ok = False
            continue
        r = hit[0]
        flag = "WITHHELD" if r["diverged"] else r["verdict"]
        bad = r["diverged"] or r["verdict"] != "YES"
        gate_ok &= not bad
        legs = f"claude={r['claude']:.2f}"
        if r.get("gemini") is not None:
            legs += f" gemini={r['gemini']:.2f}"
        print(f"  {pid:<10} {own:<14} {flag:<9} score={r['score']:.2f} "
              f"({legs}) {'  <-- FAIL' if bad else ''}")
    print(f"\n  GATE: {'PASS' if gate_ok else 'FAIL — control results below are UNINTERPRETABLE'}")

    # ---- 3. PRECISION ------------------------------------------------------
    print(f"\n=== 3. PRECISION — diagonal controls (a YES is a false positive) ===")
    print(f"{'prompt':<14} {'controls':>9} {'YES':>5} {'rate':>7}   full-shape    near-miss   withheld")
    for sec, prompt in DIAGONAL.items():
        ctrls = [p for p, d in labels.items()
                 if p.startswith(sec + "-") and not d["is_positive"]]
        sub = [r for r in graded if r["prompt_id"] == prompt and r["passage_id"] in ctrls]
        wh = sum(1 for r in ok if r["prompt_id"] == prompt
                 and r["passage_id"] in ctrls and r["diverged"])
        yes = [r for r in sub if r["verdict"] == "YES"]
        fs = [r for r in sub if labels[r["passage_id"]]["shape"] == "full-shape"]
        nm = [r for r in sub if labels[r["passage_id"]]["shape"] == "near-miss"]
        fy = sum(1 for r in fs if r["verdict"] == "YES")
        ny = sum(1 for r in nm if r["verdict"] == "YES")
        rate = f"{len(yes)}/{len(sub)}" if sub else "—"
        print(f"{prompt:<14} {len(ctrls):>9} {len(yes):>5} {rate:>7}   "
              f"{fy}/{len(fs):<10} {ny}/{len(nm):<10} {wh}")

    # ---- 4. CROSS-TALK -----------------------------------------------------
    print(f"\n=== 4. CROSS-TALK — off-diagonal YES (a prompt firing outside its shape) ===")
    for sec, prompt in DIAGONAL.items():
        off = [r for r in graded if r["prompt_id"] == prompt
               and not r["passage_id"].startswith(sec + "-")]
        yes = sum(1 for r in off if r["verdict"] == "YES")
        print(f"  {prompt:<14} {yes:>3}/{len(off):<4} "
              f"({100*yes/len(off) if off else 0:.0f}%)")

    # ---- 5. REGISTERED PREDICTIONS ----------------------------------------
    print(f"\n=== 5. REGISTERED PREDICTIONS ===")
    c10 = [p for p, d in labels.items() if p.startswith("CTL10-") and not d["is_positive"]]
    s10 = [r for r in graded if r["prompt_id"] == "RTD-10" and r["passage_id"] in c10]
    y10 = [r for r in s10 if r["verdict"] == "YES"]
    print(f"  RTD-10 — registered: YES on >= 3 of the 12 controls, 'does not survive'.")
    print(f"           observed: {len(y10)}/{len(s10)} scored controls "
          f"({len(c10)} labelled)")
    for nm in ("CTL10-11", "CTL10-13"):
        h = [r for r in ok if r["passage_id"] == nm and r["prompt_id"] == "RTD-10"]
        if h:
            r = h[0]
            print(f"           {nm} (near-miss, the discriminating cell): "
                  f"{'WITHHELD' if r['diverged'] else r['verdict']} score={r['score']:.2f}")
    c9 = [p for p, d in labels.items() if p.startswith("CTL9-") and not d["is_positive"]]
    s9 = [r for r in graded if r["prompt_id"] == "RTD-9" and r["passage_id"] in c9]
    fs9 = [r for r in s9 if labels[r["passage_id"]]["shape"] == "full-shape"]
    nm9 = [r for r in s9 if labels[r["passage_id"]]["shape"] == "near-miss"]
    print(f"  RTD-9  — registered: report full-shape and near-miss SEPARATELY; "
          f"a pooled rate hides\n           that a well-followed rule depletes its own control population.")
    print(f"           full-shape {sum(1 for r in fs9 if r['verdict']=='YES')}/{len(fs9)}"
          f"   near-miss {sum(1 for r in nm9 if r['verdict']=='YES')}/{len(nm9)}")

    if withheld:
        print(f"\n  {len(withheld)} task(s) withheld on panel divergence — counted "
              f"out of every rate above, never as a NO.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
