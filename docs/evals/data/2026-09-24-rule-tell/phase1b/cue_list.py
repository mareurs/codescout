"""Cue list for phase-1b Stage 2's counterexamples (arm NC), and how many training-side corpus
units carry each cue. Written for the Stage 2 draft, before registration.

    ~/work/claude/jevk5/.venv/bin/python cue_list.py

Reads `train` only, for the probe, and the registered seed manifest's training-side paragraphs
(stage2/seed-manifest.jsonl, side == "training"), for the counts. It reads no val, cal, T,
T-syn or S text.

Cues. For each menu head whose surface-probe val AUC (surface-probe.json) is at least 0.9, fit
surface_probe.py's probe on that head's train target units with the real labels. The K = 3
features with the largest positive coefficients are its cues: a feature is a lowercase token or
bigram, featurised exactly as the probe does, and a unit carries it when it is in the unit's
feature set.

Counts, per cue:
  - train target units of the head carrying it, split into positive and negative;
  - the cross-rule pool: units of OTHER rules' train rows carrying it. Step 3's cross-rule term
    makes every such unit a negative for the head (if the head is admitted), so a cue the pool
    already carries is countered without mining; a cue the pool lacks is not;
  - training-side manifest units carrying it, by fold. Each paragraph's text is re-read from git
    at the manifest's commit, checked against the manifest's sha1, and cut with the training
    segmenter.

Writes cue-list.json next to this file, and prints a table.
"""
import collections
import hashlib
import importlib.util
import json
import sys
from pathlib import Path

from sklearn.feature_extraction.text import CountVectorizer
from sklearn.linear_model import LogisticRegression

HERE = Path(__file__).resolve().parent
STAGE2 = HERE.parent / "stage2"
sys.path.insert(0, str(HERE.parent / "stage3"))
sys.path.insert(0, str(HERE))
import train_arm as ta  # noqa: E402
import surface_probe as sp  # noqa: E402

K = 3
PROBE_BAR = 0.9

_spec = importlib.util.spec_from_file_location("extract_seeds", STAGE2 / "extract_seeds.py")
es = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(es)


def cues_for(rows: list[dict]) -> list[dict]:
    vec = CountVectorizer(analyzer=sp.tokens, binary=True)
    x = vec.fit_transform([sp.target_unit(r) for r in rows])
    clf = LogisticRegression(C=1.0, max_iter=5000).fit(x, [r["label"] for r in rows])
    names, coef = vec.get_feature_names_out(), clf.coef_[0]
    top = sorted(range(len(names)), key=lambda i: (-coef[i], names[i]))[:K]
    return [{"feature": str(names[i]), "coef": round(float(coef[i]), 4)} for i in top]


def training_side_paragraphs() -> list[tuple[dict, str]]:
    """(manifest row, paragraph text) for every training-side paragraph of the registered seed
    manifest, re-read from git at the manifest's commit and checked against its sha1."""
    man = [json.loads(line) for line in (STAGE2 / "seed-manifest.jsonl").read_text().splitlines()]
    by_path = collections.defaultdict(list)
    for m in man:
        if m["side"] == "training":
            by_path[m["path"]].append(m)
    out = []
    for path in sorted(by_path):
        paras = es.paragraphs(es.git("show", f"{es.COMMIT}:{path}"))
        for m in by_path[path]:
            p = paras[m["para"]]
            if hashlib.sha1(p.encode()).hexdigest() != m["sha1"]:
                raise SystemExit(f"manifest sha1 mismatch: {path} paragraph {m['para']}")
            out.append((m, p))
    return out


def training_side_units() -> list[tuple[str, str]]:
    """(fold, unit text) for every unit of every training-side manifest paragraph."""
    return [(m["fold"], u) for m, p in training_side_paragraphs() for u in ta.segment(p)]


def main() -> int:
    probe = json.loads((HERE / "surface-probe.json").read_text())["per_rule"]
    train = ta.load_rows("train")
    heads = [r for r in ta.menu_from_manifest() if probe[r]["probe_auc"] >= PROBE_BAR]
    units = training_side_units()
    feats = [(f, set(sp.tokens(u))) for f, u in units]
    cross = {h: [set(sp.tokens(u)) for r in train if r["rule"] != h for u in ta.segment(r["text"])]
             for h in heads}
    folds = sorted({f for f, _ in units})
    result = {"k": K, "probe_bar": PROBE_BAR, "manifest_commit": es.COMMIT,
              "training_side_units": dict(collections.Counter(f for f, _ in units)), "heads": {}}
    print(f"training-side units: {len(units)} {result['training_side_units']}")
    print(f"{'head':22s} {'cue':24s} {'coef':>7s} {'train pos':>10s} {'train neg':>10s} {'cross pool':>11s}  corpus units by fold")
    for h in heads:
        rows = [r for r in train if r["rule"] == h]
        entry = []
        for c in cues_for(rows):
            f = c["feature"]
            pos = sum(f in set(sp.tokens(sp.target_unit(r))) for r in rows if r["label"] == 1)
            neg = sum(f in set(sp.tokens(sp.target_unit(r))) for r in rows if r["label"] == 0)
            npos = sum(r["label"] == 1 for r in rows)
            by_fold = {fo: sum(1 for fu, s in feats if fu == fo and f in s) for fo in folds}
            pool = [sum(f in s for s in cross[h]), len(cross[h])]
            entry.append({**c, "train_pos": [pos, npos], "train_neg": [neg, len(rows) - npos],
                          "cross_pool_train_units": pool, "corpus_units": by_fold})
            print(f"{h:22s} {f!r:24s} {c['coef']:7.3f} {pos:>4d}/{npos:<5d} {neg:>4d}/{len(rows) - npos:<5d} "
                  f"{pool[0]:>5d}/{pool[1]:<5d}  {by_fold}")
        result["heads"][h] = entry
    (HERE / "cue-list.json").write_text(json.dumps(result, indent=1))
    return 0


if __name__ == "__main__":
    sys.exit(main())
