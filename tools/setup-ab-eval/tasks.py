"""Task assembly: for each selected task, the image every arm runs in, its needles and its grader.

    build_private(task_dir, out) -> TaskBundle     an S5 cs-p* task
    build_foreign(record, out)   -> TaskBundle     a SWE-bench Multilingual record
    prepare_task(bundle, manifest, out)            the index (arms.prepare_index) and its Qdrant
                                                   template; returns the bundle on the run image
    TaskBundle = (id, image, prompt_path, needles, grade(patch, name) -> dict, template, record)

PRIVATE (S5). The S5 source is never modified. `assemble_private` copies the task directory,
adds the target bug's issue files to the COPY's `scrub.removePaths` (ruling R5: every
docs/issues/** file present at the parent P that the fix C modifies or deletes, plus every
docs/issues/** file at P in which leak_scan finds a raw needle), and runs S5's own
`buildTask` (build-task.mjs, through node) on the copy. That gives the S5 tree: P as one fresh
commit `task <id>`, no history, the S5 removals (CLAUDE.md, AGENTS.md, .claude/, .codescout/,
.buddy/ at every depth) and the task CLAUDE.md and TASK.md. The task image is s5/codescout (the
image S5 runs codescout tasks in) plus S5's fetched cargo home at /cargo and the tree at
/testbed, warmed with the task's buildCommand under `--network none`.

FOREIGN (SWE-bench). The Codex prepare recipe (agent.Dockerfile: the original .git and target
removed, a fresh one-commit repo `Benchmark base source`, an `agent` user, offline cargo, the
visible test command run once to warm the target), re-parameterised per record and built with
`docker build --network none`, without pilot's module constants (ruling R1). The test command is
the eval_script's (the lines between the test patch and the `Start Test Output` marker, and the
marked command). The prompt is Codex's: PROMPT_PREFIX + the problem statement, CRLF as LF.

BOTH. The agent image is agent-cs.Dockerfile FROM the task image (Task 4). Needles are
leak_scan.needles_for(...) baselined against the clean parent tree: the built S5 tree, or the
task image's /testbed (both without .git and target/). The workspace is /testbed, a plain
repository (git-dir == git-common-dir), never a linked worktree: the companion's SessionStart
marker is not emitted in one (Task 6 carry).

GRADERS. One container per grade, named `sab-grade-<id>-<name>-<hex>`, network none, the arm
resource flags, always removed. Private: S5's driverScript (grade.mjs, through node) runs inside
the container with the hidden files at /hidden, against the image's HEAD as the base commit,
and the verdict is S5's judgeNames/verdict over its result files (SUMMARY_JS mirrors grade.mjs
`summarise`, which is not exported). Foreign: Codex's grade_task flow (the patch, the test paths
restored from HEAD, the record's test_patch, the test command) parsed by baseline.grade_output.

Step 5 (no model call): `python3 tasks.py live <manifest.json> [ids...]` -> per task
setup-ab-data/controls/tasks/<id>.json and the table setup-ab-data/controls/tasks.json.
Stdlib only; docker through pilot.docker / pilot.command.
"""
import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import tempfile
import time
import uuid
from pathlib import Path
from typing import Callable, NamedTuple

import arms
import baseline
import leak_scan
import pilot
import select_tasks
import stack

HERE = Path(__file__).resolve().parent
DATA = Path(os.environ.get("SAB_DATA", "/home/marius/work/claude/setup-ab-data"))
S5_DIR = Path(os.environ.get("SAB_S5_DIR",
                             "/home/marius/work/claude/claude-plugins/effort-steering/spikes/s5"))
S5_TASKS = S5_DIR / "tasks"
S5_CARGO_HOME = Path(os.environ.get("SAB_S5_CARGO_HOME",
                                    "/home/marius/work/claude/s5-work/cargo-home"))
CODESCOUT_SRC = DATA / "build" / "codescout-src-f9b0fc9f8"
DATASET = DATA / "selection" / "inputs" / "dataset.json"
PRIVATE_BASE_IMAGE = "s5/codescout:latest"
WORKDIR = "/testbed"
NET = "sab-tasks-net"
# The index's wall-clock limit. prepare_index's default (2 h) cut off the first codescout-repo
# index (cs-p08, live): the CPU sparse embedder copied from daily use runs at ~5-7 chunks/s.
INDEX_TIMEOUT = 12 * 3600
# build-task.mjs:63, the names S5 removes at every depth.
DROP_NAMES = ("CLAUDE.md", "AGENTS.md", ".claude", ".codescout", ".buddy")
FOREIGN_ROOT_SUBJECT = "Benchmark base source"
CONFIG_SUBJECT = "sab: codescout project config"  # arms.INDEX_ARTIFACTS_COMMIT
PROMPT_PREFIX = ("Resolve the following issue in this repository. Implement the fix and verify it "
                 "using local development checks.\n\n")
START_MARK = ": '>>>>> Start Test Output'"
END_MARK = ": '>>>>> End Test Output'"
# What the leak scan reads from a run image (as the image user, uid 1000, so: what an agent of any
# arm can read) and what it leaves out.
SCAN_PATHS = ("testbed", "home/agent", "opt/sab", "tmp")
SCAN_EXCLUDED = {"testbed/target": "build output (R12): the warm cargo target the image build "
                                   "compiled from the tree itself"}
NOT_SCANNED = {
    "/usr, /cargo, /usr/local/cargo, /usr/local/rustup": "toolchain and third-party crates; "
        "no task-specific content is written there by this study",
    "/root": "not readable by the agent user",
}


class TaskBundle(NamedTuple):
    id: str
    image: str
    prompt_path: Path
    needles: leak_scan.NeedleSet
    grade: Callable[[str, str], dict]
    template: str = ""   # trailing additions: the Qdrant template after prepare_task,
    record: dict = None  # and what rebuilding the grader on another image needs


class PrivateAssembly(NamedTuple):
    id: str
    out: Path
    task: dict           # the source task.json (unchanged)
    def_dir: Path        # the copied S5 task definition, with the issue files removed
    s5_out: Path         # <out>/s5/tasks/<id>: tree/, hidden/, prompt.txt, manifest.json
    tree: Path
    manifest: dict
    needles: leak_scan.NeedleSet
    issue_files: list    # [{path, rule, needles}]
    gold_patch: Path
    prompt_path: Path


# --- small helpers -----------------------------------------------------------------------------

def _git(repo, *args):
    return select_tasks.git_bytes(repo, *args)


def _node(script, *args, timeout=600):
    """Run an ES module snippet; stdout only (warnings on stderr never reach a parsed value)."""
    p = subprocess.run(["node", "--input-type=module", "-e", script, *map(str, args)],
                       capture_output=True, timeout=timeout)
    if p.returncode:
        raise RuntimeError(f"node failed (exit {p.returncode}): {p.stderr.decode(errors='replace')[-4000:]}")
    return p.stdout.decode()


def _s5(module):
    return json.dumps((S5_DIR / module).as_uri())


def _fresh_dir(out):
    out = Path(out)
    if out.exists():
        raise FileExistsError(f"{out} already exists; a build is never merged into")
    out.mkdir(parents=True)
    return out


def tree_files(root):
    """(relative path, bytes) of every file under `root` except the top-level .git (and target/).

    A symlink contributes its target string, as build-task.mjs hashes it; it is never followed.
    """
    root = Path(root)
    for dirpath, dirnames, filenames in os.walk(root):
        rel_dir = Path(dirpath).relative_to(root)
        if rel_dir == Path("."):
            dirnames[:] = [d for d in dirnames if d not in (".git", "target")]
        for name in dirnames + filenames:
            p = Path(dirpath) / name
            if p.is_symlink():
                yield (rel_dir / name).as_posix(), os.readlink(p).encode()
        for name in filenames:
            p = Path(dirpath) / name
            if not p.is_symlink():
                yield (rel_dir / name).as_posix(), p.read_bytes()


def patch_files(patch):
    return re.findall(r"^diff --git a/(\S+) b/", patch, re.M)


def short_tag():
    """The codescout SHA the agent images are built from (the build context's HEAD)."""
    return _git(CODESCOUT_SRC, "rev-parse", "--short=9", "HEAD").decode().strip()


# --- R5: the target bug's issue files ------------------------------------------------------------

def issue_files(task, raw_needles, scratch):
    """R5. [{path, rule, needles}] for the docs/issues/** files at P that must not reach the tree."""
    repo, parent, fix = task["repoPath"], task["parent"], task["fix"]
    at_p = [p for p in _git(repo, "ls-tree", "-r", "--name-only", parent, "--", "docs/issues")
            .decode().splitlines() if p]
    if not at_p:
        return []
    found = {}
    changed = _git(repo, "diff", "--name-only", "--no-renames", parent, fix, "--", "docs/issues")
    for path in changed.decode().splitlines():
        if path in at_p:  # present at P and modified or deleted by C (a rename is D + A here)
            found[path] = {"path": path, "rule": "modified_by_fix", "needles": []}
    scratch = _fresh_dir(scratch)
    archive = _git(repo, "archive", "--format=tar", parent, "--", "docs/issues")
    subprocess.run(["tar", "-x", "--no-same-owner", "-C", str(scratch)], input=archive, check=True)
    # Raw needles on purpose: this is not a gate read but R5's "a needle is in this file". A
    # baselined set could not see them: the baseline is P itself, these files included.
    report = leak_scan.scan([scratch], None, raw_needles, allow_unbaselined=True)
    for hit in report.hits:
        rel = Path(hit.path).relative_to(scratch).as_posix()
        entry = found.setdefault(rel, {"path": rel, "rule": "holds_a_needle", "needles": []})
        if hit.needle not in entry["needles"]:
            entry["needles"].append(hit.needle)
    return sorted(found.values(), key=lambda e: e["path"])


# --- private ---------------------------------------------------------------------------------------

def grader_gold_spec(task):
    """The registered gold pathspec, without the paths the S5 tree does not hold."""
    spec = [f":(exclude,glob)**/{n}" for n in DROP_NAMES if "." in n and not n.startswith(".")]
    spec += [f":(exclude,glob)**/{n}/**" for n in DROP_NAMES if n.startswith(".")]
    return spec + [f":(exclude){p}" for p in task["scrub"]["removePaths"]]


def assemble_private(task_dir, out):
    """Everything of a private task that needs no docker: the S5 build of a copied definition
    with the R5 issue files removed, the baselined needles and the grader's gold patch."""
    task_dir = Path(task_dir)
    out = _fresh_dir(out)
    task = json.loads((task_dir / "task.json").read_text())
    full = _git(task["repoPath"], "diff", "--no-renames", task["parent"], task["fix"]
                ).decode("utf-8", "replace")
    extra = task["hidden"].get("extra", [])
    hidden_texts = {p: (task_dir / "extra-tests" / p).read_text(errors="replace") for p in extra}
    raw = leak_scan.needles_for(task, full, hidden_texts)
    issues = issue_files(task, raw, out / "issue-scan")

    def_dir = out / "def" / task["id"]
    shutil.copytree(task_dir, def_dir, symlinks=True)
    copied = json.loads(json.dumps(task))
    remove = list(copied["scrub"]["removePaths"])
    remove += [i["path"] for i in issues if i["path"] not in remove]
    copied["scrub"]["removePaths"] = remove
    (def_dir / "task.json").write_text(json.dumps(copied, indent=2) + "\n")

    script = (f"import {{buildTask}} from {_s5('build-task.mjs')};"
              "buildTask(process.argv[1], {workRoot: process.argv[2]});")
    _node(script, def_dir, out / "s5")
    s5_out = out / "s5" / "tasks" / task["id"]
    manifest = json.loads((s5_out / "manifest.json").read_text())
    tree = s5_out / "tree"
    left = [i["path"] for i in issues if os.path.lexists(tree / i["path"])]
    if left:
        raise RuntimeError(f"{task['id']}: issue files survived the build: {left}")
    needles = leak_scan.baseline_needles(raw, lambda: tree_files(tree))
    gold = out / "gold.patch"
    gold.write_text(select_tasks.private_gold_patch(copied, grader_gold_spec(copied)))
    record = {"id": task["id"], "kind": "private", "source_task_dir": str(task_dir),
              "source_task_json_sha256": hashlib.sha256((task_dir / "task.json").read_bytes()).hexdigest(),
              "s5_dir": str(S5_DIR), "s5_commit": _git(S5_DIR, "rev-parse", "HEAD").decode().strip(),
              "build_task_sha256": hashlib.sha256((S5_DIR / "build-task.mjs").read_bytes()).hexdigest(),
              "issue_files_removed": issues, "removePaths": remove,
              "s5_manifest": {k: manifest[k] for k in ("baseCommit", "treeHash", "hiddenHash", "promptHash")},
              "needles": {"kept": needles.kept, "dropped": needles.dropped,
                          "baseline_files": needles.baseline_files, "baseline_bytes": needles.baseline_bytes,
                          "baseline": "the built S5 tree without .git"},
              "gold_patch": str(gold), "gold_spec": select_tasks.private_gold_spec(copied) + grader_gold_spec(copied)}
    (out / "assembly.json").write_text(json.dumps(record, indent=2) + "\n")
    return PrivateAssembly(task["id"], out, task, def_dir, s5_out, tree, manifest, needles, issues,
                           gold, s5_out / "prompt.txt")


def _pinned_tag(image, name):
    """Tag `image`'s current id as sab-base/<name>:<id12>, so a moved tag cannot change a build."""
    image_id = pilot.docker("image", "inspect", image, "--format", "{{.Id}}").stdout.strip()
    tag = f"sab-base/{name}:{image_id.split(':')[-1][:12]}"
    pilot.docker("tag", image_id, tag)
    return tag, image_id


def _docker_build(context, dockerfile_text, tag, log, *, build_args=(), extra=(), timeout=7200,
                  offline=True):
    (Path(context) / "Dockerfile").write_text(dockerfile_text)
    argv = ["docker", "build", *(["--network", "none"] if offline else []), "-t", tag,
            "-f", str(Path(context) / "Dockerfile")]
    for a in build_args:
        argv += ["--build-arg", a]
    r = pilot.command([*argv, *extra, str(context)], timeout=timeout, check=False)
    Path(log).write_text(r.stdout)
    if r.returncode:
        raise RuntimeError(f"docker build {tag} failed (exit {r.returncode}); see {log}")
    return pilot.docker("image", "inspect", tag, "--format", "{{.Id}}").stdout.strip()


def private_dockerfile(build_command, with_cargo_home):
    cargo = "COPY --chown=1000:1000 cargo-home/ /cargo/\n" if with_cargo_home else ""
    return ("ARG BASE\nFROM ${BASE}\nUSER root\n" + cargo
            + "COPY --chown=1000:1000 tree/ /testbed/\n"
            "USER 1000:1000\nENV HOME=/home/agent CARGO_NET_OFFLINE=true\nWORKDIR /testbed\n"
            f"RUN {json.dumps(build_command)}\n"
            'RUN test -z "$(git status --porcelain)" && test "$(git rev-parse --git-dir)" = .git\n')


def build_private_task_image(a, tag, cargo_home=S5_CARGO_HOME):
    ctx = _fresh_dir(a.out / "image-ctx")
    subprocess.run(["cp", "-a", "--reflink=auto", str(a.tree), str(ctx / "tree")], check=True)
    if cargo_home is not None:
        subprocess.run(["cp", "-a", "--reflink=auto", str(cargo_home), str(ctx / "cargo-home")],
                       check=True)
    base, base_id = _pinned_tag(PRIVATE_BASE_IMAGE, "s5-codescout")
    _docker_build(ctx, private_dockerfile(a.manifest["def"]["buildCommand"], cargo_home is not None),
                  tag, a.out / "task-image-build.log", build_args=[f"BASE={base}"])
    shutil.rmtree(ctx)
    return tag


SUMMARY_JS = """
import {judgeNames, verdict, readResultFile} from %s;
import {readFileSync} from 'node:fs';
import {join} from 'node:path';
// grade.mjs summarise() (not exported), minus the docker-run infra the Python side records.
const m = JSON.parse(readFileSync(process.argv[1], 'utf8'));
const dir = process.argv[2];
const def = m.def;
const read = (n) => readResultFile(join(dir, n));
const number = (n) => { const t = read(n); return t !== null && /^\\d+\\s*$/.test(t) ? Number(t) : null; };
const gs = number('graft.status');
const graft = gs === null ? 'not_run' : gs === 0 ? 'ok' : 'failed';
const bs = graft === 'ok' ? number('build.status') : null;
const build = bs === null ? 'not_run' : bs === 0 ? 'ok' : bs === 124 ? 'timeout' : 'failed';
const step = (n) => { if (build !== 'ok') return null; const s = number(n + '.status');
  return s === null ? null : { status: s, stdout: read(n + '.out') ?? '' }; };
const f2p = judgeNames(def.testRunner, def.f2p.names, step('f2p'));
const p2p = judgeNames(def.testRunner, def.p2p.names, step('p2p'));
const infra = read('done') === null ? 'incomplete' : null;
process.stdout.write(JSON.stringify({ infra, graft, build, f2p, p2p,
  pass: verdict({ infra, graft, build, f2p, p2p }) }));
"""
DRIVER_JS = """
import {driverScript} from %s;
import {readFileSync} from 'node:fs';
const m = JSON.parse(readFileSync(process.argv[1], 'utf8'));
process.stdout.write(driverScript(m, { work: '/testbed', hidden: '/hidden', results: '/results' }));
"""


def _grade_name(task_id, name):
    return f"sab-grade-{task_id}-{name}-{uuid.uuid4().hex[:8]}"


def private_grader(a, image, gdir):
    """grade(patch, name) on `image` with S5's driver; a fresh container per grade."""
    manifest, hidden = a.manifest, a.s5_out / "hidden"
    t = manifest["def"]["timeoutsSec"]
    limit = 3 * 130 + t["build"] + 10 + 2 * (t["test"] + 10) + 300  # grade.mjs limitsOf

    def grade(patch, name):
        run_dir = _fresh_dir(Path(gdir) / f"{name}-{uuid.uuid4().hex[:8]}")
        results = run_dir / "results"
        results.mkdir()
        container = _grade_name(a.id, name)
        pilot.docker("run", "-d", "--name", container, "--network", "none", *arms.RESOURCES,
                     "--cap-drop", "ALL", "--security-opt", "no-new-privileges",
                     "-v", f"{hidden}:/hidden:ro", "-v", f"{results}:/results",
                     image, "sleep", "infinity")
        try:
            base = pilot.docker("exec", "-w", WORKDIR, container, "git", "rev-parse", "HEAD").stdout.strip()
            if patch:
                pilot.docker("exec", "-i", "-w", WORKDIR, container, "git", "apply", "--binary", "-",
                             data=patch)
            m = dict(manifest, baseCommit=base)
            (run_dir / "manifest.json").write_text(json.dumps(m))
            script = _node(DRIVER_JS % _s5("grade.mjs"), run_dir / "manifest.json")
            started = time.monotonic()
            r = pilot.docker("exec", "-w", WORKDIR, container, "sh", "-c", script,
                             timeout=limit, check=False)
            wall = round(time.monotonic() - started, 1)
            oom = pilot.docker("inspect", "-f", "{{.State.OOMKilled}}", container, check=False).stdout.strip()
        finally:
            pilot.docker("rm", "-f", container, check=False)
        (run_dir / "driver.out").write_text(r.stdout)
        summary = json.loads(_node(SUMMARY_JS % _s5("grade.mjs"), run_dir / "manifest.json", results))
        summary.update({"name": name, "exit_code": r.returncode, "wall_seconds": wall,
                        "oom_killed": oom == "true", "base_commit": base, "results": str(results)})
        if oom == "true":
            summary["infra"], summary["pass"] = "oom", False
        (run_dir / "grade.json").write_text(json.dumps(summary, indent=2) + "\n")
        return summary

    return grade


# --- foreign ---------------------------------------------------------------------------------------

def eval_parts(eval_script):
    """(pre, command): the eval_script lines after the test patch's here-document and before the
    Start marker (e.g. nushell's `cargo build`), and the one marked test command."""
    lines = eval_script.splitlines()
    try:
        start, end = lines.index(START_MARK), lines.index(END_MARK)
    except ValueError:
        raise ValueError("eval_script lacks the Start/End Test Output markers")
    body = [x for x in lines[start + 1:end] if x.strip()]
    if len(body) != 1:
        raise ValueError(f"expected one test command between the markers, got {body}")
    heredoc_end = max((i for i, x in enumerate(lines[:start]) if re.fullmatch(r"EOF_\d+", x)),
                      default=None)
    pre = [] if heredoc_end is None else [x for x in lines[heredoc_end + 1:start] if x.strip()]
    return pre, body[0]


def eval_test_command(eval_script):
    return eval_parts(eval_script)[1]


def eval_test_script(eval_script):
    pre, command = eval_parts(eval_script)
    return "\n".join([*pre, command])


def foreign_dockerfile(source_ref, test_script):
    # Codex agent.Dockerfile (tools/reliable-agent-eval, 19dccd9bc; the per-task copies under
    # /tmp/reliable-agent-eval-20261024/*/agent.Dockerfile), with the record's own test command as
    # the warm step and a clean-tree check (Task 4 note: a dirty base is refused by prepare).
    warm = json.dumps(["bash", "-o", "pipefail", "-c", test_script + "\ntrue"])
    return (f"FROM {source_ref}\nUSER root\n"
            "RUN rm -rf /testbed/.git /testbed/target /root/.bashrc /root/.bash_history \\\n"
            " && useradd --non-unique --uid 1000 --gid 1000 --create-home --shell /bin/bash agent \\\n"
            " && cd /testbed && git init -q && git config user.email diagnostic@example.invalid \\\n"
            " && git config user.name Diagnostic && git add . && git commit -qm 'Benchmark base source' \\\n"
            " && chown -R 1000:1000 /testbed /usr/local/cargo\n"
            "ENV HOME=/home/agent CARGO_NET_OFFLINE=true RUSTFLAGS=-Awarnings CARGO_BUILD_JOBS=4\n"
            "USER 1000:1000\nWORKDIR /testbed\n"
            f"RUN {warm}\n"
            'RUN test -z "$(git status --porcelain)" && test "$(git rev-parse --git-dir)" = .git\n')


def _f2p(value):
    return leak_scan._f2p_list(value)


def _source_ref(record):
    image = record["image"]
    digests = json.loads(pilot.docker("image", "inspect", image, "--format", "{{json .RepoDigests}}").stdout)
    if not digests:
        raise RuntimeError(f"{image} has no repo digest; pull it first")
    return digests[0]


RESTORE_SH = ('cd /testbed || exit 1; for p in "$@"; do '
              'if git cat-file -e "HEAD:$p" 2>/dev/null; then git checkout -q HEAD -- "$p" || exit 1; fi; '
              'git clean -fdq -- "$p" || exit 1; done')


def foreign_grader(record, image, gdir):
    test_files = patch_files(record["test_patch"])
    try:
        paths, mode = baseline.restore_paths(test_files), "restore_paths"
    except ValueError:
        paths, mode = test_files, "exact_files"
    expected = _f2p(record["FAIL_TO_PASS"]) + _f2p(record["PASS_TO_PASS"])
    script = eval_test_script(record["eval_script"])

    def grade(patch, name):
        run_dir = _fresh_dir(Path(gdir) / f"{name}-{uuid.uuid4().hex[:8]}")
        container = _grade_name(record["instance_id"], name)
        pilot.docker("run", "-d", "--name", container, "--network", "none", *arms.RESOURCES,
                     "--cap-drop", "ALL", "--security-opt", "no-new-privileges", image,
                     "sleep", "infinity")
        try:
            if patch:
                pilot.docker("exec", "-i", "-w", WORKDIR, container, "git", "apply", "--binary", "-",
                             data=patch)
            pilot.docker("exec", container, "sh", "-c", RESTORE_SH, "restore", *paths)
            pilot.docker("exec", "-i", "-w", WORKDIR, container, "git", "apply", "-",
                         data=record["test_patch"])
            started = time.monotonic()
            r = pilot.docker("exec", "-w", WORKDIR, container, "bash", "-o", "pipefail", "-c", script,
                             timeout=5400, check=False)
            wall = round(time.monotonic() - started, 1)
            oom = pilot.docker("inspect", "-f", "{{.State.OOMKilled}}", container, check=False).stdout.strip()
        finally:
            pilot.docker("rm", "-f", container, check=False)
        (run_dir / "grade.log").write_text(r.stdout)
        try:
            result = baseline.grade_output(r.stdout, r.returncode, expected)
        except ValueError as e:  # fail closed: an unexplained exit or a missing test is no pass
            result = {"pass": False, "error": str(e), "exit_code": r.returncode}
        result.update({"name": name, "restore_mode": mode, "restored": paths, "wall_seconds": wall,
                       "oom_killed": oom == "true", "log": str(run_dir / "grade.log")})
        if oom == "true":
            result["pass"] = False
        result.pop("observed_tests", None)
        (run_dir / "grade.json").write_text(json.dumps(result, indent=2) + "\n")
        return result

    return grade


def export_tree(image, paths, dest, *, excludes=()):
    """tar `paths` (relative to /) out of a container of `image`, as the image user, into dest."""
    dest = Path(dest)
    dest.mkdir(parents=True, exist_ok=True)
    present = pilot.docker("run", "--rm", "--network", "none", "--entrypoint", "sh", image, "-c",
                           'for p in "$@"; do [ -e "/$p" ] && echo "$p"; done; true', "x", *paths
                           ).stdout.split()
    if not present:
        return []
    cmd = ["docker", "run", "--rm", "--network", "none", "--entrypoint", "tar", image, "-C", "/",
           "-cf", "-", *[f"--exclude={e}" for e in excludes], *present]
    with tempfile.TemporaryFile() as errf:
        with subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=errf) as producer:
            untar = subprocess.run(["tar", "-x", "--no-same-owner", "-C", str(dest)],
                                   stdin=producer.stdout, capture_output=True)
            producer.stdout.close()
            producer.wait()
        errf.seek(0)
        err = errf.read().decode(errors="replace")
    if producer.returncode or untar.returncode:
        raise RuntimeError(f"export of {present} from {image} failed: {err[-2000:]} "
                           f"{untar.stderr.decode(errors='replace')[-2000:]}")
    return present


def assemble_foreign(record, out, task_image):
    """The foreign task image (built here), its baseline tree and the baselined needles."""
    out = _fresh_dir(out)
    tid = record["instance_id"]
    source = _source_ref(record)
    script = eval_test_script(record["eval_script"])
    ctx = _fresh_dir(out / "image-ctx")
    _docker_build(ctx, foreign_dockerfile(source, script), task_image, out / "task-image-build.log",
                  timeout=14400)
    base_tree = out / "baseline-tree"
    export_tree(task_image, ["testbed"], base_tree, excludes=["testbed/target", "testbed/.git"])
    raw = leak_scan.needles_for(record)
    needles = leak_scan.baseline_needles(raw, lambda: tree_files(base_tree / "testbed"))
    prompt = out / "prompt.txt"
    prompt.write_text(PROMPT_PREFIX + record["problem_statement"].replace("\r\n", "\n"))
    gold = out / "gold.patch"
    gold.write_text(record["patch"])
    rec = {"id": tid, "kind": "foreign", "source_image": source, "test_script": script,
           "dataset_sha256": hashlib.sha256(DATASET.read_bytes()).hexdigest() if DATASET.exists() else None,
           "needles": {"kept": needles.kept, "dropped": needles.dropped,
                       "baseline_files": needles.baseline_files, "baseline_bytes": needles.baseline_bytes,
                       "baseline": "the task image's /testbed without .git and target/"},
           "gold_patch": str(gold)}
    (out / "assembly.json").write_text(json.dumps(rec, indent=2) + "\n")
    return needles, prompt, rec


# --- the public builders -----------------------------------------------------------------------

def build_agent_image(task_image, tag, log):
    """agent-cs.Dockerfile FROM the task image, built exactly as Task 4 built it (default build
    network): BuildKit keys a RUN's cache on its network mode, so `--network none` would miss the
    cached codescout builder stage and re-run its apt-get. The task image under it was built
    offline; this layer adds only /opt/sab (and python3 if the base lacks it)."""
    ctx = Path(log).parent / "agent-ctx"
    ctx.mkdir(parents=True, exist_ok=True)
    return _docker_build(ctx, (HERE / "agent-cs.Dockerfile").read_text(),
                         tag, log, build_args=[f"BASE={task_image}"],
                         extra=["--build-context", f"codescout_src={CODESCOUT_SRC}"], offline=False)


def grader_for(record, image, gdir):
    if record["kind"] == "private":
        a = load_private(Path(record["out"]))
        return private_grader(a, image, gdir)
    return foreign_grader(json.loads(Path(record["record_path"]).read_text()), image, gdir)


def load_private(out):
    """A PrivateAssembly read back from disk (assemble_private's outputs)."""
    out = Path(out)
    rec = json.loads((out / "assembly.json").read_text())
    task = json.loads(Path(rec["source_task_dir"], "task.json").read_text())
    s5_out = out / "s5" / "tasks" / rec["id"]
    manifest = json.loads((s5_out / "manifest.json").read_text())
    tree = s5_out / "tree"
    raw = leak_scan.needles_for(task, _git(task["repoPath"], "diff", "--no-renames", task["parent"],
                                           task["fix"]).decode("utf-8", "replace"),
                                {p: (Path(rec["source_task_dir"]) / "extra-tests" / p).read_text(errors="replace")
                                 for p in task["hidden"].get("extra", [])})
    needles = leak_scan.baseline_needles(raw, lambda: tree_files(tree))
    return PrivateAssembly(rec["id"], out, task, out / "def" / rec["id"], s5_out, tree, manifest,
                           needles, rec["issue_files_removed"], Path(rec["gold_patch"]),
                           s5_out / "prompt.txt")


def build_private(task_dir, out):
    """Assemble an S5 task, build its task image and agent image; the grader runs on the latter."""
    a = assemble_private(task_dir, out)
    tag = short_tag()
    task_image = build_private_task_image(a, f"sab-task-{a.id}:{tag}")
    image = f"sab-agent-cs-{a.id}:{tag}"
    build_agent_image(task_image, image, a.out / "agent-image-build.log")
    record = {"kind": "private", "out": str(a.out), "task_image": task_image}
    return TaskBundle(a.id, image, a.prompt_path, a.needles, private_grader(a, image, a.out / "grades"),
                      "", record)


def build_foreign(record, out):
    tid = record["instance_id"]
    tag = short_tag()
    task_image = f"sab-task-{tid}:{tag}"
    needles, prompt, rec = assemble_foreign(record, out, task_image)
    out = Path(out)
    (out / "record.json").write_text(json.dumps(record, indent=2) + "\n")
    image = f"sab-agent-cs-{tid}:{tag}"
    build_agent_image(task_image, image, out / "agent-image-build.log")
    rec.update({"out": str(out), "task_image": task_image, "record_path": str(out / "record.json")})
    return TaskBundle(tid, image, prompt, needles, foreign_grader(record, image, out / "grades"), "", rec)


def stack_down_exact():
    """Retire the index-building stack by exact name: sab-qdrant, sab-dense, sab-sparse and
    sab-qv-index. Not stack.stack_down(), whose `sab-` sweep would also remove a concurrent
    grade or template-dump container (found live: Step 5 runs controls beside a prepare)."""
    return stack._remove_exact(pilot.docker, containers=["sab-qdrant", "sab-dense", "sab-sparse"],
                               volumes=[stack.INDEX_VOLUME])


def prepare_task(bundle, manifest, out, network=NET):
    """Onboard and index the agent image on the study stack (arms.prepare_index), keep the index
    as template `sab-tpl-<id>`, and return the bundle on the run image with its grader rebound."""
    run_image = bundle.image.replace("sab-agent-cs-", "sab-run-", 1)
    template = stack.TEMPLATE_PREFIX + bundle.id
    if pilot.docker("volume", "inspect", template, check=False).returncode == 0:
        raise RuntimeError(f"{template} already exists; remove_template() it first")
    urls = stack.stack_up(network, manifest)
    try:
        env = arms.study_mcp_env(manifest["mcp_env"], urls)
        arms.prepare_index(bundle.image, run_image, network, env, Path(out) / "prepare",
                           project=WORKDIR, run=bundle.id, timeout=INDEX_TIMEOUT)
        stack.index_down()
        pilot.docker("volume", "create", template)
        stack.copy_volume(stack.INDEX_VOLUME, template)
    finally:
        stack_down_exact()
    record = dict(bundle.record, run_image=run_image, template=template)
    return bundle._replace(image=run_image, template=template, record=record,
                           grade=grader_for(record, run_image, Path(record["out"]) / "grades"))


# --- Step 5 controls -----------------------------------------------------------------------------

def control_order(ns):
    """`ns` reordered so positive_control's first needle of each kind is one whose planted file
    can only match itself (Task 5 carried minor): a shingle with no kept name among its tokens,
    a path with no kept name in it. Same needles, same counts; only the order changes."""
    names = {n for n in ns.needles if leak_scan.kind_of(n) == "name"}

    def clean(n):
        kind = leak_scan.kind_of(n)
        return kind == "name" or not (set(re.findall(r"[A-Za-z0-9_]+", n)) & names)

    first = {}
    for n in ns.needles:
        kind = leak_scan.kind_of(n)
        if kind not in first and clean(n):
            first[kind] = n
    head = list(first.values())
    return ns._replace(needles=head + [n for n in ns.needles if n not in first.values()])


def history_report(run, root_subject):
    """R12 history check. `run(*argv) -> stdout bytes` runs git in the workspace."""
    def out(*argv):
        return run(*argv).decode().strip()
    git_dir = out("git", "rev-parse", "--path-format=absolute", "--git-dir")
    common = out("git", "rev-parse", "--path-format=absolute", "--git-common-dir")
    every = set(out("git", "cat-file", "--batch-all-objects", "--batch-check=%(objectname)").split())
    reachable = {line.split()[0] for line in out("git", "rev-list", "--objects", "--all").splitlines() if line}
    chain = [line.split() for line in out("git", "rev-list", "--all", "--parents", "--reverse").splitlines()]
    head_chain = out("git", "rev-list", "--reverse", "HEAD").split()
    subjects = [out("git", "log", "-1", "--format=%s", c[0]) for c in chain]
    refs = out("git", "for-each-ref", "--format=%(refname)").split()
    roots = [c[0] for c in chain if len(c) == 1]
    single = (len(roots) == 1 and all(len(c) <= 2 for c in chain)
              and [c[0] for c in chain] == head_chain)
    expected = bool(subjects) and subjects[0] == root_subject and all(s == CONFIG_SUBJECT for s in subjects[1:]) \
        and len(subjects) <= 2
    unreachable = sorted(every - reachable)
    report = {"plain_repo": git_dir == common, "git_dir": git_dir, "commits": len(chain),
              "subjects": subjects, "refs": refs, "objects": len(every),
              "unreachable_objects": unreachable, "single_chain": single,
              "expected_subjects": expected}
    report["ok"] = report["plain_repo"] and single and expected and not unreachable
    return report


DUMP_PY = r"""import json, sys, urllib.request
base = sys.argv[1]
def call(method, path, body=None):
    req = urllib.request.Request(base + path, method=method,
                                 data=None if body is None else json.dumps(body).encode(),
                                 headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=120) as r:
        return json.loads(r.read())
for c in call("GET", "/collections")["result"]["collections"]:
    offset = None
    while True:
        body = {"limit": 256, "with_payload": True, "with_vector": False}
        if offset is not None:
            body["offset"] = offset
        r = call("POST", "/collections/%s/points/scroll" % c["name"], body)["result"]
        for p in r["points"]:
            print(json.dumps({"collection": c["name"], "id": p["id"], "payload": p.get("payload")}))
        offset = r.get("next_page_offset")
        if offset is None:
            break
"""


def dump_template(bundle, dest, network=NET):
    """Every point payload of the task's Qdrant template, as JSONL (a fresh run copy, removed)."""
    run = f"dump-{bundle.id}"
    created = False
    if pilot.docker("network", "inspect", network, check=False).returncode:
        pilot.docker("network", "create", "--internal", network)
        created = True
    try:
        url = stack.qdrant_for_run(bundle.template, run, network=network)
        rest = url.replace(f":{stack.QDRANT_GRPC}", ":6333")
        r = subprocess.run(["docker", "run", "--rm", "--network", network, "--name", f"sab-dump-{bundle.id}",
                            "--entrypoint", "python3", bundle.image, "-c", DUMP_PY, rest],
                           capture_output=True, timeout=1800)
        if r.returncode:
            raise RuntimeError(f"qdrant dump failed: {r.stderr.decode(errors='replace')[-2000:]}")
        Path(dest).write_bytes(r.stdout)
    finally:
        stack.run_down(run)
        if created:
            pilot.docker("network", "rm", network, check=False)
    counts = {}
    for line in Path(dest).read_text().splitlines():
        c = json.loads(line)["collection"]
        counts[c] = counts.get(c, 0) + 1
    return counts


def config_roots(manifest):
    """The full arm's config copy and every plugin dir an arm mounts (same for every task)."""
    roots = [Path(p) for p in manifest["full_config_files"]]
    roots.append(Path(manifest["plugins_cs"]["install_path"]))
    roots += [Path(p["install_path"]) for p in manifest["plugins_full"]]
    return list(dict.fromkeys(roots))


def unlink_broken_links(root, listing):
    """Replace the dangling symlinks under `root` (an export copy) by one listing file.

    leak_scan reads a dangling link as unreadable and refuses the scan (ScanIncomplete), though
    an agent can read nothing through it either: what it can see is the link's path and its
    target string. Those go, one `path -> target` line each, into `listing`, which the caller
    scans; the links themselves are removed from the copy. Returns [(path, target)].
    """
    root = Path(root)
    found = []
    for dirpath, dirnames, filenames in os.walk(root):
        for name in dirnames + filenames:
            p = Path(dirpath) / name
            if p.is_symlink() and not p.exists():
                found.append((p.relative_to(root).as_posix(), os.readlink(p)))
    for rel, _ in found:
        (root / rel).unlink()
    if found:
        Path(listing).write_text("".join(f"{rel} -> {target}\n" for rel, target in found))
    return found

def leak_control(bundle, manifest, data):
    """Scan everything an arm can reach of the run image, the template dump, the git object
    stream and the config copy, with the positive control planted alongside."""
    data = Path(data)
    export = data / "export"
    if export.exists():
        shutil.rmtree(export)
    present = export_tree(bundle.image, SCAN_PATHS, export, excludes=list(SCAN_EXCLUDED))
    objects = data / "git-objects.bin"
    r = subprocess.run(["docker", "run", "--rm", "--network", "none", "-w", WORKDIR, "--entrypoint", "git",
                        bundle.image, "cat-file", "--batch-all-objects", "--batch"],
                       capture_output=True, timeout=1800)
    if r.returncode:
        raise RuntimeError(f"git object stream failed: {r.stderr.decode(errors='replace')[-2000:]}")
    objects.write_bytes(r.stdout)

    def in_image(*argv):
        return subprocess.run(["docker", "run", "--rm", "--network", "none", "-w", WORKDIR,
                               "--entrypoint", argv[0], bundle.image, *argv[1:]],
                              capture_output=True, check=True, timeout=600).stdout

    root_subject = (f"task {bundle.id}" if bundle.record["kind"] == "private" else FOREIGN_ROOT_SUBJECT)
    history = history_report(in_image, root_subject)
    dump = data / "qdrant-dump.jsonl"
    points = dump_template(bundle, dump)
    roots, empty = [], []
    links = data / "broken-symlinks.txt"
    broken = unlink_broken_links(export, links)
    if broken:
        roots.append(links)
    for p in present:
        d = export / p
        (roots if any(f.is_file() or f.is_symlink() for f in d.rglob("*")) else empty).append(d)
    roots.append(objects)
    roots += config_roots(manifest)
    report = leak_scan.positive_control(roots, control_order(bundle.needles), None, qdrant_dump=dump)
    return {
        "hits": [{"path": str(h.path), "needle": h.needle, "offset": h.offset} for h in report.hits],
        "control": "pass",
        "files_scanned": report.files_scanned, "bytes_scanned": report.bytes_scanned,
        "per_root": [list(x) for x in report.per_root], "needle_counts": report.needle_counts,
        "dropped_by_baseline": report.dropped,
        "empty_roots_not_scanned": [str(e) for e in empty],
        "broken_symlinks": {"count": len(broken), "scanned_as": str(links) if broken else None},
        "excluded": SCAN_EXCLUDED, "not_scanned": NOT_SCANNED,
        "qdrant_points": points, "git_object_stream_bytes": objects.stat().st_size,
        "history": history,
    }


def control_task(bundle, manifest, data):
    """Step 5 for one prepared task: leak gate with its control, history, grader controls."""
    data = Path(data)
    data.mkdir(parents=True, exist_ok=True)
    result = {"id": bundle.id, "kind": bundle.record["kind"], "image": bundle.image,
              "image_id": pilot.docker("image", "inspect", bundle.image, "--format", "{{.Id}}").stdout.strip(),
              "template": bundle.template, "prompt_sha256": hashlib.sha256(bundle.prompt_path.read_bytes()).hexdigest()}
    try:
        result["leak"] = leak_control(bundle, manifest, data)
    except leak_scan.ControlFailed as e:
        result["leak"] = {"control": "FAILED", "error": str(e)}
    gold_text = Path(bundle.record.get("gold_patch") or json.loads(
        (Path(bundle.record["out"]) / "assembly.json").read_text())["gold_patch"]).read_text()
    base = bundle.grade("", "control-base")
    gold = bundle.grade(gold_text, "control-gold")
    result["grader"] = {"base": base, "gold": gold,
                        "base_fails": base["pass"] is False and "error" not in base and not base.get("infra"),
                        "gold_passes": gold["pass"] is True}
    leak = result["leak"]
    result["pass"] = (leak.get("control") == "pass" and leak.get("hits") == [] and leak["history"]["ok"]
                      and result["grader"]["base_fails"] and result["grader"]["gold_passes"])
    (data / "controls.json").write_text(json.dumps(result, indent=2) + "\n")
    return result


# --- CLI -----------------------------------------------------------------------------------------

def bundle_for(task_id, root):
    """Build (or, when its record exists, reload) the bundle of one selected task."""
    out = Path(root) / task_id / "build"
    rec_path = Path(root) / task_id / "bundle.json"
    if rec_path.exists():
        rec = json.loads(rec_path.read_text())
        image = rec.get("run_image") or rec["image"]
        if rec["kind"] == "private":
            a = load_private(Path(rec["out"]))
            needles, prompt = a.needles, a.prompt_path
        else:
            record = json.loads(Path(rec["record_path"]).read_text())
            needles = leak_scan.baseline_needles(leak_scan.needles_for(record),
                                                 lambda: tree_files(Path(rec["out"]) / "baseline-tree" / "testbed"))
            prompt = Path(rec["out"]) / "prompt.txt"
        return TaskBundle(task_id, image, prompt, needles, grader_for(rec, image, Path(rec["out"]) / "grades"),
                          rec.get("template", ""), rec)
    if task_id.startswith("cs-"):
        b = build_private(S5_TASKS / task_id, out)
        b = b._replace(record=dict(b.record, gold_patch=str(out / "gold.patch")))
    else:
        records = {r["instance_id"]: r for r in json.loads(DATASET.read_text())}
        b = build_foreign(records[task_id], out)
    rec_path.write_text(json.dumps(dict(b.record, image=b.image), indent=2) + "\n")
    return b


def save_bundle(b, root):
    path = Path(root) / b.id / "bundle.json"
    path.write_text(json.dumps(dict(b.record, image=b.record.get("run_image", b.image),
                                    template=b.template), indent=2) + "\n")


def main(argv=None):
    ap = argparse.ArgumentParser(description="Step 5 of Task 7: build, prepare and control tasks")
    ap.add_argument("action", choices=["build", "prepare", "control", "table"])
    ap.add_argument("manifest", type=Path)
    ap.add_argument("ids", nargs="*")
    args = ap.parse_args(argv)
    manifest = json.loads(args.manifest.read_text())
    root = DATA / "tasks"
    controls = DATA / "controls"
    ids = args.ids or json.loads((DATA / "selection" / "selection.json").read_text())["selected"]
    if args.action == "table":
        rows = {}
        for i in ids:
            p = root / i / "controls" / "controls.json"
            rows[i] = json.loads(p.read_text()) if p.exists() else {"id": i, "pass": False, "missing": True}
        summary = {"model_calls": 0, "token_in_any_container": False,
                   "selection": json.loads((DATA / "selection" / "selection.json").read_text())["selected"],
                   "tasks": [{"id": i, "pass": r.get("pass"),
                              "hits": len(r.get("leak", {}).get("hits", []) or []) if r.get("leak") else None,
                              "control": r.get("leak", {}).get("control"),
                              "history_ok": r.get("leak", {}).get("history", {}).get("ok"),
                              "base_fails": r.get("grader", {}).get("base_fails"),
                              "gold_passes": r.get("grader", {}).get("gold_passes")} for i, r in rows.items()],
                   "excluded_from_scan": SCAN_EXCLUDED, "not_scanned": NOT_SCANNED, "detail": rows}
        failed = json.loads((DATA / "selection" / "selection.json").read_text()).get("failed", [])
        summary["replaced"] = {
            i: json.loads((root / i / "controls" / "controls.json").read_text())
            for i in failed if (root / i / "controls" / "controls.json").exists()}
        controls.mkdir(parents=True, exist_ok=True)
        (controls / "tasks.json").write_text(json.dumps(summary, indent=2) + "\n")
        print(json.dumps(summary["tasks"], indent=2))
        return
    for task_id in ids:
        (root / task_id).mkdir(parents=True, exist_ok=True)
        b = bundle_for(task_id, root)
        if args.action == "prepare" and not b.template:
            pilot.docker("network", "create", "--internal", NET, check=False)
            b = prepare_task(b, manifest, root / task_id)
            save_bundle(b, root)
        if args.action == "control":
            r = control_task(b, manifest, root / task_id / "controls")
            print(json.dumps({"id": task_id, "pass": r["pass"]}), flush=True)
        else:
            print(json.dumps({"id": task_id, "image": b.image, "template": b.template}), flush=True)


if __name__ == "__main__":
    main()
