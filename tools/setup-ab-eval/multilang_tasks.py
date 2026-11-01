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
import os
import re
import shlex
import shutil
import stat
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


KEEP_IGNORED = ("target", "Cargo.lock", ".venv", "generated-do-not-edit")
IMAGE_DEBRIS = ("/tmp/testdir",)


def debris_sh(paths=None, tmp="/tmp"):
    """Shell: remove the test-run debris of the original image from `tmp`: the fixed IMAGE_DEBRIS
    paths, and every top-level entry of `tmp` that holds a path the agent user (uid 1000) cannot
    read (a file without `o+r`, a directory without `o+rx`), each printed as `Removing-debris`.

    Measured: rust-analyzer's `/tmp/testdir` (63 MB of fixture projects its own test suite made on
    2026-08-12, the image build day) holds 27 zero-byte `*.lock` files, mode 0600 root; gitbutler's
    `/tmp` holds 123 entries of the same day (`.tmpXXXX`, `tmp.XXXX`) of which 55 paths (53 files
    0600, 2 dirs 0700, all root) are unreadable. The export refuses an unreadable path outside the
    exec-only allowlist, and the allowlist stays as it is: what the agent cannot read is removed
    with the directory that holds it, and the list is in the build log. Readable debris stays and
    is scanned by the gate."""
    names = " ".join(shlex.quote(p) for p in (IMAGE_DEBRIS if paths is None else paths))
    fixed = (f"for p in {names}; do [ -e \"$p\" ] && echo \"Removing-debris $p\"; rm -rf \"$p\"; done")
    t = shlex.quote(tmp)
    unreadable = r"\( \( -type d ! -perm -o=rx \) -o \( ! -type d ! -type l ! -perm -o=r \) \)"
    return (f"{fixed} && for e in {t}/* {t}/.[!.]* {t}/..?*; do [ -e \"$e\" ] || [ -L \"$e\" ] || continue; "
            f"if [ -n \"$(find \"$e\" {unreadable} -print -quit)\" ]; then echo \"Removing-debris $e\"; "
            "rm -rf \"$e\"; fi; done")



def reset_sh(base):
    """Shell steps: reset the tree to `base`, remove every untracked file (`-x`: the ignored ones
    too, `-ff`: nested repositories too) except KEEP_IGNORED, remember the base tree hash in
    `$TREE` and the tracked file list, NUL separated, in TRACKED_LIST (outside the workdir). Must
    run before the `.git` is removed. Run from the workdir.

    The raw image is a third party's snapshot taken after running the tests of the gold tree: an
    ignored file such as rust-analyzer's `test-output.log` (564 KB, the gold run's output, naming
    the FAIL_TO_PASS test with `... ok`) is untracked and ignored, so a plain `git clean` kept it,
    and the needle baseline then took its content for part of the base tree. What is kept:
    `target/` (prebuilt output, rebuilt from base by `_workspace_clean_and_rebuild`), `Cargo.lock`
    (ignored in rust-sdk, needed by the offline build), `.venv`, and `generated-do-not-edit`
    (gix-testtools' fixture cache, which the control ran with; the leak gate scans it). The
    removals are printed (`Removing ...`), so they land in task-image-build.log."""
    keep = " ".join(f"-e {shlex.quote(k)}" for k in KEEP_IGNORED)
    return [f"git reset --hard -q {base}", f"git clean -ffdx {keep}",
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
            f" && rm -rf {reports} \\\n && {debris_sh()} \\\n && "
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


F2P_DROP_REASON = "absent at base and not added by the test patch"


def _added_lines(patch):
    return "\n".join(l[1:] for l in patch.splitlines() if l.startswith("+") and not l.startswith("+++"))


def relaxed_expected(record, control_json):
    """{fail_to_pass, pass_to_pass_kept, pass_to_pass_dropped, dropped, fail_to_pass_dropped}
    from a saved control.

    The control's own logs (base.log, gold.log beside control.json) go through the record's parser.

    FAIL_TO_PASS side. A test is KEPT iff (a) the control's base log has it (that run has the test
    patch applied) or (b) its terminal name (`needle_name`: the last `::`/`$` segment, for a doctest
    the item path without `(line N)`) occurs, as a whole word, in the added lines of `test_patch`
    (a test that cannot be in the base log because the test patch does not compile at base). A test
    that is neither exists only in the gold patch: no fix can produce it by name and the prompt
    does not name it, so it is dropped for every arm and recorded as
    `{name, reason: F2P_DROP_REASON}` (rust-sdk-1021: all 9, doctests whose names carry the line
    numbers the gold patch shifts; obscura-447: 1 of 3). A kept test must not pass at base and
    must pass at gold. A task with no kept test is refused.

    LiveRefused unless: the control is not an error; the kept tests satisfy that; the gold repeat
    shows no mismatch among the kept tests (the control keeps at most 10 mismatch names: a longer
    list cannot be checked and is refused). Kept PASS_TO_PASS = those passing at gold; the others
    are dropped."""
    control_json = Path(control_json)
    ev = json.loads(control_json.read_text())
    if ev.get("id") != record["instance_id"]:
        raise LiveRefused(f"{control_json} is the control of {ev.get('id')!r}, not {record['instance_id']!r}")
    if (ev.get("verdict") or {}).get("error"):
        raise LiveRefused("the control ended in an error; re-run it")
    base = grade.run_parser(record["log_parser"], (control_json.parent / "base.log").read_text())
    gold = grade.run_parser(record["log_parser"], (control_json.parent / "gold.log").read_text())
    all_f2p, p2p = tasks._f2p(record["FAIL_TO_PASS"]), tasks._f2p(record["PASS_TO_PASS"])
    added = _added_lines(record["test_patch"])

    def named_by_test_patch(test_id):
        name = needle_name(test_id)
        return bool(name) and re.search(r"(?<![A-Za-z0-9_])" + re.escape(name) + r"(?![A-Za-z0-9_])", added)

    f2p = [n for n in all_f2p if n in base or named_by_test_patch(n)]
    f2p_dropped = [{"name": n, "reason": F2P_DROP_REASON} for n in all_f2p if n not in f2p]
    kept = [n for n in p2p if gold.get(n) == grade.PASS]
    dropped = [n for n in p2p if gold.get(n) != grade.PASS]
    reasons = []
    if not all_f2p:
        reasons.append("no FAIL_TO_PASS tests")
    elif not f2p:
        reasons.append(f"no FAIL_TO_PASS test is expected: all {len(all_f2p)} are {F2P_DROP_REASON}")
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
            "pass_to_pass_dropped": len(dropped), "dropped": dropped,
            "fail_to_pass_dropped": f2p_dropped}


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


def regular_files(root, skipped=None):
    """(relative path, bytes) of every regular file under `root` except the top-level `.git` and
    `target/`, as `tasks.tree_files` yields them, but a FIFO, socket or device is never opened
    (`tasks.tree_files` read a FIFO and blocked for ever on gitbutler's
    `generated-do-not-edit/.../named-pipe`): it is appended to `skipped` as {path, kind} and so
    recorded, not lost. A link contributes its target string and is never followed."""
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
            if p.is_symlink():
                continue
            mode = os.lstat(p).st_mode
            if not stat.S_ISREG(mode):
                if skipped is not None:
                    skipped.append({"path": (rel_dir / name).as_posix(), "kind": live_sweep._kind(mode)})
                continue
            yield (rel_dir / name).as_posix(), p.read_bytes()


def remove_specials(root):
    """Delete every FIFO, socket and device under `root` (an export copy) and return them as
    [{path, kind}]: leak_scan refuses a scan that met a non-regular file, and reading a FIFO would
    block. They hold no content an agent could read; the list goes into the evidence."""
    root = Path(root)
    found = []
    for dirpath, dirnames, filenames in os.walk(root):
        for name in filenames:
            p = Path(dirpath) / name
            mode = os.lstat(p).st_mode
            if not stat.S_ISREG(mode) and not stat.S_ISLNK(mode):
                found.append({"path": p.relative_to(root).as_posix(), "kind": live_sweep._kind(mode)})
                p.unlink()
    return found


_REMOVING = re.compile(r"^(?:#\d+ [\d.]+ )?Removing (.+)$")
_DEBRIS = re.compile(r"^(?:#\d+ [\d.]+ )?Removing-debris (.+)$")


def scrubbed_paths(build_log, pattern=_REMOVING):
    """The paths `git clean` removed in the image build (`Removing <path>` lines of the log), or
    with `_DEBRIS` the /tmp entries `debris_sh` removed (`Removing-debris <path>`)."""
    try:
        text = Path(build_log).read_text(errors="replace")
    except OSError:
        return []
    return [m.group(1) for line in text.splitlines() if (m := pattern.match(line.rstrip()))]



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
    skipped = []
    needles = leak_scan.baseline_needles(raw, lambda: regular_files(base_tree / "testbed", skipped))
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
           "baseline_specials": skipped, "scrubbed": scrubbed_paths(out / "task-image-build.log"),
           "debris_removed": scrubbed_paths(out / "task-image-build.log", _DEBRIS),
           "gold_patch": str(gold)}
    (out / "assembly.json").write_text(json.dumps(rec, indent=2) + "\n")
    return needles, prompt, rec


GRADE_PREFIX = "sab-grade-"
GRADE_STEP_TIMEOUT = 3600
_GRADE_ERRORS = (control.ControlError, grade.ParserError, RuntimeError, subprocess.TimeoutExpired,
                 ValueError, OSError)  # ValueError: odd bytes in docker output (UnicodeDecodeError)


def expected_sets(record, bundle):
    """(fail_to_pass, pass_to_pass) the grader demands: the record's lists with the bundle's
    `fail_to_pass_dropped` names removed from FAIL_TO_PASS (tests only the gold patch has) and the
    bundle's `dropped` names removed from PASS_TO_PASS. Read from the bundle, never recomputed from
    a control; a bundle written before `fail_to_pass_dropped` existed drops no F2P test."""
    exp = bundle["expected"]
    f2p_gone = {d["name"] for d in exp.get("fail_to_pass_dropped", [])}
    dropped = set(exp["dropped"])
    f2p = [n for n in tasks._f2p(record["FAIL_TO_PASS"]) if n not in f2p_gone]
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
    all_f2p = tasks._f2p(record["FAIL_TO_PASS"])
    f2p_gone = {d["name"] for d in exp.get("fail_to_pass_dropped", [])}
    if (not f2p or len(f2p) != exp["fail_to_pass"] or len(p2p) != exp["pass_to_pass_kept"]
            or not set(exp["dropped"]) <= set(all_p2p) or not f2p_gone <= set(all_f2p)
            or len(f2p_gone) != len(exp.get("fail_to_pass_dropped", []))):
        raise LiveRefused(f"{record['instance_id']}: the bundle's expected set does not match the "
                          f"record (F2P {len(f2p)} vs {exp['fail_to_pass']}, kept P2P {len(p2p)} vs "
                          f"{exp['pass_to_pass_kept']}, dropped within the record's P2P: "
                          f"{set(exp['dropped']) <= set(all_p2p)}, dropped F2P within the record's F2P: "
                          f"{f2p_gone <= set(all_f2p)})")
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
    kept = set(leak_scan.baseline_needles(shingles, lambda: regular_files(tool_root)).needles)
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
                                              lambda: regular_files(out / "baseline-tree" / "testbed"))


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
    for _, data in regular_files(baseline_root):
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


def prescan_agent_image(bundle, manifest, data, image):
    """`tasks.prescan_task` for a Live agent image: the same roots (testbed minus target, home,
    /opt/sab, tmp, the git object stream, the config roots) and the same positive control, with
    one difference: the export is cleared of FIFOs, sockets and devices before the scan
    (`remove_specials`; leak_scan refuses a tree holding one and a read of a FIFO would block) and
    they are listed in the evidence as `specials_removed`. `tasks.prescan_task` itself cannot do
    that: it exports and scans in one call. Evidence: `<data>/prescan.json`."""
    data = Path(data)
    if data.exists():
        shutil.rmtree(data)
    data.mkdir(parents=True)
    export = data / "export"
    unreadable = []
    present = tasks.export_tree(image, tasks.SCAN_PATHS, export, excludes=list(tasks.SCAN_EXCLUDED),
                                unreadable=unreadable)
    specials = remove_specials(export)
    objects = data / "git-objects.bin"
    objects.write_bytes(subprocess.run(
        ["docker", "run", "--rm", "--network", "none", "-w", tasks.WORKDIR, "--entrypoint", "git", image,
         "cat-file", "--batch-all-objects", "--batch"], capture_output=True, check=True, timeout=1800).stdout)
    links = data / "broken-symlinks.txt"
    roots = [links] if tasks.unlink_broken_links(export, links) else []
    roots += [export / p for p in present
              if any(f.is_file() or f.is_symlink() for f in (export / p).rglob("*"))]
    roots += [objects] + tasks.config_roots(manifest)
    try:
        report = leak_scan.positive_control(roots, tasks.control_order(bundle.needles), None)
        out = {"id": bundle.id, "image": image, "control": "pass", "unreadable_to_agent": unreadable,
               "reasons": ([f"unreadable to the agent-user export but not exec-only: {bad}"]
                           if (bad := tasks.unreadable_not_allowed(unreadable)) else []),
               "hits": [{"path": str(h.path), "needle": h.needle, "offset": h.offset} for h in report.hits],
               "per_root": [list(x) for x in report.per_root], "needle_counts": report.needle_counts}
    except leak_scan.ControlFailed as e:
        out = {"id": bundle.id, "image": image, "control": "FAILED", "error": str(e)}
    out["specials_removed"] = specials
    (data / "prescan.json").write_text(json.dumps(out, indent=2) + "\n")
    return out



def prescan_live(bundle_json, manifest, data):
    """The leak gate on the agent image: `prescan_agent_image` (testbed minus target, home, /opt/sab,
    tmp, git objects, config roots) plus the Live sweep (everything the agent user can read under /
    on this filesystem, `target/` included, with the gold-only literals). Writes
    `<data>/prescan.json` with `sweep` and `reasons`; `ok` is True iff `reasons` is empty."""
    bundle_json = Path(bundle_json)
    b = load_live(bundle_json)
    image = agent_image_name(b.record["task_image"])
    out = dict(prescan_agent_image(b._replace(image=image), manifest, data, image))
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


def _effective_expected(exp):
    """`expected` with the keys a bundle written before F2P dropping may lack, for comparison."""
    return dict(exp, fail_to_pass_dropped=list(exp.get("fail_to_pass_dropped", [])))


def reexpect(bundle_json, record, control_json, write=True):
    """Recompute the `expected` set of an already built bundle from its saved control (no docker).

    Returns (before, after, status): status `UNCHANGED` (bundle left alone, even if an old bundle
    lacks the `fail_to_pass_dropped` key), `CHANGED` (bundle.json rewritten atomically when `write`),
    `NO-BUNDLE` (nothing built: `after` is what the rule gives, for information) or `REFUSED`
    (after is the reason, the bundle is never rewritten)."""
    bundle_json = Path(bundle_json)
    bundle = json.loads(bundle_json.read_text()) if bundle_json.exists() else None
    before = bundle["expected"] if bundle else None
    try:
        after = relaxed_expected(record, control_json)
    except LiveRefused as e:
        return before, str(e), "REFUSED"
    if before is None:
        return before, after, "NO-BUNDLE"
    if before is not None and _effective_expected(before) == _effective_expected(after):
        return before, after, "UNCHANGED"
    if write and bundle is not None:
        bundle["expected"] = after
        tmp = bundle_json.with_name(bundle_json.name + ".tmp")
        tmp.write_text(json.dumps(bundle, indent=2) + "\n")
        os.replace(tmp, bundle_json)
    return before, after, "CHANGED"


def _expected_line(exp):
    if not isinstance(exp, dict):
        return str(exp)
    return (f"F2P {exp['fail_to_pass']} kept, "
            f"{len(exp.get('fail_to_pass_dropped', []))} dropped "
            f"{[d['name'] for d in exp.get('fail_to_pass_dropped', [])]}; "
            f"P2P {exp['pass_to_pass_kept']} kept, {exp['pass_to_pass_dropped']} dropped")



def _records_by_id(path):
    return {r["instance_id"]: r for r in records_mod.load_records(path, records_mod.DATASET_SHA256)}


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("action", choices=["build", "prescan", "arm", "tool-layer", "reexpect"])
    ap.add_argument("ids", nargs="+")
    ap.add_argument("--records", type=Path, default=RECORDS)
    ap.add_argument("--controls", type=Path, default=CONTROLS)
    ap.add_argument("--root", type=Path, default=ROOT)
    ap.add_argument("--manifest", type=Path, default=MANIFEST)
    ap.add_argument("--max-image-gb", type=float, default=control.MAX_IMAGE_GB)
    ap.add_argument("--dry-run", action="store_true", help="reexpect: never rewrite a bundle")
    ap.add_argument("--assert-unchanged", action="store_true",
                    help="reexpect: exit 2 and rewrite nothing if any bundle's expected set changes")
    args = ap.parse_args(argv)
    by_id = _records_by_id(args.records) if args.action in ("build", "reexpect") else {}
    failed = False
    if args.action == "reexpect":
        results = {}
        for task_id in args.ids:
            control._safe_id(task_id)
            results[task_id] = reexpect(args.root / task_id / "bundle.json", by_id[task_id],
                                        args.controls / task_id / "control.json", write=False)
        changed = [t for t, (_, _, s) in results.items() if s == "CHANGED"]
        for task_id, (before, after, status) in results.items():
            print(f"{task_id}: {status}\n  before: {_expected_line(before)}\n  after:  {_expected_line(after)}",
                  flush=True)
        if args.assert_unchanged and changed:
            print(f"ASSERT FAILED: changed {changed}; nothing rewritten", flush=True)
            return 2
        if not args.dry_run:
            for task_id in changed:
                reexpect(args.root / task_id / "bundle.json", by_id[task_id],
                         args.controls / task_id / "control.json", write=True)
                print(f"{task_id}: bundle.json rewritten", flush=True)
        return 1 if any(s == "REFUSED" for _, _, s in results.values()) else 0
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
