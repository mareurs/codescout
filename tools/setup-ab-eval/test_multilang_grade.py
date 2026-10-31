import json
import time
import unittest
from pathlib import Path

import multilang_grade as g

FIX = Path(__file__).resolve().parent / "fixtures" / "multilang"


class RunParserTests(unittest.TestCase):
    def test_run_parser_on_real_obscura_fixture(self):
        rec = json.loads((FIX / "obscura-426.json").read_text())
        log = (FIX / "obscura-426-gold.log").read_text()
        st = g.run_parser(rec["log_parser"], log)
        self.assertEqual(st["obscura-cli::mcp_client$test_initialize"], "pass")
        self.assertEqual(st["obscura-cli::mcp_client$test_evaluate"], "fail")
        self.assertEqual(set(st.values()), {"pass", "fail"})

    def test_run_parser_times_out_on_infinite_loop(self):
        t = time.monotonic()
        with self.assertRaises(g.ParserError):
            g.run_parser("def parser(log):\n    while True:\n        pass\n", "x", timeout=1)
        self.assertLess(time.monotonic() - t, 10)

    def test_run_parser_rejects_exception_and_garbage_output_and_non_dict_and_non_string_values(self):
        bad = [
            "def parser(log):\n    raise RuntimeError('x')\n",
            "def parser(log):\n    print('garbage')\n    import os; os._exit(0)\n",
            "def parser(log):\n    return ['a']\n",
            "def parser(log):\n    return {'a': 1}\n",
            "def parser(log):\n    return {1: 'pass'}\n",
            "def parser(log):\n    return None\n",
            "this is not python(",
            "x = 1\n",  # defines no parser
        ]
        for src in bad:
            with self.subTest(src=src):
                with self.assertRaises(g.ParserError):
                    g.run_parser(src, "log")

    def test_run_parser_cannot_read_parent_environment(self):
        src = "import os\ndef parser(log):\n    return {os.environ.get('HOME', 'none'): 'pass'}\n"
        self.assertEqual(g.run_parser(src, "log"), {"none": "pass"})


class VerdictTests(unittest.TestCase):
    def test_verdict_requires_every_expected_name_to_pass(self):
        st = {"a": "pass", "b": "fail", "c": "skip", "d": "PASS"}
        v = g.verdict(st, ["a", "b"], ["c", "d", "e"])
        self.assertFalse(v["pass"])
        self.assertEqual((v["expected"], v["not_passing"], v["missing"]), (5, 4, 1))
        self.assertEqual(sorted(v["examples"]), ["b", "c", "d", "e"])
        self.assertTrue(g.verdict({"a": "pass", "b": "pass"}, ["a"], ["b"])["pass"])

    def test_verdict_empty_expected_is_not_a_pass(self):
        self.assertFalse(g.verdict({"a": "pass"}, [], [])["pass"])

    def test_verdict_examples_capped_at_ten(self):
        names = [f"t{i}" for i in range(25)]
        self.assertEqual(len(g.verdict({}, names, [])["examples"]), 10)


class ControlTests(unittest.TestCase):
    def test_control_ok_when_f2p_fails_at_base_and_all_pass_at_gold(self):
        v = g.control_verdict({"f": "fail", "p": "pass"}, {"f": "pass", "p": "pass"}, ["f"], ["p"])
        self.assertTrue(v["ok"])
        self.assertEqual(v["reasons"], [])
        self.assertEqual((v["f2p_passing_at_base"], v["expected_not_passing_at_gold"], v["expected"]), (0, 0, 2))

    def test_control_rejects_f2p_passing_at_base(self):
        v = g.control_verdict({"f": "pass"}, {"f": "pass"}, ["f"], [])
        self.assertFalse(v["ok"])
        self.assertEqual(v["reasons"], ["fail-to-pass test passes at base"])
        self.assertEqual(v["f2p_passing_at_base"], 1)

    def test_control_rejects_no_fail_to_pass_tests(self):
        v = g.control_verdict({}, {"p": "pass"}, [], ["p"])
        self.assertFalse(v["ok"])
        self.assertIn("no fail-to-pass tests", v["reasons"])

    def test_control_rejects_71_of_330_not_passing_at_gold(self):
        f2p = ["f"]
        p2p = [f"p{i}" for i in range(329)]
        gold = {n: "pass" for n in f2p + p2p}
        for n in p2p[:71]:
            gold[n] = "fail"
        v = g.control_verdict({"f": "fail"}, gold, f2p, p2p)
        self.assertFalse(v["ok"])
        self.assertEqual(v["expected"], 330)
        self.assertEqual(v["expected_not_passing_at_gold"], 71)
        self.assertEqual(v["reasons"], ["expected tests do not pass with the gold patch"])


if __name__ == "__main__":
    unittest.main()
