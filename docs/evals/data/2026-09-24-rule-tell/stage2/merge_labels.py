"""Merge the 8 label batches and refuse anything but a complete, valid label set.

    python3 merge_labels.py <labels_dir> <out.jsonl>

Exit 2 on any missing, duplicated or unknown id, or any label outside the registered set.
Prints the label distribution and the sha256 of the merged file (sorted by id).
"""
import collections, hashlib, importlib.util, json, pathlib, sys

HERE = pathlib.Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("sel", HERE.parents[4] / "scripts/phase1-span-selector.py")
sel = importlib.util.module_from_spec(spec); spec.loader.exec_module(sel)
ALLOWED = set(sel.RULES) | {"not-a-violation", "not-a-pair", "unsure"}


def main() -> int:
    d, out = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2])
    n_rows = len((HERE / "mined-candidates.jsonl").read_text().splitlines())
    rows, bad = [], []
    for f in sorted(d.glob("labels-batch-*.jsonl")):
        for ln in f.read_text().splitlines():
            if not ln.strip():
                continue
            try:
                r = json.loads(ln)
            except json.JSONDecodeError:
                bad.append(f"{f.name}: not JSON: {ln[:80]!r}")
                continue
            if r.get("label") not in ALLOWED:
                bad.append(f"{f.name}: id {r.get('id')} label {r.get('label')!r}")
            if r.get("second_rule") not in (None, *sel.RULES):
                bad.append(f"{f.name}: id {r.get('id')} second_rule {r.get('second_rule')!r}")
            rows.append(r)
    ids = collections.Counter(r.get("id") for r in rows)
    missing = sorted(set(range(n_rows)) - ids.keys())
    dup = sorted(i for i, c in ids.items() if c > 1)
    unknown = sorted(i for i in ids if i not in range(n_rows))
    print(f"{len(rows)} labels for {n_rows} rows: missing {len(missing)}, duplicated {len(dup)}, "
          f"unknown {len(unknown)}, invalid {len(bad)}")
    for x in bad[:20]:
        print("  ", x)
    if missing or dup or unknown or bad:
        print(f"missing ids (first 20): {missing[:20]}  duplicated: {dup[:20]}  unknown: {unknown[:20]}")
        return 2
    rows.sort(key=lambda r: r["id"])
    out.write_text("".join(json.dumps(r, sort_keys=True) + "\n" for r in rows))
    print("labels:", dict(collections.Counter(r["label"] for r in rows).most_common()))
    print("sha256:", hashlib.sha256(out.read_bytes()).hexdigest())
    return 0


if __name__ == "__main__":
    sys.exit(main())
