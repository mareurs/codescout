"""Gated per-defect checker for the review cells (registration: docs/evals/review-model-vs-context-2026-09-24.md).

One question per known defect, judged by Sonnet 5 through `claude -p` on the clean subscription
channel (phase2-score-dp1.py's SubscriptionJudge), majority of 3.

GATE, run first; a failure stops scoring. For each defect: the real Codex review of that
stimulus must be judged YES (it found every defect in the set), and the same review with
that defect's paragraph(s) removed must be judged NO. The removal is the output mutation
that proves the checker can go red.
"""
from __future__ import annotations

import collections
import concurrent.futures as cf
import importlib.util
import json
import os
import pathlib
import re
import sys

REPO = pathlib.Path("/home/marius/work/claude/codescout")
HERE = pathlib.Path(__file__).resolve().parent
RUNS = pathlib.Path(os.environ.get("RMVC_OUT", HERE / "runs"))
spec = importlib.util.spec_from_file_location("sc", REPO / "scripts/phase2-score-dp1.py")
sc = importlib.util.module_from_spec(spec)
spec.loader.exec_module(sc)
JUDGE = sc.SubscriptionJudge("claude-sonnet-5", os.environ["JUDGE_CONFIG_DIR"])
assert not sc.dirty_reasons(JUDGE.config_dir), "judge channel is not clean"

DEFECTS = {
    "s1": {
        "D1": "The local-classifier pre-registration chooses the C1 arm by its any-fire rate on the held-out test set T, although the same document says T is never read during training or model selection.",
        "D2": "The Stage 2 labelling plan treats every other sentence of a paragraph as a negative for every rule, which a correction or a planted violation does not establish; or the plan does not keep an incident's variants together in one fold across train, validation and calibration.",
        "D3": "The phase-1 Score A reporter (report_corpus / --report) accepts a rows file that is empty or is missing whole texts, and still reports success (exit 0 / zero incomplete).",
        "D4": "The phase-2 scorer (scripts/phase2-score-dp1.py) keeps only per-arm totals and does not save each row's votes or verdict, so which rows changed verdict cannot be traced; or it does not refuse a contaminated (non-clean) judge profile the way the phase-1 selector does.",
        "D5": "In the JevK5 zero-shot results, the cannot_happen probability on the violating text exactly ties its value on a clean text (0.600599), so no threshold on that rule can separate them.",
        "O1": "The scoring document overstates the JevK5 ranking result: it says that on four of five violation texts the gold rule both ranks 1st or 2nd AND scores above the clean maximum, but each condition holds on a different four and both hold on only three.",
        "O2": "The scoring document says S0's 7/8 'is not below' arm 0's 9/10 and calls that prediction held, but 7/8 = 0.875 is below 0.9.",
        "O3": "The scoring document treats the family-level recall (5/17) as an 'upper bound', or says spec-tightening's ceiling cannot reach the bar, although that only bounds regrouping the same outputs, not a re-run with changed specs.",
    },
    "s2": {
        "Ma": "In the Stage 2 mined candidates, the context stored with each positive sentence (the `paragraph` field) is taken from the post-correction (new) side of the diff, so it holds the correction rather than the positive sentence: a label leak if used as model input.",
        "Mb": "The miner's count of document pairs sharing an 8-token shingle (20) is an undercount: keeping one owner per shingle misses pairs, and the true count is higher (25).",
        "Mc": "The claim that mined pairs alone cannot bring any rule to the 50-positive bar (or that the route cannot run on mined pairs) is not established: it rests on a 10-row sample and keyword hints, not on labelled counts.",
    },
}

QUESTION = """Below is a code review someone wrote of a repository. Decide whether the review reports one specific defect.

The defect: {defect}

Count it only if the review itself states this defect -- its substance, in any wording. Merely mentioning the same file, number or topic, or praising the same area, does not count. A review that reports the defect as one item among many still counts.

<review>
{review}
</review>

Quote the sentence(s) of the review that report the defect, if any. Then give your answer on a final line as ANSWER: YES or ANSWER: NO."""

# Gate fixtures: the real Codex reviews, and each with the defect's paragraph(s) removed.
REAL = {"s1": REPO / "docs/research/2026-09-24-codex-rule-tell-review.md",
        "s2": REPO / "docs/research/2026-09-24-codex-rule-tell-followup-review.md"}
CUT = {  # substrings; every blank-line block containing one is removed, with its ### heading
    "D1": ["held-out T is also a selection set", "Stage 3 in docs/evals/phase1-local-classifier-preregistration.md prohibits",
           "the T-selection contradiction is directly verified"],
    "D2": ["a correction diff does not supply every negative label", "Stage 2 calls every other sentence a negative"],
    "D3": ["whole missing texts still bypass", "The earlier within-text fix works"],
    "D4": ["saved phase-2 scoring loses row-level", "scripts/phase2-score-dp1.py main prints",
           "The phase-2 scorer also lacks the selector's clean-profile refusal"],
    "D5": ["L0 probabilities, reconstructed"],
    "O1": ["L0 probabilities, reconstructed"],
    "O2": ["The stripped-arm claim also remains overstated"],
    "O3": ["The 5/17 family result is a post-hoc regrouping"],
    "Ma": ["Positive-example context is the post-correction hunk", "`change_blocks` explicitly returns",
           "reconstructing historical before/after context"],
    "Mb": ["Miner overlap census undercounts", "Same 946 rows, same document keys"],
    "Mc": ["The mining feasibility verdict exceeds the sample", "The status note concludes that mined pairs"],
}


def cut(text: str, needles: list[str]) -> str:
    blocks = re.split(r"\n\s*\n", text)
    # A bullet list is one block; drop only the matching bullet lines inside it.
    out = []
    for b in blocks:
        if any(n in b for n in needles):
            lines = [ln for ln in b.splitlines() if not any(n in ln for n in needles)]
            if b.lstrip().startswith("- ") and lines:
                out.append("\n".join(lines))
            continue
        out.append(b)
    # drop a ### heading left with nothing under it
    res = []
    for i, b in enumerate(out):
        if b.startswith("### ") and (i + 1 == len(out) or out[i + 1].startswith("#")):
            continue
        res.append(b)
    final = "\n\n".join(res)
    assert not any(n in final for n in needles), f"cut left a needle: {needles}"
    return final


def ask(defect: str, review: str) -> str:
    for _ in range(3):
        raw, _ = JUDGE.complete(QUESTION.format(defect=defect, review=review))
        a = sc.ANS.findall(raw)
        if a:
            return a[-1].upper()
    raise ValueError("no ANSWER line after 3 tries")


def majority(defect: str, review: str) -> tuple[str, list[str]]:
    votes = [ask(defect, review) for _ in range(3)]
    return ("YES" if votes.count("YES") >= 2 else "NO"), votes


def gate() -> bool:
    jobs = []
    for stim, ds in DEFECTS.items():
        real = REAL[stim].read_text()
        for did, d in ds.items():
            jobs.append((stim, did, "positive", "YES", d, real))
            jobs.append((stim, did, "negative", "NO", d, cut(real, CUT[did])))
    with cf.ThreadPoolExecutor(6) as ex:
        res = list(ex.map(lambda j: (j, majority(j[4], j[5])), jobs))
    ok = True
    rows = []
    for (stim, did, kind, want, _, _), (got, votes) in res:
        passed = got == want
        ok &= passed
        rows.append({"stim": stim, "defect": did, "fixture": kind, "want": want, "votes": votes, "pass": passed})
        print(f"  {stim} {did:3} {kind:8} want {want:3} votes {votes} {'PASS' if passed else 'FAIL'}", flush=True)
    (RUNS.parent / "gate.json").write_text(json.dumps(rows, indent=1))
    print(f"GATE {'PASSED' if ok else 'FAILED'}: {sum(r['pass'] for r in rows)}/{len(rows)}")
    return ok


def score() -> int:
    metas = [json.loads(p.read_text()) for p in sorted(RUNS.glob("*.meta.json"))]
    jobs = []
    for m in metas:
        review = (RUNS / f"{m['id']}.review.md").read_text()
        if not review.strip() or m["exit"] != 0:
            continue
        for did, d in DEFECTS[m["stim"]].items():
            jobs.append((m["id"], did, d, review))
    with cf.ThreadPoolExecutor(6) as ex:
        res = list(ex.map(lambda j: (j[0], j[1], majority(j[2], j[3])), jobs))
    out = [{"id": rid, "defect": did, "found": got == "YES", "votes": votes} for rid, did, (got, votes) in res]
    (RUNS.parent / "scores.json").write_text(json.dumps(out, indent=1))
    by = collections.defaultdict(lambda: [0, 0])
    for r in out:
        s, m, c = r["id"].split("-")[:3]
        by[(s, m, c)][0] += r["found"]
        by[(s, m, c)][1] += 1
    for k in sorted(by):
        print(k, f"{by[k][0]}/{by[k][1]}")
    return 0


if __name__ == "__main__":
    if sys.argv[1:] == ["gate"]:
        sys.exit(0 if gate() else 1)
    if sys.argv[1:] == ["score"]:
        sys.exit(score())
    sys.exit("usage: check_reviews.py gate|score")
