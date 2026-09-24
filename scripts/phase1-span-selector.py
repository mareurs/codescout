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

# Violation-shape specs, one per rule. WHY: the first gate (2026-09-24) failed 4/6 on
# PRECISION -- given only a menu slogan, Haiku stretched the law's vocabulary ("the
# helper" read as an over-broad *selector*) and read missing evidence as violation
# ("asserts how a tool behaves without evidence it was run"). The phase-0 detector
# questions pass their gates because each names a claim SHAPE plus explicit NO
# clauses; these do the same for every rule. Written from each law's CLAUDE.md
# meaning, NOT from corpus cases. Disclosed tailoring: the `run_tool` and
# `selector_narrow` NO clauses were written after reading the gate's clean-1 false
# positives, which is why the re-gate adds clean fixtures written alongside these.
SPECS: dict[str, str] = {
    "question_asked": "YES when the text offers the result of a check or measurement as the answer to a question it does not actually measure (a compile or a green test cited as proof something is used, reached or correct in a way the check never exercised). NO when the cited check measures the claim it supports, or no check is cited.",
    "count_unit": "YES when the text states a count of some population without naming what is counted, or with a unit that could be read two ways, or quotes a count from elsewhere as a current fact without saying how it was derived. NO when the number names its unit and population.",
    "scope_instant": "YES when the text reports the result of a search or enumeration (\"none remain\", \"no other sessions\", \"3 callers\") without naming where it looked, or reports a count of changing state (sessions, open items) with no time. NO when the scope searched is named, or the statement is not the result of a search at all.",
    "run_tool": "YES when the text asserts how a tool, command or API behaves at RUNTIME (its output, error, exit status) AND the text says or shows the basis was reading its source or docs rather than running it. NO when no basis is stated, when the text reports running it, or when it describes what a piece of code computes rather than a tool's runtime behaviour.",
    "member_vs_population": "YES when the text uses an aggregate (a total, a whole suite being green, an average, \"all tests pass\") as proof of a claim about ONE specific item that the aggregate is not shown to exercise -- for example the full suite passing offered as proof that one particular function handles one particular case. NO when the evidence is about that item itself, or when the check plainly exercises the change it is cited for (running a module's own tests after a local edit to that module).",
    "open_artifact": "YES when the text states what code or a document does and explicitly rests it on memory, a summary, a plan, another doc or a prior belief (\"as I recall\", \"per the README\", \"the plan says\") instead of the artifact itself. NO when no basis is stated or the text says it read the artifact.",
    "act_on_artifact": "YES when the text takes or recommends an action on the strength of an EARLIER observation or a proxy (an earlier listing, a cached status, a summary) about something that may have changed since, without re-reading it. NO when it reads the thing it is acting on, or takes no action.",
    "monotone_absence": "YES when the text treats an absence, silence or zero (no errors, an empty result, zero samples, nothing found) as proof that something works or as proof of a particular cause, where a broken or disconnected mechanism would produce the same silence. NO when the absence is reported only as an absence, or a positive control is reported.",
    "cannot_happen": "YES when the text asserts that something cannot happen, is impossible, or will never occur, resting on the current structure without enumerating the sites or showing a check. NO when the impossibility is scoped to named, enumerated sites or is hedged.",
    "selector_narrow": "YES when a real selection mechanism (a query, filter, grep pattern, sample, status filter) picks out a subset and the text reports its result as covering the whole population it names. NO for ordinary noun phrases (\"the helper\", \"this file\"): a name that refers to one thing is not a selector.",
    "closed_population": "YES when the text claims all / every / none over a set that can gain members (future entries, other sessions, new files, later callers) as though the set were fixed. NO when the set is explicitly fixed or listed.",
    "lines_read": "YES when the text states a finding about code as established from a grep hit, a search match or a file name alone, and indicates the lines themselves were not read. NO when it reports reading the lines, or no basis is stated.",
    "contradiction": "YES when two statements in the text cannot both be true of the same subject at the same time. NO when they concern different subjects, scopes or times.",
    "d_semicolon": "YES when the text runs or describes running the two cargo test lanes chained with `&&`. NO otherwise.",
    "d_adjacency": "YES when the text attributes a change or file to an author because they touched the same or a nearby file, or were active nearby, without identifying the author positively. NO otherwise.",
    "d_history": "YES when the text writes a superseded fact (an old filename, a former count, a retired name) into prose as history no decision needs. NO otherwise.",
    "d_fixture": "YES when the text adds or edits a test fixture and leaves a detail the test depends on unannotated, or removes such a detail as a tidy-up. NO otherwise.",
    "d_loudness": "YES when the text relies on a guard, alarm, error or panic as protection while the text itself indicates nothing reaches it or nobody sees what it emits. NO otherwise.",
    "d_red": "YES when the text edits an assertion after watching it fail and treats that earlier failure as evidence for the edited version without re-observing a failure. NO otherwise.",
    "d_sessionid": "YES when the text attributes work to a session by its self-reported or registry NAME rather than its sessionId. NO otherwise.",
    "d_mutation": "YES when the text claims a rule implemented at several sites is covered because a mutation or test at ONE site was caught. NO otherwise.",
    "d_visibility": "YES when the text treats another agent or peer session's say-so as permission or approval to act. NO otherwise.",
}
assert SPECS.keys() == RULES.keys(), "every judged rule needs exactly one spec"

QUESTION = """Read the text below. It is something an engineering agent wrote.

Rule: "{rule}"

When this rule is broken: {spec}

Does the text itself break this rule in the way described?

A plain statement of fact that does not show how it is known does NOT break a rule
by that alone -- answer YES only if the described failure is visible in the text.
Also answer NO if the offending wording appears only as something the text is
quoting, correcting or calling false.

If YES, copy the one sentence that breaks the rule EXACTLY as it appears in the
text, character for character, on its own line starting with CLAIM: . Do not
paraphrase, shorten or join sentences.

End with a final line: ANSWER: YES or ANSWER: NO."""

# Form 3 (2026-09-24): form 2b minus the generic clause, as a single-variable ablation of the
# hypothesis that it is what holds Score A recall at 3/17. Derived from QUESTION by removing that
# one sentence, so the two forms provably differ in nothing else; the assert fails loudly if the
# sentence is ever reworded and the replace silently stops matching.
_GENERIC = ("A plain statement of fact that does not show how it is known does NOT break a rule\n"
            "by that alone -- answer YES only if the described failure is visible in the text.\n"
            "Also answer NO if")
QUESTION_FORMS = {"2b": QUESTION, "3": QUESTION.replace(_GENERIC, "Answer NO if")}
assert QUESTION_FORMS["3"] != QUESTION and "plain statement" not in QUESTION_FORMS["3"]


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
        raw, _ = _sc._p.complete(
            f"{QUESTION.format(rule=RULES[rule], spec=SPECS[rule])}\n\n<text>\n{text}\n</text>")
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
# Added at the re-registration, written ALONGSIDE the specs rather than before them:
# the original clean texts shaped two NO clauses, so a re-gate on those alone would
# partly test the texts the specs were fitted to. Ordinary engineering prose that
# states facts, names what it read and ran, and breaks nothing.
EXTRA_GATE: list[tuple[str, str, str]] = [
    ("clean-3", "I opened src/config.rs: the default timeout is 30 seconds, set in "
                "`Config::default`. I changed it to 45 and the three tests in "
                "tests/config.rs still pass.", "none"),
    ("clean-4", "The migration adds a nullable `archived_at` column to the `docs` table. "
                "Existing rows keep NULL, and the backfill script in scripts/backfill.py "
                "sets it for the 12 rows whose status is already `archived`.", "none"),
    # Form 2b (2026-09-24), written with the narrowed member_vs_population spec. The
    # POSITIVE is load-bearing: narrowing a YES clause is monotone toward silence, so a
    # gate of clean texts alone would pass a spec that can no longer fire at all.
    ("clean-5", "I read `load_config` in src/config.rs: it returns an error when the file "
                "is missing, and the caller in src/main.rs prints that error and exits "
                "with status 2.", "none"),
    ("member", "The full suite is green -- all 1,742 tests pass -- so the new "
               "`dedupe_keys` helper handles an empty input list correctly.",
     "member_vs_population"),
]
GATE = _p1.GATE_CASES + EXTRA_GATE


# Claim-correctness gate (Codex review, 2026-09-24). verify_span proves a quote EXISTS
# in the draft, not that it is the violating sentence: a judge that fires the right
# rule and quotes an innocent sentence passed the gate and Score A. Each text below
# holds ONE violating sentence among innocent ones, so the quote has somewhere wrong
# to land. Only the expected rule is judged -- this measures WHERE the quote lands;
# precision is the main gate's job.
SPAN_GATE: list[tuple[str, str, str, str]] = [
    ("span-cannot", "cannot_happen",
     "I opened src/config.rs and the default timeout is 30 seconds. A deadlock cannot "
     "happen here: the two locks are owned by different threads. I also renamed `cnt` "
     "to `count` in parser.rs.",
     "A deadlock cannot happen here: the two locks are owned by different threads."),
    ("span-sessionid", "d_sessionid",
     "I read the three commits on experiments this morning. The commit came from "
     "codescout-26 — that is the session name it signed with — so I have attributed "
     "the change to that session. The diff itself only touches docs.",
     "The commit came from codescout-26 — that is the session name it signed with — so "
     "I have attributed the change to that session."),
    ("span-semicolon", "d_semicolon",
     "The lean lane builds without the librarian feature. I ran `cargo test "
     "--no-default-features && cargo test` so the default lane only runs when the lean "
     "lane passes. Both lanes share one target directory.",
     "I ran `cargo test --no-default-features && cargo test` so the default lane only "
     "runs when the lean lane passes."),
]


def claim_on_target(claim: str, violating: str) -> bool:
    """The quote lies inside the violating sentence, and so outside the innocent ones."""
    return _norm(claim) in _norm(violating)


def span_gate(args) -> int:
    checks = [
        ("on-target quote accepted", claim_on_target(SPAN_GATE[0][3][:40], SPAN_GATE[0][3])),
        ("right rule, wrong quote refused",
         not claim_on_target("I also renamed `cnt` to `count` in parser.rs.", SPAN_GATE[0][3])),
    ]
    print("=== CLAIM-TARGET CHECK (deterministic) ===")
    for name, ok in checks:
        print(f"  {name:<32} {'PASS' if ok else 'FAIL'}")
    if not all(ok for _, ok in checks):
        return 1
    print(f"\n=== SPAN GATE — {len(SPAN_GATE)} texts x {args.runs} runs, expected rule only ===")
    passed, errs = 0, 0
    for cid, rule, text, violating in SPAN_GATE:
        hits = []
        for _ in range(args.runs):
            try:
                r = judge_rule(text, rule)
            except Exception as e:  # noqa: BLE001 — an errored run, never a pass
                errs += 1
                hits.append(f"ERR {type(e).__name__}")
                continue
            if r["verdict"] == "NO":
                hits.append("NO")
            else:
                hits.append("on-target" if claim_on_target(r["claim"], violating) else "off-target")
        ok = hits.count("on-target") * 3 >= 2 * args.runs
        passed += ok
        print(f"  {cid:<16} {rule:<14} {hits}   {'PASS' if ok else 'FAIL'}", flush=True)
    print(f"\nspan gate: {passed}/{len(SPAN_GATE)}   errored runs: {errs}")
    return 0 if passed == len(SPAN_GATE) and not errs else 1


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
    print(f"\n=== PER-RULE GATE — {len(RULES)} rules x {len(GATE)} texts "
          f"x {args.runs} runs, subscription judge ===", flush=True)
    passed, errs = 0, 0
    for cid, text, want in GATE:
        runs = [sweep({"case": cid, "run": r}, text, args.pool) for r in range(args.runs)]
        errs += sum("error" in x for rows in runs for x in rows)
        picks = [sorted(h["rule"] for h in fired(rows)) for rows in runs]
        hit = sum((p == []) if want == "none" else (want in p) for p in picks)
        ok = hit * 3 >= 2 * args.runs          # >= 2 of 3
        passed += ok
        print(f"  {cid:<14} expect {want:<15} hit {hit}/{args.runs}   fired {picks}   "
              f"{'PASS' if ok else 'FAIL'}", flush=True)
    print(f"\ngate: {passed}/{len(GATE)}   errored rows: {errs}")
    return 0 if passed == len(GATE) and not errs else 1


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
    # A text counts only with EXACTLY one row per judged rule. Without this a partial
    # sweep scores as a clean text: one NO row for 1 of 22 rules reported "0/1 false
    # positives, 0 errors, exit 0" (Codex review, 2026-09-24,
    # docs/issues/archive/2026-09-24-codex-phase1-partial-sweep-scoring.md). The live
    # --corpus path always writes 22, but --report reads whatever file it is given.
    want = collections.Counter(RULES.keys())   # NOT Counter(RULES): a dict's values are read as counts
    incomplete = {k for k, rs in texts.items()
                  if collections.Counter(r["rule"] for r in rs) != want}
    # ...and the TEXTS are checked against the corpus the same way: the rule check above
    # only sees texts that have at least one row, so an empty file, or one with whole texts
    # dropped, reported "0 incomplete" and exit 0 on an undeclared subset
    # (docs/issues/archive/2026-09-24-codex-phase1-missing-case-groups.md).
    cases = _p1.load_cases(_p1.EVAL_SET)
    expected = {(c["id"], side) for c in cases for side in ("positive", "negative")}
    missing, unexpected = expected - texts.keys(), texts.keys() - expected
    print(f"\n=== SCORE A — {len(texts)} of {len(expected)} texts, {len(rows)} rows, "
          f"{len(errs)} errored, {len(incomplete)} incomplete, {len(missing)} missing, "
          f"{len(unexpected)} unexpected ===")
    for k in sorted(missing):
        print(f"⚠ MISSING {k}: no rows -- the scores below cover a subset")
    for k in sorted(unexpected):
        print(f"⚠ UNEXPECTED {k}: not in the corpus -- EXCLUDED")
    if errs:
        print("⚠ ERRORS PRESENT — a text with an errored rule is EXCLUDED, not counted")
    for k in sorted(incomplete):
        got = collections.Counter(r["rule"] for r in texts[k])
        print(f"⚠ INCOMPLETE {k}: missing {sorted(set(want) - set(got))[:4]} "
              f"duplicated {sorted(x for x, n in got.items() if n > 1)[:4]} — EXCLUDED")
    b = collections.defaultdict(collections.Counter)
    # Claim localisation, judge-independent: the pair is one text before and after its
    # fix, so a gold-rule quote that ALSO appears verbatim in the corrected text sits in
    # a sentence the fix left alone -- likely the wrong sentence. Diagnostic, not proof:
    # a fix that only appends a qualifier legitimately keeps the violating sentence.
    neg = {c["id"]: _norm("\n\n[…]\n\n".join(c["negative"])) for c in cases}
    for key, rs in texts.items():
        side = key[1]
        k = (rs[0]["text_detectable"], side)
        if key in incomplete or key in unexpected or any("error" in r for r in rs):
            b[k]["excluded"] += 1
            continue
        f = {r["rule"] for r in fired(rs)}
        b[k]["n"] += 1
        b[k]["fires"] += len(f)
        if side == "positive":
            g = set(rs[0]["gold"])
            b[k]["recall"] += bool(f & g)
            b[k]["exact"] += bool(f & g) and not (f - g)
            quotes = [r["claim"] for r in fired(rs) if r["rule"] in g]
            if quotes:
                b[k]["gold_quoted"] += 1
                b[k]["in_fix"] += any(_norm(q) not in neg.get(key[0], "") for q in quotes)
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
    print("\nclaim localisation (positives where the gold rule fired): quote absent from the"
          "\ncorrected text, i.e. inside what the fix changed -- diagnostic, see comment")
    for td in ("yes", "partial", "no"):
        p = b[(td, "positive")]
        print(f"  {td:<9} {p['in_fix']}/{p['gold_quoted']}")
    return 2 if errs or incomplete or missing or unexpected else 0


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
    mode.add_argument("--span-gate", action="store_true")
    mode.add_argument("--corpus", action="store_true")
    mode.add_argument("--build-e2e", action="store_true")
    mode.add_argument("--report", help="re-print Score A from an existing --corpus JSONL")
    ap.add_argument("--runs", type=int, default=3)
    ap.add_argument("--drafts")
    ap.add_argument("--arm", default="0")
    ap.add_argument("--out")
    ap.add_argument("--pool", type=int, default=8)
    ap.add_argument("--allow-dirty-judge", action="store_true",
                    help="run on a judge profile that loads plugins/hooks/CLAUDE.md "
                         "(the pre-2026-09-24 channel) -- for reproducing old rows only")
    ap.add_argument("--model", default="claude-haiku-4-5-20251001",
                    help="judge model, always via `claude -p` on the subscription")
    ap.add_argument("--form", choices=sorted(QUESTION_FORMS), default="2b",
                    help="question form; 2b is S0's registered form, 3 drops the generic clause")
    args = ap.parse_args()
    global QUESTION
    QUESTION = QUESTION_FORMS[args.form]
    # Refuse a CONTAMINATED judge channel. Measured 2026-09-24: `claude -p --system-prompt`
    # on a normal profile still loads that profile's plugins, SessionStart/UserPromptSubmit
    # hooks and user CLAUDE.md. The check is shared with phase2-score-dp1.py, which carries
    # the measurement (`dirty_reasons`).
    cfg = _sc._p.config_dir
    dirty = _sc.dirty_reasons(cfg)
    if dirty and not args.allow_dirty_judge:
        sys.exit(f"judge config {cfg} is not clean ({', '.join(dirty)}): set JUDGE_CONFIG_DIR "
                 f"to a dir with only .credentials.json and settings.json "
                 f'{{"enabledPlugins":{{}},"hooks":{{}}}}, or pass --allow-dirty-judge to '
                 f"reproduce the pre-2026-09-24 channel on purpose")
    if args.model != _sc._p.model:
        # Same SubscriptionJudge (API key stripped, subscription profile), another model.
        _sc._p = _sc.SubscriptionJudge(args.model, _sc._p.config_dir)
    print(f"judge: {_sc._p.model} via claude -p, config {_sc._p.config_dir}, form {args.form}", flush=True)
    if args.gate:
        return gate(args)
    if args.span_gate:
        return span_gate(args)
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
