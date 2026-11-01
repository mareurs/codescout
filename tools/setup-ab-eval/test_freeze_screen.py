"""Tests for freeze_screen: the screen plan, the registration run.py accepts, the refusals and
`check`. A temp git repo stands in for the checkout, a temp data dir holds fake bundles and controls,
fake inspect and `claude`. No model, token, container or real data dir is touched."""
import hashlib
import json
import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

import freeze_screen as fs
import frozen
import multilang_tasks as mt
import run
from test_multilang_tasks import GOOD_BASE, GOOD_GOLD, record, write_control

CLI_VERSION = "2.1.309 (Claude Code)"
IDS = list(fs.POOL_IDS)


def git(repo, *args):
    return subprocess.run(["git", "-C", str(repo), *args], check=True, capture_output=True,
                          text=True).stdout


def image_id(tag, gen=0):
    return "sha256:" + hashlib.sha256(f"{tag}#{gen}".encode()).hexdigest()


class Inspect:
    def __init__(self):
        self.rebuilt = set()

    def __call__(self, image):
        gen = 1 if image in self.rebuilt else 0
        if image.startswith("sab-arm-"):
            return image_id(image, gen), image_id(image.replace("sab-arm-", "sab-run-"))
        return image_id(image, gen), None


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


class Env:
    def __init__(self, tmp):
        self.tmp = Path(tmp)
        self.tool = self.tmp / "tool"
        self.data = self.tmp / "data"
        self.run_data = self.data / "live-screen"
        self.run_data.mkdir(parents=True)
        self.inspect = Inspect()
        # frozen arm inputs, recorded in the tool dir's stage3 registration
        self.froot = self.tmp / "frozen"
        (self.froot / "config").mkdir(parents=True)
        (self.froot / "config" / "CLAUDE.md").write_text("claude\n")
        (self.froot / "plugins" / "codescout-companion").mkdir(parents=True)
        (self.froot / "plugins" / "codescout-companion" / "x.md").write_text("plugin\n")
        self.trees = frozen.tree_hashes(self.froot, ["config/CLAUDE.md", "plugins/codescout-companion"])
        # the runner's manifest copy and the claude CLI
        self.cli = self.tmp / "claude"
        self.cli.write_text("#!/bin/sh\n")
        self.manifest = {"cli": {"path": str(self.cli), "sha256": sha(self.cli), "version": CLI_VERSION},
                         "plugins_cs": {"install_path": "/h/plugins/codescout-companion"},
                         "plugins_full": [{"name": "codescout-companion",
                                           "install_path": "/h/plugins/codescout-companion"}],
                         "full_config_files": ["/h/CLAUDE.md"]}
        (self.run_data / "setup-manifest.json").write_text(json.dumps(self.manifest))
        # per task fake dataset record, control, bundle
        self.records = {}
        for tid in IDS:
            repo = tid.rsplit("-", 1)[0].replace("__", "/")
            self.records[tid] = record(instance_id=tid, repo=repo)
        controls = self.data / "live" / "controls-batch"
        for tid in IDS:
            write_control(controls / tid, GOOD_BASE, GOOD_GOLD, rid=tid)
        self.pool = fs.make_pool(self.records, controls)
        self.runner = self.tmp / "runner-tasks"
        self.runner.mkdir()
        for tid in IDS:
            self.write_bundle(tid)
            os.symlink(self.data / "live-tasks" / tid, self.runner / tid)
        # the git checkout
        self.tool.mkdir()
        git(self.tool, "init", "-q")
        git(self.tool, "config", "user.email", "t@t")
        git(self.tool, "config", "user.name", "t")
        (self.tool / "a.py").write_text("x = 1\n")
        (self.tool / fs.STAGE3_REGISTRATION).write_text(json.dumps(
            {"frozen": {"root": str(self.froot), "trees": self.trees, "extra": 1}}))
        self.write_pool()
        self.commit("init")
        self.plan = fs.screen_plan(self.pool)
        self.plan_path = self.run_data / fs.PLAN_NAME
        fs.write_plan(self.plan, self.plan_path)
        self.out = self.tool / fs.REGISTRATION

    def write_pool(self):
        (self.tool / fs.POOL).write_text(json.dumps(self.pool, indent=2) + "\n")

    def commit(self, msg):
        git(self.tool, "add", "-A")
        git(self.tool, "commit", "-q", "-m", msg)

    def write_bundle(self, tid):
        d = self.data / "live-tasks" / tid
        (d / "build").mkdir(parents=True, exist_ok=True)
        (d / "build" / "record.json").write_text(json.dumps(self.records[tid]))
        (d / "build" / "prompt.txt").write_text(f"fix {tid}\n")
        cj = self.data / "live" / "controls-batch" / tid / "control.json"
        exp = mt.relaxed_expected(self.records[tid], cj)
        (d / "bundle.json").write_text(json.dumps({
            "kind": "live", "out": str(d / "build"), "task_image": f"sab-task-{tid}:t1",
            "image": f"sab-arm-{tid}:t1", "record_path": str(d / "build" / "record.json"),
            "workdir": "/testbed", "expected": exp, "control_sha256": sha(cj),
            "run_image": f"sab-run-{tid}:t1", "arm_image": f"sab-arm-{tid}:t1"}))

    def kw(self, **over):
        kw = dict(tool_dir=self.tool, data=self.data, run_data=self.run_data,
                  pool_path=self.tool / fs.POOL, plan_path=self.plan_path, out=self.out,
                  head_fn=lambda d: "h" * 40, inspect=self.inspect, runner_tasks_root=self.runner,
                  run_cli=lambda p: CLI_VERSION + "\n")
        kw.update(over)
        return kw

    def freeze(self, **over):
        return fs.freeze_screen(**self.kw(**over))

    def check(self, **over):
        return fs.check_screen(tool_dir=self.tool, data=self.data, run_data=self.run_data,
                               registration=self.out, inspect=over.get("inspect", self.inspect),
                               runner_tasks_root=over.get("runner_tasks_root", self.runner),
                               run_cli=lambda p: CLI_VERSION)


class Case(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.mkdtemp()
        self.addCleanup(shutil.rmtree, self.tmp, True)
        self.env = Env(self.tmp)

    def assertNothingWritten(self):
        self.assertFalse(self.env.out.exists())
        self.assertFalse((self.env.run_data / fs.GO_TEMPLATE).exists())
        self.assertFalse((self.env.run_data / "GO.json").exists())

    def refused(self, text, **over):
        with self.assertRaises(fs.Refused) as cm:
            self.env.freeze(**over)
        self.assertIn(text, str(cm.exception))
        self.assertNothingWritten()


class PlanTests(Case):
    def test_screen_plan_is_bare_only_two_models_two_reps_36_runs(self):
        plan = fs.screen_plan(self.env.pool, run.MODELS, 2, 7)
        self.assertEqual(len(plan), 36)
        self.assertEqual({s.arm for s in plan}, {"bare"})
        self.assertEqual({s.model for s in plan}, {"claude-sonnet-5-5", "claude-opus-5-5"})
        self.assertEqual({s.task_id for s in plan}, set(IDS))
        self.assertEqual({(s.task_id, s.model): sorted(x.rep for x in plan
                                                       if (x.task_id, x.model) == (s.task_id, s.model))
                          for s in plan}, {(t, m): [0, 1] for t in IDS for m in run.MODELS})
        self.assertEqual(fs.plan_problems(plan, self.env.pool), [])

    def test_screen_plan_order_is_deterministic_for_a_seed(self):
        a = fs.screen_plan(self.env.pool, run.MODELS, 2, 11)
        self.assertEqual(a, fs.screen_plan(self.env.pool, run.MODELS, 2, 11))
        self.assertEqual(run.plan_sha256(a), run.plan_sha256(fs.screen_plan(self.env.pool, run.MODELS, 2, 11)))
        self.assertNotEqual(a, fs.screen_plan(self.env.pool, run.MODELS, 2, 12))


class PoolTests(Case):
    def test_make_pool_takes_counts_and_hash_from_the_control(self):
        t = self.env.pool["tasks"][0]
        cj = self.env.data / "live" / "controls-batch" / t["id"] / "control.json"
        self.assertEqual(set(t), {"id", "repo", "control_sha256", "expected_count", "dropped_count"})
        self.assertEqual((t["expected_count"], t["dropped_count"]), (3, 1))  # f1 + p1 + p3, p2 dropped
        self.assertEqual(t["control_sha256"], sha(cj))
        self.assertEqual(t["repo"], "rust-lang/rust-analyzer")
        self.assertEqual(self.env.pool["gate"], "relaxed (spec 2026-11-01)")
        self.assertEqual([x["id"] for x in self.env.pool["tasks"]], IDS)

    def test_make_pool_refuses_a_control_that_is_not_relaxed_ok(self):
        bad = dict(GOOD_GOLD, **{"t::f1": "fail"})
        write_control(self.env.data / "live" / "controls-batch" / IDS[0], GOOD_BASE, bad, rid=IDS[0])
        with self.assertRaisesRegex(fs.Refused, "not relaxed ok"):
            fs.make_pool(self.env.records, self.env.data / "live" / "controls-batch")

    def test_sanity_mismatch_is_named(self):
        self.assertEqual(len(fs.sanity_mismatches(self.env.pool)), 9)  # the fixture's counts are 3 / 1
        ok = {"tasks": [{"id": IDS[0], "expected_count": 8120, "dropped_count": 0}]}
        self.assertEqual(fs.sanity_mismatches(ok), [])

    def test_the_committed_pool_file_is_the_registered_nine(self):
        p = Path(__file__).with_name(fs.POOL)
        fs.check_pool(json.loads(p.read_text()))


class FreezeTests(Case):
    def test_freeze_writes_a_registration_run_py_accepts(self):
        reg = self.env.freeze()
        plan = run.load_plan(self.env.plan_path)
        loaded = run.load_registration(self.env.out, plan)
        budget = run.check_budget(run.Budget(50.0, 0.0, 5.0, 5.0))
        run.check_registered_budget(loaded, plan, budget)
        run._frozen_inputs(loaded, self.env.manifest)
        self.assertEqual(loaded["registration_sha256"], reg["registration_sha256"])
        self.assertEqual(loaded["stage"], "screen")
        self.assertNotEqual(loaded["stage"], 2)  # run_batch's heavy re-check is for stage 3 / 2 only
        self.assertIsNone(run.plan_reps(plan))
        self.assertEqual(loaded["budget"], {"cap_usd": 50.0, "per_call_usd": 5.0, "reserve_usd": 5.0})
        self.assertEqual(loaded["frozen"], {"root": str(self.env.froot), "trees": self.env.trees})
        self.assertEqual(loaded["models"], list(run.MODELS))
        self.assertEqual(loaded["reps"], [["bare", 2]])
        self.assertEqual(loaded["pool_sha256"], sha(self.env.tool / fs.POOL))
        self.assertEqual(loaded["plan_sha256"], run.plan_sha256(plan))
        self.assertEqual(set(loaded["files"]), {"a.py", fs.POOL, fs.STAGE3_REGISTRATION})
        self.assertEqual(loaded["claude_cli_version"], CLI_VERSION)
        self.assertEqual(loaded["claude_cli_sha256"], self.env.manifest["cli"]["sha256"])
        t = loaded["tasks"][0]
        self.assertEqual((t["task_id"], t["kind"]), (IDS[0], "live"))
        self.assertEqual(t["arm_image"], {"tag": f"sab-arm-{IDS[0]}:t1",
                                          "id": image_id(f"sab-arm-{IDS[0]}:t1")})
        self.assertEqual(t["run_image"]["tag"], f"sab-run-{IDS[0]}:t1")
        self.assertEqual(t["bundle_sha256"], sha(self.env.data / "live-tasks" / IDS[0] / "bundle.json"))
        self.assertEqual(len(loaded["tasks"]), 9)
        self.assertEqual(self.env.check(), [])

    def test_freeze_writes_go_template_never_go_json(self):
        reg = self.env.freeze()
        self.assertFalse((self.env.run_data / "GO.json").exists())
        t = json.loads((self.env.run_data / fs.GO_TEMPLATE).read_text())
        go = t["GO.json"]
        self.assertEqual((go["cap_usd"], go["per_call_usd"], go["reserve_usd"], go["spent_usd"]),
                         (50.0, 5.0, 5.0, 0.0))
        self.assertEqual(go["plan_sha256"], reg["plan_sha256"])
        self.assertEqual(go["registration_sha256"], reg["registration_sha256"])
        self.assertIn("--registration", t["command"])
        # the template, copied to GO.json by the operator, passes the runner's own check
        (self.env.run_data / "GO.json").write_text(json.dumps(go))
        plan = run.load_plan(self.env.plan_path)
        run.check_go(self.env.run_data, plan, run.Budget(50.0, 0.0, 5.0, 5.0),
                     reg["registration_sha256"])

    def test_freeze_refuses_a_task_outside_the_pool(self):
        plan = list(self.env.plan) + [run.RunSpec("some__other-1", run.MODELS[0], "bare", 0)]
        fs.write_plan(plan, self.env.plan_path)
        self.refused("outside the pool")

    def test_freeze_refuses_a_non_bare_arm(self):
        plan = list(self.env.plan)
        plan[0] = plan[0]._replace(arm="cs")
        fs.write_plan(plan, self.env.plan_path)
        self.refused("non-bare")
        plan = list(self.env.plan) + [run.RunSpec(IDS[0], run.MODELS[0], "full", 0)]
        fs.write_plan(plan, self.env.plan_path)
        self.refused("non-bare")

    def test_freeze_refuses_a_plan_that_is_not_the_whole_screen(self):
        fs.write_plan(self.env.plan[:-1], self.env.plan_path)
        self.refused("not the screen's 36 runs")

    def test_freeze_refuses_a_bundle_whose_control_is_not_relaxed_ok(self):
        # control.json (the pooled hash) is unchanged; the logs relaxed_expected reads are not
        cdir = self.env.data / "live" / "controls-batch" / IDS[1]
        write_control(cdir, GOOD_BASE, dict(GOOD_GOLD, **{"t::f1": "fail"}), rid=IDS[1])
        self.refused("not relaxed ok")

    def test_freeze_refuses_a_control_that_is_not_the_pooled_one(self):
        cdir = self.env.data / "live" / "controls-batch" / IDS[2]
        write_control(cdir, GOOD_BASE, GOOD_GOLD, rid=IDS[2], mismatch={"count": 0, "names": ["x"]})
        self.refused("not the pooled one")

    def test_freeze_refuses_a_live_bundle_with_a_template_or_a_wrong_kind(self):
        b = self.env.data / "live-tasks" / IDS[3] / "bundle.json"
        rec = json.loads(b.read_text())
        b.write_text(json.dumps(dict(rec, template="sab-tpl-x")))
        self.refused("no template")
        b.write_text(json.dumps(dict(rec, kind="foreign")))
        self.refused("bundle kind")

    def test_freeze_refuses_a_bundle_without_the_exec_only_arm_image(self):
        class Bad(Inspect):
            def __call__(s, image):
                i, _ = super().__call__(image)
                return (i, "sha256:other") if image.startswith("sab-arm-") else (i, None)
        self.refused("exec-only", inspect=Bad())

    def test_freeze_refuses_a_dirty_tool_dir_and_an_untracked_pool(self):
        (self.env.tool / "a.py").write_text("x = 2\n")
        self.refused("uncommitted changes")
        git(self.env.tool, "checkout", "a.py")
        (self.env.tool / "new.py").write_text("")
        self.refused("untracked")

    def test_freeze_refuses_changed_frozen_inputs_and_an_existing_registration(self):
        (self.env.froot / "config" / "CLAUDE.md").write_text("edited\n")
        self.refused("frozen inputs")
        (self.env.froot / "config" / "CLAUDE.md").write_text("claude\n")
        self.env.freeze()
        with self.assertRaisesRegex(fs.Refused, "--replace"):
            self.env.freeze()
        self.env.freeze(replace=True)

    def test_freeze_refuses_after_a_go_json_exists(self):
        (self.env.run_data / "GO.json").write_text("{}")
        with self.assertRaisesRegex(fs.Refused, "GO.json exists"):
            self.env.freeze()

    def test_freeze_refuses_when_the_runner_path_is_missing(self):
        os.unlink(self.env.runner / IDS[2])
        with self.assertRaises(fs.Refused) as cm:
            self.env.freeze()
        self.assertIn(IDS[2], str(cm.exception))
        self.assertIn(str(self.env.runner / IDS[2] / "bundle.json"), str(cm.exception))
        self.assertNothingWritten()

    def test_freeze_refuses_when_the_runner_path_is_another_bundle(self):
        os.unlink(self.env.runner / IDS[2])
        shutil.copytree(self.env.data / "live-tasks" / IDS[2], self.env.runner / IDS[2])  # a copy
        with self.assertRaises(fs.Refused) as cm:
            self.env.freeze()
        self.assertIn(IDS[2], str(cm.exception))
        self.assertIn("not the frozen bundle", str(cm.exception))
        self.assertNothingWritten()
        os.unlink(self.env.runner / IDS[2] / "bundle.json")  # still wrong: points at nothing
        shutil.rmtree(self.env.runner / IDS[2])
        os.symlink(self.env.data / "live-tasks" / IDS[3], self.env.runner / IDS[2])
        self.refused("not the frozen bundle")

    def test_freeze_passes_when_the_runner_path_is_a_symlink_to_the_frozen_bundle(self):
        self.assertTrue((self.env.runner / IDS[0]).is_symlink())
        reg = self.env.freeze()
        self.assertEqual(reg["runner_tasks_root"], str(self.env.runner.resolve()))
        self.assertEqual(reg["tasks"][0]["bundle_sha256"], sha(self.env.runner / IDS[0] / "bundle.json"))

    def test_freeze_refuses_a_plan_in_another_order_than_the_seed_gives(self):
        plan = list(reversed(self.env.plan))
        fs.write_plan(plan, self.env.plan_path)
        self.refused("not screen_plan")

    def test_the_budget_is_pinned_and_admits_36_runs_at_one_dollar_not_at_1_2(self):
        self.assertEqual(fs.BUDGET, {"cap_usd": 50.0, "per_call_usd": 5.0, "reserve_usd": 5.0})
        b = run.check_budget(run.Budget(50.0, 0.0, 5.0, 5.0))

        def started(cost):
            spent, n = 0.0, 0
            for _ in range(36):
                if not run.admit(b, spent):
                    break
                spent, n = spent + cost, n + 1
            return n
        self.assertEqual(started(1.0), 36)
        self.assertEqual(started(1.2), 34)
        self.assertEqual(started(0.0), 36)


    def test_freeze_refuses_a_cli_that_reports_another_version(self):
        self.refused("reports", run_cli=lambda p: "9.9.9\n")


class CheckTests(Case):
    def setUp(self):
        super().setUp()
        self.env.freeze()

    def test_check_detects_a_changed_tool_file_and_a_changed_bundle(self):
        self.assertEqual(self.env.check(), [])
        (self.env.tool / "a.py").write_text("x = 3\n")
        why = self.env.check()
        self.assertTrue(any("a.py changed" in w for w in why), why)
        (self.env.tool / "a.py").write_text("x = 1\n")
        self.assertEqual(self.env.check(), [])
        b = self.env.data / "live-tasks" / IDS[4] / "bundle.json"
        b.write_text(b.read_text() + " ")
        why = self.env.check()
        self.assertTrue(any(IDS[4] in w and "bundle" in w for w in why), why)

    def test_check_detects_a_repointed_runner_symlink(self):
        link = self.env.runner / IDS[1]
        os.unlink(link)
        os.symlink(self.env.data / "live-tasks" / IDS[0], link)
        why = self.env.check()
        self.assertTrue(any(IDS[1] in w and "not the frozen bundle" in w for w in why), why)
        os.unlink(link)
        why = self.env.check()
        self.assertTrue(any(IDS[1] in w and "does not exist" in w for w in why), why)

    def test_check_detects_a_changed_prompt(self):
        p = self.env.data / "live-tasks" / IDS[2] / "build" / "prompt.txt"
        p.write_text("another prompt\n")
        why = self.env.check()
        self.assertTrue(any(IDS[2] in w and "changed after the freeze" in w for w in why), why)

    def test_check_detects_a_changed_record(self):
        p = self.env.data / "live-tasks" / IDS[3] / "build" / "record.json"
        rec = json.loads(p.read_text())
        rec["rebuild_cmds"] = ["something else"]
        p.write_text(json.dumps(rec))
        why = self.env.check()
        self.assertTrue(any(IDS[3] in w and "changed after the freeze" in w for w in why), why)

    def test_check_flags_other_data_dirs_runner_root_and_plan_order(self):
        why = fs.check_screen(tool_dir=self.env.tool, data=self.env.tmp, run_data=self.env.run_data,
                              registration=self.env.out, inspect=self.env.inspect,
                              runner_tasks_root=self.env.runner, run_cli=lambda p: CLI_VERSION)
        self.assertTrue(any("data " in w and "registered" in w for w in why), why)
        other = self.env.tmp / "other-runner"
        other.mkdir()
        self.assertTrue(any("runner_tasks_root" in w for w in self.env.check(runner_tasks_root=other)))
        fs.write_plan(list(reversed(self.env.plan)), self.env.plan_path)
        why = self.env.check()
        self.assertTrue(any("registered seed" in w for w in why), why)


    def test_check_detects_a_retagged_image_and_changed_frozen_tree(self):
        self.env.inspect.rebuilt.add(f"sab-arm-{IDS[0]}:t1")
        why = self.env.check()
        self.assertTrue(any(IDS[0] in w for w in why), why)
        self.env.inspect.rebuilt.clear()
        (self.env.froot / "plugins" / "codescout-companion" / "x.md").write_text("edited")
        self.assertTrue(any("frozen" in w for w in self.env.check()))

    def test_check_detects_a_registration_edit_and_a_changed_pool(self):
        reg = json.loads(self.env.out.read_text())
        reg["seed"] = 1
        self.env.out.write_text(json.dumps(reg))
        self.assertTrue(any("registration_sha256" in w for w in self.env.check()))

    def test_check_refuses_a_plan_edited_into_a_non_bare_or_outside_plan(self):
        plan = list(self.env.plan)
        plan[0] = plan[0]._replace(arm="cs")
        fs.write_plan(plan, self.env.plan_path)
        why = self.env.check()
        self.assertTrue(any("non-bare" in w for w in why), why)
        self.assertTrue(any("plan file changed" in w for w in why), why)
        plan = list(self.env.plan) + [run.RunSpec("some__other-1", run.MODELS[0], "bare", 0)]
        fs.write_plan(plan, self.env.plan_path)
        self.assertTrue(any("outside the pool" in w for w in self.env.check()))

    def test_check_refuses_a_control_that_became_not_relaxed_ok(self):
        cdir = self.env.data / "live" / "controls-batch" / IDS[5]
        write_control(cdir, GOOD_BASE, dict(GOOD_GOLD, **{"t::f1": "fail"}), rid=IDS[5])
        self.assertTrue(any(IDS[5] in w and "not relaxed ok" in w for w in self.env.check()))

    def test_check_detects_a_changed_control_log_that_is_still_relaxed_ok(self):
        cdir = self.env.data / "live" / "controls-batch" / IDS[6]
        write_control(cdir, GOOD_BASE, dict(GOOD_GOLD, **{"t::p2": "pass"}), rid=IDS[6])
        self.assertTrue(any(IDS[6] in w for w in self.env.check()))

    def test_check_detects_a_control_log_change_that_leaves_the_expected_set_alone(self):
        cdir = self.env.data / "live" / "controls-batch" / IDS[8]
        write_control(cdir, GOOD_BASE, dict(GOOD_GOLD, **{"t::unrelated": "pass"}), rid=IDS[8])
        why = self.env.check()
        self.assertTrue(any(IDS[8] in w and "changed after the freeze" in w for w in why), why)


    def test_check_detects_a_control_json_that_is_not_the_pooled_one(self):
        cdir = self.env.data / "live" / "controls-batch" / IDS[7]
        write_control(cdir, GOOD_BASE, GOOD_GOLD, rid=IDS[7], mismatch={"count": 0, "names": ["x"]})
        self.assertTrue(any(IDS[7] in w and "pooled one" in w for w in self.env.check()))

    def test_check_flags_a_go_json_for_another_registration(self):
        (self.env.run_data / "GO.json").write_text(json.dumps({"registration_sha256": "x"}))
        self.assertTrue(any("another registration" in w for w in self.env.check()))


if __name__ == "__main__":
    unittest.main()
