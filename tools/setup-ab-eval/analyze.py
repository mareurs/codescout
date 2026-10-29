"""Pre-registered analysis for the setup A/B study: spec section 6's decision rule.

    decide(rows, model, comparison=("cs", "bare"), *, registered_tasks, registered_min, seed,
           instrument_failures) -> Decision
    strata(rows, model, registered_tasks, *, registered_min, seed, instrument_failures,
           comparison=("cs", "bare")) -> {"foreign": Decision|None, "private": Decision|None,
                                          "pooled": Decision}
    secondary(rows, gold_files=None, task_filter=None) -> {model: {arm: {...}}}   never gated
    rule(n_valid, registered, treat_passes, bare_passes, gmean, ci90, floor,
         instrument_failures=()) -> (outcome, reasons)                          the bare table
    cost_branch(ci90, floor) -> outcome of step 4
    compiles_all_targets(command) -> bool

Pure computation over Task 8 journal rows (run.py ROW_FIELDS). No docker, network or token.
`registered_tasks`, `registered_min`, `seed` and `instrument_failures` are keyword-only and
REQUIRED.

REGISTERED TASKS (fix round 1, I1). `registered_tasks` is the frozen task list as a mapping
{task id: "foreign" | "private"} (the bundle record's `kind`). Any main-stage row whose task is
not in it raises MalformedRow: an unregistered task can never fill the count. A development task
may not be registered. The kinds decide the stratum ("foreign", "private", or "pooled" when both
occur) and its note (Task 7 Ruling A).

REGISTERED MINIMUM (final review I-2, operator decision 2026-10-29). `registered_min` is "the
registered number of valid tasks" of spec section 6, a frozen number separate from the task
set: REGISTERED_MIN = 10 valid tasks of the 12 registered, per model. Fewer valid tasks than
`registered_min` gives "inconclusive"; 1 <= registered_min <= len(registered_tasks). A stratum
is held to the same share of its own tasks, rounded up: stratum_min(registered_min, k, n) =
ceil(registered_min * k / n) for a stratum of k of the n registered tasks (10 of 12 -> 5 of 6).
Pilot-only data (6 of the 12 tasks) therefore reads "inconclusive" in every stratum.

INSTRUMENT (fix round 1, I5). `instrument_failures` is required: the study-level failures of
the leakage gates, controls and arm-integrity checks. An explicit () means "the gates were run
and passed"; any entry gives "invalid instrument".

SPEC SECTION 6, VERBATIM (per model, using the cs - bare comparison):
  promising                  pass-rate non-inferiority holds AND the upper bound of the cost
                             ratio's 90% interval is below 1.0
  no demonstrated advantage  non-inferiority holds AND the interval contains 1.0
  worse                      non-inferiority fails, OR the lower bound of the cost interval is
                             above 1.0
  inconclusive               fewer than the registered number of valid tasks, OR the cost
                             difference is inside the variance floor
  invalid instrument         any leakage gate, control or arm-integrity check fails at the study
                             level
  "full - bare is reported under the same rule as 'the setup', never as 'codescout'."
Section 5: non-inferiority = "the cs arm passes at least as many tasks as bare minus one"; cost =
"the paired ratio cs / bare per task. Its geometric mean is reported with a 90% paired-bootstrap
interval over tasks"; floor = "bare runs twice on every task. The floor is the spread between
those two runs, on each metric."

ORDER (coordinator ruling, fix round 1, I3 + I4; binding; the rows of the table overlap):
  1. any instrument failure                             -> invalid instrument
  2. fewer valid tasks than registered                  -> inconclusive
  3. non-inferiority fails                              -> worse (the floor never masks it)
  4. cost, F = floor >= 1 (cost_branch):
       low > F                                          -> worse
       high < 1/F                                       -> promising
       low <= 1.0 <= high                               -> no demonstrated advantage
       otherwise (1/F <= high < 1.0 or 1.0 < low <= F)  -> inconclusive (cost difference inside
                                                           the variance floor)
  Ties go to the side that claims less: low == F is not worse, high == 1/F is not promising, an
  interval touching 1.0 contains it. The three bounded cases exclude each other (low > F >= 1
  means 1.0 is outside and high > 1/F; high < 1/F <= 1 means 1.0 is outside), and the rest is
  inconclusive, so step 4 is exhaustive; with F == 1 the inconclusive branch is empty.
  `reasons` lists EVERY section 6 condition that holds (not only the deciding one).

WHAT ENTERS THE STATISTIC. Rows are checked (check_row) and the journal is checked (every runId
has at most one final row and nothing after it: a second final row is a re-run). Then the latest
row of each runId, of stage "main", outcome exactly `scored`; never `dev-scored`, `invalid-arm`
(charged, never scored), `plan-limit`, `infra-oom`, `infra-other`, `unknown-spend` or `pending`.
A task that run.task_validity marks invalid for the model (an effective-effort mismatch or
unknown effort across ANY of its graded arms) is excluded. A valid task has two scored `bare`
reps and one scored treatment run. Per task, the treatment passes when its run passes, `bare`
passes when the mean of its two reps' pass is >= 0.5, and the cost ratio is treatment cost /
mean of the two bare costs.

FAIL CLOSED. check_row raises MalformedRow (a ValueError) for: not exactly ROW_FIELDS; an
unknown outcome, model or arm; a rep that is not an int or, on a main row, is outside the Stage 3
plan run.STAGE3_REPS (bare 0-1, cs 0, full 0: any other rep is a re-run, spec sections 4 and 7);
runId != run.run_id(task, model, arm, rep); stage != run.stage_of(task); and for a graded row a
grade without a bool `pass` or a cost that is not finite and > 0. Nothing is dropped silently.

DECISIONS (resolved toward a harder "promising"; the report cites the spec text of each):
  D1 floor     F = GM over the valid tasks i of max(bare0_i, bare1_i) / min(bare0_i, bare1_i)
                 = exp(mean_i |ln(bare0_i / bare1_i)|),
               the costs of the two bare reps of task i; not divided by sqrt(n). Operator
               decision 2026-10-29: "the LARGER of the two bare costs' geometric-mean readings"
               (the cautious one). The other reading, GM_i(bare0) / GM_i(bare1) taken larger over
               smaller = exp(|mean_i ln(bare0_i / bare1_i)|), is never larger (triangle
               inequality) and is 1.0 when the per-task spreads cancel; it is NOT used.
               test_analyze pins this formula with spreads that differ and change direction.
  D2 one bare  a task with fewer than two scored bare reps, or no scored treatment run, is not a
               valid task (it lowers n_valid_tasks toward "inconclusive").
  D3 ties      see ORDER; a bare mean pass of exactly 0.5 is a pass (the brief's ">= 0.5").
  D4 interval  percentile bootstrap, 10,000 resamples of n whole tasks with replacement from
               random.Random(seed), tasks in sorted task-id order; bounds are the order
               statistics floor(0.05 (B-1)) and ceil(0.95 (B-1)) of the sorted log means.
  D5 order     replaced by the coordinator's ORDER above.
  D6 label     `outcome` is one of the five spec labels; `effect` is "codescout" for cs - bare
               and "the setup" for full - bare, `claim` = "<effect>: <outcome>". Other
               comparisons raise: full - cs has a single cs run, so no floor.
  D8 strata    see REGISTERED TASKS; strata() splits the mapping by kind and filters the rows.
  D9 secondary over the statistic's population: scored, main, effort-valid rows (task_filter
               optional). See secondary() and compiles_all_targets().

Stdlib only.
"""
import math
import random
import re
import shlex
import statistics
from types import MappingProxyType
from typing import NamedTuple

import run

RESAMPLES = 10_000
# Final review I-2, operator decision 2026-10-29: at least 10 of the 12 registered tasks must be
# valid per model, or the verdict is "inconclusive". Frozen before any scored run.
REGISTERED_MIN = 10
LABELS = ("promising", "no demonstrated advantage", "worse", "inconclusive", "invalid instrument")
EFFECTS = {("cs", "bare"): "codescout", ("full", "bare"): "the setup"}
ARMS = run.arms.ARMS
KINDS = ("foreign", "private")
PLANNED_REPS = {arm: range(n) for arm, n in run.STAGE3_REPS}
SECONDARY_MEDIANS = ("inputTokens", "outputTokens", "numTurns", "wallMs")
PRIVATE_NOTE = ("the private stratum is functionally contaminated for cs/full: the frozen "
                "codescout binary post-dates every private fix (Task 7 Ruling A, "
                "binary_postdates_fix); a claim must not rest on it")
POOLED_NOTE = ("pooled includes private tasks, which are functionally contaminated for cs/full "
               "(Task 7 Ruling A); cite the foreign stratum for a claim")


class MalformedRow(ValueError):
    """A journal row that cannot be trusted; it is never silently dropped from the statistic."""


class Decision(NamedTuple):
    outcome: str                  # one of LABELS
    pass_counts: dict             # {treatment arm: tasks passed, "bare": tasks passed}
    cost_ratio_gmean: float       # None when no task is valid
    ci90: tuple                   # (low, high) or None
    floor: float                  # geometric mean of max/min of the two bare costs, or None
    n_valid_tasks: int
    effect: str = "codescout"     # "codescout" (cs - bare) or "the setup" (full - bare)
    claim: str = ""               # "<effect>: <outcome>"
    reasons: tuple = ()           # every section 6 condition that holds
    excluded: dict = MappingProxyType({})   # {task id: why it is not a valid task}
    stratum: str = "pooled"       # "foreign", "private" or "pooled", from the registered kinds
    notes: tuple = ()


class TaskUnit(NamedTuple):
    task_id: str
    bare_costs: tuple
    bare_passes: tuple
    treat_cost: float
    treat_pass: bool


# --- rows ----------------------------------------------------------------------------------------

def _finite_number(v):
    return isinstance(v, (int, float)) and not isinstance(v, bool) and math.isfinite(v)


def check_row(row):
    """Raise MalformedRow unless `row` is a well-formed journal row (see FAIL CLOSED)."""
    if not isinstance(row, dict):
        raise MalformedRow(f"not a row: {row!r}")
    missing = [f for f in run.ROW_FIELDS if f not in row]
    extra = sorted(set(row) - set(run.ROW_FIELDS))
    rid = row.get("runId")
    if missing or extra:
        raise MalformedRow(f"{rid}: missing fields {missing}, unknown fields {extra}")
    if row["outcome"] not in run.OUTCOMES + (run.PENDING,):
        raise MalformedRow(f"{rid}: unknown outcome {row['outcome']!r}")
    if row["model"] not in run.MODELS:
        raise MalformedRow(f"{rid}: unknown model {row['model']!r}")
    if row["arm"] not in ARMS:
        raise MalformedRow(f"{rid}: unknown arm {row['arm']!r}")
    if isinstance(row["rep"], bool) or not isinstance(row["rep"], int) or row["rep"] < 0:
        raise MalformedRow(f"{rid}: rep must be an int >= 0, got {row['rep']!r}")
    if not isinstance(row["taskId"], str):
        raise MalformedRow(f"{rid}: taskId must be a string")
    expect = run.run_id((row["taskId"], row["model"], row["arm"], row["rep"]))
    if rid != expect:
        raise MalformedRow(f"runId {rid!r} does not match its fields ({expect!r})")
    if row["stage"] != run.stage_of(row["taskId"]):
        raise MalformedRow(f"{rid}: stage {row['stage']!r} but the task is "
                           f"{run.stage_of(row['taskId'])!r}")
    if row["stage"] == "main" and row["rep"] not in PLANNED_REPS[row["arm"]]:
        raise MalformedRow(f"{rid}: rep {row['rep']} of {row['arm']} is outside the Stage 3 plan "
                           f"({list(PLANNED_REPS[row['arm']])}): a re-run, and spec sections 4 "
                           f"and 7 forbid one")
    if row["outcome"] == "scored" and row["stage"] == "dev":
        raise MalformedRow(f"{rid}: a development task is never `scored`")
    if row["outcome"] == "dev-scored" and row["stage"] != "dev":
        raise MalformedRow(f"{rid}: dev-scored on a main task")
    if row["outcome"] in run.GRADED:
        grade = row["grade"]
        if not isinstance(grade, dict) or type(grade.get("pass")) is not bool:
            raise MalformedRow(f"{rid}: a graded row needs a grade with a bool `pass`, got "
                               f"{grade!r}")
        if not _finite_number(row["costUsd"]) or row["costUsd"] <= 0:
            raise MalformedRow(f"{rid}: a graded row needs a finite cost > 0, got "
                               f"{row['costUsd']!r}")
    if row["capHit"] is not None and type(row["capHit"]) is not bool:
        raise MalformedRow(f"{rid}: capHit must be a bool or None, got {row['capHit']!r}")
    if row["stage"] == "main" and row["outcome"] in run.GRADED and row["capHit"] is None:
        raise MalformedRow(f"{rid}: a graded main row has a result record, so capHit is a bool")
    return row


def _latest(rows):
    """Check every row and the journal, then {runId: latest row} (run.latest_rows).

    A runId's rows are `pending` rows followed by at most one final row; a row after a final row
    is a re-run of that spec (spec sections 4 and 7: never re-run) and raises."""
    final = {}
    for r in rows:
        check_row(r)
        rid = r["runId"]
        if rid in final:
            raise MalformedRow(f"{rid}: a {r['outcome']!r} row after its final "
                               f"{final[rid]!r} row: a re-run, and spec sections 4 and 7 forbid "
                               f"one")
        if r["outcome"] != run.PENDING:
            final[rid] = r["outcome"]
    return run.latest_rows(rows)


def check_registered(registered_tasks):
    """The frozen task list: a non-empty mapping {main task id: "foreign" | "private"}."""
    if not isinstance(registered_tasks, dict) or not registered_tasks:
        raise ValueError(f"registered_tasks must be a non-empty mapping {{task id: kind}}, got "
                         f"{registered_tasks!r}")
    for task, kind in registered_tasks.items():
        if not isinstance(task, str) or not task:
            raise ValueError(f"registered task id {task!r} is not a string")
        if run.stage_of(task) != "main":
            raise ValueError(f"{task}: a development task is never registered")
        if kind not in KINDS:
            raise ValueError(f"{task}: kind {kind!r} is not one of {KINDS}")
    return registered_tasks


def _check_registered_rows(latest, registered_tasks):
    for r in latest.values():
        if r["stage"] == "main" and r["taskId"] not in registered_tasks:
            raise MalformedRow(f"{r['runId']}: task {r['taskId']!r} is not registered; an "
                               f"unregistered task never enters the statistic")


def _population(latest, task_filter=None):
    """(main rows of effort-valid tasks, {(task, model): why excluded})."""
    validity = run.task_validity(list(latest.values()))
    kept, invalid = [], {}
    for r in latest.values():
        if r["stage"] != "main":
            continue
        if task_filter is not None and r["taskId"] not in task_filter:
            continue
        v = validity.get(f"{r['taskId']}|{r['model']}")
        if v is not None and not v["valid"]:
            invalid[(r["taskId"], r["model"])] = (f"effort mismatch or unknown effort: "
                                                  f"{v['efforts']}")
            continue
        kept.append(r)
    return kept, invalid


def task_units(rows, model, treatment, task_filter=None):
    """([TaskUnit] in task-id order, {task: why it is not valid}) for one model and treatment."""
    kept, invalid = _population(_latest(rows), task_filter)
    excluded = {t: why for (t, m), why in invalid.items() if m == model}
    by_task = {}
    for r in kept:
        if r["model"] == model:
            by_task.setdefault(r["taskId"], []).append(r)
    units = []
    for task in sorted(by_task):
        scored = [r for r in by_task[task] if r["outcome"] == "scored"]
        bare = sorted((r for r in scored if r["arm"] == "bare"), key=lambda r: r["rep"])
        treat = [r for r in scored if r["arm"] == treatment]
        if len(bare) > 2 or len(treat) > 1:  # unreachable after check_row; kept as a guard
            raise MalformedRow(f"{task}|{model}: more scored runs than the plan")
        if len(bare) < 2:
            excluded[task] = f"{len(bare)} scored bare rep(s); the floor needs 2 (D2)"
            continue
        if not treat:
            excluded[task] = f"no scored {treatment} run"
            continue
        units.append(TaskUnit(task, tuple(r["costUsd"] for r in bare),
                              tuple(r["grade"]["pass"] for r in bare),
                              treat[0]["costUsd"], treat[0]["grade"]["pass"]))
    return units, excluded


# --- statistics ----------------------------------------------------------------------------------

def bootstrap_log_means(log_ratios, seed, resamples=RESAMPLES):
    """Sorted log geometric means of `resamples` draws of n whole tasks with replacement from
    random.Random(seed) (D4). Pairing is kept: each draw takes one task's log ratio whole."""
    n = len(log_ratios)
    rng = random.Random(seed)
    stats = []
    for _ in range(resamples):
        stats.append(math.fsum(log_ratios[rng.randrange(n)] for _ in range(n)) / n)
    stats.sort()
    return stats


def interval90(sorted_logs):
    """(low, high) on the ratio scale: order statistics floor(0.05 (B-1)) and ceil(0.95 (B-1))
    of the sorted log means, i.e. rounded outward (D4)."""
    b = len(sorted_logs)
    lo = (b - 1) // 20                  # floor(0.05 (B - 1))
    hi = -(-(b - 1) * 19 // 20)         # ceil(0.95 (B - 1))
    return (math.exp(sorted_logs[lo]), math.exp(sorted_logs[hi]))


def bootstrap_ci(log_ratios, seed, resamples=RESAMPLES):
    """90% percentile paired-bootstrap interval of the geometric mean over tasks."""
    return interval90(bootstrap_log_means(log_ratios, seed, resamples))


def _check_floor(floor):
    if (isinstance(floor, bool) or not isinstance(floor, (int, float))
            or not math.isfinite(floor) or floor < 1.0):
        raise ValueError(f"floor must be a finite number >= 1, got {floor!r}")


def cost_branch(ci90, floor):
    """Step 4 of ORDER: the cost outcome of a 90% interval against the floor F >= 1 (a floor that
    is not finite or is below 1 raises ValueError)."""
    _check_floor(floor)
    low, high = ci90
    if low > floor:
        return "worse"
    if high < 1.0 / floor:
        return "promising"
    if low <= 1.0 <= high:
        return "no demonstrated advantage"
    return "inconclusive"


def rule(n_valid, registered, treat_passes, bare_passes, gmean, ci90, floor,
         instrument_failures=()):
    """(outcome, reasons): spec section 6 in the coordinator's ORDER; `reasons` lists every
    section 6 condition that holds. ci90 and floor may be None only when n_valid is 0. A
    registered count below 1, or (with n_valid > 0) a floor that is not finite or is below 1,
    raises ValueError."""
    _check_int(registered, "registered", 1)
    if n_valid:
        _check_floor(floor)
    held = [f"invalid instrument: {f}" for f in instrument_failures]
    few = n_valid < registered
    if few:
        held.append(f"fewer than the registered number of valid tasks ({n_valid} < "
                    f"{registered})")
    noninferior = treat_passes >= bare_passes - 1
    held.append(f"non-inferiority {'holds' if noninferior else 'fails'} ({treat_passes} vs "
                f"{bare_passes} - 1)")
    branch = None
    if n_valid:
        low, high = ci90
        held.append(f"cost ratio geometric mean {gmean:.4g}, 90% interval [{low:.4g}, "
                    f"{high:.4g}], floor F {floor:.4g}")
        if high < 1.0:
            held.append("the upper bound of the cost ratio's 90% interval is below 1.0")
        if low <= 1.0 <= high:
            held.append("the interval contains 1.0")
        if low > 1.0:
            held.append("the lower bound of the cost interval is above 1.0")
        branch = cost_branch(ci90, floor)
        if low > floor:
            held.append(f"the lower bound is above the floor F ({floor:.4g})")
        if high < 1.0 / floor:
            held.append(f"the upper bound is below 1/F ({1 / floor:.4g})")
        if branch == "inconclusive":  # the interval misses 1.0 and is not beyond the floor
            held.append(f"the cost difference is inside the variance floor [{1 / floor:.4g}, "
                        f"{floor:.4g}]")
    if instrument_failures:
        outcome = "invalid instrument"
    elif few:
        outcome = "inconclusive"
    elif not noninferior:
        outcome = "worse"
    else:
        outcome = branch
    return outcome, tuple(held)


def _check_int(value, name, minimum):
    if isinstance(value, bool) or not isinstance(value, int) or value < minimum:
        raise ValueError(f"{name} must be an int >= {minimum}, got {value!r}")


def _check_instrument(instrument_failures):
    if (not isinstance(instrument_failures, (tuple, list))
            or not all(isinstance(f, str) and f for f in instrument_failures)):
        raise ValueError(f"instrument_failures must be a tuple of non-empty strings (() = every "
                         f"gate ran and passed), got {instrument_failures!r}")


def _stratum(registered_tasks):
    kinds = set(registered_tasks.values())
    if kinds == {"foreign"}:
        return "foreign", ()
    if kinds == {"private"}:
        return "private", (PRIVATE_NOTE,)
    return "pooled", (POOLED_NOTE,)


def floor_f(units):
    """D1: F = GM over tasks of max(bare0, bare1) / min(bare0, bare1) = exp(mean |ln(b0 / b1)|)."""
    return math.exp(math.fsum(abs(math.log(u.bare_costs[0] / u.bare_costs[1])) for u in units)
                    / len(units))


def stratum_min(registered_min, k, n):
    """The minimum for a stratum of k of the n registered tasks: the same share, rounded up
    (ceil(registered_min * k / n), exact integer arithmetic)."""
    _check_int(n, "n", 1)
    _check_int(k, "k", 1)
    _check_int(registered_min, "registered_min", 1)
    if k > n or registered_min > n:
        raise ValueError(f"stratum of {k} of {n} tasks with a minimum of {registered_min}")
    return -(-registered_min * k // n)


def _check_registered_min(registered_min, registered_tasks):
    _check_int(registered_min, "registered_min", 1)
    if registered_min > len(registered_tasks):
        raise ValueError(f"registered_min {registered_min} exceeds the {len(registered_tasks)} "
                         f"registered tasks")


def decide(rows, model, comparison=("cs", "bare"), *, registered_tasks, registered_min, seed,
           instrument_failures):
    """Spec section 6 for one model and one comparison (see the module docstring)."""
    comparison = tuple(comparison)
    if comparison not in EFFECTS:
        raise ValueError(f"comparison {comparison!r} is not one of {sorted(EFFECTS)}")
    if model not in run.MODELS:
        raise ValueError(f"model {model!r} is not one of {run.MODELS}")
    check_registered(registered_tasks)
    _check_registered_min(registered_min, registered_tasks)
    _check_int(seed, "seed", 0)
    _check_instrument(instrument_failures)
    _check_registered_rows(_latest(rows), registered_tasks)
    treatment = comparison[0]
    units, excluded = task_units(rows, model, treatment)
    n = len(units)
    counts = {treatment: sum(u.treat_pass for u in units),
              "bare": sum(sum(u.bare_passes) / 2 >= 0.5 for u in units)}
    gmean = ci = floor = None
    if n:
        logs = [math.log(u.treat_cost / (sum(u.bare_costs) / 2)) for u in units]
        gmean = math.exp(math.fsum(logs) / n)
        floor = floor_f(units)
        ci = bootstrap_ci(logs, seed)
    outcome, reasons = rule(n, registered_min, counts[treatment], counts["bare"], gmean,
                            ci, floor, tuple(instrument_failures))
    effect = EFFECTS[comparison]
    stratum, notes = _stratum(registered_tasks)
    return Decision(outcome, counts, gmean, ci, floor, n, effect, f"{effect}: {outcome}",
                    reasons, dict(sorted(excluded.items())), stratum, notes)


def strata(rows, model, registered_tasks, *, registered_min, seed, instrument_failures,
           comparison=("cs", "bare")):
    """Foreign-only, private-only and pooled Decisions (spec section 2: "reported separately and
    also pooled"; Task 7 Ruling A). Each stratum is decide() over its part of the registered
    mapping and the rows of those tasks, with stratum_min() of `registered_min`; a stratum with
    no registered task is None. Every row is checked against the WHOLE mapping first, so an
    unregistered task raises here too. Final review I-3 ruling: there is no pooled headline; a
    headline claim needs the foreign and private strata to agree (analyze_study.headline)."""
    check_registered(registered_tasks)
    _check_registered_min(registered_min, registered_tasks)
    _check_registered_rows(_latest(rows), registered_tasks)
    common = dict(comparison=comparison, seed=seed, instrument_failures=instrument_failures)
    out = {}
    n = len(registered_tasks)
    for kind in KINDS:
        part = {t: k for t, k in registered_tasks.items() if k == kind}
        out[kind] = (decide([r for r in rows if r["taskId"] in part], model,
                            registered_tasks=part,
                            registered_min=stratum_min(registered_min, len(part), n), **common)
                     if part else None)
    out["pooled"] = decide(rows, model, registered_tasks=registered_tasks,
                           registered_min=registered_min, **common)
    return out


# --- secondary (reported, never gated) -----------------------------------------------------------

_SEPARATORS = re.compile(r"&&|\|\||[;|\n]")
_ASSIGN = re.compile(r"[A-Za-z_][A-Za-z0-9_]*=.*")
_WRAPPERS = {"time", "nice", "env", "command", "exec", "nohup"}
_ALL_TARGET_FLAGS = {"--all-targets", "--tests"}
_TARGET_SELECTORS = {"--lib", "--bin", "--bins", "--test", "--example", "--examples", "--bench",
                     "--benches", "--doc"}
_TEST_SUBCOMMANDS = {"test", "t"}


def _tokens(segment):
    try:
        return shlex.split(segment)
    except ValueError:
        return segment.split()


def compiles_all_targets(command):
    """True when a Bash command compiled all targets.

    The command is split into segments at `&&`, `||`, `;`, `|` and newlines (a separator inside
    quotes splits too: a known approximation). In each segment, leading VAR=value words, the
    wrappers time/nice/env/command/exec/nohup and `timeout [-opts] DURATION` are skipped; the next
    word must be `cargo` (or a path ending in /cargo). Arguments after a bare `--` belong to the
    test harness and are ignored. The subcommand is the first argument not starting with `-` or
    `+`. The segment counts when:
      - any cargo subcommand carries `--all-targets` or `--tests` (also `--flag=...`), or
      - the subcommand is exactly `test` (or cargo's built-in alias `t`) - a whole word, so
        `cargo test-foo` and `cargo testx` do not count - and no target selector (`--lib`,
        `--bin`, `--bins`, `--test`, `--example(s)`, `--bench(es)`, `--doc`) narrows it.
    Known misses (deferred, secondary only): `bash -c 'cargo test'`, `env -u X cargo test`
    (the -u value is read as the command), `cargo --color always test` (the subcommand is read
    as `always`), and `cargo nextest run` without --tests/--all-targets."""
    for segment in _SEPARATORS.split(command or ""):
        toks = _tokens(segment)
        i = 0
        while i < len(toks):
            t = toks[i]
            if _ASSIGN.fullmatch(t) or t in _WRAPPERS:
                i += 1
            elif t == "timeout":
                i += 1
                while i < len(toks) and toks[i].startswith("-"):
                    i += 1
                i += 1  # the duration
            else:
                break
        if i >= len(toks) or toks[i].rsplit("/", 1)[-1] != "cargo":
            continue
        args = toks[i + 1:]
        if "--" in args:
            args = args[:args.index("--")]
        sub = next((j for j, a in enumerate(args) if not a.startswith(("-", "+"))), None)
        if sub is None:
            continue
        flags = {a.split("=", 1)[0] for a in args[sub + 1:]}
        if flags & _ALL_TARGET_FLAGS:
            return True
        if args[sub] in _TEST_SUBCOMMANDS and not flags & _TARGET_SELECTORS:
            return True
    return False


def _nonneg_int(v):
    return isinstance(v, int) and not isinstance(v, bool) and v >= 0


def _summary(rs, gold_files):
    out = {"n_runs": len(rs), "medians": {}, "missing": {}}
    for key in SECONDARY_MEDIANS:
        values = []
        for r in rs:
            v = r[key]
            if v is None:
                continue
            if not _finite_number(v) or v < 0:
                raise MalformedRow(f"{r['runId']}: {key} must be a number >= 0, got {v!r}")
            values.append(v)
        out["medians"][key] = statistics.median(values) if values else None
        out["missing"][key] = len(rs) - len(values)
    all_targets = cs_calls = native_calls = outside = 0
    for r in rs:
        cmds, edited, integ = r["bashCommands"], r["editedFiles"], r["integrity"]
        if not isinstance(cmds, list) or not all(isinstance(c, str) for c in cmds):
            raise MalformedRow(f"{r['runId']}: bashCommands must be a list of strings")
        if not isinstance(edited, list) or not all(isinstance(f, str) for f in edited):
            raise MalformedRow(f"{r['runId']}: editedFiles must be a list of strings")
        if (not isinstance(integ, dict) or not _nonneg_int(integ.get("codescout_calls"))
                or not _nonneg_int(integ.get("native_calls"))):
            raise MalformedRow(f"{r['runId']}: integrity needs int codescout_calls and "
                               f"native_calls")
        all_targets += any(compiles_all_targets(c) for c in cmds)
        cs_calls += integ["codescout_calls"]
        native_calls += integ["native_calls"]
        if gold_files is not None:
            if r["taskId"] not in gold_files:
                raise ValueError(f"{r['taskId']}: no gold files given")
            outside += len(set(edited) - set(gold_files[r["taskId"]]))
    n = len(rs)
    out["cap_hits"] = sum(r["capHit"] is True for r in rs)
    out["all_targets_share"] = all_targets / n if n else None
    total_calls = cs_calls + native_calls
    out["codescout_share"] = cs_calls / total_calls if total_calls else None
    out["edits_outside_gold"] = (None if gold_files is None or not n
                                 else {"total": outside, "per_run_mean": outside / n})
    return out


def secondary(rows, gold_files=None, task_filter=None):
    """Reported, never gated (spec section 5). {model: {arm: summary}} for every model and arm,
    over the statistic's population (D9): latest rows, outcome `scored`, effort-valid tasks.

    summary: n_runs; medians of inputTokens, outputTokens, numTurns, wallMs (None values are
    skipped and counted under `missing`; an empty group gives None); cap_hits = runs whose
    result record says `--max-budget-usd` ended them (capHit, final review I-6: such a run is
    scored, most likely as a fail, with its cost censored near the per-call cap; see also
    cap_hits()); all_targets_share = share
    of runs with at least one Bash command for which compiles_all_targets() is true;
    codescout_share = sum(codescout_calls) / sum(codescout_calls + native_calls) from the rows'
    integrity blocks (None when both are 0); edits_outside_gold = {total, per_run_mean} of the
    run's editedFiles not among gold_files[taskId] - rows do not carry the gold patch's files, so
    the caller passes gold_files: {task id: [path]}; None when not given (a task missing from a
    given map raises).

    Limit: bashCommands holds native Bash calls only; a shell run through codescout's
    run_command (cs, and full whose settings deny Bash) is not in all_targets_share."""
    kept, _ = _population(_latest(rows), task_filter)
    groups = {m: {a: [] for a in ARMS} for m in run.MODELS}
    for r in kept:
        if r["outcome"] == "scored":
            groups[r["model"]][r["arm"]].append(r)
    return {m: {a: _summary(rs, gold_files) for a, rs in arms_.items()}
            for m, arms_ in groups.items()}
def cap_hits(rows):
    """Final review I-6, reported, never gated: {model: {arm: {"runs", "cap_hit", "unknown"}}}
    over the latest FINAL row of every main run (any outcome: a cap hit can also be invalid-arm);
    `unknown` counts rows with capHit None (no result record)."""
    out = {m: {a: {"runs": 0, "cap_hit": 0, "unknown": 0} for a in ARMS} for m in run.MODELS}
    for r in _latest(rows).values():
        if r["stage"] != "main" or r["outcome"] == run.PENDING:
            continue
        g = out[r["model"]][r["arm"]]
        g["runs"] += 1
        g["cap_hit"] += r["capHit"] is True
        g["unknown"] += r["capHit"] is None
    return out
