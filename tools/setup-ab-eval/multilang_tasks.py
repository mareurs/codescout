"""Live (SWE-bench-Live MultiLang Rust) task bundles for the bare-arm screen.

    live_dockerfile(source_ref, record, workdir)     the task image recipe: reset to base, no answer
    relaxed_expected(record, control_json)           the relaxed expected set from the saved control
    assemble_live(record, out, task_image, workdir)  task image, baseline tree, needles, prompt, gold
    build_live(record, out, control_json)            assemble, agent image, bundle.json
    arm_live(bundle_json)                            run image tag + exec-only arm image
    prescan_live(bundle_json, manifest, data)        the leak gate on the agent image

    python3 multilang_tasks.py build|prescan|arm [--manifest M] <id> ...

The image holds the answer: the raw Live image carries the gold fix uncommitted, stale `reports/`
and the later git history. `live_dockerfile` resets the tree to `base_commit`, removes `reports/`
and `.git`, and recreates a one-commit repo `Benchmark base source` (as `tasks.foreign_dockerfile`
does) while keeping `target/`, the prebuilt output the control also used. The expected set is the
control's relaxed rule (spec 2026-11-01): FAIL_TO_PASS plus the PASS_TO_PASS tests that pass in the
saved gold run; the rest is dropped for every arm alike and recorded in bundle.json. No `template`:
a Live bundle is bare-only. Stdlib only; Docker through `pilot.docker`.
"""
import argparse
import hashlib
import json
import re
import shlex
import sys
from pathlib import Path

import arms
import leak_scan
import multilang_control as control
import multilang_grade as grade
import multilang_records as records_mod
import pilot
import tasks

KIND = "live"
RUN_PREFIX = "sab-run-"
AGENT_PREFIX = "sab-agent-cs-"
TASK_PREFIX = "sab-task-"
ARM_PREFIX = "sab-arm-"
ROOT = tasks.DATA / "live-tasks"
RECORDS = tasks.DATA / "live" / "multilang-rust.jsonl"
CONTROLS = tasks.DATA / "live" / "controls-batch"
MANIFEST = tasks.DATA / "study" / "setup-manifest.json"
_COMMIT = re.compile(r"[0-9a-f]{7,64}\Z")
_WORKDIR = re.compile(r"/testbed(?:/[A-Za-z0-9_.-]+)*\Z")


class LiveRefused(ValueError):
    """The control or the record does not allow a Live bundle."""


def live_dockerfile(source_ref, record, workdir):
    """The task image recipe. One RUN, so the chown of `target/` is one layer: `target/` is kept
    (R12: prebuilt output the agent builds on offline) but is neither tracked nor scanned."""
    base = record["base_commit"]
    if not _COMMIT.match(base):
        raise LiveRefused(f"base_commit is not a hex commit id: {base!r}")
    if not _WORKDIR.match(workdir):
        raise LiveRefused(f"workdir must be /testbed or below it: {workdir!r}")
    wd = shlex.quote(workdir)
    reports = " ".join(shlex.quote(p) for p in dict.fromkeys([f"{workdir}/reports", "/testbed/reports"]))
    gits = " ".join(shlex.quote(p) for p in dict.fromkeys([f"{workdir}/.git", "/testbed/.git"]))
    return (f"FROM {source_ref}\nUSER root\n"
            "RUN set -eu \\\n"
            f" && cd {wd} \\\n"
            f" && git reset --hard -q {base} \\\n"
            " && git clean -fdq -e target \\\n"
            f" && rm -rf {reports} \\\n"
            f" && rm -rf {gits} \\\n"
            " && git init -q \\\n"
            " && git config user.email diagnostic@example.invalid \\\n"
            " && git config user.name Diagnostic \\\n"
            " && printf '/target\\n' >> .git/info/exclude \\\n"
            f" && git add . && git commit -qm '{tasks.FOREIGN_ROOT_SUBJECT}' \\\n"
            " && (getent group 1000 >/dev/null || groupadd --gid 1000 agent) \\\n"
            " && (id -u 1000 >/dev/null 2>&1 || useradd --uid 1000 --gid 1000 --create-home "
            "--shell /bin/bash agent) \\\n"
            " && mkdir -p /home/agent \\\n"
            f" && chown -R 1000:1000 {wd} /home/agent \"${{CARGO_HOME:-/usr/local/cargo}}\"\n"
            "ENV HOME=/home/agent CARGO_NET_OFFLINE=true\n"
            f"USER 1000:1000\nWORKDIR {workdir}\n"
            'RUN test -z "$(git status --porcelain)" && test "$(git rev-parse --git-dir)" = .git '
            '&& test "$(git rev-list --count HEAD)" = 1 '
            f'&& test "$(git log -1 --format=%s)" = "{tasks.FOREIGN_ROOT_SUBJECT}" '
            f"&& test ! -e {reports.split()[0]} && test ! -e /testbed/reports\n")


def relaxed_expected(record, control_json):
    """{fail_to_pass, pass_to_pass_kept, pass_to_pass_dropped, dropped} from a saved control.

    The control's own logs (base.log, gold.log beside control.json) go through the record's parser.
    LiveRefused unless: the control is not an error; every FAIL_TO_PASS test is not passing at
    base and passes at gold; the gold repeat shows no mismatch among the kept tests (the control
    keeps at most 10 mismatch names: a longer list cannot be checked and is refused). Kept
    PASS_TO_PASS = those passing at gold; the others are dropped."""
    control_json = Path(control_json)
    ev = json.loads(control_json.read_text())
    if ev.get("id") != record["instance_id"]:
        raise LiveRefused(f"{control_json} is the control of {ev.get('id')!r}, not {record['instance_id']!r}")
    if (ev.get("verdict") or {}).get("error"):
        raise LiveRefused("the control ended in an error; re-run it")
    base = grade.run_parser(record["log_parser"], (control_json.parent / "base.log").read_text())
    gold = grade.run_parser(record["log_parser"], (control_json.parent / "gold.log").read_text())
    f2p, p2p = tasks._f2p(record["FAIL_TO_PASS"]), tasks._f2p(record["PASS_TO_PASS"])
    kept = [n for n in p2p if gold.get(n) == grade.PASS]
    dropped = [n for n in p2p if gold.get(n) != grade.PASS]
    reasons = []
    if not f2p:
        reasons.append("no FAIL_TO_PASS tests")
    if any(base.get(n) == grade.PASS for n in f2p):
        reasons.append("a FAIL_TO_PASS test passes at base")
    if any(gold.get(n) != grade.PASS for n in f2p):
        reasons.append("a FAIL_TO_PASS test does not pass at gold")
    mismatch = ev.get("gold_repeat_mismatch")
    if not isinstance(mismatch, dict):
        reasons.append("the control has no gold repeat")
    else:
        names = list(mismatch.get("names") or [])
        if mismatch.get("count", 0) > len(names):
            reasons.append(f"{mismatch['count']} gold repeat mismatches, only {len(names)} named")
        live = set(f2p) | set(kept)
        if any(n in live for n in names):
            reasons.append("tests differ between two gold runs among the kept tests")
    if reasons:
        raise LiveRefused(f"{record['instance_id']}: not relaxed ok: " + "; ".join(reasons))
    return {"fail_to_pass": len(f2p), "pass_to_pass_kept": len(kept),
            "pass_to_pass_dropped": len(dropped), "dropped": dropped}


_DOCTEST_ID = re.compile(r"Doc-tests .*? - (\S+)(?: \(line \d+\))?(?: - \w+)?\Z")


def needle_name(test_id):
    """The test-function name `leak_scan.needles_for` can read out of a Live test id: the last
    segment after `$` or `::`, without generics. The id shapes seen in the pool: `<nextest binary>
    $<test>` (obscura), `<binary path (target/..)>::<module path>::<test>` (rust-analyzer,
    gitbutler), `unittests src/lib.rs::<path>::<test>`, and a doctest `Doc-tests <crate>::<file> -
    <item path> (line N)[ - compile]`, whose name is the last segment of the item path. A segment
    that is still not a plain identifier is returned as it is: `needles_for` then refuses it."""
    m = _DOCTEST_ID.match(test_id)
    s = re.sub(r"<.*?>", "", (m.group(1) if m else test_id).rsplit("$", 1)[-1])
    return s.split("::")[-1]


def leak_record(record):
    """The record with FAIL_TO_PASS rewritten to `needle_name`s; nothing else of it changes."""
    return dict(record, FAIL_TO_PASS=[needle_name(n) for n in tasks._f2p(record["FAIL_TO_PASS"])])


def assemble_live(record, out, task_image, workdir):
    """The task image (built here), its baseline tree, the baselined needles and the files."""
    out = tasks._fresh_dir(out)
    tid = record["instance_id"]
    source = tasks._source_ref({"image": record["docker_image"]})
    ctx = tasks._fresh_dir(out / "image-ctx")
    tasks._docker_build(ctx, live_dockerfile(source, record, workdir), task_image,
                        out / "task-image-build.log", timeout=14400)
    base_tree = out / "baseline-tree"
    tasks.export_tree(task_image, ["testbed"], base_tree, excludes=["testbed/target", "testbed/.git"])
    raw = leak_scan.needles_for(leak_record(record))
    needles = leak_scan.baseline_needles(raw, lambda: tasks.tree_files(base_tree / "testbed"))
    prompt = out / "prompt.txt"
    prompt.write_text(tasks.PROMPT_PREFIX + record["problem_statement"].replace("\r\n", "\n"))
    gold = out / "gold.patch"
    gold.write_text(record["patch"])
    (out / "record.json").write_text(json.dumps(record, indent=2) + "\n")
    rec = {"id": tid, "kind": KIND, "source_image": source, "workdir": workdir,
           "base_commit": record["base_commit"],
           "needles": {"kept": needles.kept, "dropped": needles.dropped,
                       "baseline_files": needles.baseline_files, "baseline_bytes": needles.baseline_bytes,
                       "baseline": "the task image's /testbed without .git and target/"},
           "gold_patch": str(gold)}
    (out / "assembly.json").write_text(json.dumps(rec, indent=2) + "\n")
    return needles, prompt, rec


def _no_grader(patch, name):
    raise RuntimeError("the Live grader is not built yet (Task 2: multilang_tasks grade)")


def agent_image_name(task_image):
    if not task_image.startswith(TASK_PREFIX):
        raise ValueError(f"{task_image!r} is not a task image name (sab-task-<id>:<tag>)")
    return AGENT_PREFIX + task_image[len(TASK_PREFIX):]


def build_live(record, out, control_json):
    """Assemble, build the agent image, write `<out>/../bundle.json`; returns the TaskBundle.

    The control is read first: a control that is not relaxed ok refuses before any docker work.
    bundle.json keys: kind, out, task_image, image (the agent image until `arm` replaces it by
    the arm image), record_path, workdir, expected, control_sha256; no `template`."""
    control_json = Path(control_json)
    expected = relaxed_expected(record, control_json)
    workdir = json.loads(control_json.read_text()).get("workdir")
    if workdir != tasks.WORKDIR:
        raise LiveRefused(f"{record['instance_id']}: workdir {workdir!r}; the runner works in "
                          f"{tasks.WORKDIR!r} only (run.py hard codes it)")
    tid = record["instance_id"]
    out = Path(out)
    task_image = f"{TASK_PREFIX}{tid}:{tasks.short_tag()}"
    needles, prompt, rec = assemble_live(record, out, task_image, workdir)
    image = agent_image_name(task_image)
    tasks.build_agent_image(task_image, image, out / "agent-image-build.log")
    bundle = {"kind": KIND, "out": str(out), "task_image": task_image, "image": image,
              "record_path": str(out / "record.json"), "workdir": workdir, "expected": expected,
              "control_sha256": hashlib.sha256(control_json.read_bytes()).hexdigest()}
    (out.parent / "bundle.json").write_text(json.dumps(bundle, indent=2) + "\n")
    return tasks.TaskBundle(tid, image, prompt, needles, _no_grader, "", bundle)


def load_live(bundle_json):
    """A TaskBundle read back from bundle.json; the needles are recomputed from the record and the
    saved baseline tree (as `tasks.bundle_for` does for a foreign task)."""
    bundle_json = Path(bundle_json)
    rec = json.loads(bundle_json.read_text())
    if rec.get("kind") != KIND:
        raise LiveRefused(f"{bundle_json}: kind {rec.get('kind')!r}, not {KIND!r}")
    record = json.loads(Path(rec["record_path"]).read_text())
    out = Path(rec["out"])
    needles = leak_scan.baseline_needles(leak_scan.needles_for(leak_record(record)),
                                         lambda: tasks.tree_files(out / "baseline-tree" / "testbed"))
    return tasks.TaskBundle(record["instance_id"], rec["image"], out / "prompt.txt", needles,
                            _no_grader, "", rec)


def arm_live(bundle_json):
    """The image the bare arm runs in: `sab-run-<id>` (a tag of the agent image: a Live task has no
    index) plus the exec-only layer (`sab-arm-<id>`), built once and reused while its label names
    the run image's id. Rewrites bundle.json with `run_image`, `arm_image` and `image` = the arm
    image, without a `template` key. Mirrors tasks.ensure_arm minus its eager grader."""
    bundle_json = Path(bundle_json)
    rec = json.loads(bundle_json.read_text())
    if rec.get("kind") != KIND:
        raise LiveRefused(f"{bundle_json}: kind {rec.get('kind')!r}, not {KIND!r}")
    agent = agent_image_name(rec["task_image"])
    run_image = RUN_PREFIX + agent[len(AGENT_PREFIX):]
    arm = tasks.arm_image_name(run_image)
    pilot.docker("tag", agent, run_image)
    run_id = pilot.docker("image", "inspect", run_image, "--format", "{{.Id}}").stdout.strip()
    have = pilot.docker("image", "inspect", arm, "--format",
                        "{{index .Config.Labels \"" + arms.EXEC_ONLY_LABEL + "\"}}", check=False)
    if have.returncode or have.stdout.strip() != run_id:
        arms.exec_only_image(run_image, arm, Path(rec["out"]) / "arm-ctx")
    rec.pop("template", None)
    rec.update(run_image=run_image, arm_image=arm, image=arm)
    bundle_json.write_text(json.dumps(rec, indent=2) + "\n")
    return rec


def prescan_live(bundle_json, manifest, data):
    """tasks.prescan_task on the agent image: the leak gate with its positive control."""
    b = load_live(bundle_json)
    image = agent_image_name(b.record["task_image"])
    return tasks.prescan_task(b._replace(image=image), manifest, data, image=image)


def _records_by_id(path):
    return {r["instance_id"]: r for r in records_mod.load_records(path, records_mod.DATASET_SHA256)}


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("action", choices=["build", "prescan", "arm"])
    ap.add_argument("ids", nargs="+")
    ap.add_argument("--records", type=Path, default=RECORDS)
    ap.add_argument("--controls", type=Path, default=CONTROLS)
    ap.add_argument("--root", type=Path, default=ROOT)
    ap.add_argument("--manifest", type=Path, default=MANIFEST)
    ap.add_argument("--max-image-gb", type=float, default=control.MAX_IMAGE_GB)
    args = ap.parse_args(argv)
    by_id = _records_by_id(args.records) if args.action == "build" else {}
    for task_id in args.ids:
        control._safe_id(task_id)
        d = args.root / task_id
        d.mkdir(parents=True, exist_ok=True)
        if args.action == "build":
            record = by_id[task_id]
            control.ensure_image(record["docker_image"], marker=d / control.PULLED_MARKER,
                                 max_image_gb=args.max_image_gb)
            b = build_live(record, d / "build", args.controls / task_id / "control.json")
            print(json.dumps({"id": task_id, "image": b.image, "needles": b.needles.kept,
                              "expected": b.record["expected"]["fail_to_pass"]}), flush=True)
        elif args.action == "arm":
            rec = arm_live(d / "bundle.json")
            print(json.dumps({"id": task_id, "arm_image": rec["arm_image"]}), flush=True)
        else:
            r = prescan_live(d / "bundle.json", json.loads(args.manifest.read_text()), d / "prescan")
            print(json.dumps({"id": task_id, "control": r["control"], "hits": len(r.get("hits", []))}),
                  flush=True)


if __name__ == "__main__":
    sys.exit(main())
