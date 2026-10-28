"""Tests for leak_scan.py: the gate that keeps the answer out of every byte an agent can reach."""
import json
import sqlite3
import tempfile
import unittest
from pathlib import Path

import leak_scan
from leak_scan import EmptyScan, Hit

GOLD_LINE = "let mut result = compute_total(alpha, beta, gamma, delta);"  # 8 tokens
S5_TASK = {
    "id": "cs-p20",
    "f2p": {"names": [
        "librarian::adapter::s5h_adapter_progress::a_reembedding_reindex_keeps_a_caller_informed",
        "librarian::adapter::s5h_adapter_progress::the_updates_also_advance",
    ]},
    "hidden": {"files": ["src/tools/tests.rs"], "extra": ["src/librarian/adapter_progress_tests.rs"]},
}


def diff_adding(*lines, path="src/x.rs"):
    body = "".join(f"+{line}\n" for line in lines)
    return f"diff --git a/{path} b/{path}\n--- a/{path}\n+++ b/{path}\n@@ -1,1 +1,{len(lines) + 1} @@\n ctx\n{body}"


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
        hits = leak_scan.scan([self.root], None, ["a_hidden_test_name"])
        self.assertEqual([(h.path, h.needle) for h in hits], [(p, "a_hidden_test_name")])
        self.assertEqual(p.read_bytes()[hits[0].offset:].split()[0], b"a_hidden_test_name")

    def test_planted_needle_is_found_in_a_qdrant_payload_dump(self):
        dump = Path(self._td.name) / "payload.jsonl"
        # JSON escapes: the name follows a literal \n, the shingle spans an escaped newline.
        rec = {"payload": {"text": "x\nthe_hidden_one()\nlet mut result = compute_total(alpha,\n  beta, gamma, delta);"}}
        dump.write_text(json.dumps(rec) + "\n")
        self.assertIn("\\nthe_hidden_one", dump.read_text())
        self.put("a.txt", "clean")
        shingle = "let mut result compute_total alpha beta gamma delta"
        hits = leak_scan.scan([self.root], dump, ["the_hidden_one", shingle])
        self.assertEqual({(h.path, h.needle) for h in hits}, {(dump, "the_hidden_one"), (dump, shingle)})

    def test_needle_in_a_file_name_is_found(self):
        p = self.put("tests/the_hidden_one_tests.rs", "nothing here")
        hits = leak_scan.scan([self.root], None, ["the_hidden_one_tests"])
        self.assertEqual(hits, [Hit(p, "the_hidden_one_tests", -1)])

    def test_path_needle_matches_relative_path(self):
        p = self.put("src/librarian/adapter_progress_tests.rs", "x")
        self.put("src/other/tests.rs", "y")
        hits = leak_scan.scan([self.root], None, ["src/librarian/adapter_progress_tests.rs"])
        self.assertEqual([h.path for h in hits], [p])

    def test_clean_tree_returns_no_hits(self):
        self.put("src/lib.rs", "fn main() { println!(\"hi\"); }\n")
        self.put("README.md", "# readme\n")
        rep = leak_scan.scan_report([self.root], None, ["a_hidden_test_name", GOLD_NEEDLE])
        self.assertEqual(rep.hits, [])
        self.assertEqual(rep.files_scanned, 2)
        self.assertGreater(rep.bytes_scanned, 0)
        self.assertEqual(leak_scan.scan([self.root], None, ["a_hidden_test_name"]), [])

    def test_whole_word_matching(self):
        self.put("a.rs", "fn foobar() {}\nfn foo_bar() {}\nlet xfoo = 1;\n")
        self.assertEqual(leak_scan.scan([self.root], None, ["foo"]), [])
        p = self.put("b.rs", "assert!(foo());\n")
        self.assertEqual([h.path for h in leak_scan.scan([self.root], None, ["foo"])], [p])

    def test_whole_word_in_file_names(self):
        self.put("foobar.rs", "x")
        self.assertEqual(leak_scan.scan([self.root], None, ["foo"]), [])
        p = self.put("foo.rs", "x")
        self.assertEqual([h.path for h in leak_scan.scan([self.root], None, ["foo"])], [p])

    def test_binary_sqlite_bytes_are_scanned(self):
        db = self.root / "home" / "catalog.db"
        db.parent.mkdir()
        con = sqlite3.connect(db)
        con.execute("create table t(body text)")
        con.execute("insert into t values (?)", ("note: see the_hidden_one for details",))
        con.commit()
        con.close()
        hits = leak_scan.scan([self.root], None, ["the_hidden_one"])
        self.assertEqual([(h.path, h.needle) for h in hits], [(db, "the_hidden_one")])

    def test_binary_with_invalid_utf8_does_not_crash(self):
        self.put("blob.bin", b"\xff\xfe\x00\x80 the_hidden_one \xc3\x28")
        self.assertEqual(len(leak_scan.scan([self.root], None, ["the_hidden_one"])), 1)

    def test_shingle_survives_reformatting(self):
        self.put("a.rs", "let   mut result =\n        compute_total(\n  alpha,   beta,\n gamma, delta\n);\n")
        hits = leak_scan.scan([self.root], None, [GOLD_NEEDLE])
        self.assertEqual(len(hits), 1)

    def test_shingle_does_not_match_a_different_token_sequence(self):
        self.put("a.rs", "let mut result = compute_total(alpha, beta, gamma, epsilon);\n")
        self.assertEqual(leak_scan.scan([self.root], None, [GOLD_NEEDLE]), [])

    def test_zero_files_scanned_is_not_clean(self):
        with self.assertRaises(EmptyScan):
            leak_scan.scan([], None, ["x"])
        empty = self.root / "empty"
        empty.mkdir()
        with self.assertRaises(EmptyScan):
            leak_scan.scan([empty], None, ["x"])
        rep = leak_scan.scan_report([empty], None, ["x"])
        self.assertEqual((rep.hits, rep.files_scanned), ([], 0))

    def test_missing_root_is_an_error_not_a_clean_scan(self):
        with self.assertRaises(FileNotFoundError):
            leak_scan.scan([self.root / "nope"], None, ["x"])

    def test_qdrant_dump_counts_as_scanned(self):
        dump = Path(self._td.name) / "d.json"
        dump.write_text("[]")
        rep = leak_scan.scan_report([], dump, ["x"])
        self.assertEqual((rep.files_scanned, rep.bytes_scanned), (1, 2))

    def test_bad_needles_are_refused(self):
        self.put("a", "x")
        for bad in ([], [""], ["too short shingle"]):
            with self.assertRaises(ValueError, msg=repr(bad)):
                leak_scan.scan([self.root], None, bad)

    def test_dir_symlink_loop_terminates(self):
        self.put("a/f.txt", "the_hidden_one")
        (self.root / "a" / "loop").symlink_to(self.root, target_is_directory=True)
        rep = leak_scan.scan_report([self.root], None, ["the_hidden_one"])
        self.assertEqual(len(rep.hits), 1)

    def test_broken_symlink_is_reported_as_skipped(self):
        (self.root / "dead").symlink_to(self.root / "missing")
        self.put("ok.txt", "x")
        rep = leak_scan.scan_report([self.root], None, ["zzz"])
        self.assertEqual([p.name for p, _ in rep.skipped], ["dead"])
        with self.assertRaises(leak_scan.ScanIncomplete):
            leak_scan.scan([self.root], None, ["zzz"])


GOLD_NEEDLE = "let mut result compute_total alpha beta gamma delta"


class NeedlesTests(unittest.TestCase):
    def test_short_common_lines_are_not_needles(self):
        patch = diff_adding("}", "    Ok(())", "    });", "", "    // x y")
        self.assertEqual(leak_scan.needles_for(S5_TASK, patch), leak_scan.needles_for(S5_TASK, diff_adding()))
        shingles = [n for n in leak_scan.needles_for(S5_TASK, patch) if " " in n]
        self.assertEqual(shingles, [])

    def test_long_line_of_only_common_tokens_is_not_a_needle(self):
        line = "pub fn use let mut self return if else for in Some None Ok Err"
        self.assertGreaterEqual(len(line.split()), 8)
        shingles = [n for n in leak_scan.needles_for(S5_TASK, diff_adding(line)) if " " in n]
        self.assertEqual(shingles, [])

    def test_shingles_of_an_added_line(self):
        shingles = [n for n in leak_scan.needles_for(S5_TASK, diff_adding(GOLD_LINE)) if " " in n]
        self.assertEqual(shingles, [GOLD_NEEDLE])
        long = "one_aa two_bb three_cc four_dd five_ee six_ff seven_gg eight_hh nine_ii ten_jj"
        shingles = [n for n in leak_scan.needles_for(S5_TASK, diff_adding(long)) if " " in n]
        self.assertEqual(len(shingles), 3)
        self.assertEqual(shingles[0], "one_aa two_bb three_cc four_dd five_ee six_ff seven_gg eight_hh")

    def test_removed_and_context_lines_and_headers_are_not_needles(self):
        patch = "--- a/f\n+++ b/f\n@@ -1 +1 @@\n-" + GOLD_LINE + "\n " + GOLD_LINE + "\n"
        self.assertEqual([n for n in leak_scan.needles_for(S5_TASK, patch) if " " in n], [])

    def test_s5_task_needles(self):
        needles = leak_scan.needles_for(S5_TASK, diff_adding())
        self.assertIn("a_reembedding_reindex_keeps_a_caller_informed", needles)
        self.assertIn("the_updates_also_advance", needles)
        self.assertIn("src/librarian/adapter_progress_tests.rs", needles)
        self.assertNotIn("src/tools/tests.rs", needles)  # hidden.files exist in the tree legitimately
        self.assertEqual(len(needles), len(set(needles)))

    def test_s5_without_patch_text_is_refused(self):
        with self.assertRaises(ValueError):
            leak_scan.needles_for(S5_TASK)

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

    def test_extra_texts_are_shingled_in_full(self):
        text = "context line\n" + GOLD_LINE + "\n}\n"
        needles = leak_scan.needles_for(S5_TASK, diff_adding(), extra_texts=[text])
        self.assertIn(GOLD_NEEDLE, needles)

    def test_needles_round_trip_through_scan(self):
        with tempfile.TemporaryDirectory() as td:
            (Path(td) / "leak.rs").write_text("fn t() {\n    " + GOLD_LINE + "\n}\n")
            needles = leak_scan.needles_for(S5_TASK, diff_adding(GOLD_LINE, "}"))
            hits = leak_scan.scan([Path(td)], None, needles)
            self.assertEqual([h.needle for h in hits], [GOLD_NEEDLE])


if __name__ == "__main__":
    unittest.main()
