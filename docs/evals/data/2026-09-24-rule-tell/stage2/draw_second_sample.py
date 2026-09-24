"""Draw the second blind sample (registered in the binary-admission amendment).

    python3 draw_second_sample.py <out_dir>

40 rows by random.Random(20260928).sample over the row ids NOT in the first sample, which is
recomputed from its own registered seed (random.Random(20260926).sample(ids, 40)) rather than
read from any label file. Writes only the fields a labeller is shown, like make_label_batches.py.
"""
import json, pathlib, random, sys

HERE = pathlib.Path(__file__).resolve().parent
SHOWN = ("positive", "context_before", "twin", "note", "subject")


def main() -> int:
    out = pathlib.Path(sys.argv[1]); out.mkdir(parents=True, exist_ok=True)
    rows = [json.loads(l) for l in (HERE / "mined-candidates.jsonl").read_text().splitlines()]
    ids = list(range(len(rows)))
    first = set(random.Random(20260926).sample(ids, 40))
    rest = [i for i in ids if i not in first]
    sample = random.Random(20260928).sample(rest, 40)
    assert not first & set(sample) and len(set(sample)) == 40
    with open(out / "sample.jsonl", "w") as fh:
        for i in sample:
            fh.write(json.dumps({"id": i, **{k: rows[i].get(k) for k in SHOWN}}) + "\n")
    print(f"{len(rows)} rows, {len(rest)} outside the first sample; second sample ids: {sorted(sample)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
