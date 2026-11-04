"""Tests for fresh_build's pure helpers. No docker, git, gh or network."""
import hashlib
import unittest

import fresh_build as fb

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
            "a/fixtures/b.txt",
        ):
            self.assertTrue(fb.is_test_path(p), p)

    def test_source_paths(self):
        for p in ("src/lib.rs", "crates/ide/src/contest/mod.rs", "src/attests.rs"):
            self.assertFalse(fb.is_test_path(p), p)


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
            fb.apply_spec_verdicts(["a", "b", "c"], {"b": "specified"})
        msg = str(cm.exception)
        self.assertIn("a", msg)
        self.assertIn("c", msg)

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


if __name__ == "__main__":
    unittest.main()
