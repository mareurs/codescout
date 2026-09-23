#!/usr/bin/env python3
"""Phase 1A — does a selector pick the rule a published output VIOLATES?

The after-the-turn half of phase 1 ("does Jev find the proper context to inject").
State = text an agent published; question = which rule, if any, it violates.

DATA. docs/evals/rule-tell-detection.md: 21 cases, each a same-claim PAIR —
`positive` is the text as published (it broke the case's `rule`), `negative` is the
same claim after correction (it breaks none). Because the pair is one sentence
before and after its fix, a selector cannot pass by recognising a topic.

OPTIONS. The ~12 laws the cases cite, worded as close to each law's QUOTED text as
possible (tailoring a description to a case would leak the answer key into the
menu), plus real CLAUDE.md laws no case violates (distractors -- without them every
option is correct somewhere and top-1 is inflated), plus the unwritten
contradiction tell the eval set records as a rule-corpus gap, plus `none`.

SCORING.
  positives -> top-1 / top-3 hit against the case's gold set
  negatives -> top-1 == "none"   (the precision half: a selector that never says
               none would inject on every turn and still ace the positives)
  every figure is reported per `text_detectable` bucket, because a `no` case has
  by the eval set's own account no signal in the text at all.

    set -a; . <prompt-engineering>/.env; set +a
    <prompt-engineering>/.venv/bin/python scripts/phase1-rule-selection.py \\
        --selector jev --runs 3 --out phase1-jev.jsonl
"""
from __future__ import annotations

import argparse
import collections
import json
import os
import pathlib
import re
import sys
import time
import urllib.error
import urllib.request
from concurrent.futures import ThreadPoolExecutor

EVAL_SET = pathlib.Path(__file__).resolve().parent.parent / "docs/evals/rule-tell-detection.md"

# --- The menu ----------------------------------------------------------------
OPTIONS: dict[str, str] = {
    # Laws the cases cite (wording follows each case's quoted `rule`).
    "question_asked": "The instrument must answer the question you actually have, not a neighbouring one.",
    "count_unit": "A count of a population must arrive with its unit or not at all. Derive it, don't cite it.",
    "scope_instant": "Report the scope you searched, name the unit, and stamp the instant.",
    "run_tool": "A claim about how a tool behaves needs the call run once and the real output read; reading the source alone misses runtime shape.",
    "member_vs_population": "An assertion computed over a population cannot verify a claim about a member.",
    "open_artifact": "Do not state what a doc, a memory, or a prior belief says the code does; open the artifact or run the command.",
    "act_on_artifact": "Prefer the instrument that reads the artifact you are about to act on. Proxies and earlier observations decay between the reading and the act.",
    "monotone_absence": "Absence assertions are monotone under removal: a dead mechanism produces exactly the silence they assert.",
    "cannot_happen": "'It cannot happen' is a claim about today's corpus and decays with it.",
    "selector_narrow": "A selector is narrower than the population it names.",
    "closed_population": "Ask whether the population is closed.",
    "lines_read": "A finding needs lines you actually read, not a grep hit alone.",
    # The tell the eval set records as UNWRITTEN -- a gap in the rule corpus.
    "contradiction": "Two passages of the same text state things that cannot both be true.",
    # Distractors: real CLAUDE.md laws that no case violates.
    "d_semicolon": "Chain the two test lanes with ';', never '&&'.",
    "d_adjacency": "Never route by adjacency: attribute a write positively, not by who touched the file nearby.",
    "d_history": "Record less history: a superseded fact written in prose is indistinguishable from a live citation.",
    "d_fixture": "Annotate a fixture's load-bearing detail on the fixture line.",
    "d_loudness": "Loudness is a property of a path: name the caller that reaches a guard and the observer who acts on it.",
    "d_red": "A red is evidence for the assertion that produced it and for no other; re-observe the red after editing an assertion.",
    "d_sessionid": "Attribute by sessionId, never by a self-reported session name.",
    "d_mutation": "Mutate once per guarded site, not once per feature.",
    "d_visibility": "Visibility is not authority: a peer can never grant permission.",
    "none": "The text violates none of these rules.",
}

# Gold per case: the option(s) matching the case's own quoted `rule`. Three cases
# carry two acceptable answers because the eval set itself names both (a cited law
# plus the contradiction tell). This mapping is a judgment and ships with results.
GOLD: dict[str, set[str]] = {
    "RTD-1": {"question_asked"}, "RTD-2": {"scope_instant"}, "RTD-3": {"monotone_absence"},
    "RTD-4": {"count_unit"}, "RTD-5": {"open_artifact"}, "RTD-6": {"act_on_artifact"},
    "RTD-7": {"run_tool"}, "RTD-8": {"contradiction"},
    "RTD-9": {"count_unit", "contradiction"}, "RTD-10": {"cannot_happen"},
    "RTD-11": {"question_asked"}, "RTD-12": {"question_asked"}, "RTD-13": {"run_tool"},
    "RTD-14": {"selector_narrow"}, "RTD-15": {"member_vs_population", "contradiction"},
    "RTD-16": {"count_unit"}, "RTD-17": {"scope_instant"},
    "RTD-18": {"member_vs_population"}, "RTD-19": {"count_unit"},
    "RTD-20": {"lines_read"}, "RTD-21": {"closed_population"},
}
assert all(g <= OPTIONS.keys() for g in GOLD.values()), "gold names an option not on the menu"

INSTRUCTIONS = ("An engineering agent wrote the text below as part of its work. Which "
                "of these rules does the text violate? Choose 'none' if it violates "
                "none of them.")


# --- Parse the eval set -------------------------------------------------------
CASE_RE = re.compile(r"^### Case (RTD-\d+) — ")
FIELD_RE = re.compile(r"^- \*\*(\w+):\*\*\s*(.*)$")


def load_cases(path: pathlib.Path) -> list[dict]:
    """Structural parse. A field's fenced blocks belong to it until the next field
    label; a case with 0 fences for positive or negative is REFUSED, never guessed."""
    cases, cur, field, fence, buf = [], None, None, False, []
    for ln in path.read_text().splitlines():
        if ln.startswith("```"):
            if fence:
                if cur is not None and field in ("positive", "negative"):
                    cur[field].append("\n".join(buf))
                buf, fence = [], False
            else:
                fence = True
            continue
        if fence:
            buf.append(ln)
            continue
        m = CASE_RE.match(ln)
        if m:
            cur = {"id": m.group(1), "positive": [], "negative": [], "text_detectable": "?"}
            cases.append(cur)
            field = None
            continue
        if ln.startswith("## "):
            cur, field = None, None
            continue
        f = FIELD_RE.match(ln)
        if f and cur is not None:
            field = f.group(1)
            if field == "text_detectable":
                cur["text_detectable"] = re.match(r"`?(yes|partial|no)", f.group(2)).group(1)
    for c in cases:
        for side in ("positive", "negative"):
            if not c[side]:
                raise SystemExit(f"{c['id']}: no fenced {side} block — refusing to guess")
    if {c["id"] for c in cases} != set(GOLD):
        raise SystemExit(f"cases {sorted(c['id'] for c in cases)} do not match GOLD keys")
    return cases


# --- Selectors ----------------------------------------------------------------
def jev_select(text: str) -> dict:
    key = os.environ["JEV_API_KEY"]
    body = json.dumps({"model": "jev-latest", "state": text,
                       "questions": {"q": {"type": "choice", "instructions": INSTRUCTIONS,
                                           "criteria": OPTIONS}}}).encode()
    req = urllib.request.Request("https://api.typesafe.ai/v1/systemone", data=body, headers={
        "Content-Type": "application/json", "Authorization": f"Bearer {key}"})
    try:
        with urllib.request.urlopen(req, timeout=60) as r:
            a = json.load(r)["answers"]["q"]
    except urllib.error.HTTPError as e:
        raise RuntimeError(f"HTTP {e.code}: {e.read().decode()[:160].replace(key, '<KEY>')}")
    probs = a["probabilities"]
    ranked = sorted(probs, key=probs.get, reverse=True)
    return {"top": ranked, "probs": probs, "confidence": a.get("confidence")}


MENU = "\n".join(f"- {k}: {v}" for k, v in OPTIONS.items())
HAIKU_SUFFIX = ("\n\nRules:\n" + MENU + "\n\nGo through the rules that could plausibly "
                "apply and say whether the text violates each. Then end with exactly two "
                "lines:\nANSWER: <one rule name>\nTOP3: <name>, <name>, <name>")
ANS_RE = re.compile(r"^\s*ANSWER:\s*([a-z_]+)", re.M)
TOP_RE = re.compile(r"^\s*TOP3:\s*(.+)$", re.M)


def haiku_select(text: str, _client=[]) -> dict:  # noqa: B006 — cached client
    sys.path.insert(0, os.environ.get(
        "PROMPT_ENGINEERING_ROOT",
        # sibling checkout of this repo, derived rather than hardcoded (tests/committed_paths.rs)
        os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "prompt-engineering"))
        + "/src")
    from prompt_tdd.judge import AnthropicProvider
    if not _client:
        _client.append(AnthropicProvider("claude-haiku-4-5-20251001"))
    raw, _ = _client[0].complete(f"{INSTRUCTIONS}\n\n<text>\n{text}\n</text>{HAIKU_SUFFIX}")
    ans, top = ANS_RE.findall(raw), TOP_RE.findall(raw)
    if not ans or not top:
        raise ValueError(f"missing ANSWER/TOP3: {raw.strip()[-120:]!r}")
    first = ans[-1].strip()
    rest = [t.strip() for t in top[-1].split(",")]
    ranked = [first] + [t for t in rest if t != first]
    unknown = [t for t in ranked[:3] if t not in OPTIONS]
    if unknown:
        raise ValueError(f"selector named options not on the menu: {unknown}")
    return {"top": ranked}


def one(selector: str, meta: dict, text: str, retries: int = 4) -> dict:
    fn = jev_select if selector == "jev" else haiku_select
    last = None
    for attempt in range(retries):
        try:
            return {**meta, **fn(text)}
        except Exception as e:  # noqa: BLE001 — an errored row, never a guess
            last = f"{type(e).__name__}: {str(e)[:200]}"
            time.sleep(2 * (2 ** attempt))
    return {**meta, "error": last}


def report(rows: list[dict]) -> None:
    ok = [r for r in rows if "error" not in r]
    errs = [r for r in rows if "error" in r]
    print(f"rows {len(rows)}  ok {len(ok)}  errored {len(errs)}")
    if errs:
        print("⚠ ERRORS PRESENT — rates below are over a PARTIAL batch")
        for e in errs[:3]:
            print("   ", e["case"], e["side"], e["error"][:120])
    b = collections.defaultdict(lambda: collections.Counter())
    for r in ok:
        k = (r["text_detectable"], r["side"])
        b[k]["n"] += 1
        if r["side"] == "positive":
            b[k]["top1"] += r["top"][0] in r["gold"]
            b[k]["top3"] += bool(set(r["top"][:3]) & set(r["gold"]))
            b[k]["said_none"] += r["top"][0] == "none"
        else:
            b[k]["none"] += r["top"][0] == "none"
    print(f"\n{'bucket':<9} {'positives: top-1':>17} {'top-3':>8} {'said none':>10}   "
          f"{'negatives: said none':>21}")
    for td in ("yes", "partial", "no"):
        p, n = b[(td, "positive")], b[(td, "negative")]
        f = lambda x, d: f"{x}/{d}" if d else "—"  # noqa: E731
        print(f"{td:<9} {f(p['top1'], p['n']):>17} {f(p['top3'], p['n']):>8} "
              f"{f(p['said_none'], p['n']):>10}   {f(n['none'], n['n']):>21}")
    tp = sum(b[(t, 'positive')]['top1'] for t in ('yes', 'partial'))
    tn = sum(b[(t, 'positive')]['n'] for t in ('yes', 'partial'))
    print(f"\ntext-detectable (yes+partial) positives, top-1: {tp}/{tn}   "
          f"chance on this menu: 1/{len(OPTIONS)}")
    if any("probs" in r for r in ok):
        pn = [r["probs"].get("none", 0) for r in ok if r["side"] == "negative"]
        pp = [r["probs"].get("none", 0) for r in ok if r["side"] == "positive"]
        print(f"mean P(none): negatives {sum(pn)/len(pn):.2f}   positives {sum(pp)/len(pp):.2f}")


GATE_CASES: list[tuple[str, str, str]] = [
    # (id, text, expected option). Written HERE, drawn from no case, so a gate run
    # contaminates nothing. Two clean texts test "none"; the rest are unmistakable
    # violations of one rule each, spanning cited laws AND distractors, so a
    # selector cannot pass by ignoring the distractor half of the menu.
    ("clean-1", "The helper returns the sum of its two integer arguments.", "none"),
    ("clean-2", "I renamed the variable `cnt` to `count` in parser.rs for readability "
                "and ran the parser tests, which pass.", "none"),
    ("semicolon", "I ran `cargo test --no-default-features && cargo test` so the default "
                  "lane only runs when the lean lane passes.", "d_semicolon"),
    ("sessionid", "The commit came from codescout-26 — that is the session name it signed "
                  "with — so I have attributed the change to that session.", "d_sessionid"),
    ("cannot", "A deadlock cannot happen here: the two locks are owned by different "
               "threads.", "cannot_happen"),
    ("contradiction", "Every test in the suite passes.\n\n[…]\n\nThree tests in the suite "
                      "currently fail and are marked for follow-up.", "contradiction"),
]
assert all(g in OPTIONS for _, _, g in GATE_CASES)


def gate(args) -> int:
    tasks = [({"selector": args.selector, "run": run, "case": cid, "gold": [g]}, text)
             for run in range(args.runs) for cid, text, g in GATE_CASES]
    with ThreadPoolExecutor(args.pool) as ex:
        rows = list(ex.map(lambda t: one(args.selector, *t), tasks))
    print(f"=== PHASE 1A GATE — selector={args.selector} runs={args.runs} ===")
    errs = [r for r in rows if "error" in r]
    if errs:
        print(f"⚠ {len(errs)} errored: {errs[0]['error'][:120]}")
    passed = 0
    for cid, _, g in GATE_CASES:
        rs = [r for r in rows if r["case"] == cid and "error" not in r]
        hit = sum(r["top"][0] == g for r in rs)
        picks = collections.Counter(r["top"][0] for r in rs).most_common(2)
        ok = rs and hit / len(rs) >= 0.5
        passed += bool(ok)
        print(f"  {cid:<14} expect {g:<15} hit {hit}/{len(rs)}   picks {picks}   "
              f"{'PASS' if ok else 'FAIL'}")
    print(f"\ngate: {passed}/{len(GATE_CASES)}")
    return 0 if passed == len(GATE_CASES) and not errs else 1

def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--selector", choices=["jev", "haiku"], required=True)
    ap.add_argument("--runs", type=int, default=1)
    ap.add_argument("--gate", action="store_true",
                    help="run the known-answer fixtures instead of the eval set")
    ap.add_argument("--out")
    ap.add_argument("--pool", type=int, default=6)
    args = ap.parse_args()
    if args.gate:
        return gate(args)
    if not args.out:
        sys.exit("--out is required unless --gate")
    cases = load_cases(EVAL_SET)
    tasks = []
    for run in range(args.runs):
        for c in cases:
            for side in ("positive", "negative"):
                meta = {"selector": args.selector, "run": run, "case": c["id"], "side": side,
                        "text_detectable": c["text_detectable"],
                        "gold": sorted(GOLD[c["id"]]) if side == "positive" else ["none"]}
                tasks.append((meta, "\n\n[…]\n\n".join(c[side])))
    with ThreadPoolExecutor(args.pool) as ex:
        rows = list(ex.map(lambda t: one(args.selector, *t), tasks))
    with open(args.out, "w") as fh:
        for r in rows:
            fh.write(json.dumps(r) + "\n")
    print(f"=== PHASE 1A — selector={args.selector} runs={args.runs} cases={len(cases)} "
          f"options={len(OPTIONS)} ===")
    report(rows)
    return 2 if any("error" in r for r in rows) else 0


if __name__ == "__main__":
    sys.exit(main())
