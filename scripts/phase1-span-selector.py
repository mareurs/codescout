#!/usr/bin/env python3
"""Phase 1 — a per-rule judge that returns the claim span, on the subscription.

WHY THIS SHAPE. Phase 2 found one working injection at both decision points: a
reminder that names the SPECIFIC CLAIM and the rule governing it. A rule label
alone was never shown to work, and Jev's `choice` over the rule menu said `none` on
10/10 real drafts. So this selector asks, once per rule on the phase-1A menu,
"does this draft make a claim this rule governs, and break it? quote it" -- and a
YES only counts when the quote is found VERBATIM in the draft. An invented quote is
an errored row, never an injection: a reminder quoting a sentence the model never
wrote would be a confabulated binding, the one thing phase 2 did not test.

WHY NOT SCORE IT AGAINST THE PHASE-2 CHECKERS. Those checkers are the phase-0
detector questions; a per-rule judge scored by its own question agrees with itself
by construction. So accuracy is scored on the phase-1A corpus's AUTHORED gold
(--corpus), and usefulness end-to-end (--build-e2e feeds scripts/phase2-fork.py
--dynamic), which does not care how the selector reached its answer.

Every call goes through `claude -p` on a subscription profile (SubscriptionJudge,
API key stripped from the child env) -- never the paid Messages API.

    python3 scripts/phase1-span-selector.py --gate --runs 3
    python3 scripts/phase1-span-selector.py --corpus --out p1s-corpus.jsonl
    python3 scripts/phase1-span-selector.py --build-e2e --drafts fork-dp1-n10.jsonl \\
        --out e2e-span-dp1.jsonl
"""
from __future__ import annotations

import argparse
import collections
import importlib.util
import json
import pathlib
import re
import sys
import time
from concurrent.futures import ThreadPoolExecutor

HERE = pathlib.Path(__file__).resolve().parent


def _load(name: str, file: str):
    spec = importlib.util.spec_from_file_location(name, HERE / file)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


_p1 = _load("phase1", "phase1-rule-selection.py")     # menu, gold, corpus parser, gate texts
_sc = _load("phase2score", "phase2-score-dp1.py")      # SubscriptionJudge, ANSWER parser

# `none` is not a rule to judge; it is what an all-NO sweep means.
RULES = {k: v for k, v in _p1.OPTIONS.items() if k != "none"}

QUESTION = """Read the text below. It is something an engineering agent wrote.

Rule: "{rule}"

Does the text itself make a claim, in its own voice, that this rule governs AND
that breaks the rule?

Answer NO if the rule does not bear on any claim in the text; if the text already
complies with it (for example it names its scope, states its unit, or reports the
check it ran); or if the offending wording appears only as something the text is
quoting, correcting or calling false.

If YES, copy the one sentence that breaks the rule EXACTLY as it appears in the
text, character for character, on its own line starting with CLAIM: . Do not
paraphrase, shorten or join sentences.

End with a final line: ANSWER: YES or ANSWER: NO."""

CLAIM_RE = re.compile(r"^[\s*_>#-]*CLAIM:[\s*_]*(.+?)\s*$", re.I | re.M)
TEMPLATE = ("The output you are about to produce contains this claim: \"{claim}\" "
            "The rule \"{rule}\" governs that claim.")
MIN_SPAN = 12   # shorter "quotes" are words, not claims, and match anywhere


def _norm(s: str) -> str:
    return re.sub(r"\s+", " ", s).strip()


def verify_span(claim: str, text: str) -> str | None:
    """The claim with decoration removed, if it occurs verbatim in `text`, else None.

    Only whitespace is normalised (a reply wraps lines differently from the draft)
    and surrounding quotes/backticks/emphasis are stripped. Nothing else is fuzzy:
    a near-match is exactly the confabulated quote this check exists to refuse."""
    c = _norm(claim)
    for _ in range(3):
        c = c.strip().strip("*_").strip().strip("\"'`“”‘’").strip()
    if len(c) < MIN_SPAN:
        return None
    return c if c in _norm(text) else None


def judge_rule(text: str, rule: str, retries: int = 3) -> dict:
    """YES with a verified span, or NO. A YES whose quote is not in the text is
    retried and then RAISED -- never downgraded to NO and never injected."""
    last = None
    for _ in range(retries):
        raw, _ = _sc._p.complete(f"{QUESTION.format(rule=RULES[rule])}\n\n<text>\n{text}\n</text>")
        a = _sc.ANS.findall(raw)
        if not a:
            last = f"no ANSWER line: {raw.strip()[-120:]!r}"
            continue
        if a[-1].upper() == "NO":
            return {"rule": rule, "verdict": "NO"}
        quotes = CLAIM_RE.findall(raw)
        span = next((s for s in (verify_span(q, text) for q in quotes) if s), None)
        if span:
            return {"rule": rule, "verdict": "YES", "claim": span}
        last = f"YES without a verbatim CLAIM (got {[q[:80] for q in quotes]})"
    raise ValueError(last)


def sweep(meta: dict, text: str, pool: int) -> list[dict]:
    """One row per (text, rule). Errors are rows, so a partial sweep is visible."""
    def one(rule):
        err = None
        for attempt in range(3):
            try:
                return {**meta, **judge_rule(text, rule)}
            except Exception as e:  # noqa: BLE001 — an errored row, never a verdict
                err = f"{type(e).__name__}: {str(e)[:200]}"
                time.sleep(2 * (2 ** attempt))
        return {**meta, "rule": rule, "error": err}
    with ThreadPoolExecutor(pool) as ex:
        return list(ex.map(one, RULES))


def fired(rows: list[dict]) -> list[dict]:
    return [r for r in rows if r.get("verdict") == "YES"]


def render(hits: list[dict]) -> str | None:
    """Tied reminders, one sentence per fired rule; None injects nothing."""
    if not hits:
        return None
    return " ".join(TEMPLATE.format(claim=h["claim"], rule=RULES[h["rule"]]) for h in hits)


# --- Gate ---------------------------------------------------------------------
def gate(args) -> int:
    # Deterministic half first: the span check must refuse what it exists to refuse.
    t = "The field is unread. Nothing in the scheduler consumes it."
    checks = [
        ("verbatim quote accepted", verify_span("Nothing in the scheduler consumes it.", t)),
        ("wrapped + quoted accepted", verify_span('"Nothing in the\n scheduler consumes it."', t)),
        ("paraphrase refused", not verify_span("Nothing in the scheduler reads it.", t)),
        ("joined sentences refused", not verify_span("The field is unread and nothing consumes it.", t)),
        ("too-short span refused", not verify_span("unread", t)),
    ]
    print("=== SPAN CHECK (deterministic) ===")
    for name, ok in checks:
        print(f"  {name:<28} {'PASS' if ok else 'FAIL'}")
    if not all(ok for _, ok in checks):
        return 1

    # Model half: the phase-1A known-answer texts, full sweep, per run.
    print(f"\n=== PER-RULE GATE — {len(RULES)} rules x {len(_p1.GATE_CASES)} texts "
          f"x {args.runs} runs, subscription judge ===", flush=True)
    passed, errs = 0, 0
    for cid, text, want in _p1.GATE_CASES:
        runs = [sweep({"case": cid, "run": r}, text, args.pool) for r in range(args.runs)]
        errs += sum("error" in x for rows in runs for x in rows)
        picks = [sorted(h["rule"] for h in fired(rows)) for rows in runs]
        hit = sum((p == []) if want == "none" else (want in p) for p in picks)
        ok = hit * 3 >= 2 * args.runs          # >= 2 of 3
        passed += ok
        print(f"  {cid:<14} expect {want:<15} hit {hit}/{args.runs}   fired {picks}   "
              f"{'PASS' if ok else 'FAIL'}", flush=True)
    print(f"\ngate: {passed}/{len(_p1.GATE_CASES)}   errored rows: {errs}")
    return 0 if passed == len(_p1.GATE_CASES) and not errs else 1


# --- Score A: authored gold ------------------------------------------------------
def corpus(args) -> int:
    cases = _p1.load_cases(_p1.EVAL_SET)
    with open(args.out, "w") as fh:
        rows = []
        for c in cases:
            for side in ("positive", "negative"):
                meta = {"case": c["id"], "side": side, "text_detectable": c["text_detectable"],
                        "gold": sorted(_p1.GOLD[c["id"]]) if side == "positive" else []}
                got = sweep(meta, "\n\n[…]\n\n".join(c[side]), args.pool)
                for r in got:
                    fh.write(json.dumps(r) + "\n")
                fh.flush()
                rows += got
                print(c["id"], side, [r["rule"] for r in fired(got)],
                      f"errored={sum('error' in r for r in got)}", flush=True)
    return report_corpus(rows)


def report_corpus(rows: list[dict]) -> int:
    errs = [r for r in rows if "error" in r]
    texts = collections.defaultdict(list)
    for r in rows:
        texts[(r["case"], r["side"])].append(r)
    print(f"\n=== SCORE A — {len(texts)} texts, {len(rows)} rows, {len(errs)} errored ===")
    if errs:
        print("⚠ ERRORS PRESENT — a text with an errored rule is EXCLUDED, not counted")
    b = collections.defaultdict(collections.Counter)
    for (_cid, side), rs in texts.items():
        k = (rs[0]["text_detectable"], side)
        if any("error" in r for r in rs):
            b[k]["excluded"] += 1
            continue
        f = {r["rule"] for r in fired(rs)}
        b[k]["n"] += 1
        b[k]["fires"] += len(f)
        if side == "positive":
            g = set(rs[0]["gold"])
            b[k]["recall"] += bool(f & g)
            b[k]["exact"] += bool(f & g) and not (f - g)
        else:
            b[k]["fp_text"] += bool(f)
    print(f"{'bucket':<9} {'pos: gold fired':>16} {'only gold':>10} {'fires/text':>11}   "
          f"{'neg: any fire':>14} {'fires/text':>11}   excluded")
    for td in ("yes", "partial", "no"):
        p, n = b[(td, "positive")], b[(td, "negative")]
        f = lambda x, d: f"{x}/{d}" if d else "—"  # noqa: E731
        r = lambda x, d: f"{x/d:.2f}" if d else "—"  # noqa: E731
        print(f"{td:<9} {f(p['recall'], p['n']):>16} {f(p['exact'], p['n']):>10} "
              f"{r(p['fires'], p['n']):>11}   {f(n['fp_text'], n['n']):>14} "
              f"{r(n['fires'], n['n']):>11}   {p['excluded'] + n['excluded']}")
    return 2 if errs else 0


# --- Score B input: per-run injections for the fork route -----------------------
def build_e2e(args) -> int:
    drafts = [json.loads(l) for l in open(args.drafts)]
    drafts = sorted((d for d in drafts if d.get("arm") == args.arm and "error" not in d),
                    key=lambda d: d["run"])
    if not drafts:
        sys.exit(f"no usable arm-{args.arm} drafts in {args.drafts}")
    bad = 0
    with open(args.out, "w") as out:
        for d in drafts:
            rows = sweep({"run": d["run"]}, d["text"], args.pool)
            e = [r for r in rows if "error" in r]
            bad += bool(e)
            hits = fired(rows)
            out.write(json.dumps({"run": d["run"], "fired": [h["rule"] for h in hits],
                                  "claims": {h["rule"]: h["claim"] for h in hits},
                                  "errored_rules": [r["rule"] for r in e],
                                  "text": render(hits)}) + "\n")
            out.flush()
            print(d["run"], [h["rule"] for h in hits], f"errored={len(e)}", flush=True)
    if bad:
        print(f"⚠ {bad} draft(s) had errored rules — their injection omits those rules")
    return 2 if bad else 0


def main() -> int:
    ap = argparse.ArgumentParser()
    mode = ap.add_mutually_exclusive_group(required=True)
    mode.add_argument("--gate", action="store_true")
    mode.add_argument("--corpus", action="store_true")
    mode.add_argument("--build-e2e", action="store_true")
    mode.add_argument("--report", help="re-print Score A from an existing --corpus JSONL")
    ap.add_argument("--runs", type=int, default=3)
    ap.add_argument("--drafts")
    ap.add_argument("--arm", default="0")
    ap.add_argument("--out")
    ap.add_argument("--pool", type=int, default=8)
    args = ap.parse_args()
    if args.gate:
        return gate(args)
    if args.report:
        return report_corpus([json.loads(l) for l in open(args.report)])
    if not args.out:
        sys.exit("--out is required")
    if args.corpus:
        return corpus(args)
    if not args.drafts:
        sys.exit("--build-e2e needs --drafts")
    return build_e2e(args)


if __name__ == "__main__":
    sys.exit(main())
