"""Cue counterexamples for phase-1b Stage 2's arm NC: candidate units, before labelling.

    ~/work/claude/jevk5/.venv/bin/python mine_counterexamples.py --count-only
    ~/work/claude/jevk5/.venv/bin/python mine_counterexamples.py

Implements docs/evals/phase1b-local-classifier-preregistration.md § Stage 2, open decision 1 (a).

Mined cues: the cue-list.json candidates where the head's train positives carrying the cue
outnumber its train negatives carrying it plus the cross-pool units carrying it (mined_cues).

Population: every unit, cut by the training segmenter, of every training-side paragraph of the
registered seed manifest (cue_list.training_side_paragraphs: re-read from git at the manifest's
commit, sha1-checked). A unit is a candidate for a head when its feature set, surface_probe's
featurisation, contains any of that head's mined cues. The row a candidate becomes carries the
whole paragraph as its text, so every filter below reads the whole paragraph.

Filters against existing data (filter_against_existing), in order; a dropped candidate is counted
under its reason and never moved:
  1. held-out: an 8-token shingle shared with freeze_stage2.held_out_shingles() (phase 1's
     held-out texts and gate, the mined T rows, the T-syn pairs and their S seeds) or with any of
     phase 1b's new clean texts (clean_texts: clean-6 to clean-14 and the Codex texts);
  2. cross-fold: a shingle shared with a frozen train/val/cal row's text in another fold;
  3. own rule: a shingle shared with a frozen row's text of the head's own rule, in any fold.
     Those rows keep their non-target own-rule units masked, and a counterexample from the same
     paragraph would unmask one.
Then among the candidates themselves (resolve_new_vs_new):
  4. new versus new: two candidates in different folds can still share a shingle, when one passage
     was copied into two documents. The candidate in the earlier fold of FOLD_PRIORITY (val, cal,
     train) is kept and the other dropped, so text that reaches train is in neither val nor cal,
     and text in cal is not in val. Folds come from the manifest's source groups, which keep a
     document in one fold but cannot see content repeated across documents.

Draw: for the head at index i of the sorted mined heads, Random(20260941 + i) samples up to 30
train-fold, 10 val-fold and 10 cal-fold candidates, from each fold's candidates sorted by
(manifest id, unit index). Folds are the manifest's; no new split is drawn. The draw is then
checked (check_no_cross_fold_overlap): no drawn candidate may share a shingle with a frozen row or
another drawn candidate in a different fold.

--count-only prints per-head, per-fold counts after the filters, and the cross-fold overlap among
candidates that step 4 resolves. It draws nothing and writes nothing, so it can run before
registration without showing which units would be drawn; it does not need the Codex texts, and
it says so. Without it, the script refuses to run until codex-clean-texts.jsonl exists, because
the Codex texts must be in filter 1 before any candidate is drawn. It then writes
counterexamples/candidates.jsonl and counterexamples/summary.txt.
"""
import argparse
import collections
import hashlib
import json
import random
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
STAGE2 = HERE.parent / "stage2"
sys.path.insert(0, str(HERE.parent / "stage3"))
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(STAGE2))
import clean_texts as ct  # noqa: E402
import freeze_stage2 as fz  # noqa: E402

SEED = 20260941
CAPS = {"train": 30, "val": 10, "cal": 10}
FOLDS = ("train", "val", "cal")
FOLD_PRIORITY = ("val", "cal", "train")      # step 4 keeps the earlier fold's candidate
OUT = HERE / "counterexamples"
shingles = fz.gs.mp.shingles


def mined_cues(cue_list: dict) -> dict[str, list[str]]:
    """Per head, the cues the cross-rule term cannot counter: cue-bearing train positives
    outnumber cue-bearing train negatives plus cue-bearing cross-pool units."""
    out = {}
    for head, cues in sorted(cue_list["heads"].items()):
        keep = [c["feature"] for c in cues
                if c["train_pos"][0] > c["train_neg"][0] + c["cross_pool_train_units"][0]]
        if keep:
            out[head] = keep
    return out


def find_candidates(cues: dict[str, list[str]], paragraphs, segment, tokens) -> list[dict]:
    """Every (head, unit) whose unit carries one of the head's mined cues."""
    all_cues = set().union(*cues.values())
    out = []
    for m, text in paragraphs:
        units = segment(text)
        hits = [(i, u, all_cues & set(tokens(u))) for i, u in enumerate(units)]
        for head, hc in cues.items():
            for i, u, found in hits:
                matched = sorted(c for c in found if c in hc)
                if matched:
                    out.append({"cid": f"cx-{head}-{m['id']}-{i}", "head": head, "cues": matched,
                                "fold": m["fold"], "manifest_id": m["id"], "path": m["path"],
                                "para": m["para"], "group": m["group"], "unit_index": i, "unit": u,
                                "text": text, "text_sha1": hashlib.sha1(text.encode()).hexdigest()})
    return out


def filter_against_existing(cands: list[dict], held: set, frozen_by_fold: dict[str, set],
                            frozen_by_rule: dict[str, set]) -> tuple[list[dict], collections.Counter]:
    """Filters 1-3: held-out text, frozen rows in another fold, frozen rows of the head's rule."""
    other = {f: set().union(*(s for g, s in frozen_by_fold.items() if g != f)) for f in FOLDS}
    kept, n = [], collections.Counter()
    for c in cands:
        key = (c["head"], c["fold"])
        n[(*key, "found")] += 1
        sh = shingles(c["text"])
        if sh & held:
            n[(*key, "dropped: held-out shingle")] += 1
        elif sh & other[c["fold"]]:
            n[(*key, "dropped: cross-fold shingle")] += 1
        elif sh & frozen_by_rule.get(c["head"], set()):
            n[(*key, "dropped: own-rule row shingle")] += 1
        else:
            kept.append(c)
    return kept, n


def resolve_new_vs_new(cands: list[dict]) -> tuple[list[dict], collections.Counter]:
    """Filter 4: of candidates in different folds sharing a shingle, keep the earlier fold's."""
    kept, n, earlier = [], collections.Counter(), set()
    for fold in FOLD_PRIORITY:
        this_fold = set()
        for c in (c for c in cands if c["fold"] == fold):
            sh = shingles(c["text"])
            if sh & earlier:
                n[(c["head"], fold, "dropped: shingle shared with a candidate in an earlier fold")] += 1
            else:
                kept.append(c)
                this_fold |= sh
        earlier |= this_fold
    return kept, n


def cross_fold_overlap(cands: list[dict]) -> tuple[int, int]:
    """(distinct shingles shared by candidates in different folds, distinct paragraphs holding one)."""
    folds_of = collections.defaultdict(set)
    for c in cands:
        for s in shingles(c["text"]):
            folds_of[s].add(c["fold"])
    shared = {s for s, fs in folds_of.items() if len(fs) > 1}
    paras = {c["text_sha1"] for c in cands if shingles(c["text"]) & shared}
    return len(shared), len(paras)


def draw(cands: list[dict], heads: list[str]) -> list[dict]:
    drawn = []
    for hi, head in enumerate(sorted(heads)):
        rng = random.Random(SEED + hi)
        for fold in FOLDS:
            pool = sorted((c for c in cands if c["head"] == head and c["fold"] == fold),
                          key=lambda c: (c["manifest_id"], c["unit_index"]))
            drawn.extend(rng.sample(pool, min(CAPS[fold], len(pool))))
    return drawn


def check_no_cross_fold_overlap(drawn: list[dict], frozen_by_fold: dict[str, set]) -> None:
    """The augmented folds' invariant: no drawn candidate shares a shingle with a frozen row or
    with another drawn candidate in a different fold. Raises naming the first violation."""
    drawn_by_fold = collections.defaultdict(set)
    for c in drawn:
        drawn_by_fold[c["fold"]] |= shingles(c["text"])
    for c in drawn:
        sh = shingles(c["text"])
        for f in FOLDS:
            if f != c["fold"] and sh & (drawn_by_fold[f] | frozen_by_fold.get(f, set())):
                raise SystemExit(f"cross-fold overlap: {c['cid']} ({c['fold']}) shares a shingle with {f}")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--count-only", action="store_true")
    args = ap.parse_args()
    import train_arm as ta           # torch and sklearn, here so the helpers import without them
    import surface_probe as sp
    import cue_list as cl

    ct.check_against_doc()
    codex = ct.codex_clean()
    if codex is None and not args.count_only:
        raise SystemExit(f"refused: {ct.CODEX_CLEAN.name} does not exist; Step 2's Codex texts "
                         "must be in the held-out filter before candidates are drawn")
    cues = mined_cues(json.loads((HERE / "cue-list.json").read_text()))

    held = fz.held_out_shingles()
    for _, text in ct.NEW_CLEAN + (codex or []):
        held |= shingles(text)
    frozen_by_fold = collections.defaultdict(set)
    frozen_by_rule = collections.defaultdict(set)
    for f in FOLDS:
        for r in ta.load_rows(f):
            sh = shingles(r["text"])
            frozen_by_fold[r["set"]] |= sh
            frozen_by_rule[r["rule"]] |= sh

    found = find_candidates(cues, cl.training_side_paragraphs(), ta.segment, sp.tokens)
    passed, n = filter_against_existing(found, held, frozen_by_fold, frozen_by_rule)
    shared, touched = cross_fold_overlap(passed)
    eligible, n4 = resolve_new_vs_new(passed)
    n.update(n4)

    lines = [f"mined cues: {json.dumps(cues)}",
             f"held-out filter includes the Codex texts: {codex is not None}",
             f"after filters 1-3: {len({c['manifest_id'] for c in passed})} distinct manifest paragraphs "
             f"({len({c['text_sha1'] for c in passed})} distinct texts); {shared} distinct shingles shared "
             f"across folds, in {touched} distinct texts (filter 4 resolves them)"]
    drawn = [] if args.count_only else draw(eligible, list(cues))
    for head in sorted(cues):
        for fold in FOLDS:
            pool = [c for c in eligible if c["head"] == head and c["fold"] == fold]
            take = [c for c in drawn if c["head"] == head and c["fold"] == fold]
            reasons = {k[2]: v for k, v in n.items() if k[:2] == (head, fold) and k[2] != "found"}
            lines.append(f"{head:20s} {fold:5s} found {n[(head, fold, 'found')]:4d}  eligible "
                         f"{len(pool):4d}  drawn {len(take):3d}  {reasons}")
    print("\n".join(lines))
    if args.count_only:
        print("count only: nothing drawn, nothing written")
        return 0
    check_no_cross_fold_overlap(drawn, frozen_by_fold)
    OUT.mkdir(exist_ok=True)
    (OUT / "candidates.jsonl").write_text(
        "".join(json.dumps(c, ensure_ascii=False, sort_keys=True) + "\n" for c in drawn))
    (OUT / "summary.txt").write_text("\n".join(lines) + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
