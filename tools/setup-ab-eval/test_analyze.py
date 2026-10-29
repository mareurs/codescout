"""Tests for analyze: the pre-registered decision rule (spec section 6) and secondary metrics.

Every row is built by run.make_row, the runner's own row constructor, so the schema the tests
exercise is the journal's. The synthetic studies are built from the spec's wording (section 6
table), not from the implementation: each one names the condition it is meant to meet.
"""
import itertools
import math
import random
import sys
import unittest
from unittest import mock

import analyze
import run

S = "claude-sonnet-5-5"
O = "claude-opus-5-5"
TASKS = [f"org__repo-{i:02d}" for i in range(12)]
REGISTERED = {t: "foreign" for t in TASKS}
SEED = 7


def integ(cs_calls=0, native=0):
    return {"ok": True, "reasons": [], "codescout_calls": cs_calls, "native_calls": native,
            "hook_events": 1, "sidechain_calls": 0, "duplicateToolUses": 0,
            "toolUseShapeErrors": []}


def row(task, arm, rep, cost, passed, model=S, outcome="scored", effort="medium", **kw):
    graded = outcome in run.GRADED
    fields = dict(outcome=outcome, costUsd=cost, wallMs=60000, numTurns=20,
                  usage={"efforts": [effort]}, effectiveEffort=effort,
                  grade={"pass": passed} if graded else None, integrity=integ(),
                  bashCommands=[], editedFiles=[], inputTokens=1000, outputTokens=100)
    fields.update(kw)
    return run.make_row((task, model, arm, rep), "session-" + task, **fields)


def block(task, bare, cs, full=None, bare_pass=(True, True), cs_pass=True, full_pass=True,
          model=S):
    rows = [row(task, "bare", 0, bare[0], bare_pass[0], model),
            row(task, "bare", 1, bare[1], bare_pass[1], model),
            row(task, "cs", 0, cs, cs_pass, model)]
    if full is not None:
        rows.append(row(task, "full", 0, full, full_pass, model))
    return rows


def study(ratios, spread=1.0, n=12, cs_fail=(), bare_pass=(True, True), model=S, tasks=None,
          full_ratios=None):
    """Task i: bare costs (b, b * spread) with b = 1 + 0.25 i; cs = ratios[i % len] x their mean."""
    rows = []
    for i, task in enumerate((tasks or TASKS)[:n]):
        b = 1.0 + 0.25 * i
        mean = (b + b * spread) / 2
        full = None if full_ratios is None else full_ratios[i % len(full_ratios)] * mean
        rows += block(task, (b, b * spread), ratios[i % len(ratios)] * mean, full=full,
                      bare_pass=bare_pass, cs_pass=i not in cs_fail, model=model)
    return rows


def decide(rows, **kw):
    kw.setdefault("registered_tasks", REGISTERED)
    # Every registered task required unless a test says otherwise (the pre-I-2 strict rule); the
    # frozen 10-of-12 minimum has its own tests (RegisteredMinTest).
    kw.setdefault("registered_min", len(kw["registered_tasks"])
                  if isinstance(kw["registered_tasks"], dict) else 1)
    kw.setdefault("seed", SEED)
    kw.setdefault("instrument_failures", ())
    return analyze.decide(rows, kw.pop("model", S), **kw)


def without(rows, task, arm, rep=None):
    return [r for r in rows if not (r["taskId"] == task and r["arm"] == arm
                                    and (rep is None or r["rep"] == rep))]


# Spec section 6 wording -> data (F = the bare floor; the ruling's step 4 compares to F):
# promising: non-inferiority holds AND the 90% interval's upper bound < 1.0 (and < 1/F). All
#   pass; cs costs 0.55-0.65 of bare; bare reps 2% apart (F = 1.02).
PROMISING = dict(ratios=(0.55, 0.65), spread=1.02)
# no demonstrated advantage: non-inferiority holds AND the interval contains 1.0. Ratios swing
#   0.6 / 1.5, so the interval straddles 1.0.
NO_ADVANTAGE = dict(ratios=(0.6, 1.5), spread=1.01)
VARIED = dict(ratios=tuple(0.5 + 0.09 * i for i in range(12)), spread=1.01)


class DecisionRuleTest(unittest.TestCase):
    # -- the nine named tests from the brief --

    def test_promising(self):
        d = decide(study(**PROMISING))
        self.assertEqual(d.outcome, "promising")
        self.assertEqual(d.n_valid_tasks, 12)
        self.assertEqual(d.pass_counts, {"cs": 12, "bare": 12})
        self.assertLess(d.ci90[1], 1.0)
        self.assertLess(d.ci90[1], 1 / d.floor)
        self.assertAlmostEqual(d.cost_ratio_gmean, math.sqrt(0.55 * 0.65), places=12)
        self.assertAlmostEqual(d.floor, 1.02, places=12)
        self.assertEqual(d.effect, "codescout")
        self.assertIn("the upper bound of the cost ratio's 90% interval is below 1.0", d.reasons)

    def test_no_demonstrated_advantage(self):
        d = decide(study(**NO_ADVANTAGE))
        self.assertEqual(d.outcome, "no demonstrated advantage")
        self.assertLessEqual(d.ci90[0], 1.0)
        self.assertGreaterEqual(d.ci90[1], 1.0)
        self.assertIn("the interval contains 1.0", d.reasons)

    def test_worse_by_pass_rate(self):
        # cs passes 10, bare 12: 10 < 12 - 1, so non-inferiority fails, whatever the cost says.
        d = decide(study(cs_fail=(3, 8), **PROMISING))
        self.assertEqual(d.pass_counts, {"cs": 10, "bare": 12})
        self.assertLess(d.ci90[1], 1.0)  # the cost alone would be promising
        self.assertEqual(d.outcome, "worse")

    def test_worse_by_cost(self):
        # The lower bound of the cost interval is above 1.0 and above F (cs costs 1.4-1.6x bare).
        d = decide(study(ratios=(1.4, 1.6), spread=1.02))
        self.assertGreater(d.ci90[0], d.floor)
        self.assertEqual(d.pass_counts, {"cs": 12, "bare": 12})
        self.assertEqual(d.outcome, "worse")
        self.assertIn("the lower bound of the cost interval is above 1.0", d.reasons)

    def test_inconclusive_when_fewer_valid_tasks_than_registered(self):
        d = decide(study(n=11, **PROMISING))
        self.assertEqual(d.n_valid_tasks, 11)
        self.assertLess(d.ci90[1], 1.0)
        self.assertEqual(d.outcome, "inconclusive")

    def test_inconclusive_when_difference_inside_floor(self):
        # bare reps differ 2x on every task (F = 2.0); cs costs 0.8x bare: the interval is
        # exactly 0.8, below 1.0 but not below 1/F = 0.5, so without the floor it would be
        # promising.
        d = decide(study(ratios=(0.8,), spread=2.0))
        self.assertAlmostEqual(d.floor, 2.0, places=12)
        self.assertLess(d.ci90[1], 1.0)
        self.assertEqual(d.outcome, "inconclusive")
        self.assertTrue(any("inside the variance floor" in r for r in d.reasons))

    def test_invalid_arm_rows_never_enter_the_statistic(self):
        rows = study(**PROMISING)
        # One task's cs run failed the arm-integrity check: charged, never scored.
        task = TASKS[4]
        rows_with_invalid = without(rows, task, "cs") + [
            row(task, "cs", 0, 500.0, None, outcome="invalid-arm")]
        without_task = [r for r in rows if r["taskId"] != task]
        d1 = decide(rows_with_invalid)
        d2 = decide(without_task)
        self.assertEqual(d1.n_valid_tasks, 11)
        self.assertEqual(d1[:6], d2[:6])
        self.assertEqual(d1.outcome, "inconclusive")  # 11 valid tasks < 12 registered
        self.assertIn(task, d1.excluded)
        # An invalid-arm row on a bare rep is the same: the task has one scored bare rep only.
        bare_bad = without(rows, task, "bare", 1) + [
            row(task, "bare", 1, 0.0001, None, outcome="invalid-arm")]
        self.assertEqual(decide(bare_bad)[:6], d2[:6])

    def test_full_vs_bare_is_labelled_setup_not_codescout(self):
        rows = study(full_ratios=(0.55, 0.65), **PROMISING)
        d = decide(rows, comparison=("full", "bare"))
        self.assertEqual(d.outcome, "promising")
        self.assertEqual(d.effect, "the setup")
        self.assertNotIn("codescout", d.claim)
        self.assertIn("the setup", d.claim)
        self.assertEqual(d.pass_counts, {"full": 12, "bare": 12})
        cs = decide(rows)
        self.assertEqual(cs.effect, "codescout")
        self.assertIn("codescout", cs.claim)
        with self.assertRaises(ValueError):  # full - cs has no floor (one cs rep): not this rule
            decide(rows, comparison=("full", "cs"))

    def test_bootstrap_is_paired_by_task_and_seeded(self):
        # Paired: task costs span 1..1e11, every per-task ratio is exactly 0.8. Resampling whole
        # tasks keeps each pair together, so every resample's geometric mean is 0.8. An unpaired
        # bootstrap (cs and bare resampled apart) would spread over orders of magnitude.
        rows = []
        for i, task in enumerate(TASKS):
            b = 10.0 ** i
            rows += block(task, (b, b), 0.8 * b)
        d = decide(rows)
        self.assertAlmostEqual(d.ci90[0], 0.8, places=9)
        self.assertAlmostEqual(d.ci90[1], 0.8, places=9)
        # Seeded: the same seed gives the same Decision; another seed may differ (twelve
        # distinct per-task ratios, so the percentiles are not pinned to a few discrete values).
        varied = study(**VARIED)
        self.assertEqual(decide(varied, seed=1), decide(varied, seed=1))
        cis = {decide(varied, seed=s).ci90 for s in range(1, 6)}
        self.assertGreater(len(cis), 1)
        # Row order does not matter: tasks are resampled in task-id order.
        shuffled = list(varied)
        random.Random(3).shuffle(shuffled)
        self.assertEqual(decide(shuffled, seed=1), decide(varied, seed=1))
        # Whole tasks: with two per-task ratios (0.6, 1.5) each bound is the geometric mean of
        # 12 whole-task draws, i.e. 0.6^k 1.5^(12-k) to the 1/12 for an integer k.
        allowed = [math.exp((k * math.log(0.6) + (12 - k) * math.log(1.5)) / 12)
                   for k in range(13)]
        for bound in decide(study(**NO_ADVANTAGE), seed=1).ci90:
            self.assertTrue(any(abs(bound - a) < 1e-9 for a in allowed), bound)

    def test_interval_is_the_outward_5th_and_95th_order_statistics(self):
        # B = 10,000: floor(0.05 x 9,999) = 499 and ceil(0.95 x 9,999) = 9,500 (0-based).
        lo, hi = analyze.interval90([math.log(x) for x in range(1, 10_001)])
        self.assertAlmostEqual(lo, 500, places=6)
        self.assertAlmostEqual(hi, 9501, places=6)
        # B = 21: 0.05 x 20 = 1 and 0.95 x 20 = 19, exact.
        lo, hi = analyze.interval90([math.log(x) for x in range(1, 22)])
        self.assertAlmostEqual(lo, 2, places=9)
        self.assertAlmostEqual(hi, 20, places=9)
        # decide's interval is exactly that, over the 10,000 bootstrap means
        rows = study(**VARIED)
        units, _ = analyze.task_units(rows, S, "cs")
        log_ratios = [math.log(u.treat_cost / (sum(u.bare_costs) / 2)) for u in units]
        means = analyze.bootstrap_log_means(log_ratios, SEED)
        self.assertEqual(len(means), 10_000)
        self.assertEqual(means, sorted(means))
        self.assertEqual(decide(rows).ci90, (math.exp(means[499]), math.exp(means[9500])))

    def test_golden_interval_for_a_fixed_seed(self):
        # Freezes the RNG consumption: random.Random(7), 10,000 x 12 randrange(12) draws, tasks
        # in id order. Measured on CPython 3.14.8. Python guarantees the same random() stream
        # for a seed, NOT randrange's algorithm across versions, so a different Python may move
        # these digits legitimately; the study records its Python version at the freeze.
        d = decide(study(**VARIED), seed=SEED)
        self.assertEqual(sys.version_info[:2], (3, 14), "golden values measured on 3.14")
        self.assertAlmostEqual(d.ci90[0], GOLDEN_CI[0], places=12)
        self.assertAlmostEqual(d.ci90[1], GOLDEN_CI[1], places=12)

    # -- resamples, exclusions, malformed rows --

    def test_bootstrap_draws_10000_resamples_of_n_tasks(self):
        draws = []

        class Counting(random.Random):
            def randrange(self, *a, **k):
                draws.append(a)
                return super().randrange(*a, **k)

        with mock.patch.object(analyze.random, "Random", Counting):
            decide(study(**PROMISING))
        self.assertEqual(analyze.RESAMPLES, 10_000)
        self.assertEqual(len(draws), 10_000 * 12)
        self.assertTrue(all(a == (12,) for a in draws))

    def test_dev_rows_never_enter_the_statistic(self):
        rows = study(**PROMISING)
        dev = run.DEVELOPMENT_TASKS[0]
        dev_rows = [row(dev, "bare", 0, 0.01, True, outcome="dev-scored"),
                    row(dev, "bare", 1, 0.01, True, outcome="dev-scored"),
                    row(dev, "cs", 0, 100.0, False, outcome="dev-scored")]
        self.assertEqual(decide(rows + dev_rows), decide(rows))
        # A dev task can never be `scored` (make_row refuses; analyze refuses a forged row too).
        forged = dict(dev_rows[2], outcome="scored")
        with self.assertRaisesRegex(analyze.MalformedRow, "development"):
            decide(rows + [forged])
        forged_stage = dict(dev_rows[2], outcome="scored", stage="main")
        with self.assertRaisesRegex(analyze.MalformedRow, "stage"):
            decide(rows + [forged_stage])

    def test_neutral_and_infra_outcomes_never_enter_the_statistic(self):
        base = study(**PROMISING)
        task = TASKS[2]
        reference = decide([r for r in base if r["taskId"] != task])
        for outcome in ("plan-limit", "infra-oom", "infra-other", "unknown-spend"):
            with self.subTest(outcome=outcome):
                rows = without(base, task, "cs") + [
                    row(task, "cs", 0, None if outcome == "unknown-spend" else 3.0, None,
                        outcome=outcome)]
                d = decide(rows)
                self.assertEqual(d[:6], reference[:6])
                self.assertIn(task, d.excluded)
        # A run whose latest journal row is still `pending` is not scored either.
        rows = base + [row(TASKS[0], "full", 0, None, None, outcome="pending")]
        self.assertEqual(decide(rows)[:6], decide(base)[:6])

    def test_latest_row_of_a_run_wins(self):
        base = study(**PROMISING)
        cs = [r for r in base if r["taskId"] == TASKS[1] and r["arm"] == "cs"][0]
        pending_first = [dict(cs, outcome="pending", costUsd=None, grade=None)] + base
        self.assertEqual(decide(pending_first), decide(base))

    def test_effort_mismatch_task_is_excluded(self):
        rows = study(**PROMISING)
        task = TASKS[5]
        rows = [dict(r, effectiveEffort="high") if (r["taskId"] == task and r["arm"] == "cs")
                else r for r in rows]
        d = decide(rows)
        self.assertEqual(d.n_valid_tasks, 11)
        self.assertIn("effort", d.excluded[task])
        self.assertEqual(d.outcome, "inconclusive")
        # The mismatch can sit in the arm not compared: the task's comparison is still invalid.
        rows = study(full_ratios=(0.6,), **PROMISING)
        rows = [dict(r, effectiveEffort=None) if (r["taskId"] == task and r["arm"] == "full")
                else r for r in rows]
        self.assertIn(task, decide(rows).excluded)

    def test_malformed_rows_raise(self):
        base = study(**PROMISING)
        good = base[0]
        cases = {
            "missing field": {k: v for k, v in good.items() if k != "costUsd"},
            "extra field": dict(good, surprise=1),
            "pass is int": dict(good, grade={"pass": 1}),
            "pass is string": dict(good, grade={"pass": "true"}),
            "no grade": dict(good, grade=None),
            "cost None": dict(good, costUsd=None),
            "cost zero": dict(good, costUsd=0.0),
            "cost bool": dict(good, costUsd=True),
            "cost nan": dict(good, costUsd=float("nan")),
            "bad outcome": dict(good, outcome="passed"),
            "runId mismatch": dict(good, runId="x.y.z.0"),
            "unknown model": dict(good, model="claude-sonnet-5.5"),
            "bad arm": dict(good, arm="codescout"),
            "not a dict": ["row"],
        }
        for name, bad in cases.items():
            with self.subTest(name), self.assertRaises(analyze.MalformedRow):
                decide(base[1:] + [bad])
        self.assertTrue(issubclass(analyze.MalformedRow, ValueError))

    def test_more_reps_than_the_plan_raise(self):
        rows = study(**PROMISING) + [row(TASKS[0], "bare", 2, 1.0, True)]
        with self.assertRaisesRegex(analyze.MalformedRow, "bare"):
            decide(rows)

    def test_models_are_analysed_separately(self):
        sonnet = study(**PROMISING)
        opus = study(ratios=(1.4, 1.6), spread=1.02, model=O, cs_fail=(1, 2, 3))
        self.assertEqual(decide(sonnet + opus), decide(sonnet))
        self.assertEqual(decide(sonnet + opus, model=O).outcome, "worse")
        with self.assertRaises(ValueError):
            decide(sonnet, model="sonnet")

    def test_arguments_are_checked(self):
        rows = study(**PROMISING)
        for kw in ({"registered_tasks": 0}, {"registered_tasks": True},
                   {"registered_tasks": 12}, {"registered_tasks": set(TASKS)},
                   {"registered_tasks": {}},
                   {"registered_tasks": dict(REGISTERED, **{TASKS[0]: "public"})},
                   {"registered_tasks": dict(REGISTERED, **{run.DEVELOPMENT_TASKS[0]: "foreign"})},
                   {"seed": "7"}, {"seed": False}, {"comparison": ("cs", "full")},
                   {"instrument_failures": None}, {"instrument_failures": "leak gate"},
                   {"instrument_failures": ("",)}):
            with self.subTest(kw), self.assertRaises(ValueError):
                decide(rows, **kw)

    def test_invalid_instrument_overrides_everything(self):
        d = decide(study(**PROMISING), instrument_failures=("leak gate: cs-p08 overlap 3",))
        self.assertEqual(d.outcome, "invalid instrument")
        self.assertIn("leak gate", " ".join(d.reasons))

    def test_no_valid_task_is_inconclusive_without_numbers(self):
        d = decide([])
        self.assertEqual(d.outcome, "inconclusive")
        self.assertEqual(d.n_valid_tasks, 0)
        self.assertIsNone(d.cost_ratio_gmean)
        self.assertIsNone(d.ci90)
        self.assertIsNone(d.floor)

    def test_excluded_defaults_to_an_empty_mapping(self):
        d = analyze.Decision("inconclusive", {}, None, None, None, 0)
        self.assertEqual(dict(d.excluded), {})
        self.assertEqual(decide(study(**PROMISING)).excluded, {})


class FixRound1Test(unittest.TestCase):
    """Review round 1: I1 (registered set), I2 (re-runs), I5 (required instrument gates), and
    the pooled contamination note."""

    def test_unregistered_task_raises_instead_of_filling_the_count(self):
        # Reviewer's scenario: 11 registered tasks + one block of `zz__unregistered-99` gave
        # promising with n_valid 12. Now the extra block is a protocol breach and raises.
        registered = {t: "foreign" for t in TASKS[:11]}
        rows = study(n=11, **PROMISING) + block("zz__unregistered-99", (1.0, 1.0), 0.6)
        with self.assertRaisesRegex(analyze.MalformedRow, "zz__unregistered-99"):
            decide(rows, registered_tasks=registered)
        # Even a non-scored row of an unregistered main task raises; another model's too.
        rows = study(n=11, **PROMISING) + [row("zz__unregistered-99", "cs", 0, 1.0, None,
                                               model=O, outcome="invalid-arm")]
        with self.assertRaises(analyze.MalformedRow):
            decide(rows, registered_tasks=registered)
        # The count comes from the registered mapping, never from the rows.
        d = decide(study(n=11, **PROMISING), registered_tasks=REGISTERED)
        self.assertEqual((d.n_valid_tasks, d.outcome), (11, "inconclusive"))

    def test_bare_rerun_after_invalid_arm_raises(self):
        # Reviewer's scenario: bare rep 1 invalid-arm, then a new scored bare rep 2.
        rows = study(**PROMISING)
        task = TASKS[3]
        rows = without(rows, task, "bare", 1) + [
            row(task, "bare", 1, 1.0, None, outcome="invalid-arm"),
            row(task, "bare", 2, 1.0, True)]
        with self.assertRaisesRegex(analyze.MalformedRow, "re-run"):
            decide(rows)

    def test_cs_rerun_after_invalid_arm_raises(self):
        rows = study(**PROMISING)
        task = TASKS[3]
        rows = without(rows, task, "cs") + [
            row(task, "cs", 0, 1.0, None, outcome="invalid-arm"),
            row(task, "cs", 1, 0.5, True)]
        with self.assertRaisesRegex(analyze.MalformedRow, "re-run"):
            decide(rows)
        rows = study(full_ratios=(0.6,), **PROMISING) + [row(TASKS[0], "full", 1, 1.0, True)]
        with self.assertRaisesRegex(analyze.MalformedRow, "re-run"):
            decide(rows)

    def test_second_final_row_for_a_run_raises(self):
        # The same runId written again after its final row: a re-run in place.
        rows = study(**PROMISING)
        task = TASKS[3]
        rows = without(rows, task, "cs") + [
            row(task, "cs", 0, 1.0, None, outcome="invalid-arm"),
            row(task, "cs", 0, 0.5, True)]
        with self.assertRaisesRegex(analyze.MalformedRow, "re-run"):
            decide(rows)
        with self.assertRaises(analyze.MalformedRow):
            analyze.secondary(rows)

    def test_instrument_failures_is_required(self):
        rows = study(**PROMISING)
        with self.assertRaises(TypeError):
            analyze.decide(rows, S, registered_tasks=REGISTERED, registered_min=12, seed=SEED)
        with self.assertRaises(TypeError):
            analyze.strata(rows, S, REGISTERED, registered_min=12, seed=SEED)
        self.assertEqual(analyze.decide(rows, S, registered_tasks=REGISTERED, registered_min=12,
                                        seed=SEED, instrument_failures=()).outcome, "promising")

    def test_decide_notes_contamination_whenever_private_tasks_are_pooled(self):
        private = [f"cs-p{i:02d}" for i in range(6)]
        registered = {**{t: "foreign" for t in TASKS[:6]}, **{t: "private" for t in private}}
        rows = study(n=6, **PROMISING) + study(n=6, tasks=private, **PROMISING)
        d = decide(rows, registered_tasks=registered)
        self.assertEqual(d.stratum, "pooled")
        self.assertIn("contaminated", " ".join(d.notes))
        foreign_only = decide(study(**PROMISING))
        self.assertEqual((foreign_only.stratum, foreign_only.notes), ("foreign", ()))


class RulingOrderTest(unittest.TestCase):
    """The coordinator's binding order for spec section 6 (fix round 1, I3 + I4)."""

    def test_reviewer_example_directional_interval_inside_floor(self):
        out, reasons = analyze.rule(12, 12, 12, 12, 0.7, (0.55, 0.9), 1.2)
        self.assertEqual(out, "inconclusive")  # 0.9 is not below 1/1.2 = 0.833
        self.assertIn("the upper bound of the cost ratio's 90% interval is below 1.0", reasons)
        self.assertTrue(any("inside the variance floor" in r for r in reasons))

    def test_the_floor_reason_is_only_for_an_interval_that_misses_1_inside_the_floor(self):
        # the reviewer's example: the interval contains 1.0, so it is NOT "inside the floor"
        out, reasons = analyze.rule(12, 12, 12, 12, 0.7, (0.3, 1.5), 1.2)
        self.assertEqual(out, "no demonstrated advantage")
        self.assertIn("the interval contains 1.0", reasons)
        self.assertFalse(any("variance floor" in r for r in reasons), reasons)
        # the residual cases (1/F <= high < 1.0, 1.0 < low <= F) still carry it
        for ci in ((0.9, 0.95), (1.05, 1.1)):
            with self.subTest(ci=ci):
                out, reasons = analyze.rule(12, 12, 12, 12, 1.0, ci, 1.2)
                self.assertEqual(out, "inconclusive")
                self.assertTrue(any("inside the variance floor" in r for r in reasons), reasons)
        # beyond the floor is not inside it either
        for ci in ((1.3, 1.7), (0.3, 0.5)):
            with self.subTest(ci=ci):
                _, reasons = analyze.rule(12, 12, 12, 12, 1.0, ci, 1.2)
                self.assertFalse(any("variance floor" in r for r in reasons), reasons)

    def test_rule_and_cost_branch_validate_floor_and_registered_count(self):
        ci = (0.4, 0.6)
        for floor in (0.99, 0.0, -1.0, float("nan"), float("inf"), True, "2", None):
            with self.subTest(floor=floor):
                with self.assertRaises(ValueError):
                    analyze.rule(12, 12, 12, 12, 0.5, ci, floor)
                with self.assertRaises(ValueError):
                    analyze.cost_branch(ci, floor)
        for registered in (0, -1, 1.5, True, None):
            with self.subTest(registered=registered):
                with self.assertRaises(ValueError):
                    analyze.rule(12, registered, 12, 12, 0.5, ci, 1.0)
        # the boundary values are accepted; no cost interval and floor when no task is valid
        self.assertEqual(analyze.rule(12, 1, 12, 12, 0.5, ci, 1.0)[0], "promising")
        self.assertEqual(analyze.cost_branch(ci, 1.0), "promising")
        self.assertEqual(analyze.rule(0, 12, 0, 0, None, None, None)[0], "inconclusive")

    def test_reviewer_example_worse_by_cost_not_claimed_inside_floor(self):
        out, reasons = analyze.rule(12, 12, 12, 12, 1.5, (1.3, 1.7), 2.0)
        self.assertEqual(out, "inconclusive")  # low 1.3 <= F 2.0
        self.assertIn("the lower bound of the cost interval is above 1.0", reasons)
        self.assertFalse(any("above the floor" in r for r in reasons))

    def test_whole_interval_beyond_floor_is_worse(self):
        out, reasons = analyze.rule(12, 12, 12, 12, 2.5, (2.1, 2.9), 2.0)
        self.assertEqual(out, "worse")
        self.assertTrue(any("above the floor" in r for r in reasons))

    def test_too_few_tasks_lists_every_condition_that_held(self):
        out, reasons = analyze.rule(11, 12, 5, 11, 1.8, (1.5, 2.1), 1.1)
        self.assertEqual(out, "inconclusive")
        joined = " | ".join(reasons)
        self.assertIn("fewer than the registered number of valid tasks (11 < 12)", joined)
        self.assertIn("non-inferiority fails", joined)
        self.assertIn("the lower bound of the cost interval is above 1.0", joined)
        self.assertIn("above the floor", joined)

    def test_non_inferiority_failure_is_not_masked_by_the_floor(self):
        out, reasons = analyze.rule(12, 12, 9, 12, 0.9, (0.8, 0.95), 2.0)
        self.assertEqual(out, "worse")
        self.assertTrue(any("inside the variance floor" in r for r in reasons))

    def test_ties_claim_less(self):
        rule = analyze.rule
        self.assertEqual(rule(12, 12, 12, 12, 0.45, (0.4, 0.5), 2.0)[0], "inconclusive")
        self.assertEqual(rule(12, 12, 12, 12, 0.45, (0.4, 0.4999), 2.0)[0], "promising")
        self.assertEqual(rule(12, 12, 12, 12, 2.5, (2.0, 3.0), 2.0)[0], "inconclusive")
        self.assertEqual(rule(12, 12, 12, 12, 2.5, (2.0001, 3.0), 2.0)[0], "worse")
        self.assertEqual(rule(12, 12, 12, 12, 0.8, (0.7, 1.0), 1.0)[0],
                         "no demonstrated advantage")
        self.assertEqual(rule(12, 12, 12, 12, 1.2, (1.0, 1.3), 1.0)[0],
                         "no demonstrated advantage")
        # bare mean pass 0.5 counts, so 11 vs 12 holds and 10 vs 12 fails
        self.assertEqual(rule(12, 12, 11, 12, 0.5, (0.4, 0.6), 1.0)[0], "promising")
        self.assertEqual(rule(12, 12, 10, 12, 0.5, (0.4, 0.6), 1.0)[0], "worse")

    def test_instrument_first(self):
        out, reasons = analyze.rule(11, 12, 5, 12, 0.5, (0.4, 0.6), 1.0, ("leak",))
        self.assertEqual(out, "invalid instrument")
        self.assertTrue(any("fewer than" in r for r in reasons))

    def test_cost_branch_is_exhaustive_exclusive_and_reaches_every_outcome(self):
        grid = [0.3, 0.5, 0.7, 0.9, 0.99, 1.0, 1.01, 1.1, 1.4, 2.0, 2.5, 4.0]
        seen = set()
        for floor in (1.0, 1.05, 1.25, 2.0):
            for low, high in itertools.combinations_with_replacement(grid, 2):
                worse = low > floor
                promising = high < 1 / floor
                contains = low <= 1.0 <= high
                self.assertLessEqual(worse + promising + contains, 1, (low, high, floor))
                out = analyze.cost_branch((low, high), floor)
                expect = ("worse" if worse else "promising" if promising
                          else "no demonstrated advantage" if contains else "inconclusive")
                self.assertEqual(out, expect)
                if floor == 1.0:
                    self.assertNotEqual(out, "inconclusive")
                seen.add(out)
        self.assertEqual(seen, {"worse", "promising", "no demonstrated advantage",
                                "inconclusive"})
        # every label is reachable through rule()
        self.assertEqual({analyze.rule(*a)[0] for a in (
            (12, 12, 12, 12, 0.5, (0.4, 0.6), 1.0),
            (12, 12, 12, 12, 1.0, (0.8, 1.2), 1.0),
            (12, 12, 12, 12, 2.0, (1.5, 2.5), 1.0),
            (11, 12, 12, 12, 0.5, (0.4, 0.6), 1.0),
            (12, 12, 12, 12, 0.5, (0.4, 0.6), 1.0, ("x",)))}, set(analyze.LABELS))


class AmbiguityDecisionsTest(unittest.TestCase):
    """Each ambiguity is resolved toward the reading that makes `promising` harder."""

    def test_single_bare_rep_excludes_the_task(self):
        task = TASKS[6]
        d = decide(without(study(**PROMISING), task, "bare", 1))
        self.assertEqual(d.n_valid_tasks, 11)
        self.assertIn("bare", d.excluded[task])
        self.assertEqual(d.outcome, "inconclusive")

    def test_bare_mean_pass_of_one_half_is_a_pass(self):
        # Every task: one bare rep passes, one fails -> mean 0.5 -> bare passes the task.
        d = decide(study(bare_pass=(True, False), cs_fail=(0,), **PROMISING))
        self.assertEqual(d.pass_counts, {"cs": 11, "bare": 12})
        self.assertEqual(d.outcome, "promising")  # 11 >= 12 - 1: non-inferiority holds
        d = decide(study(bare_pass=(False, False), **PROMISING))
        self.assertEqual(d.pass_counts["bare"], 0)


class StrataTest(unittest.TestCase):
    def setUp(self):
        self.foreign = TASKS[:6]
        self.private = [f"cs-p{i:02d}" for i in range(6)]
        self.registered = {**{t: "foreign" for t in self.foreign},
                           **{t: "private" for t in self.private}}
        self.rows = (study(n=6, tasks=self.foreign, ratios=(0.4, 0.45), spread=1.01)
                     + study(n=6, tasks=self.private, ratios=(1.4, 1.6), spread=1.01))

    def strata(self, registered=None, rows=None):
        registered = registered or self.registered
        return analyze.strata(self.rows if rows is None else rows, S, registered,
                              registered_min=len(registered), seed=SEED, instrument_failures=())

    def test_foreign_private_and_pooled_are_separate(self):
        out = self.strata()
        self.assertEqual(set(out), {"foreign", "private", "pooled"})
        self.assertEqual(out["foreign"].outcome, "promising")
        self.assertEqual(out["private"].outcome, "worse")
        self.assertEqual(out["foreign"].n_valid_tasks, 6)
        self.assertEqual(out["private"].n_valid_tasks, 6)
        self.assertEqual(out["pooled"].n_valid_tasks, 12)
        self.assertEqual([out[k].stratum for k in ("foreign", "private", "pooled")],
                         ["foreign", "private", "pooled"])
        self.assertIn("contaminated", " ".join(out["private"].notes))
        self.assertIn("contaminated", " ".join(out["pooled"].notes))
        self.assertEqual(out["foreign"].notes, ())
        # the same function: decide over the foreign part gives the same answer
        same = decide([r for r in self.rows if r["taskId"] in self.foreign],
                      registered_tasks={t: "foreign" for t in self.foreign})
        self.assertEqual(same, out["foreign"])

    def test_registered_count_is_per_stratum(self):
        registered = dict(self.registered, **{"org__repo-11": "foreign"})
        out = self.strata(registered)
        self.assertEqual(out["foreign"].outcome, "inconclusive")  # 6 valid < 7 registered
        self.assertEqual(out["private"].outcome, "worse")
        self.assertEqual(out["pooled"].n_valid_tasks, 12)

    def test_a_stratum_without_registered_tasks_is_none(self):
        foreign_rows = [r for r in self.rows if r["taskId"] in self.foreign]
        out = self.strata({t: "foreign" for t in self.foreign}, foreign_rows)
        self.assertIsNone(out["private"])
        self.assertEqual(out["pooled"].stratum, "foreign")

    def test_an_unregistered_or_badly_kinded_task_raises(self):
        registered = dict(self.registered)
        del registered[self.private[0]]
        with self.assertRaises(analyze.MalformedRow):
            self.strata(registered)
        with self.assertRaises(ValueError):
            self.strata(dict(self.registered, **{self.foreign[0]: "public"}))


class SecondaryTest(unittest.TestCase):
    def test_empty_rows_give_every_group_without_crashing(self):
        out = analyze.secondary([])
        self.assertEqual(set(out), set(run.MODELS))
        for model in run.MODELS:
            self.assertEqual(set(out[model]), {"bare", "cs", "full"})
            for g in out[model].values():
                self.assertEqual(g["n_runs"], 0)
                self.assertEqual(g["medians"], {"inputTokens": None, "outputTokens": None,
                                                "numTurns": None, "wallMs": None})
                self.assertIsNone(g["all_targets_share"])
                self.assertIsNone(g["codescout_share"])
                self.assertIsNone(g["edits_outside_gold"])
        out = analyze.secondary([], gold_files={})
        self.assertEqual(out[S]["cs"]["edits_outside_gold"], None)

    def test_values_per_arm_and_model(self):
        rows = [
            row("org__a-1", "cs", 0, 1.0, True, inputTokens=100, outputTokens=10, numTurns=5,
                wallMs=1000, integrity=integ(3, 1), bashCommands=["cargo test"],
                editedFiles=["src/a.rs", "src/b.rs"]),
            row("org__a-2", "cs", 0, 1.0, False, inputTokens=300, outputTokens=30, numTurns=7,
                wallMs=3000, integrity=integ(1, 3), bashCommands=["cargo test --lib"],
                editedFiles=["src/c.rs"]),
            row("org__a-3", "cs", 0, 1.0, True, inputTokens=None, outputTokens=None,
                numTurns=9, wallMs=2000, integrity=integ(0, 0), bashCommands=[],
                editedFiles=[]),
            # never scored: not in any secondary figure
            row("org__a-4", "cs", 0, 9.0, None, outcome="invalid-arm", inputTokens=10 ** 9,
                integrity=integ(0, 50)),
            row("org__a-1", "bare", 0, 1.0, True, model=O, numTurns=4,
                integrity=integ(0, 4)),
        ]
        gold = {"org__a-1": ["src/a.rs"], "org__a-2": ["src/c.rs"], "org__a-3": ["x"]}
        out = analyze.secondary(rows, gold_files=gold)
        cs = out[S]["cs"]
        self.assertEqual(cs["n_runs"], 3)
        self.assertEqual(cs["medians"], {"inputTokens": 200, "outputTokens": 20, "numTurns": 7,
                                         "wallMs": 2000})
        self.assertEqual(cs["missing"]["inputTokens"], 1)
        self.assertAlmostEqual(cs["all_targets_share"], 1 / 3)
        self.assertEqual(cs["codescout_share"], 4 / 8)
        self.assertEqual(cs["edits_outside_gold"], {"total": 1, "per_run_mean": 1 / 3})
        self.assertEqual(out[O]["bare"]["n_runs"], 1)
        self.assertEqual(out[O]["bare"]["codescout_share"], 0.0)
        self.assertEqual(out[S]["bare"]["n_runs"], 0)
        # gold files are optional; a task missing from a given map fails closed
        self.assertIsNone(analyze.secondary(rows)[S]["cs"]["edits_outside_gold"])
        with self.assertRaises(ValueError):
            analyze.secondary(rows, gold_files={"org__a-1": ["src/a.rs"]})

    def test_secondary_uses_the_statistic_population(self):
        rows = study(**PROMISING)
        task = TASKS[5]
        rows = [dict(r, effectiveEffort="high") if (r["taskId"] == task and r["arm"] == "cs")
                else r for r in rows]
        out = analyze.secondary(rows)
        self.assertEqual(out[S]["cs"]["n_runs"], 11)  # the effort-invalid task is left out
        self.assertEqual(out[S]["bare"]["n_runs"], 22)
        out = analyze.secondary(rows, task_filter={TASKS[0]})
        self.assertEqual(out[S]["bare"]["n_runs"], 2)

    def test_malformed_secondary_fields_raise(self):
        good = row("org__a-1", "cs", 0, 1.0, True)
        for bad in (dict(good, bashCommands=None), dict(good, bashCommands=[3]),
                    dict(good, integrity={"codescout_calls": 1}),
                    dict(good, integrity=dict(integ(), native_calls=True)),
                    dict(good, inputTokens="100"), dict(good, editedFiles=None)):
            with self.subTest(bad), self.assertRaises(analyze.MalformedRow):
                analyze.secondary([bad], gold_files={"org__a-1": []})

    def test_compiles_all_targets(self):
        yes = ["cargo test", "cargo test --workspace", "cargo test some_filter",
               "cargo test -- --nocapture", "cargo t", "cargo +nightly test",
               "RUST_LOG=debug cargo test", "cd /w && cargo test 2>&1 | tail -50",
               "timeout 600 cargo test -q", "cargo build --tests", "cargo check --all-targets",
               "cargo clippy --all-targets -- -D warnings", "cargo test --lib --tests",
               "cargo test --no-run", "~/.cargo/bin/cargo test", "cargo nextest run --tests"]
        no = ["cargo test-foo", "cargo testx", "cargo test --lib", "cargo test --test it",
              "cargo test --test=it", "cargo test --doc", "cargo build", "cargo check",
              "cargo test --bin x", "echo 'cargo test'", "cargo test --lib -- --tests",
              "grep -r 'cargo test' .", "ls", "", "cargo"]
        for c in yes:
            with self.subTest(c):
                self.assertTrue(analyze.compiles_all_targets(c))
        for c in no:
            with self.subTest(c):
                self.assertFalse(analyze.compiles_all_targets(c))
class FloorFormulaTest(unittest.TestCase):
    """Final review I-1: F is pinned to GM over tasks of max(bare0, bare1) / min(bare0, bare1)
    (operator decision 2026-10-29, "the larger reading"). The bare spreads differ per task and
    change direction, so the other candidate formulas give other numbers."""

    SPREADS = (2.0, 0.5, 1.5, 1 / 3, 1.1, 0.8, 3.0, 0.9, 1.25, 0.4, 1.05, 0.7)

    @staticmethod
    def gm(xs):
        return math.exp(math.fsum(math.log(x) for x in xs) / len(xs))

    def bare_costs(self, spreads):
        b0 = [1.0 + 0.25 * i for i in range(len(spreads))]
        return b0, [b * s for b, s in zip(b0, spreads)]

    def rows(self, spreads):
        b0, b1 = self.bare_costs(spreads)
        rows = []
        for task, x, y in zip(TASKS, b0, b1):
            rows += block(task, (x, y), 0.6 * (x + y) / 2)
        return rows

    def test_floor_is_the_geometric_mean_of_per_task_max_over_min(self):
        b0, b1 = self.bare_costs(self.SPREADS)
        d = decide(self.rows(self.SPREADS))
        per_task = [max(x, y) / min(x, y) for x, y in zip(b0, b1)]
        self.assertAlmostEqual(d.floor, self.gm(per_task), places=12)
        # Algebraically the same: GM of the per-task max over GM of the per-task min.
        self.assertAlmostEqual(d.floor, self.gm([max(x, y) for x, y in zip(b0, b1)])
                               / self.gm([min(x, y) for x, y in zip(b0, b1)]), places=12)
        # Not the rep-0 / rep-1 reading (larger over smaller), not an arithmetic mean.
        rep_reading = max(self.gm(b0) / self.gm(b1), self.gm(b1) / self.gm(b0))
        self.assertGreater(d.floor - rep_reading, 0.3)
        self.assertGreater(math.fsum(per_task) / len(per_task) - d.floor, 0.05)

    def test_spreads_that_cancel_still_give_the_larger_floor(self):
        # 2x up, then 2x down, alternating: the rep-0 / rep-1 reading is exactly 1.0; F is 2.0.
        spreads = (2.0, 0.5) * 6
        b0, b1 = self.bare_costs(spreads)
        self.assertAlmostEqual(self.gm(b0) / self.gm(b1), 1.0, places=12)
        self.assertAlmostEqual(decide(self.rows(spreads)).floor, 2.0, places=12)


class RegisteredMinTest(unittest.TestCase):
    """Final review I-2, operator decision 2026-10-29: at least 10 of the 12 registered tasks
    valid per model; fewer is inconclusive; the registered task set stays 12."""

    def invalidate(self, rows, *tasks):
        for t in tasks:
            rows = without(rows, t, "cs") + [row(t, "cs", 0, 1.0, None, outcome="invalid-arm")]
        return rows

    def split(self):
        foreign, private = TASKS[:6], [f"cs-p{i:02d}" for i in range(6)]
        return foreign, private, {**{t: "foreign" for t in foreign},
                                  **{t: "private" for t in private}}

    def test_the_frozen_minimum_is_ten(self):
        self.assertEqual(analyze.REGISTERED_MIN, 10)
        self.assertEqual(len(REGISTERED), 12)

    def test_ten_valid_of_twelve_decides(self):
        rows = self.invalidate(study(**PROMISING), TASKS[3], TASKS[8])
        d = decide(rows, registered_min=analyze.REGISTERED_MIN)
        self.assertEqual((d.n_valid_tasks, d.outcome), (10, "promising"))
        self.assertEqual(sorted(d.excluded), [TASKS[3], TASKS[8]])
        self.assertFalse(any("fewer than" in r for r in d.reasons))
        # The same data under the old all-12 rule is inconclusive: the minimum is what decides.
        self.assertEqual(decide(rows, registered_min=12).outcome, "inconclusive")

    def test_nine_valid_of_twelve_is_inconclusive(self):
        rows = self.invalidate(study(**PROMISING), TASKS[1], TASKS[3], TASKS[8])
        d = decide(rows, registered_min=analyze.REGISTERED_MIN)
        self.assertEqual((d.n_valid_tasks, d.outcome), (9, "inconclusive"))
        self.assertIn("fewer than the registered number of valid tasks (9 < 10)", d.reasons)

    def test_pilot_only_data_is_inconclusive_in_every_stratum(self):
        foreign, private, registered = self.split()
        rows = (study(n=3, tasks=foreign, **PROMISING)
                + study(n=3, tasks=private, **PROMISING))
        out = analyze.strata(rows, S, registered, registered_min=analyze.REGISTERED_MIN,
                             seed=SEED, instrument_failures=())
        for kind, n, need in (("foreign", 3, 5), ("private", 3, 5), ("pooled", 6, 10)):
            with self.subTest(kind):
                self.assertEqual(out[kind].outcome, "inconclusive")
                self.assertIn(f"fewer than the registered number of valid tasks ({n} < {need})",
                              out[kind].reasons)
        d = decide(rows, registered_tasks=registered, registered_min=analyze.REGISTERED_MIN)
        self.assertEqual(d.outcome, "inconclusive")

    def test_a_stratum_needs_the_same_share_rounded_up(self):
        self.assertEqual(analyze.stratum_min(10, 6, 12), 5)
        self.assertEqual(analyze.stratum_min(10, 7, 12), 6)    # 5.83 -> 6
        self.assertEqual(analyze.stratum_min(10, 5, 12), 5)    # 4.17 -> 5
        self.assertEqual(analyze.stratum_min(12, 6, 12), 6)
        self.assertEqual(analyze.stratum_min(1, 1, 12), 1)
        for bad in ((13, 6, 12), (10, 13, 12), (0, 6, 12), (10, 0, 12)):
            with self.subTest(bad), self.assertRaises(ValueError):
                analyze.stratum_min(*bad)
        foreign, private, registered = self.split()
        rows = (self.invalidate(study(n=6, tasks=foreign, **PROMISING), foreign[0])
                + self.invalidate(study(n=6, tasks=private, **PROMISING), private[0], private[1]))
        out = analyze.strata(rows, S, registered, registered_min=10, seed=SEED,
                             instrument_failures=())
        self.assertEqual((out["foreign"].n_valid_tasks, out["foreign"].outcome),
                         (5, "promising"))                              # 5 of 6 >= 5
        self.assertEqual((out["private"].n_valid_tasks, out["private"].outcome),
                         (4, "inconclusive"))                           # 4 of 6 < 5
        self.assertEqual((out["pooled"].n_valid_tasks, out["pooled"].outcome),
                         (9, "inconclusive"))                           # 9 of 12 < 10

    def test_registered_min_is_required_and_validated(self):
        rows = study(**PROMISING)
        with self.assertRaises(TypeError):
            analyze.decide(rows, S, registered_tasks=REGISTERED, seed=SEED,
                           instrument_failures=())
        for bad in (0, 13, True, 10.0, None):
            with self.subTest(bad), self.assertRaises(ValueError):
                decide(rows, registered_min=bad)


# Measured 2026-10-28 on CPython 3.14.8: decide(study(**VARIED), seed=7).ci90.
GOLDEN_CI = (0.8026677052434217, 1.1010586634143393)

if __name__ == "__main__":
    unittest.main()
