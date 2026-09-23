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

    forms = sorted({r.get("form", "rubric") for r in ok})
    runs = sorted({r.get("run", 0) for r in ok})
    print(f"=== RUN — {len(ok)}/{len(rows)} rows scored, {len(errors)} errored; "
          f"form={forms}, runs per task={len(runs)}")
    cfgs = sorted({r.get("judges", "unknown") for r in ok})
    print(f"    judge configuration: {cfgs}"
          + ("   ⚠ PROVISIONAL — single family, no divergence signal"
             if cfgs != ["panel"] else ""))
    if "rubric" in forms or "native" in forms:
        print("    ⚠ form includes rubric/native — both FAILED the mutation gate "
              "(rubric biased YES, native biased NO). Only `reasoned` passed.")
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
    spreads = [r["spread"] for r in ok]
    g = args.max_spread
    print(f"\n=== 1. JUDGE CALIBRATION ON THIS CORPUS ===")
    if cfgs != ["panel"]:
        print(f"  REFUSED — scored by {cfgs}, not the cross-family panel. Every spread "
              f"is 0.0 with one judge,\n  so a clean-valley verdict here would be an "
              f"artifact of the configuration.")
    else:
        zone = [s for s in spreads if 0.20 < s <= 0.30]
        s = sorted(spreads)
        p = lambda q: s[min(int(q * (len(s) - 1)), len(s) - 1)]  # noqa: E731
        print(f"  mean={sum(s)/len(s):.3f}  p50={p(.5):.3f}  p90={p(.9):.3f}  "
              f"p95={p(.95):.3f}  max={s[-1]:.3f}")
        print(f"  over gate {g}: {sum(1 for x in s if x > g)}/{len(s)} — these are WITHHELD")
        hist(s, g)
        print(f"\n  ANTIMODE: rows in (0.20, 0.30] = {len(zone)} -> "
              f"{'CLEAN VALLEY — gate transfers' if not zone else 'gate sits ON data'}")

    # Per-task FIRE RATE over runs. Withheld rows (panel divergence) are counted
    # out, never as a NO.
    by_task: dict[tuple[str, str], list[int]] = collections.defaultdict(list)
    withheld = 0
    for r in ok:
        if r.get("diverged"):
            withheld += 1
            continue
        by_task[(r["prompt_id"], r["passage_id"])].append(r["verdict"] == "YES")
    rate = {k: sum(v) / len(v) for k, v in by_task.items() if v}

    def cell(prompt: str, pids: list[str]) -> str:
        rs = [rate[(prompt, p)] for p in pids if (prompt, p) in rate]
        if not rs:
            return "—"
        maj = sum(1 for x in rs if x >= 0.5)
        return f"{sum(rs)/len(rs):.2f} ({maj}/{len(rs)} tasks fire)"

    # ---- 2. POSITIVE GATE --------------------------------------------------
    positives = {p for p, d in labels.items() if d["is_positive"]}
    print(f"\n=== 2. POSITIVE GATE — {len(positives)} seeded positives must fire "
          f"(fire rate >= 0.5) ===")
    gate_ok = True
    for pid in sorted(positives):
        own = labels[pid]["for_prompt"]
        x = rate.get((own, pid))
        if x is None:
            print(f"  {pid:<10} {own:<14} NOT SCORED")
            gate_ok = False
            continue
        bad = x < 0.5
        gate_ok &= not bad
        print(f"  {pid:<10} {own:<14} fire rate {x:.2f}{'   <-- FAIL' if bad else ''}")
    print(f"\n  GATE: {'PASS' if gate_ok else 'FAIL — a prompt that misses its own positive is not measuring precision'}")

    # ---- 3. DIAGONAL, BY WHAT EACH LABEL MEANS -----------------------------
    # The corpus defines the two control kinds (rule-tell-controls.md § What a
    # control is here). Reading both as negatives was the defect in the first
    # analysis of this run: a FULL-SHAPE control carries every YES feature, so a
    # fire is what the prompt's wording REQUIRES; only a NEAR-MISS fire is a
    # failure to honour the prompt's own exclusions.
    print(f"\n=== 3. DIAGONAL CONTROLS — mean fire rate, and tasks firing at >= 0.5 ===")
    print("  near-miss  : a stated NO condition is met -> a fire is a PRECISION DEFECT")
    print("  full-shape : every YES feature present   -> a fire is what the wording requires;")
    print("               a non-fire means the prompt skipped its own YES branch, OR the")
    print("               label is wrong — this column cannot tell which")
    print(f"\n{'prompt':<14} {'near-miss (precision)':<28} {'full-shape':<28}")
    for sec, prompt in DIAGONAL.items():
        ctrls = [p for p, d in labels.items()
                 if p.startswith(sec + "-") and not d["is_positive"]]
        nm = [p for p in ctrls if labels[p]["shape"] == "near-miss"]
        fs = [p for p in ctrls if labels[p]["shape"] == "full-shape"]
        unk = [p for p in ctrls if labels[p]["shape"] == "UNKNOWN"]
        tail = f"   ({len(unk)} controls carry no shape label)" if unk else ""
        print(f"{prompt:<14} {cell(prompt, nm):<28} {cell(prompt, fs):<28}{tail}")

    # ---- 4. OFF-DIAGONAL ---------------------------------------------------
    # Another prompt's passages carry NO label for this prompt, so a fire there is
    # not known to be wrong. Reported as a rate, never as a false-positive rate.
    print(f"\n=== 4. OFF-DIAGONAL — UNLABELLED for this prompt; a fire rate, "
          f"NOT a false-positive rate ===")
    for sec, prompt in DIAGONAL.items():
        off = sorted({p for (pr, p) in rate if pr == prompt and not p.startswith(sec + "-")})
        print(f"  {prompt:<14} {cell(prompt, off)}")

    # ---- 5. REGISTERED PREDICTIONS ----------------------------------------
    print(f"\n=== 5. REGISTERED PREDICTIONS ===")
    c10 = [p for p, d in labels.items() if p.startswith("CTL10-") and not d["is_positive"]]
    fire10 = [p for p in c10 if rate.get(("RTD-10", p), 0) >= 0.5]
    print(f"  RTD-10 — registered: YES on >= 3 of the 12 controls, 'does not survive'.")
    print(f"           observed: {len(fire10)}/{len(c10)} controls fire at >= 0.5 "
          f"-> {'MET' if len(fire10) >= 3 else 'NOT MET'}")
    for nm in ("CTL10-11", "CTL10-13"):
        if ("RTD-10", nm) in rate:
            print(f"           {nm} (near-miss, the discriminating cell): "
                  f"fire rate {rate[('RTD-10', nm)]:.2f}")
    c9 = [p for p, d in labels.items() if p.startswith("CTL9-") and not d["is_positive"]]
    print(f"  RTD-9  — registered: report full-shape and near-miss SEPARATELY.")
    print(f"           full-shape {cell('RTD-9', [p for p in c9 if labels[p]['shape']=='full-shape'])}"
          f"   near-miss {cell('RTD-9', [p for p in c9 if labels[p]['shape']=='near-miss'])}")
    print("  contradiction — NOT SCOREABLE as extracted: its passages are joined across")
    print("           a gap that removes the material the contradiction depends on.")

    if withheld:
        print(f"\n  {withheld} row(s) withheld on panel divergence — counted out of "
              f"every rate above, never as a NO.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
