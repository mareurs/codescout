"""Tests for fresh_build: the pure helpers, the upstream fetch, the build, the gate and the orchestration. No docker, real git, gh or network."""
import contextlib
import copy
import hashlib
import io
import json
import os
import re
import shutil
import subprocess
import tarfile
import tempfile
import unittest
from collections import Counter
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
        "@@ -1,3 +1,3 @@\n"
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


CL_A = (
    "diff --git a/src/a.rs b/src/a.rs\n"
    "index 1..2 100644\n"
    "--- a/src/a.rs\n"
    "+++ b/src/a.rs\n"
    "@@ -1,4 +1,5 @@\n"
    " ctx\n"
    "--- removed dashes\n"
    "+new\r\n"
    "+new\r\n"
    "+++ added plus\n"
    " more\n"
    "\\ No newline at end of file\n"
)


class ChangedLinesTest(unittest.TestCase):
    def test_counts_signed_lines_per_file(self):
        got = fb.changed_lines(CL_A + SRC_B)
        self.assertEqual(set(got), {"src/a.rs", "src/b.rs"})
        self.assertEqual(got["src/a.rs"], Counter({"--- removed dashes": 1, "+new\r": 2, "+++ added plus": 1}))
        self.assertEqual(got["src/b.rs"], Counter({"-x": 1, "+y": 1}))

    def test_file_header_lines_are_not_changes_but_content_lines_that_look_like_them_are(self):
        got = fb.changed_lines(CL_A)["src/a.rs"]
        self.assertNotIn("--- a/src/a.rs", got)
        self.assertNotIn("+++ b/src/a.rs", got)
        self.assertIn("--- removed dashes", got)
        self.assertIn("+++ added plus", got)

    def test_context_hunk_headers_and_no_newline_marker_are_ignored(self):
        got = fb.changed_lines(CL_A)["src/a.rs"]
        for line in (" ctx", " more", "@@ -1,4 +1,5 @@", "\\ No newline at end of file"):
            self.assertNotIn(line, got)

    def test_cr_is_part_of_the_key(self):
        self.assertIn("+new\r", fb.changed_lines(CL_A)["src/a.rs"])
        self.assertNotIn("+new", fb.changed_lines(CL_A)["src/a.rs"])

    def test_blocks_without_hunks_are_present_with_no_lines(self):
        got = fb.changed_lines(MODE_ONLY + BINARY + RENAME)
        self.assertEqual(got, {"tests/run.sh": Counter(), "src/img.png": Counter(), "tests/new name.rs": Counter()})

    def test_empty_diff(self):
        self.assertEqual(fb.changed_lines(""), {})

    def test_hunk_content_never_starts_a_new_block(self):
        got = fb.changed_lines(SRC_A + "+diff --git a/x b/tests/y.rs\n")
        self.assertEqual(set(got), {"src/a.rs"})
        self.assertEqual(got["src/a.rs"]["+diff --git a/x b/tests/y.rs"], 1)

    def test_garbage_and_quoted_paths_raise(self):
        with self.assertRaises(ValueError):
            fb.changed_lines("junk\n")
        with self.assertRaises(ValueError):
            fb.changed_lines(SRC_A + QuotedPathTest.QUOTED_HEADER)


class SameChangeTest(unittest.TestCase):
    def test_context_hunk_headers_and_file_order_are_ignored(self):
        other = (
            SRC_B.replace("@@ -1 +1 @@", "@@ -10,3 +10,3 @@").replace("-x\n", " around\n-x\n")
            + SRC_A.replace("index 111..222", "index 999..888")
        )
        self.assertTrue(fb.same_change(SRC_A + SRC_B, other))

    def test_identical(self):
        self.assertTrue(fb.same_change(SRC_A + TEST_T, SRC_A + TEST_T))
        self.assertTrue(fb.same_change("", ""))

    def test_partial_patch_with_the_same_files_is_not_the_same_change(self):
        self.assertNotEqual(PARTIAL_DIFF, FETCH_DIFF)
        full = FETCH_DIFF.decode("utf-8", errors="surrogateescape")
        partial = PARTIAL_DIFF.decode("utf-8", errors="surrogateescape")
        self.assertEqual(fb.diff_files(full), fb.diff_files(partial))
        self.assertFalse(fb.same_change(full, partial))
        self.assertFalse(fb.same_change(partial, full))

    def test_fewer_lines_in_one_file(self):
        self.assertFalse(fb.same_change(SRC_A, SRC_A.replace("-old\n", "")))

    def test_multiplicity_sign_and_cr_matter(self):
        self.assertFalse(fb.same_change(SRC_A, SRC_A + "+new\n"))
        self.assertFalse(fb.same_change(SRC_A, SRC_A.replace("+new\n", "-new\n")))
        self.assertFalse(fb.same_change(SRC_A, SRC_A.replace("+new\n", "+new\r\n")))

    def test_same_lines_in_another_file_or_a_different_file_set(self):
        self.assertFalse(fb.same_change(SRC_A, SRC_A.replace("src/a.rs", "src/z.rs")))
        self.assertFalse(fb.same_change(SRC_A, SRC_A + SRC_B))

    def test_quoted_path_raises(self):
        with self.assertRaises(ValueError):
            fb.same_change(SRC_A, SRC_A + QuotedPathTest.QUOTED_HEADER)


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
BASE_N = "b" * 40
ACCEPT_DIFF = "Accept: application/vnd.github.v3.diff"
# Same files as FETCH_DIFF, other changed lines: what a rebase merge's last commit alone would give.
PARTIAL_DIFF = FETCH_DIFF.replace(b"+b\n", b"+c\n")
# A rename as GitHub and `git diff --find-renames` show it, and as `git diff --no-renames` shows it.
RENAME_PR_DIFF = (
    b"diff --git a/src/old.rs b/src/new.rs\nsimilarity index 66%\nrename from src/old.rs\n"
    b"rename to src/new.rs\nindex 111..222 100644\n--- a/src/old.rs\n+++ b/src/new.rs\n"
    b"@@ -1,2 +1,2 @@\n-x\n+y\n z\n"
)
RENAME_GIT_DIFF = (
    b"diff --git a/src/old.rs b/src/old.rs\ndeleted file mode 100644\nindex 111..000\n"
    b"--- a/src/old.rs\n+++ /dev/null\n@@ -1,2 +0,0 @@\n-x\n-z\n"
    b"diff --git a/src/new.rs b/src/new.rs\nnew file mode 100644\nindex 000..222\n"
    b"--- /dev/null\n+++ b/src/new.rs\n@@ -0,0 +1,2 @@\n+y\n+z\n"
)
PAIR_DIFF = (
    b"diff --git a/src/gone.rs b/src/gone.rs\ndeleted file mode 100644\nindex 111..000\n"
    b"--- a/src/gone.rs\n+++ /dev/null\n@@ -1 +0,0 @@\n-g\n"
    b"diff --git a/src/added.rs b/src/added.rs\nnew file mode 100644\nindex 000..222\n"
    b"--- /dev/null\n+++ b/src/added.rs\n@@ -0,0 +1 @@\n+a\n"
)


class FakeRun:
    """Stands in for subprocess.run. Answers by argv and records every call with the env it got.

    A key is the argv with the leading `gh api` or `git` stripped. A response is the stdout bytes
    (exit 0), an `(exit_code, stdout, stderr)` tuple, or an exception to raise.
    """

    def __init__(self, responses):
        self.responses = dict(responses)
        self.calls = []  # (argv tuple, env dict)
        self.timeouts = []

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
        self.timeouts.append(kw.get("timeout"))
        key = self.key(argv)
        if key not in self.responses:
            raise AssertionError("unexpected call: %r" % (key,))
        resp = self.responses[key]
        if isinstance(resp, BaseException):
            raise resp
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
        self.api = "repos/%s" % REPO
        self.pull_path = "%s/pulls/%d" % (self.api, PR)

    def pull(self, **over):
        pull = {
            "merged_at": "2026-07-10T12:00:00Z", "merge_commit_sha": MERGE, "commits": 1,
            "created_at": "2026-07-02T10:00:00Z", "title": "Fix the thing",
        }
        pull.update(over)
        return pull

    def issues(self):
        return {
            23000: {"title": "Issue one", "body": "Body one\r\nline 2", "created_at": "2026-07-05T00:00:00Z"},
            23001: {"title": "Issue two", "body": "Body two", "created_at": "2026-07-01T09:00:00Z"},
        }

    def diff_key(self, base, renames=False):
        return ("-c", "core.quotePath=false", "-c", "diff.suppressBlankEmpty=false", "-C", str(self.src), "diff",
                "--no-textconv", "--no-ext-diff", "--no-color", "--diff-algorithm=myers", "--submodule=short",
                "--find-renames" if renames else "--no-renames", "--src-prefix=a/",
                "--dst-prefix=b/", "-U3", "--binary", base, MERGE)

    def rev_key(self, n):
        return ("-C", str(self.src), "rev-parse", "%s~%d" % (MERGE, n))

    def responses(self, *, pull=None, parents=(PARENT1, PARENT2), files=None, issues=None,
                  diff=FETCH_DIFF, pr_diff=None, extra=None, clone=b"", fetch=b""):
        pull = self.pull() if pull is None else pull
        files = FETCH_FILES if files is None else files
        issues = self.issues() if issues is None else issues
        out = {
            (self.pull_path,): _j(pull),
            ("%s/commits/%s" % (self.api, MERGE),): _j({"sha": MERGE, "parents": [{"sha": p} for p in parents]}),
            ("%s/files" % self.pull_path, "--paginate"): _j(
                [f if isinstance(f, dict) else {"filename": f} for f in files]),
            (self.pull_path, "-H", ACCEPT_DIFF): FETCH_DIFF if pr_diff is None else pr_diff,
            ("clone", "--filter=blob:none", "--no-checkout", "https://github.com/" + REPO, str(self.src)): clone,
            ("-C", str(self.src), "fetch", "origin", MERGE): fetch,
        }
        if parents:
            out[self.diff_key(parents[0])] = diff
        for n, issue in issues.items():
            out[("%s/issues/%d" % (self.api, n),)] = _j(issue)
        out.update(extra or {})
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
        self.assertEqual(got["base_strategy"], "first-parent")
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
            "merge_sha", "parents", "upstream_base", "base_strategy", "created_at", "merged_at", "title",
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

    def test_every_git_diff_is_hardened_against_config(self):
        resp = self.responses(
            pull=self.pull(commits=3), parents=(PARENT1,), diff=PARTIAL_DIFF,
            extra={self.rev_key(3): BASE_N.encode(), self.diff_key(BASE_N): FETCH_DIFF},
        )
        fake = FakeRun(resp)
        self.fetch(fake)
        diffs = [a for a, _ in fake.calls if "diff" in a and a[0] == "git"]
        self.assertEqual(len(diffs), 2)
        for argv in diffs:
            for flag in ("core.quotePath=false", "diff.suppressBlankEmpty=false", "--no-textconv", "--no-ext-diff",
                         "--no-color", "--diff-algorithm=myers", "--submodule=short", "--no-renames",
                         "--src-prefix=a/", "--dst-prefix=b/", "-U3", "--binary"):
                self.assertIn(flag, argv)
            self.assertNotIn("--find-renames", argv)
            # each -c takes effect only before the subcommand
            self.assertLess(argv.index("diff.suppressBlankEmpty=false"), argv.index("diff"))

    def test_calls_in_the_specified_order(self):
        fake = FakeRun(self.responses())
        self.fetch(fake)
        self.assertEqual(fake.keys(), [
            (self.pull_path,),
            ("%s/commits/%s" % (self.api, MERGE),),
            ("%s/files" % self.pull_path, "--paginate"),
            ("%s/issues/23000" % self.api,),
            ("%s/issues/23001" % self.api,),
            (self.pull_path, "-H", ACCEPT_DIFF),
            ("clone", "--filter=blob:none", "--no-checkout", "https://github.com/" + REPO, str(self.src)),
            self.diff_key(PARENT1),
        ])
        self.assertEqual(
            [argv[:2] if argv[0] == "gh" else argv[:1] for argv, _ in fake.calls],
            [("gh", "api")] * 6 + [("git",)] * 2,
        )

    def test_existing_clone_is_fetched_not_cloned(self):
        (self.src / ".git").mkdir(parents=True)
        fake = FakeRun(self.responses())
        self.fetch(fake)
        keys = fake.keys()
        fetch_key = ("-C", str(self.src), "fetch", "origin", MERGE)
        self.assertIn(fetch_key, keys)
        self.assertFalse([k for k in keys if k[:1] == ("clone",)])
        self.assertEqual(keys.index(fetch_key), len(keys) - 2)

    def test_failing_fetch_of_an_existing_clone_raises(self):
        (self.src / ".git").mkdir(parents=True)
        resp = self.responses(fetch=(128, b"", b"fatal: could not read from remote"))
        fake = FakeRun(resp)
        with self.assertRaisesRegex(fb.FetchError, "could not read from remote"):
            self.fetch(fake)
        self.assertNotIn(self.diff_key(PARENT1), fake.keys())

    def test_failing_clone_raises(self):
        resp = self.responses(clone=(128, b"", b"fatal: repository not found"))
        with self.assertRaisesRegex(fb.FetchError, "repository not found"):
            self.fetch(FakeRun(resp))

    def test_single_parent_squash_merge(self):
        fake = FakeRun(self.responses(parents=(PARENT1,)))
        got = self.fetch(fake)
        self.assertEqual(got["upstream_base"], PARENT1)
        self.assertEqual(got["base_strategy"], "first-parent")

    def test_paginated_files_are_concatenated_pages(self):
        a, b = b'[{"filename": "src/a.rs"}]', b'[{"filename": "crates/x/src/tests/simple.rs"}]'
        key = ("%s/files" % self.pull_path, "--paginate")
        for name, body in (
            ("newline", a + b"\n" + b),
            ("back to back", a + b),
            ("empty page between", a + b"\n[]\n" + b),
            ("empty page last", a + b + b"[]"),
            ("pretty printed", a.replace(b"{", b"{\n  ") + b"\n" + b),
        ):
            resp = self.responses()
            resp[key] = body
            got = self.fetch(FakeRun(resp))
            self.assertEqual(got["pr_files"], FETCH_FILES, name)

    def test_paginated_page_that_is_not_an_array_raises(self):
        resp = self.responses()
        resp[("%s/files" % self.pull_path, "--paginate")] = b'{"message": "Not Found"}'
        with self.assertRaisesRegex(fb.FetchError, "array"):
            self.fetch(FakeRun(resp))

    def test_null_issue_body_is_empty(self):
        issues = {23000: {"title": "T", "body": None, "created_at": "2026-07-05T00:00:00Z"}}
        got = self.fetch(FakeRun(self.responses(issues=issues)), issue_numbers=[23000])
        self.assertEqual(got["problem_statement"], "T\n")

    def test_issue_without_title_raises(self):
        issues = {23000: {"body": "b", "created_at": "2026-07-05T00:00:00Z"}}
        with self.assertRaisesRegex(fb.FetchError, "title"):
            self.fetch(FakeRun(self.responses(issues=issues)), issue_numbers=[23000])

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

    def test_timestamp_must_be_the_full_utc_form(self):
        # the last one is Arabic-Indic digits: str.isdigit and \d accept them, an ASCII-only timestamp does not
        for bad in ("2026-07-10", "2026-07-10T12:00:00Zjunk", "2026-07-10T12:00:00", "2026-07-10 12:00:00Z",
                    "2026-07-10T12:00:00+00:00", "junk2026-07-10T12:00:00Z", "", 20260710,
                    "٢٠٢٦-07-10T12:00:00Z"):
            with self.assertRaisesRegex(fb.FetchError, "ISO-8601", msg=repr(bad)):
                self.fetch(FakeRun(self.responses(pull=self.pull(merged_at=bad))))
        issues = self.issues()
        issues[23000]["created_at"] = "2026-07-05"
        with self.assertRaisesRegex(fb.FetchError, "ISO-8601"):
            self.fetch(FakeRun(self.responses(issues=issues)))

    def test_cutoff_must_be_a_date(self):
        for bad in ("2026-7-1", "2026-07-01T00:00:00Z", "20260701", "2026-13-01", "2026-02-30", "", None, 20260701):
            fake = FakeRun(self.responses())
            with self.assertRaises(ValueError, msg=repr(bad)):
                self.fetch(fake, cutoff=bad)
            self.assertEqual(fake.calls, [])

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

    def test_same_files_but_other_changed_lines_is_refused_naming_the_file(self):
        # The file-set guard alone passes this: it is a partial gold patch.
        with self.assertRaises(fb.FetchError) as cm:
            self.fetch(FakeRun(self.responses(diff=PARTIAL_DIFF)))
        msg = str(cm.exception)
        self.assertIn("crates/x/src/tests/simple.rs", msg)
        self.assertNotIn("'src/a.rs'", msg)
        self.assertIn(PARENT1, msg)

    def test_rebase_merge_falls_back_to_merge_minus_commits(self):
        resp = self.responses(
            pull=self.pull(commits=3), parents=(PARENT1,), diff=PARTIAL_DIFF,
            extra={self.rev_key(3): BASE_N.encode() + b"\n", self.diff_key(BASE_N): FETCH_DIFF},
        )
        fake = FakeRun(resp)
        got = self.fetch(fake)
        self.assertEqual(got["base_strategy"], "rebase-n")
        self.assertEqual(got["upstream_base"], BASE_N)
        self.assertEqual(got["parents"], [PARENT1])
        self.assertEqual(got["diff"].encode("utf-8", errors="surrogateescape"), FETCH_DIFF)
        self.assertEqual(fake.keys()[-3:], [self.diff_key(PARENT1), self.rev_key(3), self.diff_key(BASE_N)])

    def test_rebase_merge_whose_last_commit_touches_fewer_files(self):
        resp = self.responses(
            pull=self.pull(commits=2), parents=(PARENT1,), diff=SRC_A.encode(),
            extra={self.rev_key(2): BASE_N.encode(), self.diff_key(BASE_N): FETCH_DIFF},
        )
        got = self.fetch(FakeRun(resp))
        self.assertEqual((got["base_strategy"], got["upstream_base"]), ("rebase-n", BASE_N))

    def test_first_candidate_that_matches_wins_without_rev_parse(self):
        fake = FakeRun(self.responses(pull=self.pull(commits=5), parents=(PARENT1,)))
        got = self.fetch(fake)
        self.assertEqual((got["base_strategy"], got["upstream_base"]), ("first-parent", PARENT1))
        self.assertFalse([k for k in fake.keys() if "rev-parse" in k])

    def test_no_candidate_matches_names_both_and_the_differing_files(self):
        resp = self.responses(
            pull=self.pull(commits=3), parents=(PARENT1,), diff=PARTIAL_DIFF,
            extra={self.rev_key(3): BASE_N.encode(), self.diff_key(BASE_N): PARTIAL_DIFF},
        )
        with self.assertRaises(fb.FetchError) as cm:
            self.fetch(FakeRun(resp))
        msg = str(cm.exception)
        for part in (PARENT1, BASE_N, "first-parent", "rebase-n", "crates/x/src/tests/simple.rs"):
            self.assertIn(part, msg)
        self.assertNotIn("'src/a.rs'", msg)

    def test_merge_commit_with_two_parents_never_tries_rebase_n(self):
        fake = FakeRun(self.responses(pull=self.pull(commits=3), diff=PARTIAL_DIFF))
        with self.assertRaises(fb.FetchError) as cm:
            self.fetch(fake)
        self.assertFalse([k for k in fake.keys() if "rev-parse" in k])
        self.assertNotIn("rebase-n", str(cm.exception))

    def test_single_commit_pr_never_tries_rebase_n(self):
        fake = FakeRun(self.responses(pull=self.pull(commits=1), parents=(PARENT1,), diff=PARTIAL_DIFF))
        with self.assertRaises(fb.FetchError):
            self.fetch(fake)
        self.assertFalse([k for k in fake.keys() if "rev-parse" in k])

    def test_rebase_candidate_needs_the_commits_field(self):
        pull = self.pull()
        del pull["commits"]
        with self.assertRaisesRegex(fb.FetchError, "commits"):
            self.fetch(FakeRun(self.responses(pull=pull, parents=(PARENT1,), diff=PARTIAL_DIFF)))

    def test_rev_parse_that_is_not_a_sha_raises(self):
        resp = self.responses(pull=self.pull(commits=3), parents=(PARENT1,), diff=PARTIAL_DIFF,
                              extra={self.rev_key(3): b"not a sha\n"})
        with self.assertRaisesRegex(fb.FetchError, "rev-parse"):
            self.fetch(FakeRun(resp))

    def test_failing_rev_parse_raises(self):
        resp = self.responses(pull=self.pull(commits=3), parents=(PARENT1,), diff=PARTIAL_DIFF,
                              extra={self.rev_key(3): (128, b"", b"fatal: ambiguous argument")})
        with self.assertRaisesRegex(fb.FetchError, "ambiguous argument"):
            self.fetch(FakeRun(resp))

    def test_rename_is_checked_against_the_union_of_old_and_new_path(self):
        files = [{"filename": "src/new.rs", "previous_filename": "src/old.rs", "status": "renamed"}]
        resp = self.responses(files=files, diff=RENAME_GIT_DIFF, pr_diff=RENAME_PR_DIFF,
                              extra={self.diff_key(PARENT1, renames=True): RENAME_PR_DIFF})
        fake = FakeRun(resp)
        got = self.fetch(fake)
        self.assertEqual(got["pr_files"], ["src/new.rs"])
        self.assertEqual(got["diff"].encode("utf-8", errors="surrogateescape"), RENAME_GIT_DIFF)
        self.assertEqual(fake.keys()[-2:], [self.diff_key(PARENT1), self.diff_key(PARENT1, renames=True)])

    def test_rename_whose_content_changed_is_refused(self):
        files = [{"filename": "src/new.rs", "previous_filename": "src/old.rs"}]
        other = RENAME_PR_DIFF.replace(b"+y\n", b"+Y\n")
        resp = self.responses(files=files, diff=RENAME_GIT_DIFF, pr_diff=RENAME_PR_DIFF,
                              extra={self.diff_key(PARENT1, renames=True): other})
        with self.assertRaisesRegex(fb.FetchError, "src/new.rs"):
            self.fetch(FakeRun(resp))

    def test_rename_not_reported_by_github_is_refused(self):
        resp = self.responses(files=["src/new.rs"], diff=RENAME_GIT_DIFF, pr_diff=RENAME_PR_DIFF)
        with self.assertRaises(fb.FetchError) as cm:
            self.fetch(FakeRun(resp))
        self.assertIn("only in diff: %r" % ["src/old.rs"], str(cm.exception))

    def test_delete_plus_add_pair_is_accepted_without_a_rename_diff(self):
        files = [{"filename": "src/gone.rs", "status": "removed"}, {"filename": "src/added.rs", "status": "added"}]
        fake = FakeRun(self.responses(files=files, diff=PAIR_DIFF, pr_diff=PAIR_DIFF))
        got = self.fetch(fake)
        self.assertEqual(got["pr_files"], ["src/gone.rs", "src/added.rs"])
        self.assertEqual([k for k in fake.keys() if "--find-renames" in k], [])

    def test_delete_without_its_pr_entry_is_refused(self):
        with self.assertRaises(fb.FetchError) as cm:
            self.fetch(FakeRun(self.responses(files=["src/added.rs"], diff=PAIR_DIFF, pr_diff=PAIR_DIFF)))
        self.assertIn("src/gone.rs", str(cm.exception))

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
        resp[("%s/commits/%s" % (self.api, MERGE),)] = (1, b"", b"x" * 500 + b"THE-TAIL")
        with self.assertRaises(fb.FetchError) as cm:
            self.fetch(FakeRun(resp))
        msg = str(cm.exception)
        self.assertIn("THE-TAIL", msg)
        self.assertIn("x" * 292 + "THE-TAIL", msg)
        self.assertNotIn("x" * 293, msg)

    def test_failed_git_diff_raises(self):
        resp = self.responses(diff=(128, b"", b"fatal: bad object"))
        with self.assertRaisesRegex(fb.FetchError, "bad object"):
            self.fetch(FakeRun(resp))

    def test_reply_that_is_not_json_raises(self):
        resp = self.responses()
        resp[(self.pull_path,)] = b"<html>rate limited</html>"
        with self.assertRaises(fb.FetchError):
            self.fetch(FakeRun(resp))

    def test_missing_merge_commit_sha_raises(self):
        pull = self.pull()
        del pull["merge_commit_sha"]
        with self.assertRaisesRegex(fb.FetchError, "merge_commit_sha"):
            self.fetch(FakeRun(self.responses(pull=pull)))

    def test_zero_parents_is_a_fetch_error(self):
        with self.assertRaisesRegex(fb.FetchError, "no parents"):
            self.fetch(FakeRun(self.responses(parents=())))

    def test_missing_program_is_a_fetch_error(self):
        def run(argv, **kw):
            raise FileNotFoundError(2, "No such file or directory", argv[0])
        with self.assertRaisesRegex(fb.FetchError, "gh"):
            fb.fetch_pr(REPO, PR, [23000], self.dest, run=run)

    def test_every_call_gets_the_timeout_default_and_override(self):
        fake = FakeRun(self.responses())
        self.fetch(fake)
        self.assertEqual(set(fake.timeouts), {600})
        self.assertEqual(len(fake.timeouts), len(fake.calls))
        fake = FakeRun(self.responses())
        self.fetch(fake, timeout=7)
        self.assertEqual(set(fake.timeouts), {7})

    def test_timeout_is_a_fetch_error_naming_the_argv_but_not_the_env(self):
        argv_key = (self.pull_path,)
        expired = subprocess.TimeoutExpired(cmd=["gh", "api", self.pull_path], timeout=600,
                                            output=b"partial", stderr=b"partial err")
        resp = self.responses()
        resp[argv_key] = expired
        with mock.patch.dict(os.environ, {"GITHUB_TOKEN": SECRET}):
            with self.assertRaises(fb.FetchError) as cm:
                self.fetch(FakeRun(resp))
        msg = str(cm.exception)
        self.assertIn("timed out", msg)
        self.assertIn("gh api " + self.pull_path, msg)
        self.assertNotIn(SECRET, msg)

    def test_timeout_of_a_git_command_is_a_fetch_error(self):
        resp = self.responses()
        resp[("clone", "--filter=blob:none", "--no-checkout", "https://github.com/" + REPO, str(self.src))] = (
            subprocess.TimeoutExpired(cmd=["git", "clone"], timeout=1)
        )
        with self.assertRaisesRegex(fb.FetchError, "timed out"):
            self.fetch(FakeRun(resp))

    def test_fetch_error_is_a_runtime_error(self):
        self.assertTrue(issubclass(fb.FetchError, RuntimeError))


BASE_IMAGE = "swebench/sweb.eval.x86_64.rust-lang_1776_rust-analyzer-23000:latest"
UPSTREAM_BASE = "3d5fe04754f1" + "0" * 28
SYN_SHA = "05278f431f7a71add03f39f081af38f03ac64baf"
CONTAINER = "sab-fresh-build-rust-lang__rust-analyzer-23096"
TAG = "sab-fresh-rust-lang__rust-analyzer-23096:b%s" % fb.BUILDER_VERSION
IMAGE_ID = "sha256:" + "a" * 64
BASE_ID = "sha256:" + "b" * 64
CID = "ab12" * 16  # the container ID `docker run -d` prints
TREE_SHA = "1f" * 20  # the upstream tree SHA a caller passes as `expected_tree`
OTHER_TREE = "2e" * 20
TREE = {  # the upstream tree: path -> bytes, or ("symlink", target)
    "Cargo.toml": b"[workspace]\nmembers = [\"crates/*\"]\n",
    "crates/keep/Cargo.toml": b"[package]\nname = \"keep\"\n",
    "crates/keep/src/lib.rs": b"pub fn keep() {}\n",
    ".gitignore": b"/target\n.env.*\n",
    ".env.example": b"A=1\n",  # tracked, and matched by the .gitignore above
    "scripts/run.sh": ("exec", b"#!/bin/sh\necho run\n"),  # mode 100755
    "link": ("symlink", "Cargo.toml"),
}
TREE_FILES = sorted(TREE)
WARM_OK = (
    "   Compiling stdx v0.0.0 (/testbed/crates/stdx)\n"
    "   Compiling parser v0.0.0 (/testbed/crates/parser)\n"
    "    Finished `test` profile [unoptimized + debuginfo] target(s) in 94.00s\n"
)
HAPPY_LABELS = [
    "tag-check", "base-id", "run", "clear-tracked", "copy", "extract", "list-files", "clear-dirs", "touch",
    "metadata", "warm-build", "git-attributes", "git-add", "git-add-force", "git-commit", "ls-files",
    "rev-parse", "tag-check", "docker-commit",  # the second tag-check is the re-inspect before the commit
    "inspect", "rm",
]


def blob_id(data):
    """The git blob id of `data`, computed here (not by the code under test)."""
    return hashlib.sha1(b"blob %d\0" % len(data) + data).hexdigest()


def tree_entry(content):
    """`(git mode, blob id)` of a TREE value: bytes, ("exec", bytes) or ("symlink", target)."""
    if isinstance(content, tuple) and content[0] == "symlink":
        return "120000", blob_id(content[1].encode())
    if isinstance(content, tuple):
        return "100755", blob_id(content[1])
    return "100644", blob_id(content)


TREE_ENTRIES = {path: tree_entry(content) for path, content in TREE.items()}  # path -> (mode, blob id)


def ls_s(entries):
    """`git ls-files -s -z` output for `{path: (mode, blob)}`."""
    return "".join("%s %s 0\t%s\0" % (mode, blob, path) for path, (mode, blob) in sorted(entries.items()))


PINNED = ("-c", "core.autocrlf=false", "-c", "core.safecrlf=false", "-c", "core.fileMode=true")


def exec_env(args):
    """The `-e` values of a `docker exec` argv (without the leading `docker`)."""
    options = args[:args.index(CID)]
    return [options[i + 1] for i, option in enumerate(options) if option == "-e"]


def done(args, rc=0, out=""):
    """What pilot.docker returns with check=False: stderr is merged into stdout, text mode."""
    return subprocess.CompletedProcess(list(args), rc, out, None)


def label(args):
    """Name the build stage a docker argv (without the leading `docker`) belongs to."""
    text = " ".join(args)
    if args[0] == "run":
        return "run"
    if args[0] == "cp":
        return "copy"
    if args[0] == "commit":
        return "docker-commit"
    if args[:2] == ("image", "inspect"):
        if "{{.Size}}" in text:
            return "inspect"
        if args[-1] == CTL_IMAGE:
            return "gate-image-id"
        return "base-id" if args[-1] == BASE_IMAGE else "tag-check"
    if args[0] == "rmi":
        return "rmi"
    if args[:3] == ("rm", "-f", "-v"):
        return "rm"
    if "git ls-files -z | xargs -0 rm -f" in text:
        return "clear-tracked"
    if "-print0" in text:
        return "list-files"
    if "xargs -0 rm -f" in text:
        return "delete-extras"
    if "-empty" in text:
        return "clear-dirs"
    if "tar -xf" in text:
        return "extract"
    if "touch" in text:
        return "touch"
    if "cargo metadata" in text:
        return "metadata"
    if "cargo test" in text:
        return "warm-build"
    if "--pathspec-from-file" in text:
        return "git-add-force"
    if "ls-files" in text:
        return "ls-files"
    if "^{tree}" in text:
        return "tree-rev-parse"
    if "rev-parse" in text:
        return "rev-parse"
    if " commit " in text:
        return "git-commit"
    if "info/attributes" in text:
        return "git-attributes"
    if " add -A" in text:
        return "git-add"
    return "?"


class FakeDocker:
    """Stands in for pilot.docker: records every call (argv without the leading `docker`, and its
    keyword arguments) and answers by rule. A rule is `(label_or_predicate, response)`; a response
    is a CompletedProcess, an `(exit_code, stdout)` tuple, a function `(args, kwargs) -> CompletedProcess`
    or an exception to raise. The first matching rule wins; anything else succeeds with a plausible stdout."""

    def __init__(self, *rules, files=TREE_FILES):
        self.rules = list(rules)
        self.files = list(files)
        self.calls = []

    def __call__(self, *args, **kw):
        args = tuple(str(a) for a in args)
        self.calls.append((args, kw))
        for match, resp in self.rules:
            if (match(args) if callable(match) else match == label(args)):
                if isinstance(resp, BaseException):
                    raise resp
                if isinstance(resp, tuple):
                    return done(args, *resp)
                if callable(resp):
                    return resp(args, kw)
                return resp
        return done(args, *self.default(args))

    def default(self, args):
        name = label(args)
        if name == "run":
            return 0, CID + "\n"
        if name == "tag-check":
            return 1, "Error response from daemon: No such image: %s\n" % TAG
        if name == "base-id":
            return 0, BASE_ID + "\n"
        if name == "gate-image-id":
            return 0, GATE_ID + "\n"
        if name == "list-files":
            return 0, "".join("./%s\0" % p for p in self.files)
        if name == "ls-files":
            return 0, ls_s({p: TREE_ENTRIES.get(p, ("100644", blob_id(b""))) for p in self.files})
        if name == "rev-parse":
            return 0, SYN_SHA + "\n"
        if name == "tree-rev-parse":
            return 0, TREE_SHA + "\n"
        if name == "inspect":
            return 0, IMAGE_ID + " 4294967296\n"
        if name == "warm-build":
            return 0, WARM_OK
        return 0, ""

    def labels(self):
        return [label(args) for args, _ in self.calls]

    def args_of(self, name):
        found = [args for args, _ in self.calls if label(args) == name]
        assert len(found) == 1, (name, found)
        return found[0]
    def all_args_of(self, name):
        return [args for args, _ in self.calls if label(args) == name]

    def kw_of(self, name):
        found = [kw for args, kw in self.calls if label(args) == name]
        assert len(found) == 1, (name, found)
        return found[0]


def write_tar(path, tree=TREE):
    """A `git archive`-shaped tarball of `tree`: a directory member, files (mode 0644, or 0755 for
    `("exec", data)`), symlinks (`("symlink", target)`) and hard links (`("hardlink", target_path)`)."""
    with tarfile.open(path, "w") as tar:
        info = tarfile.TarInfo("crates")
        info.type = tarfile.DIRTYPE
        tar.addfile(info)
        for name, content in sorted(tree.items()):
            info = tarfile.TarInfo(name)
            if isinstance(content, tuple) and content[0] in ("symlink", "hardlink"):
                info.type = tarfile.SYMTYPE if content[0] == "symlink" else tarfile.LNKTYPE
                info.linkname = content[1]
                tar.addfile(info)
            else:
                data = content[1] if isinstance(content, tuple) else content
                info.size = len(data)
                info.mode = 0o755 if isinstance(content, tuple) else 0o644
                tar.addfile(info, io.BytesIO(data))
    return path


def local_shell(root, env=None):
    """A FakeDocker response that runs the exec'd command for real, in `root`, with the call's stdin
    and its `-e` options (on top of `env`, when given)."""
    def run(args, kw):
        i = args.index(CID)
        options = args[:i]
        run_env = dict(os.environ if env is None else env)
        for j, option in enumerate(options):
            if option == "-e":
                key, value = options[j + 1].split("=", 1)
                run_env[key] = value
        proc = subprocess.run(list(args[i + 1:]), cwd=root, input=kw.get("data"), env=run_env,
                              stdin=None if kw.get("data") is not None else subprocess.DEVNULL,
                              capture_output=True, text=True)
        return done(args, proc.returncode, proc.stdout + proc.stderr)
    return run


class ImageTagTest(unittest.TestCase):
    def test_tag_shape(self):
        self.assertEqual(fb.image_tag(REPO, PR), TAG)
        self.assertEqual(fb.image_tag("Foo/Bar.rs", 7), "sab-fresh-foo__bar.rs-7:b%s" % fb.BUILDER_VERSION)

    def test_tag_is_a_valid_docker_reference(self):
        name = fb.image_tag("Some-Org/My_Repo", 12).split(":")[0]
        self.assertRegex(name, r"[a-z0-9]+((\.|_|__|-+)[a-z0-9]+)*\Z")

    def test_unsafe_repo_or_pr_is_refused(self):
        for bad in ("rust-analyzer", "a/b/c", "", "/x", "x/", "a b/c", "a/b;rm", "a/..", "a/b.", "a/_b", "a/b$(x)",
                    "é/x", None, 5):
            with self.assertRaises(ValueError, msg=repr(bad)):
                fb.image_tag(bad, PR)
        for bad in (0, -1, True, "23096", 1.5, None):
            with self.assertRaises(ValueError, msg=repr(bad)):
                fb.image_tag(REPO, bad)

    def test_build_error_is_a_runtime_error(self):
        self.assertTrue(issubclass(fb.BuildError, RuntimeError))


class StorageFreeTest(unittest.TestCase):
    def test_smallest_free_space_of_the_readable_roots(self):
        usage = {"/a": 300e9, "/b": 120e9}

        def fake_usage(path):
            if path not in usage:
                raise FileNotFoundError(path)
            return mock.Mock(free=usage[path])
        with mock.patch.object(fb.shutil, "disk_usage", fake_usage):
            self.assertEqual(fb.data_root_free_gb(["/a", "/missing", "/b"]), 120.0)

    def test_no_readable_root_is_a_refusal_not_a_guess(self):
        with mock.patch.object(fb.shutil, "disk_usage", side_effect=FileNotFoundError):
            with self.assertRaises(fb.BuildError):
                fb.data_root_free_gb(["/x", "/y"])

    def test_default_roots_come_from_the_daemon_config_or_var_lib(self):
        with tempfile.TemporaryDirectory() as tmp:
            cfg = Path(tmp) / "daemon.json"
            cfg.write_text(json.dumps({"data-root": "/mnt/big"}))
            self.assertEqual(fb._storage_roots(cfg), ["/mnt/big", "/var/lib/containerd"])
            cfg.write_text("not json")
            self.assertEqual(fb._storage_roots(cfg), ["/var/lib/docker", "/var/lib/containerd"])
            self.assertEqual(fb._storage_roots(Path(tmp) / "absent.json"), ["/var/lib/docker", "/var/lib/containerd"])


class BuildImageTest(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.tmp = Path(tmp.name)
        self.tar = write_tar(self.tmp / "tree.tar")
        self.evidence = self.tmp / "evidence"

    def build(self, docker, free=lambda: 500.0, **kw):
        args = dict(base_image=BASE_IMAGE, repo=REPO, pr=PR, upstream_base=UPSTREAM_BASE, tree_tar=self.tar,
                    evidence_dir=self.evidence, docker=docker, free_gb=free)
        args.update(kw)
        return fb.build_image(**args)

    def evidence_json(self, name="build-evidence.json"):
        return json.loads((self.evidence / name).read_text())

    def fails(self, fake, pattern=r".", *, commit_attempted=False, removal="id", evidence_name="build-evidence.json",
              **kw):
        """Run a build that must raise BuildError; check the invariants every failure shares.

        `commit_attempted`: the failure happened from the `docker commit` attempt on, so the tag is
        dropped with `docker rmi`; otherwise `docker rmi` and `docker commit` must not appear.
        `removal`: "id" (`docker rm -f -v <id>`), "name" (by name: `docker run` gave no answer) or None
        (no container of this build exists: nothing may be removed).
        `evidence_name`: the file the evidence is written to (a "tag exists" refusal writes the refused one).
        """
        with self.assertRaisesRegex(fb.BuildError, pattern) as cm:
            self.build(fake, **kw)
        labels = fake.labels()
        if commit_attempted:
            self.assertEqual(fake.args_of("rmi"), ("rmi", TAG))
            self.assertLess(labels.index("docker-commit"), labels.index("rmi"))
        else:
            self.assertNotIn("docker-commit", labels)
            self.assertNotIn("rmi", labels)
        ev = self.evidence_json(evidence_name)
        self.assertEqual(ev["outcome"], "error")
        if removal is None:
            self.assertNotIn("rm", labels)
            self.assertIsNone(ev["container_removed"])
            self.assertFalse(ev["container_removed_by_name"])
            self.assertTrue(ev["removal_note"])
        else:
            self.assertEqual(labels[-1], "rm")
            self.assertEqual(fake.args_of("rm"), ("rm", "-f", "-v", CID if removal == "id" else CONTAINER))
            self.assertTrue(ev["container_removed"])
            self.assertEqual(ev["container_removed_by_name"], removal == "name")
        return cm.exception

    # --- the happy path ---

    def test_happy_path_runs_the_stages_in_order(self):
        fake = FakeDocker()
        got = self.build(fake)
        self.assertEqual(fake.labels(), HAPPY_LABELS)
        self.assertEqual(got["image"], TAG)
        self.assertEqual(got["base_commit"], SYN_SHA)
        self.assertEqual(got["base_image_id"], BASE_ID)
        self.assertEqual(got["tree_tar_sha256"], hashlib.sha256(self.tar.read_bytes()).hexdigest())
        self.assertEqual(got["image_id"], IMAGE_ID)
        self.assertEqual(got["size_bytes"], 4294967296)
        self.assertEqual(got["compiling_lines"], 2)
        self.assertTrue(got["container_removed"])
        self.assertEqual(set(got["laps_seconds"]), {
            "run", "extract", "metadata", "warm_build", "git_commit", "image_commit"})
        for secs in got["laps_seconds"].values():
            self.assertGreaterEqual(secs, 0)
        self.assertEqual(set(got), {"image", "base_commit", "base_image_id", "tree_tar_sha256", "image_id",
                                    "size_bytes", "laps_seconds", "compiling_lines", "container_removed"})

    def test_container_is_started_with_network_and_the_pilot_limits(self):
        fake = FakeDocker()
        self.build(fake)
        run = fake.args_of("run")
        cidfile = Path(run[run.index("--cidfile") + 1])
        self.assertEqual((cidfile.parent.parent, cidfile.name), (self.evidence.resolve(), "run.cid"))
        self.assertEqual(run[:4] + run[6:], (
            "run", "-d", "--name", CONTAINER,
            "--cpus", "4", "--memory", "24g", BASE_ID, "sleep", "infinity"))

    def test_offline_override_is_on_the_warm_build_exec_only(self):
        fake = FakeDocker()
        self.build(fake)
        with_flag = [label(args) for args, _ in fake.calls if "CARGO_NET_OFFLINE=false" in args]
        self.assertEqual(with_flag, ["warm-build"])
        warm = fake.args_of("warm-build")
        i = warm.index("-e")
        self.assertEqual(warm[i:i + 2], ("-e", "CARGO_NET_OFFLINE=false"))
        self.assertLess(i, warm.index(CID))  # an exec option, before the container
        self.assertEqual(warm[warm.index(CID) + 1:], ("cargo", "test", "--no-run"))
        # the committed image keeps the baked offline setting: no ENV is set anywhere
        for name in ("run", "docker-commit"):
            args = fake.args_of(name)
            self.assertNotIn("-e", args)
            self.assertNotIn("--env", args)
            self.assertFalse([a for a in args if "CARGO_NET_OFFLINE" in a])

    def test_commit_restores_the_cmd_and_goes_to_the_study_tag_by_container_id(self):
        fake = FakeDocker()
        self.build(fake)
        self.assertEqual(fake.args_of("docker-commit"), ("commit", "--change", 'CMD ["/bin/bash"]', CID, TAG))
        changes = [a for args, _ in fake.calls for a in args if args[0] == "commit" and a.startswith(("CMD", "ENV"))]
        self.assertEqual(changes, ['CMD ["/bin/bash"]'])  # no ENV change: the image keeps CARGO_NET_OFFLINE=true
        self.assertEqual(fake.args_of("rm"), ("rm", "-f", "-v", CID))
        self.assertEqual(fake.labels()[-1], "rm")

    def test_every_exec_runs_in_the_workdir_by_container_id_and_the_workdir_is_a_parameter(self):
        for workdir in ("/testbed", "/work/repo"):
            with self.subTest(workdir=workdir):
                fake = FakeDocker()
                self.build(fake, workdir=workdir)
                execs = [args for args, _ in fake.calls if args[0] == "exec"]
                self.assertEqual(len(execs), 13)
                for args in execs:
                    i = args.index("-w")
                    self.assertEqual(args[i + 1], workdir)
                    self.assertEqual(args[i + 2], CID)

    # --- ownership of the image tag and of the container ---

    def test_tag_and_base_image_are_inspected_before_the_container_is_started(self):
        fake = FakeDocker()
        self.build(fake)
        self.assertEqual(fake.labels()[:3], ["tag-check", "base-id", "run"])
        self.assertEqual(fake.all_args_of("tag-check")[0], ("image", "inspect", "--format", "{{.Id}}", TAG))
        self.assertEqual(fake.args_of("base-id"), ("image", "inspect", "--format", "{{.Id}}", BASE_IMAGE))

    def test_an_existing_tag_is_refused_before_anything_is_started_or_deleted(self):
        fake = FakeDocker(("tag-check", (0, IMAGE_ID + "\n")))
        self.fails(fake, "tag exists", removal=None, evidence_name="build-evidence.refused.json")
        self.assertEqual(fake.labels(), ["tag-check"])

    def test_a_tag_check_that_is_not_a_plain_no_such_image_is_refused(self):
        for resp in ((1, "Cannot connect to the Docker daemon"), (125, "Error response: No such image: x"),
                     (1, ""), (2, "no such image")):
            with self.subTest(resp=resp):
                fake = FakeDocker(("tag-check", resp))
                self.fails(fake, "docker image inspect", removal=None)
                self.assertEqual(fake.labels(), ["tag-check"])

    def test_the_base_image_id_is_required_and_recorded(self):
        for resp in ((1, "No such image"), (0, "garbage\n"), (0, "")):
            with self.subTest(resp=resp):
                fake = FakeDocker(("base-id", resp))
                self.fails(fake, "base image", removal=None)
                self.assertEqual(fake.labels(), ["tag-check", "base-id"])
        self.build(FakeDocker())
        self.assertEqual(self.evidence_json()["base_image_id"], BASE_ID)

    def test_a_name_conflict_removes_nothing_so_a_sibling_build_survives(self):
        conflict = (125, 'docker: Error response from daemon: Conflict. The container name "/%s" is already in '
                         'use by container "%s".' % (CONTAINER, "f" * 64))
        fake = FakeDocker(("run", conflict))
        self.fails(fake, r"docker run failed \(exit 125\)", removal=None)
        self.assertEqual(fake.labels(), ["tag-check", "base-id", "run"])
        ev = self.evidence_json()
        self.assertIn("nothing was removed", ev["removal_note"])
        self.assertIsNone(ev["container_id"])

    def test_a_run_with_no_answer_removes_by_name_and_says_so(self):
        for resp in (subprocess.TimeoutExpired(cmd=["docker"], timeout=600), FileNotFoundError("docker")):
            with self.subTest(resp=repr(resp)):
                fake = FakeDocker(("run", resp))
                self.fails(fake, "docker run", removal="name")
        fake = FakeDocker(("run", KeyboardInterrupt()))
        with self.assertRaises(KeyboardInterrupt):
            self.build(fake)
        self.assertEqual(fake.args_of("rm"), ("rm", "-f", "-v", CONTAINER))
        self.assertTrue(self.evidence_json()["container_removed_by_name"])

    def test_a_run_that_printed_no_container_id_removes_by_name(self):
        fake = FakeDocker(("run", (0, "some warning, no id\n")))
        self.fails(fake, "no container ID", removal="name")

    def test_a_warning_line_before_the_container_id_is_tolerated(self):
        fake = FakeDocker(("run", (0, "WARNING: Your kernel does not support swap limit capabilities\n%s\n" % CID)))
        self.build(fake)
        self.assertEqual(fake.args_of("rm"), ("rm", "-f", "-v", CID))
    # --- the base image ID, not its reference, starts the container ---

    def test_run_uses_the_recorded_base_image_id_not_the_mutable_reference(self):
        fake = FakeDocker()
        self.build(fake)
        run = fake.args_of("run")
        self.assertEqual(run[-3:], (BASE_ID, "sleep", "infinity"))
        self.assertNotIn(BASE_IMAGE, run)
        self.assertEqual(self.evidence_json()["base_image"], BASE_IMAGE)  # the reference is still recorded

    # --- the cidfile: a container that was created but did not start is ours and is removed by ID ---

    def cidfile_run(self, rc, out, written=None):
        """A `docker run` response that behaves like docker with `--cidfile`: it checks the file is
        absent (docker refuses to start otherwise) and writes `written` into it, when given."""
        def respond(args, kw):
            path = Path(args[args.index("--cidfile") + 1])
            self.assertFalse(path.exists(), "docker refuses to start when the cidfile exists")
            if written is not None:
                path.write_text(written)
            return done(args, rc, out)
        return respond

    def test_the_cidfile_is_fresh_per_call_so_runs_sharing_an_evidence_dir_never_read_each_others(self):
        paths = []
        for _ in range(2):
            fake = FakeDocker(("run", self.cidfile_run(0, CID + "\n", written=CID)))
            self.build(fake)
            run = fake.args_of("run")
            paths.append(Path(run[run.index("--cidfile") + 1]))
            self.assertEqual(fake.args_of("rm"), ("rm", "-f", "-v", CID))
        self.assertNotEqual(paths[0], paths[1])
        for path in paths:
            self.assertEqual((path.parent.parent, path.name), (self.evidence.resolve(), "run.cid"))
            self.assertFalse(path.parent.exists(), "the per-call directory is cleaned up")

    def test_the_per_call_cidfile_directory_is_cleaned_up_after_a_failure_too(self):
        seen = []

        def respond(args, kw):
            seen.append(Path(args[args.index("--cidfile") + 1]))
            return done(args, 125, "name in use")
        self.fails(FakeDocker(("run", respond)), "docker run failed", removal=None)
        self.assertFalse(seen[0].parent.exists())
        self.assertEqual(sorted(p.name for p in self.evidence.iterdir()), ["build-evidence.json"])

    def test_a_cidfile_of_a_fixed_name_in_the_evidence_dir_is_never_read(self):
        self.evidence.mkdir()
        (self.evidence / "container.cid").write_text(CID)  # another run's ID
        fake = FakeDocker(("run", (125, "name in use")))
        self.fails(fake, "docker run failed", removal=None)

    def test_a_run_that_failed_after_creating_the_container_removes_it_by_the_cidfile_id(self):
        oci = "docker: Error response from daemon: OCI runtime create failed: unable to start container process"
        fake = FakeDocker(("run", self.cidfile_run(125, oci, written=CID)))
        self.fails(fake, r"docker run failed \(exit 125\)", removal="id")
        ev = self.evidence_json()
        self.assertEqual(ev["container_id"], CID)
        self.assertFalse(ev["container_removed_by_name"])
        self.assertEqual(fake.labels(), ["tag-check", "base-id", "run", "rm"])

    def test_a_cidfile_id_is_what_the_file_holds_even_with_a_trailing_newline(self):
        fake = FakeDocker(("run", self.cidfile_run(125, "failed to start", written=CID + "\n")))
        self.fails(fake, "docker run failed", removal="id")

    def test_a_failed_run_without_a_usable_cidfile_removes_nothing(self):
        conflict = (125, 'Conflict. The container name "/%s" is already in use by container "%s".'
                         % (CONTAINER, "f" * 64))
        for written in (None, "", "not an id", CID[:-1], CID + "0", "f" * 64 + " " + "e" * 64):
            for stale in (False, True):
                with self.subTest(written=written, stale=stale):
                    self.evidence.mkdir(exist_ok=True)
                    cidfile = self.evidence / "container.cid"
                    if stale:  # a previous run's ID must not be mistaken for this run's
                        cidfile.write_text("f" * 64)
                    elif cidfile.exists():
                        cidfile.unlink()
                    fake = FakeDocker(("run", self.cidfile_run(*conflict, written=written)))
                    self.fails(fake, r"docker run failed \(exit 125\)", removal=None)
                    self.assertIsNone(self.evidence_json()["container_id"])

    def test_a_cidfile_does_not_make_a_failed_run_with_no_answer_remove_by_id(self):
        fake = FakeDocker(("run", subprocess.TimeoutExpired(cmd=["docker"], timeout=600)))
        self.fails(fake, "docker run", removal="name")

    # --- the tag may appear during the build: re-inspect right before the commit ---

    def tag_answers(self, *answers):
        """A `tag-check` rule answering the n-th call with the n-th `(exit_code, stdout)` of `answers`."""
        seen = []

        def respond(args, kw):
            seen.append(args)
            return done(args, *answers[min(len(seen), len(answers)) - 1])
        return ("tag-check", respond)

    ABSENT = (1, "Error response from daemon: No such image: %s\n" % TAG)

    def test_the_tag_is_inspected_again_immediately_before_the_commit(self):
        fake = FakeDocker()
        self.build(fake)
        labels = fake.labels()
        self.assertEqual(labels[labels.index("docker-commit") - 1], "tag-check")
        self.assertEqual(len(fake.all_args_of("tag-check")), 2)
        self.assertEqual(fake.all_args_of("tag-check")[1], ("image", "inspect", "--format", "{{.Id}}", TAG))

    def test_a_tag_that_appears_during_the_build_is_refused_and_never_dropped(self):
        fake = FakeDocker(self.tag_answers(self.ABSENT, (0, IMAGE_ID + "\n")))
        err = self.fails(fake, "tag exists", evidence_name="build-evidence.refused.json")
        self.assertIn(TAG, str(err))
        self.assertEqual(fake.labels()[-3:], ["rev-parse", "tag-check", "rm"])  # no commit, and no rmi of a sibling's tag
        self.assertEqual(len(fake.all_args_of("tag-check")), 2)
        self.assertIn("tag exists", self.evidence_json("build-evidence.refused.json")["error"])

    def test_a_recheck_that_is_not_a_plain_no_such_image_is_refused_without_dropping_the_tag(self):
        for resp in ((1, "Cannot connect to the Docker daemon"), (2, "no such image"), (1, "")):
            with self.subTest(resp=resp):
                fake = FakeDocker(self.tag_answers(self.ABSENT, resp))
                self.fails(fake, "docker image inspect")
                self.assertEqual(fake.labels()[-2:], ["tag-check", "rm"])

    def test_a_recheck_that_times_out_is_refused_without_dropping_the_tag(self):
        calls = []

        def respond(args, kw):
            calls.append(args)
            if len(calls) == 2:
                raise subprocess.TimeoutExpired(cmd=["docker"], timeout=120)
            return done(args, *self.ABSENT)
        fake = FakeDocker(("tag-check", respond))
        self.fails(fake, "timed out")
        self.assertEqual(fake.labels()[-2:], ["tag-check", "rm"])

    # --- a "tag exists" refusal never overwrites an existing build-evidence.json ---

    def test_a_tag_exists_refusal_never_overwrites_an_existing_build_evidence(self):
        self.evidence.mkdir()
        good = self.evidence / "build-evidence.json"
        sentinel = '{"outcome": "ok", "image_id": "kept"}\n'
        for name, rule in (("at the start", ("tag-check", (0, IMAGE_ID + "\n"))),
                           ("before the commit", self.tag_answers(self.ABSENT, (0, IMAGE_ID + "\n")))):
            with self.subTest(refused=name):
                good.write_text(sentinel)
                (self.evidence / "build-evidence.refused.json").unlink(missing_ok=True)
                self.fails(FakeDocker(rule), "tag exists", removal=None if name == "at the start" else "id",
                           evidence_name="build-evidence.refused.json")
                self.assertEqual(good.read_text(), sentinel)

    def test_other_failures_and_good_builds_write_the_usual_evidence_file(self):
        self.fails(FakeDocker(("warm-build", (101, "error: boom\n"))), "boom")
        self.assertFalse((self.evidence / "build-evidence.refused.json").exists())
        self.build(FakeDocker())
        self.assertEqual(self.evidence_json()["outcome"], "ok")
        self.assertFalse((self.evidence / "build-evidence.refused.json").exists())

    # --- expected_tree: the synthetic commit's tree must be the upstream tree ---

    def test_expected_tree_is_compared_to_the_synthetic_commit_before_the_commit(self):
        fake = FakeDocker()
        self.build(fake, expected_tree=TREE_SHA)
        want = list(HAPPY_LABELS)
        want.insert(want.index("rev-parse") + 1, "tree-rev-parse")
        self.assertEqual(fake.labels(), want)
        args = fake.args_of("tree-rev-parse")
        self.assertEqual(args[args.index(CID) + 1:], ("git", "rev-parse", "HEAD^{tree}"))
        ev = self.evidence_json()
        self.assertEqual((ev["expected_tree"], ev["synthetic_tree"]), (TREE_SHA, TREE_SHA))

    def test_without_expected_tree_the_tree_is_not_asked_for(self):
        fake = FakeDocker()
        self.build(fake)
        self.assertNotIn("tree-rev-parse", fake.labels())
        self.assertNotIn("expected_tree", self.evidence_json())

    def test_a_different_tree_is_refused_naming_both_shas_before_the_commit(self):
        fake = FakeDocker(("tree-rev-parse", (0, OTHER_TREE + "\n")))
        err = self.fails(fake, "tree", expected_tree=TREE_SHA)
        self.assertIn(TREE_SHA, str(err))
        self.assertIn(OTHER_TREE, str(err))
        self.assertEqual(fake.labels()[-2:], ["tree-rev-parse", "rm"])
        self.assertEqual(len(fake.all_args_of("tag-check")), 1)  # refused before the re-inspect and the commit
        ev = self.evidence_json()
        self.assertEqual((ev["expected_tree"], ev["synthetic_tree"]), (TREE_SHA, OTHER_TREE))
        self.assertIn(OTHER_TREE, ev["error"])

    def test_a_tree_answer_that_is_not_a_sha_or_a_failing_command_is_refused(self):
        for resp in ((0, "fatal: ambiguous argument\n"), (0, ""), (128, "fatal: not a git repository\n"),
                     (0, TREE_SHA.upper() + "\n"), (0, TREE_SHA[:-1] + "\n")):
            with self.subTest(resp=resp):
                fake = FakeDocker(("tree-rev-parse", resp))
                self.fails(fake, "tree", expected_tree=TREE_SHA)

    def test_expected_tree_must_be_a_full_lowercase_40_hex_sha(self):
        for bad in ("", "abc", TREE_SHA.upper(), "1f" * 32, TREE_SHA + "\n", TREE_SHA[:-1] + "g", None, 5, b"x" * 40):
            with self.subTest(bad=bad):
                fake = FakeDocker()
                if bad is None:  # None means "not given"
                    self.build(fake, expected_tree=bad)
                    continue
                with self.assertRaises(ValueError):
                    self.build(fake, expected_tree=bad)
                self.assertEqual(fake.calls, [])

    @unittest.skipUnless(shutil.which("git"), "git is not installed")
    def test_real_git_a_tree_that_differs_from_upstream_by_line_endings_is_refused(self):
        # `git archive` applies `eol`, so its CRLF bytes pass the per-file check against the archive,
        # but the tree they make is not the upstream tree: only the tree SHA shows it
        cfg = self.tmp / "empty.gitconfig"
        cfg.write_text("")
        env = {"PATH": os.environ["PATH"], "HOME": str(self.tmp), "GIT_CONFIG_GLOBAL": str(cfg),
               "GIT_CONFIG_NOSYSTEM": "1", "LC_ALL": "C"}

        def git(root, *args):
            return subprocess.run(["git", *args], cwd=root, env=env, check=True, capture_output=True,
                                  text=True).stdout

        def repo(name, files):
            root = self.tmp / name
            root.mkdir()
            git(root, "init", "-q")
            for path, content in files.items():
                (root / path).write_bytes(content[1] if isinstance(content, tuple) else content)
                if isinstance(content, tuple):
                    (root / path).chmod(0o755)
            return root

        upstream_files = {"a.txt": b"x\n", "run.sh": ("exec", b"#!/bin/sh\n")}
        up = repo("upstream", upstream_files)
        git(up, "add", "-A")
        upstream_tree = git(up, "write-tree").strip()
        self.assertRegex(upstream_tree, r"^[0-9a-f]{40}$")
        archive_files = dict(upstream_files, **{"a.txt": b"x\r\n"})  # what `eol=crlf` makes `git archive` write
        for name, files in (("same", upstream_files), ("crlf", archive_files)):
            d = repo(name, files)
            shell = local_shell(d, env)
            fake = FakeDocker(("git-attributes", shell), ("git-add", shell), ("git-add-force", shell),
                              ("git-commit", shell), ("ls-files", shell), ("rev-parse", shell),
                              ("tree-rev-parse", shell), files=sorted(files))
            tar = write_tar(self.tmp / (name + ".tar"), files)
            with self.subTest(archive=name):
                if name == "same":
                    self.build(fake, tree_tar=tar, expected_tree=upstream_tree)
                    self.assertEqual(self.evidence_json()["synthetic_tree"], upstream_tree)
                else:
                    err = self.fails(fake, "tree", tree_tar=tar, expected_tree=upstream_tree)
                    self.assertIn(upstream_tree, str(err))
                    self.assertIn(git(d, "rev-parse", "HEAD^{tree}").strip(), str(err))

    def test_the_tag_is_dropped_only_from_the_commit_attempt_on(self):
        steps = ["tag-check", "base-id", "run", "clear-tracked", "copy", "extract", "list-files", "clear-dirs",
                 "touch", "metadata", "warm-build", "git-attributes", "git-add", "git-add-force", "git-commit",
                 "ls-files",
                 "rev-parse"]
        for name in steps:
            with self.subTest(step=name):
                fake = FakeDocker((name, (7, "step failed")))
                with self.assertRaises(fb.BuildError):
                    self.build(fake)
                self.assertNotIn("rmi", fake.labels())
                self.assertNotIn("docker-commit", fake.labels())

    def test_an_interrupt_during_the_commit_drops_the_tag_and_removes_the_container(self):
        fake = FakeDocker(("docker-commit", KeyboardInterrupt()))
        with self.assertRaises(KeyboardInterrupt):
            self.build(fake)
        self.assertEqual(fake.labels()[-3:], ["docker-commit", "rmi", "rm"])
        self.assertEqual(fake.args_of("rmi"), ("rmi", TAG))
        self.assertEqual(self.evidence_json()["outcome"], "error")

    def test_a_commit_that_gets_no_answer_or_fails_drops_the_tag(self):
        for resp in (subprocess.TimeoutExpired(cmd=["docker"], timeout=3600), (1, "no space left on device")):
            with self.subTest(resp=repr(resp)):
                fake = FakeDocker(("docker-commit", resp))
                self.fails(fake, "docker commit", commit_attempted=True)
                self.assertNotIn("inspect", fake.labels())

    def test_a_failing_inspect_after_the_commit_drops_the_tag(self):
        for resp in ((1, "no such image"), (0, "garbage\n"), (0, IMAGE_ID + " notanumber\n")):
            with self.subTest(resp=resp):
                fake = FakeDocker(("inspect", resp))
                self.fails(fake, "inspect", commit_attempted=True)

    def test_an_evidence_write_failure_after_a_good_build_drops_the_tag_and_raises_build_error(self):
        fake = FakeDocker()
        with mock.patch.object(fb, "_write_evidence", side_effect=OSError("disk full")):
            with self.assertRaisesRegex(fb.BuildError, "evidence.*disk full"):
                self.build(fake)
        self.assertEqual(fake.labels()[-2:], ["rm", "rmi"])
        self.assertEqual(fake.args_of("rmi"), ("rmi", TAG))

    def test_an_evidence_write_failure_never_replaces_a_build_error(self):
        fake = FakeDocker(("warm-build", (101, "error: the real cause\n")))
        with mock.patch.object(fb, "_write_evidence", side_effect=OSError("disk full")):
            with self.assertRaisesRegex(fb.BuildError, "the real cause"):
                self.build(fake)
        self.assertNotIn("rmi", fake.labels())  # nothing was committed
        self.assertEqual(fake.labels()[-1], "rm")

    # --- the tree swap ---

    def test_tracked_files_are_deleted_by_one_pipefail_bash_command(self):
        fake = FakeDocker()
        self.build(fake)
        args = fake.args_of("clear-tracked")
        i = args.index(CID)
        self.assertEqual(args[i + 1:], ("bash", "-o", "pipefail", "-c", "git ls-files -z | xargs -0 rm -f"))

    def test_tree_is_copied_by_resolved_path_extracted_over_the_workdir_and_the_tarball_removed(self):
        fake = FakeDocker()
        self.build(fake)
        self.assertEqual(fake.args_of("copy"), ("cp", str(self.tar.resolve()), CID + ":/tmp/sab-tree.tar"))
        script = fake.args_of("extract")[-1]
        self.assertIn("tar -xf /tmp/sab-tree.tar", script)
        self.assertIn("rm -f /tmp/sab-tree.tar", script)  # else it would be baked into the image

    def test_a_relative_tarball_path_is_passed_to_docker_cp_resolved(self):
        fake = FakeDocker()
        old = Path.cwd()
        os.chdir(self.tmp)
        try:
            self.build(fake, tree_tar=Path("tree.tar"))
        finally:
            os.chdir(old)
        self.assertEqual(fake.args_of("copy")[1], str(self.tar.resolve()))

    def test_stages_of_the_swap_run_in_order(self):
        fake = FakeDocker(("list-files", (0, "".join("./%s\0" % p for p in TREE_FILES + ["old.log"]))))
        self.build(fake)
        labels = fake.labels()
        order = ["clear-tracked", "copy", "extract", "list-files", "delete-extras", "clear-dirs", "touch",
                 "metadata", "warm-build"]
        self.assertEqual([labels.index(n) for n in order], sorted(labels.index(n) for n in order))

    def test_files_that_are_not_in_the_tarball_are_listed_and_deleted_by_nul_separated_stdin(self):
        extras = ["old/dir/x.txt", "old/dir/sub/y", "crates/keep/src/test-output.log", "has space\nand newline",
                  "-dash"]
        fake = FakeDocker(("list-files", (0, "".join("./%s\0" % p for p in TREE_FILES + extras))))
        self.build(fake)
        args, kw = fake.args_of("delete-extras"), fake.kw_of("delete-extras")
        self.assertEqual(args, ("exec", "-i", "-w", "/testbed", CID, "xargs", "-0", "rm", "-f"))
        self.assertEqual(kw["data"], "".join("./%s\0" % p for p in sorted(extras)))
        self.assertIs(kw["check"], False)
        self.assertEqual(self.evidence_json()["leftover_files_deleted"], len(extras))

    def test_nothing_is_deleted_when_the_listing_is_the_tree(self):
        fake = FakeDocker()
        self.build(fake)
        self.assertNotIn("delete-extras", fake.labels())

    def test_the_listing_skips_target_and_git_and_a_failing_listing_or_delete_raises(self):
        fake = FakeDocker()
        self.build(fake)
        args = fake.args_of("list-files")
        self.assertEqual(args[args.index(CID) + 1:], (
            "find", ".", "-path", "./target", "-prune", "-o", "-path", "./.git", "-prune", "-o",
            "(", "-type", "f", "-o", "-type", "l", ")", "-print0"))
        self.fails(FakeDocker(("list-files", (1, "find: boom"))), "list files failed")
        fake = FakeDocker(("list-files", (0, "".join("./%s\0" % p for p in TREE_FILES + ["old.log"]))),
                          ("delete-extras", (1, "rm: cannot remove")))
        self.fails(fake, "delete leftover files failed")
        for bad in ("not a path\0", "./a\0noprefix\0"):
            self.fails(FakeDocker(("list-files", (0, bad))), "not a path")

    def test_the_cleanup_really_removes_leftovers_and_spares_the_tree_target_and_git(self):
        # the real find / xargs / bash text, run on a scratch tree through build_image itself
        r = self.tmp / "tree"
        for d in ("crates/keep/src", "crates/gone/src/tests", "old/dir/sub", "target/debug/deps", ".git/objects",
                  "empty", "target/debug/emptydir"):
            (r / d).mkdir(parents=True)
        for name, content in TREE.items():
            (r / name).parent.mkdir(parents=True, exist_ok=True)
            if isinstance(content, tuple) and content[0] == "symlink":
                (r / name).symlink_to(content[1])
            else:
                (r / name).write_bytes(content[1] if isinstance(content, tuple) else content)
        leftovers = ["crates/keep/test-output.log", "crates/keep/src/stale.rs", "old/dir/x.txt", "old/dir/sub/y",
                     "crates/gone/src/old.rs", "has space\nand newline", "-dash", ".env.local"]
        for name in leftovers:
            (r / name).write_text("old")
        (r / "target/debug/deps/artifact").write_text("built")
        (r / ".git/HEAD").write_text("ref")
        shell = local_shell(r)
        fake = FakeDocker(("list-files", shell), ("delete-extras", shell), ("clear-dirs", shell), ("touch", shell))
        self.build(fake)
        for name in TREE:
            self.assertTrue(os.path.lexists(r / name), name)
        for name in leftovers + ["old", "crates/gone", "empty"]:
            self.assertFalse(os.path.lexists(r / name), name)
        self.assertEqual((r / "target/debug/deps/artifact").read_text(), "built")
        self.assertTrue((r / "target/debug/emptydir").is_dir())
        self.assertTrue((r / ".git/HEAD").is_file())
        self.assertTrue((r / "crates/keep/src").is_dir())

    def test_the_empty_directory_script_removes_nested_empties_and_spares_target_and_git(self):
        fake = FakeDocker()
        self.build(fake)
        args = fake.args_of("clear-dirs")
        i = args.index(CID)
        self.assertEqual(args[i + 1:i + 4], ("bash", "-o", "pipefail"))
        script = args[-1]
        for part in ("find .", "-mindepth 1", "-depth", "-type d", "-empty", "-delete", "while"):
            self.assertIn(part, script)
        for guarded in ("./target", "./target/*", "./.git", "./.git/*"):
            self.assertIn("-not -path '%s'" % guarded, script)
        r = self.tmp / "tree"
        for d in ("crates/gone/src/tests", "crates/keep/src", "target/debug/deps", ".git/objects/pack", "empty"):
            (r / d).mkdir(parents=True)
        (r / "crates/keep/src/lib.rs").write_text("x")
        (r / "target/debug/deps/empty_dir").mkdir()
        proc = subprocess.run(["bash", "-o", "pipefail", "-c", script], cwd=r, capture_output=True, text=True)
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertFalse((r / "crates/gone").exists())
        self.assertFalse((r / "empty").exists())
        self.assertTrue((r / "crates/keep/src/lib.rs").exists())
        self.assertTrue((r / "target/debug/deps/empty_dir").is_dir())
        self.assertTrue((r / ".git/objects/pack").is_dir())

    def test_extracted_files_are_touched_after_the_cleanup_and_before_the_build(self):
        fake = FakeDocker()
        self.build(fake)
        labels = fake.labels()
        self.assertLess(labels.index("clear-dirs"), labels.index("touch"))
        self.assertLess(labels.index("touch"), labels.index("metadata"))
        self.assertLess(labels.index("touch"), labels.index("warm-build"))

    def test_the_touch_script_leaves_target_alone_and_refreshes_every_other_file(self):
        fake = FakeDocker()
        self.build(fake)
        script = fake.args_of("touch")[-1]
        r = self.tmp / "tree"
        (r / "target").mkdir(parents=True)
        (r / "src").mkdir()
        old = 1_000_000_000
        for f in (r / "Cargo.toml", r / "src/lib.rs", r / "target/artifact"):
            f.write_text("x")
            os.utime(f, (old, old))
        (r / "src/link").symlink_to("missing-target")  # a dangling symlink must not fail the step
        proc = subprocess.run(["bash", "-o", "pipefail", "-c", script], cwd=r, capture_output=True, text=True)
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertGreater((r / "Cargo.toml").stat().st_mtime, old + 1000)
        self.assertGreater((r / "src/lib.rs").stat().st_mtime, old + 1000)
        self.assertEqual((r / "target/artifact").stat().st_mtime, old)

    def test_no_command_deletes_recursively_cleans_git_or_drops_the_network(self):
        fake = FakeDocker(("list-files", (0, "".join("./%s\0" % p for p in TREE_FILES + ["old.log"]))))
        self.build(fake)
        for args, kw in fake.calls:
            text = " ".join(args) + str(kw.get("data", ""))
            for banned in ("rm -rf", "rm -fr", "clean -f", "--network"):
                self.assertNotIn(banned, text)

    # --- cargo metadata ---

    def test_cargo_metadata_must_succeed_before_the_build(self):
        fake = FakeDocker(("metadata", (101, "error: failed to load manifest for workspace member")))
        err = self.fails(fake, "cargo metadata")
        self.assertIn("failed to load manifest", str(err))
        self.assertNotIn("warm-build", fake.labels())

    def test_cargo_metadata_fails_over_to_the_network_when_offline_fails(self):
        seen = []

        def first_metadata(args):
            if label(args) != "metadata":
                return False
            seen.append(args)
            return len(seen) == 1
        fake = FakeDocker((first_metadata, (101, "no matching package in offline mode")))
        self.build(fake)
        self.assertEqual(fake.labels().count("metadata"), 2)
        self.assertNotIn("CARGO_NET_OFFLINE=false", seen[0])
        self.assertIn("CARGO_NET_OFFLINE=false", seen[1])
        self.assertEqual(fake.labels()[-1], "rm")

    # --- the warm build ---

    def test_warm_build_log_is_kept_in_the_evidence_dir_as_returned(self):
        out = "   Compiling a v1\n   Compiling b v1\né line\n"
        fake = FakeDocker(("warm-build", (0, out)))
        got = self.build(fake)
        self.assertEqual((self.evidence / "build.log").read_bytes(), out.encode("utf-8"))
        self.assertEqual(got["compiling_lines"], 2)

    def test_carriage_returns_in_the_log_are_not_translated_by_the_writer(self):
        out = "   Compiling a v1\r\n   Compiling b v1\n"
        self.build(FakeDocker(("warm-build", (0, out))))
        self.assertEqual((self.evidence / "build.log").read_bytes(), out.encode("utf-8"))

    def test_nonzero_warm_build_exit_raises_with_the_log_tail_even_when_it_compiled(self):
        # Compiling lines present and a failing exit code: only the exit code can reject this run
        out = "".join("   Compiling c%d v1\n" % i for i in range(3)) + "error: could not compile `hir-ty`\n"
        fake = FakeDocker(("warm-build", (101, out)))
        err = self.fails(fake, r"warm build.*101")
        self.assertIn("could not compile `hir-ty`", str(err))
        self.assertEqual((self.evidence / "build.log").read_text(), out)
        self.assertNotIn("git-add", fake.labels())

    def test_exit_code_is_not_inferred_from_a_clean_looking_log(self):
        # the pilot reported a failed 0-second build as exit=0: a log with Compiling and Finished
        # lines but exit 1 is still a failure
        fake = FakeDocker(("warm-build", (1, WARM_OK)))
        self.fails(fake, r"warm build.*exit 1")

    def test_zero_compiling_lines_means_stale_binaries_and_raises(self):
        fake = FakeDocker(("warm-build", (0, "    Finished `test` profile target(s) in 0.30s\n")))
        self.fails(fake, "stale")
        self.assertNotIn("git-add", fake.labels())

    def test_empty_warm_build_output_is_stale_too(self):
        self.fails(FakeDocker(("warm-build", (0, ""))), "stale")

    def test_only_compiling_lines_count(self):
        out = "  Compiling-ish text\nwarning: Compiling x\n   Fresh serde v1\n    Finished `test` profile\n"
        self.fails(FakeDocker(("warm-build", (0, out))), "stale")

    # --- staging and the synthetic commit ---

    def test_git_add_is_followed_by_a_forced_add_of_every_tree_path_even_a_gitignored_one(self):
        fake = FakeDocker()
        self.build(fake)
        labels = fake.labels()
        self.assertEqual(labels.index("git-attributes") + 1, labels.index("git-add"))
        self.assertEqual(labels.index("git-add") + 1, labels.index("git-add-force"))
        self.assertEqual(labels.index("git-add-force") + 1, labels.index("git-commit"))
        add = fake.args_of("git-add")
        self.assertEqual(add[add.index(CID) + 1:], ("git", *PINNED, "add", "-A"))
        force, kw = fake.args_of("git-add-force"), fake.kw_of("git-add-force")
        self.assertEqual(force[force.index(CID) + 1:], ("git", *PINNED, "add", "-f", "--pathspec-from-file=-",
                                                        "--pathspec-file-nul"))
        self.assertEqual(force[:2], ("exec", "-i"))  # the path list is the standard input
        self.assertIn("GIT_LITERAL_PATHSPECS=1", exec_env(force))
        self.assertEqual(kw["data"], "".join("%s\0" % p for p in TREE_FILES))
        self.assertIn(".env.example\0", kw["data"])  # tracked upstream, matched by the new .gitignore
        self.assertIn("link\0", kw["data"])  # a symlink is a file of the tree; the directory member is not
        self.assertNotIn("crates\0", kw["data"])

    def test_every_git_add_and_commit_pins_the_config_that_could_rewrite_bytes(self):
        fake = FakeDocker()
        self.build(fake)
        seen = [args for args, _ in fake.calls if label(args) in ("git-add", "git-add-force", "git-commit")]
        self.assertEqual(len(seen), 3)
        for args in seen:
            tail = args[args.index(CID) + 1:]
            self.assertEqual(tail[:1 + len(PINNED)], ("git", *PINNED), args)
            self.assertIn("GIT_ATTR_NOSYSTEM=1", exec_env(args), args)

    def test_the_info_attributes_file_outranks_the_trees_gitattributes_before_any_add(self):
        fake = FakeDocker()
        self.build(fake)
        args = fake.args_of("git-attributes")
        i = args.index(CID)
        self.assertEqual(args[i + 1:i + 4], ("bash", "-o", "pipefail"))
        script = args[-1]
        self.assertIn("mkdir -p .git/info", script)
        self.assertIn("'* -text -ident -filter\\n'", script)
        self.assertIn("> .git/info/attributes", script)
        labels = fake.labels()
        self.assertLess(labels.index("git-attributes"), labels.index("git-add"))

    def test_git_commit_is_synthetic_with_a_fixed_identity_and_message(self):
        fake = FakeDocker()
        self.build(fake)
        args = fake.args_of("git-commit")
        tail = args[args.index(CID) + 1:]
        self.assertEqual(tail[:1 + len(PINNED) + 5], (
            "git", *PINNED, "-c", "user.name=sab", "-c", "user.email=sab@invalid", "commit"))
        msg = "synthetic base: %s %s (parent of PR %d merge)" % (REPO, UPSTREAM_BASE[:12], PR)
        self.assertEqual(tail[tail.index("-m") + 1], msg)

    def test_the_committed_index_is_compared_with_the_tree_after_the_commit(self):
        fake = FakeDocker()
        self.build(fake)
        labels = fake.labels()
        self.assertEqual(labels.index("git-commit") + 1, labels.index("ls-files"))
        args = fake.args_of("ls-files")
        self.assertEqual(args[args.index(CID) + 1:], ("git", "ls-files", "-s", "-z"))

    def test_a_file_missing_from_the_commit_raises_naming_it(self):
        entries = {p: e for p, e in TREE_ENTRIES.items() if p != ".env.example"}
        err = self.fails(FakeDocker(("ls-files", (0, ls_s(entries)))), "not the upstream tree")
        self.assertIn("missing from the commit: .env.example", str(err))
        self.assertNotIn("not in the upstream tree", str(err))
        self.assertNotIn("differs", str(err))

    def test_a_file_that_is_not_in_the_tree_raises_naming_at_most_ten(self):
        extra = ["extra%02d" % i for i in range(12)]
        entries = dict(TREE_ENTRIES, **{p: ("100644", blob_id(b"x")) for p in extra})
        err = self.fails(FakeDocker(("ls-files", (0, ls_s(entries)))), "not in the upstream tree")
        msg = str(err)
        for name in extra[:10]:
            self.assertIn(name, msg)
        for name in extra[10:]:
            self.assertNotIn(name, msg)
        self.assertIn("(+2 more)", msg)
        self.assertNotIn("missing from the commit", msg)

    def test_a_committed_blob_that_differs_fails_naming_the_path_and_both_ids(self):
        crlf = blob_id(b"[workspace]\r\nmembers = [\"crates/*\"]\r\n")
        entries = dict(TREE_ENTRIES, **{"Cargo.toml": ("100644", crlf)})
        err = self.fails(FakeDocker(("ls-files", (0, ls_s(entries)))), "not the upstream tree")
        msg = str(err)
        self.assertIn("differs: Cargo.toml", msg)
        self.assertIn(TREE_ENTRIES["Cargo.toml"][1][:12], msg)
        self.assertIn(crlf[:12], msg)
        self.assertNotIn("mode", msg)  # only the blob differs
        self.assertNotIn("missing from the commit", msg)

    def test_a_committed_mode_that_differs_fails_naming_the_path_and_both_modes(self):
        for path, mode in (("scripts/run.sh", "100644"), ("Cargo.toml", "100755"), ("link", "100644")):
            with self.subTest(path=path):
                entries = dict(TREE_ENTRIES, **{path: (mode, TREE_ENTRIES[path][1])})
                err = self.fails(FakeDocker(("ls-files", (0, ls_s(entries)))), "not the upstream tree")
                msg = str(err)
                self.assertIn("differs: " + path, msg)
                self.assertIn("mode %s -> %s" % (TREE_ENTRIES[path][0], mode), msg)
                self.assertNotIn("blob", msg)  # only the mode differs

    def test_at_most_ten_differing_paths_are_named(self):
        entries = {"f%02d" % i: ("100644", blob_id(b"x")) for i in range(12)}
        tar = write_tar(self.tmp / "many.tar", {p: b"y" for p in entries})
        fake = FakeDocker(("ls-files", (0, ls_s(entries))), files=sorted(entries))
        err = self.fails(fake, "not the upstream tree", tree_tar=tar)
        msg = str(err)
        self.assertIn("differs: f00", msg)
        self.assertIn("f09", msg)
        self.assertNotIn("f10", msg)
        self.assertIn("(+2 more)", msg)

    def test_an_unmerged_or_malformed_index_entry_raises(self):
        for bad in ("100644 %s 2\tCargo.toml\0" % ("a" * 40), "garbage\0", "100644 abc 0 Cargo.toml\0"):
            with self.subTest(bad=bad):
                self.fails(FakeDocker(("ls-files", (0, bad))), "git ls-files")

    def test_a_mismatch_stops_before_rev_parse_and_the_commit(self):
        fake = FakeDocker(("ls-files", (0, ls_s({"other": ("100644", blob_id(b""))}))))
        self.fails(fake, "not the upstream tree")
        self.assertNotIn("rev-parse", fake.labels())

    def test_base_commit_is_the_sha_the_container_reports(self):
        fake = FakeDocker(("rev-parse", (0, "%s\n" % ("ab" * 20))))
        self.assertEqual(self.build(fake)["base_commit"], "ab" * 20)

    def test_a_malformed_rev_parse_answer_raises(self):
        fake = FakeDocker(("rev-parse", (0, "fatal: not a git repository\n")))
        self.fails(fake, "rev-parse")

    # --- every step's own exit code decides ---

    def test_every_step_with_a_nonzero_exit_raises_stops_the_build_and_cleans_up(self):
        steps = {
            "tag-check": None, "base-id": None, "run": None, "clear-tracked": "id", "copy": "id", "extract": "id",
            "list-files": "id", "clear-dirs": "id", "touch": "id", "metadata": "id", "warm-build": "id",
            "git-attributes": "id", "git-add": "id", "git-add-force": "id", "git-commit": "id", "ls-files": "id",
            "rev-parse": "id",
            "docker-commit": "id",
        }
        for name, removal in steps.items():
            with self.subTest(step=name):
                fake = FakeDocker((name, (7, "step failed here")))
                err = self.fails(fake, "exit 7", commit_attempted=name == "docker-commit", removal=removal)
                self.assertIn("step failed here", str(err))
                body = [n for n in fake.labels() if n not in ("rm", "rmi")]
                self.assertEqual(body[-1], name)  # nothing ran after the failed step but the clean-up

    def test_a_step_timeout_is_a_build_error_and_the_container_is_removed(self):
        for name, removal in (("extract", "id"), ("warm-build", "id"), ("list-files", "id"), ("run", "name"),
                              ("tag-check", None), ("base-id", None), ("docker-commit", "id")):
            with self.subTest(step=name):
                fake = FakeDocker((name, subprocess.TimeoutExpired(cmd=["docker"], timeout=5)))
                self.fails(fake, "timed out", commit_attempted=name == "docker-commit", removal=removal)

    def test_a_missing_docker_binary_is_a_build_error(self):
        self.fails(FakeDocker(("tag-check", FileNotFoundError("docker"))), "docker", removal=None)

    def test_an_interrupt_still_removes_the_container_by_id(self):
        fake = FakeDocker(("warm-build", KeyboardInterrupt()))
        with self.assertRaises(KeyboardInterrupt):
            self.build(fake)
        self.assertEqual(fake.args_of("rm"), ("rm", "-f", "-v", CID))
        self.assertEqual(fake.labels()[-1], "rm")
        self.assertNotIn("docker-commit", fake.labels())
        self.assertEqual(self.evidence_json()["outcome"], "error")

    def test_exit_codes_are_read_from_the_completed_process_not_a_shell(self):
        fake = FakeDocker(("list-files", (0, "".join("./%s\0" % p for p in TREE_FILES + ["old.log"]))))
        self.build(fake)
        for args, kw in fake.calls:
            self.assertIs(kw.get("check"), False, args)  # read here, never raised by the docker wrapper
            self.assertIn("timeout", kw)
            joined = " ".join(args)
            self.assertNotIn("| tee", joined)
            self.assertNotIn("exit 0", joined)
        for name in ("delete-extras", "git-add-force"):
            self.assertEqual(fake.args_of(name)[1], "-i")

    # --- disk and arguments, all before docker ---

    def test_low_disk_refuses_before_any_docker_call(self):
        fake = FakeDocker()
        with self.assertRaisesRegex(fb.BuildError, "free"):
            self.build(fake, free=lambda: fb.MIN_FREE_GB - 0.01)
        self.assertEqual(fake.calls, [])

    def test_unreadable_or_nan_free_space_refuses(self):
        for bad in (float("nan"), None, "100", True):
            fake = FakeDocker()
            with self.assertRaises(fb.BuildError, msg=repr(bad)):
                self.build(fake, free=lambda bad=bad: bad)
            self.assertEqual(fake.calls, [])

    def test_exactly_the_minimum_is_enough(self):
        fake = FakeDocker()
        self.build(fake, free=lambda: float(fb.MIN_FREE_GB))
        self.assertEqual(fake.labels(), HAPPY_LABELS)

    def test_free_space_is_asked_once_and_before_the_first_docker_call(self):
        order = []
        fake = FakeDocker()

        def spy(*a, **k):
            order.append("docker")
            return fake(*a, **k)
        self.build(spy, free=lambda: order.append("free") or 500.0)
        self.assertEqual(order[0], "free")
        self.assertEqual(order.count("free"), 1)

    def test_bad_arguments_are_refused_before_any_docker_call(self):
        cases = [
            dict(workdir="/"), dict(workdir=""), dict(workdir="testbed"), dict(workdir="/a/../b"),
            dict(workdir="/a b"), dict(workdir="/a;b"), dict(workdir="/a'b"),
            dict(upstream_base="main"), dict(upstream_base="3d5fe04"), dict(upstream_base=UPSTREAM_BASE + "z"),
            dict(base_image=""), dict(base_image="-x"), dict(base_image="a b"), dict(repo="nope"), dict(pr=0),
        ]
        for over in cases:
            with self.subTest(**over):
                fake = FakeDocker()
                with self.assertRaises((ValueError, fb.BuildError)):
                    self.build(fake, **over)
                self.assertEqual(fake.calls, [])

    def test_a_missing_or_unusable_tree_tarball_is_refused_before_any_docker_call(self):
        garbage = self.tmp / "garbage.tar"
        garbage.write_bytes(b"not a tar archive" * 100)
        only_dirs = self.tmp / "dirs.tar"
        write_tar(only_dirs, {})
        unsafe = self.tmp / "unsafe.tar"
        write_tar(unsafe, {"../escape": b"x"})
        absolute = self.tmp / "absolute.tar"
        write_tar(absolute, {"/etc/passwd": b"x"})
        for tar, pattern in ((self.tmp / "absent.tar", "tree"), (garbage, "cannot read"),
                             (only_dirs, "no file"), (unsafe, "unsafe"), (absolute, "unsafe")):
            with self.subTest(tar=tar.name):
                fake = FakeDocker()
                with self.assertRaisesRegex(fb.BuildError, pattern):
                    self.build(fake, tree_tar=tar)
                self.assertEqual(fake.calls, [])

    def test_the_tarball_listing_is_files_and_symlinks_with_git_mode_and_blob_id(self):
        tar = self.tmp / "dotslash.tar"
        write_tar(tar, {"./a/b.rs": b"x\r\n", "c": ("symlink", "a/b.rs"), "run": ("exec", b"#!/bin/sh\n"),
                        "hl": ("hardlink", "a/b.rs")})
        entries, sha = fb._tree_files(tar)
        self.assertEqual(entries, {
            "a/b.rs": ("100644", blob_id(b"x\r\n")),  # CRLF kept: the id is of the bytes
            "c": ("120000", blob_id(b"a/b.rs")),  # a symlink's blob is its target text
            "run": ("100755", blob_id(b"#!/bin/sh\n")),
            "hl": ("100644", blob_id(b"x\r\n")),  # a hard link has the content of its target
        })
        self.assertEqual(sha, hashlib.sha256(tar.read_bytes()).hexdigest())

    def test_a_hard_link_to_a_path_that_is_not_in_the_tarball_is_refused(self):
        tar = write_tar(self.tmp / "dangling.tar", {"hl": ("hardlink", "nowhere")})
        with self.assertRaisesRegex(fb.BuildError, "hard link"):
            fb._tree_files(tar)

    @unittest.skipUnless(shutil.which("git"), "git is not installed")
    def test_real_git_global_autocrlf_and_an_eol_attribute_rewrite_bytes_unless_pinned(self):
        # the one place real git runs, on scratch repositories under the test's temp dir, with a
        # global core.autocrlf=true: the hazard is real, and the pinned sequence defeats it
        tree = {"crlf.txt": b"a\r\nb\r\n", "lf.txt": b"x\n", "run.sh": ("exec", b"#!/bin/sh\n")}
        with_attrs = dict(tree, **{".gitattributes": b"*.txt text eol=crlf\n"})
        cfg = self.tmp / "hazard.gitconfig"
        cfg.write_text("[core]\n\tautocrlf = true\n")
        env = {"PATH": os.environ["PATH"], "HOME": str(self.tmp), "GIT_CONFIG_GLOBAL": str(cfg),
               "GIT_CONFIG_NOSYSTEM": "1", "LC_ALL": "C"}

        def git(root, *args):
            return subprocess.run(["git", *args], cwd=root, env=env, check=True, capture_output=True,
                                  text=True).stdout

        def repo(name, files):
            root = self.tmp / name
            root.mkdir()
            git(root, "init", "-q")
            for path, content in files.items():
                (root / path).write_bytes(content[1] if isinstance(content, tuple) else content)
                if isinstance(content, tuple):
                    (root / path).chmod(0o755)
            return root

        def index(root):
            found = {}
            for entry in git(root, "ls-files", "-s", "-z").split("\0"):
                if entry:
                    meta, path = entry.split("\t", 1)
                    found[path] = tuple(meta.split(" ")[:2])
            return found

        want = {p: tree_entry(c) for p, c in tree.items()}
        want_attrs = {p: tree_entry(c) for p, c in with_attrs.items()}

        # 1. the hazard is real: a plain `git add -A` stores CRLF text as LF
        a = repo("plain-attrs", with_attrs)
        git(a, "add", "-A")
        self.assertNotEqual(index(a)["crlf.txt"][1], want["crlf.txt"][1])
        self.assertEqual(index(a)["crlf.txt"][1], blob_id(b"a\nb\n"))
        b = repo("plain", tree)
        git(b, "add", "-A")
        self.assertNotEqual(index(b)["crlf.txt"][1], want["crlf.txt"][1])  # global autocrlf alone is enough

        # 2. the pinned flags alone defeat the global setting: the very argv build_image runs
        fake = FakeDocker()
        self.build(fake)
        c = repo("pinned", tree)
        done_ = local_shell(c, env)(fake.args_of("git-add"), fake.kw_of("git-add"))
        self.assertEqual(done_.returncode, 0, done_.stdout)
        self.assertEqual(index(c), want)

        # 3. the whole sequence, with an eol attribute in the tree too: build_image's own compare
        # reads real `git ls-files -s -z` of the synthetic commit and passes
        tar = write_tar(self.tmp / "with-attrs.tar", with_attrs)
        d = repo("full", with_attrs)
        shell = local_shell(d, env)
        fake = FakeDocker(("git-attributes", shell), ("git-add", shell), ("git-add-force", shell),
                          ("git-commit", shell), ("ls-files", shell), ("rev-parse", shell),
                          files=sorted(with_attrs))
        got = self.build(fake, tree_tar=tar)
        self.assertRegex(got["base_commit"], r"^[0-9a-f]{40}$")
        self.assertEqual(index(d), want_attrs)

    # --- evidence and removal ---

    def test_evidence_of_a_good_build(self):
        fake = FakeDocker()
        got = self.build(fake)
        ev = self.evidence_json()
        self.assertEqual(ev["outcome"], "ok")
        self.assertIsNone(ev["error"])
        self.assertEqual(ev["image"], TAG)
        self.assertEqual(ev["container"], CONTAINER)
        self.assertEqual(ev["container_id"], CID)
        self.assertFalse(ev["container_removed_by_name"])
        self.assertEqual(ev["base_image"], BASE_IMAGE)
        self.assertEqual(ev["base_image_id"], BASE_ID)
        self.assertEqual(ev["tree_tar_sha256"], got["tree_tar_sha256"])
        self.assertEqual(ev["tree_files"], len(TREE))
        self.assertEqual(ev["leftover_files_deleted"], 0)
        self.assertEqual(ev["upstream_base"], UPSTREAM_BASE)
        self.assertEqual(ev["builder_version"], fb.BUILDER_VERSION)
        self.assertEqual(ev["base_commit"], SYN_SHA)
        self.assertEqual(ev["image_id"], IMAGE_ID)
        self.assertEqual(ev["compiling_lines"], 2)
        self.assertTrue(ev["container_removed"])
        self.assertEqual(ev["laps_seconds"], got["laps_seconds"])
        names = [n for n in HAPPY_LABELS if n != "rm"]
        names[names.index("tag-check", 1)] = "tag-recheck"  # the re-inspect before `docker commit`
        self.assertEqual([s["name"] for s in ev["steps"]], names)
        self.assertTrue(all(s["returncode"] == (1 if s["name"] in ("tag-check", "tag-recheck") else 0)
                            for s in ev["steps"]))

    def test_evidence_of_a_failed_build_names_the_error_and_the_failed_step(self):
        fake = FakeDocker(("warm-build", (101, "error: boom\n")))
        self.fails(fake, "boom")
        ev = self.evidence_json()
        self.assertIn("BuildError", ev["error"])
        self.assertIn("boom", ev["error"])
        last = ev["steps"][-1]
        self.assertEqual((last["name"], last["returncode"]), ("warm-build", 101))
        self.assertNotIn("image_id", ev)

    def test_failed_removal_after_a_good_build_is_recorded_and_not_raised(self):
        fake = FakeDocker(("rm", (1, "Error response from daemon: removal of container is already in progress")))
        got = self.build(fake)
        self.assertFalse(got["container_removed"])
        ev = self.evidence_json()
        self.assertEqual(ev["outcome"], "ok")
        self.assertFalse(ev["container_removed"])
        self.assertIn("already in progress", ev["removal_error"])

    def test_failed_removal_never_replaces_the_original_error(self):
        for rm in ((1, "cannot remove: device busy"), subprocess.TimeoutExpired(cmd=["docker"], timeout=300),
                   FileNotFoundError("docker")):
            with self.subTest(rm=repr(rm)):
                fake = FakeDocker(("warm-build", (101, "error: the real cause\n")), ("rm", rm))
                with self.assertRaisesRegex(fb.BuildError, "the real cause"):
                    self.build(fake)
                self.assertNotIn("docker-commit", fake.labels())
                ev = self.evidence_json()
                self.assertEqual(ev["outcome"], "error")
                self.assertIn("the real cause", ev["error"])
                self.assertFalse(ev["container_removed"])
                self.assertTrue(ev["removal_error"])

    def test_an_interrupt_while_removing_is_recorded_then_reraised(self):
        fake = FakeDocker(("rm", KeyboardInterrupt()))
        with self.assertRaises(KeyboardInterrupt):
            self.build(fake)
        ev = self.evidence_json()  # written before the interrupt left
        self.assertFalse(ev["container_removed"])
        self.assertIn("KeyboardInterrupt", ev["removal_error"])
        self.assertEqual(ev["outcome"], "ok")

    def test_an_interrupt_while_removing_still_records_the_build_error(self):
        fake = FakeDocker(("warm-build", (101, "error: the real cause\n")), ("rm", KeyboardInterrupt()))
        with self.assertRaises(KeyboardInterrupt):
            self.build(fake)
        ev = self.evidence_json()
        self.assertIn("the real cause", ev["error"])
        self.assertIn("KeyboardInterrupt", ev["removal_error"])

    def test_evidence_dir_is_created_when_missing(self):
        deep = self.tmp / "a" / "b"
        self.build(FakeDocker(), evidence_dir=deep)
        self.assertTrue((deep / "build.log").is_file())
        self.assertTrue((deep / "build-evidence.json").is_file())

    def test_only_study_objects_are_named_and_nothing_else_is_deleted(self):
        fake = FakeDocker()
        self.build(fake)
        for args, _ in fake.calls:
            for a in args:
                self.assertNotRegex(a, r"codescout-|pbr27-|rae-")
            self.assertNotIn("volume", args)
            self.assertNotIn("prune", args)
            self.assertNotEqual(args[0], "rmi")
        self.assertEqual([args for args, _ in fake.calls if args[0] == "rm"], [("rm", "-f", "-v", CID)])


CTL = "sab-fresh-ctl-rust-lang__rust-analyzer-23096"
CTL_IMAGE = "sab-fresh-rust-lang__rust-analyzer-23096:b%s-gate" % fb.BUILDER_VERSION
GATE_ID = "sha256:" + "c" * 64  # what `docker image inspect` says about CTL_IMAGE
CTL_BASE = "05278f431f7a71add03f39f081af38f03ac64baf"
CTL_TEST_PATCH = "diff --git a/crates/x/tests/t.rs b/crates/x/tests/t.rs\n+test\r\n"
CTL_GOLD_PATCH = "diff --git a/crates/x/src/lib.rs b/crates/x/src/lib.rs\n+fix\n"
CTL_RECORD = {
    "instance_id": "rust-lang__rust-analyzer-23096",
    "docker_image": CTL_IMAGE, "base_commit": CTL_BASE, "patch": CTL_GOLD_PATCH, "test_patch": CTL_TEST_PATCH,
    "test_cmds": ["cargo test --no-fail-fast 2>&1 | tee reports/t.log"], "rebuild_cmds": ["cargo test --no-run"],
    "print_cmds": ["cat reports/t.log"], "log_parser": "def parser(log):\n    return {}\n",
}
HASH = "target/debug/deps/hir_ty-0123456789abcdef::tests::"  # test names embed the binary hash
T_A, T_B, T_C, T_D, T_E = (HASH + n for n in "abcde")
# a: fails at base and passes with gold (F2P); b, c: pass both (P2P); d: fails both; e: passes at base only
BASE_STATUS = {T_A: "fail", T_B: "pass", T_C: "pass", T_D: "fail", T_E: "pass"}
GOLD_STATUS = {T_A: "pass", T_B: "pass", T_C: "pass", T_D: "fail", T_E: "fail"}
MISMATCH = "tests differ between two gold runs"
DOCKER_OK = ["gate-image-id", "run", "rm"]  # the docker calls of a gate that ran


class ControlFreshTest(unittest.TestCase):
    """`control_fresh` with the `multilang_control` helpers and `run_parser` replaced by fakes that
    record their calls; the real `control_verdict` decides. No docker, no real parser."""

    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.tmp = Path(tmp.name)
        self.evidence = self.tmp / "evidence"
        self.calls = []        # (helper, detail...) in call order
        self.containers = []   # the container every helper was given
        self.raises = {}       # call index -> exception raised by that helper call
        self.timeouts = []
        self.statuses = {"base": BASE_STATUS, "gold": GOLD_STATUS, "gold-repeat": GOLD_STATUS,
                         "gold-repeat-2": GOLD_STATUS}
        self.parse_calls = 0
        self.parse_raises = {}  # parse call index -> exception
        # isolation: nothing may reach the real docker. `pilot` is one module object, so this covers
        # `fresh_build.pilot.docker` and the one `multilang_control` calls; the check is in a cleanup,
        # because the code under test turns an exception raised in a step into a verdict
        self.real_docker_calls = []
        self.addCleanup(lambda: self.assertEqual(self.real_docker_calls, [], "the real docker was called"))
        self.start_patch(mock.patch.object(fb.pilot, "docker", side_effect=self.no_real_docker))
        for name, impl in (("detect_workdir", self.f_detect), ("reset_to_base", self.f_reset),
                           ("_apply", self.f_apply), ("rebuild", self.f_rebuild), ("run_tests", self.f_run_tests),
                           ("_fresh_reports", self.f_reports)):
            self.start_patch(mock.patch.object(fb.mc, name, autospec=True, side_effect=impl))
        self.start_patch(mock.patch.object(fb.grade, "run_parser", autospec=True, side_effect=self.f_parse))

    def start_patch(self, patcher):
        patcher.start()
        self.addCleanup(patcher.stop)

    def no_real_docker(self, *args, **kw):
        self.real_docker_calls.append(args)
        raise AssertionError("the real docker was called: %r" % (args,))

    # --- the fakes ---

    def hit(self, container, *detail):
        index = len(self.calls)
        self.calls.append(detail)
        self.containers.append(container)
        if index in self.raises:
            raise self.raises[index]

    def f_detect(self, container):
        self.hit(container, "detect_workdir")
        return "/testbed"

    def f_reset(self, container, workdir, base_commit):
        self.assertEqual(workdir, "/testbed")
        self.hit(container, "reset", base_commit)
        return {"dirty_before": []}

    def f_apply(self, container, workdir, patch):
        self.hit(container, "apply", {CTL_TEST_PATCH: "test", CTL_GOLD_PATCH: "gold"}.get(patch, "?"))

    def f_reports(self, container, workdir):
        self.hit(container, "fresh_reports")

    def f_rebuild(self, container, workdir, record, timeout=3600, *, steps=None, state="base"):
        self.hit(container, "rebuild", state)
        self.timeouts.append(timeout)
        steps.append({"name": state + ":rebuild", "returncode": 0, "tail": ""})

    def f_run_tests(self, container, workdir, record, timeout=3600, *, steps=None, state="base"):
        self.hit(container, "run_tests", state)
        self.timeouts.append(timeout)
        steps.append({"name": state + ":test[0]", "returncode": 0, "tail": ""})
        # a log with CRLF and a lone CR in it: it must reach the evidence file byte for byte
        return "noise\r\nlone\rcr\r\n" + json.dumps({"state": state, "statuses": self.statuses[state]}) + "\r\n"

    def f_parse(self, parser_src, log, *args, **kw):
        self.assertEqual(parser_src, CTL_RECORD["log_parser"])
        index = self.parse_calls
        self.parse_calls += 1
        if index in self.parse_raises:
            raise self.parse_raises[index]
        return json.loads(log[log.index("{"):])["statuses"]

    # --- helpers of the tests ---

    def control(self, docker=None, **kw):
        args = dict(container=CTL, evidence_dir=self.evidence, docker=docker or FakeDocker())
        args.update(kw)
        record = args.pop("record", CTL_RECORD)
        return fb.control_fresh(record, **args)

    HAPPY_CALLS = [
        ("detect_workdir",), ("reset", CTL_BASE), ("apply", "test"), ("rebuild", "base"), ("run_tests", "base"),
        ("reset", CTL_BASE), ("apply", "test"), ("apply", "gold"), ("rebuild", "gold"), ("run_tests", "gold"),
        ("fresh_reports",), ("run_tests", "gold-repeat"),
    ]

    def flaky(self, *tests, status="fail", repeat2=None):
        """The first gold repeat says `status` (a real flake) for `tests`; the second one says `repeat2`'s
        statuses for them (default pass). A `repeat2` status of None means absent."""
        self.statuses["gold-repeat"] = dict(GOLD_STATUS, **{t: status for t in tests})
        self.second_repeat(repeat2)

    def lose(self, *tests, repeat2=None):
        """The first gold repeat has NO status for `tests` (the parser lost their lines, as in the pilot);
        the second one says `repeat2`'s statuses for them (default pass). None means absent."""
        self.statuses["gold-repeat"] = {t: s for t, s in GOLD_STATUS.items() if t not in tests}
        self.second_repeat(repeat2)

    def second_repeat(self, repeat2):
        second = dict(GOLD_STATUS)
        for test, status in (repeat2 or {}).items():
            if status is None:
                del second[test]
            else:
                second[test] = status
        self.statuses["gold-repeat-2"] = second

    # --- the derivation and the verdict ---

    def test_happy_path_derives_the_sets_and_the_gate_is_ok(self):
        fake = FakeDocker()
        got = self.control(fake)
        self.assertEqual(got["f2p"], [T_A])
        self.assertEqual(got["p2p"], sorted([T_B, T_C]))  # e passes at base only: in neither set
        self.assertEqual(got["verdict"]["ok"], True)
        self.assertEqual(got["verdict"]["reasons"], [])
        self.assertEqual(got["verdict"]["expected"], 3)
        self.assertNotIn("error", got)
        self.assertEqual(got["base"], {"fail": 2, "pass": 3})
        self.assertEqual(got["gold"], {"pass": 3, "fail": 2})
        self.assertEqual(got["gold_repeat"], {"pass": 3, "fail": 2})
        self.assertEqual(got["base_status_of_f2p"], {T_A: "fail"})
        self.assertTrue(got["container_removed"])
        self.assertEqual(got["container_id"], CID)
        self.assertEqual(set(got["laps_seconds"]), {"start_and_apply_test", "base_rebuild", "base_tests",
                                                    "gold_rebuild", "gold_tests", "gold_repeat_tests"})
        self.assertIsInstance(got["wall_seconds"], float)
        json.dumps(got, allow_nan=False)  # it goes into control.json as it is

    def test_the_states_run_in_order_each_from_a_reset_to_base(self):
        self.control()
        self.assertEqual(self.calls, self.HAPPY_CALLS)
        self.assertEqual(set(self.containers), {CID})  # every helper addresses the container by its ID
        self.assertEqual(self.timeouts, [5400] * 5)

    def test_the_timeout_is_passed_to_every_rebuild_and_test_step(self):
        self.control(timeout=77)
        self.assertEqual(self.timeouts, [77] * 5)

    def test_the_verdict_is_computed_from_the_derived_sets_and_the_gold_repeat(self):
        seen = []

        def verdict(base, gold, f2p, p2p, gold_repeat=None):
            seen.append((base, gold, f2p, p2p, gold_repeat))
            return {"ok": True, "reasons": []}
        with mock.patch.object(fb.grade, "control_verdict", verdict):
            got = self.control()
        self.assertEqual(seen, [(BASE_STATUS, GOLD_STATUS, [T_A], sorted([T_B, T_C]), GOLD_STATUS)])
        self.assertIs(got["verdict"]["ok"], True)

    def test_the_logs_are_written_byte_for_byte(self):
        got = self.control()
        for name, state in (("base.log", "base"), ("gold.log", "gold"), ("gold-repeat.log", "gold-repeat")):
            data = (self.evidence / name).read_bytes()
            self.assertTrue(data.startswith(b"noise\r\nlone\rcr\r\n{"), name)
            self.assertTrue(data.endswith(b"}\r\n"), name)
            self.assertIn(state.encode(), data)
        self.assertFalse((self.evidence / "gold-repeat-2.log").exists())
        self.assertEqual(got["verdict"]["ok"], True)

    def test_an_empty_fail_to_pass_set_is_not_ok(self):
        self.statuses["base"] = GOLD_STATUS  # nothing changes between the states
        got = self.control()
        self.assertEqual(got["f2p"], [])
        self.assertIs(got["verdict"]["ok"], False)
        self.assertEqual(got["verdict"]["reasons"][0], "no fail-to-pass test")
        self.assertNotIn("error", got["verdict"])  # a real rejection, not an infrastructure error

    def test_an_empty_fail_to_pass_set_is_refused_whatever_control_verdict_says(self):
        self.statuses["base"] = GOLD_STATUS
        with mock.patch.object(fb.grade, "control_verdict", lambda *a, **k: {"ok": True, "reasons": []}):
            got = self.control()
        self.assertIs(got["verdict"]["ok"], False)
        self.assertEqual(got["verdict"]["reasons"], ["no fail-to-pass test"])

    # --- the tolerant repeat ---

    def test_one_mismatch_resolved_by_the_second_repeat_is_ok_and_recorded(self):
        self.lose(T_B)  # absent from the first repeat, as in the pilot
        got = self.control()
        self.assertEqual(self.calls[-2:], [("fresh_reports",), ("run_tests", "gold-repeat-2")])
        self.assertEqual(self.timeouts, [5400] * 6)
        self.assertIs(got["verdict"]["ok"], True)
        self.assertEqual(got["verdict"]["reasons"], [])
        self.assertEqual(got["verdict"]["tolerated_repeat_mismatch"], [T_B])
        self.assertEqual(got["verdict"]["strict_repeat_mismatch_count"], 1)  # beside the original count
        self.assertEqual(got["verdict"]["gold_repeat_mismatch"]["count"], 1)
        tol = got["tolerant_repeat"]
        self.assertEqual(tol["tests"], {T_B: {"gold": "pass", "gold_repeat": "absent", "gold_repeat_2": "pass"}})
        self.assertEqual(tol["accepted"], [T_B])
        self.assertEqual(tol["gold_repeat_2"], {"pass": 3, "fail": 2})
        self.assertEqual(got["verdict_strict"]["reasons"], [MISMATCH])  # what the strict rule said
        self.assertIs(got["verdict_strict"]["ok"], False)
        self.assertIn("gold_repeat_2_tests", got["laps_seconds"])
        self.assertTrue((self.evidence / "gold-repeat-2.log").is_file())

    def test_a_mismatch_in_the_fail_to_pass_set_is_tolerated_the_same_way(self):
        self.lose(T_A)
        got = self.control()
        self.assertIs(got["verdict"]["ok"], True)
        self.assertEqual(got["tolerant_repeat"]["tests"][T_A]["gold_repeat"], "absent")

    def test_three_mismatches_all_resolved_are_ok(self):
        self.lose(T_A, T_B, T_C)
        got = self.control()
        self.assertIs(got["verdict"]["ok"], True)
        self.assertEqual(got["verdict"]["tolerated_repeat_mismatch"], sorted([T_A, T_B, T_C]))
        self.assertEqual(got["verdict"]["strict_repeat_mismatch_count"], 3)

    def test_a_mismatch_that_persists_is_not_ok(self):
        self.lose(T_B, repeat2={T_B: "fail"})
        got = self.control()
        self.assertEqual(self.calls[-1], ("run_tests", "gold-repeat-2"))
        self.assertIs(got["verdict"]["ok"], False)
        self.assertEqual(got["verdict"]["reasons"], [MISMATCH])
        self.assertNotIn("tolerated_repeat_mismatch", got["verdict"])
        self.assertNotIn("strict_repeat_mismatch_count", got["verdict"])
        self.assertEqual(got["tolerant_repeat"]["accepted"], [])
        self.assertEqual(got["tolerant_repeat"]["tests"][T_B]["gold_repeat_2"], "fail")
        self.assertTrue((self.evidence / "gold-repeat-2.log").is_file())

    def test_a_second_repeat_that_does_not_say_pass_is_not_accepted(self):
        for status in ("fail", "skip", "unknown", None):  # None: the test is absent from the second repeat too
            with self.subTest(status=status):
                self.calls.clear()
                self.lose(T_B, repeat2={T_B: status})
                got = self.control()
                self.assertEqual(self.calls[-1], ("run_tests", "gold-repeat-2"))
                self.assertIs(got["verdict"]["ok"], False)
                self.assertEqual(got["verdict"]["reasons"], [MISMATCH])

    def test_one_unresolved_mismatch_among_three_fails_them_all(self):
        self.lose(T_A, T_B, T_C, repeat2={T_C: "fail"})
        got = self.control()
        self.assertIs(got["verdict"]["ok"], False)
        self.assertEqual(got["tolerant_repeat"]["accepted"], sorted([T_A, T_B]))
        self.assertNotIn("tolerated_repeat_mismatch", got["verdict"])

    def test_a_real_flake_that_failed_in_the_first_repeat_is_never_accepted(self):
        # fail then pass is noise the arms would be graded on, not the parser losing a line: the task is
        # rejected, for an F2P and a P2P test alike, and whatever other status the first repeat gave
        for test in (T_A, T_B):
            for status in ("fail", "skip", "unknown"):
                with self.subTest(test=test, first_repeat=status):
                    self.calls.clear()
                    self.flaky(test, status=status)  # the second repeat would say pass
                    got = self.control()
                    self.assertNotIn(("run_tests", "gold-repeat-2"), self.calls)  # nothing to tolerate: no extra run
                    self.assertIs(got["verdict"]["ok"], False)
                    self.assertEqual(got["verdict"]["reasons"], [MISMATCH])
                    self.assertNotIn("tolerant_repeat", got)
                    self.assertNotIn("tolerated_repeat_mismatch", got["verdict"])

    def test_one_flake_among_lost_lines_rejects_the_task(self):
        self.statuses["gold-repeat"] = {t: s for t, s in GOLD_STATUS.items() if t != T_B}
        self.statuses["gold-repeat"][T_C] = "fail"  # b was lost (fine), c really failed
        self.second_repeat(None)
        got = self.control()
        self.assertNotIn(("run_tests", "gold-repeat-2"), self.calls)
        self.assertIs(got["verdict"]["ok"], False)
        self.assertEqual(got["verdict"]["reasons"], [MISMATCH])

    def test_four_mismatches_skip_the_tolerant_run_and_fail(self):
        self.statuses["base"] = dict(BASE_STATUS, **{T_D: "pass"})
        self.statuses["gold"] = dict(GOLD_STATUS, **{T_D: "pass"})
        # four tests lost from the first repeat: each would be acceptable, but there are too many
        self.statuses["gold-repeat"] = {t: s for t, s in self.statuses["gold"].items()
                                        if t not in (T_A, T_B, T_C, T_D)}
        got = self.control()
        self.assertNotIn(("run_tests", "gold-repeat-2"), self.calls)
        self.assertEqual(self.calls[-1], ("run_tests", "gold-repeat"))
        self.assertIs(got["verdict"]["ok"], False)
        self.assertEqual(got["verdict"]["reasons"], [MISMATCH])
        self.assertNotIn("tolerant_repeat", got)
        self.assertFalse((self.evidence / "gold-repeat-2.log").exists())

    def test_a_mismatch_beside_another_reason_is_not_tolerated(self):
        self.statuses["base"] = GOLD_STATUS  # no fail-to-pass test
        self.lose(T_B)
        got = self.control()
        self.assertNotIn(("run_tests", "gold-repeat-2"), self.calls)
        self.assertIs(got["verdict"]["ok"], False)
        self.assertEqual(got["verdict"]["reasons"][0], "no fail-to-pass test")
        self.assertIn(MISMATCH, got["verdict"]["reasons"])

        self.calls.clear()
        self.statuses["base"] = BASE_STATUS
        verdict = {"ok": False, "reasons": [MISMATCH, "something else"], "gold_repeat_mismatch": {"count": 1}}
        with mock.patch.object(fb.grade, "control_verdict", lambda *a, **k: dict(verdict)):
            got = self.control()
        self.assertNotIn(("run_tests", "gold-repeat-2"), self.calls)
        self.assertEqual(got["verdict"]["reasons"], [MISMATCH, "something else"])

    def test_a_mismatching_test_must_pass_in_gold_too(self):
        # an expected test that does not pass in gold cannot come from derive_sets; the rule is checked anyway
        self.lose(T_D, repeat2={T_D: "pass"})  # d: fail in gold, absent then pass in the repeats
        with mock.patch.object(fb, "derive_sets", lambda base, gold: ([T_A], [T_B, T_C, T_D])), \
                mock.patch.object(fb.grade, "control_verdict",
                                  lambda *a, **k: {"ok": False, "reasons": [MISMATCH],
                                                   "gold_repeat_mismatch": {"count": 1, "names": [T_D]}}):
            got = self.control()
        self.assertIs(got["verdict"]["ok"], False)
        self.assertEqual(got["tolerant_repeat"]["accepted"], [])

    def test_a_verdict_whose_mismatch_count_disagrees_with_the_statuses_is_not_tolerated(self):
        self.lose(T_B)
        lie = {"ok": False, "reasons": [MISMATCH], "gold_repeat_mismatch": {"count": 2, "names": [T_B, T_C]}}
        with mock.patch.object(fb.grade, "control_verdict", lambda *a, **k: dict(lie)):
            got = self.control()
        self.assertNotIn(("run_tests", "gold-repeat-2"), self.calls)
        self.assertIs(got["verdict"]["ok"], False)

    def test_a_failing_second_repeat_is_an_error_verdict_that_keeps_the_strict_one(self):
        self.lose(T_B)
        self.raises[len(self.HAPPY_CALLS) + 1] = fb.mc.ControlError("boom in the second repeat")
        got = self.control()
        self.assertEqual(got["verdict"]["error"], True)
        self.assertIs(got["verdict"]["ok"], False)
        self.assertEqual(got["verdict_strict"]["reasons"], [MISMATCH])
        self.assertTrue(got["container_removed"])

    # --- a step error is a verdict, not an exception, and the container is removed ---

    def assert_error_verdict(self, got, fake, text=None):
        self.assertEqual(set(got["verdict"]), {"ok", "error", "reasons"})
        self.assertIs(got["verdict"]["ok"], False)
        self.assertIs(got["verdict"]["error"], True)
        self.assertEqual(len(got["verdict"]["reasons"]), 1)
        self.assertTrue(got["verdict"]["reasons"][0].startswith("control error: "), got["verdict"])
        if text is not None:
            self.assertIn(text, got["verdict"]["reasons"][0])
        self.assertIn("error", got)
        self.assertIsNone(got["f2p"])
        self.assertIsNone(got["p2p"])
        self.assertEqual(fake.labels(), DOCKER_OK)
        self.assertEqual(fake.args_of("rm"), ("rm", "-f", "-v", CID))
        self.assertTrue(got["container_removed"])

    def test_a_control_error_in_any_state_yields_an_error_verdict_and_removes_the_container(self):
        for index in range(len(self.HAPPY_CALLS)):
            with self.subTest(call=self.HAPPY_CALLS[index], index=index):
                self.calls.clear()
                self.raises = {index: fb.mc.ControlError("step %d failed" % index)}
                fake = FakeDocker()
                got = self.control(fake)
                self.assert_error_verdict(got, fake, "step %d failed" % index)
                self.assertEqual(len(self.calls), index + 1)  # nothing ran after the failed step

    def test_other_step_failures_are_error_verdicts_too(self):
        for exc in (subprocess.TimeoutExpired(cmd=["docker"], timeout=5), RuntimeError("docker exec failed"),
                    ValueError("bad bytes"), UnicodeDecodeError("utf-8", b"\xff", 0, 1, "invalid"),
                    OSError("disk full")):
            with self.subTest(exc=type(exc).__name__):
                self.calls.clear()
                self.raises = {4: exc}
                fake = FakeDocker()
                self.assert_error_verdict(self.control(fake), fake)

    def test_a_parser_error_in_any_state_is_an_error_verdict(self):
        for index in range(3):
            with self.subTest(parse=index):
                self.calls.clear()
                self.parse_calls = 0
                self.parse_raises = {index: fb.grade.ParserError("log parser timed out")}
                fake = FakeDocker()
                self.assert_error_verdict(self.control(fake), fake, "log parser timed out")

    def test_the_error_names_the_exception_type_and_keeps_its_tail(self):
        self.raises = {0: fb.mc.ControlError("x" * 1000 + "THE-END")}
        got = self.control()
        self.assertTrue(got["error"].startswith("ControlError: "))
        self.assertTrue(got["error"].endswith("THE-END"))
        self.assertLessEqual(len(got["verdict"]["reasons"][0]), 400)

    def test_an_interrupt_removes_the_container_and_propagates(self):
        self.raises = {4: KeyboardInterrupt()}
        fake = FakeDocker()
        with self.assertRaises(KeyboardInterrupt):
            self.control(fake)
        self.assertEqual(fake.labels(), DOCKER_OK)
        self.assertEqual(fake.args_of("rm"), ("rm", "-f", "-v", CID))

    def test_a_failed_removal_is_recorded_and_never_replaces_the_verdict(self):
        for rm in ((1, "Error response from daemon: removal of container is already in progress"),
                   subprocess.TimeoutExpired(cmd=["docker"], timeout=600), FileNotFoundError("docker")):
            with self.subTest(rm=repr(rm)):
                got = self.control(FakeDocker(("rm", rm)))
                self.assertIs(got["verdict"]["ok"], True)
                self.assertIs(got["container_removed"], False)
                self.assertTrue(got["removal_error"])
        self.calls.clear()
        self.raises = {4: fb.mc.ControlError("the real cause")}
        got = self.control(FakeDocker(("rm", (1, "device busy"))))
        self.assertIn("the real cause", got["verdict"]["reasons"][0])
        self.assertIs(got["container_removed"], False)

    # --- the container: its flags, its name and who owns it ---

    def test_the_container_is_started_with_the_frozen_gate_flags_and_8g_from_the_image_id(self):
        fake = FakeDocker()
        got = self.control(fake)
        run = fake.args_of("run")
        self.assertEqual(run[4], "--cidfile")
        cidfile = Path(run[5])
        self.assertEqual((cidfile.parent.parent, cidfile.name), (self.evidence.resolve(), "run.cid"))
        self.assertEqual(run[:4] + run[6:], (
            "run", "-d", "--name", CTL,
            "--network", "none", "--cpus", "4", "--memory", "8g", "--memory-swap", "8g", "--pids-limit", "4096",
            "--security-opt", "no-new-privileges", GATE_ID, "sleep", "infinity"))
        self.assertEqual(fake.all_args_of("gate-image-id"), [("image", "inspect", "--format", "{{.Id}}", CTL_IMAGE)])
        self.assertEqual(fake.labels(), DOCKER_OK)
        self.assertEqual(fake.args_of("rm"), ("rm", "-f", "-v", CID))
        self.assertEqual((got["image"], got["image_id"]), (CTL_IMAGE, GATE_ID))  # reference and ID are both recorded

    def test_a_gate_image_that_cannot_be_inspected_is_an_error_verdict_and_nothing_starts(self):
        for resp in ((1, "Error: No such image"), (0, "garbage\n"), (0, ""), (0, "sha256:abc\n"),
                     subprocess.TimeoutExpired(cmd=["docker"], timeout=120), FileNotFoundError("docker")):
            with self.subTest(resp=repr(resp)):
                fake = FakeDocker(("gate-image-id", resp))
                got = self.control(fake)
                self.assertEqual(fake.labels(), ["gate-image-id"])  # no container was created: nothing to remove
                self.assertEqual(set(got["verdict"]), {"ok", "error", "reasons"})
                self.assertIs(got["verdict"]["error"], True)
                self.assertEqual(self.calls, [])
                self.assertIsNone(got["container_removed"])

    def test_two_runs_sharing_an_evidence_dir_use_different_cidfiles_and_clean_up(self):
        paths = []
        for _ in range(2):
            fake = FakeDocker()
            self.control(fake)
            run = fake.args_of("run")
            paths.append(Path(run[run.index("--cidfile") + 1]))
        self.assertNotEqual(paths[0], paths[1])
        for path in paths:
            self.assertEqual((path.parent.parent, path.name), (self.evidence.resolve(), "run.cid"))
            self.assertFalse(path.parent.exists(), "the per-call directory is cleaned up")

    def test_the_memory_default_is_8g_and_a_parameter(self):
        self.assertEqual(fb.CONTROL_MEMORY, "8g")
        fake = FakeDocker()
        self.control(fake, memory="12g")
        run = fake.args_of("run")
        self.assertEqual(run[run.index("--memory") + 1], "12g")
        self.assertEqual(run[run.index("--memory-swap") + 1], "12g")

    def test_a_bad_memory_value_raises_before_docker(self):
        for bad in ("8", "8gb", "", "0g", "-1g", "8 g", None, 8, "8g --privileged", "g"):
            with self.subTest(bad=bad):
                fake = FakeDocker()
                with self.assertRaises(ValueError):
                    self.control(fake, memory=bad)
                self.assertEqual(fake.calls, [])

    def test_the_run_and_the_removal_use_the_injected_docker_with_a_timeout_and_no_check(self):
        fake = FakeDocker()
        self.control(fake)
        for args, kw in fake.calls:
            self.assertIs(kw.get("check"), False, args)
            self.assertIn("timeout", kw)

    def test_a_container_name_without_the_prefix_raises_before_any_docker_call(self):
        for bad in ("sab-ml-ctl-x", "sab-fresh-build-x", "codescout-x", "pbr27-x", "", "x", "sab-fresh-ctl-",
                    "sab-fresh-ctl-a b", "sab-fresh-ctl-a;rm", "sab-fresh-ctl-x\n", None, 5):
            with self.subTest(bad=bad):
                fake = FakeDocker()
                with self.assertRaises(ValueError):
                    self.control(fake, container=bad)
                self.assertEqual(fake.calls, [])
                self.assertEqual(self.calls, [])
                self.assertFalse(self.evidence.exists())

    def test_nothing_is_removed_before_the_run_and_nothing_is_started_by_name_twice(self):
        fake = FakeDocker()
        self.control(fake)
        self.assertEqual([args[0] for args, _ in fake.calls], ["image", "run", "rm"])  # no pre-emptive `rm` of the name

    def test_a_run_that_fails_cleanly_removes_nothing_and_runs_no_step(self):
        conflict = (125, 'docker: Error response from daemon: Conflict. The container name "/%s" is already in '
                         'use by container "%s".' % (CTL, "f" * 64))
        fake = FakeDocker(("run", conflict))
        got = self.control(fake)
        self.assertEqual(fake.labels(), ["gate-image-id", "run"])  # a sibling's container of this name is not ours to remove
        self.assertEqual(self.calls, [])
        self.assertEqual(set(got["verdict"]), {"ok", "error", "reasons"})
        self.assertIs(got["verdict"]["error"], True)
        self.assertIn("docker run failed (exit 125)", got["verdict"]["reasons"][0])
        self.assertIsNone(got["container_id"])
        self.assertIsNone(got["container_removed"])
        self.assertIn("nothing was removed", got["removal_note"])

    def test_a_run_with_no_answer_removes_by_name_and_says_so(self):
        for resp in (subprocess.TimeoutExpired(cmd=["docker"], timeout=600), FileNotFoundError("docker"),
                     (0, "some warning, no id\n")):
            with self.subTest(resp=repr(resp)):
                fake = FakeDocker(("run", resp))
                got = self.control(fake)
                self.assertEqual(fake.labels(), DOCKER_OK)
                self.assertEqual(fake.args_of("rm"), ("rm", "-f", "-v", CTL))
                self.assertIs(got["container_removed_by_name"], True)
                self.assertIs(got["verdict"]["error"], True)
                self.assertEqual(self.calls, [])

    def test_a_run_that_created_the_container_but_failed_to_start_is_removed_by_the_cidfile_id(self):
        def respond(args, kw):
            Path(args[args.index("--cidfile") + 1]).write_text(CID)
            return done(args, 125, "docker: Error response from daemon: OCI runtime create failed")
        fake = FakeDocker(("run", respond))
        got = self.control(fake)
        self.assertEqual(fake.labels(), DOCKER_OK)
        self.assertEqual(fake.args_of("rm"), ("rm", "-f", "-v", CID))
        self.assertIs(got["container_removed"], True)
        self.assertIs(got["container_removed_by_name"], False)
        self.assertEqual(self.calls, [])
        self.assertIs(got["verdict"]["error"], True)

    def test_a_fixed_name_control_cid_in_the_evidence_dir_is_never_read(self):
        self.evidence.mkdir()
        (self.evidence / "control.cid").write_text(CID)  # another run's ID, in the shared evidence dir

        def respond(args, kw):
            self.assertFalse(Path(args[args.index("--cidfile") + 1]).exists())
            return done(args, 125, "name in use")
        fake = FakeDocker(("run", respond))
        self.control(fake)
        self.assertEqual(fake.labels(), ["gate-image-id", "run"])  # nothing of ours exists, so nothing is removed

    def test_a_good_run_is_addressed_by_the_id_it_printed_not_the_name(self):
        other = "cd34" * 16
        fake = FakeDocker(("run", (0, "WARNING: no swap limit support\n%s\n" % other)))
        got = self.control(fake)
        self.assertEqual(set(self.containers), {other})
        self.assertEqual(fake.args_of("rm"), ("rm", "-f", "-v", other))
        self.assertEqual(got["container_id"], other)

    # --- the record and the evidence directory ---

    def test_a_record_missing_a_key_or_with_a_bad_value_raises_before_docker(self):
        bad = [dict((k, v) for k, v in CTL_RECORD.items() if k != key) for key in CTL_RECORD]
        bad += [dict(CTL_RECORD, base_commit="abc"), dict(CTL_RECORD, base_commit=CTL_BASE.upper()),
                dict(CTL_RECORD, docker_image="bad image"), dict(CTL_RECORD, docker_image=""),
                dict(CTL_RECORD, docker_image="-v /:/x img"), dict(CTL_RECORD, test_patch=5), None]
        for record in bad:
            with self.subTest(keys=sorted(record or ())):
                fake = FakeDocker()
                with self.assertRaises(ValueError):
                    self.control(fake, record=record)
                self.assertEqual(fake.calls, [])

    def test_a_record_that_carries_f2p_and_p2p_does_not_use_them(self):
        record = dict(CTL_RECORD, FAIL_TO_PASS=["x"], PASS_TO_PASS=["y"])
        got = self.control(record=record)
        self.assertEqual(got["f2p"], [T_A])

    def test_commands_and_instance_id_are_validated_before_docker(self):
        bad = [dict(CTL_RECORD, print_cmds=None), dict(CTL_RECORD, test_cmds="cargo test"),
               dict(CTL_RECORD, test_cmds=[]), dict(CTL_RECORD, print_cmds=[]), dict(CTL_RECORD, test_cmds=["a", 5]),
               dict(CTL_RECORD, print_cmds=("cat x",)), dict(CTL_RECORD, rebuild_cmds="cargo build"),
               dict(CTL_RECORD, rebuild_cmds=[None]), dict(CTL_RECORD, rebuild_cmds=None),
               dict(CTL_RECORD, instance_id=""), dict(CTL_RECORD, instance_id=None), dict(CTL_RECORD, instance_id=5)]
        for record in bad:
            with self.subTest(record={k: v for k, v in record.items() if k in ("test_cmds", "print_cmds",
                                                                              "rebuild_cmds", "instance_id")}):
                fake = FakeDocker()
                with self.assertRaises(ValueError):
                    self.control(fake, record=record)
                self.assertEqual(fake.calls, [])
                self.assertEqual(self.calls, [])
        got = self.control(record=dict(CTL_RECORD, rebuild_cmds=[]))  # no rebuild command is fine
        self.assertIs(got["verdict"]["ok"], True)

    # --- an internal error is a verdict too, and the evidence is written on every path ---

    def assert_internal_verdict(self, got, fake, exc):
        verdict = got["verdict"]
        self.assertEqual(set(verdict), {"ok", "error", "internal", "reasons"})
        self.assertIs(verdict["ok"], False)
        self.assertIs(verdict["error"], True)
        self.assertIs(verdict["internal"], True)
        self.assertEqual(len(verdict["reasons"]), 1)
        self.assertTrue(verdict["reasons"][0].startswith("internal error: %s: " % type(exc).__name__), verdict)
        self.assertLessEqual(len(verdict["reasons"][0]), 400)
        lines = got["internal_error_traceback"].splitlines()
        self.assertLessEqual(len(lines), 20)
        self.assertEqual(lines[-1].split(":")[0], type(exc).__name__)  # the traceback ends with the exception
        self.assertTrue(got["error"].startswith(type(exc).__name__ + ": "))
        self.assertEqual(fake.labels(), DOCKER_OK)
        self.assertEqual(fake.args_of("rm"), ("rm", "-f", "-v", CID))
        self.assertTrue(got["container_removed"])
        self.assertEqual(json.loads((self.evidence / "control-evidence.json").read_text()), got)

    def test_a_key_or_type_error_in_a_helper_is_an_internal_verdict_not_an_exception(self):
        for exc in (KeyError("print_cmds"), TypeError("'NoneType' object is not iterable"), AttributeError("x"),
                    IndexError("i"), ZeroDivisionError("z")):
            for index in (0, 4, 9, 11):
                with self.subTest(exc=type(exc).__name__, index=index):
                    self.calls.clear()
                    self.raises = {index: exc}
                    fake = FakeDocker()
                    got = self.control(fake)
                    self.assert_internal_verdict(got, fake, exc)
                    self.assertEqual(len(self.calls), index + 1)

    def test_a_key_or_type_error_after_the_states_ran_is_an_internal_verdict_too(self):
        for exc in (KeyError("t"), TypeError("bad")):
            with self.subTest(where="derive_sets", exc=type(exc).__name__):
                fake = FakeDocker()
                with mock.patch.object(fb, "derive_sets", side_effect=exc):
                    got = self.control(fake)
                self.assert_internal_verdict(got, fake, exc)
            with self.subTest(where="control_verdict", exc=type(exc).__name__):
                fake = FakeDocker()
                with mock.patch.object(fb.grade, "control_verdict", side_effect=exc):
                    got = self.control(fake)
                self.assert_internal_verdict(got, fake, exc)

    def test_the_parser_raising_a_key_error_is_an_internal_verdict(self):
        self.parse_raises = {1: KeyError("statuses")}
        fake = FakeDocker()
        self.assert_internal_verdict(self.control(fake), fake, KeyError("statuses"))

    def test_the_traceback_tail_is_the_last_20_lines(self):
        def deep(n):
            if n:
                return deep(n - 1)
            raise KeyError("deep")
        with mock.patch.object(fb.mc, "detect_workdir", side_effect=lambda c: deep(30)):
            got = self.control()
        lines = got["internal_error_traceback"].splitlines()
        self.assertEqual(len(lines), 20)
        self.assertEqual(lines[-1], "KeyError: 'deep'")

    def test_a_specific_step_error_is_not_marked_internal(self):
        self.raises = {4: fb.mc.ControlError("a step failed")}
        got = self.control()
        self.assertNotIn("internal", got["verdict"])
        self.assertNotIn("internal_error_traceback", got)

    def test_the_evidence_is_written_on_a_good_run_and_a_write_failure_does_not_raise(self):
        got = self.control()
        self.assertEqual(json.loads((self.evidence / "control-evidence.json").read_text()), got)
        self.assertTrue(got["verdict"]["ok"])
        (self.evidence / "control-evidence.json").unlink()
        (self.evidence / "control-evidence.json").mkdir()  # a directory: the write fails
        got = self.control()
        self.assertTrue(got["verdict"]["ok"])
        self.assertIn("control-evidence.json", got["evidence_write_error"])

    def test_an_interrupt_mid_run_removes_the_container_writes_the_evidence_then_reraises(self):
        for exc in (KeyboardInterrupt(), SystemExit(2)):
            with self.subTest(exc=type(exc).__name__):
                self.calls.clear()
                self.raises = {5: exc}  # the second reset_to_base: the base state is done
                fake = FakeDocker()
                with self.assertRaises(type(exc)):
                    self.control(fake)
                self.assertEqual(fake.labels(), DOCKER_OK)
                self.assertEqual(fake.args_of("rm"), ("rm", "-f", "-v", CID))
                ev = json.loads((self.evidence / "control-evidence.json").read_text())
                self.assertEqual(ev["verdict"], {"ok": False, "error": True, "interrupted": True,
                                                 "reasons": ["interrupted: " + type(exc).__name__]})
                self.assertIs(ev["container_removed"], True)
                self.assertEqual(ev["base"], {"fail": 2, "pass": 3})  # what was done is kept
                self.assertTrue((self.evidence / "base.log").is_file())

    def test_an_interrupt_while_removing_still_writes_the_evidence_and_is_reraised(self):
        fake = FakeDocker(("rm", KeyboardInterrupt()))
        with self.assertRaises(KeyboardInterrupt):
            self.control(fake)
        ev = json.loads((self.evidence / "control-evidence.json").read_text())
        self.assertIs(ev["container_removed"], False)
        # the gate passed, but the run was cut short: the evidence must say so, not read as a clean pass
        self.assertIs(ev["interrupted"], True)
        self.assertIs(ev["verdict"]["ok"], True)

    def test_a_run_that_was_not_interrupted_carries_no_interrupted_marker(self):
        self.assertNotIn("interrupted", self.control())

    def test_the_evidence_dir_is_created_and_the_steps_and_truncation_are_recorded(self):
        deep = self.tmp / "a" / "b"
        got = self.control(evidence_dir=deep)
        self.assertTrue((deep / "base.log").is_file())
        self.assertEqual([s["name"] for s in got["steps"]], [
            "base:rebuild", "base:test[0]", "gold:rebuild", "gold:test[0]", "gold-repeat:test[0]"])
        self.assertNotIn("log_truncated", got)

        def truncating(container, workdir, record, timeout=3600, *, steps=None, state="base"):
            steps.append({"name": state + ":print[0]", "returncode": 0, "tail": "", "truncated": True})
            return "{" + '"state": "%s", "statuses": %s}' % (state, json.dumps(self.statuses[state]))
        with mock.patch.object(fb.mc, "run_tests", truncating):
            self.assertIs(self.control()["log_truncated"], True)

    def test_the_result_names_what_it_ran_on(self):
        got = self.control()
        self.assertEqual(got["image"], CTL_IMAGE)
        self.assertEqual(got["container"], CTL)
        self.assertEqual(got["base_commit"], CTL_BASE)
        self.assertEqual(got["memory"], "8g")
        self.assertEqual(got["workdir"], "/testbed")

    def test_only_study_objects_are_named_and_nothing_else_is_deleted(self):
        fake = FakeDocker()
        self.control(fake)
        for args, _ in fake.calls:
            for a in args:
                self.assertNotRegex(a, r"codescout-|pbr27-|rae-")
            self.assertNotIn("volume", args)
            self.assertNotIn("prune", args)
            self.assertNotEqual(args[0], "rmi")
            self.assertNotIn(args[:2], (("image", "rm"), ("image", "prune")))


# --- Task 5: run_build and main ---

O_REPO = "rust-lang/rust-analyzer"
O_PR = 23096
O_ID = "rust-lang__rust-analyzer-23096"
O_UPSTREAM = "a1" * 20
O_TREE = "b2" * 20
O_SYN = "d4" * 20
O_IMAGE_ID = "sha256:" + "c3" * 32
O_BASE_IMAGE = "starryzhang/sweb.eval.x86_64.rust-lang_1776_rust-analyzer-22855"
O_TEMPLATE_ID = "rust-lang__rust-analyzer-22855"
O_TAG = "sab-fresh-rust-lang__rust-analyzer-23096:b1"
O_TESTS = ["crate::tests::a", "crate::tests::b", "crate::tests::c"]
O_P2P = ["crate::tests::p1", "crate::tests::p2"]
O_PS = "Issue title\r\nbody with CRLF\r\n"
O_TEST_PATCH = TEST_T.replace("+b\n", "+b\r\n")  # a CRLF line: it must reach every file byte for byte
O_DIFF = SRC_A + O_TEST_PATCH
O_TAR = b"TAR\x00BYTES\r\n\xff"
O_TOKEN = "ghp_SECRETSECRETSECRET"
O_STAGES = ["fetch", "cache-check", "tarball", "build", "control", "spec-verdicts", "record"]


def o_fetched(**over):
    out = {"merge_sha": "e5" * 20, "parents": ["f6" * 20], "upstream_base": O_UPSTREAM, "base_strategy": "first-parent",
           "created_at": "2026-08-10T10:00:00Z", "merged_at": "2026-08-11T10:00:00Z", "title": "Fix",
           "problem_statement": O_PS, "diff": O_DIFF, "pr_files": ["src/a.rs"], "issue_created_at": "2026-08-09T08:01:33Z"}
    out.update(over)
    return out


def o_control_evidence(**over):
    ev = {"id": O_ID, "image": O_TAG, "container": "sab-fresh-ctl-" + O_ID, "memory": "8g", "base_commit": O_SYN,
          "workdir": "/testbed", "f2p": list(O_TESTS), "p2p": list(O_P2P),
          "steps": [{"name": "base:rebuild", "returncode": 0, "tail": ""}],
          "base": {"fail": 3, "pass": 2}, "gold": {"pass": 5}, "gold_repeat": {"pass": 5},
          "verdict": {"ok": True, "reasons": []}, "wall_seconds": 12.5, "laps_seconds": {}}
    ev.update(over)
    return ev


class HostRun:
    """`run` for the host programs `run_build` calls itself: `docker image inspect`, `git archive`, `git rev-parse`."""

    def __init__(self, src):
        self.src = str(src)
        self.calls = []  # (argv tuple, env, timeout)
        self.fail = {}   # label -> (exit code, stderr bytes)
        self.out = {"inspect": (O_IMAGE_ID + "\n").encode(), "archive": O_TAR, "tree": (O_TREE + "\n").encode()}

    def __call__(self, argv, **kw):
        if kw.get("capture_output") is not True:
            raise AssertionError("run() must capture bytes: %r" % kw)
        for bad in ("text", "universal_newlines", "encoding", "errors"):
            if bad in kw:
                raise AssertionError("run() must not set %s" % bad)
        argv = tuple(argv)
        self.calls.append((argv, kw.get("env"), kw.get("timeout")))
        if argv == ("docker", "image", "inspect", "--format", "{{.Id}}", O_BASE_IMAGE):
            label = "inspect"
        elif argv == ("git", "-C", self.src, "archive", "--format=tar", O_UPSTREAM):
            label = "archive"
        elif argv == ("git", "-C", self.src, "rev-parse", O_UPSTREAM + "^{tree}"):
            label = "tree"
        else:
            raise AssertionError("unexpected host call: %r" % (argv,))
        if label in self.fail:
            code, err = self.fail[label]
            return subprocess.CompletedProcess(list(argv), code, b"", err)
        return subprocess.CompletedProcess(list(argv), 0, self.out[label], b"")

    def labels(self):
        return [{"docker": "inspect", "archive": "archive", "rev-parse": "tree"}[a[0] if a[0] == "docker" else a[3]]
                for a, _, _ in self.calls]


class OrchestrationTest(unittest.TestCase):
    """`run_build` with `run`, `fetch`, `build` and `control` replaced by fakes: no docker, git, gh or network."""

    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.tmp = Path(tmp.name)
        self.data = self.tmp / "data"
        self.ev = self.data / "evidence" / O_ID
        self.template = self.tmp / "template.jsonl"
        self.verdicts = self.tmp / "spec-verdicts.json"
        self.write_template()
        self.host = HostRun(self.data / "upstream" / O_ID / "src")
        self.fetch_calls, self.build_calls, self.control_calls = [], [], []
        self.fetch_raises = self.build_raises = None
        self.fetched = o_fetched()
        self.build_result = {"image": O_TAG, "base_commit": O_SYN, "base_image_id": O_IMAGE_ID,
                             "tree_tar_sha256": "0" * 64, "image_id": "sha256:" + "e5" * 32, "size_bytes": 1,
                             "laps_seconds": {}, "compiling_lines": 5, "container_removed": True}
        self.control_ev = o_control_evidence()
        self.stderr = io.StringIO()
        env = mock.patch.dict(os.environ, {"GITHUB_TOKEN": O_TOKEN})
        env.start()
        self.addCleanup(env.stop)

    # --- the fakes ---

    def fake_fetch(self, repo, pr, issue_numbers, dest, *, run=None, **kw):
        self.fetch_calls.append({"repo": repo, "pr": pr, "issue_numbers": issue_numbers, "dest": dest, "run": run})
        if self.fetch_raises is not None:
            raise self.fetch_raises
        return dict(self.fetched)

    def fake_build(self, **kw):
        tar = Path(kw["tree_tar"])
        self.build_calls.append(dict(kw, tar_bytes=tar.read_bytes() if tar.is_file() else None))
        if self.build_raises is not None:
            raise self.build_raises
        return dict(self.build_result)

    def fake_control(self, record, **kw):
        self.control_calls.append((copy.deepcopy(record), kw))
        return copy.deepcopy(self.control_ev)

    # --- helpers ---

    def write_template(self, **over):
        rec = {"instance_id": O_TEMPLATE_ID, "repo": O_REPO, "log_parser": TEMPLATE["log_parser"],
               "test_cmds": list(TEMPLATE["test_cmds"]), "rebuild_cmds": list(TEMPLATE["rebuild_cmds"]),
               "print_cmds": list(TEMPLATE["print_cmds"])}
        rec.update(over)
        other = {"instance_id": "other__repo-1", "test_cmds": ["x"]}
        self.template.write_text(json.dumps(other) + "\n\n" + json.dumps(rec) + "\n", encoding="utf-8")

    def go(self, **over):
        args = dict(repo=O_REPO, pr=O_PR, issue_numbers=[23090], base_image=O_BASE_IMAGE,
                    template_path=self.template, template_id=O_TEMPLATE_ID, data=self.data,
                    spec_verdicts_path=None, dry_run=False, run=self.host, fetch=self.fake_fetch,
                    build=self.fake_build, control=self.fake_control)
        args.update(over)
        with contextlib.redirect_stderr(self.stderr):
            return fb.run_build(**args)

    def skey(self):
        return fb.spec_key(O_PS, O_TEST_PATCH, SRC_A)

    def write_verdicts(self, tests, *, key=None, **entry):
        entry = dict({"tests": tests, "reader": "reader-1", "note": "n"}, **entry)
        self.verdicts.write_text(json.dumps({key or self.skey(): entry}), encoding="utf-8")

    def keep(self, *kept):
        """A verdict file that keeps `kept` and drops every other test of `O_TESTS`."""
        self.write_verdicts({t: "specified" if t in kept else "unspecified" for t in O_TESTS})

    def with_verdicts(self, **over):
        return self.go(spec_verdicts_path=self.verdicts, **over)

    def records(self):
        path = self.data / "records.jsonl"
        return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()] if path.exists() else None

    def cache_key(self):
        return fb.cache_key(O_REPO, O_UPSTREAM, O_IMAGE_ID)

    def make_cached(self):
        self.ev.mkdir(parents=True)
        (self.ev / "cache-key.txt").write_text(self.cache_key())
        (self.data / "controls" / O_ID).mkdir(parents=True)
        (self.data / "controls" / O_ID / "control.json").write_text("{}")

    def forget_pending(self):
        """As if the awaiting run before had never happened: the next run has to build again."""
        (self.ev / "pending.json").unlink(missing_ok=True)

    def leftovers(self):
        return sorted(str(p.relative_to(self.data)) for p in self.data.rglob("*.tmp"))

    # --- dry run ---

    def test_a_dry_run_names_the_stages_and_calls_nothing(self):
        got = self.go(dry_run=True)
        self.assertEqual((got["status"], got["exit_code"]), ("dry-run", 0))
        self.assertEqual(got["stages"], O_STAGES)
        self.assertEqual((self.host.calls, self.fetch_calls, self.build_calls, self.control_calls), ([], [], [], []))
        self.assertFalse(self.data.exists())  # no lock, no directory

    def test_a_dry_run_does_not_need_the_template_or_the_verdicts(self):
        self.template.unlink()
        self.assertEqual(self.go(dry_run=True)["status"], "dry-run")

    def test_arguments_are_validated_before_anything_runs(self):
        bad = [dict(repo="nope"), dict(repo="a/b/c"), dict(pr=0), dict(pr="5"), dict(pr=True), dict(issue_numbers=[]),
               dict(issue_numbers=[0]), dict(issue_numbers=["1"]), dict(issue_numbers=[True]),
               dict(base_image="-v /:/x"), dict(base_image=""), dict(base_image="bad image"),
               dict(template_id=""), dict(template_id=None)]
        for over in bad:
            for dry in (False, True):
                with self.subTest(over=over, dry_run=dry):
                    with self.assertRaises(ValueError):
                        self.go(dry_run=dry, **over)
        self.assertEqual((self.host.calls, self.fetch_calls, self.build_calls), ([], [], []))
        self.assertFalse(self.data.exists())

    # --- the lock and the per-run evidence directory ---

    def test_a_second_run_while_the_lock_exists_refuses_and_leaves_the_lock(self):
        self.data.mkdir()
        (self.data / ".lock").write_text("pid 1\n")
        got = self.go()
        self.assertEqual((got["status"], got["exit_code"], got["stage"]), ("refused", 2, "lock"))
        self.assertIn(".lock", got["error"])
        self.assertEqual((self.data / ".lock").read_text(), "pid 1\n")  # not ours: not removed
        self.assertEqual((self.host.calls, self.fetch_calls, self.build_calls), ([], [], []))
        self.assertFalse((self.data / "evidence").exists())

    def test_the_lock_is_held_while_the_run_works_and_released_after_every_outcome(self):
        seen = []

        def build(**kw):
            seen.append((self.data / ".lock").exists())
            return self.fake_build(**kw)
        self.go(build=build)
        self.assertEqual(seen, [True])
        self.assertFalse((self.data / ".lock").exists())
        self.forget_pending()
        self.build_raises = fb.BuildError("boom")
        self.go()
        self.assertFalse((self.data / ".lock").exists())
        self.build_raises = RuntimeError("a bug")
        with self.assertRaises(RuntimeError):
            self.go()
        self.assertFalse((self.data / ".lock").exists())
        self.build_raises = KeyboardInterrupt()
        with self.assertRaises(KeyboardInterrupt):
            self.go()
        self.assertFalse((self.data / ".lock").exists())

    def test_the_lock_is_taken_exclusively(self):
        taken = []
        real_open = os.open

        def spy(path, flags, *a, **k):
            if Path(path).name == ".lock":
                taken.append(flags)
            return real_open(path, flags, *a, **k)
        with mock.patch.object(os, "open", spy):
            self.go()
        self.assertEqual(len(taken), 1)
        self.assertTrue(taken[0] & os.O_EXCL and taken[0] & os.O_CREAT)

    def test_two_runs_use_different_per_run_evidence_directories(self):
        self.control_ev = o_control_evidence(verdict={"ok": False, "reasons": ["x"]})
        self.go()
        self.go()
        dirs = [c["evidence_dir"] for c in self.build_calls]
        self.assertEqual(len(set(dirs)), 2)
        for d in dirs:
            self.assertEqual(Path(d).parent, self.ev)
            self.assertRegex(Path(d).name, r"^run-")
            self.assertTrue(Path(d).is_dir())
        self.assertEqual([kw["evidence_dir"] for _, kw in self.control_calls], dirs)  # build and control share one

    def test_a_clock_that_repeats_still_gives_each_run_its_own_directory(self):
        self.control_ev = o_control_evidence(verdict={"ok": False, "reasons": ["x"]})
        with mock.patch.object(fb, "_stamp", return_value="T"):
            self.go()
            self.go()
        dirs = [c["evidence_dir"] for c in self.build_calls]
        self.assertEqual(len(set(dirs)), 2)

    def test_cache_key_and_pending_live_one_level_above_the_run_directory(self):
        self.go()  # no verdicts: pending
        run_dir = Path(self.build_calls[0]["evidence_dir"])
        self.assertTrue((self.ev / "pending.json").is_file())
        self.assertFalse((run_dir / "pending.json").exists())
        self.assertFalse((run_dir / "cache-key.txt").exists())

    # --- fetch and the cache ---

    def test_fetch_gets_the_arguments_a_clone_directory_and_the_credential_free_run(self):
        self.go()
        self.assertEqual(len(self.fetch_calls), 1)
        call = self.fetch_calls[0]
        self.assertEqual((call["repo"], call["pr"], call["issue_numbers"]), (O_REPO, O_PR, [23090]))
        self.assertEqual(call["dest"], self.data / "upstream" / O_ID)
        self.assertIs(call["run"], self.host)

    def test_a_cached_key_skips_the_tarball_the_build_and_the_control(self):
        self.make_cached()
        got = self.go()
        self.assertEqual((got["status"], got["exit_code"]), ("cached", 0))
        self.assertEqual(len(self.fetch_calls), 1)  # the fetch is what yields the upstream base
        self.assertEqual(self.host.labels(), ["inspect"])
        self.assertEqual((self.build_calls, self.control_calls), ([], []))
        self.assertIsNone(self.records())

    def test_the_cache_key_needs_the_matching_key_and_a_control_json(self):
        self.make_cached()
        (self.data / "controls" / O_ID / "control.json").unlink()
        self.go()
        self.assertEqual(len(self.build_calls), 1)  # a key without a control.json is not a hit
        (self.data / "controls" / O_ID / "control.json").write_text("{}")
        for what in ("repo", "base", "image", "version"):
            with self.subTest(changed=what):
                self.build_calls.clear()
                self.forget_pending()
                self.host.out["inspect"] = (O_IMAGE_ID + "\n").encode()
                key = {"repo": fb.cache_key("other/repo", O_UPSTREAM, O_IMAGE_ID),
                       "base": fb.cache_key(O_REPO, "9" * 40, O_IMAGE_ID),
                       "image": fb.cache_key(O_REPO, O_UPSTREAM, "sha256:" + "0" * 64),
                       "version": fb.cache_key(O_REPO, O_UPSTREAM, O_IMAGE_ID, version="2")}[what]
                (self.ev / "cache-key.txt").write_text(key)
                self.go()
                self.assertEqual(len(self.build_calls), 1)

    def test_a_cache_key_without_the_key_file_is_a_miss(self):
        self.make_cached()
        (self.ev / "cache-key.txt").unlink()
        self.go()
        self.assertEqual(len(self.build_calls), 1)

    def test_the_cache_key_uses_the_inspected_image_id(self):
        self.make_cached()
        self.host.out["inspect"] = ("sha256:" + "9" * 64 + "\n").encode()
        self.go()
        self.assertEqual(len(self.build_calls), 1)

    def test_a_trailing_newline_in_the_key_file_still_hits(self):
        self.make_cached()
        (self.ev / "cache-key.txt").write_text(self.cache_key() + "\n")
        self.assertEqual(self.go()["status"], "cached")

    def test_the_image_inspect_failing_or_giving_no_id_is_a_refusal(self):
        for how in ({"inspect": (1, b"Error: No such image")}, ):
            with self.subTest(how=how):
                self.host.fail = how
                got = self.go()
                self.assertEqual((got["status"], got["exit_code"], got["stage"]), ("refused", 2, "cache-check"))
                self.assertEqual(self.build_calls, [])
        self.host.fail = {}
        for out in (b"", b"not an id\n", b"sha256:abc\n"):
            with self.subTest(out=out):
                self.host.out["inspect"] = out
                got = self.go()
                self.assertEqual((got["status"], got["stage"]), ("refused", "cache-check"))
                self.assertEqual(self.build_calls, [])

    # --- the tarball and the tree proof ---

    def test_the_tarball_and_the_tree_proof_are_issued_in_order_and_the_proof_reaches_build(self):
        self.go()
        self.assertEqual(self.host.labels(), ["inspect", "archive", "tree"])
        self.assertEqual(len(self.build_calls), 1)
        call = self.build_calls[0]
        self.assertEqual(call["expected_tree"], O_TREE)
        self.assertEqual(call["tar_bytes"], O_TAR)  # the bytes of `git archive`, untranslated
        self.assertEqual(call["upstream_base"], O_UPSTREAM)
        self.assertEqual((call["base_image"], call["repo"], call["pr"]), (O_BASE_IMAGE, O_REPO, O_PR))
        self.assertEqual(Path(call["tree_tar"]).parent, Path(call["evidence_dir"]))

    def test_the_host_programs_run_without_the_token(self):
        self.go()
        self.assertTrue(self.host.calls)
        for argv, env, timeout in self.host.calls:
            self.assertNotIn("GITHUB_TOKEN", env)
            self.assertEqual(env["GIT_TERMINAL_PROMPT"], "0")
            self.assertIsInstance(timeout, int)
            self.assertNotIn(O_TOKEN, " ".join(argv))
        self.assertEqual(os.environ["GITHUB_TOKEN"], O_TOKEN)  # the process environment itself is untouched

    def test_a_failing_archive_or_tree_proof_is_a_refusal_and_nothing_is_built(self):
        for label in ("archive", "tree"):
            with self.subTest(label=label):
                self.host.fail = {label: (128, b"fatal: not a tree")}
                got = self.go()
                self.assertEqual((got["status"], got["exit_code"], got["stage"]), ("refused", 2, "tarball"))
                self.assertEqual(self.build_calls, [])
                self.assertIsNone(self.records())
        self.host.fail = {}
        for out in (b"", b"garbage\n", (O_TREE.upper() + "\n").encode(), (O_TREE[:39] + "\n").encode()):
            with self.subTest(tree=out):
                self.host.out["tree"] = out
                got = self.go()
                self.assertEqual((got["status"], got["stage"]), ("refused", "tarball"))
                self.assertEqual(self.build_calls, [])

    def test_a_tree_mismatch_reported_by_build_is_a_refusal_with_evidence(self):
        self.build_raises = fb.BuildError("synthetic tree %s differs from expected tree %s" % ("1" * 40, O_TREE))
        got = self.go()
        self.assertEqual((got["status"], got["exit_code"], got["stage"]), ("refused", 2, "build"))
        self.assertIn(O_TREE, got["error"])
        refusal = json.loads((Path(got["evidence"]) / "refusal.json").read_text())
        self.assertEqual((refusal["stage"], refusal["error"]), ("build", got["error"]))
        self.assertEqual(self.control_calls, [])
        self.assertIsNone(self.records())

    # --- build and control ---

    def test_a_build_error_is_a_refusal_not_a_crash_and_records_jsonl_is_untouched(self):
        self.data.mkdir()
        (self.data / "records.jsonl").write_text('{"instance_id": "old"}\n')
        self.build_raises = fb.BuildError("tag exists")
        got = self.go()
        self.assertEqual((got["status"], got["exit_code"]), ("refused", 2))
        self.assertEqual((self.data / "records.jsonl").read_bytes(), b'{"instance_id": "old"}\n')
        self.assertEqual(self.control_calls, [])
        self.assertFalse((self.ev / "cache-key.txt").exists())
        self.assertEqual(self.leftovers(), [])

    def test_a_bug_in_build_propagates_and_records_jsonl_is_never_half_written(self):
        self.data.mkdir()
        (self.data / "records.jsonl").write_text('{"instance_id": "old"}\n')
        self.build_raises = RuntimeError("a bug")
        with self.assertRaises(RuntimeError):
            self.go()
        self.assertEqual((self.data / "records.jsonl").read_bytes(), b'{"instance_id": "old"}\n')
        self.assertEqual(self.leftovers(), [])

    def test_a_fetch_error_is_a_refusal(self):
        self.fetch_raises = fb.FetchError("PR is not merged")
        got = self.go()
        self.assertEqual((got["status"], got["exit_code"], got["stage"]), ("refused", 2, "fetch"))
        self.assertIn("not merged", got["error"])
        self.assertEqual(self.host.calls, [])

    def test_the_control_gets_the_built_image_the_synthetic_commit_the_patches_and_the_template(self):
        self.go()
        record, kw = self.control_calls[0]
        self.assertEqual(record["docker_image"], O_TAG)
        self.assertEqual(record["base_commit"], O_SYN)
        self.assertEqual((record["patch"], record["test_patch"]), (SRC_A, O_TEST_PATCH))
        self.assertEqual(record["instance_id"], O_ID)
        self.assertEqual(record["test_cmds"], TEMPLATE["test_cmds"])
        self.assertEqual(record["rebuild_cmds"], TEMPLATE["rebuild_cmds"])
        self.assertEqual(record["print_cmds"], TEMPLATE["print_cmds"])
        self.assertEqual(record["log_parser"], TEMPLATE["log_parser"])
        self.assertEqual(kw["container"], "sab-fresh-ctl-" + O_ID)
        self.assertEqual(Path(kw["evidence_dir"]), Path(self.build_calls[0]["evidence_dir"]))

    def test_a_base_image_that_changed_during_the_build_is_refused(self):
        self.build_result["base_image_id"] = "sha256:" + "7" * 64
        got = self.go()
        self.assertEqual((got["status"], got["exit_code"], got["stage"]), ("refused", 2, "build"))
        self.assertEqual(self.control_calls, [])
        self.assertIsNone(self.records())

    # --- the gate ---

    def test_a_failed_gate_gives_exit_2_and_writes_no_record_control_or_key(self):
        self.keep(*O_TESTS)
        for verdict in ({"ok": False, "reasons": ["tests differ between two gold runs"]},
                        {"ok": False, "error": True, "reasons": ["control error: x"]}, {"reasons": []}):
            with self.subTest(verdict=verdict):
                self.control_ev = o_control_evidence(verdict=verdict)
                got = self.with_verdicts()
                self.assertEqual((got["status"], got["exit_code"]), ("gate-failed", 2))
                self.assertEqual(got["reasons"], verdict.get("reasons"))
                self.assertIsNone(self.records())
                self.assertFalse((self.data / "controls" / O_ID / "control.json").exists())
                self.assertFalse((self.ev / "cache-key.txt").exists())
                self.assertFalse((self.ev / "pending.json").exists())

    # --- the specification verdicts ---

    def test_without_a_verdict_file_the_run_stops_awaiting_and_writes_no_record(self):
        got = self.go()
        self.assertEqual((got["status"], got["exit_code"]), ("awaiting-spec-verdicts", 3))
        self.assertEqual((got["spec_key"], got["f2p"]), (self.skey(), O_TESTS))
        self.assertIsNone(self.records())
        self.assertFalse((self.data / "controls" / O_ID / "control.json").exists())
        self.assertFalse((self.ev / "cache-key.txt").exists())
        pending = json.loads((self.ev / "pending.json").read_text())
        self.assertEqual((pending["spec_key"], pending["f2p"], pending["p2p"]), (self.skey(), O_TESTS, O_P2P))
        self.assertEqual(pending["cache_key"], self.cache_key())
        # the evidence a reader needs, byte for byte
        self.assertEqual((self.ev / "patch.diff").read_bytes(), SRC_A.encode())
        self.assertEqual((self.ev / "test_patch.diff").read_bytes(), O_TEST_PATCH.encode())
        self.assertEqual((self.ev / "problem_statement.txt").read_bytes(), O_PS.encode())
        self.assertIn(b"\r\n", (self.ev / "test_patch.diff").read_bytes())

    def test_a_verdict_file_without_an_entry_for_this_spec_key_still_awaits(self):
        self.write_verdicts({t: "specified" for t in O_TESTS}, key="0" * 64)
        got = self.with_verdicts()
        self.assertEqual((got["status"], got["exit_code"]), ("awaiting-spec-verdicts", 3))
        self.assertIsNone(self.records())

    def test_verdicts_that_drop_every_f2p_make_the_task_unusable(self):
        self.keep()
        got = self.with_verdicts()
        self.assertEqual((got["status"], got["exit_code"]), ("unusable", 4))
        self.assertEqual(got["dropped_f2p"], O_TESTS)
        self.assertIsNone(self.records())
        self.assertFalse((self.data / "controls" / O_ID / "control.json").exists())
        self.assertFalse((self.ev / "cache-key.txt").exists())
        applied = json.loads((self.ev / "spec-verdicts-applied.json").read_text())
        self.assertEqual((applied["kept"], applied["dropped"], applied["reader"]), ([], O_TESTS, "reader-1"))

    def test_verdicts_that_keep_two_of_three_append_one_record(self):
        self.data.mkdir()
        (self.data / "records.jsonl").write_bytes(b'{"instance_id": "old"}\n')
        self.keep(O_TESTS[0], O_TESTS[2])
        got = self.with_verdicts()
        self.assertEqual((got["status"], got["exit_code"]), ("built", 0))
        records = self.records()
        self.assertEqual([r["instance_id"] for r in records], ["old", O_ID])  # appended, nothing else touched
        rec = records[1]
        self.assertEqual(rec["FAIL_TO_PASS"], [O_TESTS[0], O_TESTS[2]])
        self.assertEqual(rec["dropped_f2p"], [O_TESTS[1]])
        self.assertEqual(rec["PASS_TO_PASS"], O_P2P)
        self.assertEqual(set(rec), RECORD_KEYS | EXTRA_KEYS)
        self.assertEqual((rec["docker_image"], rec["base_commit"], rec["upstream_base_commit"]),
                         (O_TAG, O_SYN, O_UPSTREAM))
        self.assertEqual((rec["patch"], rec["test_patch"], rec["problem_statement"]), (SRC_A, O_TEST_PATCH, O_PS))
        self.assertEqual((rec["repo"], rec["pull_number"], rec["issue_numbers"]), (O_REPO, "23096", ["23090"]))
        self.assertEqual(rec["created_at"], self.fetched["created_at"])
        self.assertEqual(rec["test_cmds"], TEMPLATE["test_cmds"])
        self.assertTrue((self.data / "records.jsonl").read_bytes().endswith(b"\n"))
        self.assertEqual((self.ev / "cache-key.txt").read_text().strip(), self.cache_key())
        self.assertEqual(self.leftovers(), [])

    def test_control_json_has_the_keys_the_study_loaders_read(self):
        self.keep(*O_TESTS)
        self.with_verdicts()
        control = json.loads((self.data / "controls" / O_ID / "control.json").read_text())
        self.assertEqual(set(control), {"id", "image", "workdir", "steps", "base", "gold", "gold_repeat", "verdict",
                                        "wall_seconds"})
        self.assertEqual((control["id"], control["image"], control["workdir"]), (O_ID, O_TAG, "/testbed"))
        self.assertEqual(control["verdict"], {"ok": True, "reasons": []})
        self.assertEqual(control["wall_seconds"], 12.5)

    def test_a_missing_verdict_for_an_f2p_test_is_an_error_never_a_silent_keep(self):
        self.write_verdicts({O_TESTS[0]: "specified", O_TESTS[1]: "specified"})  # c has no verdict
        got = self.with_verdicts()
        self.assertEqual((got["status"], got["exit_code"], got["stage"]), ("refused", 2, "spec-verdicts"))
        self.assertIn(O_TESTS[2], got["error"])
        self.assertIsNone(self.records())
        self.assertFalse((self.data / "controls" / O_ID / "control.json").exists())
        self.assertFalse((self.ev / "cache-key.txt").exists())

    def test_a_bad_verdict_value_or_a_malformed_entry_or_file_is_a_refusal(self):
        good = {t: "specified" for t in O_TESTS}
        entries = [
            {"tests": dict(good, **{O_TESTS[0]: "maybe"}), "reader": "r"},
            {"tests": good},                        # no reader
            {"tests": good, "reader": ""},
            {"tests": good, "reader": 5},
            {"tests": good, "reader": "r", "note": 5},
            {"tests": list(good), "reader": "r"},   # not a mapping
            {"reader": "r"},
            "specified",
            [],
        ]
        for entry in entries:
            with self.subTest(entry=entry):
                self.verdicts.write_text(json.dumps({self.skey(): entry}))
                got = self.with_verdicts()
                self.assertEqual((got["status"], got["exit_code"], got["stage"]), ("refused", 2, "spec-verdicts"))
                self.assertIsNone(self.records())
        for text in ("not json", "[]", '"x"'):
            with self.subTest(text=text):
                self.verdicts.write_text(text)
                self.assertEqual(self.with_verdicts()["status"], "refused")
        self.verdicts.unlink()
        self.assertEqual(self.with_verdicts()["status"], "refused")  # a named file that is not there is an error
        self.assertIsNone(self.records())

    def test_extra_verdicts_for_tests_that_are_not_f2p_are_ignored(self):
        self.write_verdicts(dict({t: "specified" for t in O_TESTS}, **{"stale::test": "unspecified"}))
        got = self.with_verdicts()
        self.assertEqual(got["status"], "built")
        self.assertEqual(self.records()[0]["FAIL_TO_PASS"], O_TESTS)
        self.assertEqual(self.records()[0]["dropped_f2p"], [])

    # --- the second run of the two-step flow, and idempotence ---

    def test_a_run_with_the_verdicts_after_an_awaiting_run_does_not_rebuild(self):
        self.assertEqual(self.go()["status"], "awaiting-spec-verdicts")
        self.keep(O_TESTS[0], O_TESTS[1])
        got = self.with_verdicts()
        self.assertEqual((got["status"], got["exit_code"]), ("built", 0))
        self.assertEqual((len(self.build_calls), len(self.control_calls)), (1, 1))  # the image is reused
        self.assertEqual(self.host.labels().count("archive"), 1)
        self.assertEqual(self.records()[0]["FAIL_TO_PASS"], O_TESTS[:2])
        self.assertEqual(self.go()["status"], "cached")  # and then it is cached
        self.assertEqual(len(self.build_calls), 1)

    def test_a_pending_state_of_another_spec_is_not_reused(self):
        self.go()
        self.fetched = o_fetched(problem_statement=O_PS + "more")
        self.go()
        self.assertEqual(len(self.build_calls), 2)

    def test_a_pending_state_of_another_cache_key_is_not_reused(self):
        self.go()
        self.host.out["inspect"] = ("sha256:" + "9" * 64 + "\n").encode()
        self.go()
        self.assertEqual(len(self.build_calls), 2)

    def test_a_pending_file_that_cannot_be_read_is_ignored(self):
        self.go()
        (self.ev / "pending.json").write_text("{not json")
        self.go()
        self.assertEqual(len(self.build_calls), 2)

    def test_the_record_is_written_after_control_json_and_before_the_cache_key(self):
        order = []
        real = os.replace

        def spy(src, dst, *a, **k):
            order.append(Path(dst).name)
            return real(src, dst, *a, **k)
        self.keep(*O_TESTS)
        with mock.patch.object(os, "replace", spy):
            self.with_verdicts()
        done = [n for n in order if n in ("control.json", "records.jsonl", "cache-key.txt")]
        self.assertEqual(done, ["control.json", "records.jsonl", "cache-key.txt"])

    def fail_replace_of(self, name):
        real = os.replace

        def replace(src, dst, *a, **k):
            if Path(dst).name == name:
                raise OSError("disk full")
            return real(src, dst, *a, **k)
        return mock.patch.object(os, "replace", replace)

    def test_a_failed_write_of_records_jsonl_leaves_it_whole_and_writes_no_cache_key(self):
        self.data.mkdir()
        (self.data / "records.jsonl").write_bytes(b'{"instance_id": "old"}\n')
        self.keep(*O_TESTS)
        with self.fail_replace_of("records.jsonl"):
            with self.assertRaises(OSError):
                self.with_verdicts()
        self.assertEqual((self.data / "records.jsonl").read_bytes(), b'{"instance_id": "old"}\n')
        self.assertFalse((self.ev / "cache-key.txt").exists())  # the key is the last write, never before the record
        self.assertEqual(self.leftovers(), [])
        self.assertFalse((self.data / ".lock").exists())

    def test_a_crash_after_the_record_does_not_duplicate_it_on_the_next_run(self):
        self.keep(*O_TESTS)
        with self.fail_replace_of("cache-key.txt"):
            with self.assertRaises(OSError):
                self.with_verdicts()
        self.assertEqual(len(self.records()), 1)
        got = self.with_verdicts()
        self.assertEqual(got["status"], "built")
        self.assertEqual(len(self.records()), 1)  # the same record is not appended twice
        self.assertEqual((self.ev / "cache-key.txt").read_text().strip(), self.cache_key())
        self.assertEqual(len(self.build_calls), 1)

    def test_a_different_record_for_the_same_id_is_refused_not_replaced(self):
        self.data.mkdir()
        old = json.dumps({"instance_id": O_ID, "FAIL_TO_PASS": ["other"]})
        (self.data / "records.jsonl").write_text(old + "\n")
        self.keep(*O_TESTS)
        got = self.with_verdicts()
        self.assertEqual((got["status"], got["exit_code"], got["stage"]), ("refused", 2, "record"))
        self.assertEqual((self.data / "records.jsonl").read_text(), old + "\n")
        self.assertFalse((self.ev / "cache-key.txt").exists())

    def test_an_existing_file_without_a_final_newline_gets_one_before_the_record(self):
        self.data.mkdir()
        (self.data / "records.jsonl").write_text('{"instance_id": "old"}')
        self.keep(*O_TESTS)
        self.with_verdicts()
        self.assertEqual([r["instance_id"] for r in self.records()], ["old", O_ID])

    # --- the template is data ---

    def test_a_template_whose_commands_are_list_like_strings_is_parsed(self):
        self.write_template(test_cmds="['cd /testbed/repo && cargo test', 'echo done']",
                            rebuild_cmds=' ["cargo check -q"] ', print_cmds="['cat log']")
        self.go()
        record, _ = self.control_calls[0]
        self.assertEqual(record["test_cmds"], ["cd /testbed/repo && cargo test", "echo done"])
        self.assertEqual(record["rebuild_cmds"], ["cargo check -q"])
        self.assertEqual(record["print_cmds"], ["cat log"])
        self.write_template(rebuild_cmds="[]")  # no rebuild command is fine
        self.forget_pending()
        self.go()
        self.assertEqual(self.control_calls[1][0]["rebuild_cmds"], [])

    def test_a_template_that_is_not_a_literal_is_refused_and_never_executed(self):
        probe = "FB_EVAL_PROBE"
        os.environ.pop(probe, None)
        self.addCleanup(os.environ.pop, probe, None)
        bad = ["['a', str(1)]",                                           # a call: valid under eval, not a literal
               "[__import__('os').environ.setdefault(%r, '1')]" % probe,  # one that would leave a trace
               "cargo test", "[1, 2]", "['a', None]", "{'a': 'b'}", "['unterminated", "[", "('a',)", "5"]
        for text in bad:
            for key in ("test_cmds", "rebuild_cmds", "print_cmds"):
                with self.subTest(text=text, key=key):
                    self.write_template(**{key: text})
                    self.fetch_calls.clear()
                    got = self.go()
                    self.assertEqual((got["status"], got["exit_code"], got["stage"]), ("refused", 2, "template"))
                    self.assertEqual(self.fetch_calls, [])  # refused before any network
        self.assertNotIn(probe, os.environ)
        self.assertEqual(self.build_calls, [])

    def test_a_template_with_empty_or_missing_commands_or_a_bad_parser_is_refused(self):
        for over in (dict(test_cmds=[]), dict(print_cmds=[]), dict(test_cmds=["a", 5]), dict(log_parser=""),
                     dict(log_parser=["x"]), dict(rebuild_cmds=None), dict(test_cmds="[]")):
            with self.subTest(over=over):
                self.write_template(**over)
                got = self.go()
                self.assertEqual((got["status"], got["stage"]), ("refused", "template"))
        rec = json.loads(self.template.read_text().splitlines()[-1])
        del rec["print_cmds"]
        self.template.write_text(json.dumps(rec) + "\n")
        self.assertEqual(self.go()["status"], "refused")
        self.assertEqual((self.fetch_calls, self.build_calls), ([], []))

    def test_a_template_id_that_is_missing_or_ambiguous_or_a_file_that_cannot_be_read_is_refused(self):
        self.assertEqual(self.go(template_id="no__such-1")["stage"], "template")
        line = self.template.read_text().splitlines()[-1]
        self.template.write_text(line + "\n" + line + "\n")
        self.assertEqual(self.go()["stage"], "template")
        self.template.write_text("not json\n")
        self.assertEqual(self.go()["stage"], "template")
        self.template.unlink()
        self.assertEqual(self.go()["stage"], "template")
        self.assertEqual((self.fetch_calls, self.build_calls), ([], []))

    # --- the stage log and credentials ---

    def test_one_json_object_per_stage_goes_to_stderr_and_no_credential(self):
        self.keep(*O_TESTS)
        self.with_verdicts()
        lines = self.stderr.getvalue().splitlines()
        objects = [json.loads(line) for line in lines]
        self.assertEqual([o["stage"] for o in objects], O_STAGES)
        self.assertNotIn(O_TOKEN, self.stderr.getvalue())

    def test_a_token_in_an_error_message_never_reaches_stderr_the_result_or_the_evidence(self):
        self.fetch_raises = fb.FetchError("gh api failed: bad credentials %s" % O_TOKEN)
        got = self.go()
        self.assertEqual(got["status"], "refused")
        blob = self.stderr.getvalue() + json.dumps(got)
        for path in self.data.rglob("*"):
            if path.is_file():
                blob += path.read_text(errors="replace")
        self.assertNotIn(O_TOKEN, blob)
        self.assertIn("bad credentials", blob)

    def test_the_refusals_write_their_stage_to_stderr(self):
        self.build_raises = fb.BuildError("nope")
        self.go()
        objects = [json.loads(line) for line in self.stderr.getvalue().splitlines()]
        self.assertEqual(objects[-1]["stage"], "refused")
        self.assertEqual(objects[-1]["failed_stage"], "build")


class MainTest(unittest.TestCase):
    ARGV = ["build", "--repo", O_REPO, "--pr", "23096", "--issues", "23090,23091", "--base-image", O_BASE_IMAGE,
            "--template-record", "/t/rec.jsonl", "--template-id", O_TEMPLATE_ID, "--data", "/d"]

    def run_main(self, argv, result=None):
        out, err = io.StringIO(), io.StringIO()
        with mock.patch.object(fb, "run_build", return_value=result or {"status": "built", "exit_code": 0}) as rb, \
                contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
            code = fb.main(argv)
        return code, out.getvalue(), err.getvalue(), rb

    def test_the_arguments_reach_run_build(self):
        code, out, _, rb = self.run_main(self.ARGV)
        self.assertEqual(code, 0)
        kw = rb.call_args.kwargs
        self.assertEqual((kw["repo"], kw["pr"], kw["issue_numbers"]), (O_REPO, 23096, [23090, 23091]))
        self.assertEqual((kw["base_image"], kw["template_id"]), (O_BASE_IMAGE, O_TEMPLATE_ID))
        self.assertEqual((kw["template_path"], kw["data"]), (Path("/t/rec.jsonl"), Path("/d")))
        self.assertIsNone(kw["spec_verdicts_path"])
        self.assertIs(kw["dry_run"], False)
        self.assertEqual(json.loads(out), {"status": "built", "exit_code": 0})

    def test_the_optional_arguments(self):
        _, _, _, rb = self.run_main(self.ARGV + ["--spec-verdicts", "/v.json", "--dry-run"])
        self.assertEqual(rb.call_args.kwargs["spec_verdicts_path"], Path("/v.json"))
        self.assertIs(rb.call_args.kwargs["dry_run"], True)

    def test_the_exit_code_is_the_one_run_build_decided(self):
        for status, code in (("refused", 2), ("awaiting-spec-verdicts", 3), ("unusable", 4), ("cached", 0)):
            with self.subTest(status=status):
                got, out, _, _ = self.run_main(self.ARGV, {"status": status, "exit_code": code})
                self.assertEqual(got, code)
                self.assertEqual(json.loads(out)["status"], status)

    def test_a_bad_argument_value_is_exit_2_with_a_message_and_no_traceback(self):
        with mock.patch.object(fb, "run_build", side_effect=ValueError("pr must be a positive integer")) as rb, \
                contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()) as err:
            self.assertEqual(fb.main(self.ARGV), 2)
        self.assertIn("pr must be a positive integer", err.getvalue())

    def test_missing_or_malformed_arguments_are_a_usage_error(self):
        for argv in ([], ["build"], ["nope"], self.ARGV[:-2], self.ARGV + ["--bogus"],
                     [a if a != "23090,23091" else "x,y" for a in self.ARGV],
                     [a if a != "23096" else "abc" for a in self.ARGV]):
            with self.subTest(argv=argv), contextlib.redirect_stderr(io.StringIO()):
                with self.assertRaises(SystemExit) as cm:
                    fb.main(argv)
                self.assertEqual(cm.exception.code, 2)

    def test_a_dry_run_through_main_prints_the_stages_and_touches_nothing(self):
        with tempfile.TemporaryDirectory() as tmp:
            data = Path(tmp) / "data"
            argv = [a if a != "/d" else str(data) for a in self.ARGV] + ["--dry-run"]
            out, err = io.StringIO(), io.StringIO()
            with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
                code = fb.main(argv)
            self.assertEqual(code, 0)
            self.assertEqual(json.loads(out.getvalue())["stages"], O_STAGES)
            self.assertFalse(data.exists())


if __name__ == "__main__":
    unittest.main()
