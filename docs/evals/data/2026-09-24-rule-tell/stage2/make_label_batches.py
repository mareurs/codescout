"""Build the labelling batches and the operator's blind sample (registered protocol).

    python3 make_label_batches.py <out_dir> [n_batches]

Rows get `id` = their 0-based index in mined-candidates.jsonl. Batches are the rows shuffled
with seed 20260925 and cut into n equal slices. The operator's sample is
random.Random(20260926).sample(ids, 40). Only the fields the protocol shows a labeller are
written: rule_hint, marker, path and every label are left out.
"""
import importlib.util, json, pathlib, random, sys

HERE = pathlib.Path(__file__).resolve().parent
ROOT = HERE.parents[4]
spec = importlib.util.spec_from_file_location("sel", ROOT / "scripts/phase1-span-selector.py")
sel = importlib.util.module_from_spec(spec); spec.loader.exec_module(sel)

SHOWN = ("positive", "context_before", "twin", "note", "subject")


def main() -> int:
    out = pathlib.Path(sys.argv[1]); out.mkdir(parents=True, exist_ok=True)
    n = int(sys.argv[2]) if len(sys.argv) > 2 else 8
    rows = [json.loads(l) for l in (HERE / "mined-candidates.jsonl").read_text().splitlines()]
    shown = [{"id": i, **{k: r.get(k) for k in SHOWN}} for i, r in enumerate(rows)]
    menu = {k: {"law": sel.RULES[k], "spec": sel.SPECS[k]} for k in sel.RULES}
    (out / "menu.json").write_text(json.dumps(menu, indent=1))
    order = list(range(len(shown)))
    random.Random(20260925).shuffle(order)
    for b in range(n):
        ids = order[b::n]
        with open(out / f"batch-{b}.jsonl", "w") as fh:
            for i in ids:
                fh.write(json.dumps(shown[i]) + "\n")
    sample = random.Random(20260926).sample(list(range(len(shown))), 40)
    with open(out / "operator-sample.jsonl", "w") as fh:
        for i in sample:
            fh.write(json.dumps(shown[i]) + "\n")
    print(f"{len(shown)} rows -> {n} batches in {out}; operator sample 40 ids: {sorted(sample)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
