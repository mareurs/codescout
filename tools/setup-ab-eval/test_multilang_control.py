import base64
import subprocess
import unittest
from types import SimpleNamespace

import multilang_control as c

PARSER = "def parser(log):\n    return dict(l.split('=') for l in log.split())\n"
BASE = "deadbeef"


def out(stdout="", rc=0):
    return SimpleNamespace(stdout=stdout, returncode=rc)


def b64(text):
    """What a print command's wrapper prints: the log, base64 encoded."""
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
        if "git apply" in j and data == "GOLD":
            state["gold"] = True
        if "cat log" in j and "base64" in j:
            return out(b64("t1=pass t2=pass" if state.get("gold") else "t1=fail t2=pass"))
        return None
    return handler


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
        j = fake.joined()

        def idx(sub, start=0):
            for i in range(start, len(j)):
                if sub in j[i]:
                    return i
            self.fail(f"{sub!r} not found after {start}")
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
        self.assertIn(">/dev/null 2>&1", first["args"][-1])  # test output stays in the container
        self.assertTrue(all(not x["check"] for x in fake.calls))

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
        self.assertEqual(fake.joined()[-1], "rm -f sab-ml-ctl-x")

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


class RunOneTests(unittest.TestCase):
    def setUp(self):
        import tempfile
        from pathlib import Path
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.out = Path(tmp.name)

    def stub(self, pulled):
        old = c.ensure_image
        c.ensure_image = lambda image: pulled
        self.addCleanup(setattr, c, "ensure_image", old)

    def test_run_one_removes_only_an_image_it_pulled(self):
        for pulled in (True, False):
            fake = FakeDocker(happy_handler({}))
            patch_docker(self, fake)
            self.stub(pulled)
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
        self.stub(True)
        with self.assertRaises(KeyboardInterrupt):
            c.run_one(record(), self.out)
        self.assertEqual(fake.joined()[-1], "rmi img:1")

    def test_run_one_writes_json_for_early_error_without_base_gold_keys(self):
        import json
        fake = FakeDocker(lambda a, d: RuntimeError("no such image") if a[0] == "run" else None)
        patch_docker(self, fake)
        self.stub(False)
        ev = c.run_one(record(), self.out)
        self.assertFalse(ev["verdict"]["ok"])
        saved = json.loads((self.out / "o__r-1" / "control.json").read_text())
        self.assertFalse(saved["verdict"]["ok"])

    def test_run_one_refuses_unsafe_id(self):
        patch_docker(self, FakeDocker())
        self.stub(False)
        with self.assertRaises(c.ControlError):
            c.run_one(record(instance_id="../evil"), self.out)


class MainTests(unittest.TestCase):
    def setUp(self):
        import json
        import tempfile
        from pathlib import Path
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
        import json
        self.recs.write_text("".join(json.dumps(r) + "\n" for r in recs))

    def main(self, *extra):
        import contextlib
        import io
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            rc = c.main(["--records", str(self.recs), "--rule", str(self.rule),
                         "--out", str(self.out), *extra])
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
        import json
        self.rule.write_text(json.dumps(dict(RULE, min_files=1)))
        self.write(full_record(instance_id="a__a-1"), full_record(instance_id="b__b-2"))
        rc, _ = self.main("--limit", "1")
        self.assertEqual(rc, 0)
        self.assertEqual(len(self.ran), 1)

    def test_main_without_ids_applies_the_size_filter(self):
        self.write(full_record(instance_id="a__a-1"))  # RULE demands 99 files or lines
        self.main()
        self.assertEqual(self.ran, [])

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

    def test_main_never_imports_run_or_arms(self):
        import os
        import subprocess as sp
        import sys
        code = ("import sys, multilang_control; "
                "print([m for m in ('run','arms','registration') if m in sys.modules])")
        res = sp.run([sys.executable, "-B", "-c", code], cwd=os.path.dirname(os.path.abspath(__file__)),
                     capture_output=True, text=True)
        self.assertEqual(res.stdout.strip(), "[]", res.stderr)


class RmiFailureTests(unittest.TestCase):
    def setUp(self):
        import tempfile
        from pathlib import Path
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.out = Path(tmp.name)
        old = c.ensure_image
        c.ensure_image = lambda image: True
        self.addCleanup(setattr, c, "ensure_image", old)

    def handler(self, rmi):
        inner = happy_handler({})

        def h(args, data):
            if args[0] == "rmi":
                return rmi
            return inner(args, data)
        return h

    def test_failed_rmi_is_recorded_in_control_json(self):
        import json
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

    def test_main_warns_when_the_image_was_not_removed(self):
        self.write(full_record(instance_id="a__a-1"))
        c.run_one = lambda rec, out_dir, *, timeout=3600: {
            "id": "a__a-1", "verdict": {"ok": True, "reasons": []}, "image_removed": False,
            "image_remove_error": "in use"}
        _, text = self.main("--ids", "a__a-1")
        self.assertIn("WARN rmi failed img:1", text)


class AtomicWriteTests(unittest.TestCase):
    def test_control_json_is_written_via_tmp_and_os_replace(self):
        import tempfile
        from pathlib import Path
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
        old = c.ensure_image
        c.ensure_image = lambda image: False
        self.addCleanup(setattr, c, "ensure_image", old)
        patch_docker(self, FakeDocker(happy_handler({})))
        c.run_one(record(), out_dir)
        self.assertEqual(seen, [("control.json.tmp", "control.json", False, True)])
        self.assertFalse((out_dir / "o__r-1" / "control.json.tmp").exists())

    def test_failed_replace_leaves_no_control_json(self):
        import tempfile
        from pathlib import Path
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        real = c.os.replace

        def boom(src, dst):
            raise OSError("ENOSPC")
        c.os.replace = boom
        self.addCleanup(setattr, c.os, "replace", real)
        old = c.ensure_image
        c.ensure_image = lambda image: False
        self.addCleanup(setattr, c, "ensure_image", old)
        patch_docker(self, FakeDocker(happy_handler({})))
        with self.assertRaises(OSError):
            c.run_one(record(), Path(tmp.name))
        self.assertFalse((Path(tmp.name) / "o__r-1" / "control.json").exists())


if __name__ == "__main__":
    unittest.main()
