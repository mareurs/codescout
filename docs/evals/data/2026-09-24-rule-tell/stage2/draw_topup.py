"""Draw the top-up seeds (top-up amendment, change 3).

    python3 draw_topup.py            # writes seed-topup.jsonl

Registered: one random.Random(20260933); rules in sorted order; per rule `topup-plan.json`'s
seed count, sampled from the manifest's training-side ids with no prior use, without reuse
across rules. Each seed's text is re-extracted from extract_seeds.COMMIT with the extractor's
own paragraphs() and must match the manifest row's sha1, or nothing is written.
"""
import collections, hashlib, json, pathlib, random, sys

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import extract_seeds as ex          # noqa: E402


def main() -> int:
    plan = json.loads((HERE / "topup-plan.json").read_text())["rules"]
    manifest = [json.loads(l) for l in (HERE / "seed-manifest.jsonl").read_text().splitlines()]
    pool = [m["id"] for m in manifest if m["side"] == "training" and m["use"] is None]
    by_id = {m["id"]: m for m in manifest}
    rng = random.Random(20260933)
    picked = {}
    for rule in sorted(plan):
        k = plan[rule]["seeds"]
        chosen = rng.sample(pool, k)
        taken = set(chosen)
        pool = [i for i in pool if i not in taken]
        for i in chosen:
            picked[i] = rule

    by_path = collections.defaultdict(list)
    for i in picked:
        by_path[by_id[i]["path"]].append(i)
    rows = []
    for path, ids in sorted(by_path.items()):
        paras = ex.paragraphs(ex.git("show", f"{ex.COMMIT}:{path}"))
        for i in ids:
            m = by_id[i]
            text = paras[m["para"]]
            if hashlib.sha1(text.encode()).hexdigest() != m["sha1"]:
                sys.exit(f"sha1 mismatch for seed {i} ({path} #{m['para']}); nothing written")
            rows.append({**m, "use": f"topup:{picked[i]}", "text": text})
    rows.sort(key=lambda r: r["id"])
    with open(HERE / "seed-topup.jsonl", "w") as fh:
        for r in rows:
            fh.write(json.dumps(r, ensure_ascii=False) + "\n")
    per = collections.Counter(r["use"] for r in rows)
    print(f"drew {len(rows)} top-up seeds over {len(per)} rules; folds {dict(collections.Counter(r['fold'] for r in rows))}")
    assert all(per[f"topup:{r}"] == plan[r]["seeds"] for r in plan)
    return 0


if __name__ == "__main__":
    sys.exit(main())
