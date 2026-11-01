import hashlib
import json
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

import multilang_tasks as mt
import tasks

BASE = "a92166897f4b3d5f3910101d9380796fc2ce15bf"
PARSER = "def parser(log):\n    return dict(l.split('=') for l in log.split())\n"
RUN1 = "fn first_fn(a_arg: u32, b_arg: u32) -> u32 { compute_alpha(a_arg, b_arg) + offset_value }"
RUN2 = "fn second_fn(c_arg: u64, d_arg: u64) -> u64 { compute_omega(c_arg, d_arg) * scale_factor }"
PATCH = (f"diff --git a/src/a.rs b/src/a.rs\n--- a/src/a.rs\n+++ b/src/a.rs\n@@ -1,3 +1,5 @@\n"
         f"+{RUN1}\n context\n+{RUN2}\n")
TEST_PATCH = ("diff --git a/tests/t.rs b/tests/t.rs\n--- a/tests/t.rs\n+++ b/tests/t.rs\n@@ -1 +1,2 @@\n"
              "+fn test_new_behaviour() { assert_eq!(widget_total(), expected_widget_total_value); }\n")


def record(**kw):
    r = {"instance_id": "o__r-1", "docker_image": "img:1", "base_commit": BASE,
         "patch": PATCH, "test_patch": TEST_PATCH, "problem_statement": "Fix it.\r\nPlease.\r\n",
         "FAIL_TO_PASS": ["t::f1"], "PASS_TO_PASS": ["t::p1", "t::p2", "t::p3"],
         "test_cmds": ["run"], "print_cmds": ["cat log"], "log_parser": PARSER}
    r.update(kw)
    return r


def write_control(d, base, gold, *, workdir="/testbed", mismatch=None, error=False, rid="o__r-1"):
    """control.json + base.log + gold.log as multilang_control.run_one writes them."""
    d = Path(d)
    d.mkdir(parents=True, exist_ok=True)
    (d / "base.log").write_text("".join(f"{k}={v}\n" for k, v in base.items()))
    (d / "gold.log").write_text("".join(f"{k}={v}\n" for k, v in gold.items()))
    ev = {"id": rid, "workdir": workdir, "verdict": {"ok": False, "error": error},
          "gold_repeat_mismatch": mismatch or {"count": 0, "names": []}}
    (d / "control.json").write_text(json.dumps(ev))
    return d / "control.json"


GOOD_BASE = {"t::f1": "fail", "t::p1": "pass", "t::p2": "fail", "t::p3": "pass"}
GOOD_GOLD = {"t::f1": "pass", "t::p1": "pass", "t::p2": "fail", "t::p3": "pass"}


class LiveDockerfileTests(unittest.TestCase):
    def test_live_dockerfile_resets_to_base_and_keeps_target(self):
        text = mt.live_dockerfile("src@sha256:" + "a" * 64, record(), "/testbed")
        self.assertTrue(text.startswith("FROM src@sha256:" + "a" * 64 + "\nUSER root\n"))
        self.assertIn(f"git reset --hard -q {BASE}", text)
        self.assertIn("git clean -fdq -e target", text)
        for line in text.splitlines():
            if "rm -rf" in line:
                self.assertNotIn("target", line, "target/ is kept, never removed")
        self.assertIn("printf '/target\\n' >> .git/info/exclude", text)

    def test_live_dockerfile_removes_reports_in_both_locations(self):
        text = mt.live_dockerfile("src", record(), "/testbed/repo")
        self.assertIn("rm -rf /testbed/repo/reports /testbed/reports", text)
        self.assertIn("rm -rf /testbed/repo/.git /testbed/.git", text)
        same = mt.live_dockerfile("src", record(), "/testbed")
        self.assertIn("rm -rf /testbed/reports \\", same)  # one path, not listed twice

    def test_live_dockerfile_creates_agent_user_and_clean_one_commit_repo(self):
        text = mt.live_dockerfile("src", record(), "/testbed")
        self.assertIn("useradd --uid 1000 --gid 1000 --create-home", text)
        self.assertIn("groupadd --gid 1000 agent", text)
        self.assertIn("chown -R 1000:1000 /testbed /home/agent", text)
        self.assertIn("git add . && git commit -qm 'Benchmark base source'", text)
        self.assertIn("ENV HOME=/home/agent CARGO_NET_OFFLINE=true\nUSER 1000:1000\nWORKDIR /testbed\n", text)
        last = text.rstrip("\n").splitlines()[-1]
        self.assertTrue(last.startswith("RUN test -z \"$(git status --porcelain)\""))
        self.assertIn('git rev-list --count HEAD)" = 1', last)
        self.assertIn("Benchmark base source", last)
        self.assertLess(text.index("USER root"), text.index("USER 1000:1000"))

    def test_live_dockerfile_refuses_a_shell_unsafe_commit_or_workdir(self):
        with self.assertRaises(mt.LiveRefused):
            mt.live_dockerfile("src", record(base_commit="x; rm -rf /"), "/testbed")
        with self.assertRaises(mt.LiveRefused):
            mt.live_dockerfile("src", record(), "/testbed; echo")
        with self.assertRaises(mt.LiveRefused):
            mt.live_dockerfile("src", record(), "/tmp/x")


class AssembleLiveTests(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.addCleanup(lambda: __import__("shutil").rmtree(self.tmp, ignore_errors=True))
        self.builds = []
        self.base_files = {"src/a.rs": f"// base\n{RUN1}\n", "README.md": "readme\n"}

    def run_assemble(self, rec=None):
        def export(image, paths, dest, **kw):
            for rel, text in self.base_files.items():
                p = Path(dest) / "testbed" / rel
                p.parent.mkdir(parents=True, exist_ok=True)
                p.write_text(text)
            return ["testbed"]

        def build(ctx, text, tag, log, **kw):
            self.builds.append((text, tag, kw))
            Path(log).write_text("")
            return "sha256:1"

        with mock.patch.object(tasks, "_source_ref", return_value="src@sha256:" + "a" * 64), \
                mock.patch.object(tasks, "_docker_build", side_effect=build), \
                mock.patch.object(tasks, "export_tree", side_effect=export):
            return mt.assemble_live(rec or record(), self.tmp / "b", "sab-task-o__r-1:t1", "/testbed")

    def test_assemble_live_writes_prompt_with_prefix_and_lf(self):
        needles, prompt, rec = self.run_assemble()
        raw = (self.tmp / "b" / "prompt.txt").read_bytes()
        self.assertEqual(raw, (tasks.PROMPT_PREFIX + "Fix it.\nPlease.\n").encode())
        self.assertNotIn(b"\r", raw)
        self.assertEqual(prompt, self.tmp / "b" / "prompt.txt")
        self.assertEqual((self.tmp / "b" / "gold.patch").read_text(), PATCH)
        self.assertEqual(json.loads((self.tmp / "b" / "record.json").read_text())["instance_id"], "o__r-1")
        assy = json.loads((self.tmp / "b" / "assembly.json").read_text())
        self.assertEqual((assy["kind"], assy["workdir"], assy["base_commit"]), ("live", "/testbed", BASE))
        text, tag, kw = self.builds[0]
        self.assertEqual(tag, "sab-task-o__r-1:t1")
        self.assertIn(f"git reset --hard -q {BASE}", text)
        self.assertIn("FROM src@sha256:", text)

    def test_assemble_live_needles_exclude_strings_already_in_the_base_tree(self):
        needles, _, rec = self.run_assemble()
        self.assertGreater(needles.dropped["shingle"], 0)
        shingles = [n for n in needles.needles if " " in n]
        self.assertTrue(any("second_fn" in s for s in shingles))
        self.assertFalse(any("first_fn" in s or "compute_alpha" in s for s in shingles),
                         "run 1 is already in the base tree, so it is no leak evidence")
        self.assertIn("test_new_behaviour", needles.needles)  # added by test_patch, not in base
        self.assertEqual(rec["needles"]["dropped"], needles.dropped)

    def test_assemble_live_reads_a_nextest_test_id_with_a_dollar_separator(self):
        rec = record(FAIL_TO_PASS=["crate-x::bin_name$test_new_behaviour"])
        needles, _, _ = self.run_assemble(rec)
        self.assertIn("test_new_behaviour", needles.needles)
        self.assertFalse(any("$" in n for n in needles.needles))


    def test_needle_name_reads_the_live_test_id_shapes(self):
        cases = {
            "obscura-cdp::bin$test_one": "test_one",
            "Doc-tests rmcp::crates/rmcp/src/t.rs - transport::Cfg::custom_headers (line 1725) - compile":
                "custom_headers",
            "Doc-tests rmcp::crates/rmcp/src/m.rs - model::ElicitRequestParams (line 3535)":
                "ElicitRequestParams",
            "Doc-tests rmcp::crates/rmcp/src/t.rs - transport::Tr<C>::with_client (line 1596) - compile":
                "with_client",
            "unittests src/lib.rs::transport::tests::clear_pending": "clear_pending",
            "tests/a/main.rs (target/debug/deps/a-1d)::virtual_branches::init::boots_ok": "boots_ok",
            "target/debug/deps/hir_ty-49::mir::lower::tests::no_panic": "no_panic",
            "polars_core::frame::tests::test_x": "test_x",
        }
        for raw, want in cases.items():
            with self.subTest(raw):
                self.assertEqual(mt.needle_name(raw), want)
        needles, _, _ = self.run_assemble(record(FAIL_TO_PASS=[
            "Doc-tests rmcp::crates/rmcp/src/t.rs - transport::Cfg::custom_headers (line 1725) - compile"]))
        self.assertIn("custom_headers", needles.needles)


    def test_assemble_live_refuses_to_merge_into_an_existing_dir(self):
        (self.tmp / "b").mkdir()
        with self.assertRaises(FileExistsError):
            self.run_assemble()
        self.assertEqual(self.builds, [])


class BuildLiveTests(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.addCleanup(lambda: __import__("shutil").rmtree(self.tmp, ignore_errors=True))
        self.agent_calls = []

    def build(self, control_json, rec=None, out=None):
        def assemble(record_, out_, task_image, workdir):
            Path(out_).mkdir(parents=True)
            (Path(out_) / "record.json").write_text(json.dumps(record_))
            self.assembled = (task_image, workdir)
            return SimpleNamespace(kept={}), Path(out_) / "prompt.txt", {"id": record_["instance_id"]}

        with mock.patch.object(tasks, "short_tag", return_value="t1"), \
                mock.patch.object(mt, "assemble_live", side_effect=assemble), \
                mock.patch.object(tasks, "build_agent_image",
                                  side_effect=lambda *a: self.agent_calls.append(a)):
            return mt.build_live(rec or record(), out or self.tmp / "task" / "build", control_json)

    def test_build_live_bundle_records_workdir_expected_and_control_sha256(self):
        cj = write_control(self.tmp / "c", GOOD_BASE, GOOD_GOLD)
        b = self.build(cj)
        on_disk = json.loads((self.tmp / "task" / "bundle.json").read_text())
        self.assertEqual(on_disk, b.record)
        out = str(self.tmp / "task" / "build")
        self.assertEqual(on_disk, {
            "kind": "live", "out": out, "task_image": "sab-task-o__r-1:t1",
            "image": "sab-agent-cs-o__r-1:t1", "record_path": out + "/record.json",
            "workdir": "/testbed",
            "expected": {"fail_to_pass": 1, "pass_to_pass_kept": 2, "pass_to_pass_dropped": 1,
                         "dropped": ["t::p2"]},
            "control_sha256": hashlib.sha256(cj.read_bytes()).hexdigest()})
        self.assertNotIn("template", on_disk)
        self.assertEqual(b.template, "")
        self.assertEqual(self.assembled, ("sab-task-o__r-1:t1", "/testbed"))
        self.assertEqual(self.agent_calls[0][:2], ("sab-task-o__r-1:t1", "sab-agent-cs-o__r-1:t1"))

    def test_build_live_refuses_a_control_that_is_not_relaxed_ok(self):
        cases = {
            "f2p passes at base": (dict(GOOD_BASE, **{"t::f1": "pass"}), GOOD_GOLD, {}),
            "f2p fails at gold": (GOOD_BASE, dict(GOOD_GOLD, **{"t::f1": "fail"}), {}),
            "f2p missing at gold": (GOOD_BASE, {k: v for k, v in GOOD_GOLD.items() if k != "t::f1"}, {}),
            "kept test differs between gold runs": (GOOD_BASE, GOOD_GOLD,
                                                    {"mismatch": {"count": 1, "names": ["t::p1"]}}),
            "mismatch list truncated": (GOOD_BASE, GOOD_GOLD,
                                        {"mismatch": {"count": 11, "names": ["t::p2"] * 10}}),
            "control error": (GOOD_BASE, GOOD_GOLD, {"error": True}),
            "other task's control": (GOOD_BASE, GOOD_GOLD, {"rid": "o__other-2"}),
            "workdir the runner cannot use": (GOOD_BASE, GOOD_GOLD, {"workdir": "/testbed/repo"}),
        }
        for name, (base, gold, kw) in cases.items():
            with self.subTest(name):
                cj = write_control(self.tmp / name.replace(" ", "_"), base, gold, **kw)
                with self.assertRaises(mt.LiveRefused):
                    self.build(cj)
                self.assertEqual(self.agent_calls, [])
                self.assertFalse((self.tmp / "task").exists(), "nothing is written for a refusal")

    def test_a_dropped_test_that_differs_between_gold_runs_is_not_a_refusal(self):
        cj = write_control(self.tmp / "c", GOOD_BASE, GOOD_GOLD,
                           mismatch={"count": 1, "names": ["t::p2"]})
        self.assertEqual(self.build(cj).record["expected"]["dropped"], ["t::p2"])

    def test_the_relaxed_set_keeps_only_tests_that_pass_at_gold(self):
        gold = dict(GOOD_GOLD, **{"t::p3": "skipped"})  # absent or not "pass" is dropped
        gold.pop("t::p1")
        cj = write_control(self.tmp / "c", GOOD_BASE, gold)
        e = mt.relaxed_expected(record(), cj)
        self.assertEqual(e["dropped"], ["t::p1", "t::p2", "t::p3"])
        self.assertEqual((e["pass_to_pass_kept"], e["fail_to_pass"]), (0, 1))


class ArmLiveTests(unittest.TestCase):
    def test_arm_live_tags_run_image_builds_arm_and_drops_template(self):
        tmp = Path(tempfile.mkdtemp())
        self.addCleanup(lambda: __import__("shutil").rmtree(tmp, ignore_errors=True))
        bj = tmp / "bundle.json"
        bj.write_text(json.dumps({"kind": "live", "out": str(tmp / "build"), "template": "",
                                  "task_image": "sab-task-o__r-1:t1", "image": "sab-agent-cs-o__r-1:t1"}))
        calls = []

        def docker(*args, **kw):
            calls.append(args)
            if args[:2] == ("image", "inspect") and "--format" in args and "{{.Id}}" in args:
                return SimpleNamespace(stdout="sha256:run\n", returncode=0)
            return SimpleNamespace(stdout="", returncode=1)

        built = []
        with mock.patch.object(mt.pilot, "docker", docker), \
                mock.patch.object(mt.arms, "exec_only_image", side_effect=lambda *a: built.append(a)):
            rec = mt.arm_live(bj)
        self.assertIn(("tag", "sab-agent-cs-o__r-1:t1", "sab-run-o__r-1:t1"), calls)
        self.assertEqual(built[0][:2], ("sab-run-o__r-1:t1", "sab-arm-o__r-1:t1"))
        self.assertEqual((rec["run_image"], rec["arm_image"], rec["image"]),
                         ("sab-run-o__r-1:t1", "sab-arm-o__r-1:t1", "sab-arm-o__r-1:t1"))
        self.assertNotIn("template", json.loads(bj.read_text()))


if __name__ == "__main__":
    unittest.main()
