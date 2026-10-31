"""Control run of one SWE-bench-Live MultiLang Rust task in its own image, offline.

    detect_workdir(container)                      `/testbed/repo` or `/testbed`, whichever holds `.git`
    reset_to_base(container, workdir, base_commit)  the image ships the gold fix uncommitted; undo it
    rebuild(container, workdir, record)            the record's rebuild_cmds, after a patch is applied
    run_tests(container, workdir, record, timeout)  the record's test_cmds, then its print_cmds as the log
    control(record, container=...)                  base + test patch, then + gold patch; the verdict

The image holds the answer: the raw tree carries the gold fix and old reports, so a run is only
meaningful after `reset_to_base` proves HEAD is `base_commit` and the tree is clean (`target/` is
git-ignored prebuilt output and is kept). Each state follows the dataset's own harness
(SWE-bench-Live `evaluation/evaluation.py`): reset, test patch, (gold patch), rebuild, tests,
print. The gold state starts from a fresh reset; its tests then run a second time, warm, and any
expected test whose status differs between the two gold runs fails the control. Containers run
`--network none`, are named `sab-ml-ctl-*`, and are always removed with their volumes. A control
or parser failure becomes a not-ok verdict flagged `error` (re-run on the next invocation); a
real rejection carries no flag. Every rebuild, test and print step keeps its exit code and the
last 4 KB of its output. `main` holds `<out>/.lock` and turns SIGTERM and SIGHUP into
KeyboardInterrupt so the cleanup still runs. Stdlib only.
"""
import argparse
import base64
import fcntl
import json
import os
import re
import shutil
import signal
import subprocess
import sys
import time
from collections import Counter
from pathlib import Path

import multilang_grade as grade
import multilang_records as records_mod
import pilot

CONTAINER_PREFIX = "sab-ml-ctl-"
MAX_LOG_BYTES = 64 * 1024 * 1024
MAX_DIRTY_LINES = 50
START_TIMEOUT = 600
REMOVE_TIMEOUT = 600
TAIL_BYTES = 4096
STEP_OUT = "/tmp/sab-step.out"  # inside the task container, outside the repository


class ControlError(RuntimeError):
    """The container could not be brought to a trustworthy base state, or a step failed."""


def _exec(container, workdir, *cmd, data=None, timeout=600, check=True):
    """`docker exec` in `workdir`; a failing step or a timeout is a ControlError when `check`."""
    args = ["exec"]
    if data is not None:
        args.append("-i")
    if workdir:
        args += ["-w", workdir]
    try:
        return pilot.docker(*args, container, *cmd, data=data, timeout=timeout, check=check)
    except RuntimeError as e:
        raise ControlError(str(e)) from None
    except subprocess.TimeoutExpired:
        raise ControlError(f"timed out after {timeout}s: {' '.join(map(str, cmd))[:200]}") from None


def detect_workdir(container):
    """`/testbed/repo` when it holds a `.git`, else `/testbed`; ControlError when neither does."""
    for path in ("/testbed/repo", "/testbed"):
        if _exec(container, None, "test", "-d", f"{path}/.git", check=False, timeout=60).returncode == 0:
            return path
    raise ControlError("no git repository at /testbed/repo or /testbed")


def _fresh_reports(container, workdir):
    # Some records tee into `reports/` without creating it, and some run from `/testbed` while the
    # repo is `/testbed/repo`: leave an empty `reports` at both places (empty dirs are not in status).
    report_dirs = list(dict.fromkeys([f"{workdir}/reports", "/testbed/reports"]))
    _exec(container, workdir, "rm", "-rf", *report_dirs, timeout=60)
    _exec(container, workdir, "mkdir", "-p", *report_dirs, timeout=60)


def reset_to_base(container, workdir, base_commit):
    """Reset the tree to `base_commit` and prove it: HEAD equals it and `git status` is empty."""
    before = _exec(container, workdir, "git", "status", "--porcelain", timeout=120).stdout
    dirty_before = before.splitlines()[:MAX_DIRTY_LINES]
    _exec(container, workdir, "git", "reset", "--hard", "-q", base_commit, timeout=300)
    _exec(container, workdir, "git", "clean", "-fdq", "-e", "target", timeout=300)
    _fresh_reports(container, workdir)
    head = _exec(container, workdir, "git", "rev-parse", "HEAD", timeout=60).stdout.strip()
    if head != base_commit:
        raise ControlError(f"HEAD is {head[:12]!r} after reset, expected {base_commit[:12]!r}")
    after = _exec(container, workdir, "git", "status", "--porcelain", timeout=120).stdout.strip()
    if after:
        raise ControlError(f"tree not clean after reset: {after.splitlines()[0][:200]}")
    return {"dirty_before": dirty_before}


def _step_script(cmd):
    """Run `cmd` with its output in a scratch file inside the container; print only the base64 of
    the file's last TAIL_BYTES bytes, and exit with `cmd`'s exit code. A file, not a pipe: a
    background process the command leaves holding its output cannot block the step."""
    return (f"(\n{cmd}\n) >{STEP_OUT} 2>&1\nrc=$?\n"
            f"tail -c {TAIL_BYTES} {STEP_OUT} | base64 -w0\nrm -f {STEP_OUT}\nexit $rc\n")


def _decode_tail(stdout):
    """The step's tail: base64-decoded and UTF-8 with replacement; anything else (a docker
    error message, say) is kept as text. At most TAIL_BYTES of the end, never raises."""
    text = str(stdout or "")
    try:
        return base64.b64decode(text.strip(), validate=True)[-TAIL_BYTES:].decode("utf-8", "replace")
    except ValueError:
        return text[-TAIL_BYTES:]


def _step(container, workdir, name, cmd, timeout):
    """One rebuild or test command, exit code ignored (a failing build or test is data)."""
    res = _exec(container, workdir, "bash", "-o", "pipefail", "-c", _step_script(cmd),
                timeout=timeout, check=False)
    return {"name": name, "returncode": res.returncode, "tail": _decode_tail(res.stdout)}


def rebuild(container, workdir, record, timeout=3600, *, steps=None, state="base"):
    """The record's `rebuild_cmds` joined with ` ; ` as one step (blank ones dropped); nothing
    when there is none. The dataset's harness rebuilds after every patch it applies."""
    cmds = [c for c in (record.get("rebuild_cmds") or []) if isinstance(c, str) and c.strip()]
    if not cmds:
        return
    step = _step(container, workdir, f"{state}:rebuild", " ; ".join(cmds), timeout)
    if steps is not None:
        steps.append(step)


def run_tests(container, workdir, record, timeout=3600, *, steps=None, state="base"):
    """Run `test_cmds` (exit codes ignored: a failing test is data), return the `print_cmds` output.
    Each command's exit code and output tail are appended to `steps`; a print step whose output
    reached MAX_LOG_BYTES (or whose part pushed the joined log over it) carries `truncated`."""
    if not record["test_cmds"]:
        raise ControlError("record has no test_cmds")
    steps = [] if steps is None else steps
    for n, cmd in enumerate(record["test_cmds"]):
        # the bulk output stays inside the container; the log is read back through print_cmds
        steps.append(_step(container, workdir, f"{state}:test[{n}]", cmd, timeout))
    parts, size = [], 0
    for n, cmd in enumerate(record["print_cmds"]):
        # bounded and base64-encoded inside the container, so a huge or non-UTF-8 log neither
        # fills memory nor breaks the strict text decoding of pilot.command
        wrapped = f"(\n{cmd}\n) 2>&1 | head -c {MAX_LOG_BYTES} | base64 -w0"
        res = _exec(container, workdir, "bash", "-o", "pipefail", "-c", wrapped, timeout=600,
                    check=False)
        try:
            raw = base64.b64decode(res.stdout.strip(), validate=True)
        except ValueError:
            raise ControlError(f"print command output is not base64: {res.stdout[:200]!r}") from None
        part = raw.decode("utf-8", "replace")
        step = {"name": f"{state}:print[{n}]", "returncode": res.returncode,
                "tail": raw[-TAIL_BYTES:].decode("utf-8", "replace")}
        size += len(part)
        if len(raw) >= MAX_LOG_BYTES or size > MAX_LOG_BYTES:
            step["truncated"] = True
        steps.append(step)
        parts.append(part)
    return "".join(parts)[:MAX_LOG_BYTES]


def _apply(container, workdir, patch):
    _exec(container, workdir, "git", "apply", "-", data=patch, timeout=300)


def _remove_container(container):
    """(removed, reason) for `docker rm -f -v <container>`; never raises an Exception. A
    container that does not exist (it never started) counts as removed."""
    try:
        res = pilot.docker("rm", "-f", "-v", container, check=False, timeout=REMOVE_TIMEOUT)
    except (RuntimeError, subprocess.TimeoutExpired, OSError) as e:
        return False, f"{type(e).__name__}: {str(e)[-200:]}"
    if res.returncode and "No such container" not in str(res.stdout):
        return False, f"rm exit {res.returncode}: {str(res.stdout)[-200:]}"
    return True, ""


def control(record, *, container, timeout=3600):
    """(evidence, base_log, gold_log) for one record. Never raises for a task-level failure."""
    if not container.startswith(CONTAINER_PREFIX):
        raise ControlError(f"container name must start with {CONTAINER_PREFIX!r}: {container!r}")
    f2p, p2p = list(record["FAIL_TO_PASS"]), list(record["PASS_TO_PASS"])
    evidence = {"id": record["instance_id"], "image": record["docker_image"]}
    steps = evidence["steps"] = []
    base_log = gold_log = ""
    started = time.monotonic()
    try:
        # a run killed hard may have left this exact container behind: it holds nothing we trust
        pilot.docker("rm", "-f", "-v", container, check=False, timeout=REMOVE_TIMEOUT)
        pilot.docker("run", "-d", "--name", container, "--network", "none", "--cpus", "4",
                     "--memory", "24g", "--memory-swap", "24g", "--pids-limit", "4096",
                     "--security-opt", "no-new-privileges", record["docker_image"],
                     "sleep", "infinity", timeout=START_TIMEOUT)
        workdir = evidence["workdir"] = detect_workdir(container)
        evidence.update(reset_to_base(container, workdir, record["base_commit"]))
        _apply(container, workdir, record["test_patch"])
        rebuild(container, workdir, record, timeout, steps=steps, state="base")
        base_log = run_tests(container, workdir, record, timeout, steps=steps, state="base")
        base = grade.run_parser(record["log_parser"], base_log)
        evidence["base"] = dict(Counter(base.values()))
        # the gold state starts from a fresh base tree, not from what the base run left behind
        evidence["dirty_before_gold"] = reset_to_base(container, workdir,
                                                      record["base_commit"])["dirty_before"]
        _apply(container, workdir, record["test_patch"])
        _apply(container, workdir, record["patch"])
        rebuild(container, workdir, record, timeout, steps=steps, state="gold")
        gold_log = run_tests(container, workdir, record, timeout, steps=steps, state="gold")
        gold = grade.run_parser(record["log_parser"], gold_log)
        evidence["gold"] = dict(Counter(gold.values()))
        # the same tests again, warm, in the same tree: a flaky expected test shows up as a mismatch
        _fresh_reports(container, workdir)
        repeat_log = run_tests(container, workdir, record, timeout, steps=steps, state="gold-repeat")
        repeat = grade.run_parser(record["log_parser"], repeat_log)
        evidence["gold_repeat"] = dict(Counter(repeat.values()))
        verdict = grade.control_verdict(base, gold, f2p, p2p, gold_repeat=repeat)
        evidence["gold_repeat_mismatch"] = verdict["gold_repeat_mismatch"]
        evidence["verdict"] = verdict
    except (ControlError, grade.ParserError, RuntimeError, subprocess.TimeoutExpired,
            ValueError) as e:  # ValueError: odd bytes in docker output (UnicodeDecodeError)
        evidence["verdict"] = {"ok": False, "error": True,
                               "reasons": [f"control error: {str(e)[-300:]}"]}
    finally:
        removed, why = _remove_container(container)
        evidence["container_removed"] = removed
        if not removed:
            evidence["container_remove_error"] = why
    if any(s.get("truncated") for s in steps):
        evidence["log_truncated"] = True
    evidence["wall_seconds"] = round(time.monotonic() - started, 1)
    return evidence, base_log, gold_log


MIN_FREE_GB = 40
SAFE_ID = re.compile(r"[A-Za-z0-9][A-Za-z0-9_.-]*\Z")
PULLED_MARKER = "pulled-image"


def free_gb(path="/"):
    return shutil.disk_usage(path).free / 1e9


def _marked_as_ours(marker, image):
    """True when `marker` exists and names `image`: an earlier run pulled it and did not remove it."""
    if marker is None:
        return False
    try:
        return Path(marker).read_text(encoding="utf-8").strip() == image
    except (OSError, ValueError):
        return False


def ensure_image(image, marker=None):
    """True when `image` is ours to remove: this call pulled it, or it is present and `marker`
    (written before any pull, deleted only after a successful rmi) says an earlier run pulled it.
    False when it was present without that marker (nothing is pulled). ControlError when free
    space is below MIN_FREE_GB before a pull, or the pull fails."""
    if pilot.docker("image", "inspect", image, check=False, timeout=120).returncode == 0:
        return _marked_as_ours(marker, image)
    free = free_gb()
    if free < MIN_FREE_GB:
        raise ControlError(f"refusing to pull {image}: {free:.0f} GB free, need {MIN_FREE_GB}")
    if marker is not None:
        # written before the pull: a run killed during or after it leaves a record that it is ours
        Path(marker).parent.mkdir(parents=True, exist_ok=True)
        Path(marker).write_text(image + "\n", encoding="utf-8")
    try:
        pilot.docker("pull", image, timeout=3600)
    except BaseException as e:
        # the daemon may have finished the pull after the CLI failed or was killed: such an image
        # is ours and untracked, so remove it before the error leaves (never forced)
        if _remove_if_present(image) and marker is not None:
            Path(marker).unlink(missing_ok=True)
        if isinstance(e, RuntimeError):
            raise ControlError(f"pull failed: {str(e)[-300:]}") from None
        if isinstance(e, subprocess.TimeoutExpired):
            raise ControlError(f"pull of {image} timed out") from None
        raise
    return True


def _remove_image(image):
    """(removed, reason) for `docker rmi <image>` (not forced); never raises an Exception."""
    try:
        res = pilot.docker("rmi", image, check=False, timeout=600)
    except (RuntimeError, subprocess.TimeoutExpired, OSError) as e:
        return False, f"{type(e).__name__}: {str(e)[-200:]}"
    if res.returncode:
        return False, f"rmi exit {res.returncode}: {str(res.stdout)[-200:]}"
    return True, ""


def _remove_if_present(image):
    """True when `image` is gone afterwards (absent, or removed now); never raises an Exception."""
    try:
        present = pilot.docker("image", "inspect", image, check=False, timeout=120).returncode == 0
    except (RuntimeError, subprocess.TimeoutExpired, OSError):
        return False
    if not present:
        return True
    return _remove_image(image)[0]


def _safe_id(instance_id):
    if not isinstance(instance_id, str) or not SAFE_ID.match(instance_id) or len(instance_id) > 120:
        raise ControlError(f"unsafe instance id for a path or container name: {instance_id!r}")
    return instance_id


def run_one(record, out_dir, *, timeout=3600):
    """Control one record; write control.json (last), base.log, gold.log under `<out_dir>/<id>`.
    An image this call (or an interrupted earlier one, per `<id>/pulled-image`) pulled is removed
    afterwards (no force); a present one is left alone."""
    task_id = _safe_id(record["instance_id"])
    d = Path(out_dir) / task_id
    marker = d / PULLED_MARKER
    pulled = ensure_image(record["docker_image"], marker=marker)
    removal = None
    try:
        evidence, base_log, gold_log = control(record, container=f"{CONTAINER_PREFIX}{task_id}",
                                               timeout=timeout)
    finally:
        if pulled:
            removal = _remove_image(record["docker_image"])
            if removal[0]:
                marker.unlink(missing_ok=True)
    if removal is not None:
        evidence["image_removed"] = removal[0]
        if not removal[0]:
            evidence["image_remove_error"] = removal[1]
    d.mkdir(parents=True, exist_ok=True)
    (d / "base.log").write_text(base_log, encoding="utf-8")
    (d / "gold.log").write_text(gold_log, encoding="utf-8")
    # control.json is the resume marker: it appears whole or not at all
    tmp = d / "control.json.tmp"
    with tmp.open("w", encoding="utf-8") as f:
        f.write(json.dumps(evidence, indent=2, allow_nan=False) + "\n")
        f.flush()
        os.fsync(f.fileno())
    os.replace(tmp, d / "control.json")
    return evidence


def _marker_state(path):
    """"done" for a control.json with a final verdict, "error" for one whose verdict is a control
    error (re-run it), "unreadable" for anything else."""
    try:
        data = json.loads(Path(path).read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return "unreadable"
    if not isinstance(data, dict) or "verdict" not in data:
        return "unreadable"
    verdict = data["verdict"]
    if isinstance(verdict, dict) and verdict.get("error"):
        return "error"
    return "done"


def _done(path):
    """True when `path` is a control.json that parses and carries a verdict that is not an error."""
    return _marker_state(path) == "done"


def _refusal(cand, rule):
    """Why `eligible(ignore_size=True)` said no (the check itself stays in multilang_records)."""
    missing = [f for f in rule["require"] if f not in cand["nonempty"]]
    return f"required fields empty: {missing}" if missing else \
        f"created {cand['created_at']} is before the cutoff {rule['created_at_min']}"


def _lock(out_dir):
    """An fd holding an exclusive flock on `<out_dir>/.lock` (the directory is created), or None
    when another holder has it. Closing the fd releases the lock."""
    out_dir = Path(out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    fd = os.open(out_dir / ".lock", os.O_RDWR | os.O_CREAT, 0o644)
    try:
        fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError:
        os.close(fd)
        return None
    return fd


def _raise_interrupt(signum, frame):
    """SIGTERM and SIGHUP unwind like Ctrl-C, so every `finally` (container, image) still runs."""
    raise KeyboardInterrupt(f"signal {signum}")


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--records", required=True)
    ap.add_argument("--rule", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--sha256", default=records_mod.DATASET_SHA256,
                    help="expected sha256 of --records (default: the registered dataset)")
    ap.add_argument("--ids", nargs="+")
    ap.add_argument("--limit", type=int)
    ap.add_argument("--ignore-size", action="store_true",
                    help="without --ids, select every record past the cutoff with the required "
                         "fields, in draw order (the size filter is applied later, from metadata)")
    args = ap.parse_args(argv)
    out_dir = Path(args.out)
    lock = _lock(out_dir)
    if lock is None:
        print(f"LOCKED {out_dir}")
        return 3
    previous = {s: signal.signal(s, _raise_interrupt) for s in (signal.SIGTERM, signal.SIGHUP)}
    try:
        return _run(args, out_dir)
    finally:
        for s, handler in previous.items():
            signal.signal(s, handler if handler is not None else signal.SIG_DFL)
        os.close(lock)


def _run(args, out_dir):
    try:
        records = records_mod.load_records(args.records, args.sha256)
    except ValueError as e:
        print(f"STOP records file rejected, nothing was run: {e}")
        return 2
    rule = records_mod.load_rule(args.rule)
    by_id = {r["instance_id"]: r for r in records}
    refused = 0
    if args.ids:
        chosen = []
        for i in dict.fromkeys(args.ids):
            if i not in by_id:
                print(f"REFUSED {i} not in the records file")
                refused += 1
                continue
            cand = records_mod.candidate(by_id[i])
            if records_mod.eligible(cand, rule, ignore_size=True):
                chosen.append(i)
            else:
                print(f"REFUSED {i} {_refusal(cand, rule)}")
                refused += 1
    else:
        chosen = records_mod.select_eligible(records, rule, ignore_size=args.ignore_size)
    if args.limit is not None:
        chosen = chosen[:args.limit]
    counts = Counter()
    for i in chosen:
        marker = out_dir / i / "control.json"
        if marker.exists():
            state = _marker_state(marker)
            if state == "done":
                print(f"{i} SKIP control.json exists")
                counts["done"] += 1
                continue
            print(f"RERUN {i} previous run was an error" if state == "error"
                  else f"RERUN {i} control.json unreadable")
        try:
            ev = run_one(by_id[i], out_dir)
        except ControlError as e:
            print(f"SKIPPED {i} {e}")
            counts["skipped"] += 1
            continue
        verdict = ev["verdict"]
        if ev.get("image_removed") is False:
            print(f"WARN rmi failed {by_id[i]['docker_image']} {ev.get('image_remove_error', '')}".rstrip())
        if ev.get("container_removed") is False:
            print(f"WARN container not removed {CONTAINER_PREFIX}{i} "
                  f"{ev.get('container_remove_error', '')}".rstrip())
        label = "ok" if verdict["ok"] else ("ERROR" if verdict.get("error") else "REJECT")
        counts[label] += 1
        print(f"{i} {label} {'; '.join(verdict.get('reasons', []))}".rstrip())
    ran = counts["ok"] + counts["REJECT"] + counts["ERROR"]
    print(f"ran {ran}: ok {counts['ok']}, REJECT {counts['REJECT']}, ERROR {counts['ERROR']}; "
          f"SKIPPED {counts['skipped']}, REFUSED {refused}, already done {counts['done']}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
