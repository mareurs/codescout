"""Freeze Stage 2: write stage3-registration.json, the record every Stage 3 batch is bound to.

Commit the registration before the pilot's first model call. This script calls no model, reads
no token file, writes no GO.json and starts no batch. It reads files, runs `git`, runs
`<claude> --version`, `docker image inspect`, and two read-only `docker run --rm --network none`
checks per task (stage3.arm_binary_sha256: `cat /opt/sab/sha256.txt`; stage3.template_volume_
sha256: `sha256sum` over the template volume mounted read-only). It writes the registration and
the two plan files `<run-data>/stage3-{pilot,extension}-plan.json` that `run.py run --plan` reads.
Stdlib only.

THE FORM IT WRITES is the one its readers already read, built with their own functions:
  - run.load_registration: `registration_sha256` = run.registration_digest(the rest), and
    `plans.<part>.plan_sha256` (no top-level plan hash: Stage 1's form is not reused);
  - run.check_registered_budget: `plans.<part>.budget`;
  - run._frozen_inputs: `frozen.root`, `frozen.trees` (frozen.check_trees, and the manifest's
    frozen.entry_names);
  - analyze_study --registration: `registered_tasks`, `registered_min`, `bootstrap_seed`.
After writing, the script reads the file back through those functions (self-check) and refuses
when any of them would.

REFUSED (exit 1, `REFUSED: <reason>`): a tracked tool file changed or an untracked file in the
tool dir (freeze_stage1.check_clean, which compares blob ids too); the runner's manifest copy
differing from the given manifest; the CLI version or sha256 differing from the manifest;
Python other than PYTHON_VERSION; a development task in the selection; `tasks.verify` reasons
for any selected task; a private task's prescan with a reason (analyze_study.prescan_reasons);
a missing image or an arm image that is not the exec-only layer of its run image; an arm image
whose codescout sha256 differs from the arm-image record (freeze_stage1.codescout_record); a
template whose volume or dump cannot be hashed; frozen inputs that fail frozen.check_trees, do
not cover the manifest's entries, or have a secret-scan hit; Stage 3 plans that do not
partition the 12 selected tasks into 6 + 6 or do not hold 48 runs each; a part budget other
than run.STAGE3_*.

    python3 freeze_stage2.py --run-data <study dir> --manifest <setup-manifest.json> \\
        --bootstrap-seed N
"""
import argparse
import json
import platform
import subprocess
import sys
from pathlib import Path
from typing import Callable

import analyze
import analyze_study
import arms
import freeze_stage1
import frozen
import pilot
import run
import stage3
import tasks

REGISTRATION = "stage3-registration.json"
PYTHON_VERSION = "3.14.8"
DRAW_SEED = 20261030
PILOT_SEED = 20261031
EXTENSION_SEED = 20261032
PARTS = ("pilot", "extension")
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


def _task_entries(selected, root, arm_record, docker, inspect):
    entries = []
    for t in selected:
        try:
            e = stage3.task_freeze_entry(t, root, docker=docker, inspect=inspect)
        except (OSError, ValueError, KeyError, RuntimeError, subprocess.SubprocessError) as ex:
            raise Refused(f"{t}: {type(ex).__name__}: {ex}")
        for key in ("run_image", "arm_image"):
            if not e[key]["id"]:
                raise Refused(f"{t}: {e[key]['tag']} is not a local image")
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


def _check_plans(pieces, selected):
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
        tasks_ = part["tasks"]
        if sorted(kinds[t] for t in tasks_) != ["foreign"] * 3 + ["private"] * 3:
            raise Refused(f"{name}: not 3 foreign + 3 private tasks")
        order = [run.RunSpec(*s) for s in part["order"]]
        if len(order) != RUNS_PER_PART or run.plan_reps(order) != "stage3":
            raise Refused(f"{name}: {len(order)} runs, reps {run.plan_reps(order)}; expected "
                          f"{RUNS_PER_PART} with STAGE3_REPS")
        if run.plan_sha256(order) != part["plan_sha256"]:
            raise Refused(f"{name}: plan_sha256 does not hash its order")
        want = stage3.stage3_budget()._asdict()
        if part["budget"] != want:
            raise Refused(f"{name}: budget {part['budget']} is not the Stage 3 budget {want}")


def decision_rule(bootstrap_seed):
    return {
        "registered_min": analyze.REGISTERED_MIN,
        "stratum_min_rule": analyze.STRATUM_MIN_RULE,
        "tolerance": analyze.TOLERANCE_NOTE,
        "floor_f": F_FORMULA,
        "resamples": analyze.RESAMPLES,
        "interval": "90% percentile paired bootstrap over whole tasks, bounds rounded outward "
                    "(analyze.interval90)",
        "bootstrap_seed": bootstrap_seed,
        "comparisons": {f"{t} - {b}": e for (t, b), e in analyze.EFFECTS.items()},
        "comparison_note": "per model; full - bare is 'the setup', never 'codescout'",
        "order": "invalid instrument > fewer valid tasks (inconclusive) > non-inferiority "
                 "fails (worse) > cost step against F (analyze.rule)",
        "non_inferiority": "treatment passes >= bare passes - 1 (bare passes a task when the "
                           "mean of its two reps is >= 0.5)",
        "arm_integrity": analyze.ARM_INTEGRITY_RULE,
        "instrument": INSTRUMENT_RULE,
        "zero_codescout_calls": "a result, reported as zero_codescout_runs; never invalid",
        "admission": analyze.ADMISSION_RULE,
        "re_runs": NO_RERUN_RULE,
        "headline": HEADLINE_RULE,
        "agent_resources": list(arms.AGENT_RESOURCES),
        "per_call_usd": run.STAGE3_PER_CALL_USD,
        "run_timeout_s": run.RUN_TIMEOUT_S,
        "planned_reps": [list(r) for r in run.STAGE3_REPS],
        "models": list(run.MODELS),
    }


def go_templates(reg, run_data, tool_dir):
    """Per part, the GO.json the operator writes (never this script) and the run command.
    spent_usd: check_go binds GO.json's spent_usd to the batch's Budget.spent_usd, i.e. `--spent`,
    the spend OUTSIDE this data dir's journal; spent_usd() already adds every rows.jsonl row
    (Stage 1's dev rows included), so in the one study dir it is 0."""
    latched = run.last_stop(run.read_events(run_data))
    out = {}
    for name in PARTS:
        part = reg["plans"][name]
        b = part["budget"]
        go = {"cap_usd": b["cap_usd"], "per_call_usd": b["per_call_usd"],
              "reserve_usd": b["reserve_usd"], "spent_usd": b["spent_usd"],
              "plan_sha256": part["plan_sha256"],
              "registration_sha256": reg["registration_sha256"],
              "authorized_by": "operator", "acknowledged_stops": []}
        cmd = (f"python3 {Path(tool_dir) / 'run.py'} run --plan {plan_file(run_data, name)} "
               f"--data {run_data} --registration {Path(tool_dir) / REGISTRATION} "
               f"--cap {b['cap_usd']} --spent {b['spent_usd']} --per-call {b['per_call_usd']} "
               f"--reserve {b['reserve_usd']}")
        note = ("acknowledged_stops: list the id of the previous batch's stop when it was not "
                "'plan complete' (run.check_latch)")
        if name == "pilot" and latched:
            note += f"; the study dir is latched now: {latched[0]} ({latched[1]})"
        if name == "extension":
            note += ("; move the pilot's GO.json aside (GO.pilot.json) before writing this one; "
                     "if the pilot ended with a stop, list its id")
        out[name] = {"GO.json": go, "command": cmd, "note": note}
    return out


def _self_check(path, manifest, root):
    """Read the written registration back through every consumer: run.load_plan +
    load_registration + check_registered_budget for both parts, run._frozen_inputs, and
    analyze_study --registration's own checks (digest, registered_min, mapping from bundles)."""
    try:
        loaded = json.loads(Path(path).read_text())
        for name in PARTS:
            plan = run.load_plan(plan_file(loaded["run_data"], name))
            loaded = run.load_registration(path, plan)
            run.check_registered_budget(loaded, plan, stage3.stage3_budget())
        run._frozen_inputs(loaded, manifest)
        body = {k: v for k, v in loaded.items() if k != "registration_sha256"}
        if loaded["registration_sha256"] != run.registration_digest(body):
            raise ValueError("registration digest")
        if loaded["registered_min"] != analyze.REGISTERED_MIN:
            raise ValueError("registered_min")
        if stage3.registered_mapping(list(loaded["registered_tasks"]), root) != \
                loaded["registered_tasks"]:
            raise ValueError("registered_tasks differ from the bundles' kinds")
    except (run.Refused, OSError, ValueError, KeyError) as e:
        raise Refused(f"self-check: the written registration is not what its readers accept: "
                      f"{e}")


def freeze_stage2(*, tool_dir, data, manifest_path, run_data, bootstrap_seed, out=None,
                  head_fn=freeze_stage1._head, inspect: Callable = tasks._inspect_image,
                  run_cli: Callable = freeze_stage1.default_run_cli,
                  verify_fn: Callable = tasks.verify, docker: Callable = None,
                  python_version: Callable = platform.python_version,
                  frozen_root=None, pieces_fn: Callable = stage3.registration_pieces) -> dict:
    tool_dir, data, run_data = Path(tool_dir), Path(data), Path(run_data)
    docker = docker or pilot.docker
    if isinstance(bootstrap_seed, bool) or not isinstance(bootstrap_seed, int):
        raise Refused(f"bootstrap seed must be an int, got {bootstrap_seed!r}")
    if python_version() != PYTHON_VERSION:
        raise Refused(f"Python {python_version()}; the study registered CPython {PYTHON_VERSION}")

    files = freeze_stage1.tracked_files(tool_dir, exclude=REGISTRATION)
    freeze_stage1.check_clean(tool_dir, files, exclude=REGISTRATION)
    if freeze_stage1.REGISTRATION not in files:
        raise Refused(f"{freeze_stage1.REGISTRATION} (Stage 1 evidence) is not tracked")

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
    _check_plans(pieces, selected)
    mapping = pieces["registered_tasks"]
    prescan = analyze_study.prescan_reasons(mapping, root)
    if prescan:
        raise Refused(f"private prescan reasons: {json.dumps(prescan)}")

    entries = _task_entries(selected, root, codescout["arm_image_binary"]["sha256"], docker,
                            inspect)
    fz = _frozen_record(manifest, frozen_root or frozen.FROZEN)

    body = {
        "stage": 2,
        "head": head_fn(tool_dir),
        "files": {f: sha256_file(tool_dir / f) for f in files},
        "stage1_registration_sha256": sha256_file(tool_dir / freeze_stage1.REGISTRATION),
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
        "instrument_gate": {"rule": INSTRUMENT_RULE,
                            "verify_reasons": {t: [] for t in selected},
                            "prescan_reasons": prescan},
        "frozen": fz,
        "decision_rule": decision_rule(bootstrap_seed),
        "run_data": str(run_data),
        "claude_cli_version": got_cli,
        "claude_cli_sha256": manifest["cli"]["sha256"],
        "codescout": codescout,
        "python_version": python_version(),
    }
    reg = dict(body, registration_sha256=run.registration_digest(body))
    path = Path(out) if out else tool_dir / REGISTRATION
    for name in PARTS:
        plan_file(run_data, name).write_text(json.dumps(pieces["plans"][name]["order"]) + "\n")
    path.write_text(json.dumps(reg, indent=2) + "\n")
    try:
        _self_check(path, manifest, root)
    except Refused:
        path.unlink(missing_ok=True)
        raise
    return dict(reg, go_templates=go_templates(reg, run_data, tool_dir))


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--tool-dir", type=Path, default=Path(__file__).resolve().parent)
    ap.add_argument("--data", type=Path, default=tasks.DATA)
    ap.add_argument("--manifest", type=Path, required=True, help="setup-manifest.json")
    ap.add_argument("--run-data", type=Path, required=True,
                    help="the one study data dir; run_batch loads <run-data>/setup-manifest.json")
    ap.add_argument("--bootstrap-seed", type=int, required=True,
                    help="analyze's bootstrap seed; recorded (analyze_study reads it)")
    a = ap.parse_args(argv)
    try:
        reg = freeze_stage2(tool_dir=a.tool_dir, data=a.data, manifest_path=a.manifest,
                            run_data=a.run_data, bootstrap_seed=a.bootstrap_seed)
    except (Refused, OSError, ValueError, KeyError, RuntimeError,
            subprocess.SubprocessError) as e:
        print(f"REFUSED: {type(e).__name__ + ': ' if not isinstance(e, Refused) else ''}{e}",
              file=sys.stderr)
        return 1
    print(json.dumps({"registration_sha256": reg["registration_sha256"],
                      "plans": {n: {"plan_sha256": reg["plans"][n]["plan_sha256"],
                                    "runs": len(reg["plans"][n]["order"]),
                                    "tasks": reg["plans"][n]["tasks"]} for n in PARTS},
                      "go_templates": reg["go_templates"]}, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
