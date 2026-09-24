#!/usr/bin/env python3
"""Local route, Stage 1, arm L0-frozen: JevK5 zero-shot as the per-rule judge.

Registered in docs/evals/phase1-local-classifier-preregistration.md (Stage 1, and the
Stage-1 execution amendment). It reuses scripts/phase1-span-selector.py's gate and span gate
UNCHANGED and swaps only `judge_rule`. So the texts, the pass criteria, the span check and the
claim-on-target check are the very code S0 was scored by, not a copy of it.

Per (text, rule):
  1. `noul`: does the text break this rule in the way the rule's violation-shape spec
     describes? p(true) < THRESHOLD means NO.
  2. `choice` over the text's sentences, with no `none` option. JevK5 0.2.2 reads more than
     16 options in groups of 16 plus a final. The argmax sentence is the claim, copied verbatim.

JevK5 is one forward pass with no sampling, so a run is deterministic; `--check-determinism`
asserts it rather than assuming it.

Run with the JevK5 venv:
  ~/work/claude/jevk5/.venv/bin/python -u scripts/phase1-local-l0.py --gate --log l0-gate.jsonl
"""

import argparse
import importlib.util
import json
import pathlib
import re
import sys

HERE = pathlib.Path(__file__).resolve().parent
_spec = importlib.util.spec_from_file_location("selector", HERE / "phase1-span-selector.py")
sel = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(sel)

THRESHOLD = 0.5          # registered; not tuned
MODEL_ID = "alibiserikbay/JevK5"
_model = None
_log: list[dict] = []


def sentences(text: str) -> list[str]:
    """Sentence candidates for the claim. Shorter than the span check's MIN_SPAN cannot be a
    claim (verify_span refuses them), so they are not offered as options."""
    parts = re.split(r"(?<=[.!?])\s+|\n+", text)
    return [p.strip() for p in parts if len(p.strip()) >= sel.MIN_SPAN]


def noul_question(rule: str) -> dict:
    return {"type": "noul",
            "instructions": (f'Rule: "{sel.RULES[rule]}" It is broken when: {sel.SPECS[rule]} '
                             "Does the text itself break this rule in the way described?")}


def judge_rule(text: str, rule: str, retries: int = 1) -> dict:
    p = _model.decide(text, noul_question(rule))["noul"]
    row = {"rule": rule, "noul": round(p, 6)}
    if p < THRESHOLD:
        _log.append({**row, "verdict": "NO"})
        return {"rule": rule, "verdict": "NO"}
    opts = sentences(text)
    if not opts:
        raise ValueError("noul fired but the text has no sentence long enough to be a claim")
    if len(opts) == 1:
        claim = opts[0]
    else:
        q = {"type": "choice",
             "instructions": f'Which sentence breaks the rule "{sel.RULES[rule]}"?',
             "criteria": {f"s{i}": s for i, s in enumerate(opts)}}
        claim = opts[int(_model.decide(text, q)["choice"][1:])]
    span = sel.verify_span(claim, text)
    if not span:  # cannot happen for a sentence cut from the text; raised, never downgraded
        raise ValueError(f"chosen sentence failed verify_span: {claim[:80]!r}")
    _log.append({**row, "verdict": "YES", "claim": span})
    return {"rule": rule, "verdict": "YES", "claim": span}


def main() -> int:
    global _model
    ap = argparse.ArgumentParser()
    mode = ap.add_mutually_exclusive_group(required=True)
    mode.add_argument("--gate", action="store_true")
    mode.add_argument("--span-gate", action="store_true")
    mode.add_argument("--check-determinism", action="store_true")
    ap.add_argument("--runs", type=int, default=1)
    ap.add_argument("--log", help="write every (rule, noul probability, verdict) row as JSONL")
    args = ap.parse_args()
    args.pool = 1        # one model on one GPU: sweep's thread pool must not run it concurrently

    from jevk5 import JevK5
    _model = JevK5(MODEL_ID)
    sel.judge_rule = judge_rule   # the ONLY substitution; gate/span_gate/sweep are sel's own
    print(f"judge: {MODEL_ID} local, noul threshold {THRESHOLD}", flush=True)

    if args.check_determinism:
        # A neutral text in no gate, corpus or held-out set: probing a gate text before the
        # threshold is registered would be reading the answers early.
        cid, text = "neutral", ("The build script copies the assets into dist/. It then runs "
                                "the bundler with the production flag and writes a manifest.")
        a = [_model.decide(text, noul_question(r))["noul"] for r in sel.RULES]
        b = [_model.decide(text, noul_question(r))["noul"] for r in sel.RULES]
        drift = max(abs(x - y) for x, y in zip(a, b))
        print(f"determinism on {cid}: max |dp| over {len(a)} rules = {drift:.2e}")
        return 0 if drift < 1e-4 else 1

    rc = sel.gate(args) if args.gate else sel.span_gate(args)
    if args.log:
        with open(args.log, "w") as fh:
            fh.writelines(json.dumps(r) + "\n" for r in _log)
    return rc


if __name__ == "__main__":
    sys.exit(main())
