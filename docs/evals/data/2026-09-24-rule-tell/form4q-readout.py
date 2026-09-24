"""Form 4q readout against its registration (6aa61dff): Q1 gold on RTD-1/11/12, Q2 negatives' any-fire vs form 3."""
import importlib.util, json, pathlib, sys

ROOT = pathlib.Path("/home/marius/work/claude/codescout")
spec = importlib.util.spec_from_file_location("sel", ROOT / "scripts/phase1-span-selector.py")
sel = importlib.util.module_from_spec(spec); spec.loader.exec_module(sel)


def load(p):
    return [json.loads(l) for l in pathlib.Path(p).read_text().splitlines() if l.strip()]


def by_text(rows):
    out = {}
    for r in rows:
        out.setdefault((r["case"], r["side"]), []).append(r)
    return out


f3 = by_text(load(ROOT / "docs/evals/data/2026-09-24-rule-tell/p1s-S0f3-corpus.jsonl"))
f4 = by_text(load(sys.argv[1]))
print("Q1 -- question_asked positives:")
for cid in ("RTD-1", "RTD-11", "RTD-12"):
    for name, t in (("form 3", f3), ("form 4q", f4)):
        rs = t[(cid, "positive")]
        fired = [(r["rule"], r.get("claim", "")[:70]) for r in sel.fired(rs)]
        print(f"  {cid:7} {name:8} fired={fired}")
print("\nQ2 -- negatives with any fire:")
for name, t in (("form 3", f3), ("form 4q", f4)):
    negs = {k: [r["rule"] for r in sel.fired(v)] for k, v in t.items() if k[1] == "negative"}
    hit = {k: v for k, v in negs.items() if v}
    print(f"  {name:8} {len(hit)}/{len(negs)}  {sorted((k[0], v) for k, v in hit.items())}")
print("\nquestion_asked fires anywhere under 4q:")
for k, rs in sorted(f4.items()):
    for r in sel.fired(rs):
        if r["rule"] == "question_asked":
            print(f"  {k}  gold={rs[0].get('gold')}  {r['claim'][:90]!r}")
