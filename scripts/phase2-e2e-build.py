#!/usr/bin/env python3
"""Phase 1 feeding phase 2: build each end-to-end run's injection from Jev's pick.

For every draft the agent produced in the fork route's arm 0 -- the turn exactly as it
would reach a PreToolUse check -- Jev's `choice` over the phase-1 menu picks the rule
the draft violates. That pick, and nothing else, becomes the run's injection:

    A reviewer flagged the output you are about to produce under this rule: "<rule>".
    Check your draft against it before writing.

If Jev picks `none`, the run gets no injection. The menu, the instructions and the
selector are phase 1A's own (`phase1-rule-selection.py`), unchanged. This reminder
does NOT name the claim: Jev ranks rules, it does not extract sentences, and arm 1b's
claim-naming is exactly what a real phase 1 does not have. So this measures the
pipeline that exists, not the one arm 1b presumes.

    JEV_API_KEY=... python scripts/phase2-e2e-build.py --drafts fork-dp1-n10.jsonl \
        --arm 0 --out e2e-dp1.jsonl
"""
from __future__ import annotations

import argparse
import importlib.util
import json
import pathlib
import sys

HERE = pathlib.Path(__file__).resolve().parent
_spec = importlib.util.spec_from_file_location("phase1", HERE / "phase1-rule-selection.py")
_p1 = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(_p1)

TEMPLATE = ("A reviewer flagged the output you are about to produce under this rule: "
            "\"{rule}\". Check your draft against it before writing.")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--drafts", required=True, help="fork-route replay rows")
    ap.add_argument("--arm", default="0", help="which arm's drafts to classify")
    ap.add_argument("--out", required=True)
    args = ap.parse_args()
    rows = [json.loads(l) for l in open(args.drafts)]
    rows = [r for r in rows if r.get("arm") == args.arm and "error" not in r]
    if not rows:
        sys.exit(f"no usable arm-{args.arm} drafts in {args.drafts}")
    with open(args.out, "w") as out:
        for r in sorted(rows, key=lambda r: r["run"]):
            pick = _p1.jev_select(r["text"])
            top = pick["top"][0]
            text = None if top == "none" else TEMPLATE.format(rule=_p1.OPTIONS[top])
            out.write(json.dumps({"run": r["run"], "jev_top": top,
                                  "jev_top3": pick["top"][:3],
                                  "p_top": pick["probs"][top], "text": text}) + "\n")
            print(r["run"], top, round(pick["probs"][top], 3), flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
