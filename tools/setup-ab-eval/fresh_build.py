"""Pure helpers of the fresh-task builder: turn one merged upstream PR into a SWE-bench-Live-shaped record.

    is_test_path(path)                                   True for a test file by path alone
    diff_files(diff)                                     the b-side path of every file block, in order
    split_diff(diff)                                     (patch, test_patch), byte-faithful
    pick_base(parents)                                   the first parent of a merge commit
    derive_sets(base, gold)                              (FAIL_TO_PASS, PASS_TO_PASS) from two status maps
    spec_key(problem_statement, test_patch, patch)       cache key of a spec-verdict request
    apply_spec_verdicts(f2p, verdicts)                   (kept, dropped) F2P tests
    cache_key(repo, upstream_base, base_image_id, ...)   cache key of a built image
    build_record(...)                                    the task record, schema of `multilang-rust.jsonl`

Everything here is a function of its arguments. It imports the standard library only and never
touches docker, git, `gh`, the network, the filesystem or the environment, so its tests run
anywhere. A diff is treated as text with `\\n` boundaries only: it is split on the position of
each `diff --git ` line start and never through `str.splitlines`, so a CRLF blob, a lone `\\r` or a
Unicode line separator inside a hunk survives `patch + test_patch` unchanged. Stdlib only.
"""
import copy
import hashlib
import re

BUILDER_VERSION = "1"
MIN_FREE_GB = 60
CUTOFF = "2026-07-01"

TEST_PATH_RE = re.compile(r"(^|/)(tests?|testdata|fixtures)/|_tests?\.rs$|(^|/)tests?\.rs$")

SPEC_RULE = "f2p kept only if the issue states the asserted behaviour"
VERDICTS = ("specified", "unspecified")
TEMPLATE_LIST_KEYS = ("test_cmds", "rebuild_cmds", "print_cmds")

_BLOCK_START = re.compile(r"^(?=diff --git )", re.MULTILINE)
_HEADER = "diff --git "


def is_test_path(path):
    """True when `path` names a test file: a tests/test/testdata/fixtures directory, `*_test(s).rs` or `test(s).rs`."""
    return TEST_PATH_RE.search(path) is not None


def _blocks(diff):
    """The per-file blocks of `diff`, each starting at its `diff --git ` line; joined they equal `diff`."""
    if not diff:
        return []
    if not diff.startswith(_HEADER):
        raise ValueError("not a git diff: text does not start with 'diff --git '")
    return [b for b in _BLOCK_START.split(diff) if b]


def _b_path(block):
    """The b-side path of one block: its `rename to` line when present, else the header's b-side."""
    head, _, rest = block.partition("\n")
    head = head.rstrip("\r")[len(_HEADER):]
    for line in rest.split("\n"):
        if line.startswith("@@"):
            break
        if line.startswith("rename to "):
            return line[len("rename to "):].rstrip("\r")
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

    Blocks keep their order and their bytes. `ValueError` when non-empty text does not start with
    `diff --git `.
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
    return hashlib.sha256("\0".join(parts).encode("utf-8")).hexdigest()


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
