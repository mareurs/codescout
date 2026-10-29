"""Stage 3 plans and the per-task freeze pieces (final review I-4, ledger T10b M4 and the
arm-binary carry). The Stage 2 freeze script (the next task) calls these; nothing here writes a
registration, a GO.json or a plan the runner reads without the operator.

    registered_mapping(ids, tasks_root) -> {id: kind}       each bundle.json's `kind`, never typed
    pilot_split(mapping, seed) -> (pilot, extension)         metadata only: ids and kinds
    plan_part(tasks, seed) -> {...}                          one plan: STAGE3_REPS, both models
    registration_pieces(selected, tasks_root, *, split_seed, pilot_seed, extension_seed)
    arm_binary_sha256(image) -> sha256                       `cat /opt/sab/sha256.txt`, first line
    template_dump_sha256(task_id, tasks_root) -> sha256      the Qdrant dump the leak scan read
    template_volume_sha256(template) -> {sha256, files}      the template volume's bytes, read-only
    task_freeze_entry(task_id, tasks_root) -> {...}          everything above for one task

SPLIT RULE (final review ruling, 2026-10-29). The pilot is 6 of the 12 selected tasks: 3 foreign
and 3 private, drawn by ONE random.Random(split_seed) over task metadata only (id, kind), never
outcomes: for kind in ("foreign", "private"), in that order, rng.sample(sorted ids of that kind,
3). The other 6 are the extension, which runs unconditionally (never conditioned on pilot
results). Each part is a plan of its own: run.make_order(part's tasks in selection order,
run.MODELS, part seed, run.STAGE3_REPS) = 6 tasks x 2 models x (bare x2, cs, full) = 48 specs,
with its own seed, plan hash and budget (run.STAGE3_*: cap 200, spent 0, per-call 5, reserve 5;
one data dir, setup-ab-data/study, whose journal holds the earlier spend).

CLI (writes plan files, prints hashes; $0, no docker):
    python3 stage3.py plans --split-seed N --pilot-seed N --extension-seed N --out <dir>
then `python3 run.py dry-run --plan <dir>/pilot-plan.json --stage3 --data <dir>/dry-pilot`.
"""
import argparse
import hashlib
import json
import random
import re
import uuid
from pathlib import Path

import analyze
import pilot
import run
import tasks

PER_KIND = 3
KINDS = analyze.KINDS
SPLIT_RULE = ("pilot = for kind in (foreign, private): random.Random(split_seed).sample(sorted "
              "ids of that kind, 3) with one rng across both kinds; extension = the other 6; "
              "metadata only (id, kind), never outcomes; the extension runs unconditionally")
SHA_LINE = re.compile(r"([0-9a-f]{64})\s+\S*/bin/codescout$")
TEMPLATE_HASH_IMAGE = run.PROBE_IMAGE  # python:3.12-slim: sha256sum, find, sort, xargs
TEMPLATE_HASH_SCRIPT = ("cd /tpl && find . -type f -print0 | sort -z | xargs -0 -r sha256sum")


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def registered_mapping(task_ids, tasks_root=None):
    """{id: kind} in the given order, each kind read from <tasks_root>/<id>/bundle.json."""
    root = Path(tasks_root) if tasks_root is not None else tasks.DATA / "tasks"
    out = {}
    for t in task_ids:
        if t in out:
            raise ValueError(f"{t} is listed twice")
        rec = json.loads((root / t / "bundle.json").read_text())
        if rec.get("kind") not in KINDS:
            raise ValueError(f"{t}: bundle kind {rec.get('kind')!r} is not one of {KINDS}")
        out[t] = rec["kind"]
    analyze.check_registered(out)
    return out


def pilot_split(mapping, seed):
    """(pilot ids, extension ids), each in `mapping`'s order (see SPLIT RULE)."""
    if isinstance(seed, bool) or not isinstance(seed, int):
        raise ValueError(f"split seed must be an int, got {seed!r}")
    rng = random.Random(seed)
    picked = set()
    for kind in KINDS:
        ids = sorted(t for t, k in mapping.items() if k == kind)
        if len(ids) < 2 * PER_KIND:
            raise ValueError(f"{len(ids)} {kind} tasks; the split needs {2 * PER_KIND}")
        picked |= set(rng.sample(ids, PER_KIND))
    return ([t for t in mapping if t in picked], [t for t in mapping if t not in picked])


def stage3_budget():
    return run.check_budget(run.Budget(run.STAGE3_CAP_USD, 0.0, run.STAGE3_PER_CALL_USD,
                                       run.STAGE3_RESERVE_USD))


def plan_part(task_ids, seed):
    order = run.make_order(task_ids, run.MODELS, seed, run.STAGE3_REPS)
    return {"tasks": list(task_ids), "seed": seed, "models": list(run.MODELS),
            "reps": [list(r) for r in run.STAGE3_REPS], "order": [list(s) for s in order],
            "plan_sha256": run.plan_sha256(order), "budget": stage3_budget()._asdict()}


def registration_pieces(selected, tasks_root=None, *, split_seed, pilot_seed, extension_seed):
    """The Stage 3 part of a registration: the registered mapping, the registered minimum, the
    split and both plans. Refuses unless 12 tasks, 6 per kind, and the parts partition them."""
    mapping = registered_mapping(selected, tasks_root)
    counts = {k: sum(v == k for v in mapping.values()) for k in KINDS}
    if len(mapping) != 12 or counts != {"foreign": 6, "private": 6}:
        raise ValueError(f"expected 12 selected tasks, 6 per kind; got {counts}")
    if len({split_seed, pilot_seed, extension_seed}) != 3:
        raise ValueError("the split, pilot and extension seeds must differ")
    pilot_ids, ext_ids = pilot_split(mapping, split_seed)
    parts = {"pilot": plan_part(pilot_ids, pilot_seed),
             "extension": plan_part(ext_ids, extension_seed)}
    if parts["pilot"]["plan_sha256"] == parts["extension"]["plan_sha256"]:
        raise ValueError("the two plans are identical")
    return {"registered_tasks": mapping, "registered_min": analyze.REGISTERED_MIN,
            "split": {"rule": SPLIT_RULE, "seed": split_seed, "pilot": pilot_ids,
                      "extension": ext_ids},
            "plans": parts}


# --- per-task freeze pieces (read-only docker) ----------------------------------------------------

def _docker(docker):
    return docker or pilot.docker


def arm_binary_sha256(image, docker=None):
    """The codescout sha256 the arm image itself records: the first line of /opt/sab/sha256.txt,
    read by `docker run --rm --network none --entrypoint cat` (a sab- name, nothing written)."""
    name = f"sab-binhash-{uuid.uuid4().hex[:8]}"
    out = _docker(docker)("run", "--rm", "--name", name, "--network", "none", "--entrypoint",
                          "cat", image, "/opt/sab/sha256.txt", timeout=120).stdout
    first = (out.splitlines() or [""])[0].strip()
    m = SHA_LINE.fullmatch(first)
    if not m:
        raise RuntimeError(f"{image}: /opt/sab/sha256.txt first line is not a codescout sha256: "
                           f"{first!r}")
    print(f"{image}: {first}", flush=True)
    return m.group(1)


def template_dump_sha256(task_id, tasks_root=None):
    """sha256 of the Qdrant dump the leak scan read (<task>/controls/qdrant-dump.jsonl)."""
    root = Path(tasks_root) if tasks_root is not None else tasks.DATA / "tasks"
    return sha256_file(root / task_id / "controls" / "qdrant-dump.jsonl")


def template_volume_sha256(template, docker=None, timeout=3600):
    """{sha256, files} of a template volume's bytes: `sha256sum` of every file in path order, in
    a --rm sab- container with the volume mounted READ-ONLY and no network. The template is never
    written; a run copies it (stack.qdrant_for_run)."""
    if not template.startswith("sab-tpl-"):
        raise ValueError(f"{template!r} is not a study template volume")
    name = f"sab-tplhash-{uuid.uuid4().hex[:8]}"
    out = _docker(docker)("run", "--rm", "--name", name, "--network", "none", "-v",
                          f"{template}:/tpl:ro", "--entrypoint", "sh", TEMPLATE_HASH_IMAGE, "-c",
                          TEMPLATE_HASH_SCRIPT, timeout=timeout).stdout
    lines = [l for l in out.splitlines() if l.strip()]
    if not lines:
        raise RuntimeError(f"{template}: no file listed; an empty or missing template")
    return {"sha256": hashlib.sha256(out.encode()).hexdigest(), "files": len(lines)}


def task_freeze_entry(task_id, tasks_root=None, *, docker=None, inspect=None,
                      with_volume_hash=True):
    """One task's pinned identity for the Stage 2 registration."""
    root = Path(tasks_root) if tasks_root is not None else tasks.DATA / "tasks"
    inspect = inspect or tasks._inspect_image
    bundle = root / task_id / "bundle.json"
    rec = json.loads(bundle.read_text())
    arm = tasks.bundle_run_image(bundle, inspect)  # the exec-only check lives there
    entry = {"task_id": task_id, "kind": rec.get("kind"),
             "run_image": {"tag": rec["run_image"], "id": inspect(rec["run_image"])[0]},
             "arm_image": {"tag": arm, "id": inspect(arm)[0]},
             "arm_binary_sha256": arm_binary_sha256(arm, docker),
             "template": rec["template"],
             "template_dump_sha256": template_dump_sha256(task_id, root),
             "bundle_sha256": sha256_file(bundle),
             "controls_sha256": sha256_file(root / task_id / "controls" / "controls.json")}
    if with_volume_hash:
        entry["template_volume"] = template_volume_sha256(rec["template"], docker)
    if rec.get("kind") == "private":
        entry["prompt_sha256"] = sha256_file(Path(rec["out"]) / "s5" / "tasks" / task_id
                                             / "prompt.txt")
        entry["prescan_sha256"] = sha256_file(root / task_id / "prescan" / "prescan.json")
    else:
        entry["prompt_sha256"] = sha256_file(Path(rec["out"]) / "prompt.txt")
    return entry


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    sub = ap.add_subparsers(dest="action", required=True)
    p = sub.add_parser("plans", help="write the pilot and extension plan files ($0)")
    p.add_argument("--selection", type=Path,
                   default=tasks.DATA / "selection" / "selection.json")
    p.add_argument("--tasks-root", type=Path, default=tasks.DATA / "tasks")
    for flag in ("--split-seed", "--pilot-seed", "--extension-seed"):
        p.add_argument(flag, type=int, required=True)
    p.add_argument("--out", type=Path, required=True)
    b = sub.add_parser("binary", help="print each arm image's codescout sha256 (read-only)")
    b.add_argument("ids", nargs="+")
    b.add_argument("--tasks-root", type=Path, default=tasks.DATA / "tasks")
    a = ap.parse_args(argv)
    if a.action == "binary":
        for t in a.ids:
            arm_binary_sha256(tasks.bundle_run_image(a.tasks_root / t / "bundle.json"))
        return 0
    selected = json.loads(a.selection.read_text())["selected"]
    pieces = registration_pieces(selected, a.tasks_root, split_seed=a.split_seed,
                                 pilot_seed=a.pilot_seed, extension_seed=a.extension_seed)
    a.out.mkdir(parents=True, exist_ok=True)
    for name, part in pieces["plans"].items():
        (a.out / f"{name}-plan.json").write_text(json.dumps(part["order"]) + "\n")
    (a.out / "stage3-pieces.json").write_text(json.dumps(pieces, indent=2) + "\n")
    print(json.dumps({"split": pieces["split"],
                      "plans": {n: {"runs": len(p["order"]), "plan_sha256": p["plan_sha256"]}
                                for n, p in pieces["plans"].items()}}, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
