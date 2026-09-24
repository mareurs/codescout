#!/usr/bin/env python3
"""Stage 2 CANDIDATE build, mined half: in-place correction pairs from this repo's history.

NOT a freeze. No model call. Rows are candidates for the audit, never labels:
`rule` is always null and `rule_hint` is a keyword guess for an auditor.

Input: gitlog.patch in this directory, written by
    git log experiments -p -U3 --no-color --no-renames \
        --format='@@@COMMIT %H%x09%ad%x09%s' --date=short -- 'docs/**/*.md' 'docs/*.md' CLAUDE.md

Two candidate kinds:
  rewrite  -- a removed prose sentence and the added sentence most similar to it
              (difflib ratio >= PAIR_MIN) differ, and a correction marker is on the
              added block or the commit subject. positive = removed, twin = added.
  note     -- a sentence survives unchanged and an italic correction note
              (`*Corrected ...*`, `*Retracted ...*`, `*Withdrawn ...*`) is added right
              after it. positive = the kept sentence, twin = null.

Marker list (MARKERS) and why: these are the words this repo's own corrections use to
walk a published claim back -- the corpus cases read "Corrected 2026-09-21", "withdrawn,
and the replacement is narrower", "is NOT established" -- so a marker is evidence that the
edit is a CORRECTION rather than a rewording. "correct" alone is excluded: it matches
"correctly", "is correct", which assert rather than retract.
"""
from __future__ import annotations

import collections
import difflib
import hashlib
import importlib.util
import itertools
import json
import pathlib
import random
import re
import sys

HERE = pathlib.Path(__file__).resolve().parent
ROOT = pathlib.Path("/home/marius/work/claude/codescout")
PAIR_MIN = 0.35
SEED = 20260924

MARKERS = [
    r"\bcorrected\b", r"\bcorrection\b", r"\bwithdrawn\b", r"\bwithdraws?\b",
    r"\bretract(?:ed|ion|s)?\b", r"\bnot established\b", r"\bnarrower\b",
    r"\bwas wrong\b", r"\bsuperseded\b", r"\boverstate[sd]?\b", r"\bfalsified\b",
    r"\bdoes not hold\b", r"\bnot supported\b",
]
MARKER_RE = re.compile("|".join(MARKERS), re.I)
NOTE_RE = re.compile(r"^\*+\s*(Corrected|Retracted|Withdrawn|Correction)\b", re.I)

# Source docs that ARE held-out material or quote it verbatim: this campaign's
# detection/controls corpora, its scoring and pre-registration docs, its data, and the
# Codex review of it. A candidate from these is dropped whatever the shingle filter says.
HELD_OUT_DOC_RE = re.compile(
    r"rule-tell|rule-injection|phase1-local-classifier|docs/evals/data/2026-09-24-rule-tell"
    r"|codex-rule-tell-review")

_spec = importlib.util.spec_from_file_location("sel", ROOT / "scripts/phase1-span-selector.py")
sel = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(sel)
p1 = sel._p1

# Keyword hints per rule. Order is priority for the single `rule_hint`. A HINT ONLY.
HINTS: list[tuple[str, str]] = [
    ("cannot_happen", r"\bcannot (?:happen|occur)\b|\bimpossible\b|\bno site\b|\bwill never\b"),
    ("contradiction", r"\bcontradict|\bcannot both\b"),
    ("count_unit", r"\b\d+\s+of\s+\d+\b|\bn\s*=\s*\d+|\b\d+\s+(?:instances|bugs|entries|rows|files|sessions|members|tests|findings|artifacts|trackers)\b"),
    ("scope_instant", r"\bas of\b|\bno other\b|\bnone remain|\bwindow\b|\bat \d{1,2}:\d{2}"),
    ("run_tool", r"\bbyte-identical\b|\bidentical output\b|\bexit(?:s)? \d\b|\breturns?\b"),
    ("member_vs_population", r"\bfull suite\b|\ball tests pass|\bsuite is green\b|\bgreen suite\b"),
    ("monotone_absence", r"\bzero\b|\bno errors?\b|\bempty\b|\bsilen(?:t|ce)\b|\bnothing (?:fired|found|reads)"),
    ("question_asked", r"\banswers?\b|\bproves?\b|\bproof\b|\bestablish(?:es|ed)?\b"),
    ("open_artifact", r"\bleftover\b|\bas I recall\b|\bper the\b|\bthe plan says\b"),
    ("act_on_artifact", r"\bearlier (?:listing|reading|observation)\b|\bcached\b|\bstale\b"),
    ("selector_narrow", r"\bfilter\b|\bgrep\b|\bquery\b|\bsample\b"),
    ("closed_population", r"\b(?:all|every|none|no)\b"),
    ("lines_read", r"\bgrep hit\b|\bsearch match\b"),
    ("d_semicolon", r"&&"),
    ("d_sessionid", r"\bsession name\b|codescout-\d\d"),
    ("d_visibility", r"\bpermission\b|\bapprov"),
    ("d_adjacency", r"\bnearby\b|\badjacen"),
]
HINT_RES = [(r, re.compile(p, re.I)) for r, p in HINTS]
assert {r for r, _ in HINTS} <= set(sel.RULES), "hint names a rule not on the menu"

SENT_SPLIT = re.compile(r"(?<=[.!?])\s+(?=[*_`\"(A-Z0-9])")
TOK = re.compile(r"\w+")


def norm(s: str) -> str:
    return re.sub(r"\s+", " ", s).strip()


def window(text: str, sentence: str, width: int = 1500) -> str:
    """`width` characters of `text` CENTRED on `sentence`. A prefix cut from the start of the
    hunk left the positive outside its own context in 147 of 940 rows, every one of them
    at the cap. A sentence not found in `text` (should not happen) falls back to the prefix."""
    i = text.find(sentence)
    if i < 0 or len(text) <= width:
        return text[:width]
    start = max(0, min(i - (width - len(sentence)) // 2, len(text) - width))
    return text[start:start + width]

def prose_line(ln: str) -> bool:
    s = ln.strip()
    return bool(s) and not s.startswith(("|", "```", "#", "<!--", "---")) \
        and not re.match(r"^[\w-]+:\s", s)


def sentences(block: list[str]) -> list[str]:
    text = norm(" ".join(l.strip() for l in block if prose_line(l)))
    out = []
    for s in SENT_SPLIT.split(text):
        s = re.sub(r"^[-*]\s+(?=\S)", "", s).strip()   # a list bullet, not an emphasis
        if len(s) >= 40:
            out.append(s)
    return out


def shingles(text: str, n: int = 8) -> set[tuple[str, ...]]:
    t = [w.lower() for w in TOK.findall(text)]
    return {tuple(t[i:i + n]) for i in range(len(t) - n + 1)}


def hint(pos: str, twin: str | None) -> tuple[str | None, str | None, list[str]]:
    """Keywords from what the correction REMOVED first; the whole positive as fallback."""
    removed = pos
    if twin:
        tw = set(w.lower() for w in TOK.findall(twin))
        removed = " ".join(w for w in TOK.findall(pos) if w.lower() not in tw)
        removed += " " + " ".join(re.findall(r"&&|\d+\s+of\s+\d+", pos))
    for src in (removed, pos):
        hits = [(r, m.group(0)) for r, rx in HINT_RES if (m := rx.search(src))]
        if hits:
            return hits[0][0], hits[0][1], [r for r, _ in hits]
    return None, None, []


# --- parse the log --------------------------------------------------------------------
def commits(path: pathlib.Path):
    cur = None
    for ln in path.read_text(errors="replace").splitlines():
        if ln.startswith("@@@COMMIT "):
            if cur:
                yield cur
            sha, date, subj = (ln[10:].split("\t") + ["", ""])[:3]
            cur = {"sha": sha, "date": date, "subject": subj, "files": []}
        elif cur is None:
            continue
        elif ln.startswith("diff --git "):
            cur["files"].append({"path": ln.split(" b/", 1)[-1], "hunks": []})
        elif ln.startswith("@@") and cur["files"]:
            cur["files"][-1]["hunks"].append([])
        elif cur["files"] and cur["files"][-1]["hunks"] and ln[:1] in " +-" \
                and not ln.startswith(("+++", "---")):
            cur["files"][-1]["hunks"][-1].append(ln)
    if cur:
        yield cur


def change_blocks(hunk: list[str]):
    """(removed lines, added lines, old-side paragraph, new-side paragraph) per change.

    BOTH sides, deliberately: a positive is a REMOVED sentence, so its context is the OLD
    side. Carrying only the new side put the correction beside the sentence it corrects --
    the positive sat inside its own `paragraph` in 30 of 946 rows and the twin in 605
    (Codex follow-up review, 2026-09-24), so a model given that context could read the
    answer off it."""
    old_side = [l[1:] for l in hunk if l[:1] in " -"]
    new_side = [l[1:] for l in hunk if l[:1] in " +"]
    i = 0
    while i < len(hunk):
        if hunk[i][:1] in "+-":
            rem, add = [], []
            while i < len(hunk) and hunk[i][:1] in "+-":
                (rem if hunk[i][0] == "-" else add).append(hunk[i][1:])
                i += 1
            yield rem, add, old_side, new_side
        else:
            i += 1


def mine(log: pathlib.Path):
    all_commits = list(commits(log))[::-1]            # oldest first
    first_seen: dict[str, str] = {}                   # sentence hash -> first commit adding it
    rows = []
    for c in all_commits:
        subj_marker = MARKER_RE.search(c["subject"])
        for f in c["files"]:
            for hunk in f["hunks"]:
                for rem, add, old_side, new_side in change_blocks(hunk):
                    R, A = sentences(rem), sentences(add)
                    for s in A:
                        first_seen.setdefault(hashlib.sha1(s.encode()).hexdigest(), c["sha"])
                    if not R and not A:
                        continue
                    add_text = " ".join(add)
                    add_marker = MARKER_RE.search(add_text)
                    rem_marker = MARKER_RE.search(" ".join(rem))
                    before_full = norm(" ".join(l for l in old_side if prose_line(l)))
                    after_full = norm(" ".join(l for l in new_side if prose_line(l)))
                    Aset = set(A)
                    # rewrite pairs
                    for r in R:
                        if r in Aset:
                            continue
                        best = max(A, key=lambda a: difflib.SequenceMatcher(None, r, a).ratio(),
                                   default=None)
                        if best is None or best in set(R):
                            continue
                        ratio = difflib.SequenceMatcher(None, r, best).ratio()
                        if ratio < PAIR_MIN:
                            continue
                        m = MARKER_RE.search(best) or (add_marker if not rem_marker else None) \
                            or subj_marker
                        if not m:
                            continue
                        src = "twin" if MARKER_RE.search(best) else \
                              "added-block" if (add_marker and not rem_marker) else "subject"
                        rows.append(dict(kind="rewrite", sha=c["sha"], date=c["date"],
                                         subject=c["subject"], path=f["path"], positive=r,
                                         twin=best, ratio=round(ratio, 2),
                                         context_before=window(before_full, r),
                                         context_after=window(after_full, best),
                                         marker=m.group(0), marker_source=src))
                    # appended correction notes after a kept sentence
                    for j, a in enumerate(A):
                        if NOTE_RE.match(a) and j > 0 and A[j - 1] in set(R):
                            rows.append(dict(kind="note", sha=c["sha"], date=c["date"],
                                             subject=c["subject"], path=f["path"],
                                             positive=A[j - 1], twin=None, ratio=None,
                                             context_before=window(before_full, A[j - 1]),
                                             context_after=window(after_full, a),
                                             marker=NOTE_RE.match(a).group(1),
                                             marker_source="note", note=a[:300]))
    return rows, first_seen, len(all_commits)


# --- held-out set ----------------------------------------------------------------------
def held_out() -> dict[str, set]:
    src: dict[str, set] = collections.defaultdict(set)
    for c in p1.load_cases(p1.EVAL_SET):
        for side in ("positive", "negative"):
            for t in c[side]:
                src["phase1A-pairs"] |= shingles(t)
    for _, t, _ in sel.GATE:
        src["gate"] |= shingles(t)
    for _, _, t, _ in sel.SPAN_GATE:
        src["span-gate"] |= shingles(t)
    src["controls"] |= shingles((ROOT / "docs/evals/rule-tell-controls.md").read_text())
    for fp in sorted((ROOT / "docs/evals/data/2026-09-24-rule-tell").glob("fork-*.jsonl")):
        for ln in fp.read_text().splitlines():
            if ln.strip():
                t = json.loads(ln).get("text") or ""
                src["fork-drafts"] |= shingles(t)
    return src


def main() -> int:
    rows, first_seen, n_commits = mine(HERE / "gitlog.patch")
    found = len(rows)
    ho = held_out()
    dropped = collections.Counter()
    kept = []
    seen_pairs = set()
    for r in rows:
        key = (norm(r["positive"]), norm(r["twin"] or ""))
        if key in seen_pairs:                           # same pair re-committed (rebase copy)
            dropped["duplicate pair"] += 1
            continue
        seen_pairs.add(key)
        if HELD_OUT_DOC_RE.search(r["path"]):
            dropped["held-out source doc"] += 1
            continue
        sh = (shingles(r["positive"]) | shingles(r["twin"] or "")
              | shingles(r["context_before"]) | shingles(r["context_after"]))
        hit = [name for name, s in ho.items() if sh & s]
        if hit:
            for name in hit:
                dropped[f"shingle: {name}"] += 1
            dropped["shingle (any)"] += 1
            continue
        kept.append(r)

    # incidents: origin commit of the positive sentence, per path; a candidate whose
    # positive is an earlier candidate's twin joins that incident (a correction chain).
    parent: dict[str, str] = {}

    def find(x):
        while parent.setdefault(x, x) != x:
            parent[x] = parent[parent[x]]
            x = parent[x]
        return x

    twin_owner: dict[str, str] = {}
    for r in kept:
        h = hashlib.sha1(r["positive"].encode()).hexdigest()
        origin = first_seen.get(h, r["sha"])
        r["incident"] = f"{r['path']}@{origin[:10]}"
        find(r["incident"])
        if norm(r["positive"]) in twin_owner:
            parent[find(r["incident"])] = find(twin_owner[norm(r["positive"])])
        if r["twin"]:
            twin_owner[norm(r["twin"])] = r["incident"]
    for r in kept:
        r["incident"] = find(r["incident"])
        # fold unit per the amendment: incident AND its source document. Archive moves
        # change the path, so the document key is the basename, not the path.
        r["doc_group"] = pathlib.Path(r["path"]).name
        rh, kw, allh = hint(r["positive"], r["twin"])
        r.update(rule=None, rule_hint=rh, rule_hint_keyword=kw, rule_hint_all=allh)

    # cross-group shingle collisions: a preview of the across-folds filter
    by_group = collections.defaultdict(set)
    for r in kept:
        by_group[r["doc_group"]] |= shingles(r["positive"]) | shingles(r["twin"] or "")
    # EVERY pair of groups sharing a shingle. Keeping one owner per shingle recorded A-B and
    # A-C for a shingle held by A, B and C and never B-C: 20 star edges published as 25
    # pairs (docs/issues/2026-09-24-codex-miner-shingle-pair-undercount.md). The star edges
    # do preserve connected components, so the fold build can use either; this counts pairs.
    owners = collections.defaultdict(set)
    for g, ss in by_group.items():
        for s in ss:
            owners[s].add(g)
    collide = {p for gs in owners.values() if len(gs) > 1
               for p in itertools.combinations(sorted(gs), 2)}

    out = HERE / "mined-candidates.jsonl"
    with out.open("w") as fh:
        for r in kept:
            fh.write(json.dumps(r, ensure_ascii=False) + "\n")

    print(f"commits scanned: {n_commits}")
    print(f"candidates found: {found}   kept: {len(kept)}")
    print("dropped:", dict(dropped))
    print(f"kinds: {dict(collections.Counter(r['kind'] for r in kept))}")
    print(f"marker source: {dict(collections.Counter(r['marker_source'] for r in kept))}")
    print(f"distinct incidents: {len({r['incident'] for r in kept})}   "
          f"distinct doc groups: {len({r['doc_group'] for r in kept})}")
    print(f"doc-group pairs sharing an 8-token shingle: {len(collide)}")
    print("rule_hint distribution (single hint):")
    for k, n in collections.Counter(r["rule_hint"] for r in kept).most_common():
        print(f"  {str(k):22} {n}")
    print("rule_hint distribution (any hint):")
    anyc = collections.Counter(h for r in kept for h in r["rule_hint_all"])
    for k, n in anyc.most_common():
        print(f"  {k:22} {n}")
    print(f"\nrandom sample of 10, random.Random({SEED}):")
    for r in random.Random(SEED).sample(kept, min(10, len(kept))):
        print(f"--- {r['kind']} {r['sha'][:8]} {r['path']}  marker={r['marker']!r} "
              f"({r['marker_source']})  hint={r['rule_hint']}:{r['rule_hint_keyword']!r}")
        print(f"  POS : {r['positive'][:200]}")
        print(f"  TWIN: {(r['twin'] or r.get('note') or '')[:200]}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
