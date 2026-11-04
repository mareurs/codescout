"""The fresh-task builder: turn one merged upstream PR into a SWE-bench-Live-shaped record.

Pure helpers (functions of their arguments):

    is_test_path(path)                                   True for a test file by path alone
    diff_files(diff)                                     the b-side path of every file block, in order
    split_diff(diff)                                     (patch, test_patch), byte-faithful
    pick_base(parents)                                   the first parent of a merge commit
    derive_sets(base, gold)                              (FAIL_TO_PASS, PASS_TO_PASS) from two status maps
    spec_key(problem_statement, test_patch, patch)       cache key of a spec-verdict request
    apply_spec_verdicts(f2p, verdicts)                   (kept, dropped) F2P tests
    cache_key(repo, upstream_base, base_image_id, ...)   cache key of a built image
    build_record(...)                                    the task record, schema of `multilang-rust.jsonl`

Upstream fetch (the one part that runs programs, through an injected `run`):

    fetch_pr(repo, pr, issue_numbers, dest, ...)         merge commit, base, diff and issue text of a PR
    FetchError                                           a refused or failed fetch

The helpers above import the standard library only and never touch docker, git, `gh`, the network,
the filesystem or the environment, so their tests run anywhere. `fetch_pr` reads the environment and
runs `gh` and `git` (with `GITHUB_TOKEN` removed) through `run=`, which a test replaces; it does not
touch docker. A diff is treated as text with `\\n` boundaries only: it is split on the position of
each `diff --git ` line start and never through `str.splitlines`, so a CRLF blob, a lone `\\r` or a
Unicode line separator inside a hunk survives unchanged. `split_diff` regroups the blocks (source
first, then test), so `patch + test_patch != diff` for a diff that interleaves them; each block's
bytes and the order within each half are kept. A quoted path in a header raises `ValueError`: the
caller asks git for `-c core.quotePath=false`. Text is hashed with `surrogateescape`, and `fetch_pr`
decodes git output the same way, so non-UTF-8 bytes stay byte-faithful. Stdlib only.
"""
import copy
import hashlib
import json
import os
import re
import subprocess
from pathlib import Path

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
        "commit_url": "https://github.com/%s/tree/%s" % (repo, base_commit),
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


def _call(run, argv, env):
    """Run `argv` capturing bytes; `FetchError` with the last 300 characters of stderr on a failure."""
    try:
        proc = run(argv, capture_output=True, env=env)
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
    """`FetchError` unless `stamp` (an ISO-8601 time from the API) is on or after the `cutoff` date."""
    if not isinstance(stamp, str) or _DATE.match(stamp) is None:
        raise FetchError("%s is not an ISO-8601 time: %r" % (what, stamp))
    if stamp < cutoff:
        raise FetchError("%s %s is before the cutoff %s" % (what, stamp, cutoff))


def fetch_pr(repo, pr, issue_numbers, dest, *, run=subprocess.run, cutoff=CUTOFF):
    """Everything upstream of one merged PR: its merge commit, base, diff and issue text.

    Calls `gh api` for the PR, its merge commit, its changed files (all pages) and each issue, then
    clones `repo` into `<dest>/src` (or fetches the merge commit when `<dest>/src/.git` exists) and
    diffs the base against the merge commit. Returns `merge_sha`, `parents`, `upstream_base` (the
    FIRST parent), `created_at` (the PR's), `merged_at`, `title`, `problem_statement` (each issue's
    title, a newline and its body, issues joined by a blank line), `diff` (decoded with
    `surrogateescape`, byte-faithful), `pr_files` and `issue_created_at` (the earliest issue's).

    `FetchError` when: no issue number is given; the PR is not merged; it was merged, or an issue
    created, before `cutoff` (a date; the same day passes); a command fails (its message carries the
    last 300 characters of stderr only); a reply is not the expected JSON; the diff names a quoted
    path; or the set of files in the diff differs from the PR's changed files (the message names
    both differences). `gh` and `git` run with `GITHUB_TOKEN` removed; no argv carries a credential.
    """
    if not issue_numbers:
        raise FetchError("no issue numbers given: there would be no problem statement")
    env = dict(os.environ)
    env.pop("GITHUB_TOKEN", None)
    env["GIT_TERMINAL_PROMPT"] = "0"
    api = "repos/%s" % repo

    def gh(*args):
        return _call(run, ["gh", "api", *args], env)

    pull_path = "%s/pulls/%s" % (api, pr)
    pull = _json(gh(pull_path), pull_path)
    merged_at = _field(pull, "merged_at", pull_path)
    if merged_at is None:
        raise FetchError("PR %s#%s is not merged" % (repo, pr))
    _not_before(merged_at, cutoff, "PR %s#%s merged_at" % (repo, pr))
    merge_sha = _field(pull, "merge_commit_sha", pull_path)
    if not isinstance(merge_sha, str) or not merge_sha:
        raise FetchError("PR %s#%s has no merge_commit_sha" % (repo, pr))

    commit_path = "%s/commits/%s" % (api, merge_sha)
    commit = _json(gh(commit_path), commit_path)
    parents = [_field(p, "sha", commit_path + " parent") for p in _field(commit, "parents", commit_path)]
    try:
        base = pick_base(parents)
    except ValueError as exc:
        raise FetchError("merge commit %s: %s" % (merge_sha, exc)) from exc

    files_path = "%s/files" % pull_path
    pr_files = [_field(f, "filename", files_path) for f in _json_pages(gh(files_path, "--paginate"), files_path)]

    statements, created = [], []
    for n in issue_numbers:
        issue_path = "%s/issues/%s" % (api, n)
        issue = _json(gh(issue_path), issue_path)
        issue_created = _field(issue, "created_at", issue_path)
        _not_before(issue_created, cutoff, "issue %s#%s created_at" % (repo, n))
        statements.append("%s\n%s" % (_field(issue, "title", issue_path), _field(issue, "body", issue_path) or ""))
        created.append(issue_created)

    src = Path(dest) / "src"
    if (src / ".git").exists():
        _call(run, ["git", "-C", str(src), "fetch", "origin", merge_sha], env)
    else:
        _call(run, ["git", "clone", "--filter=blob:none", "--no-checkout",
                    "https://github.com/%s" % repo, str(src)], env)
    raw = _call(run, ["git", "-c", "core.quotePath=false", "-C", str(src), "diff", "--binary",
                      base, merge_sha], env)
    diff = raw.decode("utf-8", errors="surrogateescape")

    try:
        diff_paths = set(diff_files(diff))
    except ValueError as exc:
        raise FetchError("cannot read the diff %s..%s: %s" % (base, merge_sha, exc)) from exc
    only_diff = sorted(diff_paths - set(pr_files))
    only_pr = sorted(set(pr_files) - diff_paths)
    if only_diff or only_pr:
        raise FetchError(
            "diff %s..%s does not match the files of PR %s#%s: only in diff: %r; only in PR: %r"
            % (base, merge_sha, repo, pr, only_diff, only_pr)
        )
    return {
        "merge_sha": merge_sha,
        "parents": parents,
        "upstream_base": base,
        "created_at": _field(pull, "created_at", pull_path),
        "merged_at": merged_at,
        "title": _field(pull, "title", pull_path),
        "problem_statement": "\n\n".join(statements),
        "diff": diff,
        "pr_files": pr_files,
        "issue_created_at": min(created),
    }
