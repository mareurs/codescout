"""Tests for leak_scan.py: the gate that keeps the answer out of every byte an agent can reach."""
import json
import os
import sqlite3
import subprocess
import tempfile
import unittest
from pathlib import Path

import leak_scan
from leak_scan import ControlFailed, EmptyScan, Hit, NoBaseline, ScanIncomplete

GOLD_LINE = "let mut result = compute_total(alpha, beta, gamma, delta);"  # 8 tokens
GOLD_NEEDLE = "let mut result compute_total alpha beta gamma delta"
HIDDEN_PATH = "src/librarian/adapter_progress_tests.rs"
HIDDEN_TEXT = "fn the_updates_also_advance() {\n    check_progress(one_item, two_items, three_items, four_items, five_items);\n}\n"
S5_TASK = {
    "id": "cs-p20",
    "f2p": {"names": [
        "librarian::adapter::s5h_adapter_progress::a_reembedding_reindex_keeps_a_caller_informed",
        "librarian::adapter::s5h_adapter_progress::the_updates_also_advance",
    ]},
    "hidden": {"files": ["src/tools/tests.rs"], "extra": [HIDDEN_PATH]},
}
S5_NO_EXTRA = {"id": "t", "f2p": {"names": ["m::the_test_name"]}, "hidden": {"files": [], "extra": []}}
HIDDEN = {HIDDEN_PATH: HIDDEN_TEXT}


def diff_adding(*lines, path="src/x.rs"):
    body = "".join(f"+{line}\n" for line in lines)
    return f"diff --git a/{path} b/{path}\n--- a/{path}\n+++ b/{path}\n@@ -1,1 +1,{len(lines) + 1} @@\n ctx\n{body}"


def S(roots, dump, needles, **kw):
    """Mechanics tests use synthetic needles; the baseline requirement is tested separately."""
    return leak_scan.scan(roots, dump, needles, allow_unbaselined=True, **kw)


class Tmp(unittest.TestCase):
    def setUp(self):
        self._td = tempfile.TemporaryDirectory()
        self.addCleanup(self._td.cleanup)
        self.root = Path(self._td.name) / "tree"
        self.root.mkdir()

    def put(self, rel, data):
        p = self.root / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_bytes(data if isinstance(data, bytes) else data.encode())
        return p


class ScanTests(Tmp):
    def test_planted_needle_is_found_in_file_bytes(self):
        p = self.put("src/lib.rs", "fn main() {}\n// calls a_hidden_test_name here\n")
        hits = S([self.root], None, ["a_hidden_test_name"]).hits
        self.assertEqual([(h.path, h.needle) for h in hits], [(p, "a_hidden_test_name")])
        self.assertEqual(p.read_bytes()[hits[0].offset:].split()[0], b"a_hidden_test_name")

    def test_planted_needle_is_found_in_a_qdrant_payload_dump(self):
        dump = Path(self._td.name) / "payload.jsonl"
        rec = {"payload": {"text": "x\nthe_hidden_one()\nlet mut result = compute_total(alpha,\n  beta, gamma, delta);"}}
        dump.write_text(json.dumps(rec) + "\n")
        self.assertIn("\\nthe_hidden_one", dump.read_text())
        self.put("a.txt", "clean")
        hits = S([self.root], dump, ["the_hidden_one", GOLD_NEEDLE]).hits
        self.assertEqual({(h.path, h.needle) for h in hits}, {(dump, "the_hidden_one"), (dump, GOLD_NEEDLE)})

    def test_needle_in_a_file_name_is_found(self):
        p = self.put("tests/the_hidden_one_tests.rs", "nothing here")
        self.assertEqual(S([self.root], None, ["the_hidden_one_tests"]).hits, [Hit(p, "the_hidden_one_tests", -1)])

    def test_path_needle_matches_relative_path(self):
        p = self.put("src/librarian/adapter_progress_tests.rs", "x")
        self.put("src/other/tests.rs", "y")
        hits = S([self.root], None, [HIDDEN_PATH]).hits
        self.assertEqual([h.path for h in hits], [p])

    def test_clean_tree_returns_no_hits_and_counts(self):
        self.put("src/lib.rs", "fn main() { println!(\"hi\"); }\n")
        self.put("README.md", "# readme\n")
        rep = S([self.root], None, ["a_hidden_test_name", GOLD_NEEDLE])
        self.assertEqual(rep.hits, [])
        self.assertEqual(rep.files_scanned, 2)
        self.assertGreater(rep.bytes_scanned, 0)
        self.assertEqual(rep.needle_counts, {"name": 1, "path": 0, "shingle": 1})
        self.assertEqual(rep.per_root[0][1], 2)

    def test_whole_word_matching(self):
        self.put("a.rs", "fn foobar() {}\nfn foo_bar() {}\nlet xfoo = 1;\n")
        self.assertEqual(S([self.root], None, ["foo"]).hits, [])
        p = self.put("b.rs", "assert!(foo());\n")
        self.assertEqual([h.path for h in S([self.root], None, ["foo"]).hits], [p])

    def test_whole_word_in_file_names(self):
        self.put("foobar.rs", "x")
        self.assertEqual(S([self.root], None, ["foo"]).hits, [])
        p = self.put("foo.rs", "x")
        self.assertEqual([h.path for h in S([self.root], None, ["foo"]).hits], [p])

    def test_binary_sqlite_bytes_are_scanned(self):
        db = self.root / "home" / "catalog.db"
        db.parent.mkdir()
        con = sqlite3.connect(db)
        con.execute("create table t(body text)")
        con.execute("insert into t values (?)", ("note: see the_hidden_one for details",))
        con.commit()
        con.close()
        self.assertEqual([(h.path, h.needle) for h in S([self.root], None, ["the_hidden_one"]).hits],
                         [(db, "the_hidden_one")])

    def test_binary_with_invalid_utf8_does_not_crash(self):
        self.put("blob.bin", b"\xff\xfe\x00\x80 the_hidden_one \xc3\x28")
        self.assertEqual(len(S([self.root], None, ["the_hidden_one"]).hits), 1)

    def test_json_escaped_name_is_found_in_any_file_type(self):
        """I6: a name after an escaped newline is not hidden in a .md file or a SQLite value."""
        md = self.put("note.md", 'see "x\\nthe_hidden_one"')
        db = self.root / "catalog.db"
        con = sqlite3.connect(db)
        con.execute("create table t(j text)")
        con.execute("insert into t values (?)", (json.dumps({"body": "line\nthe_hidden_one()"}),))
        con.commit()
        con.close()
        got = {h.path for h in S([self.root], None, ["the_hidden_one"]).hits}
        self.assertEqual(got, {md, db})

    def test_shingle_survives_reformatting(self):
        self.put("a.rs", "let   mut result =\n        compute_total(\n  alpha,   beta,\n gamma, delta\n);\n")
        self.assertEqual(len(S([self.root], None, [GOLD_NEEDLE]).hits), 1)

    def test_shingle_does_not_match_a_different_token_sequence(self):
        self.put("a.rs", "let mut result = compute_total(alpha, beta, gamma, epsilon);\n")
        self.assertEqual(S([self.root], None, [GOLD_NEEDLE]).hits, [])

    def test_zero_files_scanned_is_not_clean(self):
        with self.assertRaises(EmptyScan):
            S([], None, ["x"])
        empty = self.root / "empty"
        empty.mkdir()
        with self.assertRaises(EmptyScan):
            S([empty], None, ["x"])

    def test_one_empty_root_among_several_is_not_clean(self):
        """I5"""
        self.put("a.txt", "x")
        empty = Path(self._td.name) / "empty"
        empty.mkdir()
        with self.assertRaises(EmptyScan):
            S([self.root, empty], None, ["zzz"])

    def test_missing_root_is_an_error_not_a_clean_scan(self):
        with self.assertRaises(FileNotFoundError):
            S([self.root / "nope"], None, ["x"])

    def test_qdrant_dump_counts_as_scanned(self):
        dump = Path(self._td.name) / "d.json"
        dump.write_text("[]")
        rep = S([], dump, ["x"])
        self.assertEqual((rep.files_scanned, rep.bytes_scanned), (1, 2))
        self.assertEqual(rep.per_root, [(f"qdrant:{dump}", 1, 2)])

    def test_bad_needles_are_refused(self):
        self.put("a", "x")
        for bad in ([], [""], ["too short shingle"], ["let mut result = compute_total(alpha, beta, gamma, delta);"],
                    ["one two three four five six seven eight-nine ten"]):
            with self.assertRaises(ValueError, msg=repr(bad)):
                S([self.root], None, bad)

    def test_dir_symlink_loop_terminates(self):
        self.put("a/f.txt", "the_hidden_one")
        (self.root / "a" / "loop").symlink_to(self.root, target_is_directory=True)
        self.assertEqual(len(S([self.root], None, ["the_hidden_one"]).hits), 1)

    def test_broken_symlink_makes_a_strict_scan_raise(self):
        (self.root / "dead").symlink_to(self.root / "missing")
        self.put("ok.txt", "x")
        with self.assertRaises(ScanIncomplete):
            S([self.root], None, ["zzz"])

    @unittest.skipIf(os.geteuid() == 0, "root ignores mode 000")
    def test_unreadable_directory_makes_a_strict_scan_raise(self):
        """I4"""
        d = self.root / "locked"
        d.mkdir()
        (d / "f.txt").write_text("the_hidden_one")
        self.put("ok.txt", "x")
        os.chmod(d, 0)
        self.addCleanup(os.chmod, d, 0o755)
        with self.assertRaises(ScanIncomplete) as cm:
            S([self.root], None, ["the_hidden_one"])
        self.assertIn("locked", str(cm.exception))

    def test_fifo_is_skipped_not_hung_on(self):
        os.mkfifo(self.root / "pipe")
        self.put("ok.txt", "x")
        with self.assertRaises(ScanIncomplete) as cm:
            S([self.root], None, ["zzz"])
        self.assertIn("pipe", str(cm.exception))

    def test_raw_needles_without_baseline_are_refused(self):
        self.put("a.txt", "x")
        with self.assertRaises(NoBaseline):
            leak_scan.scan([self.root], None, ["zzz"])


class BaselineTests(Tmp):
    def test_baseline_drops_needles_the_clean_parent_already_has(self):
        self.put("docs/archive.md", "moved: " + GOLD_LINE + "\nfn already_there() {}\n")
        self.put("src/librarian/adapter_progress_tests.rs", "// old version\n")
        raw = ["already_there", "fresh_name", GOLD_NEEDLE, "one_a two_b three_c four_d five_e six_f seven_g eight_h",
               HIDDEN_PATH]
        ns = leak_scan.baseline_needles(raw, self.root)
        self.assertEqual(ns.needles, ["fresh_name", "one_a two_b three_c four_d five_e six_f seven_g eight_h"])
        self.assertEqual(ns.dropped, {"name": 1, "path": 1, "shingle": 1})
        self.assertEqual(ns.kept, {"name": 1, "path": 0, "shingle": 1})
        self.assertEqual(ns.baseline_files, 2)
        self.assertEqual(leak_scan.scan([self.root], None, ns).hits, [])

    def test_baseline_accepts_a_callable_yielding_bytes(self):
        files = [("a/b.rs", b"fn already_there() {}")]
        ns = leak_scan.baseline_needles(["already_there", "fresh_name"], lambda: iter(files))
        self.assertEqual(ns.needles, ["fresh_name"])
        self.assertEqual((ns.baseline_files, ns.baseline_bytes), (1, 21))

    def test_empty_baseline_is_refused(self):
        with self.assertRaises(EmptyScan):
            leak_scan.baseline_needles(["x_name"], lambda: iter([]))
        with self.assertRaises(EmptyScan):
            leak_scan.baseline_needles(["x_name"], self.root)

    def test_baseline_that_removes_every_shingle_is_refused_unless_allowed(self):
        self.put("a.rs", GOLD_LINE)
        with self.assertRaises(ValueError):
            leak_scan.baseline_needles([GOLD_NEEDLE, "name_x"], self.root)
        ns = leak_scan.baseline_needles([GOLD_NEEDLE, "name_x"], self.root, allow_no_shingles=True)
        self.assertEqual(ns.needles, ["name_x"])

    def test_positive_control_finds_each_planted_kind_and_returns_real_report(self):
        self.put("a.rs", "fn clean() {}\n")
        parent = Path(self._td.name) / "parent"
        parent.mkdir()
        (parent / "p.txt").write_text("x")
        raw = ["fresh_name", GOLD_NEEDLE, "src/hidden/new_tests.rs"]
        rep = leak_scan.positive_control([self.root], raw, parent)
        self.assertEqual(rep.hits, [])
        self.assertEqual((rep.files_scanned, len(rep.per_root)), (1, 1))
        self.assertEqual(rep.needle_counts, {"name": 1, "path": 1, "shingle": 1})
        leaked = self.put("leak.rs", "x " + GOLD_LINE)
        rep = leak_scan.positive_control([self.root], raw, parent)
        self.assertEqual([(h.path, h.needle) for h in rep.hits], [(leaked, GOLD_NEEDLE)])

    def test_positive_control_fails_loudly_when_the_scanner_misses(self):
        parent = Path(self._td.name) / "parent"
        parent.mkdir()
        (parent / "p.txt").write_text("x")
        self.put("a.rs", "x")
        real = leak_scan._Matcher.matches
        leak_scan._Matcher.matches = lambda self, rel, data: []
        try:
            with self.assertRaises(ControlFailed):
                leak_scan.positive_control([self.root], ["fresh_name"], parent)
        finally:
            leak_scan._Matcher.matches = real


class NeedlesTests(unittest.TestCase):
    def shingles(self, needles):
        return [n for n in needles if " " in n]

    def test_short_common_lines_are_not_needles(self):
        patch = diff_adding("}", "    Ok(())", "    });", "", "    // x y")
        needles = leak_scan.needles_for(S5_NO_EXTRA, patch, allow_no_shingles=True)
        self.assertEqual(self.shingles(needles), [])
        self.assertEqual(needles, ["the_test_name"])

    def test_added_tokens_without_any_shingle_are_refused_unless_allowed(self):
        with self.assertRaises(ValueError):
            leak_scan.needles_for(S5_NO_EXTRA, diff_adding("}", "Ok(())"))
        leak_scan.needles_for(S5_NO_EXTRA, diff_adding())  # nothing added: nothing to refuse

    def test_long_line_of_only_common_tokens_is_not_a_needle(self):
        line = "pub fn use let mut self return if else for in Some None Ok Err"
        self.assertGreaterEqual(len(line.split()), 8)
        self.assertEqual(self.shingles(leak_scan.needles_for(S5_NO_EXTRA, diff_adding(line), allow_no_shingles=True)), [])

    def test_shingles_of_an_added_line(self):
        self.assertEqual(self.shingles(leak_scan.needles_for(S5_NO_EXTRA, diff_adding(GOLD_LINE))), [GOLD_NEEDLE])
        long = "one_aa two_bb three_cc four_dd five_ee six_ff seven_gg eight_hh nine_ii ten_jj"
        sh = self.shingles(leak_scan.needles_for(S5_NO_EXTRA, diff_adding(long)))
        self.assertEqual(len(sh), 3)
        self.assertEqual(sh[0], "one_aa two_bb three_cc four_dd five_ee six_ff seven_gg eight_hh")

    def test_shingles_span_consecutive_added_lines_but_not_other_lines(self):
        patch = diff_adding("let mut result = compute_total(alpha,", "    beta, gamma, delta);")
        self.assertEqual(self.shingles(leak_scan.needles_for(S5_NO_EXTRA, patch)), [GOLD_NEEDLE])
        split = ("--- a/f\n+++ b/f\n@@ -1 +1 @@\n+let mut result = compute_total(alpha,\n"
                 " context\n+    beta, gamma, delta);\n")
        self.assertEqual(leak_scan.needles_for(S5_NO_EXTRA, split, allow_no_shingles=True), ["the_test_name"])

    def test_removed_and_context_lines_and_headers_are_not_needles(self):
        patch = "--- a/f\n+++ b/f\n@@ -1 +1 @@\n-" + GOLD_LINE + "\n " + GOLD_LINE + "\n"
        self.assertEqual(self.shingles(leak_scan.needles_for(S5_NO_EXTRA, patch)), [])

    def test_s5_task_needles(self):
        needles = leak_scan.needles_for(S5_TASK, diff_adding(), HIDDEN)
        self.assertIn("a_reembedding_reindex_keeps_a_caller_informed", needles)
        self.assertIn("the_updates_also_advance", needles)
        self.assertIn(HIDDEN_PATH, needles)
        self.assertNotIn("src/tools/tests.rs", needles)  # hidden.files exist in the tree legitimately
        self.assertIn("fn the_updates_also_advance check_progress one_item two_items three_items four_items five_items", needles)
        self.assertEqual(len(needles), len(set(needles)))

    def test_s5_without_patch_text_or_hidden_texts_is_refused(self):
        with self.assertRaises(ValueError):
            leak_scan.needles_for(S5_TASK)
        with self.assertRaises(ValueError) as cm:
            leak_scan.needles_for(S5_TASK, diff_adding(GOLD_LINE))
        self.assertIn("hidden.extra", str(cm.exception))

    def test_swebench_record_needles(self):
        rec = {
            "FAIL_TO_PASS": "['org.apache.druid.Test#testPow', 'tests/x.py::TestA::test_b[p1]', 'test_c (mod.Cls)']",
            "test_patch": diff_adding("    public void testPow() {", "    def test_extra(self):", path="T.java"),
            "patch": diff_adding(GOLD_LINE),
        }
        needles = leak_scan.needles_for(rec)
        for n in ("testPow", "test_b", "test_c", "test_extra", GOLD_NEEDLE):
            self.assertIn(n, needles)

    def test_swebench_f2p_as_list(self):
        rec = {"FAIL_TO_PASS": ["a/b.py::test_z"], "test_patch": "", "patch": ""}
        self.assertEqual(leak_scan.needles_for(rec), ["test_z"])

    def test_unknown_shape_is_refused(self):
        with self.assertRaises(ValueError):
            leak_scan.needles_for({"id": "x"})

    def test_needles_round_trip_through_baseline_and_scan(self):
        with tempfile.TemporaryDirectory() as td:
            root, parent = Path(td) / "t", Path(td) / "p"
            root.mkdir()
            parent.mkdir()
            (parent / "base.rs").write_text("fn base() {}\n")
            (root / "leak.rs").write_text("fn t() {\n    " + GOLD_LINE + "\n}\n")
            raw = leak_scan.needles_for(S5_NO_EXTRA, diff_adding(GOLD_LINE, "}"))
            ns = leak_scan.baseline_needles(raw, parent)
            self.assertEqual([h.needle for h in leak_scan.scan([root], None, ns).hits], [GOLD_NEEDLE])


TASK_DIR = Path("/home/marius/work/claude/claude-plugins/effort-steering/spikes/s5/tasks/cs-p20")


class RealTaskEndToEnd(unittest.TestCase):
    """cs-p20, read-only git: a clean parent reads 0 hits; one planted needle reads exactly one."""

    def setUp(self):
        try:
            self.task = json.loads((TASK_DIR / "task.json").read_text())
            self.repo = self.task["repoPath"]
            for rev in (self.task["parent"], self.task["fix"]):
                subprocess.run(["git", "-C", self.repo, "cat-file", "-e", rev + "^{commit}"], check=True, capture_output=True)
        except (OSError, subprocess.CalledProcessError, KeyError) as e:
            self.skipTest(f"cs-p20 task or its repo commits unavailable: {e}")

    def test_clean_parent_is_zero_and_a_planted_needle_is_the_only_hit(self):
        t = self.task
        patch = subprocess.run(["git", "-C", self.repo, "diff", t["parent"], t["fix"]], check=True,
                               capture_output=True).stdout.decode("utf-8", "replace")
        hidden = {p: (TASK_DIR / "extra-tests" / p).read_text() for p in t["hidden"]["extra"]}
        with tempfile.TemporaryDirectory() as td:
            tree = Path(td) / "parent"
            tree.mkdir()
            archive = subprocess.Popen(["git", "-C", self.repo, "archive", t["parent"]], stdout=subprocess.PIPE)
            subprocess.run(["tar", "-x", "-C", str(tree)], stdin=archive.stdout, check=True)
            archive.stdout.close()
            self.assertEqual(archive.wait(), 0)
            raw = leak_scan.needles_for(t, patch, hidden)
            ns = leak_scan.baseline_needles(raw, tree)
            self.assertGreater(ns.dropped["shingle"], 0)  # the reviewed defect: shingles already in the parent
            self.assertGreater(ns.kept["shingle"], 0)
            rep = leak_scan.scan([tree], None, ns)
            self.assertEqual(rep.hits, [])
            self.assertGreater(rep.files_scanned, 100)
            shingle = next(n for n in ns.needles if " " in n)
            planted = tree / "planted_leak.txt"
            planted.write_text("// leaked\n" + "\n".join(shingle.split()) + "\n")
            hits = leak_scan.scan([tree], None, ns).hits
            self.assertEqual([(h.path, h.needle) for h in hits], [(planted, shingle)])
            planted.unlink()
            self.assertEqual(leak_scan.positive_control([tree], ns, tree).hits, [])


if __name__ == "__main__":
    unittest.main()
