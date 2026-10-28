"""Pre-registered analysis for the setup A/B study: spec section 6's decision rule.

    decide(rows, model, comparison=("cs", "bare"), *, registered_tasks, seed, task_filter=None,
           instrument_failures=(), stratum="pooled") -> Decision
    strata(rows, model, kinds, registered, *, seed, comparison=("cs", "bare"),
           instrument_failures=()) -> {"foreign": Decision, "private": Decision, "pooled": Decision}
    secondary(rows, gold_files=None, task_filter=None) -> {model: {arm: {...}}}   never gated
    rule(n_valid, registered, treat_passes, bare_passes, gmean, ci90, floor,
         instrument_failures=()) -> (outcome, reasons)                           the bare table
    compiles_all_targets(command) -> bool

Pure computation over Task 8 journal rows (run.py ROW_FIELDS). No docker, network or token.
`registered_tasks` and `seed` are keyword-only (the brief lists them after a defaulted argument).

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

WHAT ENTERS THE STATISTIC. The latest journal row of each runId (run.latest_rows: a pending row
followed by its final row counts once). Of those, only rows with outcome exactly `scored` (stage
"main"); never `dev-scored`, `invalid-arm` (charged, never scored), `plan-limit`, `infra-oom`,
`infra-other`, `unknown-spend` or `pending`. A task whose (task, model) run.task_validity marks
invalid (an effective-effort mismatch or unknown effort across ANY of its graded arms) is
excluded. The unit is the task: a valid task has exactly two scored `bare` reps and one scored
treatment run; per task, the treatment passes when its run passes, `bare` passes when the mean of
its two reps' pass is >= 0.5, and the cost ratio is treatment cost / mean of the two bare costs.

FAIL CLOSED. Every row is checked (check_row) before anything is counted: exact ROW_FIELDS, a
known outcome, model and arm, an int rep, runId == run.run_id(task, model, arm, rep), stage ==
run.stage_of(task) (a development task is never `scored`), and for a graded row a dict grade
with a bool `pass` and a finite cost > 0. More scored reps than the plan (bare x2, treatment x1)
raise. A bad row raises MalformedRow (a ValueError); it is never dropped.

DECISIONS (each ambiguity resolved toward the reading that makes "promising" harder; the report
task-9-report.md cites the spec text each one resolves):
  D1 floor     per valid task the bare spread is max(b0, b1) / min(b0, b1) (cost); the floor F is
               their geometric mean over the valid tasks (the cost statistic's own averaging). It
               is not divided by sqrt(n). "Inside the floor" means 1/F <= gmean <= F, ties inside
               (with a 1e-12 tolerance on the log scale).
  D2 one bare  a task with fewer than two scored bare reps has no floor and is not a valid task
               (it lowers n_valid_tasks toward "inconclusive"); likewise a task with no scored
               treatment run.
  D3 ties      "below 1.0" and "above 1.0" are strict; an interval that touches 1.0 contains it.
               A bare mean pass of exactly 0.5 is a pass (the brief's ">= 0.5").
  D4 interval  percentile bootstrap, 10,000 resamples of n whole tasks with replacement from
               random.Random(seed), tasks in sorted task-id order (row order does not matter);
               bounds are the order statistics floor(0.05 (B-1)) and ceil(0.95 (B-1)) of the
               sorted log means, rounded outward.
  D5 order     invalid instrument > inconclusive (fewer valid tasks) > worse (non-inferiority
               fails) > inconclusive (inside the floor) > worse (cost lower bound > 1.0) >
               promising > no demonstrated advantage. More valid tasks than registered is fine.
  D6 label     `outcome` is always one of the five spec labels; `effect` is "codescout" for
               cs - bare and "the setup" for full - bare, and `claim` = "<effect>: <outcome>".
               Other comparisons raise: full - cs has a single cs run, so no floor.
  D7 instrument  the gates are not in the rows; the caller passes their study-level failures as
               `instrument_failures` (any non-empty -> "invalid instrument").
  D8 strata    task kinds ("foreign" / "private") come from the caller (the bundle record's
               `kind`); rows do not carry them. Each stratum has its own registered count; the
               private and pooled Decisions carry a note that the private stratum is functionally
               contaminated for cs/full (Task 7 Ruling A: binary_postdates_fix).
  D9 secondary over the statistic's population: scored, main, effort-valid rows (task_filter
               optional). See secondary() and compiles_all_targets() for the exact definitions.

Stdlib only.
"""
import math
import random
import re
import shlex
import statistics
from typing import NamedTuple

import run

RESAMPLES = 10_000
LABELS = ("promising", "no demonstrated advantage", "worse", "inconclusive", "invalid instrument")
EFFECTS = {("cs", "bare"): "codescout", ("full", "bare"): "the setup"}
ARMS = run.arms.ARMS
KINDS = ("foreign", "private")
SECONDARY_MEDIANS = ("inputTokens", "outputTokens", "numTurns", "wallMs")
_TIE = 1e-12
PRIVATE_NOTE = ("the private stratum is functionally contaminated for cs/full: the frozen "
                "codescout binary post-dates every private fix (Task 7 Ruling A, "
                "binary_postdates_fix); a claim must not rest on it")
POOLED_NOTE = ("pooled includes the private stratum, which is functionally contaminated for "
               "cs/full (Task 7 Ruling A); cite the foreign stratum for a claim")


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
    reasons: tuple = ()
    excluded: dict = None         # {task id: why it is not a valid task}
    stratum: str = "pooled"
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
    """Raise MalformedRow unless `row` is a well-formed journal row (see the module docstring)."""
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
    return row


def _latest(rows):
    """Check every row, then {runId: latest row} (run.latest_rows)."""
    for r in rows:
        check_row(r)
    return run.latest_rows(rows)


def _population(rows, task_filter=None):
    """(scored main rows of effort-valid tasks, {(task, model): why excluded}, latest rows)."""
    latest = _latest(rows)
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
    kept, invalid = _population(rows, task_filter)
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
        if len(bare) > 2:
            raise MalformedRow(f"{task}|{model}: {len(bare)} scored bare reps; the plan has 2")
        if len(treat) > 1:
            raise MalformedRow(f"{task}|{model}: {len(treat)} scored {treatment} runs; the plan "
                               f"has 1")
        if len(bare) < 2:
            excluded[task] = (f"{len(bare)} scored bare rep(s); the floor needs 2 (D2)")
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


def rule(n_valid, registered, treat_passes, bare_passes, gmean, ci90, floor,
         instrument_failures=()):
    """(outcome, reasons): spec section 6's table in the order D5."""
    if instrument_failures:
        return "invalid instrument", tuple(f"instrument: {f}" for f in instrument_failures)
    if n_valid < registered:
        return "inconclusive", (f"{n_valid} valid tasks < {registered} registered",)
    if treat_passes < bare_passes - 1:
        return "worse", (f"non-inferiority fails: {treat_passes} < {bare_passes} - 1",)
    if abs(math.log(gmean)) <= math.log(floor) + _TIE:
        return "inconclusive", (f"cost ratio {gmean:.4g} is inside the variance floor "
                                f"[{1 / floor:.4g}, {floor:.4g}]",)
    low, high = ci90
    if low > 1.0:
        return "worse", (f"cost interval lower bound {low:.4g} > 1.0",)
    if high < 1.0:
        return "promising", (f"non-inferiority holds ({treat_passes} >= {bare_passes} - 1) and "
                             f"cost interval upper bound {high:.4g} < 1.0",)
    return "no demonstrated advantage", (f"non-inferiority holds and the cost interval "
                                         f"[{low:.4g}, {high:.4g}] contains 1.0",)


def _check_int(value, name, minimum):
    if isinstance(value, bool) or not isinstance(value, int) or value < minimum:
        raise ValueError(f"{name} must be an int >= {minimum}, got {value!r}")


def decide(rows, model, comparison=("cs", "bare"), *, registered_tasks, seed, task_filter=None,
           instrument_failures=(), stratum="pooled"):
    """Spec section 6 for one model and one comparison (see the module docstring)."""
    comparison = tuple(comparison)
    if comparison not in EFFECTS:
        raise ValueError(f"comparison {comparison!r} is not one of {sorted(EFFECTS)}")
    if model not in run.MODELS:
        raise ValueError(f"model {model!r} is not one of {run.MODELS}")
    _check_int(registered_tasks, "registered_tasks", 1)
    _check_int(seed, "seed", 0)
    treatment = comparison[0]
    units, excluded = task_units(rows, model, treatment, task_filter)
    n = len(units)
    counts = {treatment: sum(u.treat_pass for u in units),
              "bare": sum(sum(u.bare_passes) / 2 >= 0.5 for u in units)}
    gmean = ci = floor = None
    if n:
        logs = [math.log(u.treat_cost / (sum(u.bare_costs) / 2)) for u in units]
        gmean = math.exp(math.fsum(logs) / n)
        floor = math.exp(math.fsum(abs(math.log(u.bare_costs[0] / u.bare_costs[1]))
                                   for u in units) / n)
        ci = bootstrap_ci(logs, seed)
    if n == 0 and not instrument_failures:
        outcome, reasons = "inconclusive", (f"0 valid tasks < {registered_tasks} registered",)
    else:
        outcome, reasons = rule(n, registered_tasks, counts[treatment], counts["bare"], gmean,
                                ci, floor, instrument_failures)
    effect = EFFECTS[comparison]
    return Decision(outcome, counts, gmean, ci, floor, n, effect, f"{effect}: {outcome}",
                    reasons, dict(sorted(excluded.items())), stratum, ())


def strata(rows, model, kinds, registered, *, seed, comparison=("cs", "bare"),
           instrument_failures=()):
    """Foreign-only, private-only and pooled Decisions from decide(task_filter=...) (D8).

    kinds: {task id: "foreign" | "private"} for every main task in `rows` (any model);
    registered: {"foreign": int, "private": int}; pooled registers their sum."""
    for task, kind in kinds.items():
        if kind not in KINDS:
            raise ValueError(f"{task}: kind {kind!r} is not one of {KINDS}")
    for r in rows:
        check_row(r)
        if r["stage"] == "main" and r["taskId"] not in kinds:
            raise ValueError(f"{r['taskId']}: no kind given; every main task needs one")
    if set(registered) != set(KINDS):
        raise ValueError(f"registered must give exactly {KINDS}, got {sorted(registered)}")
    by_kind = {k: {t for t, v in kinds.items() if v == k} for k in KINDS}
    common = dict(comparison=comparison, seed=seed, instrument_failures=instrument_failures)
    out = {k: decide(rows, model, registered_tasks=registered[k], task_filter=by_kind[k],
                     stratum=k, **common) for k in KINDS}
    out["private"] = out["private"]._replace(notes=(PRIVATE_NOTE,))
    out["pooled"] = decide(rows, model, registered_tasks=registered["foreign"]
                           + registered["private"], task_filter=set(kinds), stratum="pooled",
                           **common)._replace(notes=(POOLED_NOTE,))
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
    `+` (so `cargo --color always test` is misread: a known limit). The segment counts when:
      - any cargo subcommand carries `--all-targets` or `--tests` (also `--flag=...`), or
      - the subcommand is exactly `test` (or cargo's built-in alias `t`) - a whole word, so
        `cargo test-foo` and `cargo testx` do not count - and no target selector (`--lib`,
        `--bin`, `--bins`, `--test`, `--example(s)`, `--bench(es)`, `--doc`) narrows it.
    `cargo nextest run` without those flags does not count (outside the brief's list)."""
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
    skipped and counted under `missing`; an empty group gives None); all_targets_share = share
    of runs with at least one Bash command for which compiles_all_targets() is true;
    codescout_share = sum(codescout_calls) / sum(codescout_calls + native_calls) from the rows'
    integrity blocks (None when both are 0); edits_outside_gold = {total, per_run_mean} of the
    run's editedFiles not among gold_files[taskId] - rows do not carry the gold patch's files, so
    the caller passes gold_files: {task id: [path]}; None when not given (a task missing from a
    given map raises).

    Limit: bashCommands holds native Bash calls only; a shell run through codescout's
    run_command (cs, and full whose settings deny Bash) is not in all_targets_share."""
    kept, _ = _population(rows, task_filter)
    groups = {m: {a: [] for a in ARMS} for m in run.MODELS}
    for r in kept:
        if r["outcome"] == "scored":
            groups[r["model"]][r["arm"]].append(r)
    return {m: {a: _summary(rs, gold_files) for a, rs in arms_.items()}
            for m, arms_ in groups.items()}
