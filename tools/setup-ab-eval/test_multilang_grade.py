import json
import os
import signal
import subprocess
import tempfile
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

    def _setsid_grandchild_src(self, pidfile, tail):
        """A parser that forks a grandchild into its own session (out of the killed group) that
        sleeps 30 s holding the inherited descriptors, then runs `tail`."""
        pf = repr(str(pidfile))
        return (
            "import os, time\n"
            "def parser(log):\n"
            "    pid = os.fork()\n"
            "    if pid == 0:\n"
            "        os.setsid()\n"
            f"        open({pf}, 'w').write(str(os.getpid()))\n"
            "        time.sleep(30)\n"
            "        os._exit(0)\n"
            f"    while not os.path.exists({pf}) or not open({pf}).read():\n"
            "        time.sleep(0.01)\n"
            + tail
        )

    def _reap_grandchild(self, pidfile):
        try:
            os.kill(int(Path(pidfile).read_text()), signal.SIGKILL)
        except (OSError, ValueError):
            pass

    def test_run_parser_timeout_is_bounded_when_a_setsid_grandchild_holds_descriptors(self):
        with tempfile.TemporaryDirectory() as d:
            pidfile = Path(d) / "pid"
            self.addCleanup(self._reap_grandchild, pidfile)
            src = self._setsid_grandchild_src(pidfile, "    time.sleep(60)\n")
            t = time.monotonic()
            with self.assertRaises(g.ParserError):
                g.run_parser(src, "x", timeout=2)
            self.assertLess(time.monotonic() - t, 2 + 5)

    def test_run_parser_normal_path_is_bounded_when_a_setsid_grandchild_holds_descriptors(self):
        with tempfile.TemporaryDirectory() as d:
            pidfile = Path(d) / "pid"
            self.addCleanup(self._reap_grandchild, pidfile)
            src = self._setsid_grandchild_src(pidfile, "    return {'t': 'pass'}\n")
            t = time.monotonic()
            try:
                g.run_parser(src, "x", timeout=2)
            except g.ParserError:
                pass  # either outcome is acceptable; hanging is not
            self.assertLess(time.monotonic() - t, 2 + 5)


    def test_run_parser_cannot_read_parent_environment(self):
        src = "import os\ndef parser(log):\n    return {os.environ.get('HOME', 'none'): 'pass'}\n"
        self.assertEqual(g.run_parser(src, "log"), {"none": "pass"})


CONNECT_SRC = (
    "import socket\n"
    "def parser(log):\n"
    "    socket.create_connection(('127.0.0.1', int(log)), timeout=3).close()\n"
    "    return {'connected': 'pass'}\n"
)


class NetworkIsolationTests(unittest.TestCase):
    def listener(self):
        import socket
        s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self.addCleanup(s.close)
        s.bind(("127.0.0.1", 0))
        s.listen(1)
        return s.getsockname()[1]

    def test_isolated_parser_cannot_reach_a_host_loopback_listener(self):
        if not g.network_isolation_available():
            self.skipTest("`unshare -rn true` does not work on this machine: no network namespace")
        port = self.listener()
        # the same parser reaches the listener without isolation, so the failure below is the namespace
        self.assertEqual(g.run_parser(CONNECT_SRC, str(port), isolate_network=False),
                         {"connected": "pass"})
        with self.assertRaises(g.ParserError):
            g.run_parser(CONNECT_SRC, str(port))

    def test_isolation_is_the_default_and_refuses_without_unshare(self):
        old = g._UNSHARE
        g._UNSHARE = ""  # probed: not available
        self.addCleanup(setattr, g, "_UNSHARE", old)
        with self.assertRaisesRegex(g.ParserError, "no network isolation available"):
            g.run_parser("def parser(log):\n    return {}\n", "x")
        self.assertEqual(g.run_parser("def parser(log):\n    return {}\n", "x", isolate_network=False), {})

    def test_probe_runs_once_and_is_cached(self):
        old_cache, old_run = g._UNSHARE, g.subprocess.run
        calls = []

        def fake_run(args, **kw):
            calls.append(list(args))
            return subprocess.CompletedProcess(args, 0, "", "")
        g._UNSHARE = None
        g.subprocess.run = fake_run
        self.addCleanup(setattr, g, "_UNSHARE", old_cache)
        self.addCleanup(setattr, g.subprocess, "run", old_run)
        self.assertTrue(g.network_isolation_available())
        self.assertTrue(g.network_isolation_available())
        self.assertEqual(len(calls), 1)
        self.assertEqual(calls[0][1:], ["-rn", "true"])


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

    def test_control_gold_repeat_without_mismatch_is_ok(self):
        gold = {"f": "pass", "p": "pass"}
        v = g.control_verdict({"f": "fail"}, gold, ["f"], ["p"], gold_repeat=dict(gold, other="fail"))
        self.assertTrue(v["ok"], v)
        self.assertEqual(v["gold_repeat_mismatch"], {"count": 0, "names": []})

    def test_control_gold_repeat_mismatch_is_not_ok_with_reason(self):
        gold = {"f": "pass", "p": "pass"}
        v = g.control_verdict({"f": "fail"}, gold, ["f"], ["p"], gold_repeat={"f": "pass", "p": "fail"})
        self.assertFalse(v["ok"])
        self.assertEqual(v["reasons"], ["tests differ between two gold runs"])
        self.assertEqual(v["gold_repeat_mismatch"], {"count": 1, "names": ["p"]})

    def test_control_gold_repeat_missing_names_count_and_names_are_capped(self):
        names = [f"p{i}" for i in range(15)]
        gold = {n: "pass" for n in ["f"] + names}
        v = g.control_verdict({}, gold, ["f"], names, gold_repeat={"f": "pass"})
        self.assertEqual(v["gold_repeat_mismatch"]["count"], 15)
        self.assertEqual(v["gold_repeat_mismatch"]["names"], names[:10])

    def test_control_without_gold_repeat_has_no_mismatch_field(self):
        v = g.control_verdict({"f": "fail"}, {"f": "pass"}, ["f"], [])
        self.assertNotIn("gold_repeat_mismatch", v)


if __name__ == "__main__":
    unittest.main()
