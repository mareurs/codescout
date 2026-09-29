#!/usr/bin/env python3
"""The estimator for the System-1 labelled sample: rates, intervals, the registered decision, self-agreement.

Model-free and stdlib-only. Decision rule (registered before any label exists, threshold T = 0.10):

  go            Wilson 95% lower bound >= T
  no-go         Wilson 95% upper bound <  T
  inconclusive  otherwise

  Guards (each forces inconclusive and is named in the result):
    unresolved_share       more than 25% of the substantive labelled cases are `unresolved`
    interval_disagreement  the session-bootstrap interval gives a different outcome from Wilson

A HIT is a labelled case whose delivery is `quiet` or `interrupt`; `silent` (which includes every
`none` and `unresolved` label) is never a hit.
"""
import math
import random
import statistics

Z95 = 1.959964
THRESHOLD = 0.10
HIT_DELIVERIES = frozenset({"quiet", "interrupt"})
STRATA = ("substantive", "routine")  # order fixes the RNG draw order of the stratified bootstrap
# Breakdown vocabularies. Kept here, not imported from label.py, so a stale key cannot silently vanish.
LABEL_KEYS = ("verify", "qualify", "correct", "none", "unresolved")
DELIVERY_KEYS = ("silent", "quiet", "interrupt")


def wilson(k, n, z=Z95):
    """Wilson score interval for k hits in n trials.

    From (p_hat - p)^2 = z^2 p (1 - p) / n:
      centre = (p_hat + z^2 / 2n) / (1 + z^2 / n)
      half   = z * sqrt(p_hat (1 - p_hat) / n + z^2 / 4n^2) / (1 + z^2 / n)
    Clamped to [0, 1] (at k = 0 the lower bound is 0 exactly; floats give -1e-18).
    n == 0 gives the widest interval (0.0, 1.0): no data cannot support any outcome.
    """
    if n < 0 or k < 0 or k > n:
        raise ValueError(f"wilson: need 0 <= k <= n, got k={k}, n={n}")
    if n == 0:
        return (0.0, 1.0)
    p = k / n
    denom = 1 + z * z / n
    centre = (p + z * z / (2 * n)) / denom
    half = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / denom
    return (max(0.0, centre - half), min(1.0, centre + half))


def outcome(lo, hi, t=THRESHOLD):
    """`go` when lo >= t (lo == t IS go), `no-go` when hi < t (hi == t is NOT no-go), else `inconclusive`."""
    if lo >= t:
        return "go"
    if hi < t:
        return "no-go"
    return "inconclusive"


def _bounds(rates):
    """2.5 / 97.5 percentile of the resampled rates, by index into the sorted list.

    With b values and c = floor(2.5% of b) = (25 * b) // 1000: lo = sorted[c], hi = sorted[b - 1 - c].
    So exactly c values lie strictly below lo and c strictly above hi (b = 10000: sorted[250], sorted[9749]).
    Below b = 40 this degrades to (min, max).
    """
    s = sorted(rates)
    c = (25 * len(s)) // 1000
    return (s[c], s[len(s) - 1 - c])


def _pooled(pairs):
    """Pooled rate over (hits, units) pairs: total hits / total units (not a mean of per-session rates)."""
    return sum(h for h, _ in pairs) / sum(u for _, u in pairs)


def _resample_rates(pairs, rng, b):
    """b pooled rates, each from resampling the sessions (pairs) with replacement, same number drawn."""
    m = len(pairs)
    return [_pooled(rng.choices(pairs, k=m)) for _ in range(b)]


def _session_pairs(hits_by_session):
    pairs = [(sum(h), len(h)) for _, h in sorted(hits_by_session.items()) if h]
    if not pairs:
        raise ValueError("session bootstrap needs at least one session with at least one unit")
    return pairs


def session_bootstrap(hits_by_session, seed, b=10000):
    """Session-resampled 95% percentile interval of the pooled hit rate.

    hits_by_session maps a session id to that session's per-unit hit flags (1/0). Each of b resamples
    draws len(sessions) sessions with replacement (random.Random(seed)), pools hits / units over the
    draw, and (lo, hi) are the 2.5 and 97.5 percentiles by the index rule in `_bounds`.
    Sessions with no units are ignored. Deterministic per seed.
    """
    rng = random.Random(seed)
    return _bounds(_resample_rates(_session_pairs(hits_by_session), rng, b))


def decide(k, n, unresolved, boot, t=THRESHOLD):
    """The registered decision for k hits among n substantive labelled cases.

    n counts EVERY substantive labelled case, `unresolved` ones included; `unresolved` is how many of
    those n are labelled unresolved. Unresolved cases are silent, so they are in n and never in k.
    The unresolved guard trips when unresolved / n > 25%, tested as `unresolved * 4 > n` (exact integers:
    exactly 25% does not trip). `boot` is the (lo, hi) session-bootstrap interval.
    """
    if k < 0 or unresolved < 0 or n < 0 or k + unresolved > n:
        raise ValueError(f"decide: inconsistent counts k={k}, unresolved={unresolved}, n={n}")
    w = wilson(k, n)
    wilson_outcome = outcome(*w, t=t)
    boot_outcome = outcome(*boot, t=t)
    guards = []
    if unresolved * 4 > n:
        guards.append("unresolved_share")
    if wilson_outcome != boot_outcome:
        guards.append("interval_disagreement")
    return {"wilson": w, "boot": tuple(boot), "wilson_outcome": wilson_outcome, "boot_outcome": boot_outcome,
            "outcome": "inconclusive" if guards else wilson_outcome, "guards": guards}


def weighted_overall(rates, counts):
    """sum(rate_s * count_s) / sum(count_s) over the strata in `rates`; a missing count is size 0."""
    total = sum(counts.get(s, 0) for s in rates)
    if total <= 0:
        raise ValueError("weighted_overall: total stratum size is zero")
    return sum(r * counts.get(s, 0) for s, r in rates.items()) / total


def cohen_kappa(a, b):
    """Cohen's kappa = (po - pe) / (1 - pe) for two equal-length label lists.

    po is the observed agreement, pe = sum over categories of (count_a / N) * (count_b / N).
    When pe == 1 (both lists are one identical constant, so po == 1 too) kappa is undefined; we return
    1.0. pe == 1 is tested on integers (sum(ca * cb) == N * N) to avoid float edges.
    """
    if len(a) != len(b) or not a:
        raise ValueError("cohen_kappa: need two non-empty lists of equal length")
    n = len(a)
    po = sum(1 for x, y in zip(a, b) if x == y) / n
    cats = set(a) | set(b)
    pe_num = sum(a.count(c) * b.count(c) for c in cats)
    if pe_num == n * n:
        return 1.0
    pe = pe_num / (n * n)
    return (po - pe) / (1 - pe)


def _is_hit(r):
    return r["delivery"] in HIT_DELIVERIES


def _breakdowns(recs, key):
    by_label = {k: 0 for k in LABEL_KEYS}
    by_delivery = {k: 0 for k in DELIVERY_KEYS}
    by_kind = {}
    by_recall = {"y": {"n": 0, "k": 0}, "n": {"n": 0, "k": 0}}
    for r in recs:
        for l in r["labels"]:
            by_label[l] += 1
        by_delivery[r["delivery"]] += 1
        for slot in (by_kind.setdefault(key[r["case_id"]]["kind"], {"n": 0, "k": 0}), by_recall[r["recall"]]):
            slot["n"] += 1
            slot["k"] += _is_hit(r)
    return {"by_label": by_label, "by_delivery": by_delivery, "by_kind": by_kind, "by_recall": by_recall}


def _hits_by_session(recs, key):
    out = {}
    for r in recs:
        out.setdefault(key[r["case_id"]]["copy_id"], []).append(1 if _is_hit(r) else 0)
    return out


def _figures(records, key, frame_counts, seed, b):
    by_stratum = {s: [r for r in records if key[r["case_id"]]["stratum"] == s] for s in STRATA}
    out = {}
    sub = by_stratum["substantive"]
    if sub:
        k = sum(_is_hit(r) for r in sub)
        unresolved = sum(1 for r in sub if r["labels"] == ["unresolved"])
        boot = session_bootstrap(_hits_by_session(sub, key), seed, b)
        out["substantive"] = {"n": len(sub), "k": k, "unresolved": unresolved, "rate": k / len(sub),
                              "decision": decide(k, len(sub), unresolved, boot), **_breakdowns(sub, key)}
    else:
        out["substantive"] = None
    rou = by_stratum["routine"]
    if rou:
        k = sum(_is_hit(r) for r in rou)
        out["routine"] = {"n": len(rou), "k": k, "rate": k / len(rou), "wilson": wilson(k, len(rou)),
                          **_breakdowns(rou, key)}
    else:
        out["routine"] = None
    sizes = {s: sum(frame_counts.get(s, {}).values()) for s in STRATA}
    active = [s for s in STRATA if sizes[s] > 0]
    if active and all(by_stratum[s] for s in active):
        rates = {s: sum(_is_hit(r) for r in by_stratum[s]) / len(by_stratum[s]) for s in active}
        rng = random.Random(seed)
        drawn = {s: _resample_rates(_session_pairs(_hits_by_session(by_stratum[s], key)), rng, b) for s in active}
        total = sum(sizes[s] for s in active)
        combined = [sum(sizes[s] * drawn[s][i] for s in active) / total for i in range(b)]
        out["overall"] = {"rate": weighted_overall(rates, sizes), "boot": _bounds(combined),
                          "weights": {s: sizes[s] for s in active}}
    else:
        out["overall"] = None
    secs = [r["seconds"] for r in records]
    out["median_seconds"] = statistics.median(secs) if secs else None
    return out


def estimate(labels, key, frame_counts, seed, relabels=None, b=10000):
    """Everything the labelled sample can say, with every figure both ways.

    labels    label.py records (case_id, labels, delivery, recall, seconds, ...)
    key       case_id -> {"stratum", "kind", "copy_id"}; copy_id is the session the bootstrap resamples
    frame_counts  {stratum: {kind: n}} from sampler.frame_counts; absent strata / kinds count as 0
    Returns {"all_cases": figs, "without_recall_flagged": figs, "self_agreement": {n, agreement, kappa} | None}
    where figs = {substantive, routine, overall, median_seconds}. `substantive`/`routine` are None when
    that stratum has no labels; `overall` is None when a stratum with a non-zero frame weight has none.
    The decision is figs["substantive"]["decision"] under the all_cases branch.
    """
    seen = set()
    for r in labels:
        cid = r["case_id"]
        if cid not in key:
            raise ValueError(f"label for unknown case_id {cid!r}")
        if cid in seen:
            raise ValueError(f"duplicate label for case_id {cid!r}")
        seen.add(cid)
        if key[cid]["stratum"] not in STRATA:
            raise ValueError(f"case {cid!r} has unknown stratum {key[cid]['stratum']!r}")
    kept = [r for r in labels if r["recall"] != "y"]
    out = {"all_cases": _figures(labels, key, frame_counts, seed, b),
           "without_recall_flagged": _figures(kept, key, frame_counts, seed, b),
           "self_agreement": None}
    if relabels:
        first = {r["case_id"]: r["delivery"] for r in labels}
        pairs, again = [], set()
        for r in relabels:
            cid = r["case_id"]
            if cid not in first:
                raise ValueError(f"relabel for case_id {cid!r} that was never labelled")
            if cid in again:
                raise ValueError(f"duplicate relabel for case_id {cid!r}")
            again.add(cid)
            pairs.append((first[cid], r["delivery"]))
        a, c = [p[0] for p in pairs], [p[1] for p in pairs]
        out["self_agreement"] = {"n": len(pairs), "agreement": sum(x == y for x, y in pairs) / len(pairs),
                                 "kappa": cohen_kappa(a, c)}
    return out
