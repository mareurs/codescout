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

Filters, in this order; a dropped candidate is counted under its reason and never moved:
  1. held-out: an 8-token shingle shared with freeze_stage2.held_out_shingles() (phase 1's
     held-out texts and gate, the mined T rows, the T-syn pairs and their S seeds) or with any of
     phase 1b's new clean texts (clean_texts: clean-6 to clean-14 and the Codex texts);
  2. cross-fold: a shingle shared with a frozen train/val/cal row's text in another fold;
  3. own rule: a shingle shared with a frozen row's text of the head's own rule, in any fold.
     Those rows keep their non-target own-rule units masked, and a counterexample from the same
     paragraph would unmask one.

Draw: for the head at index i of the sorted mined heads, Random(20260941 + i) samples up to 30
train-fold, 10 val-fold and 10 cal-fold candidates, from each fold's candidates sorted by
(manifest id, unit index). Folds are the manifest's; no new split is drawn.

--count-only prints per-head, per-fold counts after the filters. It draws nothing and writes
nothing, so it can run before registration without showing which units would be drawn; it does
not need the Codex texts, and it says so. Without it, the script refuses to run until
codex-clean-texts.jsonl exists, because the Codex texts must be in filter 1 before any
candidate is drawn. It then writes counterexamples/candidates.jsonl and counterexamples/summary.txt.
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


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--count-only", action="store_true")
    args = ap.parse_args()
    import train_arm as ta           # torch and sklearn, here so mined_cues imports without them
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
    frozen = [r for f in ("train", "val", "cal") for r in ta.load_rows(f)]
    by_fold = collections.defaultdict(set)
    by_rule = collections.defaultdict(set)
    for r in frozen:
        sh = shingles(r["text"])
        by_fold[r["set"]] |= sh
        by_rule[r["rule"]] |= sh

    all_cues = set().union(*cues.values())
    cands = collections.defaultdict(list)              # (head, fold) -> candidates
    n = collections.Counter()
    for m, text in cl.training_side_paragraphs():
        units = ta.segment(text)
        hits = [(i, u, all_cues & set(sp.tokens(u))) for i, u in enumerate(units)]
        hits = [(i, u, found) for i, u, found in hits if found]
        if not hits:
            continue
        sh = shingles(text)
        other_folds = set().union(*(s for f, s in by_fold.items() if f != m["fold"]))
        for head, hc in cues.items():
            for i, u, found in hits:
                matched = sorted(c for c in found if c in hc)
                if not matched:
                    continue
                key = (head, m["fold"])
                n[(*key, "found")] += 1
                if sh & held:
                    n[(*key, "dropped: held-out shingle")] += 1
                elif sh & other_folds:
                    n[(*key, "dropped: cross-fold shingle")] += 1
                elif sh & by_rule[head]:
                    n[(*key, "dropped: own-rule row shingle")] += 1
                else:
                    cands[key].append({
                        "cid": f"cx-{head}-{m['id']}-{i}", "head": head, "cues": matched,
                        "fold": m["fold"], "manifest_id": m["id"], "path": m["path"],
                        "para": m["para"], "group": m["group"], "unit_index": i, "unit": u,
                        "text": text, "text_sha1": hashlib.sha1(text.encode()).hexdigest()})

    lines = [f"mined cues: {json.dumps(cues)}",
             f"held-out filter includes the Codex texts: {codex is not None}"]
    drawn = []
    for hi, head in enumerate(sorted(cues)):
        rng = random.Random(SEED + hi)
        for fold in ("train", "val", "cal"):
            pool = sorted(cands[(head, fold)], key=lambda c: (c["manifest_id"], c["unit_index"]))
            take = [] if args.count_only else rng.sample(pool, min(CAPS[fold], len(pool)))
            drawn.extend(take)
            reasons = {k[2]: v for k, v in n.items() if k[:2] == (head, fold) and k[2] != "found"}
            lines.append(f"{head:20s} {fold:5s} found {n[(head, fold, 'found')]:4d}  eligible "
                         f"{len(pool):4d}  drawn {len(take):3d}  {reasons}")
    print("\n".join(lines))
    if args.count_only:
        print("count only: nothing drawn, nothing written")
        return 0
    OUT.mkdir(exist_ok=True)
    (OUT / "candidates.jsonl").write_text(
        "".join(json.dumps(c, ensure_ascii=False, sort_keys=True) + "\n" for c in drawn))
    (OUT / "summary.txt").write_text("\n".join(lines) + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
