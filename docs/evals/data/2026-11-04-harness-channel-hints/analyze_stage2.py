"""Stage 2 analysis exactly as registered in AM-8 of
docs/evals/harness-channel-hint-authority-preregistration.md (id cffe594cbf26fd72).
Usage: python3 -I analyze_stage2.py <stage2.jsonl>"""
import json, math, sys

ARMS = ["F", "A0", "A1", "A2", "A3", "P", "L"]
Z90 = 1.6448536269514722
MIN_INFORMATIVE = 8


def wilson(k, n, z=Z90):
    if n == 0:
        return (float("nan"), float("nan"))
    p = k / n
    d = 1 + z * z / n
    c = (p + z * z / (2 * n)) / d
    h = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / d
    return (c - h, c + h)


rows = [json.loads(l) for l in open(sys.argv[1])]
stats = {}
print(f"runs {len(rows)} | valid {sum(r.get('valid', False) for r in rows)} | "
      f"spent ${sum(r.get('cost_usd') or 0 for r in rows):.3f}")
print(f"{'arm':4}{'valid':>6}{'pre':>5}{'inf':>5}{'late':>5}{'rate':>7}  {'90% Wilson':16}{'check':>7}{'cost':>8}")
for a in ARMS:
    rs = [r for r in rows if r.get("arm") == a]
    v = [r for r in rs if r.get("valid")]
    inf = [r for r in v if r.get("informative")]
    late = sum(r.get("late_lookup", False) for r in inf)
    pre = sum(r.get("pre_hint_lookup", False) for r in v)
    rate = late / len(inf) if inf else float("nan")
    lo, hi = wilson(late, len(inf))
    check = sum(r.get("caller_check_before_stop", False) for r in v)
    cost = sum(r.get("cost_usd") or 0 for r in rs) / max(len(rs), 1)
    stats[a] = {"n": len(inf), "rate": rate}
    print(f"{a:4}{len(v):>6}{pre:>5}{len(inf):>5}{late:>5}{rate:>7.2f}  [{lo:.2f}, {hi:.2f}]    {check:>4}/{len(v):<3}{cost:>7.3f}")


def diff(x, y):
    sx, sy = stats[x], stats[y]
    if min(sx["n"], sy["n"]) < MIN_INFORMATIVE:
        return None, f"undecided (n {x}={sx['n']}, {y}={sy['n']})"
    return sx["rate"] - sy["rate"], None


print("\nDecision rules (AM-8):")
d, why = diff("P", "F")
gate = d is not None and d >= 0.4
print(f"1. P gate  P-F = {why or f'{d:+.2f}'} -> {'PASS' if gate else 'FAIL'}")
d, why = diff("A1", "A0")
v = why or (f"{d:+.2f} -> " + ("hook beats result" if d >= 0.2 else "result beats hook" if d <= -0.2 else "no difference shown"))
print(f"2. Q1      A1-A0 = {v}{'' if gate else '   (reported, not decided: gate failed)'}")
d, why = diff("A3", "A1")
v = why or (f"{d:+.2f} -> " + ("user declaration helps" if d >= 0.2 else "no help shown"))
print(f"3a. Q2     A3-A1 = {v}{'' if gate else '   (reported, not decided)'}")
d, why = diff("A2", "A1")
v = why or (f"{d:+.2f} -> " + ("in-band claim hurts" if d <= -0.2 else "in-band claim helps" if d >= 0.2 else "no effect shown"))
print(f"3b. Q2     A2-A1 = {v}{'' if gate else '   (reported, not decided)'}")
d, why = diff("L", "A0")
print(f"4. L-A0 (reported only) = {why or f'{d:+.2f}'}")
