import base64
import hashlib
import json
import os
import signal
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path
from types import SimpleNamespace

import multilang_control as c

PARSER = "def parser(log):\n    return dict(l.split('=') for l in log.split())\n"
BASE = "deadbeef"


def out(stdout="", rc=0):
    return SimpleNamespace(stdout=stdout, returncode=rc)


def b64(text):
    """What a print command's (or a step's tail) wrapper prints: the bytes, base64 encoded."""
    raw = text if isinstance(text, bytes) else text.encode()
    return base64.b64encode(raw).decode()


class FakeDocker:
    """Records every call; `handler(args, data)` returns a reply, an exception to raise, or None.
    Like pilot.command, a non-zero reply raises RuntimeError unless the call passed check=False."""

    def __init__(self, handler=None):
        self.calls = []
        self.handler = handler

    def __call__(self, *args, data=None, timeout=None, check=True):
        args = tuple(str(a) for a in args)
        self.calls.append({"args": args, "data": data, "check": check, "timeout": timeout})
        reply = self.handler(args, data) if self.handler else None
        if isinstance(reply, BaseException):
            raise reply
        if reply is None:
            reply = out()
        if check and reply.returncode:
            raise RuntimeError(f"docker failed (exit {reply.returncode}): {reply.stdout[-4000:]}")
        return reply

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
            state["gold"] = False  # a reset drops the gold patch
        if "git apply" in j and data == "GOLD":
            state["gold"] = True
        if "cat log" in j and "base64" in j:
            return out(b64("t1=pass t2=pass" if state.get("gold") else "t1=fail t2=pass"))
        return None
    return handler


def finder(testcase, fake):
    """idx(sub, start, data): the first call at or after `start` whose joined args contain `sub`."""
    j = fake.joined()

    def idx(sub, start=0, data=None):
        for i in range(start, len(fake.calls)):
            if sub in j[i] and (data is None or fake.calls[i]["data"] == data):
                return i
        testcase.fail(f"{sub!r} not found after {start}: {j[start:]}")
    return idx


def is_test_step(joined):
    return "run-tests" in joined


def is_print(joined):
    return "cat log" in joined


class DetectWorkdirTests(unittest.TestCase):
    def test_detect_workdir_prefers_testbed_repo(self):
        patch_docker(self, FakeDocker(lambda a, d: out(rc=0)))
        self.assertEqual(c.detect_workdir("sab-ml-ctl-x"), "/testbed/repo")

    def test_detect_workdir_falls_back_to_testbed(self):
        fake = FakeDocker(lambda a, d: out(rc=1 if "/testbed/repo/.git" in " ".join(a) else 0))
        patch_docker(self, fake)
        self.assertEqual(c.detect_workdir("sab-ml-ctl-x"), "/testbed")
        self.assertFalse(fake.calls[0]["check"])

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

    def test_reset_recreates_empty_reports_dirs_after_removing_them(self):
        fake = FakeDocker(self.handler())
        patch_docker(self, fake)
        c.reset_to_base("sab-ml-ctl-x", "/testbed/repo", BASE)
        idx = finder(self, fake)
        clean = idx("git clean")
        rm = idx("rm -rf /testbed/repo/reports /testbed/reports", clean)
        mk = idx("mkdir -p /testbed/repo/reports /testbed/reports", rm)
        head = idx("rev-parse HEAD", mk)
        idx("status --porcelain", head)  # the clean check still runs, after the directories exist

    def test_reset_with_workdir_testbed_does_not_duplicate_reports_dir(self):
        fake = FakeDocker(self.handler())
        patch_docker(self, fake)
        c.reset_to_base("sab-ml-ctl-x", "/testbed", BASE)
        self.assertTrue(any(x.endswith("mkdir -p /testbed/reports") for x in fake.joined()))

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
                return out(b64("A\n"), rc=1)
            if "cat b" in j:
                return out(b64("B\n"))
        fake = FakeDocker(h)
        patch_docker(self, fake)
        log = c.run_tests("sab-ml-ctl-x", "/testbed/repo",
                          record(print_cmds=["cat a", "cat b"]), timeout=7)
        self.assertEqual(log, "A\nB\n")
        first = fake.calls[0]
        self.assertEqual(first["args"][:6], ("exec", "-w", "/testbed/repo", "sab-ml-ctl-x", "bash", "-o"))
        self.assertFalse(first["check"])
        self.assertEqual(first["timeout"], 7)
        # test output stays in the container: into a scratch file, of which only the tail comes back
        self.assertIn(f">{c.STEP_OUT} 2>&1", first["args"][-1])
        self.assertTrue(all(not x["check"] for x in fake.calls))

    def test_run_tests_records_each_step_with_returncode_and_tail(self):
        def h(args, data):
            j = " ".join(args)
            if "run-tests" in j:
                return out(b64("error[E0308]: mismatched types\n"), rc=101)
            if "cat log" in j:
                return out(b64("t1=pass\n"), rc=0)
        patch_docker(self, FakeDocker(h))
        steps = []
        c.run_tests("sab-ml-ctl-x", "/testbed", record(test_cmds=["run-tests", "run-tests 2"]),
                    steps=steps, state="base")
        self.assertEqual(steps, [
            {"name": "base:test[0]", "returncode": 101, "tail": "error[E0308]: mismatched types\n"},
            {"name": "base:test[1]", "returncode": 101, "tail": "error[E0308]: mismatched types\n"},
            {"name": "base:print[0]", "returncode": 0, "tail": "t1=pass\n"},
        ])

    def test_step_wrapper_bounds_the_tail_and_keeps_the_exit_code(self):
        fake = FakeDocker()
        patch_docker(self, fake)
        c.run_tests("sab-ml-ctl-x", "/testbed", record(print_cmds=[]))
        script = fake.calls[0]["args"][-1]
        self.assertTrue(script.startswith("(\nrun-tests\n)"), script)
        self.assertIn(f"tail -c {c.TAIL_BYTES} {c.STEP_OUT} | base64 -w0", script)
        self.assertIn("rc=$?", script)
        self.assertTrue(script.rstrip().endswith("exit $rc"), script)

    def test_step_wrapper_runs_in_bash_and_returns_the_exit_code_and_the_tail(self):
        # the wrapper itself, run by a real local bash (no docker): exit code and last bytes survive
        with tempfile.TemporaryDirectory() as d:
            old = c.STEP_OUT
            c.STEP_OUT = f"{d}/step.out"
            self.addCleanup(setattr, c, "STEP_OUT", old)
            script = c._step_script("for i in $(seq 1 3000); do echo line$i; done; exit 7")
            res = subprocess.run(["bash", "-o", "pipefail", "-c", script], capture_output=True, text=True)
            self.assertEqual(res.returncode, 7)
            tail = c._decode_tail(res.stdout)
            self.assertEqual(len(tail.encode()), c.TAIL_BYTES)
            self.assertTrue(tail.endswith("line3000\n"))
            self.assertFalse(os.path.exists(c.STEP_OUT))

    def test_step_tail_tolerates_output_that_is_not_base64(self):
        patch_docker(self, FakeDocker(
            lambda a, d: out("Error response from daemon: container stopped", rc=126)
            if "run-tests" in " ".join(a) else (out(b64("t1=pass")) if "cat log" in " ".join(a) else None)))
        steps = []
        c.run_tests("sab-ml-ctl-x", "/testbed", record(), steps=steps, state="gold")
        self.assertEqual(steps[0], {"name": "gold:test[0]", "returncode": 126,
                                    "tail": "Error response from daemon: container stopped"})

    def test_print_step_tail_is_the_end_of_the_log(self):
        log = "x" * 10000 + "END"
        patch_docker(self, FakeDocker(lambda a, d: out(b64(log)) if "cat log" in " ".join(a) else None))
        steps = []
        self.assertEqual(c.run_tests("sab-ml-ctl-x", "/testbed", record(), steps=steps), log)
        self.assertEqual(len(steps[-1]["tail"]), c.TAIL_BYTES)
        self.assertTrue(steps[-1]["tail"].endswith("END"))
        self.assertNotIn("truncated", steps[-1])

    def test_print_step_at_the_cap_is_marked_truncated(self):
        old = c.MAX_LOG_BYTES
        c.MAX_LOG_BYTES = 32
        self.addCleanup(setattr, c, "MAX_LOG_BYTES", old)
        patch_docker(self, FakeDocker(lambda a, d: out(b64("y" * 32)) if "cat log" in " ".join(a) else None))
        steps = []
        c.run_tests("sab-ml-ctl-x", "/testbed", record(), steps=steps)
        self.assertIs(steps[-1]["truncated"], True)

    def test_joined_log_over_the_cap_marks_the_last_print_step_truncated(self):
        old = c.MAX_LOG_BYTES
        c.MAX_LOG_BYTES = 32
        self.addCleanup(setattr, c, "MAX_LOG_BYTES", old)
        patch_docker(self, FakeDocker(lambda a, d: out(b64("z" * 20)) if "cat" in " ".join(a) else None))
        steps = []
        log = c.run_tests("sab-ml-ctl-x", "/testbed", record(print_cmds=["cat a", "cat b"]), steps=steps)
        self.assertEqual(len(log), 32)
        self.assertNotIn("truncated", steps[-2])
        self.assertIs(steps[-1]["truncated"], True)

    def test_run_tests_bounds_print_output_inside_the_container(self):
        fake = FakeDocker(lambda a, d: out(b64("x")) if "cat log" in " ".join(a) else None)
        patch_docker(self, fake)
        c.run_tests("sab-ml-ctl-x", "/testbed", record())
        printed = fake.calls[-1]["args"][-1]
        self.assertIn(f"head -c {c.MAX_LOG_BYTES}", printed)
        self.assertIn("base64", printed)

    def test_run_tests_truncates_joined_log_to_the_cap(self):
        big = "x" * (c.MAX_LOG_BYTES + 10)
        patch_docker(self, FakeDocker(lambda a, d: out(b64(big)) if "cat log" in " ".join(a) else None))
        self.assertEqual(len(c.run_tests("sab-ml-ctl-x", "/testbed", record())), c.MAX_LOG_BYTES)

    def test_run_tests_replaces_non_utf8_bytes_instead_of_raising(self):
        patch_docker(self, FakeDocker(
            lambda a, d: out(b64(b"\xff\xfe t1=pass")) if "cat log" in " ".join(a) else None))
        log = c.run_tests("sab-ml-ctl-x", "/testbed", record())
        self.assertTrue(log.endswith("t1=pass"))
        self.assertIn("�", log)

    def test_run_tests_garbage_print_output_is_control_error(self):
        patch_docker(self, FakeDocker(
            lambda a, d: out("Error response from daemon: gone") if "cat log" in " ".join(a) else None))
        with self.assertRaises(c.ControlError):
            c.run_tests("sab-ml-ctl-x", "/testbed", record())

    def test_run_tests_empty_test_cmds_raises(self):
        patch_docker(self, FakeDocker())
        with self.assertRaises(c.ControlError):
            c.run_tests("sab-ml-ctl-x", "/testbed", record(test_cmds=[]))

    def test_run_tests_timeout_is_control_error(self):
        patch_docker(self, FakeDocker(lambda a, d: subprocess.TimeoutExpired("docker", 1)))
        with self.assertRaises(c.ControlError):
            c.run_tests("sab-ml-ctl-x", "/testbed", record())


class RebuildTests(unittest.TestCase):
    def test_rebuild_joins_the_commands_skips_blank_ones_and_records_a_step(self):
        fake = FakeDocker(lambda a, d: out(b64("Finished dev"), rc=0))
        patch_docker(self, fake)
        steps = []
        c.rebuild("sab-ml-ctl-x", "/testbed/repo",
                  record(rebuild_cmds=["cargo build", "  ", "cargo test --no-run"]), timeout=9,
                  steps=steps, state="gold")
        self.assertEqual(len(fake.calls), 1)
        call = fake.calls[0]
        self.assertEqual(call["args"][:7],
                         ("exec", "-w", "/testbed/repo", "sab-ml-ctl-x", "bash", "-o", "pipefail"))
        self.assertTrue(call["args"][-1].startswith("(\ncargo build ; cargo test --no-run\n)"))
        self.assertFalse(call["check"])
        self.assertEqual(call["timeout"], 9)
        self.assertEqual(steps, [{"name": "gold:rebuild", "returncode": 0, "tail": "Finished dev"}])

    def test_rebuild_failure_is_data_not_an_error(self):
        patch_docker(self, FakeDocker(lambda a, d: out(b64("error: could not compile"), rc=101)))
        steps = []
        c.rebuild("sab-ml-ctl-x", "/testbed", record(rebuild_cmds=["cargo build"]), steps=steps)
        self.assertEqual(steps[0]["returncode"], 101)
        self.assertEqual(steps[0]["tail"], "error: could not compile")

    def test_rebuild_is_skipped_when_empty_blank_or_missing(self):
        for rec in (record(), record(rebuild_cmds=[]), record(rebuild_cmds=["", "  "]),
                    record(rebuild_cmds=None)):
            fake = FakeDocker()
            patch_docker(self, fake)
            steps = []
            c.rebuild("sab-ml-ctl-x", "/testbed", rec, steps=steps)
            self.assertEqual((fake.calls, steps), ([], []), rec.get("rebuild_cmds", "missing"))


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
        self.assertEqual(ev["gold_repeat"], {"pass": 2})
        self.assertEqual(ev["gold_repeat_mismatch"], {"count": 0, "names": []})
        self.assertIs(ev["container_removed"], True)
        self.assertIn("wall_seconds", ev)
        j = fake.joined()
        idx = finder(self, fake)
        run = idx("run -d --name sab-ml-ctl-x")
        self.assertIn("--network none", j[run])
        self.assertTrue(j[run].endswith("img:1 sleep infinity"))
        reset = idx("reset --hard", run)
        test_patch = idx("git apply -", reset, "TEST")
        run1 = idx("run-tests", test_patch)
        gold = idx("git apply -", run1, "GOLD")
        run2 = idx("run-tests", gold)
        self.assertEqual(j[-1], "rm -f -v sab-ml-ctl-x")
        self.assertLess(run2, len(j) - 1)

    def test_control_run_flags_and_leftover_container_removed_first(self):
        fake = FakeDocker(happy_handler({}))
        patch_docker(self, fake)
        c.control(record(), container="sab-ml-ctl-x")
        j = fake.joined()
        self.assertEqual(j[0], "rm -f -v sab-ml-ctl-x")
        self.assertFalse(fake.calls[0]["check"])
        self.assertTrue(j[1].startswith("run -d --name sab-ml-ctl-x "), j[1])
        for flag in ("--network none", "--pids-limit 4096", "--security-opt no-new-privileges",
                     "--memory 24g", "--memory-swap 24g", "--cpus 4"):
            self.assertIn(flag, j[1])
        self.assertTrue(j[1].endswith("img:1 sleep infinity"))

    def test_every_docker_rm_of_the_container_removes_its_volumes(self):
        for handler in (happy_handler({}), lambda a, d: RuntimeError("boom") if a[0] == "run" else None):
            fake = FakeDocker(handler)
            patch_docker(self, fake)
            c.control(record(), container="sab-ml-ctl-x")
            rms = [x["args"] for x in fake.calls if x["args"][0] == "rm"]
            self.assertEqual(len(rms), 2)
            self.assertTrue(all(a == ("rm", "-f", "-v", "sab-ml-ctl-x") for a in rms), rms)

    def test_control_base_state_order_with_rebuild(self):
        fake = FakeDocker(happy_handler({}))
        patch_docker(self, fake)
        ev, _, _ = c.control(record(rebuild_cmds=["cargo build"]), container="sab-ml-ctl-x")
        self.assertTrue(ev["verdict"]["ok"], ev["verdict"])
        idx = finder(self, fake)
        run = idx("run -d --name")
        reset = idx("reset --hard", run)
        reports = idx("mkdir -p /testbed/reports", reset)
        test_patch = idx("git apply -", reports, "TEST")
        rebuild = idx("cargo build", test_patch)
        tests = idx("run-tests", rebuild)
        printed = idx("cat log", tests)
        # nothing else touches the tree between the test patch and the base print
        j = fake.joined()
        between = j[test_patch + 1:printed]
        self.assertFalse(any("git apply" in x or "reset --hard" in x for x in between), between)
        self.assertEqual(idx("git apply -", 0, "TEST"), test_patch)
        self.assertGreater(idx("git apply -", 0, "GOLD"), printed)

    def test_control_gold_state_starts_from_a_fresh_base_and_rebuilds_then_runs_twice(self):
        fake = FakeDocker(happy_handler({}))
        patch_docker(self, fake)
        c.control(record(rebuild_cmds=["cargo build"]), container="sab-ml-ctl-x")
        idx = finder(self, fake)
        j = fake.joined()
        base_print = idx("cat log", idx("run-tests"))
        reset2 = idx("reset --hard", base_print)
        clean2 = idx("git clean -fdq -e target", reset2)
        reports = idx("mkdir -p /testbed/reports", clean2)
        test_patch = idx("git apply -", reports, "TEST")
        gold_patch = idx("git apply -", test_patch, "GOLD")
        rebuild = idx("cargo build", gold_patch)
        tests1 = idx("run-tests", rebuild)
        print1 = idx("cat log", tests1)
        tests2 = idx("run-tests", print1 + 1)
        print2 = idx("cat log", tests2)
        self.assertEqual(j[print2 + 1:], ["rm -f -v sab-ml-ctl-x"])
        # the repeat is in the same container, warm: no reset, no patch, no rebuild before it
        between = j[print1 + 1:tests2]
        self.assertFalse(any(s in x for x in between for s in ("reset --hard", "git clean", "git apply",
                                                                "cargo build")), between)
        # but the reports dirs are fresh, so a stale report cannot stand in for the repeat
        self.assertTrue(any("rm -rf /testbed/reports" in x for x in between), between)
        self.assertEqual(sum("cargo build" in x for x in j), 2)
        self.assertEqual(sum("reset --hard" in x for x in j), 2)

    def test_control_records_steps_for_every_state(self):
        patch_docker(self, FakeDocker(happy_handler({})))
        ev, _, _ = c.control(record(rebuild_cmds=["cargo build"]), container="sab-ml-ctl-x")
        self.assertEqual([s["name"] for s in ev["steps"]], [
            "base:rebuild", "base:test[0]", "base:print[0]",
            "gold:rebuild", "gold:test[0]", "gold:print[0]",
            "gold-repeat:test[0]", "gold-repeat:print[0]"])
        self.assertTrue(all(set(s) >= {"name", "returncode", "tail"} for s in ev["steps"]))
        self.assertNotIn("log_truncated", ev)

    def test_control_records_log_truncated(self):
        old = c.MAX_LOG_BYTES
        c.MAX_LOG_BYTES = 16
        self.addCleanup(setattr, c, "MAX_LOG_BYTES", old)
        inner = happy_handler({})
        patch_docker(self, FakeDocker(
            lambda a, d: out(b64("t1=fail t2=pass " * 4)) if "cat log" in " ".join(a) else inner(a, d)))
        ev, _, _ = c.control(record(), container="sab-ml-ctl-x")
        self.assertIs(ev["log_truncated"], True)

    def test_control_gold_repeat_mismatch_is_not_ok(self):
        state, prints = {}, []
        inner = happy_handler(state)

        def h(args, data):
            if "cat log" in " ".join(args) and state.get("gold"):
                prints.append(1)
                return out(b64("t1=pass t2=pass" if len(prints) == 1 else "t1=pass t2=fail"))
            return inner(args, data)
        patch_docker(self, FakeDocker(h))
        ev, _, gold_log = c.control(record(), container="sab-ml-ctl-x")
        self.assertEqual(gold_log, "t1=pass t2=pass")
        self.assertFalse(ev["verdict"]["ok"])
        self.assertNotIn("error", ev["verdict"])
        self.assertEqual(ev["verdict"]["reasons"], ["tests differ between two gold runs"])
        self.assertEqual(ev["gold_repeat_mismatch"], {"count": 1, "names": ["t2"]})

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
        self.assertEqual(fake.joined()[-1], "rm -f -v sab-ml-ctl-x")

    def test_control_error_verdict_is_flagged_and_keeps_the_tail_of_the_message(self):
        inner = happy_handler({})
        patch_docker(self, FakeDocker(
            lambda a, d: RuntimeError("H" * 500 + "the real cause") if "git apply" in " ".join(a)
            else inner(a, d)))
        ev, _, _ = c.control(record(), container="sab-ml-ctl-x")
        v = ev["verdict"]
        self.assertIs(v["error"], True)
        self.assertFalse(v["ok"])
        self.assertTrue(v["reasons"][0].endswith("the real cause"), v["reasons"][0])
        self.assertLessEqual(len(v["reasons"][0]), len("control error: ") + 300)

    def test_control_real_rejection_is_not_flagged_as_error(self):
        inner = happy_handler({})
        patch_docker(self, FakeDocker(
            lambda a, d: out(b64("t1=fail t2=fail")) if "cat log" in " ".join(a) else inner(a, d)))
        ev, _, _ = c.control(record(), container="sab-ml-ctl-x")
        self.assertFalse(ev["verdict"]["ok"])
        self.assertNotIn("error", ev["verdict"])

    def test_final_rm_failure_is_recorded_not_raised(self):
        for failure in (subprocess.TimeoutExpired("docker", 600), RuntimeError("daemon gone"),
                        OSError("no docker binary"), out("permission denied", rc=1)):
            rms = []
            inner = happy_handler({})

            def h(args, data, failure=failure, rms=rms):
                if args[0] == "rm":
                    rms.append(1)
                    if len(rms) == 2:
                        return failure
                return inner(args, data)
            fake = FakeDocker(h)
            patch_docker(self, fake)
            ev, _, _ = c.control(record(), container="sab-ml-ctl-x")
            self.assertTrue(ev["verdict"]["ok"], ev["verdict"])  # the verdict is not masked
            self.assertIs(ev["container_removed"], False)
            self.assertTrue(ev["container_remove_error"])
            self.assertEqual(fake.calls[-1]["timeout"], 600)
            self.assertFalse(fake.calls[-1]["check"])

    def test_final_rm_of_a_container_that_never_started_is_not_a_failure(self):
        patch_docker(self, FakeDocker(
            lambda a, d: RuntimeError("no such image") if a[0] == "run"
            else (out("Error response from daemon: No such container: sab-ml-ctl-x", rc=1)
                  if a[0] == "rm" else None)))
        ev, _, _ = c.control(record(), container="sab-ml-ctl-x")
        self.assertIs(ev["container_removed"], True)

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
        self.assertEqual(fake.joined()[-1], "rm -f -v sab-ml-ctl-x")

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
        self.assertIs(ev["verdict"]["error"], True)
        self.assertTrue(ev["verdict"]["reasons"][0].startswith("control error: "))

    def test_control_unpassable_task_is_not_ok(self):
        inner = happy_handler({})

        def h(args, data):
            if "cat log" in " ".join(args):
                return out(b64("t1=fail t2=fail"))
            return inner(args, data)
        patch_docker(self, FakeDocker(h))
        ev, _, _ = c.control(record(), container="sab-ml-ctl-x")
        self.assertFalse(ev["verdict"]["ok"])
        self.assertEqual(ev["verdict"]["expected_not_passing_at_gold"], 2)

    def test_control_non_utf8_log_does_not_raise(self):
        inner = happy_handler({})

        def h(args, data):
            if "cat log" in " ".join(args):
                return out(b64(b"\xff\xfe t1=pass t2=pass"))
            return inner(args, data)
        patch_docker(self, FakeDocker(h))
        ev, _, _ = c.control(record(), container="sab-ml-ctl-x")
        self.assertIn("ok", ev["verdict"])

    def test_control_undecodable_docker_output_is_not_ok_not_a_crash(self):
        inner = happy_handler({})

        def h(args, data):
            if "rev-parse HEAD" in " ".join(args):
                return UnicodeDecodeError("utf-8", b"\xff", 0, 1, "invalid start byte")
            return inner(args, data)
        fake = FakeDocker(h)
        patch_docker(self, fake)
        ev, _, _ = c.control(record(), container="sab-ml-ctl-x")
        self.assertFalse(ev["verdict"]["ok"])
        self.assertTrue(ev["verdict"]["reasons"][0].startswith("control error: "))
        self.assertEqual(fake.joined()[-1], "rm -f -v sab-ml-ctl-x")

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
        self.assertEqual(fake.joined()[-1], "rm -f -v sab-ml-ctl-x")


RULE = {"created_at_min": "2025-06-01", "require": ["patch"], "min_files": 99,
        "min_added_lines": 99, "max_added_lines": 1000, "draw": {"salt": "s"}}


def full_record(**kw):
    r = record(**kw)
    r.setdefault("repo", "o/r")
    r.setdefault("created_at", "2026-01-01T00:00:00Z")
    r["patch"] = kw.get("patch", "diff --git a/a.rs b/a.rs\n+x\n")
    return r


class DiskGuardTests(unittest.TestCase):
    def patch_free(self, gb):
        old = c.free_gb
        c.free_gb = lambda path="/": gb
        self.addCleanup(setattr, c, "free_gb", old)

    def test_ensure_image_refuses_pull_below_min_free(self):
        fake = FakeDocker(lambda a, d: out(rc=1) if "inspect" in a else None)
        patch_docker(self, fake)
        self.patch_free(c.MIN_FREE_GB - 1)
        with self.assertRaises(c.ControlError):
            c.ensure_image("img:1")
        self.assertFalse(any(x.startswith("pull") for x in fake.joined()))

    def test_ensure_image_pulls_when_absent_and_space_ok(self):
        fake = FakeDocker(lambda a, d: out(rc=1) if "inspect" in a else None)
        patch_docker(self, fake)
        self.patch_free(c.MIN_FREE_GB + 1)
        self.assertTrue(c.ensure_image("img:1"))
        self.assertIn("pull img:1", fake.joined())

    def test_ensure_image_does_not_pull_or_remove_a_present_image(self):
        fake = FakeDocker(lambda a, d: out(rc=0))
        patch_docker(self, fake)
        self.patch_free(0)  # even a full disk does not matter: nothing is pulled
        self.assertFalse(c.ensure_image("img:1"))
        self.assertFalse(any(x.startswith(("pull", "rmi")) for x in fake.joined()))

    def test_ensure_image_pull_failure_is_control_error(self):
        fake = FakeDocker(lambda a, d: out(rc=1))
        patch_docker(self, fake)
        self.patch_free(c.MIN_FREE_GB + 1)
        with self.assertRaises(c.ControlError):
            c.ensure_image("img:1")


class PulledMarkerTests(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.dir = Path(tmp.name)
        self.marker = self.dir / "o__r-1" / "pulled-image"
        old = c.free_gb
        c.free_gb = lambda path="/": 500
        self.addCleanup(setattr, c, "free_gb", old)

    def test_marker_is_written_before_the_pull_and_kept_after(self):
        seen = []

        def h(args, data):
            if args[0] == "image":
                return out(rc=1)
            if args[0] == "pull":
                seen.append(self.marker.read_text(encoding="utf-8").strip() if self.marker.exists() else None)
        patch_docker(self, FakeDocker(h))
        self.assertTrue(c.ensure_image("img:1", marker=self.marker))
        self.assertEqual(seen, ["img:1"])
        self.assertTrue(self.marker.exists())

    def test_present_image_with_our_marker_counts_as_pulled_by_us(self):
        self.marker.parent.mkdir(parents=True)
        self.marker.write_text("img:1\n", encoding="utf-8")
        fake = FakeDocker(lambda a, d: out(rc=0))
        patch_docker(self, fake)
        self.assertTrue(c.ensure_image("img:1", marker=self.marker))
        self.assertFalse(any(x.startswith("pull") for x in fake.joined()))

    def test_present_image_without_marker_or_with_another_images_marker_is_not_ours(self):
        patch_docker(self, FakeDocker(lambda a, d: out(rc=0)))
        self.assertFalse(c.ensure_image("img:1", marker=self.marker))
        self.marker.parent.mkdir(parents=True)
        self.marker.write_text("other:2\n", encoding="utf-8")
        self.assertFalse(c.ensure_image("img:1", marker=self.marker))

    def test_failed_pull_with_no_image_left_deletes_the_marker(self):
        patch_docker(self, FakeDocker(
            lambda a, d: out(rc=1) if a[0] == "image" else (RuntimeError("denied") if a[0] == "pull" else None)))
        with self.assertRaises(c.ControlError):
            c.ensure_image("img:1", marker=self.marker)
        self.assertFalse(self.marker.exists())

    def test_failed_pull_whose_image_cannot_be_removed_keeps_the_marker(self):
        state = {"pulled": False}

        def h(args, data):
            if args[0] == "image":
                return out(rc=0 if state["pulled"] else 1)
            if args[0] == "pull":
                state["pulled"] = True
                return subprocess.TimeoutExpired("docker", 1)
            if args[0] == "rmi":
                return out("image is in use", rc=1)
        patch_docker(self, FakeDocker(h))
        with self.assertRaises(c.ControlError):
            c.ensure_image("img:1", marker=self.marker)
        self.assertTrue(self.marker.exists())  # the next run finds it and removes the image

    def run_one_resumed(self, with_marker):
        out_dir = self.dir / ("m" if with_marker else "n")
        marker = out_dir / "o__r-1" / "pulled-image"
        if with_marker:
            marker.parent.mkdir(parents=True)
            marker.write_text("img:1\n", encoding="utf-8")
        inner = happy_handler({})
        fake = FakeDocker(lambda a, d: out(rc=0) if a[:2] == ("image", "inspect") else inner(a, d))
        patch_docker(self, fake)
        ev = c.run_one(record(), out_dir)
        return fake, ev, marker

    def test_resume_with_marker_and_image_present_removes_the_image_and_the_marker(self):
        fake, ev, marker = self.run_one_resumed(True)
        self.assertEqual(fake.joined()[-1], "rmi img:1")
        self.assertIs(ev["image_removed"], True)
        self.assertFalse(marker.exists())

    def test_resume_without_marker_leaves_the_present_image(self):
        fake, ev, marker = self.run_one_resumed(False)
        self.assertFalse(any(x.startswith("rmi") for x in fake.joined()))
        self.assertNotIn("image_removed", ev)
        self.assertFalse(marker.exists())

    def test_failed_rmi_keeps_the_marker(self):
        self.marker.parent.mkdir(parents=True)
        self.marker.write_text("img:1\n", encoding="utf-8")
        inner = happy_handler({})
        patch_docker(self, FakeDocker(
            lambda a, d: out(rc=0) if a[:2] == ("image", "inspect")
            else (out("in use", rc=1) if a[0] == "rmi" else inner(a, d))))
        ev = c.run_one(record(), self.dir)
        self.assertIs(ev["image_removed"], False)
        self.assertTrue(self.marker.exists())


def stub_ensure(testcase, pulled):
    old = c.ensure_image
    c.ensure_image = lambda image, marker=None: pulled
    testcase.addCleanup(setattr, c, "ensure_image", old)


class RunOneTests(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.out = Path(tmp.name)

    def test_run_one_removes_only_an_image_it_pulled(self):
        for pulled in (True, False):
            fake = FakeDocker(happy_handler({}))
            patch_docker(self, fake)
            stub_ensure(self, pulled)
            ev = c.run_one(record(), self.out / str(pulled))
            self.assertTrue(ev["verdict"]["ok"])
            self.assertEqual("rmi img:1" in fake.joined(), pulled)
            d = self.out / str(pulled) / "o__r-1"
            for name in ("control.json", "base.log", "gold.log"):
                self.assertTrue((d / name).exists(), name)
            if pulled:
                self.assertEqual(fake.joined()[-1], "rmi img:1")  # no -f

    def test_run_one_removes_pulled_image_when_control_raises(self):
        inner = happy_handler({})
        fake = FakeDocker(lambda a, d: KeyboardInterrupt() if "reset --hard" in " ".join(a)
                          else inner(a, d))
        patch_docker(self, fake)
        stub_ensure(self, True)
        with self.assertRaises(KeyboardInterrupt):
            c.run_one(record(), self.out)
        self.assertEqual(fake.joined()[-1], "rmi img:1")

    def test_run_one_writes_json_for_early_error_without_base_gold_keys(self):
        fake = FakeDocker(lambda a, d: RuntimeError("no such image") if a[0] == "run" else None)
        patch_docker(self, fake)
        stub_ensure(self, False)
        ev = c.run_one(record(), self.out)
        self.assertFalse(ev["verdict"]["ok"])
        saved = json.loads((self.out / "o__r-1" / "control.json").read_text())
        self.assertFalse(saved["verdict"]["ok"])
        self.assertIs(saved["verdict"]["error"], True)

    def test_run_one_refuses_unsafe_id(self):
        patch_docker(self, FakeDocker())
        stub_ensure(self, False)
        with self.assertRaises(c.ControlError):
            c.run_one(record(instance_id="../evil"), self.out)

    def test_run_one_writes_utf8_whatever_the_locale(self):
        # an ASCII locale (no UTF-8 mode, no coercion) makes the default text encoding ASCII
        here = os.path.dirname(os.path.abspath(__file__))
        code = (
            "import json, sys, multilang_control as c\n"
            "c.ensure_image = lambda image, marker=None: False\n"
            "c.control = lambda record, *, container, timeout=3600: "
            "({'id': 'o__r-1', 'note': 'caf\\u00e9', 'verdict': {'ok': True}}, 'b\\u00e9', 'g\\u00e9')\n"
            "c.run_one({'instance_id': 'o__r-1', 'docker_image': 'img:1'}, sys.argv[1])\n"
        )
        env = {"PATH": os.environ.get("PATH", "/usr/bin:/bin"), "LC_ALL": "C", "PYTHONUTF8": "0",
               "PYTHONCOERCECLOCALE": "0"}
        res = subprocess.run([sys.executable, "-B", "-c", code, str(self.out)], cwd=here, env=env,
                             capture_output=True, text=True)
        self.assertEqual(res.returncode, 0, res.stderr[-2000:])
        d = self.out / "o__r-1"
        self.assertEqual((d / "base.log").read_bytes(), "bé".encode("utf-8"))
        self.assertEqual((d / "gold.log").read_bytes(), "gé".encode("utf-8"))
        self.assertEqual(json.loads((d / "control.json").read_bytes().decode("utf-8"))["note"], "café")


class MainTests(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.dir = Path(tmp.name)
        self.rule = self.dir / "rule.json"
        self.rule.write_text(json.dumps(RULE))
        self.recs = self.dir / "r.jsonl"
        self.out = self.dir / "out"
        self.ran = []
        old = c.run_one

        def fake_run_one(rec, out_dir, *, timeout=3600):
            self.ran.append(rec["instance_id"])
            return {"id": rec["instance_id"], "verdict": {"ok": False, "reasons": ["why"]}}
        c.run_one = fake_run_one
        self.addCleanup(setattr, c, "run_one", old)

    def write(self, *recs):
        self.recs.write_text("".join(json.dumps(r) + "\n" for r in recs))

    def main(self, *extra, sha=True):
        import contextlib
        import io
        buf = io.StringIO()
        pin = ["--sha256", hashlib.sha256(self.recs.read_bytes()).hexdigest()] if sha else []
        with contextlib.redirect_stdout(buf):
            rc = c.main(["--records", str(self.recs), "--rule", str(self.rule),
                         "--out", str(self.out), *pin, *extra])
        return rc, buf.getvalue()

    def test_main_skips_ids_with_existing_control_json(self):
        self.write(full_record(instance_id="a__a-1"), full_record(instance_id="b__b-2"))
        (self.out / "a__a-1").mkdir(parents=True)
        (self.out / "a__a-1" / "control.json").write_text('{"verdict": {"ok": true}}')
        rc, text = self.main("--ids", "a__a-1", "b__b-2")
        self.assertEqual(rc, 0)
        self.assertEqual(self.ran, ["b__b-2"])
        self.assertIn("b__b-2 REJECT why", text)

    def test_main_ids_overrides_selection_order_but_not_the_cutoff(self):
        self.write(full_record(instance_id="a__a-1"),
                   full_record(instance_id="old__o-1", created_at="2024-01-01T00:00:00Z"),
                   full_record(instance_id="c__c-3"))
        rc, text = self.main("--ids", "c__c-3", "old__o-1", "a__a-1", "nope__n-0")
        self.assertEqual(rc, 0)
        self.assertEqual(self.ran, ["c__c-3", "a__a-1"])  # given order; size filter bypassed
        self.assertIn("REFUSED old__o-1", text)
        self.assertIn("REFUSED nope__n-0", text)

    def test_main_without_ids_uses_select_eligible_and_limit(self):
        self.rule.write_text(json.dumps(dict(RULE, min_files=1)))
        self.write(full_record(instance_id="a__a-1"), full_record(instance_id="b__b-2"))
        rc, _ = self.main("--limit", "1")
        self.assertEqual(rc, 0)
        self.assertEqual(len(self.ran), 1)

    def test_main_without_ids_applies_the_size_filter(self):
        self.write(full_record(instance_id="a__a-1"))  # RULE demands 99 files or lines
        self.main()
        self.assertEqual(self.ran, [])

    def test_main_ignore_size_selects_small_records_in_draw_order_but_keeps_the_cutoff(self):
        import select_tasks
        ids = [f"s__s-{i}" for i in range(6)]
        self.write(*[full_record(instance_id=i) for i in ids],
                   full_record(instance_id="old__o-1", created_at="2024-01-01T00:00:00Z"))
        rc, _ = self.main("--ignore-size")
        self.assertEqual(rc, 0)
        self.assertEqual(self.ran, sorted(ids, key=lambda i: (select_tasks.draw_key(RULE, i), i)))

    def test_main_skipped_pull_does_not_abort_the_run(self):
        self.write(full_record(instance_id="a__a-1"), full_record(instance_id="b__b-2"))

        def boom(rec, out_dir, *, timeout=3600):
            self.ran.append(rec["instance_id"])
            if rec["instance_id"] == "a__a-1":
                raise c.ControlError("disk low")
            return {"id": rec["instance_id"], "verdict": {"ok": True, "reasons": []}}
        c.run_one = boom
        rc, text = self.main("--ids", "a__a-1", "b__b-2")
        self.assertEqual(rc, 0)
        self.assertEqual(self.ran, ["a__a-1", "b__b-2"])
        self.assertIn("SKIPPED a__a-1 disk low", text)
        self.assertIn("b__b-2 ok", text)

    def test_main_counts_each_outcome_separately(self):
        ids = ["ok__o-1", "rej__r-1", "err__e-1", "skip__s-1", "done__d-1"]
        self.write(*[full_record(instance_id=i) for i in ids])
        (self.out / "done__d-1").mkdir(parents=True)
        (self.out / "done__d-1" / "control.json").write_text('{"verdict": {"ok": true}}')
        verdicts = {"ok__o-1": {"ok": True, "reasons": []},
                    "rej__r-1": {"ok": False, "reasons": ["fail-to-pass test passes at base"]},
                    "err__e-1": {"ok": False, "error": True, "reasons": ["control error: x"]}}

        def fake(rec, out_dir, *, timeout=3600):
            if rec["instance_id"] == "skip__s-1":
                raise c.ControlError("disk low")
            return {"id": rec["instance_id"], "verdict": verdicts[rec["instance_id"]]}
        c.run_one = fake
        rc, text = self.main("--ids", *ids, "nope__n-0")
        self.assertEqual(rc, 0)
        self.assertIn("err__e-1 ERROR control error: x", text)
        self.assertIn("rej__r-1 REJECT fail-to-pass test passes at base", text)
        self.assertEqual(text.strip().splitlines()[-1],
                         "ran 3: ok 1, REJECT 1, ERROR 1; SKIPPED 1, REFUSED 1, already done 1")

    def test_main_reruns_a_task_whose_previous_run_was_an_error(self):
        self.write(full_record(instance_id="a__a-1"))
        (self.out / "a__a-1").mkdir(parents=True)
        (self.out / "a__a-1" / "control.json").write_text(
            '{"verdict": {"ok": false, "error": true, "reasons": ["control error: x"]}}')
        _, text = self.main("--ids", "a__a-1")
        self.assertEqual(self.ran, ["a__a-1"])
        self.assertIn("RERUN a__a-1 previous run was an error", text)

    def test_main_refuses_a_records_file_with_the_wrong_hash_before_any_docker_call(self):
        self.write(full_record(instance_id="a__a-1"))
        fake = FakeDocker()
        patch_docker(self, fake)
        for extra, sha in ((("--sha256", "0" * 64), False), ((), False)):  # explicit, then the default
            rc, text = self.main("--ids", "a__a-1", *extra, sha=sha)
            self.assertNotEqual(rc, 0)
            self.assertIn("STOP", text)
            self.assertIn("sha256", text)
        self.assertIn(c.records_mod.DATASET_SHA256, text)  # the default is the pinned constant
        self.assertEqual(self.ran, [])
        self.assertEqual(fake.calls, [])

    def test_main_takes_a_lock_and_refuses_when_it_is_held(self):
        self.write(full_record(instance_id="a__a-1"))
        fd = c._lock(self.out)
        self.assertIsNotNone(fd)
        try:
            rc, text = self.main("--ids", "a__a-1")
        finally:
            os.close(fd)
        self.assertNotEqual(rc, 0)
        self.assertIn(f"LOCKED {self.out}", text)
        self.assertEqual(self.ran, [])
        rc, _ = self.main("--ids", "a__a-1")  # released: the next run proceeds
        self.assertEqual((rc, self.ran), (0, ["a__a-1"]))

    def test_main_holds_the_lock_and_the_signal_handlers_while_running(self):
        self.write(full_record(instance_id="a__a-1"))
        seen = {}

        def probe(rec, out_dir, *, timeout=3600):
            seen["lock_free"] = c._lock(self.out) is not None
            seen["term"] = signal.getsignal(signal.SIGTERM)
            seen["hup"] = signal.getsignal(signal.SIGHUP)
            return {"id": rec["instance_id"], "verdict": {"ok": True, "reasons": []}}
        c.run_one = probe
        before = (signal.getsignal(signal.SIGTERM), signal.getsignal(signal.SIGHUP))
        self.main("--ids", "a__a-1")
        self.assertEqual(seen, {"lock_free": False, "term": c._raise_interrupt, "hup": c._raise_interrupt})
        self.assertEqual((signal.getsignal(signal.SIGTERM), signal.getsignal(signal.SIGHUP)), before)

    def test_sigterm_during_a_run_unwinds_as_keyboard_interrupt_and_restores_handlers(self):
        self.write(full_record(instance_id="a__a-1"))

        def killed(rec, out_dir, *, timeout=3600):
            os.kill(os.getpid(), signal.SIGTERM)
            time.sleep(5)  # the handler interrupts this
            self.fail("SIGTERM did not interrupt the run")
        c.run_one = killed
        before = signal.getsignal(signal.SIGTERM)
        with self.assertRaises(KeyboardInterrupt):
            self.main("--ids", "a__a-1")
        self.assertEqual(signal.getsignal(signal.SIGTERM), before)
        fd = c._lock(self.out)  # and the lock was released
        self.assertIsNotNone(fd)
        os.close(fd)

    def test_raise_interrupt_raises_keyboard_interrupt(self):
        for sig in (signal.SIGTERM, signal.SIGHUP):
            with self.assertRaises(KeyboardInterrupt):
                c._raise_interrupt(sig, None)

    def test_main_never_imports_run_or_arms(self):
        code = ("import sys, multilang_control; "
                "print([m for m in ('run','arms','registration') if m in sys.modules])")
        res = subprocess.run([sys.executable, "-B", "-c", code],
                             cwd=os.path.dirname(os.path.abspath(__file__)),
                             capture_output=True, text=True)
        self.assertEqual(res.stdout.strip(), "[]", res.stderr)


class LockTests(unittest.TestCase):
    def test_a_second_lock_on_the_same_out_dir_fails_until_the_first_is_released(self):
        with tempfile.TemporaryDirectory() as d:
            out_dir = Path(d) / "new" / "out"
            first = c._lock(out_dir)  # creates the directory
            self.assertIsNotNone(first)
            self.assertTrue((out_dir / ".lock").exists())
            self.assertIsNone(c._lock(out_dir))
            os.close(first)
            again = c._lock(out_dir)
            self.assertIsNotNone(again)
            os.close(again)


class RmiFailureTests(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.out = Path(tmp.name)
        stub_ensure(self, True)

    def handler(self, rmi):
        inner = happy_handler({})

        def h(args, data):
            if args[0] == "rmi":
                return rmi
            return inner(args, data)
        return h

    def test_failed_rmi_is_recorded_in_control_json(self):
        patch_docker(self, FakeDocker(self.handler(out("image is in use", rc=1))))
        ev = c.run_one(record(), self.out)
        self.assertIs(ev["image_removed"], False)
        saved = json.loads((self.out / "o__r-1" / "control.json").read_text())
        self.assertIs(saved["image_removed"], False)
        self.assertIn("image is in use", saved["image_remove_error"])

    def test_successful_rmi_is_recorded(self):
        patch_docker(self, FakeDocker(self.handler(out())))
        self.assertIs(c.run_one(record(), self.out)["image_removed"], True)

    def test_rmi_timeout_does_not_mask_a_control_exception(self):
        inner = happy_handler({})

        def h(args, data):
            if args[0] == "rmi":
                return subprocess.TimeoutExpired("docker", 1)
            if "reset --hard" in " ".join(args):
                return KeyboardInterrupt()
            return inner(args, data)
        patch_docker(self, FakeDocker(h))
        with self.assertRaises(KeyboardInterrupt):
            c.run_one(record(), self.out)

    def test_rmi_error_is_recorded_not_raised(self):
        patch_docker(self, FakeDocker(self.handler(RuntimeError("daemon gone"))))
        ev = c.run_one(record(), self.out)
        self.assertIs(ev["image_removed"], False)
        self.assertIn("daemon gone", ev["image_remove_error"])


class PullFailureTests(unittest.TestCase):
    def fake(self, pull_error, appears):
        state = {"pulled": False}

        def h(args, data):
            if args[0] == "image":
                return out(rc=0 if state["pulled"] else 1)
            if args[0] == "pull":
                state["pulled"] = appears
                return pull_error
        fake = FakeDocker(h)
        patch_docker(self, fake)
        old = c.free_gb
        c.free_gb = lambda path="/": 500
        self.addCleanup(setattr, c, "free_gb", old)
        return fake

    def test_failed_pull_whose_image_appears_is_removed_unforced(self):
        for err in (RuntimeError("broken pipe"), subprocess.TimeoutExpired("docker", 1)):
            fake = self.fake(err, True)
            with self.assertRaises(c.ControlError):
                c.ensure_image("img:1")
            self.assertEqual(fake.joined()[-1], "rmi img:1")

    def test_failed_pull_without_image_does_not_call_rmi(self):
        fake = self.fake(RuntimeError("denied"), False)
        with self.assertRaises(c.ControlError):
            c.ensure_image("img:1")
        self.assertFalse(any(x.startswith("rmi") for x in fake.joined()))

    def test_interrupted_pull_removes_an_image_that_appears_and_reraises(self):
        fake = self.fake(KeyboardInterrupt(), True)
        with self.assertRaises(KeyboardInterrupt):
            c.ensure_image("img:1")
        self.assertEqual(fake.joined()[-1], "rmi img:1")


class ResumeMarkerTests(MainTests):
    def mark(self, text):
        d = self.out / "a__a-1"
        d.mkdir(parents=True)
        (d / "control.json").write_text(text)

    def test_truncated_control_json_is_rerun(self):
        self.write(full_record(instance_id="a__a-1"))
        self.mark('{"id": "a__a-1", "verd')
        rc, text = self.main("--ids", "a__a-1")
        self.assertEqual(self.ran, ["a__a-1"])
        self.assertIn("RERUN a__a-1 control.json unreadable", text)

    def test_verdictless_control_json_is_rerun(self):
        self.write(full_record(instance_id="a__a-1"))
        self.mark('{"id": "a__a-1"}')
        self.main("--ids", "a__a-1")
        self.assertEqual(self.ran, ["a__a-1"])

    def test_non_object_control_json_is_rerun(self):
        self.write(full_record(instance_id="a__a-1"))
        self.mark("[]")
        self.main("--ids", "a__a-1")
        self.assertEqual(self.ran, ["a__a-1"])

    def test_valid_control_json_is_skipped(self):
        self.write(full_record(instance_id="a__a-1"))
        self.mark('{"verdict": {"ok": true}}')
        self.main("--ids", "a__a-1")
        self.assertEqual(self.ran, [])

    def test_valid_rejection_is_skipped_not_rerun(self):
        self.write(full_record(instance_id="a__a-1"))
        self.mark('{"verdict": {"ok": false, "reasons": ["fail-to-pass test passes at base"]}}')
        self.main("--ids", "a__a-1")
        self.assertEqual(self.ran, [])

    def test_main_warns_when_the_image_was_not_removed(self):
        self.write(full_record(instance_id="a__a-1"))
        c.run_one = lambda rec, out_dir, *, timeout=3600: {
            "id": "a__a-1", "verdict": {"ok": True, "reasons": []}, "image_removed": False,
            "image_remove_error": "in use"}
        _, text = self.main("--ids", "a__a-1")
        self.assertIn("WARN rmi failed img:1", text)

    def test_main_warns_when_the_container_was_not_removed(self):
        self.write(full_record(instance_id="a__a-1"))
        c.run_one = lambda rec, out_dir, *, timeout=3600: {
            "id": "a__a-1", "verdict": {"ok": True, "reasons": []}, "container_removed": False,
            "container_remove_error": "TimeoutExpired"}
        _, text = self.main("--ids", "a__a-1")
        self.assertIn("WARN container not removed sab-ml-ctl-a__a-1 TimeoutExpired", text)


class AtomicWriteTests(unittest.TestCase):
    def test_control_json_is_written_via_tmp_and_os_replace(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        out_dir = Path(tmp.name)
        seen = []
        real = c.os.replace

        def spy(src, dst):
            seen.append((Path(src).name, Path(dst).name, Path(dst).exists(),
                         Path(src).read_text().endswith("}\n")))
            return real(src, dst)
        c.os.replace = spy
        self.addCleanup(setattr, c.os, "replace", real)
        stub_ensure(self, False)
        patch_docker(self, FakeDocker(happy_handler({})))
        c.run_one(record(), out_dir)
        self.assertEqual(seen, [("control.json.tmp", "control.json", False, True)])
        self.assertFalse((out_dir / "o__r-1" / "control.json.tmp").exists())

    def test_failed_replace_leaves_no_control_json(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        real = c.os.replace

        def boom(src, dst):
            raise OSError("ENOSPC")
        c.os.replace = boom
        self.addCleanup(setattr, c.os, "replace", real)
        stub_ensure(self, False)
        patch_docker(self, FakeDocker(happy_handler({})))
        with self.assertRaises(OSError):
            c.run_one(record(), Path(tmp.name))
        self.assertFalse((Path(tmp.name) / "o__r-1" / "control.json").exists())


if __name__ == "__main__":
    unittest.main()
