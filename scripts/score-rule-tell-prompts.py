#!/usr/bin/env python3
"""Score the rule-tell detector prompts with prompt-engineering's cross-family panel.

Two modes, and `mutation` GATES `score`:

  mutation  15 hand-written rows (5 prompts x clear-YES / clear-NO / realistic
            near-miss). Confirms the judge can SPLIT before any corpus row is
            spent. skill-eval-playbook:L-13 — a blatant all-wrong mutant is too
            easy to catch; the near-miss is where a weak rubric silently passes.
            These rows are written here, not drawn from the corpus, so running
            this contaminates nothing.

  score     the 285 blind tasks (5 prompts x 57 passages, full cross product).
            Emits one JSONL row per task so response<->score stays bindable
            (prompt-tdd-operating-guide OP-4: nothing persists a per-run record).

WHY A PANEL. PanelJudge treats cross-family disagreement as a calibration signal,
never a vote: judges[0] (claude) is the authority, gemini can raise doubt but can
never override, and a spread past max_spread WITHHOLDS the grade rather than
averaging it. cross-family-panel-calibration pins max_spread=0.25 in a measured
empty valley at n=64 -- but its own refresh rule says thresholds do not transfer
corpora, so this run REPORTS its own spread distribution and does not refit the
gate after seeing it.

THE ADAPTER. RUBRIC_PROMPT opens "You are evaluating an AI agent's response",
which is false of these passages. One fixed suffix, byte-identical across all
five prompts, corrects that and maps YES/NO onto the 0-1 scale. It deliberately
PERMITS an intermediate score: forcing a binary rails every row at 0/1 and the
spread reads 0.000, which is exactly the rails-only null L-1 warns is a green bar
hiding the risky region.

    PE="${PROMPT_ENGINEERING_ROOT:-$(cd "$(dirname "$0")/../.." && pwd)/prompt-engineering}"
    set -a; . "$PE/.env"; set +a
    "$PE/.venv/bin/python" \
        scripts/score-rule-tell-prompts.py --mode mutation
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
from concurrent.futures import ThreadPoolExecutor

# Sibling checkout of this repo, derived rather than hardcoded (tests/committed_paths.rs).
PE = os.environ.get("PROMPT_ENGINEERING_ROOT", os.path.join(
    os.path.dirname(os.path.abspath(__file__)), "..", "..", "prompt-engineering"))
if not pathlib.Path(PE, "src", "prompt_tdd", "judge.py").is_file():
    sys.exit(f"prompt_tdd not found under {PE}/src — set PROMPT_ENGINEERING_ROOT")
sys.path.insert(0, str(pathlib.Path(PE, "src")))

from prompt_tdd.judge import AnthropicProvider, GeminiProvider, LLMJudge, PanelJudge

CLAUDE_MODEL = "claude-haiku-4-5-20251001"
GEMINI_MODEL = "gemini-2.5-flash"

# Byte-identical across all five prompts. Appended, never interleaved, so each
# prompt reaches the judge verbatim above it.
ADAPTER = """

---
The text you are scoring is an excerpt from a project document: prose written by
engineers about their own codebase. It is NOT a reply produced by an AI agent --
disregard any framing above that calls it one.

Return 1.0 if the answer to the question is YES and 0.0 if it is NO. Return an
intermediate value ONLY when the text genuinely sits between the YES and NO
conditions as they are written -- do not use one to express uncertainty about a
text that clearly meets one condition or the other."""


def adapt(prompt: str) -> str:
    return prompt + ADAPTER


# --- Mutation fixtures -------------------------------------------------------
# (prompt_id, clear-YES text, clear-NO text, realistic near-miss text).
# The near-miss is the load-bearing one: it is a single-feature step away from
# the pole, which is where a weak rubric passes silently.
MUTATION_CASES = {
    "RTD-8": (
        "Nothing reads the `retry_budget` field. I grepped `src/scheduler.rs` and found "
        "no use of it anywhere, so the field is dead and the struct member can go.",
        "No consumer under `src/scheduler/` reads the `retry_budget` field. A "
        "workspace-wide `grep -r retry_budget` returns only its definition in "
        "`types.rs` and the two tests listed below.",
        # near-miss: the negative IS scoped and the search DOES span the claim,
        # but the flat opening clause reads like an unrestricted one.
        "Nothing in the scheduler reads `retry_budget`. A workspace-wide grep returns "
        "only its definition.",
    ),
    "RTD-9": (
        "The migration path has run exactly once, ever -- there is a single row in "
        "`migrations_applied`.",
        "The migration path has run exactly once within the window "
        "`migrations_applied` retains, which is 90 days.",
        # near-miss: gestures at coverage ("when the table shipped") without
        # stating a span, so it is neither a clean all-time claim nor a windowed one.
        "The migration path has run exactly once -- a single row in "
        "`migrations_applied`, which we have been writing since the table shipped.",
    ),
    "RTD-3": (
        "The scan found zero orphaned rows. This is because the reconciler now runs "
        "before the sweep.",
        "The scan found zero orphaned rows. The reconciler running before the sweep is "
        "one explanation; the scan may also simply not reach rows written in the last "
        "hour, which we have not ruled out.",
        # near-miss in the FALSE-NEGATIVE direction: cites a commit, which reads
        # like evidence, but the prompt says "not speculation" counts toward YES.
        "The scan found zero orphaned rows, because the reconciler now runs before the "
        "sweep. This is not speculation -- the ordering change is commit `4f2a1c`.",
    ),
    "RTD-10": (
        "A prefix collision cannot happen: the two allocators write into separate "
        "namespaces.",
        "A prefix collision cannot happen on the write path: `append_entry` and "
        "`update_entry` are the only two writers, and both allocate through "
        "`next_index`.",
        # near-miss: structural reason PLUS one named site, but the claim stays global.
        "A prefix collision is prevented by the namespace separation. We checked "
        "`append_entry`.",
    ),
    "contradiction": (
        "Every entry in this ledger carries a `Status` field; the template has required "
        "one since the ledger was created.\n\n[...]\n\nA field-presence sweep this week "
        "found that 39 of the 57 entries carry no disposition at all.",
        "Every entry in this ledger carries a `Status` field.\n\n[...]\n\nThat was not "
        "always so: entries written before the 2026-08 pass lacked one, and the pass "
        "backfilled all of them.",
        # near-miss: the two passages are about DIFFERENT subjects (all entries vs
        # new entries), so it is not a contradiction -- but it reads like one.
        "Every entry in this ledger carries a `Status` field.\n\n[...]\n\nThe template "
        "requires a `Status` line on every new entry.",
    ),
}


class TextOnlyAnthropic:
    """AnthropicProvider with `complete_structured` withheld, forcing the text path.

    WHY. `LLMJudge._call` prefers a provider's structured path, and on this corpus
    that path returned schema-valid GARBAGE. Measured 2026-09-23 over 285 rows:
    72% came back with EMPTY `reasoning`, and 52 carried scores no judge emits --
    1e-121, ten rows at 1e-16, one at 1.0018 which is outside the declared 0-1
    range. Every reasoning-present row was sane (80/80); every degenerate score
    sat in a reasoning-empty row. A zero in that cell is a mechanism, not noise.

    `complete_structured` returns (None, 0.0) only on an exception or a type
    mismatch, so `Verdict(score=1e-121, reasoning="")` is WELL-FORMED and sails
    through. skill-eval-playbook L-8 hardened the harness against an UNPARSEABLE
    verdict; this one parses. Schema-validity and meaningfulness came apart, and
    the installed guard only checks the first.

    The text path re-surfaces a genuine failure through `_extract_verdict_json`,
    which RAISES rather than returning 0.0 -- so a bad row errors instead of
    posing as a confident NO.
    """

    def __init__(self, model: str) -> None:
        self.model = model
        self._inner = AnthropicProvider(model)

    def _get_client(self):
        return self._inner._get_client()

    def complete(self, prompt: str) -> tuple[str, float]:
        return self._inner.complete(prompt)

def build_panel(max_spread: float, judges: str) -> PanelJudge:
    """`judges` is an EXPLICIT choice, never a fallback.

    A dead second leg must not silently degrade a cross-family panel into a
    single judge that still reports a `spread` of 0.0 on every row -- that reads
    as perfect agreement and is the "one blind spot counted twice" failure. Two
    Anthropic models would be worse than one, not better: same family, shared
    blind spot, and a spread distribution that looks like corroboration. So the
    only two legal values are the real panel and a named single judge.
    """
    legs = [LLMJudge(CLAUDE_MODEL, provider=TextOnlyAnthropic(CLAUDE_MODEL))]
    if judges == "panel":
        legs.append(LLMJudge(GEMINI_MODEL, provider=GeminiProvider(GEMINI_MODEL)))
    return PanelJudge(legs, max_spread=max_spread)


def banner(judges: str) -> str:
    if judges == "panel":
        return f"cross-family panel: {CLAUDE_MODEL} (authority) + {GEMINI_MODEL}"
    if judges == "jev":
        return (f"TypeSafe Jev ({JEV_MODEL}) `noul` — returns calibrated P(true), no "
                f"reasoning; a fire is P >= 0.5, fixed before the run.")
    return (f"SINGLE JUDGE {CLAUDE_MODEL} — NOT the panel the pre-registration "
            f"names.\n  No divergence signal exists in this run: every `spread` is "
            f"0.0 by construction,\n  and no row can be withheld. Verdicts are "
            f"PROVISIONAL pending a cross-family re-run.")


def score_one(panel, meta: dict, rubric: str, text: str, retries: int = 4) -> dict:
    last = None
    for attempt in range(retries):
        try:
            r = panel.evaluate_rubric(text, rubric, 0.5)
            per = dict((lbl.split("-")[0], sc) for lbl, sc, _ in r.per_judge)
            # A verdict must be IN RANGE and must CARRY REASONING. Both failed
            # silently on the 2026-09-23 structured-path run (72% empty reasoning,
            # 52 degenerate scores incl. 1e-121 and one at 1.0018, outside [0,1]),
            # and neither is recoverable after the fact -- an empty-reasoning 0.0
            # is indistinguishable from a considered NO. So both are refused at
            # the source and surfaced as ERRORED rows rather than as scores.
            body = (r.reasoning.split("): ", 1)[-1].strip()
                    if "spread=" in r.reasoning else r.reasoning.strip())
            if not 0.0 <= r.score <= 1.0:
                raise ValueError(f"score {r.score!r} outside [0,1] -- not a verdict")
            if not body:
                raise ValueError(f"empty judge reasoning on score {r.score!r} -- "
                                 f"degenerate structured output, not a verdict")
            return {**meta, "score": r.score, "claude": per.get("claude"),
                    "gemini": per.get("gemini"), "spread": round(r.spread, 4),
                    "diverged": r.diverged, "verdict": "YES" if r.score >= 0.5 else "NO",
                    "reasoning": body[:600]}
        except Exception as e:  # noqa: BLE001 — surfaced as an errored row, never a score
            last = f"{type(e).__name__}: {e}"
            time.sleep(2 * (2 ** attempt))
    return {**meta, "error": last}


def run(panel, tasks, pool: int) -> list[dict]:
    with ThreadPoolExecutor(max_workers=pool) as ex:
        futs = [ex.submit(score_one, panel, m, r, t) for m, r, t in tasks]
        return [f.result() for f in futs]
NATIVE_YN = re.compile(r"^\W*(YES|NO)\b", re.I)


def native_one(provider, meta: dict, prompt: str, text: str, retries: int = 4) -> dict:
    """Ask the prompt in the form it was WRITTEN for: prompt, then the text, then a
    bare YES/NO. No rubric wrapper, no 0-1 scale, no threshold.

    WHY THIS IS THE DEFAULT. The rubric form embeds the prompt as the `criterion`
    of prompt_tdd's RUBRIC_PROMPT -- a 0-1 "quality" scale with the text placed
    ABOVE a prompt that says "the text below", and YES := score >= 0.5. Measured
    2026-09-23 over 285 tasks: the two forms agreed on 241, and all 44
    disagreements ran ONE way, rubric YES -> native NO (73 YES vs 29). 22 of the
    rubric YESes were intermediate scores crossing the 0.5 line -- including
    CTL10-13, where the judge wrote "pushes toward NO" and then scored 0.7.
    A wrapper that only ever adds YESes is a bias, not noise.
    """
    last = None
    for attempt in range(retries):
        try:
            raw, _ = provider.complete(f"{prompt}\n\n<text>\n{text}\n</text>")
            m = NATIVE_YN.match(raw.strip())
            if not m:
                raise ValueError(f"unparseable native answer: {raw.strip()[:80]!r}")
            v = m.group(1).upper()
            return {**meta, "verdict": v, "score": 1.0 if v == "YES" else 0.0,
                    "spread": 0.0, "diverged": False, "raw": raw.strip()[:200]}
        except Exception as e:  # noqa: BLE001 — surfaced as an errored row, never a verdict
            last = f"{type(e).__name__}: {e}"
            time.sleep(2 * (2 ** attempt))
    return {**meta, "error": last}
REASONED_SUFFIX = """

Before answering, go through each condition in the question one at a time and
say whether the text meets it. Then give your answer on a final line as
ANSWER: YES or ANSWER: NO."""

REASONED_ANSWER = re.compile(r"^\s*ANSWER:\s*(YES|NO)\b", re.I | re.M)


def reasoned_one(provider, meta: dict, prompt: str, text: str, retries: int = 4) -> dict:
    """The prompt as written, then the text, then room to reason before a final
    `ANSWER:` line. The DEFAULT form, and the only one that passed the mutation gate.

    WHY. Measured 2026-09-23, mutation gate at n=10: the BARE native form answered
    NO to its own clear-YES fixture 10/10 on RTD-8 and RTD-3 (3/10 on RTD-9), and on
    RTD-8 gave a wrong reason for it. The same prompts, allowed to walk their
    conditions first, answered YES with sound reasoning. The prompts' own "Answer YES
    or NO, and nothing else" is the right contract for a trained classifier that
    returns a probability, and the wrong one for an LLM judge: it demands the format
    before the reasoning (prompt-hamsa Heuristic 5).

    The LAST `ANSWER:` line is the verdict, so a condition-by-condition walk that
    says "YES, it meets (a)" can never be read as the answer. A missing answer line
    or an empty reasoning body is refused as an errored row.
    """
    last = None
    for attempt in range(retries):
        try:
            raw, _ = provider.complete(f"{prompt}\n\n<text>\n{text}\n</text>{REASONED_SUFFIX}")
            answers = REASONED_ANSWER.findall(raw)
            if not answers:
                raise ValueError(f"no ANSWER: line in reasoned reply: {raw.strip()[-120:]!r}")
            body = REASONED_ANSWER.split(raw)[0].strip()
            if not body:
                raise ValueError("reasoned reply carried an answer and no reasoning")
            v = answers[-1].upper()
            return {**meta, "verdict": v, "score": 1.0 if v == "YES" else 0.0,
                    "spread": 0.0, "diverged": False, "reasoning": body[-600:]}
        except Exception as e:  # noqa: BLE001 — surfaced as an errored row, never a verdict
            last = f"{type(e).__name__}: {e}"
            time.sleep(2 * (2 ** attempt))
    return {**meta, "error": last}
JEV_URL = "https://api.typesafe.ai/v1/systemone"
JEV_MODEL = "jev-latest"
# The one sentence describing an output Jev does not produce. Whitespace-tolerant
# because the contradiction prompt wraps it across a line break.
JEV_STRIP = re.compile(r"\s*Answer\s+YES\s+or\s+NO,\s+and\s+nothing\s+else\.")


def jev_instructions(prompt: str) -> str:
    """The prompt verbatim minus the answer-format sentence -- REFUSING if that
    sentence is not there exactly once, since a strip that silently did nothing is
    indistinguishable from one that worked (the pre-registration's own rule)."""
    stripped, n = JEV_STRIP.subn("", prompt)
    if n != 1:
        raise ValueError(f"expected the answer-format sentence exactly once, found {n}")
    return stripped.strip()


def jev_one(_provider, meta: dict, prompt: str, text: str, retries: int = 4) -> dict:
    """One `noul` question per request: the passage is the state, the prompt is the
    instructions, no criteria. P(true) >= 0.5 is a fire -- fixed in the
    pre-registration, not tuned on the corpus. The key is read from the
    environment and never logged."""
    import urllib.error
    import urllib.request
    key = os.environ.get("JEV_API_KEY")
    if not key:
        return {**meta, "error": "JEV_API_KEY not set"}
    try:
        instructions = jev_instructions(prompt)
    except ValueError as e:
        return {**meta, "error": str(e)}
    body = json.dumps({"model": JEV_MODEL, "state": text,
                       "questions": {"q": {"type": "noul", "instructions": instructions}}}).encode()
    last = None
    for attempt in range(retries):
        try:
            req = urllib.request.Request(JEV_URL, data=body, headers={
                "Content-Type": "application/json", "Authorization": f"Bearer {key}"})
            with urllib.request.urlopen(req, timeout=60) as r:
                data = json.load(r)
            p = float(data["answers"]["q"]["noul"])
            if not 0.0 <= p <= 1.0:
                raise ValueError(f"noul {p!r} outside [0,1]")
            return {**meta, "verdict": "YES" if p >= 0.5 else "NO", "score": p, "p": p,
                    "spread": 0.0, "diverged": False, "jev_model": data.get("model")}
        except urllib.error.HTTPError as e:
            last = f"HTTP {e.code}: {e.read().decode()[:160].replace(key, '<KEY>')}"
        except Exception as e:  # noqa: BLE001 — surfaced as an errored row, never a verdict
            last = f"{type(e).__name__}: {str(e)[:160].replace(key, '<KEY>')}"
        time.sleep(2 * (2 ** attempt))
    return {**meta, "error": last}


def score_tasks(args, specs: list[tuple[dict, str, str]]) -> list[dict]:
    """One dispatcher for both modes, so mutation and score cannot run different forms.

    `specs` carries each prompt RAW; the rubric form adapts it here and only here.
    Every row is stamped with its form, judge and run index so the analyser can
    refuse a conclusion the configuration cannot support.
    """
    expanded = [({**meta, "form": args.form, "judges": args.judges, "run": i}, prompt, text)
                for i in range(args.runs) for meta, prompt, text in specs]
    if args.form in ("native", "reasoned", "jev"):
        provider = None
        if args.form != "jev":
            provider = TextOnlyAnthropic(CLAUDE_MODEL)
            provider._get_client()
        fn = {"native": native_one, "reasoned": reasoned_one, "jev": jev_one}[args.form]
        with ThreadPoolExecutor(max_workers=args.pool) as ex:
            futs = [ex.submit(fn, provider, m, p, t) for m, p, t in expanded]
            return [f.result() for f in futs]
    panel = build_panel(args.max_spread, args.judges)
    panel.judges[0].provider._get_client()
    return run(panel, [(m, adapt(p), t) for m, p, t in expanded], args.pool)


def mutation(args) -> int:
    specs = [({"prompt_id": pid, "kind": kind}, PROMPTS[pid], text)
             for pid, (yes, no, near) in MUTATION_CASES.items()
             for kind, text in (("clear_yes", yes), ("clear_no", no), ("near_miss", near))]
    rows = score_tasks(args, specs)
    errors = [r for r in rows if "error" in r]
    ok = [r for r in rows if "error" not in r]

    print(f"=== MUTATION CHECK — form={args.form} runs={args.runs} — {banner(args.judges)} ===")
    print(f"rows={len(rows)} ok={len(ok)} errors={len(errors)}\n")
    for e in errors:
        print(f"  ERROR {e['prompt_id']}/{e['kind']}: {e['error']}")

    # Mean score per (prompt, kind) over runs: the fire RATE in native form.
    acc: dict[tuple[str, str], list[float]] = collections.defaultdict(list)
    for r in ok:
        acc[(r["prompt_id"], r["kind"])].append(r["score"])
    print(f"{'prompt':<14} {'clear_yes':>10} {'near_miss':>10} {'clear_no':>10}   split?")
    splits = 0
    for pid in MUTATION_CASES:
        cells = {k: acc.get((pid, k)) for k in ("clear_yes", "near_miss", "clear_no")}
        if not all(cells.values()):
            print(f"{pid:<14} INCOMPLETE")
            continue
        y, m, n = (sum(v) / len(v) for v in (cells["clear_yes"], cells["near_miss"], cells["clear_no"]))
        split = y >= 0.7 and n <= 0.3
        splits += split
        print(f"{pid:<14} {y:>10.2f} {m:>10.2f} {n:>10.2f}   {'PASS' if split else 'FAIL'}")

    print(f"\npoles split on {splits}/{len(MUTATION_CASES)} prompts")
    print("near-miss scores are NOT graded here — they are reported so the "
          "near-gate\nregion is visible before the corpus run.")
    if errors:
        print("\n⚠ ERRORS PRESENT — do not read the table above as a clean result.")
        return 2
    return 0 if splits == len(MUTATION_CASES) else 1


def score(args) -> int:
    rows_in = [json.loads(l) for l in open(args.tasks)]
    print(f"=== JUDGE — form={args.form} runs={args.runs} — {banner(args.judges)}")
    specs = [({"task_id": r["task_id"], "prompt_id": r["prompt_id"],
               "passage_id": r["passage_id"], "parts": r.get("parts", 1)},
              r["prompt"], r["text"])
             for r in rows_in]
    t0 = time.monotonic()
    rows = score_tasks(args, specs)
    with open(args.out, "w") as fh:
        for r in rows:
            fh.write(json.dumps(r) + "\n")
    errors = [r for r in rows if "error" in r]
    ok = [r for r in rows if "error" not in r]
    print(f"=== SCORED {len(ok)}/{len(rows)} in {time.monotonic()-t0:.0f}s "
          f"— errors={len(errors)} — rows -> {args.out}")
    if errors:
        print("⚠ ERRORS PRESENT — the rates below are over a PARTIAL batch.")
    return 2 if errors else 0


PROMPTS: dict[str, str] = {}


def load_prompts(path: str) -> None:
    for line in open(path):
        r = json.loads(line)
        PROMPTS.setdefault(r["prompt_id"], r["prompt"])


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--mode", choices=["mutation", "score"], required=True)
    ap.add_argument("--form", choices=["reasoned", "native", "rubric", "jev"], default="reasoned",
                    help="reasoned (default) = the prompt as written, then reason per "
                         "condition, then a final ANSWER line -- the only form that "
                         "passed the mutation gate; native = bare YES/NO, measurably "
                         "biased to NO; rubric = prompt_tdd's 0-1 RUBRIC_PROMPT, "
                         "measurably biased to YES. The last two are kept only to "
                         "reproduce those findings")
    ap.add_argument("--runs", type=int, default=1,
                    help="samples per task; the judge varies between calls, so a "
                         "single sample is a verdict and >=10 is a rate")
    ap.add_argument("--tasks", default="blind-tasks.jsonl")
    ap.add_argument("--out", default="scored.jsonl")
    ap.add_argument("--judges", choices=["panel", "claude", "jev"], required=True,
                    help="panel = cross-family, needs a live GEMINI_API_KEY; "
                         "claude = single judge, provisional and labelled so in "
                         "the output and in every emitted row")
    ap.add_argument("--max-spread", type=float, default=0.25)
    ap.add_argument("--pool", type=int, default=6)
    args = ap.parse_args()
    if (args.form == "jev") != (args.judges == "jev"):
        sys.exit("--form jev and --judges jev go together: the judge IS the form.")
    if args.form != "rubric" and args.judges == "panel":
        sys.exit(f"--form {args.form} is single-judge by construction: PanelJudge scores "
                 "rubrics, not YES/NO answers. Use --judges claude, or --form rubric.")
    if args.runs < 1:
        sys.exit("--runs must be >= 1")
    load_prompts(args.tasks)
    missing = set(MUTATION_CASES) - set(PROMPTS)
    if args.mode == "mutation" and missing:
        sys.exit(f"mutation fixtures name prompts absent from {args.tasks}: {sorted(missing)}")
    return mutation(args) if args.mode == "mutation" else score(args)


if __name__ == "__main__":
    sys.exit(main())
