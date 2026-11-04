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

The helpers above import the standard library only and never touch docker, git, `gh`, the network,
the filesystem or the environment, so their tests run anywhere. `fetch_pr` reads the environment and
runs `gh` and `git` (with `GITHUB_TOKEN` removed) through `run=`, which a test replaces; it does not
touch docker. `build_image` runs `docker` through `docker=` (default `pilot.docker`, the one other
module this file imports), which a test replaces, and reads the exit code of every step from the
returned `CompletedProcess` and nowhere else.

A diff is treated as text with `\\n` boundaries only: it is split on the position of
each `diff --git ` line start and never through `str.splitlines`, so a CRLF blob, a lone `\\r` or a
Unicode line separator inside a hunk survives unchanged. `split_diff` regroups the blocks (source
first, then test), so `patch + test_patch != diff` for a diff that interleaves them; each block's
bytes and the order within each half are kept. A quoted path in a header raises `ValueError`: the
caller asks git for `-c core.quotePath=false`. Text is hashed with `surrogateescape`, and `fetch_pr`
decodes git output the same way, so non-UTF-8 bytes stay byte-faithful. Stdlib only.
"""
import collections
import copy
import datetime
import hashlib
import json
import os
import re
import shutil
import subprocess
import time
from pathlib import Path

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
    merge_sha = _field(pull, "merge_commit_sha", pull_path)
    if not isinstance(merge_sha, str) or not merge_sha:
        raise FetchError("PR %s#%s has no merge_commit_sha" % (repo, pr))

    commit_path = "%s/commits/%s" % (api, merge_sha)
    commit = _json(gh(commit_path), commit_path)
    parents = [_field(p, "sha", commit_path + " parent") for p in _field(commit, "parents", commit_path)]
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
_LOG_TAIL = 1500
_EVIDENCE_TAIL = 500
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
    "metadata-network": "cargo metadata (network)", "warm-build": "warm build", "git-add": "git add",
    "git-commit": "git commit", "rev-parse": "git rev-parse", "docker-commit": "docker commit",
    "inspect": "docker image inspect", "rmi": "docker rmi",
}


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


class _Build:
    """The docker calls of one image build and their record. Every exit code comes from the
    `CompletedProcess` of the docker call itself (`check=False`), never from a wrapper."""

    def __init__(self, docker, container, workdir):
        self.docker = docker
        self.container = container
        self.workdir = workdir
        self.steps = []
        self.call_raised = False  # the latest call died without an answer (timeout, no docker binary)

    def run(self, step, *args, timeout):
        """One docker call. `BuildError` when it times out or cannot start; a failing exit code is returned."""
        name = _STEP_NAMES[step]
        start = time.monotonic()
        self.call_raised = False
        problem = None
        try:
            proc = self.docker(*args, check=False, timeout=timeout)
        except subprocess.TimeoutExpired as exc:
            problem, cause = "%s timed out after %ss" % (name, timeout), exc
        except (OSError, ValueError, RuntimeError) as exc:
            problem, cause = "%s: cannot run docker: %s" % (name, exc), exc
        seconds = round(time.monotonic() - start, 1)
        if problem is not None:
            self.call_raised = True
            self.steps.append({"name": step, "returncode": None, "seconds": seconds, "error": problem})
            raise BuildError(problem) from cause
        self.steps.append({"name": step, "returncode": proc.returncode, "seconds": seconds,
                           "tail": _text(proc)[-_EVIDENCE_TAIL:]})
        return proc

    def must(self, step, *args, timeout):
        """`run`, and `BuildError` carrying the output tail unless the exit code is 0."""
        proc = self.run(step, *args, timeout=timeout)
        if proc.returncode != 0:
            raise BuildError("%s failed (exit %s): %s" % (_STEP_NAMES[step], proc.returncode,
                                                          _text(proc).rstrip()[-_LOG_TAIL:]))
        return proc

    def exec(self, step, *cmd, env=(), check=True, timeout=1800):
        """`docker exec` of `cmd` in the work directory; `env` entries are `-e` options of this exec only."""
        args = ["exec"]
        for entry in env:
            args += ["-e", entry]
        args += ["-w", self.workdir, self.container, *cmd]
        return (self.must if check else self.run)(step, *args, timeout=timeout)

    def bash(self, step, script, timeout=1800):
        return self.exec(step, "bash", "-o", "pipefail", "-c", script, timeout=timeout)

    def drop_tag(self, tag):
        """Best-effort `docker rmi <tag>` of the tag this build just made; never raises."""
        try:
            self.run("rmi", "rmi", tag, timeout=600)
        except BuildError:
            pass

    def remove(self):
        """`docker rm -f -v` of the container. Never raises: the outcome is returned for the evidence."""
        try:
            proc = self.docker("rm", "-f", "-v", self.container, check=False, timeout=600)
        except Exception as exc:  # a failed removal must not replace the error that got us here
            return {"container_removed": False, "removal_error": "%s: %s" % (type(exc).__name__, exc)}
        out = _text(proc).rstrip()
        if proc.returncode == 0 or "No such container" in out:
            return {"container_removed": True, "removal_error": None}
        return {"container_removed": False, "removal_error": "exit %s: %s" % (proc.returncode, out[-_LOG_TAIL:])}


def _write_evidence(evidence_dir, ev):
    (evidence_dir / "build-evidence.json").write_text(
        json.dumps(ev, indent=2, allow_nan=False) + "\n", encoding="utf-8")


def build_image(*, base_image, repo, pr, upstream_base, tree_tar, evidence_dir, docker=pilot.docker,
                free_gb=data_root_free_gb, workdir="/testbed"):
    """Build the study image of one PR: the upstream tree swapped into a task image, warm-built, committed.

    `base_image` is a task image whose `workdir` repo has no remote and one synthetic commit;
    `tree_tar` is the `git archive` of `upstream_base`. Steps, each exit code read from the docker
    call itself: refuse under `MIN_FREE_GB` free (`free_gb()`, before any docker call); `docker run`
    a container `sab-fresh-build-<id>` with network; delete the tracked files and the empty or
    absent directories (not `target`, not `.git`); copy in and extract the tree and touch its
    files; `cargo metadata` must succeed (offline first, then with the network); warm
    `cargo test --no-run` with `-e CARGO_NET_OFFLINE=false` on that exec only, whose log must hold
    at least one `Compiling` line (none means stale binaries); `git add -A` and a synthetic commit;
    `docker commit` to `image_tag(repo, pr)`. The container is removed in a `finally`; no tag is
    created before every check passed.

    Writes `build.log` (the warm build's output, byte for byte) and `build-evidence.json` to
    `evidence_dir`, also on failure. Returns `image`, `base_commit` (the synthetic commit's SHA),
    `image_id`, `size_bytes`, `laps_seconds`, `compiling_lines` and `container_removed` (a failed
    removal is recorded, never raised). `ValueError` for a bad argument, `BuildError` for a refused
    or failed build; both before or without leaving an image tag.
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
    tree_tar, evidence_dir = Path(tree_tar), Path(evidence_dir)
    if not tree_tar.is_file():
        raise BuildError("tree tarball %s is not a file" % tree_tar)
    free = free_gb()
    if isinstance(free, bool) or not isinstance(free, (int, float)) or not free >= MIN_FREE_GB:
        raise BuildError("%s GB free on the docker storage, at least %d GB needed" % (free, MIN_FREE_GB))
    evidence_dir.mkdir(parents=True, exist_ok=True)

    build = _Build(docker, container, workdir)
    ev = {
        "builder_version": BUILDER_VERSION, "repo": repo, "pr": pr, "upstream_base": upstream_base,
        "base_image": base_image, "image": tag, "container": container, "workdir": workdir,
        "outcome": "error", "error": None, "steps": build.steps,
    }
    laps = ev["laps_seconds"] = {}
    mark = [time.monotonic()]

    def lap(key):
        now = time.monotonic()
        laps[key] = round(now - mark[0], 1)
        mark[0] = now

    failed = False
    try:
        build.must("run", "run", "-d", "--name", container, "--cpus", "4", "--memory", "24g", base_image,
                   "sleep", "infinity", timeout=600)
        lap("run")

        build.bash("clear-tracked", "git ls-files -z | xargs -0 rm -f", timeout=600)
        build.bash("clear-dirs", _EMPTY_DIRS, timeout=600)
        lap("clear")

        build.must("copy", "cp", str(tree_tar), "%s:%s" % (container, _TREE_TAR), timeout=1800)
        build.bash("extract", "tar -xf %s -C . && rm -f %s" % (_TREE_TAR, _TREE_TAR))
        build.bash("touch", _TOUCH)
        lap("extract")

        meta = ("cargo", "metadata", "--format-version", "1")
        if build.exec("metadata", *meta, check=False, timeout=1200).returncode != 0:
            build.exec("metadata-network", *meta, env=("CARGO_NET_OFFLINE=false",), timeout=1200)
        lap("metadata")

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
        build.exec("git-add", "git", "add", "-A", timeout=3600)
        build.exec("git-commit", "git", "-c", "user.name=sab", "-c", "user.email=sab@invalid", "commit", "-q",
                   "-m", message, timeout=3600)
        base_commit = _text(build.exec("rev-parse", "git", "rev-parse", "HEAD", timeout=120)).strip()
        if _SHA.fullmatch(base_commit) is None:
            raise BuildError("git rev-parse HEAD gave no commit SHA: %r" % base_commit[-_LOG_TAIL:])
        lap("git_commit")

        try:
            build.must("docker-commit", "commit", container, tag, timeout=3600)
        except BuildError:
            if build.call_raised:  # no answer: the daemon may still have made the tag
                build.drop_tag(tag)
            raise
        try:
            proc = build.run("inspect", "image", "inspect", "--format", "{{.Id}} {{.Size}}", tag, timeout=120)
            fields = _text(proc).split()
            if proc.returncode != 0 or len(fields) != 2 or not fields[0].startswith("sha256:") \
                    or not fields[1].isdigit():
                raise BuildError("docker image inspect of %s failed (exit %s): %s"
                                 % (tag, proc.returncode, _text(proc).rstrip()[-_LOG_TAIL:]))
        except BaseException:
            build.drop_tag(tag)  # the tag exists only because of the commit just above
            raise
        lap("image_commit")

        ev.update(outcome="ok", base_commit=base_commit, image_id=fields[0], size_bytes=int(fields[1]),
                  compiling_lines=compiling)
    except BaseException as exc:
        failed = True
        ev["error"] = "%s: %s" % (type(exc).__name__, exc)
        raise
    finally:
        ev.update(build.remove())
        try:
            _write_evidence(evidence_dir, ev)
        except OSError:
            if not failed:
                raise
    return {"image": tag, "base_commit": ev["base_commit"], "image_id": ev["image_id"],
            "size_bytes": ev["size_bytes"], "laps_seconds": laps, "compiling_lines": ev["compiling_lines"],
            "container_removed": ev["container_removed"]}
