"""The fresh-task builder: turn one merged upstream PR into a SWE-bench-Live-shaped record.

Pure helpers (functions of their arguments):

    is_test_path(path)                                   True for a test file by path alone
    diff_files(diff)                                     the b-side path of every file block, in order
    split_diff(diff)                                     (patch, test_patch), byte-faithful
    changed_lines(diff)                                  {path: Counter of the +/- hunk lines}
    same_change(a, b)                                    True when two diffs change the same lines
    pick_base(parents)                                   the first parent of a merge commit
    derive_sets(base, gold)                              (FAIL_TO_PASS, PASS_TO_PASS) from two status maps
    spec_key(problem_statement, test_patch, patch)       cache key of a spec-verdict request
    apply_spec_verdicts(f2p, verdicts)                   (kept, dropped) F2P tests
    cache_key(repo, upstream_base, base_image_id, ...)   cache key of a built image
    build_record(...)                                    the task record, schema of `multilang-rust.jsonl`

Upstream fetch (the one part that runs programs, through an injected `run`):

    fetch_pr(repo, pr, issue_numbers, dest, ...)         merge commit, base, diff and issue text of a PR
    FetchError                                           a refused or failed fetch

Image build (runs docker, through an injected `docker`):

    image_tag(repo, pr)                                  the study image tag of a PR
    build_image(base_image=, repo=, pr=, ...)            task image -> upstream tree swapped in, built, committed
    BuildError                                           a refused or failed build; no image tag is left
    data_root_free_gb()                                  free GB of the docker storage, the default disk guard

Derive and strict gate (runs docker through `docker=` and the study's `multilang_control` helpers):

    control_fresh(record, container=, evidence_dir=)     F2P/P2P derived from base+test vs gold+test, then the
                                                         strict gate (8g, a gold repeat, a tolerant 2nd repeat)

Orchestration (runs nothing itself but `docker image inspect`, `git archive` and `git rev-parse` through `run=`):

    run_build(repo=, pr=, issue_numbers=, base_image=, template_path=, template_id=, data=, ...)
                                                         fetch, cache check, tarball, build, control, spec
                                                         verdicts, record; returns {status, exit_code, ...}
    main(argv)                                           the `build` command line; its exit code is the result's

The helpers above import the standard library only and never touch docker, git, `gh`, the network,
the filesystem or the environment, so their tests run anywhere. `fetch_pr` reads the environment and
runs `gh` and `git` (with `GITHUB_TOKEN` removed) through `run=`, which a test replaces; it does not
touch docker. `build_image` runs `docker` through `docker=` (default `pilot.docker`, imported here
with `multilang_control` and `multilang_grade`), which a test replaces, and reads the exit code of every
step from the returned `CompletedProcess` and nowhere else. `control_fresh` starts and removes its
container through `docker=`; the state steps are `multilang_control`'s helpers (which use `pilot.docker`
themselves, read-only imports of a frozen module) and `multilang_grade.run_parser` /
`control_verdict`; a test replaces those module attributes.

A diff is treated as text with `\\n` boundaries only: it is split on the position of
each `diff --git ` line start and never through `str.splitlines`, so a CRLF blob, a lone `\\r` or a
Unicode line separator inside a hunk survives unchanged. `split_diff` regroups the blocks (source
first, then test), so `patch + test_patch != diff` for a diff that interleaves them; each block's
bytes and the order within each half are kept. A quoted path in a header raises `ValueError`: the
caller asks git for `-c core.quotePath=false`. Text is hashed with `surrogateescape`, and `fetch_pr`
decodes git output the same way, so non-UTF-8 bytes stay byte-faithful. Stdlib only.

Usage. A rust-analyzer PR, first without verdicts (this builds, runs the strict gate and stops with exit 3
and the `spec_key` and F2P tests to judge; the second run reuses the image and goes on):

    python3 fresh_build.py build --repo rust-lang/rust-analyzer --pr 23096 --issues 23090 \\
        --base-image starryzhang/sweb.eval.x86_64.rust-lang_1776_rust-analyzer-22855 \\
        --template-record /home/marius/work/claude/setup-ab-data/live/multilang-rust.jsonl \\
        --template-id rust-lang__rust-analyzer-22855 --data DIR [--spec-verdicts spec-verdicts.json] [--dry-run]

Exit codes: 0 done (or cached), 2 refused or gate failed, 3 awaiting spec verdicts, 4 unusable (no F2P test
kept). The work directory in the image is fixed (`/testbed`, `build_image`'s default) and is not a CLI
option, so a base image with another workdir cannot be used. One JSON object per stage goes to stderr, the
result to stdout. `DIR` gets `records.jsonl`,
`controls/<id>/control.json` and `evidence/<id>/`.

`spec-verdicts.json` (the F2P rule: a test is kept only if the issue states the behaviour it asserts; an F2P
test without a verdict is an error, never a keep). Keyed by the `spec_key` the exit-3 run printed; test names
embed the compiled-binary hash, so they belong to the image of that run:

    {"<spec_key>": {"tests": {"<test name>": "specified" | "unspecified"},
                    "reader": "<who or what decided>", "note": "<free text, optional>"}}
"""
import argparse
import ast
import collections
import copy
import datetime
import hashlib
import json
import os
import re
import shutil
import signal
import subprocess
import sys
import tarfile
import tempfile
import time
import traceback
from pathlib import Path

import multilang_control as mc
import multilang_grade as grade
import pilot

BUILDER_VERSION = "1"
MIN_FREE_GB = 60
CUTOFF = "2026-07-01"

# Any path component (directory or file name) containing "test" or "e2e". This is the rule the
# SWE-bench-Live MultiLang split follows: it reproduces the dataset's patch/test_patch partition
# on all 1,259 files of the 171 saved Rust records.
TEST_PATH_RE = re.compile(r"(^|/)[^/]*(test|e2e)[^/]*(/|$)", re.IGNORECASE)

SPEC_RULE = "f2p kept only if the issue states the asserted behaviour"
VERDICTS = ("specified", "unspecified")
TEMPLATE_LIST_KEYS = ("test_cmds", "rebuild_cmds", "print_cmds")

_BLOCK_START = re.compile(r"^(?=diff --git )", re.MULTILINE)
_HEADER = "diff --git "


def is_test_path(path):
    """True when any component of `path` contains "test" or "e2e" (case-insensitive), as in the dataset's split."""
    return TEST_PATH_RE.search(path) is not None


def _blocks(diff):
    """The per-file blocks of `diff`, each starting at its `diff --git ` line; joined they equal `diff`."""
    if not diff:
        return []
    if not diff.startswith(_HEADER):
        raise ValueError("not a git diff: text does not start with 'diff --git '")
    return [b for b in _BLOCK_START.split(diff) if b]


def _quoted(where):
    return ValueError(
        "path quoted by git in the diff header; the builder refuses it "
        "(git quotes a path with a double quote, backslash, tab or newline whatever the settings; "
        "for non-ASCII names run git with -c core.quotePath=false): %r" % where
    )


def _b_path(block):
    """The b-side path of one block: its `rename to` line when present, else the header's b-side."""
    head, _, rest = block.partition("\n")
    head = head.rstrip("\r")[len(_HEADER):]
    if head.startswith('"') or ' "b/' in head:
        raise _quoted(head)
    renamed = None
    for line in rest.split("\n"):
        if line.startswith("@@"):
            break
        for prefix in ("rename to ", "copy to "):
            if line.startswith(prefix):
                target = line[len(prefix):].rstrip("\r")
                if target.startswith('"'):
                    raise _quoted(target)
                if prefix == "rename to ":
                    renamed = target
    if renamed is not None:
        return renamed
    # Without a rename both sides are the same path: "a/P b/P" is 2*len(P) + 5 long.
    length = (len(head) - 5) // 2
    if length > 0:
        path = head[len(head) - length:]
        if head == "a/%s b/%s" % (path, path):
            return path
    m = re.match(r"a/.* b/(.*)$", head)
    if m is None:
        raise ValueError("unparseable diff header: %r" % head)
    return m.group(1)


def diff_files(diff):
    """The b-side path of every `diff --git a/X b/Y` block in `diff`, in order."""
    return [_b_path(b) for b in _blocks(diff)]


def split_diff(diff):
    """`(patch, test_patch)`: the blocks of `diff` whose b-side path is not / is a test path.

    Blocks keep their bytes and, within each half, their order. `ValueError` when non-empty text
    does not start with `diff --git `, or when a header carries a quoted path.
    Blocks are regrouped, source first and then test: `patch + test_patch` equals `diff` only when
    no source block follows a test block.
    """
    patch, test_patch = [], []
    for block in _blocks(diff):
        (test_patch if is_test_path(_b_path(block)) else patch).append(block)
    return "".join(patch), "".join(test_patch)


def changed_lines(diff):
    """`{b-side path: Counter}` of the changed lines of every file block of `diff`.

    A changed line is a hunk line that starts with `+` or `-`, kept as its exact text with the sign
    and any `\\r`. The `---`/`+++` file header lines are not hunk lines and are skipped by position
    (a removed line whose text is `-- x` reads `--- x` and does count). Context lines, hunk
    headers, `\\ No newline` markers and file order are ignored; a block without hunks (mode,
    rename, binary) is present with an empty Counter. `ValueError` as `diff_files`.
    """
    out = {}
    for block in _blocks(diff):
        counts = collections.Counter()
        in_hunk = False
        for line in block.split("\n"):
            if line.startswith("@@"):
                in_hunk = True
            elif in_hunk and line[:1] in ("+", "-"):
                counts[line] += 1
        out.setdefault(_b_path(block), collections.Counter()).update(counts)
    return out


def same_change(a, b):
    """True when two diffs change the same files by the same lines (see `changed_lines`)."""
    return changed_lines(a) == changed_lines(b)


def pick_base(parents):
    """The first parent of a commit. `ValueError` on an empty list."""
    if not parents:
        raise ValueError("commit has no parents")
    return parents[0]


def derive_sets(base, gold):
    """`(f2p, p2p)`, both sorted, from `{test: status}` maps at base and at gold.

    F2P: passes at gold and does not pass at base (failing or absent). P2P: passes in both. A test
    that passes at base and not at gold lands in neither.
    """
    passing_gold = {t for t, s in gold.items() if s == "pass"}
    passing_base = {t for t, s in base.items() if s == "pass"}
    return sorted(passing_gold - passing_base), sorted(passing_gold & passing_base)


def _digest(*parts):
    # surrogateescape: git output is decoded the same way to stay byte-faithful.
    return hashlib.sha256("\0".join(parts).encode("utf-8", errors="surrogateescape")).hexdigest()


def spec_key(problem_statement, test_patch, patch):
    """sha256 hex of the three texts joined with a NUL byte."""
    return _digest(problem_statement, test_patch, patch)


def apply_spec_verdicts(f2p, verdicts):
    """`(kept, dropped)` from `f2p` by `verdicts` (`{test: "specified" | "unspecified"}`), input order kept.

    `KeyError` naming every F2P test without a verdict; `ValueError` for any other verdict value.
    """
    missing = [t for t in f2p if t not in verdicts]
    if missing:
        raise KeyError("no spec verdict for: " + ", ".join(missing))
    bad = {t: verdicts[t] for t in f2p if verdicts[t] not in VERDICTS}
    if bad:
        raise ValueError("verdict must be 'specified' or 'unspecified': %r" % bad)
    return ([t for t in f2p if verdicts[t] == "specified"],
            [t for t in f2p if verdicts[t] == "unspecified"])


def cache_key(repo, upstream_base, base_image_id, version=BUILDER_VERSION):
    """sha256 hex of repo, upstream base commit, base image id and builder version, NUL-joined."""
    return _digest(repo, upstream_base, base_image_id, version)


def build_record(*, repo, pr, issue_numbers, created_at, problem_statement, base_commit,
                 upstream_base_commit, patch, test_patch, f2p, p2p, dropped_f2p, template,
                 docker_image):
    """The task record: the keys of a `multilang-rust.jsonl` record plus four builder keys.

    `instance_id` is `<owner>__<name>-<pr>`. `test_cmds`, `rebuild_cmds` and `print_cmds` are
    copied from `template` as lists (`ValueError` if one is not a list), `log_parser` as is; the
    hints are empty. `commit_urls` is empty: the gold commits are not an input here.
    """
    for key in TEMPLATE_LIST_KEYS:
        if not isinstance(template[key], list):
            raise ValueError("template[%r] must be a list, got %s" % (key, type(template[key]).__name__))
    return {
        "FAIL_TO_PASS": list(f2p),
        "PASS_TO_PASS": list(p2p),
        "all_hints_text": "",
        "base_commit": base_commit,
        "commit_url": "https://github.com/%s/tree/%s" % (repo, upstream_base_commit),
        "commit_urls": [],
        "created_at": created_at,
        "docker_image": docker_image,
        "hints_text": "",
        "instance_id": "%s-%s" % (repo.replace("/", "__"), pr),
        "issue_numbers": [str(n) for n in issue_numbers],
        "log_parser": template["log_parser"],
        "patch": patch,
        "print_cmds": copy.deepcopy(template["print_cmds"]),
        "problem_statement": problem_statement,
        "pull_number": str(pr),
        "rebuild_cmds": copy.deepcopy(template["rebuild_cmds"]),
        "repo": repo,
        "test_cmds": copy.deepcopy(template["test_cmds"]),
        "test_patch": test_patch,
        "upstream_base_commit": upstream_base_commit,
        "dropped_f2p": list(dropped_f2p),
        "spec_rule": SPEC_RULE,
        "builder_version": BUILDER_VERSION,
    }


class FetchError(RuntimeError):
    """The upstream fetch was refused (unmerged PR, before the cutoff, file mismatch) or a command failed."""


_STDERR_TAIL = 300
_DATE = re.compile(r"\d{4}-\d\d-\d\d")
# ASCII digits only: `\d` would take Arabic-Indic ones
_STAMP = re.compile(r"[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z")
_SHA = re.compile(r"[0-9a-f]{40}([0-9a-f]{24})?")
DIFF_ACCEPT = "Accept: application/vnd.github.v3.diff"


def _call(run, argv, env, timeout):
    """Run `argv` capturing bytes; `FetchError` on a failure (stderr: last 300 characters) or a timeout."""
    try:
        proc = run(argv, capture_output=True, env=env, timeout=timeout)
    except subprocess.TimeoutExpired as exc:
        raise FetchError("%s timed out after %ss" % (" ".join(argv), timeout)) from exc
    except OSError as exc:
        raise FetchError("cannot run %s: %s" % (argv[0], exc)) from exc
    if proc.returncode != 0:
        tail = proc.stderr.decode("utf-8", errors="replace").rstrip()[-_STDERR_TAIL:]
        raise FetchError("%s failed (exit %s): %s" % (" ".join(argv), proc.returncode, tail))
    return proc.stdout


def _json(stdout, what):
    try:
        return json.loads(stdout)
    except ValueError as exc:
        raise FetchError("%s: reply is not JSON: %s" % (what, exc)) from exc


def _json_pages(stdout, what):
    """The items of `gh api --paginate` output: one JSON array per page, concatenated."""
    try:
        text = stdout.decode("utf-8")
    except UnicodeDecodeError as exc:
        raise FetchError("%s: reply is not UTF-8: %s" % (what, exc)) from exc
    decoder, pos, items = json.JSONDecoder(), 0, []
    while True:
        while pos < len(text) and text[pos].isspace():
            pos += 1
        if pos >= len(text):
            return items
        try:
            page, pos = decoder.raw_decode(text, pos)
        except ValueError as exc:
            raise FetchError("%s: reply is not JSON: %s" % (what, exc)) from exc
        if not isinstance(page, list):
            raise FetchError("%s: a page is not a JSON array" % what)
        items.extend(page)


def _field(obj, key, what):
    if not isinstance(obj, dict) or key not in obj:
        raise FetchError("%s has no %r field" % (what, key))
    return obj[key]


def _not_before(stamp, cutoff, what):
    """`FetchError` unless `stamp` (`YYYY-MM-DDTHH:MM:SSZ` from the API) is on or after the `cutoff` date."""
    if not isinstance(stamp, str) or _STAMP.fullmatch(stamp) is None:
        raise FetchError("%s is not an ISO-8601 UTC time (YYYY-MM-DDTHH:MM:SSZ): %r" % (what, stamp))
    if stamp < cutoff:
        raise FetchError("%s %s is before the cutoff %s" % (what, stamp, cutoff))


def _check_cutoff(cutoff):
    if not isinstance(cutoff, str) or _DATE.fullmatch(cutoff) is None:
        raise ValueError("cutoff must be a YYYY-MM-DD date, got %r" % (cutoff,))
    try:
        datetime.date.fromisoformat(cutoff)
    except ValueError:
        raise ValueError("cutoff is not a calendar date: %r" % (cutoff,)) from None


def _git_diff(run, env, timeout, src, base, merge_sha, renames):
    """`git diff base merge_sha` as text (surrogateescape), with the diff-shaping options pinned.

    Pinned on the command line: path quoting, textconv and external diff drivers, colour, the diff
    algorithm, blank context lines (`diff.suppressBlankEmpty`), submodule display, the a/ and b/
    prefixes, three context lines, binary patches and the rename mode below. Nothing else is
    neutralised: git configuration these flags do not name, and `.gitattributes` in the clone, can
    still change the output.

    `renames=False` (the stored patch) lists a rename as a delete plus an add; `renames=True` lets
    git pair them as GitHub does, for comparing with the PR's own diff.
    """
    raw = _call(run, ["git", "-c", "core.quotePath=false", "-c", "diff.suppressBlankEmpty=false", "-C", str(src),
                      "diff", "--no-textconv", "--no-ext-diff", "--no-color", "--diff-algorithm=myers",
                      "--submodule=short", "--find-renames" if renames else "--no-renames",
                      "--src-prefix=a/", "--dst-prefix=b/", "-U3", "--binary", base, merge_sha], env, timeout)
    return raw.decode("utf-8", errors="surrogateescape")


def fetch_pr(repo, pr, issue_numbers, dest, *, run=subprocess.run, cutoff=CUTOFF, timeout=600):
    """Everything upstream of one merged PR: its merge commit, base, diff and issue text.

    Calls `gh api` for the PR, its merge commit, its changed files (all pages), each issue and the
    PR's own diff, then clones `repo` into `<dest>/src` (or fetches the merge commit when
    `<dest>/src/.git` exists) and diffs a base against the merge commit. Returns `merge_sha`,
    `parents`, `upstream_base`, `base_strategy`, `created_at` (the PR's), `merged_at`, `title`,
    `problem_statement` (each issue's title, a newline and its body, issues joined by a blank
    line), `diff` (decoded with `surrogateescape`, byte-faithful, renames listed as a delete plus an
    add), `pr_files` (GitHub's `filename`s) and `issue_created_at` (the earliest issue's).

    The base is the first candidate whose diff is the PR's change. Candidates, in order:
    `parents[0]` (`base_strategy` "first-parent": a merge or squash commit), then, only when the
    merge commit has exactly one parent and the PR has more than one commit, `<merge>~<commits>`
    (`"rebase-n"`: a rebase merge, where the merge commit is the LAST rebased commit and the first
    parent alone gives a partial patch). A candidate qualifies when the files of its diff equal the
    PR's `filename`s plus `previous_filename`s, and `same_change` holds against the PR's own diff.

    `ValueError` for a `cutoff` that is not a `YYYY-MM-DD` date. `FetchError` when: no issue number
    is given; the PR is not merged; it was merged, or an issue created, before `cutoff` (the same
    day passes); a time is not `YYYY-MM-DDTHH:MM:SSZ`; a command fails or exceeds `timeout` seconds
    (the message carries the argv and the last 300 characters of stderr only); a reply is not the
    expected JSON; a diff names a quoted path; or no candidate base qualifies (the message lists
    each candidate with the file-set difference or the files whose changed lines differ). `gh` and
    `git` run with `GITHUB_TOKEN` removed; no argv carries a credential.
    """
    _check_cutoff(cutoff)
    if not issue_numbers:
        raise FetchError("no issue numbers given: there would be no problem statement")
    env = dict(os.environ)
    env.pop("GITHUB_TOKEN", None)
    env["GIT_TERMINAL_PROMPT"] = "0"
    api = "repos/%s" % repo

    def gh(*args):
        return _call(run, ["gh", "api", *args], env, timeout)

    pull_path = "%s/pulls/%s" % (api, pr)
    pull = _json(gh(pull_path), pull_path)
    merged_at = _field(pull, "merged_at", pull_path)
    if merged_at is None:
        raise FetchError("PR %s#%s is not merged" % (repo, pr))
    _not_before(merged_at, cutoff, "PR %s#%s merged_at" % (repo, pr))
    # a PR opened before the cutoff and merged after it is not a task of this study: refuse before any build
    _not_before(_field(pull, "created_at", pull_path), cutoff, "PR %s#%s created_at" % (repo, pr))
    merge_sha = _field(pull, "merge_commit_sha", pull_path)
    if not isinstance(merge_sha, str) or not merge_sha:
        raise FetchError("PR %s#%s has no merge_commit_sha" % (repo, pr))
    if _SHA.fullmatch(merge_sha) is None:  # it becomes a git argument below
        raise FetchError("PR %s#%s merge_commit_sha is not a full lowercase commit SHA: %r" % (repo, pr, merge_sha[:80]))

    commit_path = "%s/commits/%s" % (api, merge_sha)
    commit = _json(gh(commit_path), commit_path)
    parents = [_field(p, "sha", commit_path + " parent") for p in _field(commit, "parents", commit_path)]
    for parent in parents:
        if not isinstance(parent, str) or _SHA.fullmatch(parent) is None:
            raise FetchError("merge commit %s: parent sha is not a full lowercase commit SHA: %r"
                             % (merge_sha, str(parent)[:80]))
    try:
        first_parent = pick_base(parents)
    except ValueError as exc:
        raise FetchError("merge commit %s: %s" % (merge_sha, exc)) from exc

    files_path = "%s/files" % pull_path
    files = _json_pages(gh(files_path, "--paginate"), files_path)
    pr_files = [_field(f, "filename", files_path) for f in files]
    # A rename is `filename` (new) plus `previous_filename` (old); `--no-renames` shows both paths.
    expected_paths = set(pr_files) | {f["previous_filename"] for f in files if f.get("previous_filename")}

    statements, created = [], []
    for n in issue_numbers:
        issue_path = "%s/issues/%s" % (api, n)
        issue = _json(gh(issue_path), issue_path)
        issue_created = _field(issue, "created_at", issue_path)
        _not_before(issue_created, cutoff, "issue %s#%s created_at" % (repo, n))
        statements.append("%s\n%s" % (_field(issue, "title", issue_path), _field(issue, "body", issue_path) or ""))
        created.append(issue_created)

    pr_diff = gh(pull_path, "-H", DIFF_ACCEPT).decode("utf-8", errors="surrogateescape")
    try:
        pr_changes = changed_lines(pr_diff)
    except ValueError as exc:
        raise FetchError("cannot read the diff of PR %s#%s: %s" % (repo, pr, exc)) from exc

    src = Path(dest) / "src"
    if (src / ".git").exists():
        _call(run, ["git", "-C", str(src), "fetch", "origin", merge_sha], env, timeout)
    else:
        _call(run, ["git", "clone", "--filter=blob:none", "--no-checkout",
                    "https://github.com/%s" % repo, str(src)], env, timeout)

    def candidates():
        yield first_parent, "first-parent"
        if len(parents) == 1:
            commits = _field(pull, "commits", pull_path)
            if isinstance(commits, int) and not isinstance(commits, bool) and commits > 1:
                out = _call(run, ["git", "-C", str(src), "rev-parse", "%s~%d" % (merge_sha, commits)], env, timeout)
                sha = out.decode("ascii", errors="replace").strip()
                if _SHA.fullmatch(sha) is None:
                    raise FetchError("git rev-parse %s~%d did not print a commit id: %r" % (merge_sha, commits, sha))
                yield sha, "rebase-n"

    def mismatch(base):
        """`(diff, None)` when `base..merge` is the PR's change, else `(diff, why not)`."""
        diff = _git_diff(run, env, timeout, src, base, merge_sha, renames=False)
        try:
            diff_paths = set(diff_files(diff))
            only_diff = sorted(diff_paths - expected_paths)
            only_pr = sorted(expected_paths - diff_paths)
            if only_diff or only_pr:
                return diff, "only in diff: %r; only in PR: %r" % (only_diff, only_pr)
            # GitHub pairs renames; compare like with like, so ask git for the paired form too.
            has_rename = any(f.get("previous_filename") for f in files)
            paired = _git_diff(run, env, timeout, src, base, merge_sha, renames=True) if has_rename else diff
            got = changed_lines(paired)
        except ValueError as exc:
            raise FetchError("cannot read the diff %s..%s: %s" % (base, merge_sha, exc)) from exc
        differing = sorted(p for p in set(got) | set(pr_changes) if got.get(p) != pr_changes.get(p))
        if differing:
            return diff, "changed lines differ from the PR's own diff in: %r" % differing
        return diff, None

    tried = []
    for base, strategy in candidates():
        diff, why = mismatch(base)
        if why is None:
            break
        tried.append("%s %s: %s" % (strategy, base, why))
    else:
        raise FetchError("no base candidate gives the change of PR %s#%s (merge %s); tried: %s"
                         % (repo, pr, merge_sha, " | ".join(tried)))
    return {
        "merge_sha": merge_sha,
        "parents": parents,
        "upstream_base": base,
        "base_strategy": strategy,
        "created_at": _field(pull, "created_at", pull_path),
        "merged_at": merged_at,
        "title": _field(pull, "title", pull_path),
        "problem_statement": "\n\n".join(statements),
        "diff": diff,
        "pr_files": pr_files,
        "issue_created_at": min(created),
    }


class BuildError(RuntimeError):
    """The image build was refused (disk, arguments) or a step failed. No image tag is left behind."""


_DOCKER_NAME = re.compile(r"[A-Za-z0-9]+(?:(?:[._]|__|-+)[A-Za-z0-9]+)*")
_BASE_IMAGE = re.compile(r"[A-Za-z0-9][A-Za-z0-9_.:/@-]*")
_WORKDIR = re.compile(r"/[A-Za-z0-9_.-]+(?:/[A-Za-z0-9_.-]+)*")
_TREE_TAR = "/tmp/sab-tree.tar"  # inside the container; removed by the extract step, never committed
_CONTAINER_ID = re.compile(r"^[0-9a-f]{64}$", re.MULTILINE)
_SHA40 = re.compile(r"[0-9a-f]{40}")  # a full SHA-1 object name: a tree, or the synthetic commit
_IMAGE_ID = re.compile(r"sha256:[0-9a-f]{64}")
_TOOLCHAIN = re.compile(r"[A-Za-z0-9][A-Za-z0-9._-]*")  # a rustup toolchain name, as `cargo +<name>` takes it
_LISTED = 10  # differing paths named in an error
_LOG_TAIL = 1500
_EVIDENCE_TAIL = 500
_EVIDENCE_FILE = "build-evidence.json"
_REFUSED_FILE = "build-evidence.refused.json"  # a "tag exists" refusal: never replaces `_EVIDENCE_FILE`
_COMPILING = re.compile(r"^[ \t]*Compiling ", re.MULTILINE)

# Both scripts run with `bash -o pipefail -c` in the work directory. The first deletes, until none is
# left, every empty directory except the top-level `target` and `.git` (a crate directory kept only
# as empty `src/` or `tests/` breaks the `crates/*` workspace glob). The second sets every file's
# mtime to now except under `target/` and `.git`: `git archive` gives old mtimes, older than the
# warm `target/`, and cargo would reuse stale test binaries.
_EMPTY_DIRS = (
    "while :; do "
    "found=$(find . -mindepth 1 -depth -type d -empty -not -path './target' -not -path './target/*' "
    "-not -path './.git' -not -path './.git/*' -print -delete) || exit 1; "
    "[ -n \"$found\" ] || break; done"
)
_TOUCH = r"find . \( -path ./target -o -path ./.git \) -prune -o \( -type f -o -type l \) -exec touch -h {} +"
_STEP_NAMES = {
    "run": "docker run", "clear-tracked": "delete tracked files", "clear-dirs": "delete empty directories",
    "copy": "docker cp", "extract": "extract tree", "touch": "touch files", "metadata": "cargo metadata",
    "metadata-network": "cargo metadata (network)", "toolchain": "toolchain versions", "warm-build": "warm build", "git-add": "git add",
    "git-add-force": "git add -f", "git-commit": "git commit", "ls-files": "git ls-files",
    "rev-parse": "git rev-parse", "docker-commit": "docker commit",
    "inspect": "docker image inspect", "rmi": "docker rmi", "tag-check": "docker image inspect (tag)",
    "tag-recheck": "docker image inspect (tag, before the commit)", "tree-rev-parse": "git rev-parse HEAD^{tree}",
    "gate-image-id": "docker image inspect (gate image)",
    "base-id": "docker image inspect (base image)", "list-files": "list files",
    "delete-extras": "delete leftover files", "git-attributes": "write .git/info/attributes",
}
# Pinned on every `git add` and `git commit`, so the base image's git configuration cannot rewrite bytes:
# no CRLF conversion, a real file mode, no system attributes file. `.git/info/attributes` outranks the
# tree's own .gitattributes (`text`/`eol`/filters would still convert on `git add`), so it is written
# first with the attributes that make git store the working-copy bytes as they are.
_GIT_PINNED = ("-c", "core.autocrlf=false", "-c", "core.safecrlf=false", "-c", "core.fileMode=true")
_GIT_ENV = ("GIT_ATTR_NOSYSTEM=1",)
_INFO_ATTRIBUTES = "mkdir -p .git/info && printf '* -text -ident -filter\\n' > .git/info/attributes"


def _fresh_id(repo, pr):
    """`<owner>__<repo>-<pr>`, lowercase: the id both the container name and the image tag are built from.

    `ValueError` unless `repo` is `owner/name` and the id is a valid docker name.
    """
    if isinstance(pr, bool) or not isinstance(pr, int) or pr <= 0:
        raise ValueError("pr must be a positive integer, got %r" % (pr,))
    if not isinstance(repo, str) or repo.count("/") != 1:
        raise ValueError("repo must be 'owner/name', got %r" % (repo,))
    owner, name = repo.split("/")
    ident = ("%s__%s-%d" % (owner, name, pr)).lower()
    if not all(_DOCKER_NAME.fullmatch(part) for part in (owner, name, ident)):
        raise ValueError("repo %r cannot be part of a docker name" % (repo,))
    return ident


def image_tag(repo, pr):
    """`sab-fresh-<owner>__<repo>-<pr>:b<BUILDER_VERSION>`, the study image of one PR."""
    return "sab-fresh-%s:b%s" % (_fresh_id(repo, pr), BUILDER_VERSION)


def _storage_roots(config=Path("/etc/docker/daemon.json")):
    """Where docker keeps its data: the daemon's `data-root` (or `/var/lib/docker`) and the containerd store."""
    try:
        root = json.loads(Path(config).read_text(encoding="utf-8")).get("data-root")
    except (OSError, ValueError, AttributeError):
        root = None
    return [root if isinstance(root, str) and root else "/var/lib/docker", "/var/lib/containerd"]


def data_root_free_gb(roots=None):
    """The smallest free space, in GB (1e9 bytes), over the docker storage roots that can be read.

    `BuildError` when none can be read: the disk guard fails closed rather than guessing. Reads the
    file system only; it runs no docker command.
    """
    free = []
    for root in _storage_roots() if roots is None else roots:
        try:
            free.append(shutil.disk_usage(root).free / 1e9)
        except OSError:
            continue
    if not free:
        raise BuildError("cannot read the free space of the docker storage; refusing to build")
    return min(free)


def _text(proc):
    out = proc.stdout
    if out is None:
        return ""
    return out.decode("utf-8", errors="replace") if isinstance(out, bytes) else str(out)


def _git_blob_id(data):
    return hashlib.sha1(b"blob %d\0" % len(data) + data).hexdigest()


def _tree_files(tree_tar):
    """`(entries, sha256)` of the tree tarball: `{path: (git mode, git blob id)}` for every file and
    symlink in it (directories are not listed), and the hex SHA-256 of the archive.

    The mode is `100755` when any execute bit is set, `100644` otherwise, `120000` for a symlink
    (whose blob is its target text). The blob id is git's SHA-1 of `blob <size>\\0<bytes>`, of the
    bytes as archived: it is what `git add` must store. A hard link has the entry of its target.
    `BuildError` for an archive that cannot be read, holds no file, names an unsafe or non-UTF-8
    path, or has a hard link whose target is not in it."""
    digest, entries, links = hashlib.sha256(), {}, {}
    try:
        with open(tree_tar, "rb") as f:
            for chunk in iter(lambda: f.read(1 << 20), b""):
                digest.update(chunk)
        with tarfile.open(tree_tar, "r:*") as tar:
            for member in tar:
                if not (member.isreg() or member.issym() or member.islnk()):
                    continue
                name = _tar_path(member.name, tree_tar)
                if member.islnk():
                    links[name] = _tar_path(member.linkname, tree_tar)
                elif member.issym():
                    try:
                        target = member.linkname.encode("utf-8")
                    except UnicodeEncodeError:
                        raise BuildError("tree tarball %s: symlink %s has a target that is not UTF-8"
                                         % (tree_tar, name)) from None
                    entries[name] = ("120000", _git_blob_id(target))
                else:
                    mode = "100755" if member.mode & 0o111 else "100644"
                    entries[name] = (mode, _git_blob_id(tar.extractfile(member).read()))
    except (OSError, tarfile.TarError) as exc:
        raise BuildError("cannot read tree tarball %s: %s" % (tree_tar, exc)) from exc
    for name, target in links.items():
        if target not in entries:
            raise BuildError("tree tarball %s: hard link %s points at %s, which is not a file of the archive"
                             % (tree_tar, name, target))
        entries[name] = entries[target]
    if not entries:
        raise BuildError("tree tarball %s holds no file" % tree_tar)
    return entries, digest.hexdigest()


def _tar_path(name, tree_tar):
    """`name` without a leading `./`; `BuildError` for an unsafe or non-UTF-8 path."""
    clean = name
    while clean.startswith("./"):
        clean = clean[2:]
    if clean.startswith("/") or {"", ".", ".."} & set(clean.split("/")) or "\0" in clean:
        raise BuildError("tree tarball %s holds an unsafe path: %r" % (tree_tar, name))
    try:
        clean.encode("utf-8")
    except UnicodeEncodeError:
        raise BuildError("tree tarball %s holds a path that is not UTF-8: %r" % (tree_tar, clean)) from None
    return clean


_INDEX_ENTRY = re.compile(r"([0-7]{6}) ([0-9a-f]{40}|[0-9a-f]{64}) ([0-3])\t(.+)", re.DOTALL)


def _index_entries(text):
    """`{path: (mode, blob id)}` of `git ls-files -s -z` output. `BuildError` for an entry that is not
    `<mode> <blob> <stage>\\t<path>` or is not at stage 0 (an unmerged path)."""
    found = {}
    for raw in text.split("\0"):
        if not raw:
            continue
        m = _INDEX_ENTRY.fullmatch(raw)
        if m is None:
            raise BuildError("git ls-files printed something that is not an index entry: %r" % raw[:200])
        if m.group(3) != "0":
            raise BuildError("git ls-files lists %s at stage %s (unmerged)" % (m.group(4), m.group(3)))
        found[m.group(4)] = (m.group(1), m.group(2))
    return found


def _tree_mismatch(expected, committed):
    """What differs between the tarball's `{path: (mode, blob)}` and the committed index, as text; '' when equal."""
    parts = []
    missing, extra = set(expected) - set(committed), set(committed) - set(expected)
    changed = {p for p in set(expected) & set(committed) if expected[p] != committed[p]}
    if missing:
        parts.append("missing from the commit: " + _named(missing))
    if extra:
        parts.append("not in the upstream tree: " + _named(extra))
    if changed:
        shown = sorted(changed)[:_LISTED]
        notes = []
        for path in shown:
            (want_mode, want_blob), (mode, blob) = expected[path], committed[path]
            what = []
            if want_mode != mode:
                what.append("mode %s -> %s" % (want_mode, mode))
            if want_blob != blob:
                what.append("blob %s -> %s" % (want_blob[:12], blob[:12]))
            notes.append("%s (%s)" % (path, ", ".join(what)))
        more = " (+%d more)" % (len(changed) - len(shown)) if len(changed) > len(shown) else ""
        parts.append("differs: " + ", ".join(notes) + more)
    return "; ".join(parts)


def _nul_list(paths, prefix=""):
    return "".join("%s%s\0" % (prefix, p) for p in sorted(paths))


def _from_nul(text, prefix, what):
    """The paths of a NUL-separated listing, each starting with `prefix` (removed). `BuildError` otherwise."""
    out = set()
    for entry in text.split("\0"):
        if not entry:
            continue
        if not entry.startswith(prefix):
            raise BuildError("%s printed something that is not a path: %r" % (what, entry[:200]))
        out.add(entry[len(prefix):])
    return out


def _named(paths):
    shown = sorted(paths)[:_LISTED]
    return ", ".join(shown) + (" (+%d more)" % (len(paths) - len(shown)) if len(paths) > len(shown) else "")


def _cidfile_id(path):
    """The container ID `docker run --cidfile` wrote to `path`: 64 hex digits (surrounding whitespace
    allowed), else None (no file, an empty one, anything else)."""
    if path is None:
        return None
    try:
        text = Path(path).read_text(encoding="ascii").strip()
    except (OSError, ValueError):
        return None
    return text if re.fullmatch(r"[0-9a-f]{64}", text) else None


def _cid_dir(evidence_dir):
    """A new directory inside `evidence_dir` (absolute, unique) for the `--cidfile` of ONE `docker run`.
    A fixed file name in a directory several runs share would let one run read another's container ID
    and remove that container. The caller removes the directory afterwards."""
    try:
        return Path(tempfile.mkdtemp(prefix=".cid-", dir=Path(evidence_dir).resolve()))
    except OSError as exc:
        raise BuildError("cannot create a cidfile directory in %s: %s" % (evidence_dir, exc)) from exc


class _Build:
    """The docker calls of one image build and their record. Every exit code comes from the
    `CompletedProcess` of the docker call itself (`check=False`), never from a wrapper.

    The container is addressed by the ID `docker run -d` printed. `name` is used only when `docker
    run` gave no answer at all (timeout, no docker binary): then the container may exist, and it is
    removed by name. A `docker run` that answered with a non-zero exit created nothing of ours (the
    usual cause is a name held by another build), so nothing is removed, unless the `--cidfile` it
    was given holds a container ID: then the container was created by this run and is removed by ID."""

    def __init__(self, docker, name, workdir):
        self.docker = docker
        self.name = name
        self.workdir = workdir
        self.cid = None
        self.unanswered = False  # `docker run` gave no answer: a container of this name may be ours
        self.note = None
        self.interrupt = None  # a KeyboardInterrupt/SystemExit met while removing: re-raised after the evidence
        self.steps = []

    def run(self, step, *args, timeout, data=None):
        """One docker call. `BuildError` when it times out or cannot start; a failing exit code is returned."""
        label = _STEP_NAMES[step]
        start = time.monotonic()
        problem = None
        extra = {} if data is None else {"data": data}
        try:
            proc = self.docker(*args, check=False, timeout=timeout, **extra)
        except subprocess.TimeoutExpired as exc:
            problem, cause = "%s timed out after %ss" % (label, timeout), exc
        except (OSError, ValueError, RuntimeError) as exc:
            problem, cause = "%s: cannot run docker: %s" % (label, exc), exc
        seconds = round(time.monotonic() - start, 1)
        if problem is not None:
            self.steps.append({"name": step, "returncode": None, "seconds": seconds, "error": problem})
            raise BuildError(problem) from cause
        self.steps.append({"name": step, "returncode": proc.returncode, "seconds": seconds,
                           "tail": _text(proc)[-_EVIDENCE_TAIL:]})
        return proc

    def must(self, step, *args, timeout, data=None):
        """`run`, and `BuildError` carrying the output tail unless the exit code is 0."""
        proc = self.run(step, *args, timeout=timeout, data=data)
        if proc.returncode != 0:
            raise BuildError("%s failed (exit %s): %s" % (_STEP_NAMES[step], proc.returncode,
                                                          _text(proc).rstrip()[-_LOG_TAIL:]))
        return proc

    def start(self, *args, timeout, cidfile=None):
        """`docker run -d`; remembers the container ID. See the class docstring for what is removed after a failure.

        With `cidfile` (the path `args` pass as `--cidfile`, in a fresh directory made for this call, see
        `_cid_dir`): a `docker run` that exits non-zero but left a 64-hex ID in it DID create a container
        (it was created, then failed to start): that ID is ours."""
        try:
            proc = self.run("run", *args, timeout=timeout)
        except BaseException:  # no answer (timeout, no binary, interrupt): a container of this name may be ours
            self.unanswered = True
            raise
        if proc.returncode != 0:
            created = _cidfile_id(cidfile)
            if created is None:
                self.note = ("docker run exited %s: it created no container of this build, so nothing was removed"
                             % proc.returncode)
            else:
                self.cid = created
                self.note = ("docker run exited %s after creating container %s (its cidfile): removed by ID"
                             % (proc.returncode, created))
            raise BuildError("docker run failed (exit %s): %s" % (proc.returncode, _text(proc).rstrip()[-_LOG_TAIL:]))
        ids = _CONTAINER_ID.findall(_text(proc))
        if not ids:
            self.unanswered = True
            raise BuildError("docker run printed no container ID: %r" % _text(proc)[-_LOG_TAIL:])
        self.cid = ids[-1]

    def exec(self, step, *cmd, env=(), data=None, check=True, timeout=1800):
        """`docker exec` of `cmd` in the work directory; `env` entries are `-e` options of this exec
        only; `data` (text) is the standard input."""
        args = ["exec"]
        if data is not None:
            args.append("-i")
        for entry in env:
            args += ["-e", entry]
        args += ["-w", self.workdir, self.cid, *cmd]
        return (self.must if check else self.run)(step, *args, timeout=timeout, data=data)

    def bash(self, step, script, timeout=1800):
        return self.exec(step, "bash", "-o", "pipefail", "-c", script, timeout=timeout)

    def drop_tag(self, tag):
        """Best-effort `docker rmi <tag>`, for a tag this build attempted to create; never raises."""
        try:
            self.run("rmi", "rmi", tag, timeout=600)
        except BuildError:
            pass

    def remove(self):
        """`docker rm -f -v` of the container, by ID (by name only after a `docker run` with no answer).
        Never raises: the outcome is returned for the evidence. An interrupt met here is kept in
        `self.interrupt` for the caller to re-raise once the evidence is written."""
        if self.cid is not None:
            target, by_name = self.cid, False
        elif self.unanswered:
            target, by_name = self.name, True
        else:
            return {"container_id": None, "container_removed": None, "container_removed_by_name": False,
                    "removal_error": None, "removal_note": self.note or "no container was created by this build"}
        rec = {"container_id": self.cid, "container_removed_by_name": by_name, "removal_note": self.note}
        try:
            proc = self.docker("rm", "-f", "-v", target, check=False, timeout=600)
        except BaseException as exc:  # a failed removal must not replace the error that got us here
            if not isinstance(exc, Exception):
                self.interrupt = exc
            rec.update(container_removed=False, removal_error="%s: %s" % (type(exc).__name__, exc))
            return rec
        out = _text(proc).rstrip()
        if proc.returncode == 0 or "No such container" in out:
            rec.update(container_removed=True, removal_error=None)
        else:
            rec.update(container_removed=False, removal_error="exit %s: %s" % (proc.returncode, out[-_LOG_TAIL:]))
        return rec


class _TagExists(BuildError):
    """The image tag is already there: a refusal, whose evidence must not replace another build's."""


def _require_tag_absent(build, step, tag, when):
    """`_TagExists` when the tag exists, `BuildError` unless it is plainly absent (`docker image
    inspect` exit 1 and "no such image")."""
    proc = build.run(step, "image", "inspect", "--format", "{{.Id}}", tag, timeout=120)
    if proc.returncode == 0:
        raise _TagExists("tag exists: image %s is already there %s; this build never overwrites or deletes "
                         "an image it did not make (the cache check belongs to the caller)" % (tag, when))
    if proc.returncode != 1 or "no such image" not in _text(proc).lower():
        raise BuildError("docker image inspect %s failed (exit %s): %s"
                         % (tag, proc.returncode, _text(proc).rstrip()[-_LOG_TAIL:]))


def _write_evidence(evidence_dir, ev, name=_EVIDENCE_FILE):
    (evidence_dir / name).write_text(json.dumps(ev, indent=2, allow_nan=False) + "\n", encoding="utf-8")


def build_image(*, base_image, repo, pr, upstream_base, tree_tar, evidence_dir, docker=pilot.docker,
                free_gb=data_root_free_gb, workdir="/testbed", expected_tree=None, toolchains=()):
    """Build the study image of one PR: the upstream tree swapped into a task image, warm-built, committed.

    `base_image` is a task image whose `workdir` repo has no remote and one synthetic commit;
    `tree_tar` is the `git archive` of `upstream_base`. Steps, each exit code read from the docker
    call itself: refuse under `MIN_FREE_GB` free (`free_gb()`, before any docker call) and an
    unreadable tarball; refuse when the image tag already exists (the cache belongs to the caller;
    this build never overwrites or deletes an image it did not make); read the base image's ID;
    `docker run` a container `sab-fresh-build-<id>` with network, kept by the ID it prints; delete
    the tracked files, copy in and extract the tree; list the files that remain and delete every
    one that is not in the tarball (a leftover untracked or ignored file, a file of an old
    directory), then the empty directories (not `target`, not `.git`); touch the files;
    `cargo metadata` must succeed (offline first, then with the network); warm `cargo test
    --no-run` with `-e CARGO_NET_OFFLINE=false` on that exec only, whose log must hold at least one
    `Compiling` line (none means stale binaries); `git add -A`, then `git add -f` of every tarball
    path (the new `.gitignore` would drop tracked files that it matches), a synthetic commit, and a
    check that `git ls-files -s` equals the tarball's `{path: (mode, blob id)}`: the blob id is git's own
    hash of the archived bytes, so a rewritten line ending, filter or file mode is an error, not a silent
    difference. That check compares the commit with the archive, and `git archive` applies the `eol`,
    `export-ignore` and `export-subst` attributes, so an archive can differ from the upstream tree: with
    `expected_tree` (a full lowercase 40-hex tree SHA, `git rev-parse <upstream_base>^{tree}` on the
    host clone) the commit's `HEAD^{tree}` must equal it, or the build is refused before `docker
    commit`, naming both SHAs; the evidence records `expected_tree` and `synthetic_tree`. Every `git add` and `git commit` pins `core.autocrlf=false`, `core.safecrlf=false`,
    `core.fileMode=true` and `GIT_ATTR_NOSYSTEM=1`, after `.git/info/attributes` is set to
    `* -text -ident -filter` (it outranks the tree's `.gitattributes`); the tag is inspected once more
    right before `docker commit` and the build is refused if it appeared meanwhile (a sibling build),
    so a later failure never drops another build's image; `docker commit` (restoring
    `CMD ["/bin/bash"]`) to `image_tag(repo, pr)`. The container starts from the base image's ID (the one
    read above), not from its mutable reference.

    The container is removed in a `finally`, by ID. A `docker run` that answered with a failure
    removes nothing (the name may belong to another build) unless its `--cidfile`
    (`run.cid` in a directory `.cid-*` made inside `evidence_dir` for this call and removed after it) holds a container ID, which is
    removed by ID; one that gave no answer removes by
    name, and the evidence says so. The tag is dropped (`docker rmi`) on any failure from the
    `docker commit` attempt on, `KeyboardInterrupt` included, and never otherwise.

    `toolchains` (plain rustup names such as `stable`, from the template's `cargo +stable` commands): after
    `cargo metadata`, one step records `cargo -V` and `rustc -V` for the default toolchain and each named
    one into the evidence (`toolchains`, `toolchain_versions`). Evidence only: a failure there refuses nothing.

    Writes `build.log` (the warm build's output as `docker exec` returned it: `pilot.command` decodes
    text mode, so it is not byte-exact for a non-UTF-8 or carriage-return stream) and
    `build-evidence.json` to `evidence_dir`, also on failure, except that a "tag exists" refusal (at the
    start or before the commit) writes `build-evidence.refused.json` and never touches
    `build-evidence.json`. Returns `image`, `base_commit` (the
    synthetic commit's SHA), `base_image_id`, `tree_tar_sha256`, `image_id`, `size_bytes`,
    `laps_seconds`, `compiling_lines` and `container_removed` (a failed removal is recorded, never
    raised over the original error). `ValueError` for a bad argument, `BuildError` for a refused or
    failed build.
    """
    ident = _fresh_id(repo, pr)
    tag = image_tag(repo, pr)
    container = "sab-fresh-build-" + ident
    if not isinstance(base_image, str) or _BASE_IMAGE.fullmatch(base_image) is None:
        raise ValueError("base_image is not an image reference: %r" % (base_image,))
    if not isinstance(upstream_base, str) or _SHA.fullmatch(upstream_base) is None:
        raise ValueError("upstream_base is not a full commit SHA: %r" % (upstream_base,))
    if not isinstance(workdir, str) or _WORKDIR.fullmatch(workdir) is None or {".", ".."} & set(workdir.split("/")):
        raise ValueError("workdir must be a plain absolute path below /, got %r" % (workdir,))
    if expected_tree is not None and (not isinstance(expected_tree, str) or _SHA40.fullmatch(expected_tree) is None):
        raise ValueError("expected_tree is not a full lowercase 40-hex tree SHA: %r" % (expected_tree,))
    if isinstance(toolchains, (str, bytes)) or not all(isinstance(t, str) and _TOOLCHAIN.fullmatch(t) for t in toolchains):
        raise ValueError("toolchains must be plain rustup toolchain names (stable, nightly-2026-01-01), got %r"
                         % (toolchains,))
    toolchains = list(toolchains)
    tree_tar, evidence_dir = Path(tree_tar), Path(evidence_dir)
    if not tree_tar.is_file():
        raise BuildError("tree tarball %s is not a file" % tree_tar)
    free = free_gb()
    if isinstance(free, bool) or not isinstance(free, (int, float)) or not free >= MIN_FREE_GB:
        raise BuildError("%s GB free on the docker storage, at least %d GB needed" % (free, MIN_FREE_GB))
    tree_tar = tree_tar.resolve()
    expected, tar_sha = _tree_files(tree_tar)
    evidence_dir.mkdir(parents=True, exist_ok=True)

    build = _Build(docker, container, workdir)
    cid_dir = None  # a directory of its own for this call's `--cidfile`, removed in the `finally`
    ev = {
        "builder_version": BUILDER_VERSION, "repo": repo, "pr": pr, "upstream_base": upstream_base,
        "base_image": base_image, "image": tag, "container": container, "workdir": workdir,
        "tree_tar_sha256": tar_sha, "tree_files": len(expected),
        "outcome": "error", "error": None, "steps": build.steps,
    }
    laps = ev["laps_seconds"] = {}
    mark = [time.monotonic()]

    def lap(key):
        now = time.monotonic()
        laps[key] = round(now - mark[0], 1)
        mark[0] = now

    failed = False
    commit_attempted = False
    refused = False  # a "tag exists" refusal: its evidence goes to `_REFUSED_FILE`, never over `_EVIDENCE_FILE`
    try:
        _require_tag_absent(build, "tag-check", tag, "before this build")
        proc = build.must("base-id", "image", "inspect", "--format", "{{.Id}}", base_image, timeout=120)
        ids = _text(proc).split()
        if not ids or _IMAGE_ID.fullmatch(ids[-1]) is None:
            raise BuildError("base image %s: docker image inspect gave no image ID: %r"
                             % (base_image, _text(proc)[-_LOG_TAIL:]))
        ev["base_image_id"] = ids[-1]

        cid_dir = _cid_dir(evidence_dir)
        cidfile = cid_dir / "run.cid"
        build.start("run", "-d", "--name", container, "--cidfile", str(cidfile), "--cpus", "4", "--memory", "24g",
                    ev["base_image_id"], "sleep", "infinity", timeout=600, cidfile=cidfile)
        ev["container_id"] = build.cid
        lap("run")

        build.bash("clear-tracked", "git ls-files -z | xargs -0 rm -f", timeout=600)
        build.must("copy", "cp", str(tree_tar), "%s:%s" % (build.cid, _TREE_TAR), timeout=1800)
        build.bash("extract", "tar -xf %s -C . && rm -f %s" % (_TREE_TAR, _TREE_TAR))
        # what is left besides the new tree: untracked or ignored files of the old tree, and the
        # files of directories the new tree does not have. `target/` and `.git` are not looked at.
        listing = build.exec("list-files", "find", ".", "-path", "./target", "-prune", "-o", "-path", "./.git",
                             "-prune", "-o", "(", "-type", "f", "-o", "-type", "l", ")", "-print0", timeout=600)
        extras = _from_nul(_text(listing), "./", "find") - set(expected)
        ev["leftover_files_deleted"] = len(extras)
        if extras:
            build.exec("delete-extras", "xargs", "-0", "rm", "-f", data=_nul_list(extras, "./"), timeout=600)
        build.bash("clear-dirs", _EMPTY_DIRS, timeout=600)
        build.bash("touch", _TOUCH)
        lap("extract")

        meta = ("cargo", "metadata", "--format-version", "1")
        if build.exec("metadata", *meta, check=False, timeout=1200).returncode != 0:
            build.exec("metadata-network", *meta, env=("CARGO_NET_OFFLINE=false",), timeout=1200)
        lap("metadata")

        # evidence only (a toolchain pinned by the repository may differ from the default one): never a refusal
        names = [""] + ["+" + t for t in toolchains]
        script = ('for t in %s; do echo "== ${t:-default}"; cargo $t -V 2>&1; rustc $t -V 2>&1; done; true'
                   % " ".join(n or '""' for n in names))
        try:
            versions = _text(build.exec("toolchain", "bash", "-o", "pipefail", "-c", script, check=False,
                                        timeout=300))[-_LOG_TAIL:]
        except BuildError as exc:
            versions = "not recorded: %s" % exc
        ev["toolchains"], ev["toolchain_versions"] = names, versions

        warm = build.exec("warm-build", "cargo", "test", "--no-run", env=("CARGO_NET_OFFLINE=false",),
                          check=False, timeout=7200)
        log = _text(warm)
        with open(evidence_dir / "build.log", "w", encoding="utf-8", errors="replace", newline="") as f:
            f.write(log)
        if warm.returncode != 0:
            raise BuildError("warm build failed (exit %s): %s" % (warm.returncode, log.rstrip()[-_LOG_TAIL:]))
        compiling = len(_COMPILING.findall(log))
        if compiling < 1:
            raise BuildError("warm build printed no Compiling line: the test binaries are stale "
                             "(archive mtimes are older than target/)")
        lap("warm_build")

        message = "synthetic base: %s %s (parent of PR %d merge)" % (repo, upstream_base[:12], pr)
        git = ("git", *_GIT_PINNED)
        build.bash("git-attributes", _INFO_ATTRIBUTES, timeout=600)
        build.exec("git-add", *git, "add", "-A", env=_GIT_ENV, timeout=3600)
        # `git add -A` leaves out a tracked file the new .gitignore matches: add every path of the tree
        build.exec("git-add-force", *git, "add", "-f", "--pathspec-from-file=-", "--pathspec-file-nul",
                   env=_GIT_ENV + ("GIT_LITERAL_PATHSPECS=1",), data=_nul_list(expected), timeout=3600)
        build.exec("git-commit", *git, "-c", "user.name=sab", "-c", "user.email=sab@invalid", "commit", "-q",
                   "-m", message, env=_GIT_ENV, timeout=3600)
        # content, not only names: every blob id and mode of the commit must be the tarball's
        committed = _index_entries(_text(build.exec("ls-files", "git", "ls-files", "-s", "-z", timeout=600)))
        mismatch = _tree_mismatch(expected, committed)
        if mismatch:
            raise BuildError("the synthetic commit is not the upstream tree (%s)" % mismatch)
        base_commit = _text(build.exec("rev-parse", "git", "rev-parse", "HEAD", timeout=120)).strip()
        if _SHA.fullmatch(base_commit) is None:
            raise BuildError("git rev-parse HEAD gave no commit SHA: %r" % base_commit[-_LOG_TAIL:])
        lap("git_commit")

        if expected_tree is not None:
            ev["expected_tree"] = expected_tree
            tree = _text(build.exec("tree-rev-parse", "git", "rev-parse", "HEAD^{tree}", timeout=120)).strip()
            if _SHA40.fullmatch(tree) is None:
                raise BuildError("git rev-parse HEAD^{tree} gave no tree SHA: %r" % tree[-_LOG_TAIL:])
            ev["synthetic_tree"] = tree
            if tree != expected_tree:
                raise BuildError("the synthetic commit's tree %s is not the expected upstream tree %s (the archive "
                                 "and the upstream tree differ, e.g. by an eol, export-ignore or export-subst "
                                 "attribute)" % (tree, expected_tree))
        _require_tag_absent(build, "tag-recheck", tag, "(it appeared while this build ran)")

        commit_attempted = True
        build.must("docker-commit", "commit", "--change", 'CMD ["/bin/bash"]', build.cid, tag, timeout=3600)
        proc = build.run("inspect", "image", "inspect", "--format", "{{.Id}} {{.Size}}", tag, timeout=120)
        fields = _text(proc).split()
        if proc.returncode != 0 or len(fields) != 2 or _IMAGE_ID.fullmatch(fields[0]) is None \
                or not fields[1].isdigit():
            raise BuildError("docker image inspect of %s failed (exit %s): %s"
                             % (tag, proc.returncode, _text(proc).rstrip()[-_LOG_TAIL:]))
        lap("image_commit")

        ev.update(outcome="ok", base_commit=base_commit, image_id=fields[0], size_bytes=int(fields[1]),
                  compiling_lines=compiling)
    except BaseException as exc:
        failed = True
        refused = isinstance(exc, _TagExists)
        ev["error"] = "%s: %s" % (type(exc).__name__, exc)
        if commit_attempted:
            build.drop_tag(tag)  # from the commit attempt on only: the tag exists because of this build
        raise
    finally:
        if cid_dir is not None:
            shutil.rmtree(cid_dir, ignore_errors=True)
        ev.update(build.remove())
        evidence_error = None
        try:
            _write_evidence(evidence_dir, ev, _REFUSED_FILE if refused else _EVIDENCE_FILE)
        except OSError as exc:
            evidence_error = exc
        if build.interrupt is not None:
            raise build.interrupt
        if evidence_error is not None and not failed:
            build.drop_tag(tag)
            raise BuildError("cannot write the build evidence: %s" % evidence_error) from evidence_error
    return {"image": tag, "base_commit": ev["base_commit"], "base_image_id": ev["base_image_id"],
            "tree_tar_sha256": tar_sha, "image_id": ev["image_id"], "size_bytes": ev["size_bytes"],
            "laps_seconds": laps, "compiling_lines": ev["compiling_lines"],
            "container_removed": ev["container_removed"]}


CONTROL_PREFIX = "sab-fresh-ctl-"
# The gate runs at the grader's frozen limit. In the pilot it FAILED at 24 GB only because two tests'
# `--nocapture` output interleaved and the parser lost one test's line although the test passed; at 8 GB
# the same gate passed. A gate passed at another memory limit proves nothing about the grader's.
CONTROL_MEMORY = "8g"
TOLERATED_MISMATCHES = 3
_MEMORY = re.compile(r"[1-9][0-9]*[kmg]")
_RECORD_KEYS = ("instance_id", "docker_image", "base_commit", "patch", "test_patch", "test_cmds", "rebuild_cmds",
                "print_cmds", "log_parser")
_CONTROL_EVIDENCE = "control-evidence.json"  # written on every path, an interrupt included
_NO_F2P = "no fail-to-pass test"
_REPEAT_MISMATCH = "tests differ between two gold runs"
_BASE_NO_TESTS = "base_parsed_no_tests"
_P2P_EMPTY = "p2p_empty"
_LOG_TRUNCATED = "log_truncated"
# what a failing step raises: ControlError, ParserError and BuildError are RuntimeError/ValueError;
# ValueError also covers odd bytes in docker output (UnicodeDecodeError), OSError a log that cannot be written
_STEP_ERRORS = (RuntimeError, ValueError, OSError, subprocess.TimeoutExpired)


def _write_log(evidence_dir, name, text):
    """`text` as it is: UTF-8, and no newline translation (a CRLF or a lone CR in a log stays)."""
    with open(evidence_dir / name, "w", encoding="utf-8", errors="replace", newline="") as f:
        f.write(text)


def _check_record(record):
    """`ValueError` unless `record` has what `control_fresh` reads, in the shape it reads it: a caller bug,
    caught before docker rather than as a `KeyError` or `TypeError` in the middle of a run."""
    if not isinstance(record, dict):
        raise ValueError("record must be a dict, got %s" % type(record).__name__)
    missing = [key for key in _RECORD_KEYS if key not in record]
    if missing:
        raise ValueError("record lacks %s" % ", ".join(missing))
    if not isinstance(record["instance_id"], str) or not record["instance_id"]:
        raise ValueError("instance_id must be a non-empty string, got %r" % (record["instance_id"],))
    image, base_commit = record["docker_image"], record["base_commit"]
    if not isinstance(image, str) or _BASE_IMAGE.fullmatch(image) is None:
        raise ValueError("docker_image is not an image reference: %r" % (image,))
    if not isinstance(base_commit, str) or _SHA40.fullmatch(base_commit) is None:
        raise ValueError("base_commit is not a full commit SHA: %r" % (base_commit,))
    for key in ("patch", "test_patch", "log_parser"):
        if not isinstance(record[key], str):
            raise ValueError("record[%r] must be a string" % key)
    for key, needed in (("test_cmds", True), ("print_cmds", True), ("rebuild_cmds", False)):
        cmds = record[key]
        if not isinstance(cmds, list) or not all(isinstance(cmd, str) for cmd in cmds) or (needed and not cmds):
            raise ValueError("record[%r] must be a list of strings%s, got %r"
                             % (key, " (not empty)" if needed else "", cmds))


def _write_control_evidence(evidence_dir, ev):
    """`ev` as `control-evidence.json`; a failed write is recorded in `ev`, never raised."""
    try:
        (evidence_dir / _CONTROL_EVIDENCE).write_text(
            json.dumps(ev, indent=2, allow_nan=False, default=str) + "\n", encoding="utf-8")
    except (OSError, ValueError) as exc:
        ev["evidence_write_error"] = "%s: %s" % (type(exc).__name__, exc)


def control_fresh(record, *, container, evidence_dir, memory=CONTROL_MEMORY, timeout=5400, docker=pilot.docker):
    """Derive FAIL_TO_PASS / PASS_TO_PASS of a fresh task and run the strict gate, offline, in one container.

    `record` carries `instance_id`, `docker_image`, `base_commit` (the synthetic commit), `patch`,
    `test_patch`, `test_cmds`, `rebuild_cmds`, `print_cmds` (lists of strings; `test_cmds` and
    `print_cmds` not empty) and `log_parser`; any F2P/P2P it carries is not used. `container` must start
    with `sab-fresh-ctl-`. `ValueError` for a bad argument, before any docker call: a caller bug.

    The gate image is inspected first (`docker image inspect --format {{.Id}}`; the ID is recorded as
    `image_id`), then the container (`--network none --cpus 4 --memory <memory> --memory-swap <memory>
    --pids-limit 4096 --security-opt no-new-privileges`, `memory` 8g: the frozen grader's limit) is
    started from that ID, not from the mutable reference. It walks three states, each from
    `reset_to_base`, like `multilang_control.control`: base + test patch, gold + test patch, then the
    same gold state again (`_fresh_reports`, tests only). `derive_sets(base, gold)` gives F2P and P2P
    (test names embed the binary hash, so they belong to this image);
    `grade.control_verdict(base, gold, f2p, p2p, gold_repeat=repeat)` is the verdict. An empty F2P set
    is not ok, with the reason `no fail-to-pass test`.

    The gate fails closed on a base state that shows no regression check: `base_parsed_no_tests` (the base
    log parsed no test at all, e.g. the test patch needs a symbol the gold patch adds, so every gold pass
    would look like an F2P test), `p2p_empty` (nothing left to guard against regressions) and
    `log_truncated` (F2P and P2P come from logs that were cut). `f2p_absent_at_base` counts the F2P tests
    that are absent, not failing, at base; it is evidence, not a reason by itself.

    Tolerant repeat, for the one failure the pilot saw (a parser losing a test's line to interleaved
    `--nocapture` output): when the only reason is `tests differ between two gold runs`, at most
    `TOLERATED_MISMATCHES` tests differ, the verdict agrees with that count, and EVERY differing test
    is ABSENT (no status at all) from the first repeat, the gold tests run a second time. A differing
    test is accepted only if it is `pass` in gold and in that run; all must be, else the strict
    verdict stands. A test that has a status in the first repeat (`fail`, `skip`, anything) is a real
    flake: no second run, the task is rejected, because accepting it would put noise into the arms'
    grades. Any other reason is never tolerated. When it is applied, `verdict.ok` is True with no
    reasons, `verdict.tolerated_repeat_mismatch` names the tests and `verdict.strict_repeat_mismatch_count`
    keeps the original count (`gold_repeat_mismatch.count` stays too, so `count > 0` with `ok` is
    not a contradiction); `tolerant_repeat` holds the three statuses of each differing test and
    `verdict_strict` the verdict it replaced.

    Never raises for a failure of the run. A `ControlError`, a `ParserError`, a docker failure or
    timeout becomes `verdict = {"ok": False, "error": True, "reasons": ["control error: ..."]}`; any
    other `Exception` (a bug: `KeyError`, `TypeError`, ...) becomes `{"ok": False, "error": True,
    "internal": True, "reasons": ["internal error: <Type>: <message>"]}` with the last 20 lines of the
    traceback in `internal_error_traceback`; both set `error`. `f2p` and `p2p` are None when the failure
    came before they were derived. A `BaseException` (`KeyboardInterrupt`, `SystemExit`) is re-raised,
    after the container is removed and the evidence is written (`verdict.interrupted`).

    The container is removed in a `finally`, by the ID `docker run -d` printed: a `docker run` that
    answered with a failure removes nothing (the name may belong to another run), unless its
    `--cidfile` holds an ID, and one that gave no answer removes by name (see `_Build`). The cidfile
    lives in a directory made for this call and removed after it, so runs sharing `evidence_dir` never
    read each other's ID. Nothing is removed before the run; no image is ever deleted.

    Logs go to `evidence_dir` (`base.log`, `gold.log`, `gold-repeat.log`, `gold-repeat-2.log`), byte for
    byte, and the returned evidence to `control-evidence.json` on every path (a failed write is
    recorded as `evidence_write_error`, never raised). Returns the evidence: `f2p`, `p2p`, `verdict`,
    `base`/`gold`/`gold_repeat` status counters, `steps`, `laps_seconds`, `wall_seconds`, `image_id`,
    `container_id`, `container_removed` (None when nothing was created, False with `removal_error` when
    the removal failed), `error` when the run failed.
    """
    if not isinstance(container, str) or not container.startswith(CONTROL_PREFIX) \
            or _DOCKER_NAME.fullmatch(container) is None:
        raise ValueError("container name must be a docker name starting with %r: %r" % (CONTROL_PREFIX, container))
    if not isinstance(memory, str) or _MEMORY.fullmatch(memory) is None:
        raise ValueError("memory must look like 8g, got %r" % (memory,))
    if isinstance(timeout, bool) or not isinstance(timeout, int) or timeout <= 0:
        raise ValueError("timeout must be a positive integer, got %r" % (timeout,))
    _check_record(record)
    image, base_commit = record["docker_image"], record["base_commit"]
    evidence_dir = Path(evidence_dir)
    evidence_dir.mkdir(parents=True, exist_ok=True)

    started = mark = time.monotonic()
    build = _Build(docker, container, None)
    cid_dir = None  # a directory of its own for this call's `--cidfile`, removed in the `finally`
    ev = {"id": record["instance_id"], "image": image, "container": container, "memory": memory,
          "base_commit": base_commit, "f2p": None, "p2p": None, "steps": []}
    steps = ev["steps"]
    laps = ev["laps_seconds"] = {}

    def lap(key):
        nonlocal mark
        now = time.monotonic()
        laps[key] = round(now - mark, 1)
        mark = now

    def state_run(cid, workdir, state):
        """The tests of one state: the log is written before it is parsed, so a parser failure keeps it."""
        log = mc.run_tests(cid, workdir, record, timeout, steps=steps, state=state)
        _write_log(evidence_dir, state + ".log", log)
        return grade.run_parser(record["log_parser"], log)

    try:
        proc = build.must("gate-image-id", "image", "inspect", "--format", "{{.Id}}", image, timeout=120)
        ids = _text(proc).split()
        if not ids or _IMAGE_ID.fullmatch(ids[-1]) is None:
            raise BuildError("gate image %s: docker image inspect gave no image ID: %r"
                             % (image, _text(proc)[-_LOG_TAIL:]))
        ev["image_id"] = ids[-1]
        cid_dir = _cid_dir(evidence_dir)
        cidfile = cid_dir / "run.cid"
        build.start("run", "-d", "--name", container, "--cidfile", str(cidfile),
                    "--network", "none", "--cpus", "4", "--memory", memory, "--memory-swap", memory,
                    "--pids-limit", "4096", "--security-opt", "no-new-privileges", ev["image_id"], "sleep",
                    "infinity", timeout=600, cidfile=cidfile)
        cid = build.cid
        workdir = ev["workdir"] = mc.detect_workdir(cid)
        ev["dirty_before_base"] = mc.reset_to_base(cid, workdir, base_commit)["dirty_before"]
        mc._apply(cid, workdir, record["test_patch"])
        lap("start_and_apply_test")
        mc.rebuild(cid, workdir, record, timeout, steps=steps, state="base")
        lap("base_rebuild")
        base = state_run(cid, workdir, "base")
        ev["base"] = dict(collections.Counter(base.values()))
        lap("base_tests")

        # the gold state starts from a fresh base tree, not from what the base run left behind
        ev["dirty_before_gold"] = mc.reset_to_base(cid, workdir, base_commit)["dirty_before"]
        mc._apply(cid, workdir, record["test_patch"])
        mc._apply(cid, workdir, record["patch"])
        mc.rebuild(cid, workdir, record, timeout, steps=steps, state="gold")
        lap("gold_rebuild")
        gold = state_run(cid, workdir, "gold")
        ev["gold"] = dict(collections.Counter(gold.values()))
        lap("gold_tests")

        # the same tests again, warm, in the same tree: a flaky expected test shows up as a mismatch
        mc._fresh_reports(cid, workdir)
        repeat = state_run(cid, workdir, "gold-repeat")
        ev["gold_repeat"] = dict(collections.Counter(repeat.values()))
        lap("gold_repeat_tests")

        f2p, p2p = derive_sets(base, gold)
        ev["f2p"], ev["p2p"] = f2p, p2p
        # F2P tests that are absent, not failing, at base: with a base that compiled nothing this is all of them
        ev["f2p_absent_at_base"] = sum(1 for n in f2p if n not in base)
        ev["base_status_of_f2p"] = {n: base.get(n, "absent") for n in f2p}
        verdict = dict(grade.control_verdict(base, gold, f2p, p2p, gold_repeat=repeat))
        if not f2p:  # no test to separate the arms, whatever else the verdict says
            verdict["ok"] = False
            verdict["reasons"] = [_NO_F2P] + [r for r in verdict.get("reasons", []) if r != "no fail-to-pass tests"]
        # fail closed: a base state that parsed no test (the test patch may need a symbol the gold patch adds)
        # turns every gold pass into an F2P test, and an empty P2P leaves no regression check at all
        blind = ([_BASE_NO_TESTS] if not base else []) + ([_P2P_EMPTY] if not p2p else [])
        if blind:
            verdict["ok"] = False
            verdict["reasons"] = [*verdict.get("reasons", []), *blind]
        ev["verdict"] = verdict

        mismatch = sorted(n for n in set(f2p) | set(p2p) if gold.get(n) != repeat.get(n))
        reported = (verdict.get("gold_repeat_mismatch") or {}).get("count")
        # only a test the parser LOST (no status in the first repeat) may be tolerated; one that has a
        # status there is a real flake and rejects the task without a second run
        if verdict.get("reasons") == [_REPEAT_MISMATCH] and 0 < len(mismatch) <= TOLERATED_MISMATCHES \
                and reported == len(mismatch) and all(n not in repeat for n in mismatch):
            ev["verdict_strict"] = verdict  # recorded before the run: an error in it must not lose it
            mc._fresh_reports(cid, workdir)
            repeat2 = state_run(cid, workdir, "gold-repeat-2")
            lap("gold_repeat_2_tests")
            accepted = [n for n in mismatch if gold.get(n) == grade.PASS and repeat2.get(n) == grade.PASS]
            ev["tolerant_repeat"] = {
                "tests": {n: {"gold": gold.get(n, "absent"), "gold_repeat": repeat.get(n, "absent"),
                              "gold_repeat_2": repeat2.get(n, "absent")} for n in mismatch},
                "accepted": accepted, "gold_repeat_2": dict(collections.Counter(repeat2.values()))}
            if len(accepted) == len(mismatch):
                ev["verdict"] = dict(verdict, ok=True, reasons=[], tolerated_repeat_mismatch=accepted,
                                     strict_repeat_mismatch_count=reported)
    except _STEP_ERRORS as exc:
        ev["error"] = "%s: %s" % (type(exc).__name__, str(exc)[-400:])
        ev["verdict"] = {"ok": False, "error": True, "reasons": ["control error: %s" % str(exc)[-300:]]}
    except Exception as exc:  # a bug, not a step failure: still a verdict, with the traceback tail
        ev["error"] = "%s: %s" % (type(exc).__name__, str(exc)[-400:])
        ev["internal_error_traceback"] = "\n".join(traceback.format_exc().splitlines()[-20:])
        ev["verdict"] = {"ok": False, "error": True, "internal": True,
                         "reasons": ["internal error: %s: %s" % (type(exc).__name__, str(exc)[-300:])]}
    except BaseException as exc:  # KeyboardInterrupt, SystemExit: cleaned up and recorded, then re-raised
        ev["error"] = "%s: %s" % (type(exc).__name__, str(exc)[-400:])
        ev["verdict"] = {"ok": False, "error": True, "interrupted": True,
                         "reasons": ["interrupted: %s" % type(exc).__name__]}
        raise
    finally:
        if cid_dir is not None:
            shutil.rmtree(cid_dir, ignore_errors=True)
        ev.update(build.remove())  # never raises an Exception
        if build.interrupt is not None:  # an interrupt during the removal: a passing gate is not a clean run
            ev["interrupted"] = True
        if any(step.get("truncated") for step in steps):
            ev["log_truncated"] = True  # F2P and P2P come from these logs: a cut one cannot be trusted
            verdict = ev.get("verdict")
            if isinstance(verdict, dict) and verdict.get("ok") is True:
                ev["verdict"] = dict(verdict, ok=False, reasons=[*verdict.get("reasons", []), _LOG_TRUNCATED])
        ev["wall_seconds"] = round(time.monotonic() - started, 1)
        _write_control_evidence(evidence_dir, ev)
        if build.interrupt is not None:
            raise build.interrupt
    return ev


# --- orchestration: run_build and the command line ---

STAGES = ("fetch", "cache-check", "tarball", "build", "control", "spec-verdicts", "record")
EXIT_OK, EXIT_REFUSED, EXIT_AWAITING, EXIT_UNUSABLE = 0, 2, 3, 4
_HOST_TIMEOUT = 120
_ARCHIVE_TIMEOUT = 1800
_RUN_DIR = re.compile(r"run-[A-Za-z0-9-]+")
_CONTROL_JSON_KEYS = ("id", "image", "image_id", "workdir", "steps", "base", "gold", "gold_repeat", "verdict",
                      "wall_seconds")
_CONTROL_LOGS = ("base.log", "gold.log", "gold-repeat.log", "gold-repeat-2.log")
_REQUIRED_LOGS = ("base.log", "gold.log")  # `multilang_tasks.relaxed_expected` reads them beside control.json
_SECRET_ENV = ("GITHUB_TOKEN", "GH_TOKEN")
_MIN_SECRET = 8  # a shorter value would mangle ordinary text when scrubbed


class _Refused(RuntimeError):
    """A refusal of `run_build` that is neither a fetch nor a build error (a verdict file, a record conflict)."""


def _scrub(text):
    """`text` with the value of a token in the environment replaced: no credential in a log or an evidence file."""
    for name in _SECRET_ENV:
        secret = os.environ.get(name)
        if secret and len(secret) >= _MIN_SECRET:
            text = text.replace(secret, "***")
    return text


def _stage(stage, **fields):
    """One JSON object on stderr."""
    line = json.dumps({"stage": stage, **fields}, default=str, allow_nan=False)
    print(_scrub(line), file=sys.stderr, flush=True)


def _atomic_write(path, data):
    """`data` (bytes) as the whole of `path`: a temporary file in the same directory, flushed, then renamed.

    The reader sees the old file or the new one, never half of either; a failed write leaves the old file
    and no temporary file.
    """
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, tmp = tempfile.mkstemp(dir=path.parent, prefix="." + path.name + ".", suffix=".tmp")
    try:
        with os.fdopen(fd, "wb") as f:
            f.write(data)
            f.flush()
            os.fsync(f.fileno())
        os.chmod(tmp, 0o644)
        os.replace(tmp, path)
    except BaseException:
        try:
            os.unlink(tmp)
        except OSError:
            pass
        raise


def _atomic_json(path, obj):
    _atomic_write(path, (json.dumps(obj, indent=2, allow_nan=False, default=str) + "\n").encode("utf-8"))


def _atomic_text(path, text):
    _atomic_write(path, text.encode("utf-8", errors="surrogateescape"))


def _stamp():
    return datetime.datetime.now(datetime.timezone.utc).strftime("%Y%m%dT%H%M%S%fZ")


def _new_run_dir(ev_dir):
    """A new `<ev_dir>/run-<timestamp>` directory, never one that exists: runs do not share logs or cidfiles."""
    ev_dir.mkdir(parents=True, exist_ok=True)
    for attempt in range(1000):
        run_dir = ev_dir / ("run-%s%s" % (_stamp(), "" if attempt == 0 else "-%d" % attempt))
        try:
            run_dir.mkdir()
        except FileExistsError:
            continue
        return run_dir
    raise OSError("cannot make a new run directory under %s" % ev_dir)


def _take_lock(data):
    """The path of `<data>/.lock` created by this call, or None when the file already exists.

    Created with `O_EXCL`: a second run on the same data directory finds the file and refuses. It holds
    this process's pid and start time; a failed write removes the file again, so no run is locked out by
    a lock nobody holds. A lock that a killed run left behind is not removed here; the refusal quotes it.
    """
    data.mkdir(parents=True, exist_ok=True)
    lock = data / ".lock"
    try:
        fd = os.open(lock, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o644)
    except FileExistsError:
        return None
    try:
        try:
            os.write(fd, ("pid %d, started %s\n" % (os.getpid(), _stamp())).encode("ascii"))
        finally:
            os.close(fd)
    except BaseException:
        try:
            lock.unlink()
        except OSError:
            pass
        raise
    return lock


def _lock_holder(lock):
    """What the lock file says (its pid and start time), for the refusal message."""
    try:
        text = Path(lock).read_text(encoding="utf-8", errors="replace").strip()
    except OSError:
        return "unreadable"
    return text[:200] or "empty"


def _template_list(key, value):
    """The command list of a template field: a list of strings, or a string that holds the literal of one.

    The string is read with `ast.literal_eval`: the template's command strings are data and are never
    executed here, so `['a', str(1)]` is refused. (`log_parser` is another matter: it is Python source that
    `multilang_grade.run_parser` runs in its own `python3 -I` subprocess; it is copied, not evaluated here.)
    """
    if isinstance(value, str):
        text = value.strip()
        if not text.startswith("["):
            raise ValueError("template %s is a string that is not a list literal: %r" % (key, value[:80]))
        try:
            value = ast.literal_eval(text)
        except (ValueError, SyntaxError, MemoryError, RecursionError) as exc:
            raise ValueError("template %s is not a literal list of strings: %s" % (key, exc)) from exc
    if not isinstance(value, list) or not all(isinstance(cmd, str) for cmd in value):
        raise ValueError("template %s must be a list of strings, got %r" % (key, value))
    return value


def _load_template(path, template_id):
    """`{test_cmds, rebuild_cmds, print_cmds, log_parser}` of the one record `template_id` in a JSONL file.

    `ValueError` for a file that cannot be read or parsed, an id that is absent or appears twice, a
    command field that is not a list of strings (or a string holding one literal), an empty `test_cmds`
    or `print_cmds`, or a `log_parser` that is not a non-empty string. Checked before any network use.
    """
    try:
        text = Path(path).read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError) as exc:
        raise ValueError("cannot read the template file %s: %s" % (path, exc)) from exc
    found = []
    for number, line in enumerate(text.split("\n"), 1):
        if not line.strip():
            continue
        try:
            rec = json.loads(line)
        except ValueError as exc:
            raise ValueError("template file %s line %d is not JSON: %s" % (path, number, exc)) from exc
        if isinstance(rec, dict) and rec.get("instance_id") == template_id:
            found.append(rec)
    if len(found) != 1:
        raise ValueError("template file %s has %d records with instance_id %r, need exactly one"
                         % (path, len(found), template_id))
    rec = found[0]
    out = {}
    for key in TEMPLATE_LIST_KEYS:
        if key not in rec:
            raise ValueError("template record %r has no %s" % (template_id, key))
        out[key] = _template_list(key, rec[key])
    for key in ("test_cmds", "print_cmds"):
        if not out[key]:
            raise ValueError("template %s is empty" % key)
    parser = rec.get("log_parser")
    if not isinstance(parser, str) or not parser.strip():
        raise ValueError("template log_parser must be a non-empty string")
    out["log_parser"] = parser
    return out


def _spec_entry(path, key):
    """The entry of the spec-verdicts file at `path` for `key` (None when the file has none), validated.

    The file is `{"<spec_key>": {"tests": {"<test name>": "specified" | "unspecified"}, "reader":
    "<who or what decided>", "note": "<free text>"}}`; `note` is optional. `ValueError` for a file that
    cannot be read, is not such an object, or has a malformed entry for `key`.
    """
    try:
        raw = json.loads(Path(path).read_text(encoding="utf-8"))
    except (OSError, ValueError) as exc:
        raise ValueError("cannot read the spec verdicts %s: %s" % (path, exc)) from exc
    if not isinstance(raw, dict):
        raise ValueError("spec verdicts %s must be a JSON object keyed by spec_key" % path)
    if key not in raw:
        return None
    entry = raw[key]
    if not isinstance(entry, dict):
        raise ValueError("the spec verdicts entry for %s must be an object" % key)
    if not isinstance(entry.get("tests"), dict):
        raise ValueError("the spec verdicts entry for %s needs a 'tests' object" % key)
    if not isinstance(entry.get("reader"), str) or not entry["reader"].strip():
        raise ValueError("the spec verdicts entry for %s needs a non-empty 'reader': who or what decided" % key)
    if "note" in entry and not isinstance(entry["note"], str):
        raise ValueError("the spec verdicts entry for %s: 'note' must be a string" % key)
    return entry


def _host(run, argv, env, timeout):
    """Stdout bytes of a host program; `BuildError` when it fails (`fetch_pr`'s `_call`, re-typed)."""
    try:
        return _call(run, argv, env, timeout)
    except FetchError as exc:
        raise BuildError(str(exc)) from exc


_TOOLCHAIN_USE = re.compile(r"\b(?:cargo|rustc)\s+\+([A-Za-z0-9][A-Za-z0-9._-]*)")


def _template_toolchains(template):
    """The rustup toolchains (`cargo +stable ...`) the template's commands name, sorted: evidence for the build."""
    return sorted({name for key in TEMPLATE_LIST_KEYS for cmd in template[key] for name in _TOOLCHAIN_USE.findall(cmd)})


def _template_digest(template):
    """sha256 hex of the template fields that decide the grading, JSON with sorted keys and no spaces.

    The F2P/P2P sets come from the commands and the parser the gate ran with; the record carries the
    ones of the template at the time it is written. A resume checks the two agree.
    """
    fields = {key: template[key] for key in (*TEMPLATE_LIST_KEYS, "log_parser")}
    text = json.dumps(fields, sort_keys=True, separators=(",", ":"), ensure_ascii=True, allow_nan=False)
    return hashlib.sha256(text.encode("ascii")).hexdigest()


def _verify_resume(pending, digest, run, env):
    """`_Refused` unless the pending state still describes what the gate validated.

    Refusal is the only safe outcome: a rebuild would hit "tag exists". Checked: the template digest the
    gate ran with equals the current one; `base_commit` is a full SHA and the gate's; `image` is a
    reference and the gate's; and `docker image inspect` of that tag gives the image id the gate ran on
    (a retagged or removed image is refused).
    """
    gated_digest = pending.get("template_digest")
    if gated_digest != digest:
        raise _Refused("the template differs from the one the gate ran with (commands or log_parser changed): "
                       "gated with digest %s, now %s; the pending state cannot be resumed with other grading "
                       "commands" % (gated_digest, digest))
    control_ev = pending["control"]
    base_commit, image = pending.get("base_commit"), pending.get("image")
    if not isinstance(base_commit, str) or _SHA40.fullmatch(base_commit) is None:
        raise _Refused("pending base_commit is not a full commit SHA: %r" % (base_commit,))
    if control_ev.get("base_commit") != base_commit:
        raise _Refused("pending base_commit %s is not the one the gate ran on (%r)"
                       % (base_commit, control_ev.get("base_commit")))
    if not isinstance(image, str) or _BASE_IMAGE.fullmatch(image) is None:
        raise _Refused("pending image is not an image reference: %r" % (image,))
    if control_ev.get("image") != image:
        raise _Refused("pending image %s is not the one the gate ran on (%r)" % (image, control_ev.get("image")))
    gated_id = control_ev.get("image_id")
    if not isinstance(gated_id, str) or _IMAGE_ID.fullmatch(gated_id) is None:
        raise _Refused("the gate evidence has no valid image_id: %r" % (gated_id,))
    out = _host(run, ["docker", "image", "inspect", "--format", "{{.Id}}", image], env, _HOST_TIMEOUT)
    ids = out.decode("utf-8", errors="replace").split()
    if not ids or ids[-1] != gated_id:
        raise _Refused("image %s is %s now, not %s as when the gate ran: it was retagged or replaced"
                       % (image, ids[-1] if ids else "absent", gated_id))


def _read_pending(path):
    """The saved `pending.json` as a dict, or None when it is absent or cannot be read."""
    try:
        pending = json.loads(Path(path).read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return None
    return pending if isinstance(pending, dict) else None


def _pending_changes(pending, key, skey, upstream_base, base_image_id):
    """What differs between a saved pending state and this run's inputs (empty when it is for this run)."""
    if pending.get("cache_key") == key and pending.get("spec_key") == skey:
        return []
    changed = []
    if pending.get("upstream_base") != upstream_base:
        changed.append("upstream base (was %s, now %s)" % (pending.get("upstream_base"), upstream_base))
    if pending.get("base_image_id") != base_image_id:
        changed.append("base image id (was %s, now %s)" % (pending.get("base_image_id"), base_image_id))
    if pending.get("spec_key") != skey:
        changed.append("problem statement or patches (spec key was %s, now %s)" % (pending.get("spec_key"), skey))
    return changed or ["cache key (the repository or the builder version: was %s, now %s)"
                       % (pending.get("cache_key"), key)]


def _verify_cached(control_json, run, env):
    """`_Refused` unless the image `control.json` names still has the id the gate ran on (never a silent rebuild)."""
    try:
        control_doc = json.loads(Path(control_json).read_text(encoding="utf-8"))
    except (OSError, ValueError) as exc:
        raise _Refused("cannot read %s to check the cached image: %s" % (control_json, exc)) from exc
    image = control_doc.get("image") if isinstance(control_doc, dict) else None
    gated = control_doc.get("image_id") if isinstance(control_doc, dict) else None
    if not isinstance(image, str) or _BASE_IMAGE.fullmatch(image) is None \
            or not isinstance(gated, str) or _IMAGE_ID.fullmatch(gated) is None:
        raise _Refused("%s names no image and image_id to check the cache against" % (control_json,))
    try:
        out = _host(run, ["docker", "image", "inspect", "--format", "{{.Id}}", image], env, _HOST_TIMEOUT)
    except BuildError as exc:
        raise _Refused("the cached image %s (id %s) is gone: %s; the builder does not rebuild it silently"
                       % (image, gated, exc)) from exc
    ids = out.decode("utf-8", errors="replace").split()
    if not ids or ids[-1] != gated:
        raise _Refused("the cached image %s has id %s now, not %s as when the gate ran: it was retagged or "
                       "replaced; the builder does not rebuild it silently" % (image, ids[-1] if ids else "absent",
                                                                                gated))


def _load_pending(pending, key, skey):
    """`pending` (see `_read_pending`) when it is for this cache key and spec key with a passing gate, else None.

    Whether it still describes the image and the grading commands is `_verify_resume`'s question and a
    refusal when not; a pending state of other inputs is refused earlier, by `_pending_changes`.
    """
    if not isinstance(pending, dict) or pending.get("cache_key") != key or pending.get("spec_key") != skey:
        return None
    control_ev = pending.get("control")
    if not isinstance(control_ev, dict) or not _gate_ok(control_ev):
        return None
    return pending


def _gate_ok(evidence):
    verdict = evidence.get("verdict")
    return (isinstance(verdict, dict) and verdict.get("ok") is True
            and isinstance(evidence.get("f2p"), list) and isinstance(evidence.get("p2p"), list))


def _control_doc(evidence):
    """The content of `control.json`: the nine keys the study's loaders know, the gated image id, and the repeat.

    `multilang_tasks.relaxed_expected` reads a top-level `gold_repeat_mismatch` (`{count, names}`) and
    refuses a control whose kept tests differ between the two gold runs. A task that passed the tolerant
    repeat keeps the tolerated names in `verdict.gold_repeat_mismatch`, which the loader would refuse; so the
    top-level key lists only the mismatches that still count (empty for a tolerated task) and
    `tolerated_repeat_mismatch` holds the tolerated names. The gate's own verdict is kept whole.
    `ValueError` for a verdict without a gold-repeat record.
    """
    verdict = evidence["verdict"]
    mismatch = verdict.get("gold_repeat_mismatch") if isinstance(verdict, dict) else None
    if not isinstance(mismatch, dict) or isinstance(mismatch.get("count"), bool) \
            or not isinstance(mismatch.get("count"), int) or not isinstance(mismatch.get("names", []), list):
        raise ValueError("the gate verdict has no usable gold_repeat_mismatch {count, names}: the study loaders "
                         "cannot read a control without it")
    tolerated = list(verdict.get("tolerated_repeat_mismatch") or [])
    doc = {name: evidence[name] for name in _CONTROL_JSON_KEYS}
    doc["gold_repeat_mismatch"] = {"count": max(0, mismatch["count"] - len(tolerated)),
                                   "names": [n for n in mismatch.get("names", []) if n not in tolerated]}
    doc["tolerated_repeat_mismatch"] = tolerated
    return doc


def _write_control_dir(control_dir, run_dir, evidence):
    """`control.json` and, beside it, the control run's logs (`base.log`, `gold.log` and the repeat logs that exist).

    The study's loaders read the logs from the directory of control.json. `control.json` is written last: it
    is the marker, whole or not at all. `ValueError` (nothing written) for a missing `base.log` or `gold.log`
    or a verdict the loaders cannot read.
    """
    doc = _control_doc(evidence)
    logs = {}
    for name in _CONTROL_LOGS:
        path = Path(run_dir) / name
        if path.is_file():
            logs[name] = path.read_bytes()
        elif name in _REQUIRED_LOGS:
            raise ValueError("the control run left no %s in %s" % (name, run_dir))
    for name, data in logs.items():
        _atomic_write(Path(control_dir) / name, data)
    _atomic_json(Path(control_dir) / "control.json", doc)


def _record_slot(path, record):
    """`(existing bytes, "append" | "present")` for `record` in the JSONL file `path`.

    `_Refused` when the file already holds a different record with the same `instance_id`: it is never
    replaced here. A line that is not JSON is left alone.
    """
    try:
        existing = Path(path).read_bytes()
    except FileNotFoundError:
        return b"", "append"
    for line in existing.split(b"\n"):
        try:
            old = json.loads(line)
        except ValueError:
            continue
        if isinstance(old, dict) and old.get("instance_id") == record["instance_id"]:
            if old == record:
                return existing, "present"
            raise _Refused("%s already holds a different record for %s; remove it on purpose to replace it"
                           % (path, record["instance_id"]))
    return existing, "append"


def _write_refusal(run_dir, payload):
    """`refusal-<UTC stamp>.json` in `run_dir`, a name no earlier refusal has: they are all kept."""
    for attempt in range(1000):
        path = Path(run_dir) / ("refusal-%s%s.json" % (_stamp(), "" if attempt == 0 else "-%d" % attempt))
        try:
            fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o644)
        except FileExistsError:
            continue
        with os.fdopen(fd, "w", encoding="utf-8") as f:
            f.write(json.dumps(payload, indent=2, allow_nan=False) + "\n")
        return path
    raise OSError("cannot name a refusal file in %s" % run_dir)


def _refused(run_dir, stage, error, **fields):
    message = _scrub(error)
    _stage("refused", failed_stage=stage, error=message)
    if run_dir is not None:
        try:
            _write_refusal(run_dir, {"stage": stage, "error": message})
        except OSError:
            pass
    out = {"status": "refused", "exit_code": EXIT_REFUSED, "stage": stage, "error": message,
           "evidence": None if run_dir is None else str(run_dir)}
    out.update(fields)
    return out


def _check_arguments(repo, pr, issue_numbers, base_image, template_id):
    """The study id of the task; `ValueError` for any argument a caller got wrong, before anything runs."""
    ident = _fresh_id(repo, pr)
    if not isinstance(issue_numbers, (list, tuple)) or not issue_numbers or not all(
            isinstance(n, int) and not isinstance(n, bool) and n > 0 for n in issue_numbers):
        raise ValueError("issue_numbers must be a non-empty list of positive integers, got %r" % (issue_numbers,))
    if not isinstance(base_image, str) or _BASE_IMAGE.fullmatch(base_image) is None:
        raise ValueError("base_image is not an image reference: %r" % (base_image,))
    if not isinstance(template_id, str) or not template_id:
        raise ValueError("template_id must be a non-empty string, got %r" % (template_id,))
    return ident


def run_build(*, repo, pr, issue_numbers, base_image, template_path, template_id, data,
              spec_verdicts_path=None, dry_run=False, run=subprocess.run, fetch=fetch_pr, build=build_image,
              control=control_fresh):
    """Turn one merged upstream PR into a record, an image and a `control.json`, or say why not.

    Stages, in order: `fetch` (cheap, and it yields the real upstream base); `cache-check` (the key is
    `cache_key(repo, upstream_base, base image id)`; the image id is read with `docker image inspect`
    through `run`); `tarball` (`git archive` of the upstream base, and its `^{tree}` SHA, which `build`
    must reproduce: `expected_tree`); `build`; `control` (the strict gate); `spec-verdicts`; `record`.
    Returns a dict with `status` and `exit_code`:

        dry-run                 0   the planned `stages`; nothing external is called, nothing is written
        cached                  0   the key matches and `controls/<id>/control.json` exists
        built                   0   the record is in `<data>/records.jsonl`
        refused                 2   a refusal (lock, template, fetch, build, verdicts...): `stage`, `error`
        gate-failed             2   the strict gate is not ok: `reasons`; no record
        awaiting-spec-verdicts  3   gate ok, no verdicts for this `spec_key` yet; `f2p` lists the tests
        unusable                4   the verdicts keep no F2P test; nothing is appended

    `ValueError` for an argument a caller got wrong. `FetchError`, `BuildError` and any other refusal
    are results (exit 2), with `<run dir>/refusal.json`; an unexpected exception propagates, after the
    lock is released. Layout under `data`: `.lock` (held for the run, created exclusively); `records.jsonl`
    (one line per task, appended atomically); `controls/<id>/control.json`; `evidence/<id>/` with
    `patch.diff`, `test_patch.diff`, `problem_statement.txt`, `pending.json` (the gate passed; what the
    verdict step needs, so the second run does not rebuild), `spec-verdicts-applied.json`, `cache-key.txt`
    (the last write: a key means the record is in) and one `run-<timestamp>/` per run holding the tarball
    and the logs of `build` and `control`; `upstream/<id>/src` (the clone `fetch` keeps).

    A run that finds `pending.json` for the same cache key and spec key skips tarball, build and control,
    and goes on with the verdicts, in the run directory of the run that built the image; it first checks
    that the template's grading fields (commands and `log_parser`, as a digest), the image tag (its id)
    and the base commit are the ones the gate ran with, and refuses when not. A gate that failed leaves
    `gate-failed.json`: a later run refuses, since retrying until a gate passes biases the selection;
    delete that file and the image by hand to retry on purpose. A pending state made for other inputs (the
    upstream base, the base image, the problem statement or the patches changed) is refused up front, naming
    them, before any evidence file is overwritten. A cache hit is checked against the image: `control.json`
    carries the gated `image_id`, and `docker image inspect` of the tag must give it, else the run is refused
    (a pruned or retagged image is never rebuilt silently). `controls/<id>/` also holds the control logs
    (`base.log`, `gold.log`, the repeat logs) the study's loaders read beside `control.json`, whose top-level
    `gold_repeat_mismatch` lists only the mismatches that still count (see `_control_doc`). The builder never deletes an image, so a task whose image exists but
    has no pending state is refused by `build` ("tag exists"). The template's command
    strings are data, read with `ast.literal_eval`; its `log_parser` is copied as it is (Python source that
    `multilang_grade.run_parser` later runs in its own sandboxed subprocess). No credential is printed or written; host programs run
    with `GITHUB_TOKEN` removed.
    """
    ident = _check_arguments(repo, pr, issue_numbers, base_image, template_id)
    if dry_run:
        return {"status": "dry-run", "exit_code": EXIT_OK, "stages": list(STAGES)}
    instance_id = "%s-%s" % (repo.replace("/", "__"), pr)
    data = Path(data)
    lock = _take_lock(data)
    if lock is None:
        return _refused(None, "lock", "%s exists (%s): another run holds this data directory; check that process "
                        "is gone (a killed run leaves the file), then remove it" % (data / ".lock",
                                                                                   _lock_holder(data / ".lock")))
    try:
        ev_dir = data / "evidence" / instance_id
        run_dir = _new_run_dir(ev_dir)
        return _run_locked(repo=repo, pr=pr, issue_numbers=list(issue_numbers), base_image=base_image,
                           template_path=template_path, template_id=template_id, data=data,
                           spec_verdicts_path=spec_verdicts_path, run=run, fetch=fetch, build=build,
                           control=control, ident=ident, instance_id=instance_id, ev_dir=ev_dir, run_dir=run_dir)
    finally:
        try:
            lock.unlink()
        except FileNotFoundError:
            pass


def _run_locked(*, repo, pr, issue_numbers, base_image, template_path, template_id, data, spec_verdicts_path,
                run, fetch, build, control, ident, instance_id, ev_dir, run_dir):
    stage = "template"
    try:
        template = _load_template(template_path, template_id)
        template_digest = _template_digest(template)

        stage = "fetch"
        dest = data / "upstream" / instance_id
        dest.mkdir(parents=True, exist_ok=True)
        fetched = fetch(repo, pr, issue_numbers, dest, run=run)
        upstream_base = fetched["upstream_base"]
        patch, test_patch = split_diff(fetched["diff"])
        problem_statement = fetched["problem_statement"]
        skey = spec_key(problem_statement, test_patch, patch)
        _stage("fetch", upstream_base=upstream_base, base_strategy=fetched.get("base_strategy"),
               merge_sha=fetched.get("merge_sha"), spec_key=skey)
        if _SHA.fullmatch(upstream_base) is None:
            raise BuildError("fetch gave an upstream base that is not a full commit SHA: %r" % (upstream_base,))

        stage = "cache-check"
        env = dict(os.environ)
        env.pop("GITHUB_TOKEN", None)
        env["GIT_TERMINAL_PROMPT"] = "0"
        out = _host(run, ["docker", "image", "inspect", "--format", "{{.Id}}", base_image], env, _HOST_TIMEOUT)
        ids = out.decode("utf-8", errors="replace").split()
        if not ids or _IMAGE_ID.fullmatch(ids[-1]) is None:
            raise BuildError("docker image inspect %s gave no image ID: %r" % (base_image, ids[-1:] or ""))
        base_image_id = ids[-1]
        key = cache_key(repo, upstream_base, base_image_id)
        control_json = data / "controls" / instance_id / "control.json"
        try:
            stored = (ev_dir / "cache-key.txt").read_text(encoding="utf-8").strip()
        except (OSError, UnicodeDecodeError):
            stored = None
        hit = stored == key and control_json.is_file()
        _stage("cache-check", cache_key=key, base_image_id=base_image_id, hit=hit)
        if hit:
            _verify_cached(control_json, run, env)
            return {"status": "cached", "exit_code": EXIT_OK, "id": instance_id, "cache_key": key}

        # a failed gate is not retried by deleting the image until one passes: that would bias the selection
        marker = ev_dir / "gate-failed.json"
        if marker.exists():
            stage = "gate-failed"
            raise _Refused("the gate failed in an earlier run of this task (see %s); retrying is a deliberate "
                           "act: delete that file AND the image %s by hand, then run again"
                           % (marker, image_tag(repo, pr)))

        # a pending state of other inputs belongs to an image that exists: refuse now, naming what changed,
        # before the evidence files are overwritten with this run's (a rebuild would only be "tag exists")
        saved = _read_pending(ev_dir / "pending.json")
        if saved is not None:
            changed = _pending_changes(saved, key, skey, upstream_base, base_image_id)
            if changed:
                stage = "resume"
                raise _Refused("the pending state of an earlier run (image %s exists) was made for other inputs: "
                               "%s; restore them, or build again on purpose by deleting pending.json AND the image "
                               "by hand" % (image_tag(repo, pr), "; ".join(changed)))

        # what a reader of the verdicts needs, and what a refusal below must not lose
        _atomic_text(ev_dir / "patch.diff", patch)
        _atomic_text(ev_dir / "test_patch.diff", test_patch)
        _atomic_text(ev_dir / "problem_statement.txt", problem_statement)

        pending = _load_pending(saved, key, skey)
        if pending is not None:
            stage = "resume"
            _verify_resume(pending, template_digest, run, env)
            image, base_commit, evidence = pending["image"], pending["base_commit"], pending["control"]
            # work in the run directory of the run that built it; the one made for this call is unused
            name = pending.get("run_dir")
            earlier = ev_dir / name if isinstance(name, str) and _RUN_DIR.fullmatch(name) else None
            if earlier is not None and earlier != run_dir and not earlier.is_symlink() and earlier.is_dir() \
                    and earlier.resolve().parent == ev_dir.resolve():
                try:
                    run_dir.rmdir()
                except OSError:
                    pass
                else:
                    run_dir = earlier
            _stage("resume", pending=str(ev_dir / "pending.json"), image=image, run_dir=str(run_dir))
        else:
            stage = "tarball"
            src = dest / "src"
            tar = _host(run, ["git", "-C", str(src), "archive", "--format=tar", upstream_base], env,
                        _ARCHIVE_TIMEOUT)
            if not tar:
                raise BuildError("git archive %s printed nothing" % upstream_base)
            tree_tar = run_dir / "upstream.tar"
            tree_tar.write_bytes(tar)
            out = _host(run, ["git", "-C", str(src), "rev-parse", upstream_base + "^{tree}"], env, _HOST_TIMEOUT)
            tree = out.decode("ascii", errors="replace").strip()
            if _SHA40.fullmatch(tree) is None:
                raise BuildError("git rev-parse %s^{tree} did not print a tree SHA: %r" % (upstream_base, tree[:80]))
            _stage("tarball", bytes=len(tar), expected_tree=tree, tree_tar=str(tree_tar))

            stage = "build"
            built = build(base_image=base_image, repo=repo, pr=pr, upstream_base=upstream_base,
                          tree_tar=tree_tar, evidence_dir=run_dir, expected_tree=tree,
                          toolchains=_template_toolchains(template))
            if built["base_image_id"] != base_image_id:
                raise _Refused("the base image changed during the build: %s, then %s (the image %s was built "
                               "from the second)" % (base_image_id, built["base_image_id"], built["image"]))
            image, base_commit = built["image"], built["base_commit"]
            _stage("build", image=image, base_commit=base_commit, laps_seconds=built.get("laps_seconds"),
                   compiling_lines=built.get("compiling_lines"))

            stage = "control"
            stub = {"instance_id": instance_id, "docker_image": image, "base_commit": base_commit,
                    "patch": patch, "test_patch": test_patch, "test_cmds": template["test_cmds"],
                    "rebuild_cmds": template["rebuild_cmds"], "print_cmds": template["print_cmds"],
                    "log_parser": template["log_parser"]}
            evidence = control(stub, container=CONTROL_PREFIX + ident, evidence_dir=run_dir)
            verdict = evidence.get("verdict")
            reasons = verdict.get("reasons") if isinstance(verdict, dict) else None
            reasons = reasons if isinstance(reasons, list) else []
            ok = _gate_ok(evidence)
            _stage("control", ok=ok, f2p=len(evidence.get("f2p") or []), p2p=len(evidence.get("p2p") or []),
                   f2p_absent_at_base=evidence.get("f2p_absent_at_base"), reasons=reasons)
            if not ok:
                _atomic_json(ev_dir / "gate-failed.json", {
                    "instance_id": instance_id, "cache_key": key, "verdict": verdict, "reasons": reasons,
                    "run_dir": run_dir.name, "image": image})
                return {"status": "gate-failed", "exit_code": EXIT_REFUSED, "id": instance_id,
                        "reasons": reasons, "evidence": str(run_dir)}
            _atomic_json(ev_dir / "pending.json", {
                "instance_id": instance_id, "cache_key": key, "spec_key": skey, "image": image,
                "base_commit": base_commit, "upstream_base": upstream_base, "base_image_id": base_image_id,
                "created_at": fetched["created_at"], "f2p": evidence["f2p"], "p2p": evidence["p2p"],
                "f2p_absent_at_base": evidence.get("f2p_absent_at_base"), "template_digest": template_digest, "run_dir": run_dir.name, "control": evidence})

        f2p, p2p = evidence["f2p"], evidence["p2p"]
        stage = "spec-verdicts"
        entry = None if spec_verdicts_path is None else _spec_entry(spec_verdicts_path, skey)
        if entry is None:
            _stage("spec-verdicts", awaiting=True, spec_key=skey, f2p=len(f2p))
            return {"status": "awaiting-spec-verdicts", "exit_code": EXIT_AWAITING, "id": instance_id,
                    "spec_key": skey, "f2p": f2p, "f2p_absent_at_base": evidence.get("f2p_absent_at_base"),
                    "pending": str(ev_dir / "pending.json"),
                    "evidence": str(run_dir)}
        try:
            kept, dropped = apply_spec_verdicts(f2p, entry["tests"])
        except KeyError as exc:  # an F2P test without a verdict: never a silent keep
            raise _Refused(str(exc.args[0])) from exc
        _atomic_json(ev_dir / "spec-verdicts-applied.json", {
            "spec_key": skey, "reader": entry["reader"], "note": entry.get("note"), "kept": kept,
            "dropped": dropped, "tests": {t: entry["tests"][t] for t in f2p},
            "ignored": sorted(t for t in entry["tests"] if t not in f2p)})
        _stage("spec-verdicts", kept=len(kept), dropped=len(dropped), reader=entry["reader"])
        if not kept:
            return {"status": "unusable", "exit_code": EXIT_UNUSABLE, "id": instance_id, "spec_key": skey,
                    "dropped_f2p": dropped, "evidence": str(run_dir)}

        stage = "record"
        record = build_record(repo=repo, pr=pr, issue_numbers=issue_numbers, created_at=fetched["created_at"],
                              problem_statement=problem_statement, base_commit=base_commit,
                              upstream_base_commit=upstream_base, patch=patch, test_patch=test_patch, f2p=kept,
                              p2p=p2p, dropped_f2p=dropped, template=template, docker_image=image)
        records_path = data / "records.jsonl"
        existing, action = _record_slot(records_path, record)
        # control.json, then the record, then the key: the key is the mark that all of it is in
        _write_control_dir(control_json.parent, run_dir, evidence)
        if action == "append":
            sep = b"" if not existing or existing.endswith(b"\n") else b"\n"
            line = json.dumps(record, allow_nan=False).encode("utf-8") + b"\n"
            _atomic_write(records_path, existing + sep + line)
        _atomic_text(ev_dir / "cache-key.txt", key + "\n")
        _stage("record", records=str(records_path), action=action, f2p=len(kept), dropped_f2p=len(dropped))
        return {"status": "built", "exit_code": EXIT_OK, "id": instance_id, "image": image,
                "record": str(records_path), "control": str(control_json), "f2p": kept, "dropped_f2p": dropped,
                "p2p_count": len(p2p), "resumed": pending is not None}
    except (FetchError, BuildError, ValueError, _Refused) as exc:
        return _refused(run_dir, stage, str(exc) or type(exc).__name__)


def main(argv=None):
    """The command line: `fresh_build.py build ...` (see the module docstring). Exit code: see `run_build`."""
    ap = argparse.ArgumentParser(prog="fresh_build.py", description=__doc__.split("\n")[0])
    sub = ap.add_subparsers(dest="command", required=True)
    b = sub.add_parser("build", help="build the record, image and control of one merged upstream PR")
    b.add_argument("--repo", required=True, help="owner/name")
    b.add_argument("--pr", required=True, type=int)
    b.add_argument("--issues", required=True, type=_issue_list, help="comma-separated issue numbers")
    b.add_argument("--base-image", required=True, help="a task image of the same repository")
    b.add_argument("--template-record", required=True, type=Path,
                   help="a records JSONL file; the commands and the parser of one record are reused")
    b.add_argument("--template-id", required=True, help="instance_id of that record")
    b.add_argument("--data", required=True, type=Path, help="data directory (records.jsonl, controls/, evidence/)")
    b.add_argument("--spec-verdicts", type=Path, default=None, help="spec-verdicts.json (see the docstring)")
    b.add_argument("--dry-run", action="store_true", help="print the planned stages and exit")
    args = ap.parse_args(argv)
    try:
        result = run_build(repo=args.repo, pr=args.pr, issue_numbers=args.issues, base_image=args.base_image,
                           template_path=args.template_record, template_id=args.template_id, data=args.data,
                           spec_verdicts_path=args.spec_verdicts, dry_run=args.dry_run)
    except ValueError as exc:
        _stage("refused", failed_stage="arguments", error=str(exc))
        return EXIT_REFUSED
    print(json.dumps(result, indent=2, default=str, allow_nan=False))
    return result["exit_code"]


def _issue_list(text):
    try:
        numbers = [int(part) for part in text.split(",")]
    except ValueError:
        raise argparse.ArgumentTypeError("not a comma-separated list of issue numbers: %r" % text) from None
    if not numbers or any(n <= 0 for n in numbers):
        raise argparse.ArgumentTypeError("issue numbers must be positive: %r" % text)
    return numbers


def _raise_interrupt(signum, frame):
    """SIGTERM and SIGHUP unwind like Ctrl-C, so every `finally` (container, image tag, lock) still runs."""
    raise KeyboardInterrupt("signal %d" % signum)


if __name__ == "__main__":
    signal.signal(signal.SIGTERM, _raise_interrupt)
    signal.signal(signal.SIGHUP, _raise_interrupt)
    try:
        sys.exit(main())
    except KeyboardInterrupt:
        print("interrupted", file=sys.stderr)
        sys.exit(130)
