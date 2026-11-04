"""Tests for fresh_build: the pure helpers and the upstream fetch. No docker, real git, gh or network."""
import hashlib
import json
import os
import re
import subprocess
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
WARM_OK = (
    "   Compiling stdx v0.0.0 (/testbed/crates/stdx)\n"
    "   Compiling parser v0.0.0 (/testbed/crates/parser)\n"
    "    Finished `test` profile [unoptimized + debuginfo] target(s) in 94.00s\n"
)
HAPPY_LABELS = [
    "run", "clear-tracked", "clear-dirs", "copy", "extract", "touch", "metadata", "warm-build", "git-add",
    "git-commit", "rev-parse", "docker-commit", "inspect", "rm",
]


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
        return "inspect"
    if args[0] == "rmi":
        return "rmi"
    if args[:3] == ("rm", "-f", "-v"):
        return "rm"
    if "git ls-files -z | xargs -0 rm -f" in text:
        return "clear-tracked"
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
    if "rev-parse" in text:
        return "rev-parse"
    if " commit " in text:
        return "git-commit"
    if "git add -A" in text:
        return "git-add"
    return "?"


class FakeDocker:
    """Stands in for pilot.docker: records every call (argv without the leading `docker`, and its
    keyword arguments) and answers by rule. A rule is `(label_or_predicate, response)`; a response
    is a CompletedProcess, an `(exit_code, stdout)` tuple or an exception to raise. The first
    matching rule wins; anything else succeeds with a plausible stdout."""

    def __init__(self, *rules):
        self.rules = list(rules)
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
                return resp
        return done(args, 0, self.default(args))

    @staticmethod
    def default(args):
        name = label(args)
        if name == "rev-parse":
            return SYN_SHA + "\n"
        if name == "inspect":
            return IMAGE_ID + " 4294967296\n"
        if name == "warm-build":
            return WARM_OK
        return ""

    def labels(self):
        return [label(args) for args, _ in self.calls]

    def args_of(self, name):
        found = [args for args, _ in self.calls if label(args) == name]
        assert len(found) == 1, (name, found)
        return found[0]


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
        self.tar = self.tmp / "tree.tar"
        self.tar.write_bytes(b"tar bytes")
        self.evidence = self.tmp / "evidence"

    def build(self, docker, free=lambda: 500.0, **kw):
        args = dict(base_image=BASE_IMAGE, repo=REPO, pr=PR, upstream_base=UPSTREAM_BASE, tree_tar=self.tar,
                    evidence_dir=self.evidence, docker=docker, free_gb=free)
        args.update(kw)
        return fb.build_image(**args)

    def evidence_json(self):
        return json.loads((self.evidence / "build-evidence.json").read_text())

    def fails(self, fake, pattern=r".", committed=False, **kw):
        """Run a build that must raise BuildError; check the invariants every failure shares.

        `committed=True` is for the one failure that happens inside `docker commit` itself.
        """
        with self.assertRaisesRegex(fb.BuildError, pattern) as cm:
            self.build(fake, **kw)
        labels = fake.labels()
        if not committed:
            self.assertNotIn("docker-commit", labels)
        self.assertEqual(labels[-1], "rm")
        self.assertEqual(fake.args_of("rm"), ("rm", "-f", "-v", CONTAINER))
        ev = self.evidence_json()
        self.assertEqual(ev["outcome"], "error")
        self.assertTrue(ev["container_removed"])
        return cm.exception

    # --- the happy path ---

    def test_happy_path_runs_the_stages_in_order(self):
        fake = FakeDocker()
        got = self.build(fake)
        self.assertEqual(fake.labels(), HAPPY_LABELS)
        self.assertEqual(got["image"], TAG)
        self.assertEqual(got["base_commit"], SYN_SHA)
        self.assertEqual(got["image_id"], IMAGE_ID)
        self.assertEqual(got["size_bytes"], 4294967296)
        self.assertEqual(got["compiling_lines"], 2)
        self.assertTrue(got["container_removed"])
        self.assertEqual(set(got["laps_seconds"]), {
            "run", "clear", "extract", "metadata", "warm_build", "git_commit", "image_commit"})
        for secs in got["laps_seconds"].values():
            self.assertGreaterEqual(secs, 0)
        self.assertEqual(set(got), {"image", "base_commit", "image_id", "size_bytes", "laps_seconds",
                                    "compiling_lines", "container_removed"})

    def test_container_is_started_with_network_and_the_pilot_limits(self):
        fake = FakeDocker()
        self.build(fake)
        self.assertEqual(fake.args_of("run"), (
            "run", "-d", "--name", CONTAINER, "--cpus", "4", "--memory", "24g", BASE_IMAGE, "sleep", "infinity"))

    def test_offline_override_is_on_the_warm_build_exec_only(self):
        fake = FakeDocker()
        self.build(fake)
        with_flag = [label(args) for args, _ in fake.calls if "CARGO_NET_OFFLINE=false" in args]
        self.assertEqual(with_flag, ["warm-build"])
        warm = fake.args_of("warm-build")
        i = warm.index("-e")
        self.assertEqual(warm[i:i + 2], ("-e", "CARGO_NET_OFFLINE=false"))
        self.assertLess(i, warm.index(CONTAINER))  # an exec option, before the container name
        self.assertEqual(warm[warm.index(CONTAINER) + 1:], ("cargo", "test", "--no-run"))
        # the committed image keeps the baked offline setting: nothing alters its config
        for name in ("run", "docker-commit"):
            args = fake.args_of(name)
            self.assertNotIn("-e", args)
            self.assertNotIn("--env", args)
            self.assertNotIn("--change", args)
            self.assertFalse([a for a in args if "CARGO_NET_OFFLINE" in a])

    def test_commit_goes_to_the_study_tag_and_the_container_is_removed_last(self):
        fake = FakeDocker()
        self.build(fake)
        self.assertEqual(fake.args_of("docker-commit"), ("commit", CONTAINER, TAG))
        self.assertEqual(fake.args_of("rm"), ("rm", "-f", "-v", CONTAINER))
        self.assertEqual(fake.labels()[-1], "rm")

    def test_every_exec_runs_in_the_workdir_and_the_workdir_is_a_parameter(self):
        for workdir in ("/testbed", "/work/repo"):
            with self.subTest(workdir=workdir):
                fake = FakeDocker()
                self.build(fake, workdir=workdir)
                execs = [args for args, _ in fake.calls if args[0] == "exec"]
                self.assertEqual(len(execs), 9)
                for args in execs:
                    i = args.index("-w")
                    self.assertEqual(args[i + 1], workdir)
                    self.assertEqual(args[i + 2], CONTAINER)

    # --- the tree swap ---

    def test_tracked_files_are_deleted_by_one_pipefail_bash_command(self):
        fake = FakeDocker()
        self.build(fake)
        args = fake.args_of("clear-tracked")
        i = args.index(CONTAINER)
        self.assertEqual(args[i + 1:], ("bash", "-o", "pipefail", "-c", "git ls-files -z | xargs -0 rm -f"))

    def test_empty_directories_are_deleted_except_target_and_git(self):
        fake = FakeDocker()
        self.build(fake)
        args = fake.args_of("clear-dirs")
        i = args.index(CONTAINER)
        self.assertEqual(args[i + 1:i + 4], ("bash", "-o", "pipefail"))
        script = args[-1]
        for part in ("find .", "-mindepth 1", "-depth", "-type d", "-empty", "-delete", "while"):
            self.assertIn(part, script)
        for guarded in ("./target", "./target/*", "./.git", "./.git/*"):
            self.assertIn("-not -path '%s'" % guarded, script)

    def test_the_empty_directory_script_removes_nested_empties_and_spares_target_and_git(self):
        # the real shell text, run on a scratch tree: the one place it is verified
        fake = FakeDocker()
        self.build(fake)
        script = fake.args_of("clear-dirs")[-1]
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

    def test_tree_is_copied_in_extracted_over_the_workdir_and_the_tarball_is_removed(self):
        fake = FakeDocker()
        self.build(fake)
        self.assertEqual(fake.args_of("copy"), ("cp", str(self.tar), CONTAINER + ":/tmp/sab-tree.tar"))
        script = fake.args_of("extract")[-1]
        self.assertIn("tar -xf /tmp/sab-tree.tar", script)
        self.assertIn("rm -f /tmp/sab-tree.tar", script)  # else it would be baked into the image

    def test_extracted_files_are_touched_after_extraction_and_before_the_build(self):
        fake = FakeDocker()
        self.build(fake)
        labels = fake.labels()
        self.assertEqual(labels.index("extract") + 1, labels.index("touch"))
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
        fake = FakeDocker()
        self.build(fake)
        for args, _ in fake.calls:
            text = " ".join(args)
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

    def test_warm_build_log_is_kept_in_the_evidence_dir_byte_for_byte(self):
        out = "   Compiling a v1\r\n   Compiling b v1\né line\n"
        fake = FakeDocker(("warm-build", (0, out)))
        got = self.build(fake)
        self.assertEqual((self.evidence / "build.log").read_bytes(), out.encode("utf-8"))
        self.assertEqual(got["compiling_lines"], 2)

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

    # --- later stages ---

    def test_git_commit_is_synthetic_with_a_fixed_identity_and_message(self):
        fake = FakeDocker()
        self.build(fake)
        add = fake.args_of("git-add")
        self.assertEqual(add[add.index(CONTAINER) + 1:], ("git", "add", "-A"))
        args = fake.args_of("git-commit")
        tail = args[args.index(CONTAINER) + 1:]
        self.assertEqual(tail[:6], ("git", "-c", "user.name=sab", "-c", "user.email=sab@invalid", "commit"))
        msg = "synthetic base: %s %s (parent of PR %d merge)" % (REPO, UPSTREAM_BASE[:12], PR)
        self.assertEqual(tail[tail.index("-m") + 1], msg)

    def test_base_commit_is_the_sha_the_container_reports(self):
        fake = FakeDocker(("rev-parse", (0, "%s\n" % ("ab" * 20))))
        self.assertEqual(self.build(fake)["base_commit"], "ab" * 20)

    def test_a_malformed_rev_parse_answer_raises(self):
        fake = FakeDocker(("rev-parse", (0, "fatal: not a git repository\n")))
        self.fails(fake, "rev-parse")

    def test_docker_commit_failure_raises_and_leaves_no_tag_behind(self):
        fake = FakeDocker(("docker-commit", (1, "no space left on device")))
        self.fails(fake, "no space left", committed=True)
        self.assertNotIn("inspect", fake.labels())
        self.assertNotIn("rmi", fake.labels())

    def test_docker_commit_with_no_answer_drops_a_tag_the_daemon_may_have_made(self):
        fake = FakeDocker(("docker-commit", subprocess.TimeoutExpired(cmd=["docker"], timeout=3600)))
        self.fails(fake, "timed out", committed=True)
        self.assertEqual(fake.args_of("rmi"), ("rmi", TAG))
        self.assertLess(fake.labels().index("rmi"), fake.labels().index("rm"))

    def test_image_inspect_failure_after_commit_removes_the_tag_it_just_created(self):
        fake = FakeDocker(("inspect", (1, "no such image")))
        self.fails(fake, "inspect", committed=True)
        self.assertEqual(fake.args_of("rmi"), ("rmi", TAG))

    def test_malformed_inspect_output_raises_and_removes_the_new_tag(self):
        fake = FakeDocker(("inspect", (0, "garbage\n")))
        self.fails(fake, "inspect", committed=True)
        self.assertEqual(fake.args_of("rmi"), ("rmi", TAG))

    # --- every step's own exit code decides ---

    def test_every_step_with_a_nonzero_exit_raises_stops_the_build_and_removes_the_container(self):
        steps = ["run", "clear-tracked", "clear-dirs", "copy", "extract", "touch", "metadata", "warm-build",
                 "git-add", "git-commit", "rev-parse", "docker-commit"]
        for name in steps:
            with self.subTest(step=name):
                fake = FakeDocker((name, (7, "step failed here")))
                err = self.fails(fake, "exit 7", committed=name == "docker-commit")
                self.assertIn("step failed here", str(err))
                self.assertEqual(fake.labels()[-2], name)  # nothing ran after the failed step but the removal

    def test_a_step_timeout_is_a_build_error_and_the_container_is_removed(self):
        for name in ("run", "extract", "warm-build", "docker-commit"):
            with self.subTest(step=name):
                fake = FakeDocker((name, subprocess.TimeoutExpired(cmd=["docker"], timeout=5)))
                self.fails(fake, "timed out", committed=name == "docker-commit")

    def test_a_missing_docker_binary_is_a_build_error(self):
        self.fails(FakeDocker(("run", FileNotFoundError("docker"))), "docker")

    def test_an_interrupt_still_removes_the_container(self):
        fake = FakeDocker(("warm-build", KeyboardInterrupt()))
        with self.assertRaises(KeyboardInterrupt):
            self.build(fake)
        self.assertEqual(fake.labels()[-1], "rm")
        self.assertNotIn("docker-commit", fake.labels())
        self.assertEqual(self.evidence_json()["outcome"], "error")

    def test_exit_codes_are_read_from_the_completed_process_not_a_shell(self):
        fake = FakeDocker()
        self.build(fake)
        for args, kw in fake.calls:
            self.assertIs(kw.get("check"), False, args)  # read here, never raised by the docker wrapper
            self.assertIn("timeout", kw)
            joined = " ".join(args)
            self.assertNotIn("| tee", joined)
            self.assertNotIn("exit 0", joined)

    # --- disk ---

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

    # --- argument checks, all before docker ---

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

    def test_a_missing_tree_tarball_is_refused_before_any_docker_call(self):
        fake = FakeDocker()
        with self.assertRaisesRegex(fb.BuildError, "tree"):
            self.build(fake, tree_tar=self.tmp / "absent.tar")
        self.assertEqual(fake.calls, [])

    # --- evidence and removal ---

    def test_evidence_of_a_good_build(self):
        fake = FakeDocker()
        got = self.build(fake)
        ev = self.evidence_json()
        self.assertEqual(ev["outcome"], "ok")
        self.assertIsNone(ev["error"])
        self.assertEqual(ev["image"], TAG)
        self.assertEqual(ev["container"], CONTAINER)
        self.assertEqual(ev["base_image"], BASE_IMAGE)
        self.assertEqual(ev["upstream_base"], UPSTREAM_BASE)
        self.assertEqual(ev["builder_version"], fb.BUILDER_VERSION)
        self.assertEqual(ev["base_commit"], SYN_SHA)
        self.assertEqual(ev["image_id"], IMAGE_ID)
        self.assertEqual(ev["compiling_lines"], 2)
        self.assertTrue(ev["container_removed"])
        self.assertEqual(ev["laps_seconds"], got["laps_seconds"])
        self.assertEqual([s["name"] for s in ev["steps"]], [n for n in HAPPY_LABELS if n != "rm"])
        self.assertTrue(all(s["returncode"] == 0 for s in ev["steps"]))

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
        self.assertEqual([args for args, _ in fake.calls if args[0] == "rm"], [("rm", "-f", "-v", CONTAINER)])


if __name__ == "__main__":
    unittest.main()
