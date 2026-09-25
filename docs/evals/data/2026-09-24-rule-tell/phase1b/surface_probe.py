"""Surface-token probe, for the phase-1b design (phase-1 pre-registration, § Diagnostics after
the stop). Reads train and val only.

Per menu rule: a bag-of-tokens logistic regression on the TARGET UNIT's text alone, fitted on
that rule's train rows and scored by AUC on that rule's val rows. A high AUC means the rule's
positive and negative target units are separable by surface tokens, which a trained arm can
learn instead of the rule. Tokens keep punctuation, so `&&` and `;` are features.

Control: the same probe fitted on within-rule shuffled train labels (`random.Random(20260939)`)
should score near 0.5; if it does not, the probe itself leaks and its numbers are withheld.

Token tally: fixed probe patterns, counted in positive vs negative target units (train + val).
"""
import json
import random
import re
import sys
from pathlib import Path

from sklearn.feature_extraction.text import CountVectorizer
from sklearn.linear_model import LogisticRegression
from sklearn.metrics import roc_auc_score

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "stage3"))
import train_arm as ta  # noqa: E402

TOKEN_RE = re.compile(r"[a-z0-9_]+|&&|\|\||[^\sa-z0-9_]")
CONTROL_SEED = 20260939
PROBES = {
    "d_semicolon": {"&&": r"&&", ";": r";"},
    "d_sessionid": {"a codescout-XX name": r"codescout-[0-9a-z]{2}\b", "sessionid": r"session.?id"},
}


def tokens(s: str) -> list[str]:
    t = TOKEN_RE.findall(s.lower())
    return t + [f"{a} {b}" for a, b in zip(t, t[1:])]


def target_unit(row: dict) -> str:
    return ta.segment(row["text"])[row["target"]]


def probe_auc(tr: list[dict], va: list[dict], labels: list[int]) -> float:
    vec = CountVectorizer(analyzer=tokens, binary=True)
    x = vec.fit_transform([target_unit(r) for r in tr])
    clf = LogisticRegression(C=1.0, max_iter=5000).fit(x, labels)
    p = clf.predict_proba(vec.transform([target_unit(r) for r in va]))[:, 1]
    return roc_auc_score([r["label"] for r in va], p)


def main() -> int:
    menu = ta.menu_from_manifest()
    train, val = ta.load_rows("train"), ta.load_rows("val")
    rng = random.Random(CONTROL_SEED)
    out = {"per_rule": {}, "tally": {}}
    print(f"{'rule':22s} {'train n':>8s} {'val n':>6s} {'probe AUC':>10s} {'control AUC':>12s}")
    for rule in menu:
        tr = [r for r in train if r["rule"] == rule]
        va = [r for r in val if r["rule"] == rule]
        auc = probe_auc(tr, va, [r["label"] for r in tr])
        shuffled = [r["label"] for r in tr]
        rng.shuffle(shuffled)
        ctl = probe_auc(tr, va, shuffled)
        out["per_rule"][rule] = {"train_n": len(tr), "val_n": len(va), "probe_auc": auc, "control_auc": ctl}
        print(f"{rule:22s} {len(tr):>8d} {len(va):>6d} {auc:>10.3f} {ctl:>12.3f}")
    aucs = [v["probe_auc"] for v in out["per_rule"].values()]
    ctls = [v["control_auc"] for v in out["per_rule"].values()]
    print(f"rules with probe AUC >= 0.9: {sum(a >= 0.9 for a in aucs)} of {len(menu)}; "
          f"control AUC mean {sum(ctls) / len(ctls):.3f}, range {min(ctls):.3f}-{max(ctls):.3f}")
    rows = train + val
    for rule, pats in PROBES.items():
        for name, pat in pats.items():
            rx = re.compile(pat, re.I)
            pos = [r for r in rows if r["rule"] == rule and r["label"] == 1]
            neg = [r for r in rows if r["rule"] == rule and r["label"] == 0]
            oth = [r for r in rows if r["rule"] != rule]
            c = lambda rs: sum(bool(rx.search(target_unit(r))) for r in rs)  # noqa: E731
            out["tally"][f"{rule} / {name}"] = {"pos": [c(pos), len(pos)], "neg": [c(neg), len(neg)],
                                               "other_rules": [c(oth), len(oth)]}
            print(f"  {rule} target units containing {name!r}: positives {c(pos)}/{len(pos)}, "
                  f"negatives {c(neg)}/{len(neg)}, other rules' rows {c(oth)}/{len(oth)}")
    (HERE / "surface-probe.json").write_text(json.dumps(out, indent=1))
    return 0


if __name__ == "__main__":
    sys.exit(main())
