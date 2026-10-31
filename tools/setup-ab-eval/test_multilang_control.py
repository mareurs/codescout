import subprocess
import unittest
from types import SimpleNamespace

import multilang_control as c

PARSER = "def parser(log):\n    return dict(l.split('=') for l in log.split())\n"
BASE = "deadbeef"


def out(stdout="", rc=0):
    return SimpleNamespace(stdout=stdout, returncode=rc)


class FakeDocker:
    """Records every call; `handler(args, data)` returns a reply, an exception to raise, or None."""

    def __init__(self, handler=None):
        self.calls = []
        self.handler = handler

    def __call__(self, *args, data=None, timeout=None, check=True):
        args = tuple(str(a) for a in args)
        self.calls.append({"args": args, "data": data, "check": check, "timeout": timeout})
        if self.handler:
            reply = self.handler(args, data)
            if isinstance(reply, BaseException):
                raise reply
            if reply is not None:
                return reply
        return out()

    def joined(self):
        return [" ".join(x["args"]) for x in self.calls]


def patch_docker(testcase, fake):
    old = c.pilot.docker
    c.pilot.docker = fake
    testcase.addCleanup(setattr, c.pilot, "docker", old)


def record(**kw):
    r = {"instance_id": "o__r-1", "docker_image": "img:1", "base_commit": BASE,
         "patch": "GOLD", "test_patch": "TEST", "test_cmds": ["run-tests"],
         "print_cmds": ["cat log"], "log_parser": PARSER,
         "FAIL_TO_PASS": ["t1"], "PASS_TO_PASS": ["t2"]}
    r.update(kw)
    return r


def happy_handler(state):
    def handler(args, data):
        j = " ".join(args)
        if "test -d /testbed/repo/.git" in j:
            return out(rc=1)
        if "test -d /testbed/.git" in j:
            return out(rc=0)
        if "rev-parse HEAD" in j:
            return out(BASE + "\n")
        if "status --porcelain" in j:
            return out("" if state.get("reset") else " M a.rs\n")
        if "reset --hard" in j:
            state["reset"] = True
        if "git apply" in j and data == "GOLD":
            state["gold"] = True
        if "cat log" in j:
            return out("t1=pass t2=pass" if state.get("gold") else "t1=fail t2=pass")
        return None
    return handler


class DetectWorkdirTests(unittest.TestCase):
    def test_detect_workdir_prefers_testbed_repo(self):
        patch_docker(self, FakeDocker(lambda a, d: out(rc=0)))
        self.assertEqual(c.detect_workdir("sab-ml-ctl-x"), "/testbed/repo")

    def test_detect_workdir_falls_back_to_testbed(self):
        patch_docker(self, FakeDocker(lambda a, d: out(rc=1 if "/testbed/repo/.git" in " ".join(a) else 0)))
        self.assertEqual(c.detect_workdir("sab-ml-ctl-x"), "/testbed")

    def test_detect_workdir_raises_without_git(self):
        patch_docker(self, FakeDocker(lambda a, d: out(rc=1)))
        with self.assertRaises(c.ControlError):
            c.detect_workdir("sab-ml-ctl-x")


class ResetTests(unittest.TestCase):
    def handler(self, head=BASE, dirty_after=""):
        state = {}

        def h(args, data):
            j = " ".join(args)
            if "rev-parse HEAD" in j:
                return out(head + "\n")
            if "status --porcelain" in j:
                return out(dirty_after if state.get("reset") else "".join(f" M f{i}.rs\n" for i in range(80)))
            if "reset --hard" in j:
                state["reset"] = True
        return h

    def test_reset_records_dirty_before(self):
        fake = FakeDocker(self.handler())
        patch_docker(self, fake)
        ev = c.reset_to_base("sab-ml-ctl-x", "/testbed", BASE)
        self.assertEqual(len(ev["dirty_before"]), 50)
        self.assertEqual(ev["dirty_before"][0], " M f0.rs")
        joined = fake.joined()
        self.assertTrue(any("git reset --hard -q " + BASE in j for j in joined))
        self.assertTrue(any("git clean -fdq -e target" in j for j in joined))
        self.assertTrue(any("rm -rf /testbed/reports" in j for j in joined))

    def test_reset_to_base_raises_when_head_differs_after_reset(self):
        patch_docker(self, FakeDocker(self.handler(head="other")))
        with self.assertRaises(c.ControlError):
            c.reset_to_base("sab-ml-ctl-x", "/testbed", BASE)

    def test_reset_to_base_raises_when_tree_still_dirty(self):
        patch_docker(self, FakeDocker(self.handler(dirty_after="?? stray\n")))
        with self.assertRaises(c.ControlError):
            c.reset_to_base("sab-ml-ctl-x", "/testbed", BASE)


class RunTestsTests(unittest.TestCase):
    def test_run_tests_ignores_test_exit_code_and_returns_print_output(self):
        def h(args, data):
            j = " ".join(args)
            if "run-tests" in j:
                return out("noise", rc=101)
            if "cat a" in j:
                return out("A\n")
            if "cat b" in j:
                return out("B\n")
        fake = FakeDocker(h)
        patch_docker(self, fake)
        log = c.run_tests("sab-ml-ctl-x", "/testbed/repo",
                          record(print_cmds=["cat a", "cat b"]), timeout=7)
        self.assertEqual(log, "A\nB\n")
        first = fake.calls[0]
        self.assertEqual(first["args"][:6], ("exec", "-w", "/testbed/repo", "sab-ml-ctl-x", "bash", "-o"))
        self.assertFalse(first["check"])
        self.assertEqual(first["timeout"], 7)

    def test_run_tests_truncates_to_64_mib(self):
        big = "x" * (c.MAX_LOG_BYTES + 10)
        patch_docker(self, FakeDocker(lambda a, d: out(big) if "cat log" in " ".join(a) else None))
        self.assertEqual(len(c.run_tests("sab-ml-ctl-x", "/testbed", record())), c.MAX_LOG_BYTES)

    def test_run_tests_empty_test_cmds_raises(self):
        patch_docker(self, FakeDocker())
        with self.assertRaises(c.ControlError):
            c.run_tests("sab-ml-ctl-x", "/testbed", record(test_cmds=[]))

    def test_run_tests_timeout_is_control_error(self):
        patch_docker(self, FakeDocker(lambda a, d: subprocess.TimeoutExpired("docker", 1)))
        with self.assertRaises(c.ControlError):
            c.run_tests("sab-ml-ctl-x", "/testbed", record())


class ControlTests(unittest.TestCase):
    def test_control_ok_path_orders_calls(self):
        fake = FakeDocker(happy_handler({}))
        patch_docker(self, fake)
        ev, base_log, gold_log = c.control(record(), container="sab-ml-ctl-x")
        self.assertTrue(ev["verdict"]["ok"], ev["verdict"])
        self.assertEqual((base_log, gold_log), ("t1=fail t2=pass", "t1=pass t2=pass"))
        self.assertEqual(ev["workdir"], "/testbed")
        self.assertEqual(ev["image"], "img:1")
        self.assertEqual(ev["id"], "o__r-1")
        self.assertEqual(ev["dirty_before"], [" M a.rs"])
        self.assertEqual(ev["base"], {"fail": 1, "pass": 1})
        self.assertEqual(ev["gold"], {"pass": 2})
        self.assertIn("wall_seconds", ev)
        j = fake.joined()

        def idx(sub, start=0, data=None):
            for i in range(start, len(fake.calls)):
                if sub in j[i] and (data is None or fake.calls[i]["data"] == data):
                    return i
            self.fail(f"{sub!r} not found after {start}")
        run = idx("run -d --name sab-ml-ctl-x")
        self.assertIn("--network none", j[run])
        self.assertTrue(j[run].endswith("img:1 sleep infinity"))
        reset = idx("reset --hard", run)
        test_patch = idx("git apply -", reset, "TEST")
        run1 = idx("run-tests", test_patch)
        gold = idx("git apply -", run1, "GOLD")
        run2 = idx("run-tests", gold)
        self.assertEqual(j[-1], "rm -f sab-ml-ctl-x")
        self.assertLess(run2, len(j) - 1)

    def test_control_always_removes_container_on_error(self):
        inner = happy_handler({})

        def h(args, data):
            if "git apply" in " ".join(args):
                return RuntimeError("apply failed")
            return inner(args, data)
        fake = FakeDocker(h)
        patch_docker(self, fake)
        ev, _, _ = c.control(record(), container="sab-ml-ctl-x")
        self.assertFalse(ev["verdict"]["ok"])
        self.assertTrue(ev["verdict"]["reasons"][0].startswith("control error: "))
        self.assertEqual(fake.joined()[-1], "rm -f sab-ml-ctl-x")

    def test_control_removes_container_on_unexpected_exception(self):
        inner = happy_handler({})

        def h(args, data):
            if "reset --hard" in " ".join(args):
                return KeyboardInterrupt()
            return inner(args, data)
        fake = FakeDocker(h)
        patch_docker(self, fake)
        with self.assertRaises(KeyboardInterrupt):
            c.control(record(), container="sab-ml-ctl-x")
        self.assertEqual(fake.joined()[-1], "rm -f sab-ml-ctl-x")

    def test_control_refuses_container_name_without_prefix(self):
        for name in ("codescout-x", "pbr27-a", "rae-b", "sab-ml-x", ""):
            fake = FakeDocker()
            patch_docker(self, fake)
            with self.assertRaises(c.ControlError):
                c.control(record(), container=name)
            self.assertEqual(fake.calls, [])

    def test_control_marks_parser_error_as_not_ok(self):
        patch_docker(self, FakeDocker(happy_handler({})))
        ev, _, _ = c.control(record(log_parser="def parser(log):\n    raise SystemExit(3)\n"),
                             container="sab-ml-ctl-x")
        self.assertFalse(ev["verdict"]["ok"])
        self.assertTrue(ev["verdict"]["reasons"][0].startswith("control error: "))

    def test_control_unpassable_task_is_not_ok(self):
        inner = happy_handler({})

        def h(args, data):
            if "cat log" in " ".join(args):
                return out("t1=fail t2=fail")
            return inner(args, data)
        patch_docker(self, FakeDocker(h))
        ev, _, _ = c.control(record(), container="sab-ml-ctl-x")
        self.assertFalse(ev["verdict"]["ok"])
        self.assertEqual(ev["verdict"]["expected_not_passing_at_gold"], 2)

    def test_control_dirty_after_reset_is_not_ok_without_running_tests(self):
        inner = happy_handler({})

        def h(args, data):
            if "status --porcelain" in " ".join(args):
                return out("?? stray\n")
            return inner(args, data)
        fake = FakeDocker(h)
        patch_docker(self, fake)
        ev, base_log, gold_log = c.control(record(), container="sab-ml-ctl-x")
        self.assertFalse(ev["verdict"]["ok"])
        self.assertEqual((base_log, gold_log), ("", ""))
        self.assertFalse(any("run-tests" in x for x in fake.joined()))
        self.assertEqual(fake.joined()[-1], "rm -f sab-ml-ctl-x")


if __name__ == "__main__":
    unittest.main()
