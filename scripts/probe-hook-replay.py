#!/usr/bin/env python3
"""probe-hook-replay.py -- would a changed commit hook have refused real past commits?

Replays a pre-commit script over history BY EXECUTION. For each historical commit C with parent P it
builds a throwaway repo whose HEAD is P and whose index is C's tree (so `git diff --cached` is exactly
C's staged change), runs the CANDIDATE script and a BASELINE script in it, and compares their exit
codes. The script is its own firing oracle: nobody has to re-derive "when would this gate have fired",
which is the step that overcounted 29 as 55 on 2026-10-16 when it was done by hand.

    # edited scripts/pre-commit-ledger-counts.py in a worktree; replay it against what is committed:
    python3 scripts/probe-hook-replay.py --script scripts/pre-commit-ledger-counts.py \\
        --paths docs/issues docs/trackers --since 2026-09-03

    # two scripts that are not the committed one:
    python3 scripts/probe-hook-replay.py --script cand.py --baseline-script base.py --json

The 2x2 it prints is (baseline refused?, candidate refused?):

    0,0  both accept                                  0,1  the candidate NEWLY REFUSES -- read these
    1,0  the candidate accepts what the baseline refused (a loosening)
    1,1  both refuse -- commits made without the hook enforcing, or noise; the BASELINE for any rate

Read `0,1` against `1,1`: events the baseline also refused are not false refusals of the candidate.

BLIND SPOTS, and they decide whether a number from this probe means anything:
  * Only a hook that is a function of (HEAD, index, the files under --paths) can be replayed. One that
    reads other state -- a session log, a transcript, the environment -- sees an empty version of it
    here. `pre-commit-ledger-counts.py` qualifies; `pre-commit-foreign-index.sh` and
    `pre-commit-unreviewed-content.sh` do not.
  * Only the files under --paths are materialised in the throwaway worktree. The index is complete.
  * The replay stages the WHOLE commit. A session that committed by pathspec or from a private index
    may have shown the hook less. The two agree for ordinary single-session commits.
  * Merge commits and the root commit are skipped (a merge has no single parent to stage against).
  * Exit status 3 means nothing was replayed. A zero is never printed as a clean result: every reading
    here is an existence report over a selection, and an empty selection looks like a healthy history.
"""
from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile
from collections import Counter
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

EXIT_NOTHING_REPLAYED = 3
RUN_TIMEOUT_SECS = 30
STDERR_HEAD = 400
# Above this share of replayed commits refused by the BASELINE, the reconstruction is more likely wrong
# than the history is bad: the real gate refused almost nothing that landed.
BASELINE_SUSPECT_SHARE = 0.5
BASELINE_SUSPECT_MIN_COMMITS = 3


def _clean_env() -> dict[str, str]:
    """The caller's environment minus every git variable that would redirect a child's repo."""
    return {k: v for k, v in os.environ.items() if not k.startswith("GIT_") or k in ("GIT_EXEC_PATH",)}


def git(repo: Path | str, *args: str, input: bytes | None = None) -> subprocess.CompletedProcess:
    return subprocess.run(
        ["git", "-C", str(repo), *args], input=input, capture_output=True, env=_clean_env()
    )


def git_text(repo: Path | str, *args: str) -> str:
    r = git(repo, *args)
    if r.returncode != 0:
        raise SystemExit(f"git {' '.join(args)} failed in {repo}: {r.stderr.decode(errors='replace').strip()}")
    return r.stdout.decode(errors="replace")


def interpreter_for(path: str) -> list[str]:
    if path.endswith(".py"):
        return [sys.executable]
    if path.endswith(".sh"):
        return ["bash"]
    return []


def commits_to_replay(repo: Path, paths: list[str], since: str | None, limit: int | None) -> list[tuple[str, str, str]]:
    cmd = ["log", "--reverse", "--no-merges", "--format=%H%x09%ad%x09%s", "--date=short"]
    if since:
        cmd.append(f"--since={since}")
    if paths:
        cmd += ["--", *paths]
    out = git_text(repo, *cmd)
    rows = []
    for line in out.splitlines():
        sha, date, subject = line.split("\t", 2)
        if not git_text(repo, "rev-list", "--parents", "-n", "1", sha).split()[1:]:
            continue  # the root commit has no parent to stage against
        rows.append((sha, date, subject))
    return rows[-limit:] if limit else rows


def build_repo(root: Path, repo: Path, objects: str, parent: str, commit: str, paths: list[str]) -> Path:
    """A throwaway repo: HEAD = parent, index = commit's tree, worktree = commit's files under `paths`."""
    t = Path(tempfile.mkdtemp(prefix="replay-", dir=root))
    git(t, "init", "-q")
    (t / ".git" / "objects" / "info").mkdir(parents=True, exist_ok=True)
    (t / ".git" / "objects" / "info" / "alternates").write_text(objects + "\n")
    git(t, "update-ref", "refs/heads/replay", parent)
    git(t, "symbolic-ref", "HEAD", "refs/heads/replay")
    git(t, "read-tree", commit)
    listing = git(t, "ls-files", "-z", "--", *(paths or ["."])).stdout
    if listing:
        git(t, "checkout-index", "-f", "-z", "--stdin", input=listing)
    return t


def run_hook(t: Path, interpreter: list[str], relpath: str) -> tuple[int, str]:
    try:
        r = subprocess.run(
            [*interpreter, relpath], cwd=t, capture_output=True, text=True,
            env=_clean_env(), timeout=RUN_TIMEOUT_SECS,
        )
    except subprocess.TimeoutExpired:
        return 124, f"(timed out after {RUN_TIMEOUT_SECS}s)"
    return r.returncode, (r.stderr or r.stdout)[:STDERR_HEAD]


def replay_commit(
    repo: Path, root: Path, objects: str, paths: list[str], relpath: str, interpreter: list[str],
    baseline_bytes: bytes, candidate_bytes: bytes, row: tuple[str, str, str],
) -> dict:
    """Replay ONE commit: the baseline, then the candidate, each in a fresh throwaway repo state.

    Independent of every other commit, which is what lets `--jobs` run these in parallel.
    """
    sha, date, subject = row
    parent = git_text(repo, "rev-parse", f"{sha}^").strip()
    t = build_repo(root, repo, objects, parent, sha, paths)
    try:
        dest = t / relpath
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_bytes(baseline_bytes)
        b_rc, b_err = run_hook(t, interpreter, relpath)
        dest.write_bytes(candidate_bytes)
        c_rc, c_err = run_hook(t, interpreter, relpath)
    finally:
        shutil.rmtree(t, ignore_errors=True)
    return {
        "commit": sha[:10], "date": date, "subject": subject,
        "base_rc": b_rc, "cand_rc": c_rc,
        "base_stderr_head": b_err, "cand_stderr_head": c_err,
    }



def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--repo", default=None, help="the repository to replay (default: the one containing the cwd)")
    ap.add_argument("--script", required=True, help="the CANDIDATE hook script")
    ap.add_argument("--baseline-script", help="a file to use as the baseline (default: --script's path at --baseline-rev)")
    ap.add_argument("--baseline-rev", default="HEAD", help="revision for the default baseline (default: HEAD)")
    ap.add_argument("--paths", nargs="+", default=[], help="replay only commits touching these paths; also the worktree materialised")
    ap.add_argument("--since", help="git --since for the commit window")
    ap.add_argument("--limit", type=int, help="replay only the most recent N commits")
    ap.add_argument("--json", action="store_true", help="machine-readable output")
    ap.add_argument("--jobs", type=int, default=1, help="replay this many commits in parallel (default 1); a speed-up only, the result is identical")
    ap.add_argument("--tmpdir", help="where to build the throwaway repos (default: the system temp dir)")
    args = ap.parse_args()

    repo = Path(args.repo or git_text(Path.cwd(), "rev-parse", "--show-toplevel").strip()).resolve()
    objects = git_text(repo, "rev-parse", "--path-format=absolute", "--git-path", "objects").strip()

    cand = Path(args.script).resolve()
    try:
        relpath = cand.relative_to(repo).as_posix()
    except ValueError:
        relpath = f"scripts/{cand.name}"
    interpreter = interpreter_for(relpath)

    if args.baseline_script:
        baseline_bytes = Path(args.baseline_script).read_bytes()
    else:
        r = git(repo, "show", f"{args.baseline_rev}:{relpath}")
        if r.returncode != 0:
            raise SystemExit(
                f"no baseline: {args.baseline_rev}:{relpath} does not exist. Give --baseline-script, "
                "or a --script path that is committed."
            )
        baseline_bytes = r.stdout
    candidate_bytes = cand.read_bytes()

    commits = commits_to_replay(repo, args.paths, args.since, args.limit)
    root = Path(tempfile.mkdtemp(prefix="probe-hook-replay-", dir=args.tmpdir))
    pairs: Counter = Counter()
    try:
        # `map` yields in submission order, so --jobs changes the speed and never the result.
        with ThreadPoolExecutor(max_workers=max(1, args.jobs)) as pool:
            events = list(pool.map(
                lambda row: replay_commit(repo, root, objects, args.paths, relpath, interpreter,
                                          baseline_bytes, candidate_bytes, row),
                commits,
            ))
    finally:
        shutil.rmtree(root, ignore_errors=True)
    for e in events:
        pairs[f"{int(e['base_rc'] != 0)},{int(e['cand_rc'] != 0)}"] += 1

    summary = {
        "replayed": len(events),
        "pairs": {k: pairs.get(k, 0) for k in ("0,0", "0,1", "1,0", "1,1")},
        "events": events,
    }
    if not events:
        print("probe-hook-replay: replayed 0 commits -- the selection matched nothing, so there is no "
              "reading to report (check --paths, --since and that the repo has non-root history).", file=sys.stderr)
        return EXIT_NOTHING_REPLAYED
    base_refused = pairs.get("1,0", 0) + pairs.get("1,1", 0)
    if len(events) >= BASELINE_SUSPECT_MIN_COMMITS and base_refused / len(events) > BASELINE_SUSPECT_SHARE:
        print(f"probe-hook-replay: warning: the baseline refused {base_refused} of {len(events)} replayed "
              "commits. The real gate let these land, so the reconstruction is more likely wrong than the "
              "history -- is the hook reading state this replay does not provide?", file=sys.stderr)

    if args.json:
        print(json.dumps(summary, ensure_ascii=False))
        return 0
    print(f"replayed {summary['replayed']} commits   (baseline rc, candidate rc)")
    for k, label in (("0,0", "both accept"), ("0,1", "candidate NEWLY REFUSES"),
                     ("1,0", "candidate accepts what the baseline refused"), ("1,1", "both refuse (the noise floor)")):
        print(f"  {k}  {summary['pairs'][k]:5d}  {label}")
    new = [e for e in events if e["base_rc"] == 0 and e["cand_rc"] != 0]
    if new:
        print("\ncommits the candidate would newly refuse (date commit subject / first refusal line):")
        for e in new:
            first = (e["cand_stderr_head"].strip().splitlines() or ["(no output)"])[0][:120]
            print(f"  {e['date']} {e['commit']} {e['subject'][:70]}\n      {first}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
