"""Arithmetic for the phase-1b audit, from the frozen manifest only (no draw, no labels).

Per head B: expected audit cells = 300 x (share of train+val+cal rows whose rule is not B), and
the largest flagged count k with Wilson 95% upper bound <= 5% at that n (the admission limit).
Also the expected number of pairs whose BOTH texts are drawn among 300 of the 2,671 rows."""
import json, math

man = json.load(open("docs/evals/data/2026-09-24-rule-tell/stage2/frozen/freeze-manifest.json"))
rows = {}
for fold in ("train", "val", "cal"):
    for r, c in man["files"][fold]["per_rule"].items():
        rows[r] = rows.get(r, 0) + c["pos"] + c["neg"]
N = sum(rows.values())


def wilson_upper(k, n, z=1.96):
    p = k / n
    return (p + z * z / (2 * n) + z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n))) / (1 + z * z / n)


print(f"rows {N}")
for r in sorted(rows):
    n = round(300 * (N - rows[r]) / N)
    k = max(k for k in range(0, 40) if wilson_upper(k, n) <= 0.05)
    print(f"{r:22s} rows {rows[r]:4d}  expected audit cells {n:3d}  admitted if flagged <= {k} "
          f"(upper at {k}: {wilson_upper(k, n):.4f}; at {k + 1}: {wilson_upper(k + 1, n):.4f})")
pairs = man["files"]["train"]["positives"] + man["files"]["val"]["positives"] + man["files"]["cal"]["positives"]
exp_both = pairs * (300 / N) * (299 / (N - 1))
print(f"pairs (positive rows) {pairs}; expected pairs with both texts drawn: {exp_both:.1f}")
