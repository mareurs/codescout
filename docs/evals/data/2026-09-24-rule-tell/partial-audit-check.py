"""Mechanical checks for the partial audit (eb48fe28), then S0's rows for the same 9 positives."""
import collections, importlib.util, json, pathlib, sys

ROOT = pathlib.Path("/home/marius/work/claude/codescout")
S = pathlib.Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("sel", ROOT / "scripts/phase1-span-selector.py")
sel = importlib.util.module_from_spec(spec); spec.loader.exec_module(sel)
p1 = sel._p1

cases = {c["id"]: c for c in p1.load_cases(p1.EVAL_SET)}
v = json.loads((S / "partial-audit-verdicts.json").read_text())["verdicts"]
assert sorted(v) == sorted(k for k, c in cases.items() if c["text_detectable"] == "partial"), "population"

J = "\n\n[…]\n\n"
final = {}
for cid, r in v.items():
    pos, neg = J.join(cases[cid]["positive"]), J.join(cases[cid]["negative"])
    ok = True
    for q in [r["quote"]] + ([r["quote2"]] if "quote2" in r else []):
        in_pos = sel.verify_span(q, pos) is not None
        in_neg = sel._norm(q) in sel._norm(neg)
        exempt = cid == "RTD-16"
        print(f"{cid:7} in_pos={in_pos!s:5} absent_from_neg={not in_neg!s:5} {'(exempt)' if exempt else ''} {q[:60]!r}")
        ok &= in_pos and (not in_neg or exempt)
    final[cid] = r["verdict"] if ok else "N"
tally = collections.Counter(final.values())
print("\nverdicts after checks:", dict(sorted(final.items(), key=lambda kv: int(kv[0][4:]))))
print(f"V={tally['V']} S={tally['S']} N={tally['N']}  V+S={tally['V'] + tally['S']}/9")

print("\nS0 rows, partial positives (rules fired / claim quoted):")
for form, f in (("2b", "p1s-S0b-corpus.jsonl"), ("3", "p1s-S0f3-corpus.jsonl")):
    rows = [json.loads(l) for l in (ROOT / "docs/evals/data/2026-09-24-rule-tell" / f).read_text().splitlines() if l.strip()]
    for cid in sorted(v, key=lambda s: int(s[4:])):
        rs = [r for r in rows if r["case"] == cid and r["side"] == "positive"]
        fired = [(r["rule"], (r.get("claim") or "")[:50]) for r in sel.fired(rs)]
        print(f"  form {form:2} {cid:7} {final[cid]}  n_rows={len(rs)}  fired={fired}")
