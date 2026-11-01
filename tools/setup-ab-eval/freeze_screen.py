"""Freeze the Live Rust screen: write screen-registration.json, the record the operator's go is bound to.

The screen is the bare arm only, over the tasks of the pool file (default multilang-pool-v3.json: seven
tasks), two models, two reps (28 runs). This script calls no model, reads no token, writes no GO.json (only
GO.template.json, which the operator turns into GO.json by hand) and starts no container: it
reads files, runs `git`, `<claude> --version` and `docker image inspect` (through
tasks._inspect_image, injectable). Stdlib only.

    freeze_screen.py pool    write the pool file from the dataset and the saved controls
    freeze_screen.py verify-pool  re-derive the pool file's entries from the real controls
    freeze_screen.py plan    write the screen plan (JSON [[task, model, arm, rep], ...])
    freeze_screen.py freeze  write the registration, GO.template.json (never GO.json)
    freeze_screen.py check   re-hash the tool files, pool, plan, bundles, images and frozen trees

`freeze` and `check` refuse (named reason) a plan that has a task outside the pool, a non-bare arm
(run._run_one's bare-only guard fires only after run_batch has started its network and proxy and
latches a stop, so the refusal has to come first), or a bundle whose control is not relaxed ok.
The registration is stage "screen": run.run_batch applies its heavy host re-check only to stage
3 plans and to `stage == 2`, so a screen registration is verified by `check` here, not by it.
"""
import argparse
import json
import os
import platform
import subprocess
import sys
import uuid
from pathlib import Path
from typing import Callable

import freeze_stage1
import frozen
import multilang_tasks as mt
import run
import tasks

REGISTRATION = "screen-registration.json"
POOL_V1 = "multilang-pool-v1.json"  # the registered history, never edited
POOL_V2 = "multilang-pool-v2.json"  # superseded by v3, never edited
POOL = "multilang-pool-v3.json"  # the current pool: the only one freeze and check accept
STAGE3_REGISTRATION = "stage3-registration.json"
GO_TEMPLATE = "GO.template.json"
PLAN_NAME = "screen-plan.json"
POOL_NAME = "setup-ab live rust v3"
AMENDS = "setup-ab live rust v2"
GATE = "relaxed (spec 2026-11-01)"
REGISTERED = "2026-11-01"
SEED = 20261101
MODELS = run.MODELS
REPS = 2
# cap 50: run k is admitted iff the first k-1 runs cost <= cap - reserve - per_call = $40, so all 28
# runs of pool v3 start iff the first 27 average <= $1.48 (at $1.0 each all 28; at $1.6 not all)
BUDGET = {"cap_usd": 50.0, "per_call_usd": 5.0, "reserve_usd": 5.0}
# F2P / kept P2P / dropped P2P of the nine saved controls (task 1 report): a sanity table for the
# `pool` command only (a pool file that disagrees is not written); freeze reads the pool file's own
# ids and counts and re-verifies them against the real controls.
DROPPED = [{"id": "pola-rs__polars-28484",
            "reason": "its build and sweep pass, but the empty and the gold grade were both OOM-killed at "
                      "the frozen 8 GB limit (0 of 1555 tests ran; gold grade 1595 s; image 85.9 GB); "
                      "decided before any model run"},
           {"id": "gitbutlerapp__gitbutler-14920",
            "reason": "round-3 prescan had 0 hits, but the gold grade fails in the grading environment: "
                      "3 expected tests fail identically in the empty and the gold run (the F2P test "
                      "passes): read_file_from_workspace_security::rejects_dotdot_traversal and "
                      "::rejects_symlink_escape write to workdir.parent() and get Permission denied as "
                      "uid 1000 (/ is root-owned 755), and command::land::land_no_ff_creates_signed_merge_"
                      "commit fails with 'ssh-keygen should produce a signing key' (cause not isolated); "
                      "the control ran as root, the grade runs as the image user; keeping the task needs a "
                      "second post-hoc rule plus an unexplained failure; decided before any model run; it "
                      "may return in a later pool version after investigation"}]
SANITY = {
    "rust-lang__rust-analyzer-22789": (1, 8119, 0),
    "pola-rs__polars-28484": (1, 1554, 0),
    "gitbutlerapp__gitbutler-14920": (1, 3426, 0),
    "modelcontextprotocol__rust-sdk-1021": (9, 654, 0),
    "h4ckf0r0day__obscura-412": (1, 247, 71),
    "h4ckf0r0day__obscura-477": (1, 358, 3),
    "h4ckf0r0day__obscura-414": (1, 258, 71),
    "h4ckf0r0day__obscura-447": (3, 272, 71),
    "h4ckf0r0day__obscura-390": (4, 235, 71),
}
_TASK_KEYS = {"id", "repo", "control_sha256", "expected_count", "dropped_count"}


class Refused(RuntimeError):
    """The freeze (or the check) is refused; the message names the reason."""


def sha256_file(path) -> str:
    return freeze_stage1.sha256_file(path)


# --- pool file --------------------------------------------------------------------------------

def make_pool(records, controls_dir, ids, *, name=POOL_NAME, amends=None, dropped=None) -> dict:
    """The pool file body for `ids` from dataset records ({id: record}) and
    `<controls_dir>/<id>/control.json`. expected_count = FAIL_TO_PASS + kept PASS_TO_PASS and
    dropped_count come from multilang_tasks.relaxed_expected, the rule the grader uses. An id the
    dataset or the controls do not have is Refused."""
    entries = []
    for tid in ids:
        if tid not in records:
            raise Refused(f"{tid}: not in the dataset records")
        record = records[tid]
        cj = Path(controls_dir) / tid / "control.json"
        try:
            exp = mt.relaxed_expected(record, cj)
            csha = sha256_file(cj)
        except (mt.LiveRefused, OSError, ValueError, KeyError) as e:
            raise Refused(f"{tid}: {type(e).__name__}: {e}")
        repo = record["repo"]
        if not tid.startswith(repo.replace("/", "__") + "-"):
            raise Refused(f"{tid}: record repo {repo!r} does not match the id's owner prefix")
        entries.append({"id": tid, "repo": repo, "control_sha256": csha,
                        "expected_count": exp["fail_to_pass"] + exp["pass_to_pass_kept"],
                        "dropped_count": exp["pass_to_pass_dropped"]})
    body = {"pool": name, "registered": REGISTERED, "gate": GATE}
    if amends is not None:
        body["amends"] = amends
    if dropped is not None:
        body["dropped"] = dropped
    body["tasks"] = entries
    return body


def verify_pool(pool, records, controls_dir) -> list:
    """Reasons the pool file's entries are not what the real controls give ([] = ok): each id is
    re-derived with make_pool (dataset record, control.json sha256, relaxed_expected counts)."""
    out = []
    try:
        check_pool(pool)
    except Refused as e:
        return [str(e)]
    for t in pool["tasks"]:
        try:
            real = make_pool(records, controls_dir, [t["id"]])["tasks"][0]
        except Refused as e:
            out.append(str(e))
            continue
        if real != t:
            out.append(f"{t['id']}: the pool file says {t}, the real control gives {real}")
    return out



def sanity_mismatches(pool) -> list:
    out = []
    for t in pool["tasks"]:
        want = SANITY.get(t["id"])
        if want is not None and (t["expected_count"], t["dropped_count"]) != (want[0] + want[1], want[2]):
            out.append(f"{t['id']}: expected_count {t['expected_count']} dropped {t['dropped_count']}, "
                       f"the controls batch showed {want[0] + want[1]} dropped {want[2]}")
    return out


def check_pool(pool) -> dict:
    """The pool, or Refused when it is not a pool file in the registered shape. The id set and the
    counts are the file's own; task_entry re-verifies each entry against its real control."""
    if not isinstance(pool, dict) or not isinstance(pool.get("tasks"), list) or not pool["tasks"]:
        raise Refused("pool file: no tasks list")
    for t in pool["tasks"]:
        if not isinstance(t, dict) or set(t) != _TASK_KEYS:
            raise Refused(f"pool entry {t.get('id') if isinstance(t, dict) else t!r}: keys "
                          f"{sorted(t) if isinstance(t, dict) else t!r}, want {sorted(_TASK_KEYS)}")
    ids = [t["id"] for t in pool["tasks"]]
    if len(set(ids)) != len(ids):
        raise Refused(f"pool file lists a task twice: {ids}")
    if pool.get("gate") != GATE:
        raise Refused(f"pool gate {pool.get('gate')!r}, the registered gate is {GATE!r}")
    for d in pool.get("dropped") or []:
        if not isinstance(d, dict) or not d.get("id") or not d.get("reason"):
            raise Refused(f"pool 'dropped' entry {d!r} needs an id and a reason")
        if d["id"] in ids:
            raise Refused(f"{d['id']} is both in the pool and dropped from it")
    return pool


# --- plan -------------------------------------------------------------------------------------

def screen_plan(pool, models=MODELS, reps=REPS, seed=SEED) -> list:
    """Bare arm only: per task, per model, `reps` runs; the order inside a block is shuffled by
    one random.Random(seed) (run.make_order), deterministic for a seed."""
    ids = [t["id"] for t in pool["tasks"]]
    return run.make_order(ids, tuple(models), seed, (("bare", reps),))


def plan_problems(plan, pool) -> list:
    """Reasons a plan is not the screen: a task outside the pool, a non-bare arm, or a shape other
    than every pool task x both models x reps 0..REPS-1 (28 runs for 7 tasks)."""
    ids = {t["id"] for t in pool["tasks"]}
    out = []
    outside = sorted({s.task_id for s in plan} - ids)
    if outside:
        out.append(f"the plan has task(s) outside the pool: {outside}")
    nonbare = sorted({f"{s.task_id}:{s.arm}" for s in plan if s.arm != "bare"})
    if nonbare:
        out.append(f"the plan has non-bare arm(s) {nonbare}: a live task runs the bare arm only "
                   f"(run._run_one would refuse it only after the batch started)")
    if not out:
        want = sorted((t, m, "bare", r) for t in ids for m in MODELS for r in range(REPS))
        if sorted(tuple(s) for s in plan) != want:
            out.append(f"the plan is not the screen's {len(want)} runs (every pool task x "
                       f"{list(MODELS)} x {REPS} reps, bare); it has {len(plan)}")
    return out


def write_plan(plan, path):
    Path(path).write_text(json.dumps([list(s) for s in plan]) + "\n")


# --- per task bundle ---------------------------------------------------------------------------

def _control_path(data, tid):
    return Path(data) / "live" / "controls-batch" / tid / "control.json"


def runner_bundle(runner_root, data, tid) -> Path:
    """The bundle.json the runner opens for `tid` (run.load_task_from_bundle: <tasks root>/<id>/
    bundle.json, tasks.DATA / "tasks" by default). It must exist and be the very file of the
    frozen `<data>/live-tasks/<id>/bundle.json` (a symlink tasks/<id> -> live-tasks/<id> is the
    intended arrangement); Refused otherwise, naming the task and the path."""
    root = Path(runner_root) if runner_root is not None else tasks.DATA / "tasks"
    rp = root / tid / "bundle.json"
    frozen_bj = Path(data) / "live-tasks" / tid / "bundle.json"
    if not rp.is_file():
        raise Refused(f"{tid}: the runner would open {rp}, which does not exist: link "
                      f"{root / tid} -> {frozen_bj.parent}")
    if os.path.realpath(rp) != os.path.realpath(frozen_bj):
        raise Refused(f"{tid}: the runner would open {rp} (real path {os.path.realpath(rp)}), "
                      f"not the frozen bundle {os.path.realpath(frozen_bj)}")
    return rp


def task_entry(data, entry, inspect, runner_root=None) -> dict:
    """The registered record of one pool task; Refused when its bundle, control or images do not
    fit the pool entry. Every byte the runner reads is read through the runner's own path."""
    tid = entry["id"]
    bj = runner_bundle(runner_root, data, tid)
    try:
        bundle = json.loads(bj.read_text())
    except (OSError, ValueError) as e:
        raise Refused(f"{tid}: bundle.json unreadable ({e})")
    if bundle.get("kind") != mt.KIND:
        raise Refused(f"{tid}: bundle kind {bundle.get('kind')!r}, not {mt.KIND!r}")
    if "template" in bundle:
        raise Refused(f"{tid}: a live bundle has no template")
    cj = _control_path(data, tid)
    try:
        csha = sha256_file(cj)
    except OSError as e:
        raise Refused(f"{tid}: control.json unreadable ({e})")
    if csha != entry["control_sha256"] or bundle.get("control_sha256") != entry["control_sha256"]:
        raise Refused(f"{tid}: control.json sha256 {csha}, bundle {bundle.get('control_sha256')}, "
                      f"pool {entry['control_sha256']}: the control is not the pooled one")
    try:
        record_path = Path(bundle["record_path"])
        record = json.loads(record_path.read_text())
        exp = mt.relaxed_expected(record, cj)  # raises unless the control is relaxed ok
    except (mt.LiveRefused, OSError, ValueError, KeyError) as e:
        raise Refused(f"{tid}: the control is not relaxed ok: {type(e).__name__}: {e}")
    if bundle.get("expected") != exp:
        raise Refused(f"{tid}: bundle expected {bundle.get('expected')} differs from the control's {exp}")
    if (exp["fail_to_pass"] + exp["pass_to_pass_kept"], exp["pass_to_pass_dropped"]) != \
            (entry["expected_count"], entry["dropped_count"]):
        raise Refused(f"{tid}: the control gives {exp['fail_to_pass'] + exp['pass_to_pass_kept']} "
                      f"expected / {exp['pass_to_pass_dropped']} dropped, the pool says "
                      f"{entry['expected_count']} / {entry['dropped_count']}")
    try:
        arm = tasks.bundle_run_image(bj, inspect)  # the exec-only check lives there
    except (OSError, RuntimeError) as e:
        raise Refused(f"{tid}: arm image / exec-only check failed: {e}")
    run_tag = bundle["run_image"]
    try:
        # what the agent sees (run.load_task_from_bundle reads <out>/prompt.txt) and how it is
        # graded (the grader reads record_path); gold.patch is read by neither (written only by
        # assemble_live), so it is not hashed
        prompt_sha = sha256_file(Path(bundle["out"]) / "prompt.txt")
        record_sha = sha256_file(record_path)
    except (OSError, KeyError) as e:
        raise Refused(f"{tid}: prompt or record unreadable: {e}")
    return {"task_id": tid, "kind": mt.KIND,
            "run_image": {"tag": run_tag, "id": inspect(run_tag)[0]},
            "arm_image": {"tag": arm, "id": inspect(arm)[0]},
            "bundle_sha256": sha256_file(bj),
            "prompt_sha256": prompt_sha,
            "record_sha256": record_sha,
            # relaxed_expected reads the control's logs, which control_sha256 (control.json only)
            # does not cover: freeze their bytes too
            "control_logs_sha256": {n: sha256_file(cj.parent / n) for n in ("base.log", "gold.log")}}


def _runner_root(runner_root):
    return Path(runner_root) if runner_root is not None else tasks.DATA / "tasks"


def _frozen_from_stage3(tool_dir) -> dict:
    path = Path(tool_dir) / STAGE3_REGISTRATION
    try:
        fz = json.loads(path.read_text())["frozen"]
        root, trees = fz["root"], fz["trees"]
        frozen.check_trees(root, trees)
    except (OSError, ValueError, KeyError, frozen.Mismatch) as e:
        raise Refused(f"frozen inputs from {path}: {type(e).__name__}: {e}")
    return {"root": root, "trees": trees}


def _go_template(reg, tool_dir, run_data, plan_path, out, python):
    b = reg["budget"]
    go = {"cap_usd": b["cap_usd"], "per_call_usd": b["per_call_usd"],
          "reserve_usd": b["reserve_usd"], "spent_usd": 0.0,
          "plan_sha256": reg["plan_sha256"], "registration_sha256": reg["registration_sha256"],
          "authorized_by": "operator", "acknowledged_stops": []}
    cmd = (f"{python} {Path(tool_dir).resolve() / 'run.py'} run --plan {Path(plan_path).resolve()} "
           f"--data {Path(run_data).resolve()} --registration {Path(out).resolve()} "
           f"--cap {b['cap_usd']} --spent 0.0 --per-call {b['per_call_usd']} "
           f"--reserve {b['reserve_usd']}")
    return {"GO.json": go, "command": cmd,
            "note": "the operator writes GO.json (this script never does); a screen is calibration "
                    "data, not a comparison; if a previous batch ended in a stop, list its id in "
                    "acknowledged_stops"}


# --- freeze ------------------------------------------------------------------------------------

def _git_head(tool_dir) -> str:
    return freeze_stage1._git(tool_dir, "rev-parse", "HEAD").strip()


def pool_guard(pool, rel_pool, current_pool=None, dropped=None) -> list:
    """Reasons this pool is not the one freeze and check accept: a pool other than the current one
    (name or path), or a pool listing an id the code has dropped. `current_pool` ((name, path)) and
    `dropped` exist for the tests; the CLI never passes them."""
    name, path = current_pool if current_pool is not None else (POOL_NAME, POOL)
    gone = {d["id"]: d for d in (DROPPED if dropped is None else dropped)}
    out = []
    if pool.get("pool") != name or rel_pool != path:
        out.append(f"pool {pool.get('pool')!r} at {rel_pool!r} is not the current pool {name!r} at "
                   f"{path!r}: an older pool is history, a screen is frozen on the current one only")
    for t in pool["tasks"]:
        if t["id"] in gone:
            out.append(f"{t['id']} is in the pool but was dropped from it ({gone[t['id']]['reason'][:80]}...)")
    return out


def freeze_screen(*, tool_dir, data, run_data, pool_path, plan_path, out, replace=False,
                  seed=SEED, head_fn: Callable = _git_head,
                  inspect: Callable = tasks._inspect_image,
                  run_cli: Callable = freeze_stage1.default_run_cli,
                  python=sys.executable, runner_tasks_root=None, current_pool=None,
                  dropped=None) -> dict:
    tool_dir, data, run_data = Path(tool_dir), Path(data), Path(run_data)
    pool_path, plan_path, out = Path(pool_path), Path(plan_path), Path(out)
    if out.exists() and not replace:
        raise Refused(f"{out} exists: a registration is replaced only with --replace")
    if (run_data / "GO.json").exists():
        raise Refused(f"{run_data / 'GO.json'} exists: freeze before any go, not after")

    try:
        files = freeze_stage1.tracked_files(tool_dir, exclude=REGISTRATION)
        freeze_stage1.check_clean(tool_dir, files, exclude=REGISTRATION)
    except freeze_stage1.Refused as e:
        raise Refused(str(e))
    try:
        rel_pool = pool_path.resolve().relative_to(tool_dir.resolve()).as_posix()
    except ValueError:
        raise Refused(f"{pool_path} is not inside {tool_dir}")
    if rel_pool not in files:
        raise Refused(f"{rel_pool} is not tracked: commit the pool file before the freeze")

    try:
        pool = check_pool(json.loads(pool_path.read_text()))
        plan = run.load_plan(plan_path)
    except (OSError, ValueError) as e:
        raise Refused(f"pool or plan unreadable: {type(e).__name__}: {e}")
    guard = pool_guard(pool, rel_pool, current_pool, dropped)
    if guard:
        raise Refused("; ".join(guard))
    problems = plan_problems(plan, pool)
    if problems:
        raise Refused("; ".join(problems))
    if plan != screen_plan(pool, MODELS, REPS, seed):
        raise Refused(f"the plan is not screen_plan(pool, seed={seed}): same runs, other order or "
                      f"seed")

    entries = [task_entry(data, e, inspect, runner_tasks_root) for e in pool["tasks"]]

    copy = run_data / "setup-manifest.json"
    if not copy.is_file():
        raise Refused(f"{copy} is missing: the runner loads its manifest from there")
    manifest = json.loads(copy.read_text())
    try:
        got_cli = run_cli(manifest["cli"]["path"]).strip()
        if got_cli != manifest["cli"]["version"].strip():
            raise Refused(f"CLI {manifest['cli']['path']} reports {got_cli!r}, the manifest froze "
                          f"{manifest['cli']['version']!r}")
        run._check_cli(manifest)
    except (OSError, KeyError, subprocess.SubprocessError, run.Refused) as e:
        raise Refused(f"CLI check: {type(e).__name__}: {e}")

    body = {
        "stage": "screen",
        "head": head_fn(tool_dir),
        "files": {f: sha256_file(tool_dir / f) for f in files},
        "pool_sha256": sha256_file(pool_path),
        "pool_path": rel_pool,
        "tasks": entries,
        "seed": seed,
        "plan_path": str(plan_path.resolve()),
        "plan_sha256": run.plan_sha256(plan),
        "budget": dict(BUDGET),
        "models": list(MODELS),
        "reps": [["bare", REPS]],
        "data": str(data.resolve()),
        "run_data": str(run_data.resolve()),
        "runner_tasks_root": str(_runner_root(runner_tasks_root).resolve()),
        "manifest_sha256": sha256_file(copy),
        "claude_cli_version": got_cli,
        "claude_cli_sha256": manifest["cli"]["sha256"],
        "frozen": _frozen_from_stage3(tool_dir),
        "frozen_source_sha256": sha256_file(tool_dir / STAGE3_REGISTRATION),
        "python_version": platform.python_version(),
    }
    reg = dict(body, registration_sha256=run.registration_digest(body))

    tmp = out.with_name(f".{out.name}.tmp-{uuid.uuid4().hex[:8]}")
    tmp.write_text(json.dumps(reg, indent=2) + "\n")
    try:  # read it back through the runner's own readers before it is the registration
        loaded = run.load_registration(tmp, plan)
        run.check_registered_budget(loaded, plan, run.check_budget(
            run.Budget(BUDGET["cap_usd"], 0.0, BUDGET["per_call_usd"], BUDGET["reserve_usd"])))
        run._frozen_inputs(loaded, manifest)
    except run.Refused as e:
        tmp.unlink(missing_ok=True)
        raise Refused(f"self-check: the written registration is not what run.py accepts: {e}")
    tmp.replace(out)
    run_data.mkdir(parents=True, exist_ok=True)
    (run_data / GO_TEMPLATE).write_text(
        json.dumps(_go_template(reg, tool_dir, run_data, plan_path, out, python), indent=2) + "\n")
    return reg


# --- check -------------------------------------------------------------------------------------

def check_screen(*, tool_dir, data, run_data, registration, inspect: Callable = tasks._inspect_image,
                 run_cli: Callable = freeze_stage1.default_run_cli, runner_tasks_root=None,
                 pool_path=None, current_pool=None, dropped=None) -> list:
    """Re-hash what the registration froze; the list of reasons it no longer holds ([] = ok)."""
    tool_dir, data, run_data = Path(tool_dir), Path(data), Path(run_data)
    try:
        reg = json.loads(Path(registration).read_text())
    except (OSError, ValueError) as e:
        return [f"registration unreadable: {e}"]
    why = []
    body = {k: v for k, v in reg.items() if k != "registration_sha256"}
    if reg.get("registration_sha256") != run.registration_digest(body):
        why.append("registration_sha256 does not match the registration's content")
    if reg.get("stage") != "screen":
        why.append(f"stage {reg.get('stage')!r}, not 'screen'")
    for key, got in (("data", data), ("run_data", run_data),
                     ("runner_tasks_root", _runner_root(runner_tasks_root))):
        if str(Path(got).resolve()) != reg.get(key):
            why.append(f"{key} {Path(got).resolve()} is not the registered {reg.get(key)}")
    for f, want in (reg.get("files") or {}).items():
        try:
            if sha256_file(tool_dir / f) != want:
                why.append(f"tool file {f} changed after the freeze")
        except OSError:
            why.append(f"tool file {f} is missing")
    try:
        now = set(freeze_stage1.tracked_files(tool_dir, exclude=REGISTRATION))
        if now != set(reg.get("files") or {}):
            why.append(f"tracked tool files differ from the frozen set: "
                       f"{sorted(now ^ set(reg.get('files') or {}))}")
    except freeze_stage1.Refused as e:
        why.append(str(e))
    pool = None
    try:
        pool_file = tool_dir / reg["pool_path"]
        if sha256_file(pool_file) != reg.get("pool_sha256"):
            why.append("the pool file changed after the freeze")
        pool = check_pool(json.loads(pool_file.read_text()))
        why += pool_guard(pool, reg["pool_path"], current_pool, dropped)
        if pool_path is not None and Path(pool_path).resolve() != pool_file.resolve():
            why.append(f"--pool {Path(pool_path).resolve()} is not the registered pool {pool_file.resolve()}")
    except (OSError, ValueError, KeyError, Refused) as e:
        why.append(f"pool: {e}")
    plan = None
    try:
        plan = run.load_plan(reg["plan_path"])
        if run.plan_sha256(plan) != reg.get("plan_sha256"):
            why.append("the plan file changed after the freeze")
        if pool is not None:
            why += plan_problems(plan, pool)
            if not plan_problems(plan, pool) and plan != screen_plan(pool, MODELS, REPS, reg.get("seed")):
                why.append("the plan is not screen_plan(pool, the registered seed)")
    except (OSError, ValueError, KeyError) as e:
        why.append(f"plan: {type(e).__name__}: {e}")
    if reg.get("budget") != BUDGET:
        why.append(f"budget {reg.get('budget')} is not the registered {BUDGET}")
    if pool is not None:
        by_id = {t["task_id"]: t for t in reg.get("tasks") or []}
        for e in pool["tasks"]:
            try:
                now_entry = task_entry(data, e, inspect, runner_tasks_root)
            except Refused as ex:
                why.append(str(ex))
                continue
            if now_entry != by_id.get(e["id"]):
                why.append(f"{e['id']}: bundle or image changed after the freeze")
    fz = reg.get("frozen") or {}
    try:
        frozen.check_trees(fz.get("root"), fz.get("trees"))
    except frozen.Mismatch as ex:
        why.append(str(ex))
    copy = run_data / "setup-manifest.json"
    try:
        if sha256_file(copy) != reg.get("manifest_sha256"):
            why.append("setup-manifest.json changed after the freeze")
        manifest = json.loads(copy.read_text())
        run._check_cli(manifest)
        if manifest["cli"]["sha256"] != reg.get("claude_cli_sha256"):
            why.append("the CLI hash is not the frozen one")
        if run_cli(manifest["cli"]["path"]).strip() != reg.get("claude_cli_version"):
            why.append("the CLI version is not the frozen one")
        run._frozen_inputs(reg, manifest)
    except (OSError, ValueError, KeyError, run.Refused, subprocess.SubprocessError) as ex:
        why.append(f"manifest or CLI: {type(ex).__name__}: {ex}")
    if (run_data / "GO.json").exists():
        try:
            go = json.loads((run_data / "GO.json").read_text())
            if go.get("registration_sha256") != reg.get("registration_sha256"):
                why.append("GO.json names another registration")
        except (OSError, ValueError):
            why.append("GO.json is unreadable")
    return why


# --- cli ---------------------------------------------------------------------------------------

def main(argv=None) -> int:
    here = Path(__file__).resolve().parent
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("action", choices=["pool", "verify-pool", "plan", "freeze", "check"])
    ap.add_argument("--tool-dir", type=Path, default=here)
    ap.add_argument("--data", type=Path, default=tasks.DATA, help="setup-ab data root")
    ap.add_argument("--run-data", type=Path, default=None, help="default <data>/live-screen")
    ap.add_argument("--pool", type=Path, default=None, help=f"default <tool-dir>/{POOL}")
    ap.add_argument("--plan", type=Path, default=None, help=f"default <run-data>/{PLAN_NAME}")
    ap.add_argument("--out", type=Path, default=None, help=f"default <tool-dir>/{REGISTRATION}")
    ap.add_argument("--seed", type=int, default=SEED)
    ap.add_argument("--replace", action="store_true")
    ap.add_argument("--records", type=Path, default=mt.RECORDS)
    a = ap.parse_args(argv)
    run_data = a.run_data or a.data / "live-screen"
    pool_path = a.pool or a.tool_dir / POOL
    plan_path = a.plan or run_data / PLAN_NAME
    out = a.out or a.tool_dir / REGISTRATION
    controls = a.data / "live" / "controls-batch"
    try:
        if a.action == "pool":
            dropped_ids = {d["id"] for d in DROPPED}
            ids = [i for i in SANITY if i not in dropped_ids]
            pool = make_pool(mt._records_by_id(a.records), controls, ids, amends=AMENDS,
                             dropped=DROPPED)
            bad = sanity_mismatches(pool)
            if bad:
                print("MISMATCH (pool file not written):\n  " + "\n  ".join(bad), file=sys.stderr)
                return 1
            if pool_path.exists() and not a.replace:
                raise Refused(f"{pool_path} exists; pass --replace")
            pool_path.write_text(json.dumps(pool, indent=2) + "\n")
            print(json.dumps({"pool": str(pool_path), "tasks": len(pool["tasks"])}))
        elif a.action == "verify-pool":
            why = verify_pool(json.loads(pool_path.read_text()), mt._records_by_id(a.records), controls)
            print(json.dumps({"ok": not why, "reasons": why}))
            return 1 if why else 0
        elif a.action == "plan":
            plan = screen_plan(check_pool(json.loads(pool_path.read_text())), seed=a.seed)
            run_data.mkdir(parents=True, exist_ok=True)
            if plan_path.exists() and not a.replace:
                raise Refused(f"{plan_path} exists; pass --replace")
            write_plan(plan, plan_path)
            print(json.dumps({"plan": str(plan_path), "runs": len(plan),
                              "plan_sha256": run.plan_sha256(plan), "seed": a.seed}))
        elif a.action == "freeze":
            reg = freeze_screen(tool_dir=a.tool_dir, data=a.data, run_data=run_data,
                                pool_path=pool_path, plan_path=plan_path, out=out,
                                replace=a.replace, seed=a.seed)
            print(json.dumps({"registration": str(out), "registration_sha256": reg["registration_sha256"],
                              "plan_sha256": reg["plan_sha256"], "runs": len(run.load_plan(plan_path)),
                              "go_template": str(run_data / GO_TEMPLATE)}))
        else:
            why = check_screen(tool_dir=a.tool_dir, data=a.data, run_data=run_data, registration=out,
                               pool_path=a.pool)
            print(json.dumps({"ok": not why, "reasons": why}))
            return 1 if why else 0
    except (Refused, OSError, ValueError, KeyError, RuntimeError, subprocess.SubprocessError) as e:
        print(f"REFUSED: {type(e).__name__ + ': ' if not isinstance(e, Refused) else ''}{e}",
              file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
