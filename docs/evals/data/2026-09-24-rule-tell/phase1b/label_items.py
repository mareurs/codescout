"""Phase 1b Step 1: the labellers' items, blind to where each came from.

    python3 label_items.py [--audit DIR]        (default DIR: phase1b/audit)

The pre-registration puts three kinds of item through Step 1's two labellers in the same runs:
the audit sample (draw_audit_sample.py), phase 1b's new clean texts (clean_texts.py: clean-6 to
clean-14 and the three Codex texts) and the counterexample candidates (mine_counterexamples.py).
A labeller told, or able to tell, which kind an item is could judge the kinds differently, so:
- every item has the same shape, {"id", "sentence", "paragraph"}; a clean text is one item whose
  sentence and paragraph are both the whole text;
- all items go through ONE random.Random(20260942).shuffle, from a fixed order (audit items by
  draw index, clean texts in registration order, candidates sorted by cid), and are then numbered
  L0001, L0002, ... in shuffled order, so an id says nothing about the item's kind.
The Claude labeller reads items.jsonl in fixed batches of 25, in file order; Codex reads it whole.

Writes DIR/items.jsonl (what the labellers see) and DIR/key.jsonl (id -> kind and reference, read
by score_audit.py alone). Refuses until the Codex texts and the drawn candidates both exist,
because they are labelled in the same runs as the audit sample.
"""
import argparse
import json
import random
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import clean_texts as ct  # noqa: E402

SEED = 20260942
BATCH = 25


def entries(sample: list[dict], clean: list[tuple[str, str]], candidates: list[dict]) -> list[dict]:
    """The fixed order the shuffle starts from."""
    out = [{"source": "audit", "ref": s["item"], "sentence": s["sentence"], "paragraph": s["paragraph"]}
           for s in sorted(sample, key=lambda s: s["item"])]
    out += [{"source": "clean", "ref": cid, "sentence": text, "paragraph": text} for cid, text in clean]
    out += [{"source": "counterexample", "ref": c["cid"], "sentence": c["unit"], "paragraph": c["text"]}
            for c in sorted(candidates, key=lambda c: c["cid"])]
    return out


def blind(ents: list[dict]) -> tuple[list[dict], list[dict]]:
    """(items, key): one seeded shuffle, then ids in shuffled order."""
    order = list(ents)
    random.Random(SEED).shuffle(order)
    items, key = [], []
    for n, e in enumerate(order, 1):
        lid = f"L{n:04d}"
        items.append({"id": lid, "sentence": e["sentence"], "paragraph": e["paragraph"]})
        key.append({"id": lid, "source": e["source"], "ref": e["ref"], "sentence": e["sentence"]})
    return items, key


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--audit", type=Path, default=HERE / "audit")
    ap.add_argument("--candidates", type=Path, default=HERE / "counterexamples" / "candidates.jsonl")
    args = ap.parse_args()
    ct.check_against_doc()
    codex = ct.codex_clean()
    cand_path = args.candidates
    if codex is None or not cand_path.exists():
        raise SystemExit("refused: the Codex clean texts and the drawn counterexample candidates are labelled "
                         "in the same runs as the audit sample; generate both first")
    sample = [json.loads(line) for line in (args.audit / "sample.jsonl").read_text().splitlines()]
    candidates = [json.loads(line) for line in cand_path.read_text().splitlines() if line.strip()]
    items, key = blind(entries(sample, ct.NEW_CLEAN + codex, candidates))
    (args.audit / "items.jsonl").write_text("".join(json.dumps(i, ensure_ascii=False) + "\n" for i in items))
    (args.audit / "key.jsonl").write_text("".join(json.dumps(k, ensure_ascii=False) + "\n" for k in key))
    kinds = {s: sum(k["source"] == s for k in key) for s in ("audit", "clean", "counterexample")}
    print(f"{len(items)} items {kinds}; Claude batches of {BATCH}: {-(-len(items) // BATCH)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
