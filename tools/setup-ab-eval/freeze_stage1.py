"""Freeze Stage 1: write stage1-registration.json, the record the operator's go is bound to.

Commit the registration before the first model call. This script calls no model, reads no token
file, writes no GO.json and touches no container or volume: it reads files, runs `git`, runs
`<claude> --version` and `docker image inspect` (through tasks._inspect_image). Stdlib only.

It refuses (exit 1, named reason) when any tracked file of tools/setup-ab-eval has uncommitted
changes, when `tasks.py verify` has a reason for a Stage 1 or a selected task, when a Stage 1
development task is in selection.json's `selected`, or when a bundle lacks its arm image, its
template, or an arm image that is the exec-only layer over its run image.
"""
import argparse
import hashlib
import json
import platform
import subprocess
import sys
from pathlib import Path
from typing import Callable

import run
import tasks

REGISTRATION = "stage1-registration.json"
PER_CALL_USD = 2.00
# run.py's dry-run holds one call's worst case as the reserve (`--reserve` defaults to --per-call),
# and check_go only compares GO.json to the batch's Budget, so the reserve is the same value.
RESERVE_USD = PER_CALL_USD


class Refused(RuntimeError):
    """The freeze is refused; the message names the reason."""


def sha256_file(path) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def _git(repo, *args) -> str:
    r = subprocess.run(["git", "-C", str(repo), *args], capture_output=True, text=True, timeout=60)
    if r.returncode:
        raise Refused(f"git {' '.join(args)} failed in {repo}: {r.stderr.strip()}")
    return r.stdout


def default_run_cli(path) -> str:
    return subprocess.run([str(path), "--version"], capture_output=True, text=True, timeout=30,
                          check=True).stdout


def tracked_files(tool_dir) -> list:
    """Sorted tracked files of tool_dir (relative to it), without the registration itself."""
    out = _git(tool_dir, "ls-files", "-z", "--", ".").split("\0")
    return sorted(f for f in out if f and f != REGISTRATION)


def check_clean(tool_dir, files):
    """Refused when any listed file differs from HEAD (staged or not). The registration, rewritten
    by this very script, is not part of what it freezes."""
    changed = [f for f in _git(tool_dir, "diff", "--name-only", "--relative", "HEAD", "-z",
                               "--", ".").split("\0") if f and f != REGISTRATION]
    if changed:
        raise Refused(f"uncommitted changes in {sorted(changed)}: commit them before the freeze")
    missing = [f for f in files if not (Path(tool_dir) / f).is_file()]
    if missing:
        raise Refused(f"tracked files missing from the working tree: {missing}")


def _bundle(data, task_id):
    path = Path(data) / "tasks" / task_id / "bundle.json"
    try:
        return path, json.loads(path.read_text())
    except (OSError, ValueError) as e:
        raise Refused(f"{task_id}: bundle.json unreadable ({e})")


def task_entry(data, task_id, inspect) -> dict:
    path, rec = _bundle(data, task_id)
    if not rec.get("arm_image"):
        raise Refused(f"{task_id}: bundle has no arm image; run `tasks.py arm`")
    if not rec.get("template"):
        raise Refused(f"{task_id}: bundle records no Qdrant template (not prepared yet)")
    try:
        arm = tasks.bundle_run_image(path, inspect)  # the exec-only check lives there
    except RuntimeError as e:
        raise Refused(f"{task_id}: arm image / exec-only check failed: {e}")
    run_tag = rec["run_image"]
    return {"task_id": task_id, "kind": rec.get("kind"),
            "run_image": {"tag": run_tag, "id": inspect(run_tag)[0]},
            "arm_image": {"tag": arm, "id": inspect(arm)[0]},
            "template": rec["template"]}


def _digest(body) -> str:
    return hashlib.sha256(json.dumps(body, sort_keys=True, separators=(",", ":"),
                                     ensure_ascii=True).encode()).hexdigest()


def freeze_stage1(*, tool_dir, data, manifest_path, seed, out=None,
                  inspect: Callable = tasks._inspect_image, run_cli: Callable = default_run_cli,
                  verify_fn: Callable = tasks.verify) -> dict:
    tool_dir, data = Path(tool_dir), Path(data)
    dev = list(run.STAGE1_DEV_TASKS)

    sel_path = data / "selection" / "selection.json"
    try:
        selected = list(json.loads(sel_path.read_text())["selected"])
    except (OSError, ValueError, KeyError) as e:
        raise Refused(f"{sel_path} unreadable ({e})")
    both = sorted(set(dev) & set(selected))
    if both:
        raise Refused(f"development task(s) {both} are in selection.json's selected: development "
                      f"tasks are never scored")

    files = tracked_files(tool_dir)
    check_clean(tool_dir, files)

    ids = dev + [s for s in selected if s not in dev]
    reasons = {k: v for k, v in verify_fn(ids, data / "tasks", selected=selected,
                                          inspect=inspect).items() if v}
    if reasons:
        raise Refused(f"tasks.py verify has reasons: {json.dumps(reasons)}")

    manifest = json.loads(Path(manifest_path).read_text())
    got_cli = run_cli(manifest["cli"]["path"]).strip()
    if got_cli != manifest["cli"]["version"].strip():
        raise Refused(f"CLI {manifest['cli']['path']} reports {got_cli!r}, the manifest froze "
                      f"{manifest['cli']['version']!r}")

    entries = [task_entry(data, t, inspect) for t in dev]
    plan = run.stage1_order(seed)
    budget = run.check_budget(run.Budget(run.STAGE1_CAP_USD, 0.0, PER_CALL_USD, RESERVE_USD))

    body = {
        "stage": 1,
        "head": _git(tool_dir, "rev-parse", "HEAD").strip(),
        "files": {f: sha256_file(tool_dir / f) for f in files},
        "manifest_sha256": sha256_file(manifest_path),
        "selection_rule_sha256": sha256_file(tool_dir / "selection-rule.json"),
        "selection_sha256": sha256_file(sel_path),
        "selected": selected,
        "tasks": entries,
        "seed": seed,
        "models": list(run.MODELS),
        "reps": [list(r) for r in run.STAGE1_REPS],
        "order": [list(s) for s in plan],
        "plan_sha256": run.plan_sha256(plan),
        "budget": {"cap_usd": budget.cap_usd, "per_call_usd": budget.per_call_usd,
                   "reserve_usd": budget.reserve_usd},
        "reserve_rule": "run.py dry-run: --reserve defaults to --per-call (one call's worst case)",
        "claude_cli_version": got_cli,
        "codescout": {"source": "setup-manifest.json (version record of the frozen binary; the "
                                "binary was not read or run)",
                      "binary_sha256": manifest["codescout"]["binary_sha256"],
                      "version_json": manifest["codescout"]["version_json"]},
        "python_version": platform.python_version(),
    }
    reg = dict(body, registration_sha256=_digest(body))
    (Path(out) if out else tool_dir / REGISTRATION).write_text(json.dumps(reg, indent=2) + "\n")
    return reg


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--tool-dir", type=Path, default=Path(__file__).resolve().parent)
    ap.add_argument("--data", type=Path, default=tasks.DATA)
    ap.add_argument("--manifest", type=Path, required=True, help="setup-manifest.json")
    ap.add_argument("--seed", type=int, required=True, help="Stage 1 arm-order seed; recorded")
    a = ap.parse_args(argv)
    try:
        reg = freeze_stage1(tool_dir=a.tool_dir, data=a.data, manifest_path=a.manifest, seed=a.seed)
    except Refused as e:
        print(f"REFUSED: {e}", file=sys.stderr)
        return 1
    print(json.dumps({"registration_sha256": reg["registration_sha256"],
                      "plan_sha256": reg["plan_sha256"], "runs": len(reg["order"])}))
    return 0


if __name__ == "__main__":
    sys.exit(main())
