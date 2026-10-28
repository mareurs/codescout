"""Tests for analyze: the pre-registered decision rule (spec section 6) and secondary metrics.

Every row is built by run.make_row, the runner's own row constructor, so the schema the tests
exercise is the journal's. The synthetic studies are built from the spec's wording (section 6
table), not from the implementation: each one names the condition it is meant to meet.
"""
import math
import random
import unittest
from unittest import mock

import analyze
import run

S = "claude-sonnet-5-5"
O = "claude-opus-5-5"
TASKS = [f"org__repo-{i:02d}" for i in range(12)]
REG = 12
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
    kw.setdefault("registered_tasks", REG)
    kw.setdefault("seed", SEED)
    return analyze.decide(rows, kw.pop("model", S), **kw)


# Spec section 6 wording -> data:
# promising: non-inferiority holds AND the 90% interval's upper bound < 1.0. All pass; cs costs
#   0.55-0.65 of bare; bare reps 2% apart (the floor), so the difference is outside it.
PROMISING = dict(ratios=(0.55, 0.65), spread=1.02)
# no demonstrated advantage: non-inferiority holds AND the interval contains 1.0. Ratios swing
#   0.6 / 1.5 (geometric mean 0.95, outside a 1% floor), so the interval straddles 1.0.
NO_ADVANTAGE = dict(ratios=(0.6, 1.5), spread=1.01)


class DecisionRuleTest(unittest.TestCase):
    # -- the nine named tests from the brief --

    def test_promising(self):
        d = decide(study(**PROMISING))
        self.assertEqual(d.outcome, "promising")
        self.assertEqual(d.n_valid_tasks, 12)
        self.assertEqual(d.pass_counts, {"cs": 12, "bare": 12})
        self.assertLess(d.ci90[1], 1.0)
        self.assertAlmostEqual(d.cost_ratio_gmean, math.sqrt(0.55 * 0.65), places=12)
        self.assertAlmostEqual(d.floor, 1.02, places=12)
        self.assertEqual(d.effect, "codescout")

    def test_no_demonstrated_advantage(self):
        d = decide(study(**NO_ADVANTAGE))
        self.assertEqual(d.outcome, "no demonstrated advantage")
        self.assertLessEqual(d.ci90[0], 1.0)
        self.assertGreaterEqual(d.ci90[1], 1.0)
        self.assertGreater(abs(math.log(d.cost_ratio_gmean)), math.log(d.floor))

    def test_worse_by_pass_rate(self):
        # cs passes 10, bare 12: 10 < 12 - 1, so non-inferiority fails, whatever the cost says.
        d = decide(study(cs_fail=(3, 8), **PROMISING))
        self.assertEqual(d.pass_counts, {"cs": 10, "bare": 12})
        self.assertLess(d.ci90[1], 1.0)  # the cost alone would be promising
        self.assertEqual(d.outcome, "worse")

    def test_worse_by_cost(self):
        # The lower bound of the cost interval is above 1.0 (cs costs 1.4-1.6x bare).
        d = decide(study(ratios=(1.4, 1.6), spread=1.02))
        self.assertGreater(d.ci90[0], 1.0)
        self.assertEqual(d.pass_counts, {"cs": 12, "bare": 12})
        self.assertEqual(d.outcome, "worse")

    def test_inconclusive_when_fewer_valid_tasks_than_registered(self):
        d = decide(study(n=11, **PROMISING))
        self.assertEqual(d.n_valid_tasks, 11)
        self.assertLess(d.ci90[1], 1.0)
        self.assertEqual(d.outcome, "inconclusive")

    def test_inconclusive_when_difference_inside_floor(self):
        # bare reps differ 2x on every task (floor 2.0); cs costs 0.8x bare: 0.8 is inside
        # [1/2, 2]. The interval is exactly 0.8 (constant ratio), so without the floor this
        # would be promising.
        d = decide(study(ratios=(0.8,), spread=2.0))
        self.assertAlmostEqual(d.floor, 2.0, places=12)
        self.assertLess(d.ci90[1], 1.0)
        self.assertEqual(d.outcome, "inconclusive")

    def test_invalid_arm_rows_never_enter_the_statistic(self):
        rows = study(**PROMISING)
        # One task's cs run failed the arm-integrity check: charged, never scored. Its row keeps
        # its cost and a failing grade would be meaningless; the task loses its cs measurement.
        task = TASKS[4]
        bad = [r for r in rows if r["taskId"] == task and r["arm"] == "cs"][0]
        rows_with_invalid = [r for r in rows if r is not bad] + [
            row(task, "cs", 0, 500.0, None, outcome="invalid-arm")]
        without_task = [r for r in rows if r["taskId"] != task]
        d1 = decide(rows_with_invalid)
        d2 = decide(without_task)
        self.assertEqual(d1.n_valid_tasks, 11)
        self.assertEqual(d1[:6], d2[:6])
        self.assertEqual(d1.outcome, "inconclusive")  # 11 valid tasks < 12 registered
        self.assertIn(task, d1.excluded)
        # An invalid-arm row on a bare rep is the same: the task has one scored bare rep only.
        bare_bad = [r for r in rows if not (r["taskId"] == task and r["arm"] == "bare"
                                            and r["rep"] == 1)]
        bare_bad.append(row(task, "bare", 1, 0.0001, None, outcome="invalid-arm"))
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
        varied = study(ratios=tuple(0.5 + 0.09 * i for i in range(12)), spread=1.01)
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
        rows = study(ratios=tuple(0.5 + 0.09 * i for i in range(12)), spread=1.01)
        units, _ = analyze.task_units(rows, S, "cs")
        log_ratios = [math.log(u.treat_cost / (sum(u.bare_costs) / 2)) for u in units]
        means = analyze.bootstrap_log_means(log_ratios, SEED)
        self.assertEqual(len(means), 10_000)
        self.assertEqual(means, sorted(means))
        self.assertEqual(decide(rows).ci90, (math.exp(means[499]), math.exp(means[9500])))

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
        without = decide([r for r in base if r["taskId"] != task])
        for outcome in ("plan-limit", "infra-oom", "infra-other", "unknown-spend"):
            with self.subTest(outcome=outcome):
                rows = [r for r in base if not (r["taskId"] == task and r["arm"] == "cs")]
                rows.append(row(task, "cs", 0, None if outcome == "unknown-spend" else 3.0,
                                None, outcome=outcome))
                d = decide(rows)
                self.assertEqual(d[:6], without[:6])
                self.assertIn(task, d.excluded)
        # A run whose latest journal row is still `pending` is not scored either.
        rows = base + [row(TASKS[0], "full", 0, None, None, outcome="pending")]
        self.assertEqual(decide(rows)[:6], decide(base)[:6])

    def test_latest_row_of_a_run_wins(self):
        base = study(**PROMISING)
        task = TASKS[1]
        cs = [r for r in base if r["taskId"] == task and r["arm"] == "cs"][0]
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
        for kw in ({"registered_tasks": 0}, {"registered_tasks": True}, {"seed": "7"},
                   {"seed": False}, {"comparison": ("cs", "full")}):
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


class AmbiguityDecisionsTest(unittest.TestCase):
    """Each ambiguity is resolved toward the reading that makes `promising` harder."""

    def test_single_bare_rep_excludes_the_task(self):
        rows = study(**PROMISING)
        task = TASKS[6]
        rows = [r for r in rows if not (r["taskId"] == task and r["arm"] == "bare"
                                        and r["rep"] == 1)]
        d = decide(rows)
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

    def test_interval_touching_one_contains_one(self):
        out, _ = analyze.rule(12, 12, 12, 12, 0.8, (0.7, 1.0), 1.01)
        self.assertEqual(out, "no demonstrated advantage")
        out, _ = analyze.rule(12, 12, 12, 12, 1.2, (1.0, 1.3), 1.01)
        self.assertEqual(out, "no demonstrated advantage")
        out, _ = analyze.rule(12, 12, 12, 12, 0.8, (0.7, 0.999999), 1.01)
        self.assertEqual(out, "promising")

    def test_difference_on_the_floor_is_inside(self):
        out, _ = analyze.rule(12, 12, 12, 12, 0.5, (0.4, 0.6), 2.0)
        self.assertEqual(out, "inconclusive")
        out, _ = analyze.rule(12, 12, 12, 12, 2.0, (1.5, 2.5), 2.0)
        self.assertEqual(out, "inconclusive")
        out, _ = analyze.rule(12, 12, 12, 12, 0.49, (0.4, 0.6), 2.0)
        self.assertEqual(out, "promising")

    def test_precedence(self):
        # fewer valid tasks beats worse; worse by pass rate beats the floor; the floor beats
        # worse by cost and promising.
        self.assertEqual(analyze.rule(11, 12, 5, 12, 2.0, (1.5, 2.5), 1.0)[0], "inconclusive")
        self.assertEqual(analyze.rule(12, 12, 5, 12, 0.9, (0.8, 0.95), 2.0)[0], "worse")
        self.assertEqual(analyze.rule(12, 12, 12, 12, 1.5, (1.2, 1.8), 2.0)[0], "inconclusive")
        self.assertEqual(analyze.rule(12, 12, 12, 12, 1.5, (1.2, 1.8), 1.1)[0], "worse")
        self.assertEqual(analyze.rule(12, 12, 12, 12, 0.9, (0.8, 1.1), 1.05)[0],
                         "no demonstrated advantage")
        self.assertEqual(analyze.rule(12, 12, 12, 12, 0.9, (0.8, 0.95), 1.05,
                                      instrument_failures=("x",))[0], "invalid instrument")

    def test_more_valid_tasks_than_registered_is_not_inconclusive(self):
        self.assertEqual(decide(study(**PROMISING), registered_tasks=10).outcome, "promising")


class StrataTest(unittest.TestCase):
    def setUp(self):
        self.foreign = TASKS[:6]
        self.private = [f"cs-p{i:02d}" for i in range(6)]
        self.kinds = {**{t: "foreign" for t in self.foreign},
                      **{t: "private" for t in self.private}}
        self.rows = (study(n=6, tasks=self.foreign, ratios=(0.4, 0.45), spread=1.01)
                     + study(n=6, tasks=self.private, ratios=(1.4, 1.6), spread=1.01))

    def test_foreign_private_and_pooled_are_separate(self):
        out = analyze.strata(self.rows, S, self.kinds, {"foreign": 6, "private": 6}, seed=SEED)
        self.assertEqual(set(out), {"foreign", "private", "pooled"})
        self.assertEqual(out["foreign"].outcome, "promising")
        self.assertEqual(out["private"].outcome, "worse")
        self.assertEqual(out["foreign"].n_valid_tasks, 6)
        self.assertEqual(out["private"].n_valid_tasks, 6)
        self.assertEqual(out["pooled"].n_valid_tasks, 12)
        self.assertEqual(out["foreign"].stratum, "foreign")
        self.assertIn("contaminated", " ".join(out["private"].notes))
        self.assertIn("contaminated", " ".join(out["pooled"].notes))
        self.assertEqual(out["foreign"].notes, ())
        # the filter is the same function: decide(task_filter=...) gives the same answer
        same = analyze.decide(self.rows, S, registered_tasks=6, seed=SEED,
                              task_filter=set(self.foreign))
        self.assertEqual(same[:6], out["foreign"][:6])

    def test_registered_count_is_per_stratum(self):
        out = analyze.strata(self.rows, S, self.kinds, {"foreign": 7, "private": 6}, seed=SEED)
        self.assertEqual(out["foreign"].outcome, "inconclusive")
        self.assertEqual(out["pooled"].n_valid_tasks, 12)

    def test_a_task_without_a_kind_raises(self):
        kinds = dict(self.kinds)
        del kinds[self.private[0]]
        with self.assertRaises(ValueError):
            analyze.strata(self.rows, S, kinds, {"foreign": 6, "private": 6}, seed=SEED)
        with self.assertRaises(ValueError):
            analyze.strata(self.rows, S, dict(self.kinds, **{self.foreign[0]: "public"}),
                           {"foreign": 6, "private": 6}, seed=SEED)


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


if __name__ == "__main__":
    unittest.main()
