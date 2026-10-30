"""What a Stage 3 registration binds, re-checked against the live host (Task 11 review round 1).

    grader_inputs(task_ids, tasks_root, s5_dir) -> {...}     sha256 of everything a grader reads
    mjs_closure(entry) -> [Path]                             grade.mjs and its local imports
    check(reg, *, data, tool_dir, inspect, s5_dir, ...)      raises Mismatch with every reason

The freeze (freeze_stage2.py) records; run.run_batch (before any side effect), analyze_study
--registration and `freeze_stage2.py --check` (before each part) call check():
  - tool files (I-1): the tracked files of the tool dir (stage3-registration.json excepted)
    are exactly the registered ones, the tree is clean against HEAD (freeze_stage1.check_clean:
    no untracked file, blob ids of the working-tree bytes = HEAD's), and each file's sha256
    equals the registered one;
  - data dir (I-2): `data` resolves to the registered `run_data`, and its setup-manifest.json
    hashes to `manifest_sha256`;
  - images (I-1): every registered run and arm image tag still has the registered id;
  - grader inputs (I-3, `graders=True`): per foreign task record.json, per private task the
    hidden/ tree (frozen.tree_sha256) and manifest.json, and grade.mjs with every local module
    it imports (transitively), read live from the claude-plugins checkout; the grader's S5 dir
    must be the registered one;
  - deep (`deep=True`, --check only): every task entry recomputed (stage3.task_freeze_entry:
    image ids, per-image codescout sha, template dump AND volume hash, prompt, bundle,
    controls, prescan) equals the registered one, and the frozen trees still hash and scan clean.
Stdlib only. Reads files and runs `git`, `docker image inspect` and (deep only) the two
read-only `docker run --rm --network none` checks of stage3.
"""
import json
import re
from pathlib import Path

import freeze_stage1
import frozen
import stage3
import tasks

REGISTRATION = "stage3-registration.json"
_IMPORT = re.compile(r"""(?:\bfrom\s+|\bimport\s*\(\s*|^\s*import\s+)['"](\.{1,2}/[^'"]+)['"]""",
                     re.M)


class Mismatch(RuntimeError):
    """The live host differs from what the registration records."""


def mjs_closure(entry):
    """[entry and every module it imports by a relative specifier, transitively], sorted."""
    seen, todo = set(), [Path(entry).resolve()]
    while todo:
        p = todo.pop()
        if p in seen:
            continue
        if not p.is_file():
            raise Mismatch(f"{p} is imported but missing")
        seen.add(p)
        for spec in _IMPORT.findall(p.read_text(errors="replace")):
            todo.append((p.parent / spec).resolve())
    return sorted(seen)


def grader_inputs(task_ids, tasks_root=None, s5_dir=None):
    """sha256 of every grader input (I-3, operator ruling 2026-10-30; no vendoring)."""
    root = Path(tasks_root) if tasks_root is not None else tasks.DATA / "tasks"
    s5 = Path(s5_dir) if s5_dir is not None else tasks.S5_DIR
    per_task = {}
    for t in task_ids:
        rec = json.loads((root / t / "bundle.json").read_text())
        if rec.get("kind") == "private":
            s5_out = Path(rec["out"]) / "s5" / "tasks" / t
            per_task[t] = {"hidden_tree": frozen.tree_sha256(s5_out / "hidden"),
                           "manifest_json": freeze_stage1.sha256_file(s5_out / "manifest.json")}
        else:
            per_task[t] = {"record_json": freeze_stage1.sha256_file(rec["record_path"])}
    return {"s5_dir": str(s5.resolve()),
            "grade_mjs": {str(p): freeze_stage1.sha256_file(p)
                          for p in mjs_closure(s5 / "grade.mjs")},
            "tasks": per_task}


def _files_reasons(reg, tool_dir):
    try:
        tracked = freeze_stage1.tracked_files(tool_dir, exclude=REGISTRATION)
        freeze_stage1.check_clean(tool_dir, tracked, exclude=REGISTRATION)
    except freeze_stage1.Refused as e:
        return [f"tool files: {e}"]
    want = reg.get("files") or {}
    if set(tracked) != set(want):
        return [f"tool files: tracked {sorted(set(tracked) ^ set(want))} differ from the "
                f"registered set"]
    return [f"tool file {f} changed after the freeze" for f in sorted(want)
            if freeze_stage1.sha256_file(Path(tool_dir) / f) != want[f]]


def check(reg, *, data=None, tool_dir, inspect=tasks._inspect_image, s5_dir=None,
          files=True, graders=True, deep=False, docker=None, tasks_root=None):
    """Raise Mismatch listing every difference between `reg` (a Stage 2 registration) and the
    live host; see the module docstring for what each switch covers (`files=False` only for the
    freeze's own self-check, which has just run check_clean)."""
    why = _files_reasons(reg, tool_dir) if files else []
    if tasks_root is None:
        tasks_root = reg.get("tasks_root")
    if data is not None:
        if Path(data).resolve() != Path(reg["run_data"]).resolve():
            why.append(f"data dir {Path(data).resolve()} is not the registered run_data "
                       f"{reg['run_data']}: the $200 cap counts only that dir's journal")
        manifest = Path(data) / "setup-manifest.json"
        got = freeze_stage1.sha256_file(manifest) if manifest.is_file() else None
        if got != reg["manifest_sha256"]:
            why.append(f"{manifest} hashes {got}, the registration froze {reg['manifest_sha256']}")
    for e in reg["tasks"]:
        for key in ("run_image", "arm_image"):
            got = inspect(e[key]["tag"])[0]
            if got != e[key]["id"]:
                why.append(f"{e['task_id']}: {e[key]['tag']} is {got}, registered {e[key]['id']}")
    if graders:
        s5 = Path(s5_dir) if s5_dir is not None else tasks.S5_DIR
        want = reg["grader_inputs"]
        if str(s5.resolve()) != want["s5_dir"]:
            why.append(f"the graders read S5 from {s5.resolve()}, registered {want['s5_dir']}")
        else:
            try:
                got = grader_inputs([e["task_id"] for e in reg["tasks"]], tasks_root, s5)
            except (OSError, ValueError, KeyError, Mismatch) as e:
                why.append(f"grader inputs unreadable: {e}")
            else:
                for name in sorted(set(got["grade_mjs"]) | set(want["grade_mjs"])):
                    if got["grade_mjs"].get(name) != want["grade_mjs"].get(name):
                        why.append(f"grader module {name} changed after the freeze")
                for t in sorted(want["tasks"]):
                    if got["tasks"].get(t) != want["tasks"][t]:
                        why.append(f"{t}: grader inputs changed after the freeze")
    if deep:
        for e in reg["tasks"]:
            try:
                now = stage3.task_freeze_entry(e["task_id"], tasks_root, docker=docker,
                                               inspect=inspect)
            except (OSError, ValueError, KeyError, RuntimeError) as ex:
                why.append(f"{e['task_id']}: {type(ex).__name__}: {ex}")
                continue
            for k in sorted(set(now) | set(e)):
                if now.get(k) != e.get(k):
                    why.append(f"{e['task_id']}: {k} differs from the registration")
        fz = reg["frozen"]
        try:
            frozen.check_trees(fz["root"], fz["trees"])
        except frozen.Mismatch as ex:
            why.append(str(ex))
        else:
            if frozen.scan_tree(fz["root"])[1]:
                why.append(f"secret scan hit under {fz['root']}")
    if why:
        raise Mismatch("; ".join(why))
