"""Tests for scripts/measure/estimate.py -- the labelled-sample estimator and its registered decision rule.

Synthetic data only. Expected values are hand-computed literals, never recomputed with the code's formula.
The Wilson literals were derived independently from the score-interval quadratic
(1 + z^2/n) p^2 - (2 p_hat + z^2/n) p + p_hat^2 = 0 at z = 1.959964, solved with the quadratic formula.
"""
import importlib.util
import pathlib
import sys
import unittest

REPO_ROOT = pathlib.Path(__file__).resolve().parents[1]
MEASURE = REPO_ROOT / "scripts" / "measure"
sys.path.insert(0, str(MEASURE))


def _load(name):
    spec = importlib.util.spec_from_file_location(f"measure_{name}", MEASURE / f"{name}.py")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


est = _load("estimate")


def rec(cid, labels, delivery, recall="n", seconds=10.0):
    """A record with the Task 3 label schema (label.py label_one)."""
    return {"case_id": cid, "packet_sha256": "f" * 64, "labels": list(labels), "delivery": delivery,
            "note": "", "recall": recall, "seconds": seconds, "labelled_at": "2026-09-29T10:00:00Z"}


class WilsonTests(unittest.TestCase):
    def test_wilson_matches_hand_computed_values(self):
        # The plan's four values, each re-derived independently before being pinned here:
        # (0,60) hi 0.060172; (1,60) hi 0.088551; (11,60) lo 0.105578; (12,60) lo 0.118285.
        self.assertAlmostEqual(est.wilson(0, 60)[1], 0.0602, delta=1e-4)
        self.assertAlmostEqual(est.wilson(1, 60)[1], 0.0886, delta=1e-4)
        self.assertAlmostEqual(est.wilson(11, 60)[0], 0.1056, delta=1e-4)
        self.assertAlmostEqual(est.wilson(12, 60)[0], 0.1183, delta=1e-4)

    def test_wilson_more_independent_literals_on_both_ends(self):
        lo, hi = est.wilson(6, 60)
        self.assertAlmostEqual(lo, 0.046643, delta=1e-5)
        self.assertAlmostEqual(hi, 0.201495, delta=1e-5)
        lo, hi = est.wilson(30, 60)  # symmetric about 0.5
        self.assertAlmostEqual(lo, 0.377350, delta=1e-5)
        self.assertAlmostEqual(hi, 0.622650, delta=1e-5)
        lo, hi = est.wilson(2, 60)
        self.assertAlmostEqual(lo, 0.009189, delta=1e-5)
        self.assertAlmostEqual(hi, 0.113638, delta=1e-5)
        lo, hi = est.wilson(10, 60)
        self.assertAlmostEqual(lo, 0.093132, delta=1e-5)
        self.assertAlmostEqual(hi, 0.280316, delta=1e-5)

    def test_wilson_edges_stay_inside_unit_interval(self):
        self.assertEqual(est.wilson(0, 60)[0], 0.0)
        self.assertEqual(est.wilson(60, 60)[1], 1.0)
        # n = 7 and n = 20 are inputs where the unclamped float is -2.8e-17 and 1.0000000000000002
        self.assertEqual(est.wilson(0, 7)[0], 0.0)
        self.assertEqual(est.wilson(20, 20)[1], 1.0)
        self.assertEqual(est.wilson(0, 0), (0.0, 1.0))  # no data: the widest interval, never a division error

    def test_wilson_rejects_impossible_counts(self):
        for k, n in ((-1, 10), (11, 10), (1, -1)):
            with self.assertRaises(ValueError):
                est.wilson(k, n)


class OutcomeTests(unittest.TestCase):
    def test_outcome_exact_boundaries_registered(self):
        self.assertEqual(est.outcome(0.10, 0.20), "go")               # lo == T is go (>=, not >)
        self.assertEqual(est.outcome(0.0999, 0.20), "inconclusive")   # just under
        self.assertEqual(est.outcome(0.0, 0.10), "inconclusive")      # hi == T is NOT no-go (< , not <=)
        self.assertEqual(est.outcome(0.0, 0.0999), "no-go")           # just under
        self.assertEqual(est.outcome(0.05, 0.15), "inconclusive")

    def test_outcome_threshold_is_a_parameter(self):
        self.assertEqual(est.outcome(0.2, 0.3, t=0.2), "go")
        self.assertEqual(est.outcome(0.0, 0.2, t=0.2), "inconclusive")
        self.assertEqual(est.outcome(0.0, 0.19, t=0.2), "no-go")

    def test_outcome_at_the_registered_boundaries(self):
        # n = 60, bootstrap intervals chosen (literals) to agree with the Wilson outcome so no guard trips.
        agree = {1: (0.0, 0.05), 2: (0.03, 0.20), 10: (0.08, 0.28), 11: (0.12, 0.30)}
        want = {1: "no-go", 2: "inconclusive", 10: "inconclusive", 11: "go"}
        for k in (1, 2, 10, 11):
            d = est.decide(k, 60, 0, agree[k])
            self.assertEqual(d["wilson_outcome"], want[k], k)
            self.assertEqual(d["boot_outcome"], want[k], k)
            self.assertEqual(d["guards"], [], k)
            self.assertEqual(d["outcome"], want[k], k)
        d = est.decide(0, 60, 0, (0.0, 0.03))
        self.assertEqual(d["outcome"], "no-go")

    def test_decide_reports_both_intervals(self):
        d = est.decide(11, 60, 0, (0.12, 0.30))
        self.assertAlmostEqual(d["wilson"][0], 0.105578, delta=1e-5)
        self.assertEqual(tuple(d["boot"]), (0.12, 0.30))


class GuardTests(unittest.TestCase):
    def test_unresolved_guard_trips_above_a_quarter(self):
        # Intervals AGREE (both go) so only the unresolved guard can be what refuses.
        boot = (0.12, 0.30)
        d = est.decide(11, 60, 16, boot)
        self.assertEqual(d["guards"], ["unresolved_share"])
        self.assertEqual(d["wilson_outcome"], "go")
        self.assertEqual(d["boot_outcome"], "go")
        self.assertEqual(d["outcome"], "inconclusive")
        d = est.decide(11, 60, 15, boot)  # exactly 25%: does not trip
        self.assertEqual(d["guards"], [])
        self.assertEqual(d["outcome"], "go")

    def test_unresolved_guard_boundary_with_a_share_that_is_not_a_float_edge(self):
        boot = (0.5, 0.9)
        d = est.decide(6, 8, 2, boot)  # 2/8 = 25% exactly
        self.assertEqual(d["guards"], [])
        d = est.decide(5, 8, 3, boot)  # 3/8 = 37.5%
        self.assertEqual(d["guards"], ["unresolved_share"])
        d = est.decide(0, 3, 1, (0.0, 0.5))  # 1/3 > 25%; wilson(0,3) upper ~0.56 -> both inconclusive, agree
        self.assertEqual(d["guards"], ["unresolved_share"])
        self.assertEqual(d["outcome"], "inconclusive")

    def test_interval_disagreement_makes_it_inconclusive(self):
        # unresolved = 0 so the other guard cannot be what refuses.
        d = est.decide(11, 60, 0, (0.05, 0.20))  # wilson go, boot inconclusive
        self.assertEqual((d["wilson_outcome"], d["boot_outcome"]), ("go", "inconclusive"))
        self.assertEqual(d["guards"], ["interval_disagreement"])
        self.assertEqual(d["outcome"], "inconclusive")
        d = est.decide(1, 60, 0, (0.02, 0.11))  # wilson no-go, boot inconclusive
        self.assertEqual((d["wilson_outcome"], d["boot_outcome"]), ("no-go", "inconclusive"))
        self.assertEqual(d["guards"], ["interval_disagreement"])
        self.assertEqual(d["outcome"], "inconclusive")
        d = est.decide(1, 60, 0, (0.15, 0.30))  # wilson no-go, boot go: opposite ends
        self.assertEqual(d["guards"], ["interval_disagreement"])
        self.assertEqual(d["outcome"], "inconclusive")
        d = est.decide(5, 60, 0, (0.15, 0.30))  # wilson inconclusive, boot go
        self.assertEqual(d["guards"], ["interval_disagreement"])
        self.assertEqual(d["outcome"], "inconclusive")

    def test_both_guards_can_be_listed_together(self):
        d = est.decide(11, 60, 16, (0.05, 0.20))
        self.assertEqual(sorted(d["guards"]), ["interval_disagreement", "unresolved_share"])
        self.assertEqual(d["outcome"], "inconclusive")

    def test_decide_rejects_inconsistent_counts(self):
        for k, n, u in ((-1, 10, 0), (5, 10, -1), (6, 10, 5), (11, 10, 0)):
            with self.assertRaises(ValueError):
                est.decide(k, n, u, (0.0, 1.0))

    def test_decide_threshold_is_passed_through(self):
        d = est.decide(30, 60, 0, (0.4, 0.6), t=0.3)
        self.assertEqual(d["outcome"], "go")


class BootstrapTests(unittest.TestCase):
    def test_percentile_indices_pinned(self):
        # b = 40: 2.5% of 40 is exactly one value cut from each end -> sorted[1], sorted[38].
        rates = [float(i) for i in range(40)][::-1]  # unsorted on purpose
        self.assertEqual(est._bounds(rates), (1.0, 38.0))
        # b = 10000 -> sorted[250], sorted[9749]: 250 values strictly below lo and 250 strictly above hi.
        rates = [float(i) for i in range(10000)]
        self.assertEqual(est._bounds(rates), (250.0, 9749.0))

    def test_pooled_rate_is_hits_over_units_not_mean_of_rates(self):
        self.assertAlmostEqual(est._pooled([(1, 1), (0, 98)]), 1 / 99, places=12)
        self.assertEqual(est._pooled([(3, 4), (1, 4)]), 0.5)

    def test_equal_rate_sessions_give_a_degenerate_interval(self):
        hits = {f"s{i}": [1, 0, 0, 0] for i in range(6)}  # every session is exactly 25%
        lo, hi = est.session_bootstrap(hits, seed=3, b=500)
        self.assertEqual((lo, hi), (0.25, 0.25))

    def test_session_bootstrap_is_reproducible_and_widens_under_clustering(self):
        clustered = {"s0": [1] * 10}
        clustered.update({f"s{i}": [0] * 10 for i in range(1, 10)})  # 10 hits, all in one session of ten
        spread = {f"s{i}": [1] + [0] * 9 for i in range(10)}          # the same 10 hits, one per session
        a = est.session_bootstrap(clustered, seed=11, b=2000)
        b = est.session_bootstrap(clustered, seed=11, b=2000)
        self.assertEqual(a, b)  # reproducible per seed
        lo_s, hi_s = est.session_bootstrap(spread, seed=11, b=2000)
        self.assertEqual((lo_s, hi_s), (0.1, 0.1))  # resampling SESSIONS of equal rate cannot move the rate
        lo_c, hi_c = a
        self.assertGreater(hi_c - lo_c, 0.25)  # session draws: 0..3+ of the loaded session -> 0.0 .. >= 0.3
        self.assertEqual(lo_c, 0.0)
        self.assertGreaterEqual(hi_c, 0.3)

    def test_seed_is_used(self):
        hits = {"a": [1, 1, 0], "b": [0, 0, 0], "c": [1, 0, 0], "d": [0, 0, 0], "e": [1, 1, 1], "f": [0, 1, 0]}
        results = {est.session_bootstrap(hits, seed=s, b=60) for s in range(8)}
        self.assertGreater(len(results), 1)  # a constant seed would make every run identical

    def test_session_bootstrap_ignores_empty_sessions_and_rejects_no_data(self):
        self.assertEqual(est.session_bootstrap({"a": [1, 1], "empty": []}, seed=1, b=50), (1.0, 1.0))
        with self.assertRaises(ValueError):
            est.session_bootstrap({}, seed=1, b=50)
        with self.assertRaises(ValueError):
            est.session_bootstrap({"a": []}, seed=1, b=50)


class WeightedAndKappaTests(unittest.TestCase):
    def test_weighted_overall_uses_frame_counts(self):
        # (0.2*100 + 0.0*300) / 400 = 0.05
        self.assertAlmostEqual(est.weighted_overall({"substantive": 0.2, "routine": 0.0},
                                                    {"substantive": 100, "routine": 300}), 0.05, places=12)
        # (0.1*1 + 0.3*3) / 4 = 0.25 -- neither the unweighted mean (0.2) nor swapped weights (0.15)
        self.assertAlmostEqual(est.weighted_overall({"substantive": 0.1, "routine": 0.3},
                                                    {"substantive": 1, "routine": 3}), 0.25, places=12)

    def test_weighted_overall_ignores_a_stratum_of_size_zero_and_rejects_empty(self):
        self.assertAlmostEqual(est.weighted_overall({"substantive": 0.4, "routine": 0.9},
                                                    {"substantive": 10, "routine": 0}), 0.4, places=12)
        with self.assertRaises(ValueError):
            est.weighted_overall({"substantive": 0.4}, {"substantive": 0})

    def test_kappa_hand_computed(self):
        a = ["silent"] * 4 + ["quiet"] * 6
        b = ["silent", "silent"] + ["quiet"] * 7 + ["silent"]
        # agree at positions 1,2 and 5..9 -> po = 7/10. marginals a: s4 q6; b: s3 q7.
        # pe = .4*.3 + .6*.7 = .54 ; kappa = (.7-.54)/(1-.54) = 8/23
        self.assertAlmostEqual(est.cohen_kappa(a, b), 8 / 23, places=9)
        self.assertAlmostEqual(est.cohen_kappa(a, b), 0.347826087, places=9)

    def test_kappa_edges(self):
        same = ["silent"] * 5
        self.assertEqual(est.cohen_kappa(same, same), 1.0)  # pe == 1: defined as 1.0 (po is necessarily 1 too)
        mixed = ["silent", "quiet", "interrupt", "quiet"]
        self.assertEqual(est.cohen_kappa(mixed, mixed), 1.0)
        # complete disagreement on two balanced categories: po = 0, pe = .5 -> kappa = -1
        self.assertAlmostEqual(est.cohen_kappa(["silent", "quiet"], ["quiet", "silent"]), -1.0, places=12)
        with self.assertRaises(ValueError):
            est.cohen_kappa(["silent"], ["silent", "quiet"])
        with self.assertRaises(ValueError):
            est.cohen_kappa([], [])


def _fixture():
    """12 substantive + 6 routine labelled cases over sessions copyA/B/C. Hand-computed expectations in the tests."""
    sub = [
        # cid, labels, delivery, recall, seconds, kind, copy
        ("c01", ["verify"], "quiet", "n", 10, "top", "copyA"),
        ("c02", ["correct"], "interrupt", "n", 20, "top", "copyA"),
        ("c03", ["none"], "silent", "n", 30, "top", "copyA"),
        ("c04", ["unresolved"], "silent", "n", 40, "top", "copyA"),
        ("c05", ["qualify", "verify"], "quiet", "y", 50, "top", "copyB"),
        ("c06", ["none"], "silent", "n", 60, "top", "copyB"),
        ("c07", ["none"], "silent", "n", 70, "handback", "copyB"),
        ("c08", ["none"], "silent", "y", 80, "handback", "copyB"),
        ("c09", ["correct"], "interrupt", "n", 90, "handback", "copyC"),
        ("c10", ["none"], "silent", "n", 100, "handback", "copyC"),
        ("c11", ["none"], "silent", "n", 110, "handback", "copyC"),
        ("c12", ["verify"], "quiet", "y", 120, "handback", "copyC"),
    ]
    rou = [
        ("r01", ["verify"], "quiet", "n", 5, "top", "copyA"),
        ("r02", ["none"], "silent", "n", 15, "top", "copyA"),
        ("r03", ["none"], "silent", "n", 25, "top", "copyB"),
        ("r04", ["none"], "silent", "n", 35, "top", "copyB"),
        ("r05", ["none"], "silent", "n", 45, "top", "copyC"),
        ("r06", ["correct"], "interrupt", "y", 55, "top", "copyC"),
    ]
    labels, key = [], {}
    for stratum, rows in (("substantive", sub), ("routine", rou)):
        for cid, lab, dl, rc, sec, kind, copy in rows:
            labels.append(rec(cid, lab, dl, rc, sec))
            key[cid] = {"stratum": stratum, "kind": kind, "copy_id": copy}
    frame = {"substantive": {"top": 300, "handback": 100}, "routine": {"top": 600}}
    return labels, key, frame


class EstimateTests(unittest.TestCase):
    def setUp(self):
        self.labels, self.key, self.frame = _fixture()
        self.out = est.estimate(self.labels, self.key, self.frame, seed=5, b=300)

    def test_substantive_decision_counts_all_cases(self):
        s = self.out["all_cases"]["substantive"]
        self.assertEqual((s["n"], s["k"], s["unresolved"]), (12, 5, 1))  # hits c01 c02 c05 c09 c12
        self.assertAlmostEqual(s["rate"], 5 / 12, places=12)
        d = s["decision"]
        self.assertAlmostEqual(d["wilson"][0], 0.193260, delta=1e-5)  # independent quadratic-formula literal
        self.assertAlmostEqual(d["wilson"][1], 0.680489, delta=1e-5)
        # wilson lower 0.193 >= T=0.10 is `go`; the seed-5 b=300 session bootstrap over the 3 sessions is (0.25, 0.5),
        # also `go`; 1 unresolved of 12 is under a quarter. Observed values, so the outcome is the specific `go`.
        self.assertEqual((d["wilson_outcome"], d["boot_outcome"]), ("go", "go"))
        self.assertEqual((d["guards"], d["outcome"]), ([], "go"))

    def test_substantive_without_recall_flagged_cases(self):
        s = self.out["without_recall_flagged"]["substantive"]
        self.assertEqual((s["n"], s["k"], s["unresolved"]), (9, 3, 1))  # drops c05 c08 c12
        self.assertAlmostEqual(s["rate"], 1 / 3, places=12)

    def test_routine_rate_and_interval(self):
        r = self.out["all_cases"]["routine"]
        self.assertEqual((r["n"], r["k"]), (6, 2))  # r01, r06
        self.assertAlmostEqual(r["rate"], 1 / 3, places=12)
        self.assertAlmostEqual(r["wilson"][0], 0.096771, delta=1e-5)  # independent literal for (2, 6)
        self.assertAlmostEqual(r["wilson"][1], 0.700007, delta=1e-5)
        r = self.out["without_recall_flagged"]["routine"]
        self.assertEqual((r["n"], r["k"]), (5, 1))  # drops r06
        self.assertAlmostEqual(r["rate"], 0.2, places=12)

    def test_overall_weighted_rate_uses_frame_counts(self):
        # (5/12 * 400 + 2/6 * 600) / 1000 = 11/30 ; without recall (1/3*400 + 1/5*600)/1000 = 19/75
        self.assertAlmostEqual(self.out["all_cases"]["overall"]["rate"], 11 / 30, places=12)
        self.assertAlmostEqual(self.out["without_recall_flagged"]["overall"]["rate"], 19 / 75, places=12)

    def test_overall_interval_is_a_reproducible_bracket(self):
        lo, hi = self.out["all_cases"]["overall"]["boot"]
        self.assertTrue(0.0 <= lo <= hi <= 1.0)
        again = est.estimate(self.labels, self.key, self.frame, seed=5, b=300)
        self.assertEqual(again["all_cases"]["overall"]["boot"], (lo, hi))
        self.assertEqual(again["all_cases"]["substantive"]["decision"]["boot"],
                         self.out["all_cases"]["substantive"]["decision"]["boot"])
    def test_overall_bootstrap_is_weighted_by_frame_counts(self):
        # Every substantive case a hit, every routine case silent, one session each: each resample gives
        # rates 1.0 and 0.0, so the weighted overall is exactly 100/400 = 0.25 in every resample.
        labels = [rec("s1", ["verify"], "quiet"), rec("s2", ["verify"], "quiet"),
                  rec("r1", ["none"], "silent"), rec("r2", ["none"], "silent")]
        key = {"s1": {"stratum": "substantive", "kind": "top", "copy_id": "A"},
               "s2": {"stratum": "substantive", "kind": "top", "copy_id": "B"},
               "r1": {"stratum": "routine", "kind": "top", "copy_id": "A"},
               "r2": {"stratum": "routine", "kind": "top", "copy_id": "B"}}
        out = est.estimate(labels, key, {"substantive": {"top": 100}, "routine": {"top": 300}}, seed=4, b=100)
        self.assertEqual(out["all_cases"]["overall"]["rate"], 0.25)
        self.assertEqual(out["all_cases"]["overall"]["boot"], (0.25, 0.25))

    def test_overall_bootstrap_uses_the_seed(self):
        results = {est.estimate(self.labels, self.key, self.frame, seed=s, b=60)["all_cases"]["overall"]["boot"]
                   for s in range(1, 9)}
        self.assertGreater(len(results), 1)  # a constant seed would make every run identical

    def test_substantive_boot_is_session_resampled_from_the_key(self):
        # Three sessions, all substantive hits are in copyA: rebuild with c01..c04 hit and everything else silent.
        labels = []
        key = {}
        for i in range(12):
            cid = f"c{i:02d}"
            labels.append(rec(cid, ["verify"], "quiet") if i < 4 else rec(cid, ["none"], "silent"))
            key[cid] = {"stratum": "substantive", "kind": "top", "copy_id": ["A", "B", "C"][i // 4]}
        out = est.estimate(labels, key, {"substantive": {"top": 100}}, seed=2, b=400)
        lo, hi = out["all_cases"]["substantive"]["decision"]["boot"]
        self.assertEqual(lo, 0.0)   # sessions drawn without A -> rate 0
        self.assertEqual(hi, 1.0)   # A drawn three times -> rate 1 (a unit-level resample could never reach it)

    def test_breakdowns_by_label_delivery_kind_recall(self):
        s = self.out["all_cases"]["substantive"]
        self.assertEqual(s["by_delivery"], {"silent": 7, "quiet": 3, "interrupt": 2})
        self.assertEqual(s["by_label"], {"verify": 3, "qualify": 1, "correct": 2, "none": 6, "unresolved": 1})
        self.assertEqual(s["by_kind"], {"top": {"n": 6, "k": 3}, "handback": {"n": 6, "k": 2}})
        self.assertEqual(s["by_recall"], {"y": {"n": 3, "k": 2}, "n": {"n": 9, "k": 3}})
        w = self.out["without_recall_flagged"]["substantive"]
        # drops c05 (quiet, qualify+verify), c08 (silent, none), c12 (quiet, verify)
        self.assertEqual(w["by_delivery"], {"silent": 6, "quiet": 1, "interrupt": 2})
        self.assertEqual(w["by_label"], {"verify": 1, "qualify": 0, "correct": 2, "none": 5, "unresolved": 1})
        self.assertEqual(w["by_kind"], {"top": {"n": 5, "k": 2}, "handback": {"n": 4, "k": 1}})
        self.assertEqual(w["by_recall"], {"y": {"n": 0, "k": 0}, "n": {"n": 9, "k": 3}})

    def test_median_seconds_with_and_without_recall(self):
        self.assertEqual(self.out["all_cases"]["median_seconds"], 47.5)            # 18 values: (45+50)/2
        self.assertEqual(self.out["without_recall_flagged"]["median_seconds"], 37.5)  # 14 values: (35+40)/2

    def test_hit_means_quiet_or_interrupt_only(self):
        labels = [rec("a", ["none"], "silent"), rec("b", ["unresolved"], "silent"),
                  rec("c", ["verify"], "quiet"), rec("d", ["correct"], "interrupt")]
        key = {c: {"stratum": "substantive", "kind": "top", "copy_id": "x" + c} for c in "abcd"}
        out = est.estimate(labels, key, {"substantive": {"top": 4}}, seed=1, b=50)
        self.assertEqual(out["all_cases"]["substantive"]["k"], 2)

    def test_self_agreement_kappa_on_delivery(self):
        relabels = [rec("c01", ["none"], "silent"), rec("c02", ["correct"], "interrupt"),
                    rec("c03", ["none"], "silent"), rec("c04", ["unresolved"], "silent"),
                    rec("c09", ["verify"], "quiet"), rec("c10", ["verify"], "quiet")]
        out = est.estimate(self.labels, self.key, self.frame, seed=5, relabels=relabels, b=100)
        sa = out["self_agreement"]
        self.assertEqual(sa["n"], 6)
        self.assertAlmostEqual(sa["agreement"], 0.5, places=12)
        self.assertAlmostEqual(sa["kappa"], 5 / 23, places=9)  # po .5, pe 13/36 -> (5/36)/(23/36)
        self.assertIsNone(self.out["self_agreement"])

    def test_missing_stratum_or_kind_in_frame_counts_counts_as_zero(self):
        out = est.estimate(self.labels, self.key, {"substantive": {"top": 300}}, seed=5, b=100)
        # routine absent from the frame -> weight 0; substantive weighs by its top-only size
        self.assertAlmostEqual(out["all_cases"]["overall"]["rate"], 5 / 12, places=12)

    def test_unknown_or_duplicate_case_ids_are_refused(self):
        with self.assertRaises(ValueError):
            est.estimate(self.labels + [rec("zzz", ["none"], "silent")], self.key, self.frame, seed=1, b=50)
        with self.assertRaises(ValueError):
            est.estimate(self.labels + [rec("c01", ["none"], "silent")], self.key, self.frame, seed=1, b=50)
        with self.assertRaises(ValueError):
            est.estimate(self.labels, self.key, self.frame, seed=1, b=50,
                         relabels=[rec("nope", ["none"], "silent")])

    def test_stratum_with_frame_weight_but_no_labels_gives_no_overall(self):
        only_sub = [l for l in self.labels if l["case_id"].startswith("c")]
        out = est.estimate(only_sub, self.key, self.frame, seed=1, b=50)
        self.assertIsNone(out["all_cases"]["overall"])
        self.assertIsNone(out["all_cases"]["routine"])


def _substantive(k, n, unresolved=0, recall_from=None):
    """n substantive labelled cases in n distinct sessions: k hits (quiet) first, `unresolved` unresolved LAST,
    the rest none.
    Cases with index >= recall_from (if given) carry recall 'y'. Distinct sessions mean the session bootstrap
    is a plain bootstrap over cases, the shape a real 60-case sample has when no session repeats."""
    labels, key = [], {}
    for i in range(n):
        cid = f"c{i:03d}"
        if i < k:
            l = rec(cid, ["verify"], "quiet")
        elif i >= n - unresolved:
            l = rec(cid, ["unresolved"], "silent")
        else:
            l = rec(cid, ["none"], "silent")
        if recall_from is not None and i >= recall_from:
            l["recall"] = "y"
        labels.append(l)
        key[cid] = {"stratum": "substantive", "kind": "top", "copy_id": f"s{i:03d}"}
    return labels, key


class UnresolvedGuardThroughEstimateTests(unittest.TestCase):
    """The guard is wired into the production decision: exercised through estimate(), not decide() alone.
    20 hits of 60 => wilson lower ~0.23 and a session bootstrap lower ~0.22: both `go`, so the unresolved
    share is the ONLY thing that can refuse."""

    def _decision(self, unresolved):
        labels, key = _substantive(20, 60, unresolved)
        out = est.estimate(labels, key, {}, seed=3, b=1000)
        return out["all_cases"]["substantive"]["decision"], out["all_cases"]["substantive"]

    def test_exactly_a_quarter_unresolved_does_not_trip(self):
        d, s = self._decision(15)  # 15 / 60 = 25% exactly
        self.assertEqual(s["unresolved"], 15)
        self.assertEqual((d["wilson_outcome"], d["boot_outcome"]), ("go", "go"))
        self.assertEqual(d["guards"], [])
        self.assertEqual(d["outcome"], "go")

    def test_one_over_a_quarter_trips_only_the_unresolved_guard(self):
        d, s = self._decision(16)  # 16 / 60 > 25%
        self.assertEqual(s["unresolved"], 16)
        self.assertEqual((d["wilson_outcome"], d["boot_outcome"]), ("go", "go"))  # intervals agree: no other guard
        self.assertEqual(d["guards"], ["unresolved_share"])
        self.assertEqual(d["outcome"], "inconclusive")

    def test_guard_is_evaluated_per_recall_branch(self):
        # 16 unresolved sit in the recall-flagged tail: the all-cases branch trips, the without-recall one does not.
        labels, key = _substantive(20, 60, 16, recall_from=44)  # indices 44..59 are the unresolved tail
        for l in labels[:44]:
            self.assertEqual(l["recall"], "n")
        out = est.estimate(labels, key, {}, seed=3, b=1000)
        self.assertEqual(out["all_cases"]["substantive"]["decision"]["guards"], ["unresolved_share"])
        w = out["without_recall_flagged"]["substantive"]
        self.assertEqual((w["n"], w["unresolved"]), (44, 0))
        self.assertEqual(w["decision"]["guards"], [])
        self.assertEqual(w["decision"]["outcome"], "go")


class RealisticBootstrapEndToEndTests(unittest.TestCase):
    """Decision boundaries through estimate() with a REAL session bootstrap (60 distinct sessions, b = 10000),
    not the hand-picked agreeing tuples that test_outcome_at_the_registered_boundaries uses.
    Seeds checked, k = 11 (the registered go boundary), 100 seeds 0..99, guard-free otherwise:
      go in 35 of 100 (first: 1, 2, 9, 10, 11, 15, 16, 21); inconclusive + interval_disagreement in 65.
    k = 1 no-go, k = 10 inconclusive, k = 12 go in 100 / 100. Reproduce: session_bootstrap on 60 one-unit
    sessions, hits {s00..s10} = 1."""

    def _run(self, k, seed):
        labels, key = _substantive(k, 60)
        out = est.estimate(labels, key, {}, seed=seed, b=10000)
        return out["all_cases"]["substantive"]["decision"]

    def test_k11_outcome_is_seed_dependent_at_the_registered_boundary(self):
        # OPEN DESIGN ISSUE, not a property to rely on: at k = 11 of 60 the bootstrap's 2.5% quantile lands on
        # the discrete atoms 5/60 and 6/60 (P(X <= 5 | Binomial(60, 11/60)) = 0.0256), so the bootstrap lower
        # bound straddles 10% and the outcome depends on the seed even with no session clustering: `go` in
        # 35 of 100 seeds, `interval_disagreement` in the other 65. k >= 12 is `go` at every seed tried.
        # The operator was asked to rule on this before registration. These two pins fix the two behaviours.
        d = self._run(11, seed=1)
        self.assertEqual((d["wilson_outcome"], d["boot_outcome"]), ("go", "go"))
        self.assertEqual((d["guards"], d["outcome"]), ([], "go"))
        d = self._run(11, seed=0)
        self.assertEqual((d["wilson_outcome"], d["boot_outcome"]), ("go", "inconclusive"))
        self.assertEqual((d["guards"], d["outcome"]), (["interval_disagreement"], "inconclusive"))

    def test_k12_is_go(self):
        d = self._run(12, seed=0)
        self.assertEqual((d["guards"], d["outcome"]), ([], "go"))

    def test_k10_is_inconclusive(self):
        d = self._run(10, seed=0)
        self.assertEqual((d["guards"], d["outcome"]), ([], "inconclusive"))

    def test_k1_is_no_go(self):
        d = self._run(1, seed=0)
        self.assertEqual((d["guards"], d["outcome"]), ([], "no-go"))


class EstimateWiringTests(unittest.TestCase):
    """estimate() hands the RIGHT sessions, seed and b to the bootstrap, on both recall branches."""

    def setUp(self):
        # 8 sessions; session i has i + 1 units, unit j is a hit when (i + j) % 3 == 0; units j == 0 of even
        # sessions are recall-flagged. Uneven session sizes keep the pooled-rate atoms fine-grained.
        self.labels, self.key = [], {}
        self.all_hits, self.kept_hits = {}, {}
        for i in range(8):
            for j in range(i + 1):
                cid = f"w{i}_{j}"
                hit = (i + j) % 3 == 0
                flagged = j == 0 and i % 2 == 0
                l = rec(cid, ["verify"], "quiet") if hit else rec(cid, ["none"], "silent")
                l["recall"] = "y" if flagged else "n"
                self.labels.append(l)
                self.key[cid] = {"stratum": "substantive", "kind": "top" if j % 2 else "handback",
                                 "copy_id": f"sess{i}"}
                self.all_hits.setdefault(f"sess{i}", []).append(int(hit))
                if not flagged:
                    self.kept_hits.setdefault(f"sess{i}", []).append(int(hit))
        self.frame = {"substantive": {"top": 200}}
        # Routine cases in their own sessions, one of them unresolved: they must not leak into the substantive
        # decision (its sessions, k, n or unresolved count). Sizes 1..4, hits where (i + j) % 2 == 0.
        self.rou_hits = {}
        for i in range(4):
            for j in range(i + 1):
                cid = f"x{i}_{j}"
                hit = (i + j) % 2 == 0
                if i == 3 and j == 3:
                    l = rec(cid, ["unresolved"], "silent")
                    hit = False
                else:
                    l = rec(cid, ["verify"], "quiet") if hit else rec(cid, ["none"], "silent")
                self.labels.append(l)
                self.key[cid] = {"stratum": "routine", "kind": "top", "copy_id": f"rsess{i}"}
                self.rou_hits.setdefault(f"rsess{i}", []).append(int(hit))

    def test_decision_bootstrap_uses_copy_id_sessions_seed_and_b_on_both_branches(self):
        out = est.estimate(self.labels, self.key, self.frame, seed=9, b=40)
        self.assertEqual(out["all_cases"]["substantive"]["decision"]["boot"],
                         est.session_bootstrap(self.all_hits, 9, 40))
        self.assertEqual(out["without_recall_flagged"]["substantive"]["decision"]["boot"],
                         est.session_bootstrap(self.kept_hits, 9, 40))

    def test_the_two_expected_bootstraps_are_not_accidentally_equal(self):
        # Positive control for the test above: a mutant that swapped the branches or dropped the recall
        # filter would be invisible if these coincided.
        self.assertNotEqual(est.session_bootstrap(self.all_hits, 9, 40), est.session_bootstrap(self.kept_hits, 9, 40))
        self.assertNotEqual(est.session_bootstrap(self.all_hits, 9, 40), est.session_bootstrap(self.all_hits, 0, 40))
        self.assertNotEqual(est.session_bootstrap(self.all_hits, 9, 40),
                            est.session_bootstrap(self.all_hits, 9, 10000))

    def test_overall_bootstrap_uses_the_same_sessions_seed_and_b(self):
        # One stratum in the frame => the weighted bootstrap reduces to that stratum's session bootstrap.
        out = est.estimate(self.labels, self.key, self.frame, seed=9, b=40)
        lo, hi = est.session_bootstrap(self.all_hits, 9, 40)
        got = out["all_cases"]["overall"]["boot"]
        self.assertAlmostEqual(got[0], lo, places=12)
        self.assertAlmostEqual(got[1], hi, places=12)
        lo, hi = est.session_bootstrap(self.kept_hits, 9, 40)
        got = out["without_recall_flagged"]["overall"]["boot"]
        self.assertAlmostEqual(got[0], lo, places=12)
        self.assertAlmostEqual(got[1], hi, places=12)

    def test_decision_counts_come_from_the_right_stratum_and_branch(self):
        out = est.estimate(self.labels, self.key, self.frame, seed=9, b=40)
        s = out["all_cases"]["substantive"]
        self.assertEqual((s["n"], s["k"]), (36, sum(map(sum, self.all_hits.values()))))
        w = out["without_recall_flagged"]["substantive"]
        self.assertEqual((w["n"], w["k"]), (32, sum(map(sum, self.kept_hits.values()))))
        self.assertEqual(w["decision"]["wilson"], est.wilson(w["k"], w["n"]))  # wired k, n (wilson tested on literals)
        self.assertEqual(s["decision"]["wilson"], est.wilson(s["k"], s["n"]))

    def test_routine_cases_do_not_leak_into_the_substantive_decision(self):
        out = est.estimate(self.labels, self.key, self.frame, seed=9, b=40)
        s = out["all_cases"]["substantive"]
        self.assertEqual(s["unresolved"], 0)  # the one unresolved case is routine
        self.assertEqual(out["all_cases"]["routine"]["n"], 10)
        self.assertEqual(out["all_cases"]["routine"]["k"], sum(map(sum, self.rou_hits.values())))
        self.assertEqual(s["decision"]["boot"], est.session_bootstrap(self.all_hits, 9, 40))  # substantive sessions only

    def test_two_stratum_overall_bootstrap_draws_strata_in_order_from_one_seeded_stream(self):
        # With b = 40 the first stratum's stream position decides the second's draws, so a wrong b, seed,
        # stratum order or grouping at the overall site shifts the result. Expected value built from the
        # same primitives by hand: substantive first, then routine, weights 100 / 300 by frame size.
        import random
        frame = {"substantive": {"top": 100}, "routine": {"top": 300}}
        out = est.estimate(self.labels, self.key, frame, seed=9, b=40)
        rng = random.Random(9)
        sub = est._resample_rates([(sum(h), len(h)) for _, h in sorted(self.all_hits.items())], rng, 40)
        rou = est._resample_rates([(sum(h), len(h)) for _, h in sorted(self.rou_hits.items())], rng, 40)
        want = est._bounds([(100 * a + 300 * b) / 400 for a, b in zip(sub, rou)])
        got = out["all_cases"]["overall"]["boot"]
        self.assertAlmostEqual(got[0], want[0], places=12)
        self.assertAlmostEqual(got[1], want[1], places=12)


if __name__ == "__main__":
    unittest.main()
