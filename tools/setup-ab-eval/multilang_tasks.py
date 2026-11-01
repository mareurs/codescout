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
import shutil
import subprocess
import sys
import time
import uuid
from pathlib import Path

import arms
import leak_scan
import live_sweep
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


def orphan_sweep_sh(workdir):
    """Shell: delete every `target/*/deps/<stem>.*` (and the extensionless binary) whose dep-info
    `<stem>.d` lists a workspace source, with the fingerprints that carry the stem's hash. A
    dep-info line is `<output path>: <sources>`; a workspace source is a relative path
    (`crates/x/src/lib.rs`) or one under `workdir`, a dependency's is an absolute registry path.
    (The output path is absolute in every file, so it must not be what decides.) Dependency
    artifacts stay. Run from the workdir."""
    wd = workdir.rstrip("/") + "/"
    return ("for d in target/*/deps; do [ -d \"$d\" ] || continue; "
            "for f in \"$d\"/*.d; do [ -e \"$f\" ] || continue; "
            "sed -n '1s/^[^ ]*: //p' \"$f\" | tr ' ' '\\n' "
            f"| grep -qE {shlex.quote('^([^/]|' + wd.replace('.', '\\.') + ')')} || continue; "
            "s=$(basename \"$f\" .d); "
            "rm -rf \"$d/$s\" \"$d/$s\".*; "
            "rm -rf \"$(dirname \"$d\")\"/.fingerprint/*-\"${s##*-}\"; done; done")


def _workspace_clean_and_rebuild(record, workdir):
    """Shell for the `target/` of the raw image. It was built by a third party from a tree with
    the gold fix applied, so its workspace artifacts (build scripts, test binaries, incremental
    caches) hold gold-only text (verified: a gold-only line was in `build-script-build`, in
    `incremental/*.o` and in the gold test's binary). Third-party dependency artifacts are kept
    warm; everything built from the workspace is removed and rebuilt from the base tree:
      1. `cargo clean -p <every workspace package>`, dev and release;
      2. every `incremental/` dir (only workspace crates are compiled incrementally);
      3. `orphan_sweep_sh`: the artifacts of a target that no longer exists at base (the gold
         test), which `cargo clean -p` cannot name;
      4. the record's rebuild commands, offline."""
    packages = ("$(cargo metadata --no-deps --offline --format-version 1 "
                "| grep -o '\"name\":\"[^\"]*\",\"version\":\"' "
                "| sed 's/\"name\":\"\\([^\"]*\\)\".*/\\1/' | sort -u)")
    cmds = [c.replace("\r", " ").replace("\n", " ; ") for c in (record.get("rebuild_cmds") or [])
            if isinstance(c, str) and c.strip()]
    steps = [
        f"PK={packages}",
        '[ -n "$PK" ]',
        "ARGS=$(for p in $PK; do printf ' -p %s' \"$p\"; done)",
        "cargo clean --offline $ARGS",
        "cargo clean --offline --release $ARGS",
        "rm -rf target/*/incremental",
        orphan_sweep_sh(workdir),
        *[f"( {c} )" for c in cmds],
    ]
    return " \\\n && ".join(" " + s if i == 0 else s for i, s in enumerate(steps))


TRACKED_LIST = "/tmp/sab-tracked.lst"
# `.git/info/attributes` outranks the tree's .gitattributes: with these unset, `git add` stores
# the working-copy bytes as they are (no CRLF to LF, no filter), so a blob that is CRLF in the
# base commit stays CRLF (rust-analyzer: 3 files under `text` whose blobs carry CRLF)
GIT_ATTRIBUTES = "* -text -ident -filter"


def reset_sh(base):
    """Shell steps: reset the tree to `base`, remove untracked files (not `target/`), remember the
    base tree hash in `$TREE` and the tracked file list, NUL separated, in TRACKED_LIST (outside
    the workdir). Must run before the `.git` is removed. Run from the workdir."""
    return [f"git reset --hard -q {base}", "git clean -fdq -e target",
            "TREE=$(git rev-parse HEAD^{tree})", f"git ls-files -z > {TRACKED_LIST}"]


def recreate_repo_sh(gits):
    """Shell steps: remove `gits` (the old `.git` paths), start a one-commit repo and prove its tree
    is byte for byte the base tree in `$TREE`. Tracked files that .gitignore matches are added by
    the saved list with `-f` (`git add .` alone drops them: gitbutler's `.env.*`, polars' 38
    files); the attributes above keep every blob as it was. Run from the workdir after `reset_sh`."""
    return [f"rm -rf {gits}", "git init -q",
            "git config user.email diagnostic@example.invalid", "git config user.name Diagnostic",
            "git config core.autocrlf false", "git config core.safecrlf false",
            "mkdir -p .git/info", "printf '/target\\n' >> .git/info/exclude",
            f"printf '{GIT_ATTRIBUTES}\\n' > .git/info/attributes",
            f"git add -f --pathspec-from-file={TRACKED_LIST} --pathspec-file-nul", "git add .",
            f"git commit -qm '{tasks.FOREIGN_ROOT_SUBJECT}'",
            'test "$(git rev-parse HEAD^{tree})" = "$TREE"', f"rm -f {TRACKED_LIST}"]


def live_dockerfile(source_ref, record, workdir):
    """The task image recipe. One RUN, so the chown of `target/` is one layer: `target/` is kept
    (R12: prebuilt output the agent builds on offline) but its workspace artifacts are rebuilt from
    the base tree (`_workspace_clean_and_rebuild`); it is neither tracked nor scanned by tree."""
    base = record["base_commit"]
    if not _COMMIT.match(base):
        raise LiveRefused(f"base_commit is not a hex commit id: {base!r}")
    if not _WORKDIR.match(workdir):
        raise LiveRefused(f"workdir must be /testbed or below it: {workdir!r}")
    wd = shlex.quote(workdir)
    reports = " ".join(shlex.quote(p) for p in dict.fromkeys([f"{workdir}/reports", "/testbed/reports"]))
    gits = " ".join(shlex.quote(p) for p in dict.fromkeys([f"{workdir}/.git", "/testbed/.git"]))
    steps = ["set -eu", "export CARGO_NET_OFFLINE=true", f"cd {wd}", *reset_sh(base)]
    return (f"FROM {source_ref}\nUSER root\n"
            "RUN " + " \\\n && ".join(steps) + " \\\n"
            f" &&{_workspace_clean_and_rebuild(record, workdir)} \\\n"
            f" && rm -rf {reports} \\\n && "
            + " \\\n && ".join(recreate_repo_sh(gits)) + " \\\n"
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


GRADE_PREFIX = "sab-grade-"
GRADE_STEP_TIMEOUT = 3600
_GRADE_ERRORS = (control.ControlError, grade.ParserError, RuntimeError, subprocess.TimeoutExpired,
                 ValueError, OSError)  # ValueError: odd bytes in docker output (UnicodeDecodeError)


def expected_sets(record, bundle):
    """(fail_to_pass, pass_to_pass) the grader demands: the record's lists, the bundle's dropped
    names removed from PASS_TO_PASS. Read from the bundle's stored `expected["dropped"]`, never
    recomputed from a control."""
    dropped = set(bundle["expected"]["dropped"])
    f2p = list(tasks._f2p(record["FAIL_TO_PASS"]))
    p2p = [n for n in tasks._f2p(record["PASS_TO_PASS"]) if n not in dropped]
    return f2p, p2p


def live_grader(record, bundle, image, gdir):
    """`grade(patch, name) -> dict` for a Live task: a fresh container of `image`
    (`--network none`, the arm resources, always removed, `sab-grade-<id>-<name>-<hex>`), the
    agent's patch (when not empty), the agent-touched test files restored and the record's
    `test_patch` applied, the record's rebuild, test and print commands, the record's `log_parser`
    over the log, and `verdict` over the bundle's expected set. A failure of the grader itself
    (parser error, timeout, docker error, a patch that does not apply) is `pass: False` with an
    `error` field, never an exception and never a pass; an empty patch never passes."""
    f2p, p2p = expected_sets(record, bundle)
    exp, all_p2p = bundle["expected"], tasks._f2p(record["PASS_TO_PASS"])
    if (len(f2p) != exp["fail_to_pass"] or len(p2p) != exp["pass_to_pass_kept"]
            or not set(exp["dropped"]) <= set(all_p2p)):
        raise LiveRefused(f"{record['instance_id']}: the bundle's expected set does not match the "
                          f"record (F2P {len(f2p)} vs {exp['fail_to_pass']}, kept P2P {len(p2p)} vs "
                          f"{exp['pass_to_pass_kept']}, dropped within the record's P2P: "
                          f"{set(exp['dropped']) <= set(all_p2p)})")
    workdir = bundle["workdir"]
    if workdir != tasks.WORKDIR:
        raise LiveRefused(f"workdir {workdir!r}; the runner works in {tasks.WORKDIR!r} only")
    test_files = tasks.patch_files(record["test_patch"])
    tid = record["instance_id"]
    gdir = Path(gdir)

    def grade_one(patch, name):
        container = f"{GRADE_PREFIX}{tid}-{name}-{uuid.uuid4().hex[:8]}"
        result = {"pass": False, "name": name, "oom_killed": False, "container": container,
                  "restored": test_files}
        log = ""
        started = time.monotonic()
        steps = []
        try:
            run_dir = gdir / f"{name}-{uuid.uuid4().hex[:8]}"
            run_dir.mkdir(parents=True)
            result["log"] = str(run_dir / "grade.log")
            try:
                pilot.docker("run", "-d", "--name", container, "--network", "none", *arms.RESOURCES,
                             "--cap-drop", "ALL", "--security-opt", "no-new-privileges",
                             "--pids-limit", "4096", image, "sleep", "infinity",
                             timeout=control.START_TIMEOUT)
                if patch:
                    control._exec(container, workdir, "git", "apply", "--binary", "-", data=patch,
                                  timeout=300)
                control._exec(container, workdir, "sh", "-c", tasks.RESTORE_SH, "restore", *test_files,
                              timeout=300)
                control._exec(container, workdir, "git", "apply", "-", data=record["test_patch"],
                              timeout=300)
                # as the control does before every test run: a record may tee into reports/ without
                # a mkdir, and anything the agent shipped under reports/ must not reach the parser
                control._fresh_reports(container, workdir)
                control.rebuild(container, workdir, record, GRADE_STEP_TIMEOUT, steps=steps, state="grade")
                log = control.run_tests(container, workdir, record, GRADE_STEP_TIMEOUT, steps=steps,
                                        state="grade")
                statuses = grade.run_parser(record["log_parser"], log)
                result.update(grade.verdict(statuses, f2p, p2p))
                if not patch and result["pass"]:
                    result.update({"pass": False, "error": "an empty patch passed the tests"})
            finally:
                try:
                    oom = pilot.docker("inspect", "-f", "{{.State.OOMKilled}}", container, check=False,
                                       timeout=120).stdout.strip()
                    result["oom_killed"] = oom == "true"
                except _GRADE_ERRORS:
                    pass
        except _GRADE_ERRORS as e:
            result.update({"pass": False, "error": f"{type(e).__name__}: {str(e)[-300:]}"})
        finally:
            removed, why = control._remove_container(container)
            result["container_removed"] = removed
            if not removed:
                result["pass"] = False
                result["error"] = result.get("error") or f"container not removed: {why}"
        if result["oom_killed"]:
            result["pass"] = False
        result["wall_seconds"] = round(time.monotonic() - started, 1)
        result["steps"] = [{k: s[k] for k in ("name", "returncode")} for s in steps]
        try:
            if "log" in result:
                Path(result["log"]).write_text(log)
                (Path(result["log"]).parent / "grade.json").write_text(
                    json.dumps(dict(result, steps=steps), indent=2) + "\n")
        except OSError as e:
            result.update({"pass": False, "error": f"cannot write the grade evidence: {e}"})
        return result

    return grade_one


def live_grader_from_record(bundle, image, gdir):
    """`live_grader` for a bundle.json dict (what `tasks.grader_for` is handed)."""
    record = json.loads(Path(bundle["record_path"]).read_text())
    return live_grader(record, bundle, image, gdir)



def agent_image_name(task_image):
    if not task_image.startswith(TASK_PREFIX):
        raise ValueError(f"{task_image!r} is not a task image name (sab-task-<id>:<tag>)")
    return AGENT_PREFIX + task_image[len(TASK_PREFIX):]


TOOL_LAYER = "opt/sab"
TOOL_CACHE = tasks.DATA / "live-tool-layer"


def tool_layer_manifest(image):
    """{relative path: sha256} of every file under /opt/sab in `image`, and the sha256 of that
    listing. /opt/sab is the agent-tool layer (`agent-cs.Dockerfile`: codescout, node,
    rust-analyzer and their libraries), added the same way to every task image."""
    r = pilot.docker("run", "--rm", "--network", "none", "--entrypoint", "sh", image, "-c",
                     "cd /opt/sab && find . -type f | sort | xargs sha256sum")
    files = {}
    for line in r.stdout.splitlines():
        digest, _, path = line.partition("  ")
        if digest and path:
            files[path] = digest
    if not files:
        raise RuntimeError(f"{image}: /opt/sab holds no file")
    return files, hashlib.sha256(r.stdout.encode()).hexdigest()


def tool_layer_tree(image, cache=None):
    """(root, manifest, manifest sha256): a copy of the image's /opt/sab on the host, cached by the
    manifest hash (the layer is the same for every task of one build) and verified file by file
    against the manifest, so what is baselined is exactly what the image holds."""
    cache = Path(cache or TOOL_CACHE)
    files, digest = tool_layer_manifest(image)
    dest = cache / digest[:16]
    root = dest / TOOL_LAYER

    def intact():
        return root.is_dir() and all(
            (root / p).is_file() and hashlib.sha256((root / p).read_bytes()).hexdigest() == h
            for p, h in files.items())

    if not intact():
        if dest.exists():
            shutil.rmtree(dest)
        cache.mkdir(parents=True, exist_ok=True)
        tasks.export_tree(image, [TOOL_LAYER], dest)
        if not intact():
            raise RuntimeError(f"the export of /opt/sab from {image} does not match its manifest")
    return root, files, digest


def baseline_tool_layer(needles, tool_root):
    """(NeedleSet, number dropped): `needles` without the SHINGLES that the tool layer's files
    already contain (a generic JS phrase in the node binary is no evidence of a leak). Name and
    path needles are never dropped here. ValueError (leak_scan) when that would drop every
    shingle. The base-tree counts of the NeedleSet are kept."""
    shingles = [n for n in needles.needles if leak_scan.kind_of(n) == "shingle"]
    if not shingles:
        return needles, 0
    kept = set(leak_scan.baseline_needles(shingles, lambda: tasks.tree_files(tool_root)).needles)
    new = [n for n in needles.needles if leak_scan.kind_of(n) != "shingle" or n in kept]
    counts = dict.fromkeys(leak_scan.KINDS, 0)
    for n in new:
        counts[leak_scan.kind_of(n)] += 1
    return needles._replace(needles=new, kept=counts), len(shingles) - len(kept)


def apply_tool_layer(image, needles, cache=None):
    """`baseline_tool_layer` against the real /opt/sab of `image`; returns (needles, info) with
    info = {manifest_sha256, files, dropped: {shingle: n}} for assembly.json."""
    root, files, digest = tool_layer_tree(image, cache)
    new, dropped = baseline_tool_layer(needles, root)
    return new, {"manifest_sha256": digest, "files": files, "dropped": {"shingle": dropped},
                 "baseline": "every file of the agent image's /opt/sab (shingles only)"}


def build_live(record, out, control_json):
    """Assemble, build the agent image, baseline the needles against its tool layer, write
    `<out>/../bundle.json`; returns the TaskBundle.

    The control is read first: a control that is not relaxed ok refuses before any docker work.
    bundle.json keys: kind, out, task_image, image (the agent image until `arm` replaces it by
    the arm image), record_path, workdir, expected, control_sha256; no `template`.
    assembly.json gets `needles.tool_layer_dropped` and `tool_layer` (the manifest sha256s)."""
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
    needles, tool = apply_tool_layer(image, needles)
    rec["needles"].update(kept=needles.kept, tool_layer_dropped=tool["dropped"])
    rec["tool_layer"] = tool
    (out / "assembly.json").write_text(json.dumps(rec, indent=2) + "\n")
    bundle = {"kind": KIND, "out": str(out), "task_image": task_image, "image": image,
              "record_path": str(out / "record.json"), "workdir": workdir, "expected": expected,
              "control_sha256": hashlib.sha256(control_json.read_bytes()).hexdigest()}
    (out.parent / "bundle.json").write_text(json.dumps(bundle, indent=2) + "\n")
    return tasks.TaskBundle(tid, image, prompt, needles,
                            live_grader(record, bundle, image, out / "grades"), "", bundle)


def _base_needles(rec):
    """The needles of a built bundle after the base-tree baseline only (what `assemble_live` made)."""
    record = json.loads(Path(rec["record_path"]).read_text())
    out = Path(rec["out"])
    return record, leak_scan.baseline_needles(leak_scan.needles_for(leak_record(record)),
                                              lambda: tasks.tree_files(out / "baseline-tree" / "testbed"))


def load_live(bundle_json):
    """A TaskBundle read back from bundle.json; the needles are recomputed from the record, the
    saved baseline tree and the agent image's tool layer, as `build_live` made them (as
    `tasks.bundle_for` does for a foreign task)."""
    bundle_json = Path(bundle_json)
    rec = json.loads(bundle_json.read_text())
    if rec.get("kind") != KIND:
        raise LiveRefused(f"{bundle_json}: kind {rec.get('kind')!r}, not {KIND!r}")
    record, needles = _base_needles(rec)
    out = Path(rec["out"])
    root, _, _ = tool_layer_tree(agent_image_name(rec["task_image"]))
    needles, _ = baseline_tool_layer(needles, root)
    return tasks.TaskBundle(record["instance_id"], rec["image"], out / "prompt.txt", needles,
                            live_grader(record, rec, rec["image"], out / "grades"), "", rec)


def record_tool_layer(bundle_json, cache=None):
    """Write `needles.tool_layer_dropped` and `tool_layer` into the assembly.json of a bundle built
    before the tool layer was baselined (the needles themselves are recomputed by `load_live`)."""
    rec = json.loads(Path(bundle_json).read_text())
    _, needles = _base_needles(rec)
    needles, tool = apply_tool_layer(agent_image_name(rec["task_image"]), needles, cache)
    path = Path(rec["out"]) / "assembly.json"
    assy = json.loads(path.read_text())
    assy["needles"].update(kept=needles.kept, tool_layer_dropped=tool["dropped"])
    assy["tool_layer"] = tool
    path.write_text(json.dumps(assy, indent=2) + "\n")
    return tool




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


SWEEP_EXCLUDES = ("/proc", "/sys", "/dev", "/usr/local/cargo", "/usr/local/rustup", "/opt/sab", "/sweep")
SWEEP_TIMEOUT = 4 * 3600
SWEEP_MIN_LITERAL = 30


def gold_literals(record, baseline_root):
    """The gold-only added lines: every stripped added line of `patch` and `test_patch` with at
    least SWEEP_MIN_LITERAL characters that no file of the baseline tree contains. Compiled output
    keeps such a line verbatim, so a byte search finds it where the token scan cannot run."""
    cands = {}
    for key in ("patch", "test_patch"):
        for line in record[key].splitlines():
            if line.startswith("+") and not line.startswith("+++"):
                s = line[1:].strip()
                if len(s) >= SWEEP_MIN_LITERAL:
                    cands.setdefault(s, s.encode())
    for _, data in tasks.tree_files(baseline_root):
        for s in [s for s, b in cands.items() if b in data]:
            del cands[s]
        if not cands:
            break
    return list(cands)


def run_sweep(image, needles, literals, data):
    """live_sweep.py inside `image` as its user (uid 1000), network off; the report is saved as
    `<data>/sweep.json`. Raises when the sweep itself fails (fail closed)."""
    data = Path(data)
    data.mkdir(parents=True, exist_ok=True)
    spec = data / "sweep-input.json"
    spec.write_text(json.dumps({"needles": list(needles), "literals": list(literals),
                                "excludes": list(SWEEP_EXCLUDES)}))
    mounts = []
    for src, dst in ((tasks.HERE / "live_sweep.py", "/sweep/live_sweep.py"),
                     (tasks.HERE / "leak_scan.py", "/sweep/leak_scan.py"), (spec, "/sweep/input.json")):
        mounts += ["-v", f"{src}:{dst}:ro"]
    r = pilot.docker("run", "--rm", "--network", "none", "--cpus", "4", "--memory", "16g",
                     "--cap-drop", "ALL", "--security-opt", "no-new-privileges", *mounts,
                     "--entrypoint", "python3", image, "/sweep/live_sweep.py", "/sweep/input.json", "/",
                     timeout=SWEEP_TIMEOUT)
    report = json.loads(r.stdout)
    (data / "sweep.json").write_text(json.dumps(report, indent=2) + "\n")
    return report


def sweep_reasons(report):
    """Why a sweep report fails the gate: any needle or literal hit anywhere the agent can read,
    and any patch, diff, eval script or gold-named file outside the operating system's own dirs."""
    reasons = []
    if report["hits"]:
        reasons.append(f"sweep: {len(report['hits'])} needle/literal hits, first {report['hits'][0]}")
    stray = [p for p in report["suspicious"] if not p.startswith(live_sweep.SYSTEM_PREFIXES)]
    if stray:
        reasons.append(f"sweep: patch/diff/eval-like files outside system dirs: {stray[:5]}")
    if not report["files"]:
        reasons.append("sweep: 0 files scanned")
    return reasons


def prescan_live(bundle_json, manifest, data):
    """The leak gate on the agent image: tasks.prescan_task (testbed minus target, home, /opt/sab,
    tmp, git objects, config roots) plus the Live sweep (everything the agent user can read under /
    on this filesystem, `target/` included, with the gold-only literals). Writes
    `<data>/prescan.json` with `sweep` and `reasons`; `ok` is True iff `reasons` is empty."""
    bundle_json = Path(bundle_json)
    b = load_live(bundle_json)
    image = agent_image_name(b.record["task_image"])
    out = dict(tasks.prescan_task(b._replace(image=image), manifest, data, image=image))
    reasons = list(out.get("reasons") or [])
    if out.get("control") != "pass":
        reasons.append(f"prescan control {out.get('control')}: {out.get('error')}")
    if out.get("hits"):
        reasons.append(f"prescan: {len(out['hits'])} needle hits in the agent roots")
    record = json.loads(Path(b.record["record_path"]).read_text())
    literals = gold_literals(record, Path(b.record["out"]) / "baseline-tree" / "testbed")
    report = run_sweep(image, b.needles.needles, literals, data)
    reasons += sweep_reasons(report)
    out.update(sweep={k: v for k, v in report.items() if k not in ("hits", "unreadable")}
               | {"hits": report["hits"], "literals": len(literals),
                  "unreadable_count": report["unreadable_count"]},
               reasons=reasons, ok=not reasons)
    (Path(data) / "prescan.json").write_text(json.dumps(out, indent=2) + "\n")
    return out


def _records_by_id(path):
    return {r["instance_id"]: r for r in records_mod.load_records(path, records_mod.DATASET_SHA256)}


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("action", choices=["build", "prescan", "arm", "tool-layer"])
    ap.add_argument("ids", nargs="+")
    ap.add_argument("--records", type=Path, default=RECORDS)
    ap.add_argument("--controls", type=Path, default=CONTROLS)
    ap.add_argument("--root", type=Path, default=ROOT)
    ap.add_argument("--manifest", type=Path, default=MANIFEST)
    ap.add_argument("--max-image-gb", type=float, default=control.MAX_IMAGE_GB)
    args = ap.parse_args(argv)
    by_id = _records_by_id(args.records) if args.action == "build" else {}
    failed = False
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
        elif args.action == "tool-layer":
            t = record_tool_layer(d / "bundle.json")
            print(json.dumps({"id": task_id, "tool_layer_dropped": t["dropped"],
                              "manifest_sha256": t["manifest_sha256"]}), flush=True)
        else:
            r = prescan_live(d / "bundle.json", json.loads(args.manifest.read_text()), d / "prescan")
            print(json.dumps({"id": task_id, "control": r["control"], "hits": len(r.get("hits", [])),
                              "sweep_hits": len(r["sweep"]["hits"]), "ok": r["ok"],
                              "reasons": r["reasons"]}), flush=True)
            failed = failed or not r["ok"]
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
