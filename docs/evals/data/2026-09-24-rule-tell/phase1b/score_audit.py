"""Phase 1b Step 1: from the two labellers' answers to the admission file, arm NC's counterexample
rows and Step 2's clean-text verdicts.

    python3 score_audit.py [--audit DIR]        (default DIR: phase1b/audit)

Implements docs/evals/phase1b-local-classifier-preregistration.md § Step 1, § Step 2 and § Stage 2
open decision 1 (a):
- Answers: DIR/labels-codex.jsonl and DIR/labels-claude.jsonl. Each must answer every item of
  DIR/key.jsonl exactly once with a valid object: `rules` and `unsure_rules` lists of menu keys,
  `unsure` a boolean that is true exactly when `unsure_rules` is non-empty. Anything else is
  refused here; run_labellers.py re-runs a bad call once, and a file that still fails stops
  phase 1b at Step 1.
- The union rule: an item is flagged for rule R when either labeller lists R or is unsure about R.
- Admission, per head B: B's audit cells are the audit items whose row's rule is not B. B is
  admitted when the Wilson 95% upper bound of flagged cells over audit cells is at most 5%.
- Masking: every (sentence, R) flagged on an audit or a counterexample item is written to the
  admission file, and train_arm keeps that (unit text, R) cell masked everywhere the text appears.
  Masking costs data and never correctness. Clean texts are gate texts, never training input, so
  their flags only drop them.
- Candidates: an item that neither labeller flags for any rule, or is unsure about, becomes a
  label-0 row for its head in its own fold; any other is dropped and counted. A labeller's verdict
  can only drop a candidate, never set a target.
- Clean texts: a text either labeller flags or is unsure about is dropped, not rewritten. Phase 1b
  stops at Step 2 when fewer than 2 of the 3 swap texts, fewer than 2 of the 3 Codex texts, or
  fewer than 7 of the 12 new texts survive.

Writes DIR/admission.json (train_arm --cross), DIR/counterexamples.jsonl (train_arm --extra-rows),
DIR/result.json and DIR/summary.txt. Exits 3 when a Step 2 stop rule fires, after writing.
"""
import argparse
import collections
import json
import math
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import clean_texts as ct  # noqa: E402

LIMIT = 0.05                                    # the admission bound on the Wilson upper bound
SWAP = ("clean-12", "clean-13", "clean-14")     # Step 2's swap texts
LABELLERS = ("codex", "claude")


def wilson_upper(k: int, n: int, z: float = 1.96) -> float:
    """The Wilson 95% upper bound, audit_arithmetic.py's formula."""
    p = k / n
    return (p + z * z / (2 * n) + z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n))) / (1 + z * z / n)


def valid(a, menu: set) -> bool:
    def rule_list(x):
        return isinstance(x, list) and all(isinstance(r, str) and r in menu for r in x)
    return (isinstance(a, dict) and isinstance(a.get("id"), str) and rule_list(a.get("rules"))
            and isinstance(a.get("unsure"), bool) and rule_list(a.get("unsure_rules"))
            and a["unsure"] == bool(a["unsure_rules"]))


def load_answers(lines: list[str], ids: set, menu: set, who: str) -> dict:
    got, problems = {}, []
    for line in lines:
        if not line.strip():
            continue
        a = json.loads(line)
        if not valid(a, menu):
            problems.append(f"invalid answer {a.get('id') if isinstance(a, dict) else a!r}")
        elif a["id"] not in ids:
            problems.append(f"unknown id {a['id']}")
        elif a["id"] in got:
            problems.append(f"duplicate {a['id']}")
        else:
            got[a["id"]] = a
    if ids - set(got):
        problems.append(f"{len(ids - set(got))} items unanswered")
    if problems:
        raise SystemExit(f"refused: {who}: " + "; ".join(problems[:5]))
    return got


def flags(a: dict) -> set:
    return set(a["rules"]) | set(a["unsure_rules"])


def union(codex: dict, claude: dict) -> dict:
    return {i: flags(codex[i]) | flags(claude[i]) for i in codex}


def check_key(key: list[dict], sample: dict, clean: dict, cands: dict) -> None:
    """Every source item is in the key exactly once, under its own text: a key that drifted from
    the items it indexes would score the wrong sentences."""
    seen = collections.Counter((k["source"], k["ref"]) for k in key)
    want = ({("audit", i) for i in sample} | {("clean", c) for c in clean} | {("counterexample", c) for c in cands})
    if set(seen) != want or any(v != 1 for v in seen.values()):
        raise SystemExit("refused: key.jsonl does not index each audit item, clean text and candidate exactly once")
    text = {"audit": lambda r: sample[r]["sentence"], "clean": lambda r: clean[r], "counterexample": lambda r: cands[r]["unit"]}
    bad = [k["id"] for k in key if text[k["source"]](k["ref"]) != k["sentence"]]
    if bad:
        raise SystemExit(f"refused: key sentences differ from their sources: {bad[:5]}")


def admission(key: list[dict], sample: dict, flagged: dict, menu: list[str]) -> dict:
    out = {}
    for b in menu:
        cells = [k["id"] for k in key if k["source"] == "audit" and sample[k["ref"]]["rule"] != b]
        k = sum(b in flagged[i] for i in cells)
        up = wilson_upper(k, len(cells))
        out[b] = dict(cells=len(cells), flagged=k, wilson_upper=up, admitted=up <= LIMIT)
    return out


def masked(key: list[dict], flagged: dict) -> list[dict]:
    pairs = {(k["sentence"], r) for k in key if k["source"] in ("audit", "counterexample") for r in flagged[k["id"]]}
    return [{"unit": u, "head": h} for u, h in sorted(pairs)]


def counterexample_rows(key: list[dict], flagged: dict, cands: dict) -> tuple[list[dict], dict]:
    rows, dropped = [], collections.Counter()
    for k in key:
        if k["source"] != "counterexample":
            continue
        c = cands[k["ref"]]
        if flagged[k["id"]]:
            dropped[f"{c['head']}|{c['fold']}"] += 1
            continue
        rows.append({"id": c["cid"], "set": c["fold"], "rule": c["head"], "source": "counterexample",
                     "text": c["text"], "target": c["unit_index"], "label": 0})
    return sorted(rows, key=lambda r: r["id"]), dict(sorted(dropped.items()))


def clean_verdicts(key: list[dict], flagged: dict, codex_ids: list[str]) -> dict:
    alive = {k["ref"]: not flagged[k["id"]] for k in key if k["source"] == "clean"}
    swap = sum(alive[c] for c in SWAP)
    codex = sum(alive[c] for c in codex_ids)
    total = sum(alive.values())
    stop = []
    if swap < 2:
        stop.append(f"{swap} of 3 swap texts survive")
    if codex < 2:
        stop.append(f"{codex} of 3 Codex texts survive")
    if total < 7:
        stop.append(f"{total} of {len(alive)} new texts survive")
    return dict(survived=sorted(c for c, v in alive.items() if v), dropped=sorted(c for c, v in alive.items() if not v),
                swap=swap, codex=codex, total=total, stop=stop)


def agreement(key: list[dict], codex: dict, claude: dict, menu: list[str]) -> dict:
    """Per rule, over every item: flagged by both, by Codex only, by Claude only. Reported only."""
    out = {}
    for r in menu:
        a = {k["id"] for k in key if r in flags(codex[k["id"]])}
        b = {k["id"] for k in key if r in flags(claude[k["id"]])}
        out[r] = dict(both=len(a & b), codex_only=len(a - b), claude_only=len(b - a))
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--audit", type=Path, default=HERE / "audit")
    ap.add_argument("--candidates", type=Path, default=HERE / "counterexamples" / "candidates.jsonl")
    args = ap.parse_args()
    d = args.audit
    ct.check_against_doc()
    codex_clean = ct.codex_clean()
    if codex_clean is None:
        raise SystemExit("refused: the Codex clean texts do not exist")
    menu = sorted(json.loads((d / "menu.json").read_text()))
    key = [json.loads(line) for line in (d / "key.jsonl").read_text().splitlines()]
    sample = {s["item"]: s for s in map(json.loads, (d / "sample.jsonl").read_text().splitlines())}
    cands = {c["cid"]: c for c in map(json.loads, args.candidates.read_text().splitlines())}
    clean = dict(ct.NEW_CLEAN + codex_clean)
    check_key(key, sample, clean, cands)
    ids = {k["id"] for k in key}
    ans = {w: load_answers((d / f"labels-{w}.jsonl").read_text().splitlines(), ids, set(menu), w) for w in LABELLERS}
    flagged = union(ans["codex"], ans["claude"])

    adm = admission(key, sample, flagged, menu)
    admitted = [b for b in menu if adm[b]["admitted"]]
    mask = masked(key, flagged)
    rows, dropped = counterexample_rows(key, flagged, cands)
    cv = clean_verdicts(key, flagged, [c for c, _ in codex_clean])
    (d / "admission.json").write_text(json.dumps({"admitted": admitted, "masked": mask}, indent=1, ensure_ascii=False) + "\n")
    (d / "counterexamples.jsonl").write_text("".join(json.dumps(r, ensure_ascii=False) + "\n" for r in rows))
    result = dict(admission=adm, admitted=admitted, removed=[b for b in menu if b not in admitted],
                  masked=len(mask), counterexamples=dict(admitted=len(rows), dropped=dropped),
                  clean=cv, agreement=agreement(key, ans["codex"], ans["claude"], menu))
    (d / "result.json").write_text(json.dumps(result, indent=1) + "\n")
    lines = [f"{b:22s} {v['flagged']:3d}/{v['cells']:3d}  upper {v['wilson_upper']:.4f}  "
             f"{'admitted' if v['admitted'] else 'REMOVED (Haiku-only)'}" for b, v in adm.items()]
    lines += [f"masked (unit text, head) cells: {len(mask)}",
              f"counterexample rows admitted: {len(rows)}; dropped by head|fold: {dropped}",
              f"clean texts survived {cv['total']}/12 (swap {cv['swap']}/3, Codex {cv['codex']}/3); dropped {cv['dropped']}"]
    if cv["stop"]:
        lines.append("STOP at Step 2: " + "; ".join(cv["stop"]))
    (d / "summary.txt").write_text("\n".join(lines) + "\n")
    print("\n".join(lines))
    return 3 if cv["stop"] else 0


if __name__ == "__main__":
    raise SystemExit(main())
