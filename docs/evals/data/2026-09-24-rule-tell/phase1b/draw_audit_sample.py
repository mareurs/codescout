"""Phase 1b Step 1: the cross-rule audit's menu and sample, before any labelling.

    python3 draw_audit_sample.py [--out DIR]        (default DIR: phase1b/audit)

Implements docs/evals/phase1b-local-classifier-preregistration.md § Step 1:
- menu.json: the 14 local rules, each with its LAW text and its form-2b SPEC text, taken from the
  gate code's own strings (phase1-span-selector.py's RULES and SPEC_FORMS["2b"]), so they are
  byte-identical to phase 1's. Phase 1 built its labelling menu the same way
  (stage2/make_label_batches.py) and committed no menu.json, so the strings are the reference.
- sample.jsonl: 300 rows by random.Random(20260936).sample over the 2,671 rows of train, val and
  cal in file order (train, val, cal); then, for the row at draw index i, one unit uniformly by
  random.Random(20260936 + i).randrange over its text's units, cut by the training segmenter.
  Each line keeps the row's rule, fold and target, which the scorer needs and no labeller sees:
  labeller-facing items are built later, blind, by label_items.py.

Reads the frozen train/val/cal files only. Draws nothing from T, the T-syn sets or a gate text.
"""
import argparse
import importlib.util
import json
import random
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
STAGE2 = HERE.parent / "stage2"
FROZEN = STAGE2 / "frozen"
ROOT = HERE.parents[4]
sys.path.insert(0, str(STAGE2))
from segment import segment  # noqa: E402

SEED = 20260936
N_ROWS = 300
POPULATION = 2671               # train 1,791 + val 521 + cal 359, the registered population
FOLDS = ("train", "val", "cal")


def load_gate():
    spec = importlib.util.spec_from_file_location("span_selector", ROOT / "scripts/phase1-span-selector.py")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def local_menu() -> list[str]:
    man = json.loads((FROZEN / "freeze-manifest.json").read_text())
    return sorted(man["files"]["train"]["per_rule"])      # train_arm.menu_from_manifest, without torch


def build_menu(gate, menu: list[str]) -> dict:
    return {k: {"law": gate.RULES[k], "spec": gate.SPEC_FORMS["2b"][k]} for k in menu}


def population() -> list[dict]:
    return [json.loads(line) for f in FOLDS for line in (FROZEN / f"{f}.jsonl").read_text().splitlines()]


def draw(rows: list[dict], seg=segment) -> list[dict]:
    if len(rows) != POPULATION:
        raise SystemExit(f"refused: the population is {len(rows)} rows, not the registered {POPULATION}")
    out = []
    for i, j in enumerate(random.Random(SEED).sample(range(len(rows)), N_ROWS)):
        r = rows[j]
        units = seg(r["text"])
        u = random.Random(SEED + i).randrange(len(units))
        out.append({"item": i, "row_id": r["id"], "set": r["set"], "rule": r["rule"], "label": r["label"],
                    "target": r["target"], "unit_index": u, "is_target": u == r["target"],
                    "sentence": units[u], "paragraph": r["text"]})
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", type=Path, default=HERE / "audit")
    args = ap.parse_args()
    menu = local_menu()
    sample = draw(population())
    args.out.mkdir(parents=True, exist_ok=True)
    (args.out / "menu.json").write_text(json.dumps(build_menu(load_gate(), menu), indent=1, ensure_ascii=False) + "\n")
    (args.out / "sample.jsonl").write_text("".join(json.dumps(s, ensure_ascii=False) + "\n" for s in sample))
    per_rule = {r: sum(s["rule"] != r for s in sample) for r in menu}
    print(f"{len(sample)} unit instances from {len({s['row_id'] for s in sample})} rows; "
          f"{sum(s['is_target'] for s in sample)} are their row's target unit")
    print("audit cells per head (sampled rows of other rules): " + json.dumps(per_rule))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
