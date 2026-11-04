"""Tests for fresh_build: the pure helpers and the upstream fetch. No docker, real git, gh or network."""
import hashlib
import json
import os
import re
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import fresh_build as fb

DATASET = "/home/marius/work/claude/setup-ab-data/live/multilang-rust.jsonl"

SRC_A = (
    "diff --git a/src/a.rs b/src/a.rs\n"
    "index 111..222 100644\n"
    "--- a/src/a.rs\n"
    "+++ b/src/a.rs\n"
    "@@ -1 +1 @@\n"
    "-old\n"
    "+new\n"
)
TEST_T = (
    "diff --git a/crates/x/src/tests/simple.rs b/crates/x/src/tests/simple.rs\n"
    "index 333..444 100644\n"
    "--- a/crates/x/src/tests/simple.rs\n"
    "+++ b/crates/x/src/tests/simple.rs\n"
    "@@ -1 +1 @@\n"
    "-a\n"
    "+b\n"
)
SRC_B = (
    "diff --git a/src/b.rs b/src/b.rs\n"
    "index 555..666 100644\n"
    "--- a/src/b.rs\n"
    "+++ b/src/b.rs\n"
    "@@ -1 +1 @@\n"
    "-x\n"
    "+y\n"
)
RENAME = (
    "diff --git a/src/old name.rs b/tests/new name.rs\n"
    "similarity index 100%\n"
    "rename from src/old name.rs\n"
    "rename to tests/new name.rs\n"
)

TEMPLATE = {
    "test_cmds": ["cd /testbed/repo && cargo test"],
    "rebuild_cmds": ["cd /testbed/repo ; cargo check -q"],
    "print_cmds": ["cd /testbed/repo && cat test-output.log"],
    "log_parser": "def parser(log):\n    return {}\n",
}

RECORD_KEYS = {
    "FAIL_TO_PASS", "PASS_TO_PASS", "all_hints_text", "base_commit", "commit_url",
    "commit_urls", "created_at", "docker_image", "hints_text", "instance_id",
    "issue_numbers", "log_parser", "patch", "print_cmds", "problem_statement",
    "pull_number", "rebuild_cmds", "repo", "test_cmds", "test_patch",
}
EXTRA_KEYS = {"upstream_base_commit", "dropped_f2p", "spec_rule", "builder_version"}


class ConstantsTest(unittest.TestCase):
    def test_values(self):
        self.assertEqual(fb.BUILDER_VERSION, "1")
        self.assertEqual(fb.MIN_FREE_GB, 60)
        self.assertEqual(fb.CUTOFF, "2026-07-01")


class IsTestPathTest(unittest.TestCase):
    def test_test_paths(self):
        for p in (
            "crates/hir-ty/src/tests/simple.rs",
            "tests/x.rs",
            "src/lib_tests.rs",
            "src/lib_test.rs",
            "src/tests.rs",
            "testdata/a.json",
            "crates/parser/test_data/generated/runner.rs",
            "crates/hir-def/src/macro_expansion_tests/mod.rs",
            "crates/ide-db/src/test_data/x.txt",
            "docs/security/matrix-e2ee-guide.md",
            "rust/src/core/context_kernel/envelope_e2e.rs",
            "src/Tests/Upper.rs",
        ):
            self.assertTrue(fb.is_test_path(p), p)

    def test_source_paths(self):
        for p in ("src/lib.rs", "src/main.rs", "crates/hir-def/src/lib.rs", "docs/guide.md"):
            self.assertFalse(fb.is_test_path(p), p)


@unittest.skipUnless(os.path.exists(DATASET), "dataset file not present")
class DatasetSplitAgreementTest(unittest.TestCase):
    """is_test_path reproduces the dataset's own patch / test_patch partition, file by file."""

    HEADER = re.compile(r"^diff --git a/.* b/(.*)$", re.MULTILINE)

    def test_every_file_of_every_record(self):
        files = 0
        with open(DATASET, encoding="utf-8") as fh:
            for line in fh:
                rec = json.loads(line)
                for field, want in (("patch", False), ("test_patch", True)):
                    for path in self.HEADER.findall(rec[field]):
                        files += 1
                        self.assertEqual(
                            fb.is_test_path(path.rstrip("\r")), want,
                            "%s %s: %s" % (rec["instance_id"], field, path),
                        )
        self.assertGreater(files, 0)


class DiffFilesTest(unittest.TestCase):
    def test_order_is_diff_order(self):
        self.assertEqual(
            fb.diff_files(SRC_A + TEST_T + SRC_B),
            ["src/a.rs", "crates/x/src/tests/simple.rs", "src/b.rs"],
        )

    def test_rename_reports_b_side_even_with_spaces(self):
        self.assertEqual(fb.diff_files(RENAME), ["tests/new name.rs"])

    def test_empty(self):
        self.assertEqual(fb.diff_files(""), [])

    def test_crlf_header_does_not_leak_cr_into_path(self):
        self.assertEqual(fb.diff_files(SRC_A.replace("\n", "\r\n")), ["src/a.rs"])


class SplitDiffTest(unittest.TestCase):
    def test_three_files_one_test_two_source(self):
        patch, test_patch = fb.split_diff(SRC_A + TEST_T + SRC_B)
        self.assertEqual(patch, SRC_A + SRC_B)
        self.assertEqual(test_patch, TEST_T)

    def test_crlf_content_survives_byte_for_byte(self):
        crlf_src = SRC_A.replace("+new\n", "+new\r\n").replace("-old\n", "-old\r\n")
        crlf_test = TEST_T.replace("+b\n", "+b\r\n")
        diff = crlf_src + crlf_test
        patch, test_patch = fb.split_diff(diff)
        self.assertIn("+new\r\n", patch)
        self.assertIn("+b\r\n", test_patch)
        self.assertEqual(patch + test_patch, diff)

    def test_lone_cr_and_unicode_separators_are_not_line_breaks(self):
        src = SRC_A + "+a\rb c\x0cd\n"
        patch, test_patch = fb.split_diff(src + TEST_T)
        self.assertEqual(patch, src)
        self.assertEqual(test_patch, TEST_T)

    def test_rename_block_classified_by_b_side(self):
        patch, test_patch = fb.split_diff(SRC_A + RENAME)
        self.assertEqual(patch, SRC_A)
        self.assertEqual(test_patch, RENAME)

    def test_diff_git_inside_added_content_is_not_a_boundary(self):
        src = SRC_A + "+diff --git a/x b/tests/y.rs\n"
        patch, test_patch = fb.split_diff(src)
        self.assertEqual(patch, src)
        self.assertEqual(test_patch, "")

    def test_empty(self):
        self.assertEqual(fb.split_diff(""), ("", ""))

    def test_garbage_raises(self):
        with self.assertRaises(ValueError):
            fb.split_diff("not a diff\n" + SRC_A)
        with self.assertRaises(ValueError):
            fb.split_diff("\n" + SRC_A)


MODE_ONLY = "diff --git a/tests/run.sh b/tests/run.sh\nold mode 100644\nnew mode 100755\n"
BINARY = (
    "diff --git a/src/img.png b/src/img.png\n"
    "index 111..222 100644\n"
    "Binary files a/src/img.png and b/src/img.png differ\n"
)
COPY = (
    "diff --git a/src/a.rs b/tests/c.rs\n"
    "similarity index 100%\n"
    "copy from src/a.rs\n"
    "copy to tests/c.rs\n"
)


class BlockShapesTest(unittest.TestCase):
    def test_mode_only_block(self):
        self.assertEqual(fb.diff_files(MODE_ONLY), ["tests/run.sh"])
        self.assertEqual(fb.split_diff(SRC_A + MODE_ONLY), (SRC_A, MODE_ONLY))

    def test_binary_block(self):
        self.assertEqual(fb.diff_files(BINARY), ["src/img.png"])
        self.assertEqual(fb.split_diff(BINARY + TEST_T), (BINARY, TEST_T))

    def test_copy_block_classified_by_b_side(self):
        self.assertEqual(fb.diff_files(COPY), ["tests/c.rs"])
        self.assertEqual(fb.split_diff(SRC_A + COPY), (SRC_A, COPY))

    def test_interleaved_diff_is_regrouped(self):
        diff = TEST_T + SRC_A + MODE_ONLY + SRC_B
        patch, test_patch = fb.split_diff(diff)
        self.assertEqual(patch, SRC_A + SRC_B)
        self.assertEqual(test_patch, TEST_T + MODE_ONLY)
        self.assertNotEqual(patch + test_patch, diff)
        self.assertEqual(sorted((patch + test_patch).split("diff --git ")), sorted(diff.split("diff --git ")))


class QuotedPathTest(unittest.TestCase):
    QUOTED_HEADER = 'diff --git "a/src/t\\303\\251.rs" "b/src/t\\303\\251.rs"\n--- a/x\n'
    QUOTED_B_ONLY = 'diff --git a/src/x.rs "b/src/t\\303\\251.rs"\n'
    QUOTED_RENAME = (
        "diff --git a/src/a.rs b/src/b.rs\n"
        "similarity index 100%\n"
        "rename from src/a.rs\n"
        'rename to "src/t\\303\\251.rs"\n'
    )
    QUOTED_COPY = (
        "diff --git a/src/a.rs b/src/b.rs\n"
        "similarity index 100%\n"
        "copy from src/a.rs\n"
        'copy to "src/t\\303\\251.rs"\n'
    )
    HUNK_QUOTES = (
        "diff --git a/src/a.rs b/src/a.rs\n"
        "index 111..222 100644\n"
        "--- a/src/a.rs\n"
        "+++ b/src/a.rs\n"
        "@@ -1,4 +1,4 @@\n"
        '+rename to "x"\n'
        '-rename to "x"\n'
        ' rename to "x"\n'
        ' "b/\n'
    )

    def test_quoted_path_raises_in_both_entry_points(self):
        for diff in (self.QUOTED_HEADER, self.QUOTED_B_ONLY, self.QUOTED_RENAME, self.QUOTED_COPY):
            with self.assertRaises(ValueError, msg=diff):
                fb.diff_files(SRC_A + diff)
            with self.assertRaises(ValueError, msg=diff):
                fb.split_diff(SRC_A + diff)

    def test_quote_in_hunk_content_is_not_a_quoted_path(self):
        diff = self.HUNK_QUOTES + TEST_T
        self.assertEqual(fb.diff_files(diff), ["src/a.rs", "crates/x/src/tests/simple.rs"])
        self.assertEqual(fb.split_diff(diff), (self.HUNK_QUOTES, TEST_T))
        self.assertEqual(fb.diff_files(self.HUNK_QUOTES), ["src/a.rs"])

    def test_error_says_git_quoted_the_path_and_the_builder_refuses_it(self):
        with self.assertRaises(ValueError) as cm:
            fb.diff_files(SRC_A + self.QUOTED_HEADER)
        msg = str(cm.exception)
        self.assertIn("quoted by git", msg)
        self.assertIn("refuses", msg)
        # core.quotePath=false helps non-ASCII names only; it must not be offered as the cure.
        self.assertIn("non-ASCII", msg)
        self.assertNotIn("use -c core.quotePath=false", msg)
        with self.assertRaises(ValueError) as cm:
            fb.diff_files(SRC_A + self.QUOTED_RENAME)
        self.assertIn("quoted by git", str(cm.exception))


class PickBaseTest(unittest.TestCase):
    def test_one_parent(self):
        self.assertEqual(fb.pick_base(["aaa"]), "aaa")

    def test_two_parents_takes_first(self):
        self.assertEqual(fb.pick_base(["aaa", "bbb"]), "aaa")

    def test_empty_raises(self):
        with self.assertRaises(ValueError):
            fb.pick_base([])


class DeriveSetsTest(unittest.TestCase):
    def test_f2p_p2p(self):
        base = {"a": "pass", "b": "fail", "c": "pass", "d": "pass"}
        gold = {"a": "pass", "b": "pass", "c": "fail", "e": "pass", "f": "fail"}
        f2p, p2p = fb.derive_sets(base, gold)
        # b failed at base; e is absent at base (counted F2P).
        self.assertEqual(f2p, ["b", "e"])
        self.assertEqual(p2p, ["a"])
        # c passed at base and failed at gold: in neither. d is absent at gold. f never passes.
        for name in ("c", "d", "f"):
            self.assertNotIn(name, f2p + p2p)

    def test_sorted(self):
        gold = {"z": "pass", "m": "pass", "a": "pass"}
        f2p, p2p = fb.derive_sets({"q": "pass"}, gold)
        self.assertEqual(f2p, ["a", "m", "z"])
        self.assertEqual(p2p, [])
        f2p, p2p = fb.derive_sets(gold, gold)
        self.assertEqual((f2p, p2p), ([], ["a", "m", "z"]))


class SpecKeyTest(unittest.TestCase):
    def test_hash_is_nul_joined_sha256(self):
        want = hashlib.sha256(b"ps\0tp\0p").hexdigest()
        self.assertEqual(fb.spec_key("ps", "tp", "p"), want)

    def test_changes_with_each_input(self):
        base = fb.spec_key("ps", "tp", "p")
        self.assertNotEqual(base, fb.spec_key("ps2", "tp", "p"))
        self.assertNotEqual(base, fb.spec_key("ps", "tp2", "p"))
        self.assertNotEqual(base, fb.spec_key("ps", "tp", "p2"))

    def test_boundary_shift_changes_key(self):
        self.assertNotEqual(fb.spec_key("ab", "c", "d"), fb.spec_key("a", "bc", "d"))


class ApplySpecVerdictsTest(unittest.TestCase):
    def test_kept_and_dropped_keep_input_order(self):
        kept, dropped = fb.apply_spec_verdicts(
            ["t3", "t1", "t2"],
            {"t1": "specified", "t2": "unspecified", "t3": "specified", "extra": "specified"},
        )
        self.assertEqual(kept, ["t3", "t1"])
        self.assertEqual(dropped, ["t2"])

    def test_missing_verdict_names_every_test(self):
        with self.assertRaises(KeyError) as cm:
            fb.apply_spec_verdicts(["t_a", "t_b", "t_c"], {"t_b": "specified"})
        msg = str(cm.exception)
        self.assertIn("t_a", msg)
        self.assertIn("t_c", msg)
        self.assertNotIn("t_b", msg)

    def test_bad_value_raises(self):
        with self.assertRaises(ValueError):
            fb.apply_spec_verdicts(["a"], {"a": "maybe"})

    def test_empty_f2p(self):
        self.assertEqual(fb.apply_spec_verdicts([], {}), ([], []))


class CacheKeyTest(unittest.TestCase):
    def test_hash_and_default_version(self):
        want = hashlib.sha256(b"r\0u\0img\0" + fb.BUILDER_VERSION.encode()).hexdigest()
        self.assertEqual(fb.cache_key("r", "u", "img"), want)

    def test_changes_with_each_input(self):
        base = fb.cache_key("r", "u", "img")
        self.assertNotEqual(base, fb.cache_key("r2", "u", "img"))
        self.assertNotEqual(base, fb.cache_key("r", "u2", "img"))
        self.assertNotEqual(base, fb.cache_key("r", "u", "img2"))
        self.assertNotEqual(base, fb.cache_key("r", "u", "img", version="2"))


class SurrogateEscapeTest(unittest.TestCase):
    """Text decoded from git with errors='surrogateescape' must still hash."""

    def test_spec_key_hashes_the_original_bytes(self):
        raw = b"caf\xff"
        text = raw.decode("utf-8", errors="surrogateescape")
        self.assertEqual(text, "caf\udcff")
        self.assertEqual(fb.spec_key(text, "t", "p"), hashlib.sha256(raw + b"\0t\0p").hexdigest())

    def test_cache_key_accepts_lone_surrogate(self):
        a = fb.cache_key("r", "\udcff", "img")
        self.assertEqual(len(a), 64)
        self.assertNotEqual(a, fb.cache_key("r", "\udcfe", "img"))


class BuildRecordTest(unittest.TestCase):
    def _build(self, **over):
        kw = dict(
            repo="rust-lang/rust-analyzer", pr=23096, issue_numbers=[23000, "23001"],
            created_at="2026-07-02T10:00:00Z", problem_statement="ps",
            base_commit="b" * 40, upstream_base_commit="u" * 40, patch=SRC_A,
            test_patch=TEST_T, f2p=["t1"], p2p=["t2"], dropped_f2p=["t3"],
            template=TEMPLATE, docker_image="sab-fresh-rust-lang__rust-analyzer-23096:b1",
        )
        kw.update(over)
        return fb.build_record(**kw)

    def test_keys_and_instance_id(self):
        rec = self._build()
        self.assertEqual(set(rec), RECORD_KEYS | EXTRA_KEYS)
        self.assertEqual(rec["instance_id"], "rust-lang__rust-analyzer-23096")

    def test_values(self):
        rec = self._build()
        self.assertEqual(rec["FAIL_TO_PASS"], ["t1"])
        self.assertEqual(rec["PASS_TO_PASS"], ["t2"])
        self.assertEqual(rec["dropped_f2p"], ["t3"])
        self.assertEqual(rec["pull_number"], "23096")
        self.assertEqual(rec["issue_numbers"], ["23000", "23001"])
        self.assertEqual(rec["repo"], "rust-lang/rust-analyzer")
        self.assertEqual(rec["base_commit"], "b" * 40)
        self.assertEqual(rec["upstream_base_commit"], "u" * 40)
        self.assertEqual(rec["hints_text"], "")
        self.assertEqual(rec["all_hints_text"], "")
        self.assertEqual(rec["builder_version"], fb.BUILDER_VERSION)
        self.assertEqual(
            rec["spec_rule"], "f2p kept only if the issue states the asserted behaviour"
        )
        self.assertEqual(rec["patch"], SRC_A)
        self.assertEqual(rec["test_patch"], TEST_T)
        self.assertEqual(
            rec["commit_url"], "https://github.com/rust-lang/rust-analyzer/tree/" + "b" * 40
        )
        self.assertEqual(rec["commit_urls"], [])

    def test_template_lists_copied_not_strings_and_not_aliased(self):
        rec = self._build()
        for k in ("test_cmds", "rebuild_cmds", "print_cmds"):
            self.assertIsInstance(rec[k], list)
            self.assertEqual(rec[k], TEMPLATE[k])
            self.assertIsNot(rec[k], TEMPLATE[k])
        self.assertEqual(rec["log_parser"], TEMPLATE["log_parser"])

    def test_template_cmds_as_string_rejected(self):
        bad = dict(TEMPLATE, test_cmds="['cargo test']")
        with self.assertRaises(ValueError):
            self._build(template=bad)


REPO = "rust-lang/rust-analyzer"
PR = 23096
MERGE = "m" * 40
PARENT1 = "1" * 40
PARENT2 = "2" * 40
SECRET = "ghp_sekret_value_must_never_travel"
# Bytes as git emits them: a CRLF content line and a byte that is not UTF-8.
FETCH_DIFF = SRC_A.encode().replace(b"+new\n", b"+new\r\n+caf\xff\n") + TEST_T.encode()
FETCH_FILES = ["src/a.rs", "crates/x/src/tests/simple.rs"]


class FakeRun:
    """Stands in for subprocess.run. Answers by argv and records every call with the env it got.

    A key is the argv with the leading `gh api` or `git` stripped. A response is the stdout bytes
    (exit 0) or an `(exit_code, stdout, stderr)` tuple.
    """

    def __init__(self, responses):
        self.responses = dict(responses)
        self.calls = []  # (argv tuple, env dict)

    @staticmethod
    def key(argv):
        argv = tuple(argv)
        if argv[:2] == ("gh", "api"):
            return argv[2:]
        if argv[:1] == ("git",):
            return argv[1:]
        raise AssertionError("unexpected program: %r" % (argv,))

    def __call__(self, argv, **kw):
        # Bytes in, bytes out: no text mode, so a CRLF blob cannot be translated.
        if kw.get("capture_output") is not True:
            raise AssertionError("run() must capture bytes: %r" % kw)
        for bad in ("text", "universal_newlines", "encoding", "errors"):
            if bad in kw:
                raise AssertionError("run() must not set %s" % bad)
        self.calls.append((tuple(argv), kw.get("env")))
        key = self.key(argv)
        if key not in self.responses:
            raise AssertionError("unexpected call: %r" % (key,))
        resp = self.responses[key]
        if isinstance(resp, tuple):
            code, out, err = resp
        else:
            code, out, err = 0, resp, b""
        return subprocess.CompletedProcess(list(argv), code, out, err)

    def keys(self):
        return [self.key(argv) for argv, _ in self.calls]


def _j(obj):
    return json.dumps(obj).encode()


class FetchPrTest(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.dest = Path(tmp.name)
        self.src = self.dest / "src"

    def pull(self, **over):
        pull = {
            "merged_at": "2026-07-10T12:00:00Z", "merge_commit_sha": MERGE,
            "created_at": "2026-07-02T10:00:00Z", "title": "Fix the thing",
        }
        pull.update(over)
        return pull

    def issues(self):
        return {
            23000: {"title": "Issue one", "body": "Body one\r\nline 2", "created_at": "2026-07-05T00:00:00Z"},
            23001: {"title": "Issue two", "body": "Body two", "created_at": "2026-07-01T09:00:00Z"},
        }

    def responses(self, *, pull=None, parents=(PARENT1, PARENT2), files=None, issues=None,
                  diff=FETCH_DIFF, clone=b"", fetch=b""):
        pull = self.pull() if pull is None else pull
        files = FETCH_FILES if files is None else files
        issues = self.issues() if issues is None else issues
        api = "repos/%s" % REPO
        out = {
            ("%s/pulls/%d" % (api, PR),): _j(pull),
            ("%s/commits/%s" % (api, MERGE),): _j({"sha": MERGE, "parents": [{"sha": p} for p in parents]}),
            ("%s/pulls/%d/files" % (api, PR), "--paginate"): _j([{"filename": f} for f in files]),
            ("clone", "--filter=blob:none", "--no-checkout", "https://github.com/" + REPO, str(self.src)): clone,
            ("-C", str(self.src), "fetch", "origin", MERGE): fetch,
            ("-c", "core.quotePath=false", "-C", str(self.src), "diff", "--binary", parents[0], MERGE): diff,
        }
        for n, issue in issues.items():
            out[("%s/issues/%d" % (api, n),)] = _j(issue)
        return out

    def fetch(self, fake, issue_numbers=(23000, 23001), **kw):
        return fb.fetch_pr(REPO, PR, list(issue_numbers), self.dest, run=fake, **kw)

    def test_happy_path(self):
        fake = FakeRun(self.responses())
        with mock.patch.dict(os.environ, {"GITHUB_TOKEN": SECRET}):
            got = self.fetch(fake)
        self.assertEqual(got["merge_sha"], MERGE)
        self.assertEqual(got["parents"], [PARENT1, PARENT2])
        self.assertEqual(got["upstream_base"], PARENT1)
        self.assertEqual(got["created_at"], "2026-07-02T10:00:00Z")
        self.assertEqual(got["merged_at"], "2026-07-10T12:00:00Z")
        self.assertEqual(got["title"], "Fix the thing")
        self.assertEqual(
            got["problem_statement"],
            "Issue one\nBody one\r\nline 2\n\nIssue two\nBody two",
        )
        self.assertEqual(got["pr_files"], FETCH_FILES)
        self.assertEqual(got["issue_created_at"], "2026-07-01T09:00:00Z")
        self.assertEqual(set(got), {
            "merge_sha", "parents", "upstream_base", "created_at", "merged_at", "title",
            "problem_statement", "diff", "pr_files", "issue_created_at",
        })

    def test_diff_is_byte_faithful_and_base_is_first_parent(self):
        fake = FakeRun(self.responses())
        got = self.fetch(fake)
        self.assertEqual(got["diff"].encode("utf-8", errors="surrogateescape"), FETCH_DIFF)
        self.assertIn("+new\r\n", got["diff"])
        self.assertIn("caf\udcff", got["diff"])
        diff_argv = fake.calls[-1][0]
        self.assertEqual(diff_argv[-2:], (PARENT1, MERGE))
        self.assertNotIn(PARENT2, diff_argv)

    def test_calls_in_the_specified_order(self):
        fake = FakeRun(self.responses())
        self.fetch(fake)
        api = "repos/%s" % REPO
        self.assertEqual(fake.keys(), [
            ("%s/pulls/%d" % (api, PR),),
            ("%s/commits/%s" % (api, MERGE),),
            ("%s/pulls/%d/files" % (api, PR), "--paginate"),
            ("%s/issues/23000" % api,),
            ("%s/issues/23001" % api,),
            ("clone", "--filter=blob:none", "--no-checkout", "https://github.com/" + REPO, str(self.src)),
            ("-c", "core.quotePath=false", "-C", str(self.src), "diff", "--binary", PARENT1, MERGE),
        ])
        self.assertEqual(fake.calls[0][0][:2], ("gh", "api"))
        self.assertEqual(fake.calls[-1][0][0], "git")

    def test_existing_clone_is_fetched_not_cloned(self):
        (self.src / ".git").mkdir(parents=True)
        fake = FakeRun(self.responses())
        self.fetch(fake)
        keys = fake.keys()
        self.assertIn(("-C", str(self.src), "fetch", "origin", MERGE), keys)
        self.assertFalse([k for k in keys if k[:1] == ("clone",)])
        self.assertLess(keys.index(("-C", str(self.src), "fetch", "origin", MERGE)), len(keys) - 1)

    def test_single_parent_squash_merge(self):
        fake = FakeRun(self.responses(parents=(PARENT1,)))
        self.assertEqual(self.fetch(fake)["upstream_base"], PARENT1)

    def test_paginated_files_are_concatenated_pages(self):
        resp = self.responses()
        api = "repos/%s" % REPO
        resp[("%s/pulls/%d/files" % (api, PR), "--paginate")] = (
            _j([{"filename": "src/a.rs"}]) + b"\n" + _j([{"filename": "crates/x/src/tests/simple.rs"}])
        )
        got = self.fetch(FakeRun(resp))
        self.assertEqual(got["pr_files"], FETCH_FILES)

    def test_null_issue_body_is_empty(self):
        issues = {23000: {"title": "T", "body": None, "created_at": "2026-07-05T00:00:00Z"}}
        got = self.fetch(FakeRun(self.responses(issues=issues)), issue_numbers=[23000])
        self.assertEqual(got["problem_statement"], "T\n")

    def test_not_merged_refused_before_anything_else(self):
        fake = FakeRun(self.responses(pull=self.pull(merged_at=None, merge_commit_sha=MERGE)))
        with self.assertRaisesRegex(fb.FetchError, "not merged"):
            self.fetch(fake)
        self.assertEqual(len(fake.calls), 1)

    def test_merged_before_cutoff_refused(self):
        fake = FakeRun(self.responses(pull=self.pull(merged_at="2026-06-30T23:59:59Z")))
        with self.assertRaisesRegex(fb.FetchError, "cutoff"):
            self.fetch(fake)
        self.assertEqual(len(fake.calls), 1)

    def test_merged_on_the_cutoff_day_is_accepted(self):
        fake = FakeRun(self.responses(pull=self.pull(merged_at="2026-07-01T00:00:00Z")))
        self.assertEqual(self.fetch(fake)["merged_at"], "2026-07-01T00:00:00Z")

    def test_cutoff_parameter_is_honoured(self):
        fake = FakeRun(self.responses())
        with self.assertRaisesRegex(fb.FetchError, "cutoff"):
            self.fetch(fake, cutoff="2026-08-01")

    def test_issue_created_before_cutoff_refused_before_any_git(self):
        issues = self.issues()
        issues[23001]["created_at"] = "2026-06-30T23:59:59Z"
        fake = FakeRun(self.responses(issues=issues))
        with self.assertRaisesRegex(fb.FetchError, "23001.*cutoff"):
            self.fetch(fake)
        self.assertFalse([a for a, _ in fake.calls if a[0] == "git"])

    def test_no_issue_numbers_refused(self):
        fake = FakeRun(self.responses())
        with self.assertRaisesRegex(fb.FetchError, "issue"):
            self.fetch(fake, issue_numbers=[])
        self.assertEqual(fake.calls, [])

    def test_diff_files_differ_from_pr_files_names_the_difference(self):
        for files, only_diff, only_pr in (
            (["src/a.rs", "src/extra.rs", "crates/x/src/tests/simple.rs"], [], ["src/extra.rs"]),
            (["src/a.rs"], ["crates/x/src/tests/simple.rs"], []),
        ):
            with self.assertRaises(fb.FetchError, msg=files) as cm:
                self.fetch(FakeRun(self.responses(files=files)))
            msg = str(cm.exception)
            for path in only_diff + only_pr:
                self.assertIn(path, msg)
            self.assertIn("only in diff: %r" % only_diff, msg)
            self.assertIn("only in PR: %r" % only_pr, msg)

    def test_same_files_in_another_order_is_accepted(self):
        got = self.fetch(FakeRun(self.responses(files=list(reversed(FETCH_FILES)))))
        self.assertEqual(got["pr_files"], list(reversed(FETCH_FILES)))

    def test_quoted_path_in_the_diff_is_a_fetch_error(self):
        diff = FETCH_DIFF + b'diff --git "a/src/t\\303\\251.rs" "b/src/t\\303\\251.rs"\n'
        with self.assertRaisesRegex(fb.FetchError, "quoted by git"):
            self.fetch(FakeRun(self.responses(diff=diff)))

    def test_garbage_diff_is_a_fetch_error(self):
        with self.assertRaises(fb.FetchError):
            self.fetch(FakeRun(self.responses(diff=b"fatal-ish text\n")))

    def test_github_token_is_removed_from_every_env(self):
        fake = FakeRun(self.responses())
        with mock.patch.dict(os.environ, {"GITHUB_TOKEN": SECRET, "KEEP_ME": "1"}):
            self.fetch(fake)
            self.assertEqual(os.environ["GITHUB_TOKEN"], SECRET)
        self.assertGreater(len(fake.calls), 5)
        for argv, env in fake.calls:
            self.assertIsNotNone(env, argv)
            self.assertNotIn("GITHUB_TOKEN", env, argv)
            self.assertNotIn(SECRET, env.values(), argv)
            self.assertEqual(env.get("KEEP_ME"), "1")

    def test_no_argv_mentions_a_token_or_the_secret(self):
        fake = FakeRun(self.responses())
        with mock.patch.dict(os.environ, {"GITHUB_TOKEN": SECRET}):
            self.fetch(fake)
        for argv, _ in fake.calls:
            for part in argv:
                self.assertNotIn("token", part.lower(), argv)
                self.assertNotIn(SECRET, part, argv)

    def test_failed_command_carries_only_the_last_300_chars_of_stderr(self):
        resp = self.responses()
        api = "repos/%s" % REPO
        resp[("%s/commits/%s" % (api, MERGE),)] = (1, b"", b"x" * 500 + b"THE-TAIL")
        with self.assertRaises(fb.FetchError) as cm:
            self.fetch(FakeRun(resp))
        msg = str(cm.exception)
        self.assertIn("THE-TAIL", msg)
        self.assertIn("x" * 292 + "THE-TAIL", msg)
        self.assertNotIn("x" * 293, msg)

    def test_failed_git_diff_raises(self):
        resp = self.responses()
        resp[("-c", "core.quotePath=false", "-C", str(self.src), "diff", "--binary", PARENT1, MERGE)] = (
            128, b"", b"fatal: bad object",
        )
        with self.assertRaisesRegex(fb.FetchError, "bad object"):
            self.fetch(FakeRun(resp))

    def test_reply_that_is_not_json_raises(self):
        resp = self.responses()
        resp[("repos/%s/pulls/%d" % (REPO, PR),)] = b"<html>rate limited</html>"
        with self.assertRaises(fb.FetchError):
            self.fetch(FakeRun(resp))

    def test_missing_merge_commit_sha_raises(self):
        pull = self.pull()
        del pull["merge_commit_sha"]
        with self.assertRaisesRegex(fb.FetchError, "merge_commit_sha"):
            self.fetch(FakeRun(self.responses(pull=pull)))

    def test_missing_program_is_a_fetch_error(self):
        def run(argv, **kw):
            raise FileNotFoundError(2, "No such file or directory", argv[0])
        with self.assertRaisesRegex(fb.FetchError, "gh"):
            fb.fetch_pr(REPO, PR, [23000], self.dest, run=run)

    def test_fetch_error_is_a_runtime_error(self):
        self.assertTrue(issubclass(fb.FetchError, RuntimeError))


if __name__ == "__main__":
    unittest.main()
