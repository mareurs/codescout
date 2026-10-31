"""Control run of one SWE-bench-Live MultiLang Rust task in its own image, offline.

    detect_workdir(container)                      `/testbed/repo` or `/testbed`, whichever holds `.git`
    reset_to_base(container, workdir, base_commit)  the image ships the gold fix uncommitted; undo it
    run_tests(container, workdir, record, timeout)  the record's test_cmds, then its print_cmds as the log
    control(record, container=...)                  base + test patch, then + gold patch; the verdict

The image holds the answer: the raw tree carries the gold fix and old reports, so a run is only
meaningful after `reset_to_base` proves HEAD is `base_commit` and the tree is clean (`target/` is
git-ignored prebuilt output and is kept). Containers run `--network none`, are named
`sab-ml-ctl-*`, and are always removed. Any control or parser failure becomes a not-ok verdict.
Stdlib only.
"""
import argparse
import base64
import json
import os
import re
import shutil
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


def reset_to_base(container, workdir, base_commit):
    """Reset the tree to `base_commit` and prove it: HEAD equals it and `git status` is empty."""
    before = _exec(container, workdir, "git", "status", "--porcelain", timeout=120).stdout
    dirty_before = before.splitlines()[:MAX_DIRTY_LINES]
    _exec(container, workdir, "git", "reset", "--hard", "-q", base_commit, timeout=300)
    _exec(container, workdir, "git", "clean", "-fdq", "-e", "target", timeout=300)
    # Some records tee into `reports/` without creating it, and some run from `/testbed` while the
    # repo is `/testbed/repo`: leave an empty `reports` at both places (empty dirs are not in status).
    report_dirs = list(dict.fromkeys([f"{workdir}/reports", "/testbed/reports"]))
    _exec(container, workdir, "rm", "-rf", *report_dirs, timeout=60)
    _exec(container, workdir, "mkdir", "-p", *report_dirs, timeout=60)
    head = _exec(container, workdir, "git", "rev-parse", "HEAD", timeout=60).stdout.strip()
    if head != base_commit:
        raise ControlError(f"HEAD is {head[:12]!r} after reset, expected {base_commit[:12]!r}")
    after = _exec(container, workdir, "git", "status", "--porcelain", timeout=120).stdout.strip()
    if after:
        raise ControlError(f"tree not clean after reset: {after.splitlines()[0][:200]}")
    return {"dirty_before": dirty_before}


def run_tests(container, workdir, record, timeout=3600):
    """Run `test_cmds` (exit codes ignored: a failing test is data), return the `print_cmds` output."""
    if not record["test_cmds"]:
        raise ControlError("record has no test_cmds")
    for cmd in record["test_cmds"]:
        # output is discarded inside the container: the log is read back through print_cmds
        _exec(container, workdir, "bash", "-o", "pipefail", "-c", f"(\n{cmd}\n) >/dev/null 2>&1",
              timeout=timeout, check=False)
    parts = []
    for cmd in record["print_cmds"]:
        # bounded and base64-encoded inside the container, so a huge or non-UTF-8 log neither
        # fills memory nor breaks the strict text decoding of pilot.command
        wrapped = f"(\n{cmd}\n) 2>&1 | head -c {MAX_LOG_BYTES} | base64 -w0"
        encoded = _exec(container, workdir, "bash", "-o", "pipefail", "-c", wrapped, timeout=600,
                        check=False).stdout
        try:
            parts.append(base64.b64decode(encoded.strip(), validate=True).decode("utf-8", "replace"))
        except ValueError:
            raise ControlError(f"print command output is not base64: {encoded[:200]!r}") from None
    return "".join(parts)[:MAX_LOG_BYTES]


def _apply(container, workdir, patch):
    _exec(container, workdir, "git", "apply", "-", data=patch, timeout=300)


def control(record, *, container, timeout=3600):
    """(evidence, base_log, gold_log) for one record. Never raises for a task-level failure."""
    if not container.startswith(CONTAINER_PREFIX):
        raise ControlError(f"container name must start with {CONTAINER_PREFIX!r}: {container!r}")
    f2p, p2p = list(record["FAIL_TO_PASS"]), list(record["PASS_TO_PASS"])
    evidence = {"id": record["instance_id"], "image": record["docker_image"]}
    base_log = gold_log = ""
    started = time.monotonic()
    try:
        pilot.docker("run", "-d", "--name", container, "--network", "none", "--cpus", "4",
                     "--memory", "24g", "--memory-swap", "24g", record["docker_image"],
                     "sleep", "infinity", timeout=START_TIMEOUT)
        workdir = evidence["workdir"] = detect_workdir(container)
        evidence.update(reset_to_base(container, workdir, record["base_commit"]))
        _apply(container, workdir, record["test_patch"])
        base_log = run_tests(container, workdir, record, timeout)
        base = grade.run_parser(record["log_parser"], base_log)
        evidence["base"] = dict(Counter(base.values()))
        _apply(container, workdir, record["patch"])
        gold_log = run_tests(container, workdir, record, timeout)
        gold = grade.run_parser(record["log_parser"], gold_log)
        evidence["gold"] = dict(Counter(gold.values()))
        evidence["verdict"] = grade.control_verdict(base, gold, f2p, p2p)
    except (ControlError, grade.ParserError, RuntimeError, subprocess.TimeoutExpired,
            ValueError) as e:  # ValueError: odd bytes in docker output (UnicodeDecodeError)
        evidence["verdict"] = {"ok": False, "reasons": [f"control error: {str(e)[:300]}"]}
    finally:
        pilot.docker("rm", "-f", container, check=False)
    evidence["wall_seconds"] = round(time.monotonic() - started, 1)
    return evidence, base_log, gold_log


MIN_FREE_GB = 40
SAFE_ID = re.compile(r"[A-Za-z0-9][A-Za-z0-9_.-]*\Z")


def free_gb(path="/"):
    return shutil.disk_usage(path).free / 1e9


def ensure_image(image):
    """True when this call pulled `image`; False when it was already present (nothing is pulled).
    ControlError when free space is below MIN_FREE_GB before a pull, or the pull fails."""
    if pilot.docker("image", "inspect", image, check=False, timeout=120).returncode == 0:
        return False
    free = free_gb()
    if free < MIN_FREE_GB:
        raise ControlError(f"refusing to pull {image}: {free:.0f} GB free, need {MIN_FREE_GB}")
    try:
        pilot.docker("pull", image, timeout=3600)
    except BaseException as e:
        # the daemon may have finished the pull after the CLI failed or was killed: such an image
        # is ours and untracked, so remove it before the error leaves (never forced)
        _remove_if_present(image)
        if isinstance(e, RuntimeError):
            raise ControlError(f"pull failed: {str(e)[:300]}") from None
        if isinstance(e, subprocess.TimeoutExpired):
            raise ControlError(f"pull of {image} timed out") from None
        raise
    return True


def _remove_image(image):
    """(removed, reason) for `docker rmi <image>` (not forced); never raises an Exception."""
    try:
        res = pilot.docker("rmi", image, check=False, timeout=600)
    except (RuntimeError, subprocess.TimeoutExpired, OSError) as e:
        return False, f"{type(e).__name__}: {str(e)[:200]}"
    if res.returncode:
        return False, f"rmi exit {res.returncode}: {str(res.stdout)[-200:]}"
    return True, ""


def _remove_if_present(image):
    try:
        present = pilot.docker("image", "inspect", image, check=False, timeout=120).returncode == 0
    except (RuntimeError, subprocess.TimeoutExpired, OSError):
        return
    if present:
        _remove_image(image)


def _safe_id(instance_id):
    if not isinstance(instance_id, str) or not SAFE_ID.match(instance_id) or len(instance_id) > 120:
        raise ControlError(f"unsafe instance id for a path or container name: {instance_id!r}")
    return instance_id


def run_one(record, out_dir, *, timeout=3600):
    """Control one record; write control.json (last), base.log, gold.log under `<out_dir>/<id>`.
    An image this call pulled is removed afterwards (no force); a present one is left alone."""
    task_id = _safe_id(record["instance_id"])
    pulled = ensure_image(record["docker_image"])
    removal = None
    try:
        evidence, base_log, gold_log = control(record, container=f"{CONTAINER_PREFIX}{task_id}",
                                               timeout=timeout)
    finally:
        if pulled:
            removal = _remove_image(record["docker_image"])
    if removal is not None:
        evidence["image_removed"] = removal[0]
        if not removal[0]:
            evidence["image_remove_error"] = removal[1]
    d = Path(out_dir) / task_id
    d.mkdir(parents=True, exist_ok=True)
    (d / "base.log").write_text(base_log)
    (d / "gold.log").write_text(gold_log)
    # control.json is the resume marker: it appears whole or not at all
    tmp = d / "control.json.tmp"
    with tmp.open("w") as f:
        f.write(json.dumps(evidence, indent=2, allow_nan=False) + "\n")
        f.flush()
        os.fsync(f.fileno())
    os.replace(tmp, d / "control.json")
    return evidence


def _done(path):
    """True when `path` is a control.json that parses and carries a verdict."""
    try:
        return "verdict" in json.loads(Path(path).read_text())
    except (OSError, ValueError, TypeError):
        return False


def _refusal(cand, rule):
    """Why `eligible(ignore_size=True)` said no (the check itself stays in multilang_records)."""
    missing = [f for f in rule["require"] if f not in cand["nonempty"]]
    return f"required fields empty: {missing}" if missing else \
        f"created {cand['created_at']} is before the cutoff {rule['created_at_min']}"


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--records", required=True)
    ap.add_argument("--rule", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--ids", nargs="+")
    ap.add_argument("--limit", type=int)
    args = ap.parse_args(argv)
    records = records_mod.load_records(args.records)
    rule = records_mod.load_rule(args.rule)
    by_id = {r["instance_id"]: r for r in records}
    if args.ids:
        chosen = []
        for i in dict.fromkeys(args.ids):
            if i not in by_id:
                print(f"REFUSED {i} not in the records file")
                continue
            cand = records_mod.candidate(by_id[i])
            if records_mod.eligible(cand, rule, ignore_size=True):
                chosen.append(i)
            else:
                print(f"REFUSED {i} {_refusal(cand, rule)}")
    else:
        chosen = records_mod.select_eligible(records, rule)
    if args.limit is not None:
        chosen = chosen[:args.limit]
    out_dir, ran, accepted = Path(args.out), 0, 0
    for i in chosen:
        marker = out_dir / i / "control.json"
        if marker.exists():
            if _done(marker):
                print(f"{i} SKIP control.json exists")
                continue
            print(f"RERUN {i} control.json unreadable")
        try:
            ev = run_one(by_id[i], out_dir)
        except ControlError as e:
            print(f"SKIPPED {i} {e}")
            continue
        verdict = ev["verdict"]
        if ev.get("image_removed") is False:
            print(f"WARN rmi failed {by_id[i]['docker_image']} {ev.get('image_remove_error', '')}".rstrip())
        ran += 1
        accepted += bool(verdict["ok"])
        print(f"{i} {'ok' if verdict['ok'] else 'REJECT'} {'; '.join(verdict.get('reasons', []))}".rstrip())
    print(f"ran {ran}, ok {accepted}, rejected {ran - accepted}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
