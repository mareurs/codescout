"""Freeze Stage 2: write stage3-registration.json, the record every Stage 3 batch is bound to.

Commit the registration before the pilot's first model call. This script calls no model, reads
no token file, writes no GO.json and starts no batch. It reads files, runs `git`, runs
`<claude> --version`, `docker image inspect`, `docker volume inspect`, and two read-only
`docker run --rm --network none` checks per task (stage3.arm_binary_sha256: `cat
/opt/sab/sha256.txt`; stage3.template_volume_sha256: `sha256sum` over the template volume
mounted read-only, refused BEFORE any `-v` when the volume is missing, so nothing is created).
It writes the registration and the two plan files `<run-data>/stage3-{pilot,extension}-
plan.json` that `run.py run --plan` reads. Progress goes to stderr, the summary JSON to stdout.
Stdlib only.

THE FORM IT WRITES is the one its readers already read, built with their own functions:
  - run.load_registration: `registration_sha256` = run.registration_digest(the rest), and
    `plans.<part>.plan_sha256` (no top-level plan hash: Stage 1's form is not reused);
  - run.check_registered_budget: `plans.<part>.budget`;
  - run._frozen_inputs: `frozen.root`, `frozen.trees` (frozen.check_trees, and the manifest's
    frozen.entry_names);
  - analyze_study --registration: `registered_tasks`, `registered_min`, `bootstrap_seed`;
  - registration.check (run_batch, analyze_study, `--check`): `files`, `run_data`,
    `manifest_sha256`, `tasks[*].{run_image,arm_image}`, `grader_inputs`, and (deep) every task
    entry and the frozen trees.

WRITING (Task 11 review round 1, I-4). An existing registration (on disk, or tracked at HEAD)
refuses unless `--replace`. Everything is written to temp files in the target dirs first; the
self-check reads the TEMP files back through every reader above; only then are the plan files
and, last, the registration moved into place with os.replace. Any failure (any exception)
removes the temp files and leaves every pre-existing file byte-identical: a refusal writes
nothing.

REFUSED (exit 1, `REFUSED: <reason>`): a tracked tool file changed or an untracked file in the
tool dir (freeze_stage1.check_clean, which compares blob ids too); stage1-registration.json not
tracked; an existing registration without --replace; `--run-data` not holding exactly Stage 1's
12 development runs (every runId of stage1-registration.json's order, nothing else, none
pending, journal covered) or holding a latched stop (I-2); the runner's manifest copy differing
from the given manifest; the CLI version or sha256 differing from the manifest; Python other
than PYTHON_VERSION, or a `python3` on PATH that is not this interpreter (the printed commands
name it); a development task in the selection; `tasks.verify` reasons for any selected task; a
private task's prescan with a reason (analyze_study.prescan_reasons); a missing image or an arm
image that is not the exec-only layer of its run image; an arm image whose codescout sha256
differs from the arm-image record (freeze_stage1.codescout_record); a template volume that is
missing or cannot be hashed; a grader input that cannot be read (registration.grader_inputs);
frozen inputs that fail frozen.check_trees, do not cover the manifest's entries, or have a
secret-scan hit; Stage 3 plans that do not partition the 12 selected tasks into 6 + 6 of 3
foreign + 3 private each, whose plan_sha256 does not hash its order, or that do not hold 48 runs
each; a part budget other than run.STAGE3_*; a self-check failure.

    python3 freeze_stage2.py --run-data <study dir> --manifest <setup-manifest.json> \\
        --bootstrap-seed N --aborted-archive <dir> --carried-spend <usd> [--replace]
    python3 freeze_stage2.py --check --run-data <study dir>     re-verify, write nothing

TASK 12 (re-freeze after pilot attempt 1, operator 2026-10-30): RUN_TIMEOUT_S 3600, 24 GB per
arm, and the aborted attempt archived: `--aborted-archive` must hold its rows.jsonl and
batch.jsonl (ABORTED_RUNS pilot runs, the `missing cost` stop ABORTED_STOP_ID), and
`--carried-spend` must equal those runs' spend as run.spent_usd charges it (Stage 1's rows stay
in the study journal and are counted there, never carried). The registration records
`aborted_pilot_1` (archive, file and tree hashes, runs listing, run ids, reason, spend) and
`carried_spend_usd`, which both parts' GO.json `spent_usd` and `--spent` must equal (run_batch
refuses otherwise); registration.check re-hashes the archive. A Stage 3 run dir still in the
study's runs/ refuses the freeze.
"""
import argparse
import json
import os
import platform
import shutil
import subprocess
import sys
import uuid
from pathlib import Path
from typing import Callable

import analyze
import analyze_study
import arms
import freeze_stage1
import frozen
import registration
import run
import stage3
import tasks

REGISTRATION = registration.REGISTRATION
PYTHON_VERSION = "3.14.8"
DRAW_SEED = 20261030
PILOT_SEED = 20261031
EXTENSION_SEED = 20261032
PARTS = ("pilot", "extension")
# Task 12 (operator 2026-10-30): pilot attempt 1 under registration 6ff9185c0, archived.
ABORTED_RUNS = 10
ABORTED_STOP_ID = "stop-48-82024115"
ABORTED_REASON = f"timeout (RUN_TIMEOUT_S 1800) -> missing cost stop {ABORTED_STOP_ID}"
RUNS_PER_PART = 6 * len(run.MODELS) * sum(n for _, n in run.STAGE3_REPS)  # 48
F_FORMULA = ("F = GM over the valid tasks i of max(bare0_i, bare1_i) / min(bare0_i, bare1_i) "
             "= exp(mean_i |ln(bare0_i / bare1_i)|), the two bare reps' costs; operator "
             "decision 2026-10-29: the larger reading (analyze.floor_f)")
HEADLINE_RULE = ("no single pooled headline: the foreign and private strata are decided "
                 "separately and a headline claim needs both to agree (analyze_study.headline)")
NO_RERUN_RULE = ("no re-runs: a runId with any journal row is never run again; a main row "
                 "outside STAGE3_REPS is a re-run and is refused by analyze.check_row")
INSTRUMENT_RULE = ("instrument failures (analyze_study.instrument_failures): every `tasks.py "
                   "verify` reason of a registered task, every private prescan reason, and per "
                   "model and arm ARM_INTEGRITY_RULE")
INTERVAL_RULE = ("90% percentile paired bootstrap over whole tasks, bounds rounded outward "
                 "(analyze.interval90)")
ORDER_RULE = ("invalid instrument > fewer valid tasks (inconclusive) > non-inferiority fails "
              "(worse) > cost step against F (analyze.rule)")
NON_INFERIORITY_RULE = ("treatment passes >= bare passes - 1 (bare passes a task when the mean "
                        "of its two reps is >= 0.5)")
ZERO_CALLS_RULE = "a result, reported as zero_codescout_runs; never invalid"
COMPARISON_NOTE = "per model; full - bare is 'the setup', never 'codescout'"
PILOT_GO_NOTE = ("keep Stage 1's GO.json history: replace GO.json, do not delete rows.jsonl "
                 "(move Stage 1's GO.json aside as GO.stage1.json first)")
RUNBOOK_NOTE = ("the extension may run before or after the pilot (unconditional by ruling); never "
                "edit the tool dir, never pull claude-plugins, keep sab-* images until analysis "
                "has run; run `freeze_stage2.py --check --run-data <study>` before each part")

Refused = freeze_stage1.Refused
sha256_file = freeze_stage1.sha256_file


def plan_file(run_data, part):
    return Path(run_data) / f"stage3-{part}-plan.json"


def _selected(data):
    path = Path(data) / "selection" / "selection.json"
    try:
        selected = list(json.loads(path.read_text())["selected"])
    except (OSError, ValueError, KeyError) as e:
        raise Refused(f"{path} unreadable ({e})")
    dev = sorted(set(selected) & set(run.DEVELOPMENT_TASKS))
    if dev:
        raise Refused(f"development task(s) {dev} are in selection.json's selected: development "
                      f"tasks are never scored")
    if len(selected) != 12 or len(set(selected)) != 12:
        raise Refused(f"selection.json selects {len(selected)} tasks ({len(set(selected))} "
                      f"distinct); Stage 3 registers 12")
    return path, selected


def stage1_journal(tool_dir, run_data):
    """I-2: `run_data` holds exactly Stage 1's development runs (every runId of
    stage1-registration.json's order, nothing else, none pending), its journal covers them, and
    no stop is latched. Returns {runs, spend_usd}."""
    try:
        order = json.loads((Path(tool_dir) / freeze_stage1.REGISTRATION).read_text())["order"]
        want = {run.run_id(s) for s in order}
        rows = run.read_rows(run_data)
        run.check_journal(run_data)
        latched = run.last_stop(run.read_events(run_data))
    except (OSError, ValueError, KeyError, run.Refused) as e:
        raise Refused(f"{run_data}: the Stage 1 journal cannot be read ({e})")
    latest = run.latest_rows(rows)
    if set(latest) != want:
        raise Refused(f"{run_data} holds {len(latest)} runIds, not exactly Stage 1's "
                      f"{len(want)} ({sorted(set(latest) ^ want)[:4]} ...): the cap counts only "
                      f"the study dir's journal")
    bad = [r["runId"] for r in latest.values() if r["outcome"] == run.PENDING
           or r["stage"] != "dev" or r["costUsd"] is None]
    if bad:
        raise Refused(f"{run_data}: Stage 1 rows not final or not development rows: {bad}")
    if latched:
        raise Refused(f"{run_data} is latched ({latched[0]}: {latched[1]}); inspect it first")
    return {"runs": len(latest), "spend_usd": round(sum(r["costUsd"] for r in latest.values()),
                                                    6)}


def aborted_record(archive, tool_dir, carried_spend):
    """Task 12: the archived pilot attempt 1 (operator 2026-10-30), verified and recorded.

    The archive must hold rows.jsonl and batch.jsonl with exactly ABORTED_RUNS runIds that are
    not Stage 1's, and the `missing cost` stop ABORTED_STOP_ID. Its pilot spend is computed the
    way run.spent_usd charges (a known cost, else the per_call of the run's own call), over the
    NON-Stage-1 runs only, and `carried_spend` must equal it: the study journal keeps Stage 1's
    rows, so Stage 1's spend is counted there once and must not be carried again."""
    archive = Path(archive).resolve()
    if not archive.is_dir():
        raise Refused(f"aborted-pilot archive {archive} is not a directory")
    try:
        order = json.loads((Path(tool_dir) / freeze_stage1.REGISTRATION).read_text())["order"]
        stage1 = {run.run_id(s) for s in order}
        rows = run.read_rows(archive)
        events = run.read_events(archive)
    except (OSError, ValueError, KeyError, run.Refused) as e:
        raise Refused(f"aborted-pilot archive {archive} unreadable: {e}")
    if not (archive / run.ROWS).is_file() or not (archive / run.EVENTS).is_file():
        raise Refused(f"aborted-pilot archive {archive} lacks {run.ROWS} or {run.EVENTS}")
    latest = {rid: r for rid, r in run.latest_rows(rows).items() if rid not in stage1}
    if len(latest) != ABORTED_RUNS:
        raise Refused(f"the archive holds {len(latest)} pilot runs, not {ABORTED_RUNS}")
    stops = [e for e in events if e.get("event") == "stop"
             and e.get("stopId") == ABORTED_STOP_ID and e.get("reason") == "missing cost"]
    if not stops:
        raise Refused(f"the archive has no `missing cost` stop {ABORTED_STOP_ID}")
    per_run, fallback = run.call_per_call(events, stage3.stage3_budget())
    pilot_spend = sum(r["costUsd"] if r["costUsd"] is not None
                      else per_run.get(rid, fallback) for rid, r in latest.items())
    if (isinstance(carried_spend, bool) or not isinstance(carried_spend, (int, float))
            or abs(float(carried_spend) - pilot_spend) > 1e-9):
        raise Refused(f"--carried-spend {carried_spend!r} is not the archived pilot runs' spend "
                      f"{pilot_spend!r} (the study journal already counts Stage 1's spend; "
                      f"carry only the {ABORTED_RUNS} archived runs)")
    runs_dir = archive / "runs"
    return {"archive": str(archive),
            "files": {name: sha256_file(archive / name) for name in (run.ROWS, run.EVENTS)},
            "tree_sha256": frozen.tree_sha256(archive),
            "runs_listing": sorted(p.name for p in runs_dir.iterdir()) if runs_dir.is_dir()
            else [],
            "run_ids": sorted(latest),
            "run_count": len(latest),
            "reason": ABORTED_REASON,
            "stop_id": ABORTED_STOP_ID,
            "pilot_spend_usd": pilot_spend,
            "carried_spend_usd": float(carried_spend)}


def _no_stale_run_dirs(run_data, pieces):
    """A Stage 3 runId whose runs/<runId> dir is still in the study dir would stop its run at
    mkdir (a run dir is never reused): the aborted pilot's run dirs must be archived too."""
    rids = {run.run_id(run.RunSpec(*s)) for p in pieces["plans"].values() for s in p["order"]}
    stale = sorted(p.name for p in (Path(run_data) / "runs").glob("*") if p.name in rids)
    if stale:
        raise Refused(f"{len(stale)} Stage 3 run dir(s) still in {run_data}/runs "
                      f"({stale[:3]} ...): archive them with the aborted pilot")

def _task_entries(selected, root, arm_record, docker, inspect):
    entries = []
    for t in selected:
        try:
            e = stage3.task_freeze_entry(t, root, docker=docker, inspect=inspect)
        except (OSError, ValueError, KeyError, RuntimeError, subprocess.SubprocessError) as ex:
            raise Refused(f"{t}: {type(ex).__name__}: {ex}")
        if e["arm_binary_sha256"] != arm_record:
            raise Refused(f"{t}: {e['arm_image']['tag']} holds codescout "
                          f"{e['arm_binary_sha256']}, the arm-image record says {arm_record}")
        entries.append(e)
    return entries


def _frozen_record(manifest, root):
    root = Path(root)
    try:
        rec = json.loads((root / frozen.RECORD).read_text())
        frozen.check_trees(root, rec["trees"])
    except (OSError, ValueError, KeyError, frozen.Mismatch) as e:
        raise Refused(f"frozen inputs under {root}: {e}")
    if set(rec["trees"]) != set(frozen.entry_names(manifest)):
        raise Refused(f"frozen inputs {sorted(rec['trees'])} do not cover the manifest's "
                      f"{sorted(frozen.entry_names(manifest))}")
    files, hits = frozen.scan_tree(root)
    if hits:
        raise Refused(f"secret scan of {root}: {len(hits)} file(s) match: "
                      f"{[f'{p} ({', '.join(n)})' for p, n in hits]}")
    return {"root": str(root.resolve()), "trees": rec["trees"],
            "excluded_files": rec.get("excluded_files", []),
            "secret_scan": {"files_scanned": files, "hits": 0,
                            "patterns": sorted(frozen.SECRET_PATTERNS)},
            "frozen_json_sha256": sha256_file(root / frozen.RECORD)}


def _check_plans(pieces, selected, carried=0.0):
    parts = pieces["plans"]
    if set(parts) != set(PARTS):
        raise Refused(f"stage3 parts {sorted(parts)} are not {list(PARTS)}")
    p, e = parts["pilot"]["tasks"], parts["extension"]["tasks"]
    if set(p) & set(e):
        raise Refused(f"the pilot and the extension overlap: {sorted(set(p) & set(e))}")
    if len(p) != 6 or len(e) != 6 or sorted(p + e) != sorted(selected):
        raise Refused(f"the parts ({len(p)} + {len(e)} tasks) do not partition the 12 selected")
    kinds = pieces["registered_tasks"]
    for name in PARTS:
        part = parts[name]
        if sorted(kinds[t] for t in part["tasks"]) != ["foreign"] * 3 + ["private"] * 3:
            raise Refused(f"{name}: not 3 foreign + 3 private tasks")
        order = [run.RunSpec(*s) for s in part["order"]]
        if len(order) != RUNS_PER_PART or run.plan_reps(order) != "stage3":
            raise Refused(f"{name}: {len(order)} runs, reps {run.plan_reps(order)}; expected "
                          f"{RUNS_PER_PART} with STAGE3_REPS")
        if run.plan_sha256(order) != part["plan_sha256"]:
            raise Refused(f"{name}: plan_sha256 does not hash its order")
        want = stage3.stage3_budget(carried)._asdict()
        if part["budget"] != want:
            raise Refused(f"{name}: budget {part['budget']} is not the Stage 3 budget {want}")


def decision_rule(bootstrap_seed):
    return {
        "registered_min": analyze.REGISTERED_MIN,
        "stratum_min_rule": analyze.STRATUM_MIN_RULE,
        "tolerance": analyze.TOLERANCE_NOTE,
        "floor_f": F_FORMULA,
        "resamples": analyze.RESAMPLES,
        "interval": INTERVAL_RULE,
        "bootstrap_seed": bootstrap_seed,
        "comparisons": {f"{t} - {b}": e for (t, b), e in analyze.EFFECTS.items()},
        "comparison_note": COMPARISON_NOTE,
        "order": ORDER_RULE,
        "non_inferiority": NON_INFERIORITY_RULE,
        "arm_integrity": analyze.ARM_INTEGRITY_RULE,
        "instrument": INSTRUMENT_RULE,
        "zero_codescout_calls": ZERO_CALLS_RULE,
        "admission": analyze.ADMISSION_RULE,
        "re_runs": NO_RERUN_RULE,
        "headline": HEADLINE_RULE,
        "agent_resources": list(arms.AGENT_RESOURCES),
        "per_call_usd": run.STAGE3_PER_CALL_USD,
        "run_timeout_s": run.RUN_TIMEOUT_S,
        "planned_reps": [list(r) for r in run.STAGE3_REPS],
        "models": list(run.MODELS),
    }


def go_templates(reg, tool_dir, python):
    """Per part, the GO.json the operator writes (never this script) and the run command.
    spent_usd: check_go binds GO.json's spent_usd to the batch's Budget.spent_usd, i.e. `--spent`,
    the spend OUTSIDE this data dir's journal; spent_usd() already adds every rows.jsonl row
    (Stage 1's dev rows included), so in the one study dir it is 0."""
    run_data = reg["run_data"]
    out = {}
    for name in PARTS:
        part = reg["plans"][name]
        b = part["budget"]
        go = {"cap_usd": b["cap_usd"], "per_call_usd": b["per_call_usd"],
              "reserve_usd": b["reserve_usd"], "spent_usd": b["spent_usd"],
              "plan_sha256": part["plan_sha256"],
              "registration_sha256": reg["registration_sha256"],
              "authorized_by": "operator", "acknowledged_stops": []}
        cmd = (f"{python} {Path(tool_dir).resolve() / 'run.py'} run --plan "
               f"{plan_file(run_data, name)} --data {run_data} --registration "
               f"{Path(tool_dir).resolve() / REGISTRATION} --cap {b['cap_usd']} "
               f"--spent {b['spent_usd']} --per-call {b['per_call_usd']} "
               f"--reserve {b['reserve_usd']}")
        note = ("acknowledged_stops: list the id of the previous batch's stop when it was not "
                "'plan complete' (run.check_latch); " + RUNBOOK_NOTE)
        if name == "pilot":
            note += "; " + PILOT_GO_NOTE
        else:
            note += ("; move the pilot's GO.json aside (GO.pilot.json) before writing this one; "
                     "if the pilot ended with a stop, list its id")
        out[name] = {"GO.json": go, "command": cmd, "note": note}
    return out


def _self_check(reg_path, plan_paths, manifest, root, check_kw):
    """Read the TEMP registration and plan files back through every consumer: run.load_plan +
    load_registration + check_registered_budget for both parts, run._frozen_inputs,
    analyze_study --registration's own checks (digest, registered_min, mapping from bundles),
    and registration.check (data dir, manifest, images, grader inputs)."""
    try:
        loaded = None
        for name in PARTS:
            plan = run.load_plan(plan_paths[name])
            loaded = run.load_registration(reg_path, plan)
            run.check_registered_budget(loaded, plan, stage3.stage3_budget(
                loaded["carried_spend_usd"]))
        run._frozen_inputs(loaded, manifest)
        body = {k: v for k, v in loaded.items() if k != "registration_sha256"}
        if loaded["registration_sha256"] != run.registration_digest(body):
            raise ValueError("registration digest")
        if loaded["registered_min"] != analyze.REGISTERED_MIN:
            raise ValueError("registered_min")
        if stage3.registered_mapping(list(loaded["registered_tasks"]), root) != \
                loaded["registered_tasks"]:
            raise ValueError("registered_tasks differ from the bundles' kinds")
        registration.check(loaded, files=False, **check_kw)
    except Exception as e:
        raise Refused(f"self-check: the written registration is not what its readers accept: "
                      f"{type(e).__name__}: {e}")


def _tracked(tool_dir, name):
    return bool(freeze_stage1._git(tool_dir, "ls-files", "--", name).strip())


def python3_on_path():
    """The interpreter `python3` on PATH actually runs (a pyenv shim is asked, not resolved as a
    file): its own sys.executable, resolved; None when there is none."""
    exe = shutil.which("python3")
    if not exe:
        return None
    try:
        p = subprocess.run([exe, "-I", "-c", "import sys; print(sys.executable)"],
                           capture_output=True, text=True, timeout=30)
    except (OSError, subprocess.SubprocessError):
        return None
    out = p.stdout.strip()
    return str(Path(out).resolve()) if p.returncode == 0 and out else None

def freeze_stage2(*, tool_dir, data, manifest_path, run_data, bootstrap_seed, out=None,
                  replace=False, head_fn=freeze_stage1._head,
                  inspect: Callable = tasks._inspect_image,
                  run_cli: Callable = freeze_stage1.default_run_cli,
                  verify_fn: Callable = tasks.verify, docker: Callable = None,
                  python_version: Callable = platform.python_version,
                  executable=sys.executable, python_on_path: Callable = None,
                  frozen_root=None, s5_dir=None, node_version: Callable = None,
                  aborted_archive, carried_spend,
                  pieces_fn: Callable = stage3.registration_pieces) -> dict:
    tool_dir, data = Path(tool_dir), Path(data)
    run_data, manifest_path = Path(run_data).resolve(), Path(manifest_path).resolve()
    if isinstance(bootstrap_seed, bool) or not isinstance(bootstrap_seed, int):
        raise Refused(f"bootstrap seed must be an int, got {bootstrap_seed!r}")
    if python_version() != PYTHON_VERSION:
        raise Refused(f"Python {python_version()}; the study registered CPython {PYTHON_VERSION}")
    python = Path(executable).resolve()
    on_path = (python_on_path or python3_on_path)()
    if on_path is None or Path(on_path).resolve() != python:
        raise Refused(f"`python3` on PATH runs {on_path}, not this interpreter {python}: the "
                      f"printed commands name {python}; run the freeze with that interpreter")
    path = Path(out) if out else tool_dir / REGISTRATION
    if not replace and (path.exists() or (out is None and _tracked(tool_dir, REGISTRATION))):
        raise Refused(f"{path} exists (or is tracked): a registration is replaced only with "
                      f"--replace")

    files = freeze_stage1.tracked_files(tool_dir, exclude=REGISTRATION)
    freeze_stage1.check_clean(tool_dir, files, exclude=REGISTRATION)
    if freeze_stage1.REGISTRATION not in files:
        raise Refused(f"{freeze_stage1.REGISTRATION} (Stage 1 evidence) is not tracked")
    journal = stage1_journal(tool_dir, run_data)
    aborted = aborted_record(aborted_archive, tool_dir, carried_spend)
    carried = aborted["carried_spend_usd"]

    sel_path, selected = _selected(data)
    root = data / "tasks"
    reasons = {k: v for k, v in verify_fn(selected, root, selected=selected,
                                          inspect=inspect).items() if v}
    if reasons:
        raise Refused(f"tasks.py verify has reasons: {json.dumps(reasons)}")

    copy = run_data / "setup-manifest.json"
    if not copy.is_file():
        raise Refused(f"{copy} is missing: the runner loads its manifest from there")
    if sha256_file(copy) != sha256_file(manifest_path):
        raise Refused(f"{copy} differs from {manifest_path}: the runner would load another "
                      f"manifest than the one frozen")
    manifest = json.loads(copy.read_text())
    got_cli = run_cli(manifest["cli"]["path"]).strip()
    if got_cli != manifest["cli"]["version"].strip():
        raise Refused(f"CLI {manifest['cli']['path']} reports {got_cli!r}, the manifest froze "
                      f"{manifest['cli']['version']!r}")
    try:
        run._check_cli(manifest)
    except (run.Refused, OSError) as e:
        raise Refused(str(e))
    codescout = freeze_stage1.codescout_record(tool_dir, manifest)

    try:
        pieces = pieces_fn(selected, root, split_seed=DRAW_SEED, pilot_seed=PILOT_SEED,
                           extension_seed=EXTENSION_SEED)
    except ValueError as e:
        raise Refused(f"stage3 plans: {e}")
    for part in pieces["plans"].values():  # the GO.json spent_usd of both parts (check_go)
        part["budget"] = dict(part["budget"], spent_usd=carried)
    _check_plans(pieces, selected, carried)
    _no_stale_run_dirs(run_data, pieces)
    mapping = pieces["registered_tasks"]
    prescan = analyze_study.prescan_reasons(mapping, root)
    if prescan:
        raise Refused(f"private prescan reasons: {json.dumps(prescan)}")

    fz = _frozen_record(manifest, frozen_root or frozen.FROZEN)  # cheap: before any docker run
    entries = _task_entries(selected, root, codescout["arm_image_binary"]["sha256"], docker,
                            inspect)
    try:
        graders = registration.grader_inputs(selected, root, s5_dir)
    except (OSError, ValueError, KeyError, registration.Mismatch) as e:
        raise Refused(f"grader inputs: {type(e).__name__}: {e}")
    node = (node_version or registration.host_node_version)()
    if not node:
        raise Refused("`node --version` failed: node runs the private grader")

    body = {
        "stage": 2,
        "head": head_fn(tool_dir),
        "files": {f: sha256_file(tool_dir / f) for f in files},
        "stage1_registration_sha256": sha256_file(tool_dir / freeze_stage1.REGISTRATION),
        "stage1_journal": journal,
        "aborted_pilot_1": aborted,
        "carried_spend_usd": carried,
        "manifest_path": str(copy),
        "manifest_sha256": sha256_file(copy),
        "selection_rule_sha256": sha256_file(tool_dir / "selection-rule.json"),
        "selection_sha256": sha256_file(sel_path),
        "selected": selected,
        "registered_tasks": mapping,
        "registered_min": pieces["registered_min"],
        "stratum_min": pieces["stratum_min"],
        "rules": pieces["rules"],
        "bootstrap_seed": bootstrap_seed,
        "split": pieces["split"],
        "draw_seed": DRAW_SEED,
        "plans": pieces["plans"],
        "tasks": entries,
        "grader_inputs": graders,
        "instrument_gate": {"rule": INSTRUMENT_RULE,
                            "verify_reasons": {t: [] for t in selected},
                            "prescan_reasons": prescan},
        "frozen": fz,
        "decision_rule": decision_rule(bootstrap_seed),
        "run_data": str(run_data),
        "tasks_root": str(root.resolve()),
        "claude_cli_version": got_cli,
        "claude_cli_sha256": manifest["cli"]["sha256"],
        "codescout": codescout,
        "python_version": python_version(),
        "python_executable": str(python),
        "node_version": node,
    }
    reg = dict(body, registration_sha256=run.registration_digest(body))
    templates = go_templates(reg, tool_dir, python)

    tag = uuid.uuid4().hex[:8]
    tmp_reg = path.with_name(f".{path.name}.tmp-{tag}")
    tmp_plans = {n: plan_file(run_data, n).with_name(f".{plan_file(run_data, n).name}.tmp-{tag}")
                 for n in PARTS}
    try:
        for name in PARTS:
            tmp_plans[name].write_text(json.dumps(pieces["plans"][name]["order"]) + "\n")
        tmp_reg.write_text(json.dumps(reg, indent=2) + "\n")
        _self_check(tmp_reg, tmp_plans, manifest, root,
                    dict(data=run_data, tool_dir=tool_dir, inspect=inspect, s5_dir=s5_dir,
                         runner_tasks_root=root, node_version=lambda: node))
        for name in PARTS:
            os.replace(tmp_plans[name], plan_file(run_data, name))
        os.replace(tmp_reg, path)
    finally:
        for p in (tmp_reg, *tmp_plans.values()):
            p.unlink(missing_ok=True)
    return dict(reg, go_templates=templates)


def check_main(a, *, inspect=tasks._inspect_image, docker=None, s5_dir=None,
               node_version=None) -> dict:
    """--check: the committed registration (round 2, N-1a) against the live tree, data dir,
    images, task files, template content, per-image binary, node, grader inputs and frozen
    trees. Writes nothing."""
    path = a.registration or (a.tool_dir / REGISTRATION)
    reg = json.loads(Path(path).read_text())
    body = {k: v for k, v in reg.items() if k != "registration_sha256"}
    if reg.get("registration_sha256") != run.registration_digest(body):
        raise Refused(f"{path}: registration_sha256 does not match its content")
    try:
        registration.check_committed(path, a.tool_dir)
        registration.check(reg, data=a.run_data, tool_dir=a.tool_dir, inspect=inspect,
                           s5_dir=s5_dir, deep=True, docker=docker,
                           runner_tasks_root=a.data / "tasks", node_version=node_version)
    except registration.Mismatch as e:
        raise Refused(str(e))
    return {"ok": True, "registration": str(path),
            "registration_sha256": reg["registration_sha256"]}


def main(argv=None, **check_kw) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--tool-dir", type=Path, default=Path(__file__).resolve().parent)
    ap.add_argument("--data", type=Path, default=tasks.DATA)
    ap.add_argument("--manifest", type=Path, help="setup-manifest.json (freeze)")
    ap.add_argument("--run-data", type=Path, required=True,
                    help="the one study data dir; run_batch loads <run-data>/setup-manifest.json")
    ap.add_argument("--bootstrap-seed", type=int, help="analyze's bootstrap seed (freeze)")
    ap.add_argument("--aborted-archive", type=Path,
                    help="(freeze) the archived pilot attempt 1 (rows.jsonl, batch.jsonl, runs/)")
    ap.add_argument("--carried-spend", type=float,
                    help="(freeze) the archived pilot runs' spend, carried as --spent")
    ap.add_argument("--replace", action="store_true",
                    help="replace an existing registration (never implied)")
    ap.add_argument("--check", action="store_true",
                    help="re-verify the committed registration; write nothing")
    ap.add_argument("--registration", type=Path, default=None, help="(--check) its path")
    a = ap.parse_args(argv)
    try:
        if a.check:
            out = check_main(a, **check_kw)
        else:
            if (a.manifest is None or a.bootstrap_seed is None or a.aborted_archive is None
                    or a.carried_spend is None):
                ap.error("the freeze needs --manifest, --bootstrap-seed, --aborted-archive and "
                         "--carried-spend")
            reg = freeze_stage2(tool_dir=a.tool_dir, data=a.data, manifest_path=a.manifest,
                                run_data=a.run_data, bootstrap_seed=a.bootstrap_seed,
                                aborted_archive=a.aborted_archive,
                                carried_spend=a.carried_spend, replace=a.replace)
            out = {"registration_sha256": reg["registration_sha256"],
                   "plans": {n: {"plan_sha256": reg["plans"][n]["plan_sha256"],
                                 "runs": len(reg["plans"][n]["order"]),
                                 "tasks": reg["plans"][n]["tasks"]} for n in PARTS},
                   "go_templates": reg["go_templates"]}
    except (Refused, OSError, ValueError, KeyError, RuntimeError,
            subprocess.SubprocessError) as e:
        print(f"REFUSED: {type(e).__name__ + ': ' if not isinstance(e, Refused) else ''}{e}",
              file=sys.stderr)
        return 1
    print(json.dumps(out, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
