"""Tests for freeze_stage1: the registration that Stage 1 spending is bound to.

A temp git repo stands in for the checkout, a fake docker (`inspect`) and a fake `claude` stand in
for the host; no model, token, container or real data dir is touched."""
import contextlib
import io
import json
import subprocess
import tempfile
import unittest
from pathlib import Path

import freeze_stage1
import run
import tasks

DEV = run.STAGE1_DEV_TASKS
SELECTED = ["cs-p08", "tokio-rs__axum-691"]
CLI_VERSION = "2.1.309 (Claude Code)"
GIT_FULL = "f9b0fc9f867a0b6f5b6bb0d0b436fb303e1c5248"
MANIFEST = {
    "cli": {"path": "/x/claude", "version": CLI_VERSION, "sha256": "ab"},
    "codescout": {"binary_sha256": "host-bin",
                  "version_json": {"version": "0.15.0", "git_sha": "f9b0fc9", "git_sha_full": GIT_FULL}},
}
LIVE_CONTROL = {"image": {"codescout": {
    "sha": GIT_FULL, "binary_sha256": "arm-bin",
    "version_json": {"version": "0.15.0", "git_sha": "f9b0fc9", "git_sha_full": GIT_FULL}}}}


def git(repo, *args):
    return subprocess.run(["git", "-C", str(repo), *args], check=True, capture_output=True,
                          text=True).stdout


def image_id(tag):
    return "sha256:" + tag.replace(":", "-")


class Fake:
    """inspect(image) -> (id, exec-only label). Arm images carry their run image's id."""

    def __init__(self):
        self.missing = set()
        self.bad_label = set()

    def __call__(self, image):
        if image in self.missing:
            return None, None
        if image.startswith("sab-arm-"):
            run_tag = image.replace("sab-arm-", "sab-run-")
            return image_id(image), ("sha256:wrong" if image in self.bad_label else image_id(run_tag))
        return image_id(image), None


class Env:
    def __init__(self, tmp, selected=SELECTED):
        self.tmp = Path(tmp)
        self.repo = self.tmp / "repo"
        self.tool = self.repo / "tools" / "setup-ab-eval"
        self.tool.mkdir(parents=True)
        (self.tool / "a.py").write_text("a = 1\n")
        (self.tool / "b.py").write_text("b = 2\n")
        (self.tool / "selection-rule.json").write_text('{"rule": 1}\n')
        (self.tool / "task4-live-control.json").write_text(json.dumps(LIVE_CONTROL))
        git(self.repo, "init", "-q")
        git(self.repo, "config", "user.email", "t@t")
        git(self.repo, "config", "user.name", "t")
        git(self.repo, "add", "-A")
        git(self.repo, "commit", "-q", "-m", "init")
        self.data = self.tmp / "data"
        for t in DEV:
            d = self.data / "tasks" / t
            d.mkdir(parents=True)
            (d / "bundle.json").write_text(json.dumps({
                "id": t, "kind": "foreign", "run_image": f"sab-run-{t}:f9b0",
                "arm_image": f"sab-arm-{t}:f9b0", "template": f"sab-tpl-{t}"}))
        (self.data / "selection").mkdir()
        (self.data / "selection" / "selection.json").write_text(json.dumps({"selected": list(selected)}))
        self.manifest = self.tmp / "setup-manifest.json"
        self.manifest.write_text(json.dumps(MANIFEST))
        self.run_data = self.tmp / "run-data"   # where run_batch loads setup-manifest.json from
        self.run_data.mkdir()
        (self.run_data / "setup-manifest.json").write_text(json.dumps(MANIFEST))
        self.inspect = Fake()
        self.verify_calls = []

    def verify(self, ids, root, *, selected=(), inspect=None):
        self.verify_calls.append((list(ids), list(selected)))
        return {i: [] for i in ids}

    def freeze(self, seed=20261027, **kw):
        args = dict(tool_dir=self.tool, data=self.data, manifest_path=self.manifest, run_data=self.run_data,
                    seed=seed,
                    inspect=self.inspect, run_cli=lambda path: CLI_VERSION + "\n",
                    verify_fn=self.verify)
        args.update(kw)
        return freeze_stage1.freeze_stage1(**args)

    def commit(self, msg="change"):
        git(self.repo, "add", "-A")
        git(self.repo, "commit", "-q", "-m", msg)


class FreezeTest(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.env = Env(self._tmp.name)

    def test_the_registration_records_files_hashes_images_order_budget_and_versions(self):
        reg = self.env.freeze()
        self.assertEqual(sorted(reg["files"]), ["a.py", "b.py", "selection-rule.json", "task4-live-control.json"])
        self.assertEqual(reg["manifest_sha256"], freeze_stage1.sha256_file(self.env.manifest))
        self.assertEqual(reg["selection_rule_sha256"],
                         freeze_stage1.sha256_file(self.env.tool / "selection-rule.json"))
        self.assertEqual([t["task_id"] for t in reg["tasks"]], list(DEV))
        t0 = reg["tasks"][0]
        self.assertEqual(t0["kind"], "foreign")
        self.assertEqual(t0["run_image"], {"tag": f"sab-run-{DEV[0]}:f9b0",
                                           "id": image_id(f"sab-run-{DEV[0]}:f9b0")})
        self.assertEqual(t0["arm_image"], {"tag": f"sab-arm-{DEV[0]}:f9b0",
                                           "id": image_id(f"sab-arm-{DEV[0]}:f9b0")})
        self.assertEqual(t0["template"], f"sab-tpl-{DEV[0]}")
        self.assertEqual(reg["budget"], {"cap_usd": 25.0, "per_call_usd": 2.0, "reserve_usd": 2.0})
        self.assertEqual(reg["claude_cli_version"], CLI_VERSION)
        self.assertEqual(reg["codescout"]["host_binary"]["version_json"]["version"], "0.15.0")
        self.assertEqual(reg["head"], git(self.env.repo, "rev-parse", "HEAD").strip())
        self.assertTrue(reg["python_version"])
        self.assertEqual(reg["seed"], 20261027)
        on_disk = json.loads((self.env.tool / "stage1-registration.json").read_text())
        self.assertEqual(on_disk, reg)

    def test_stage1_is_one_run_per_arm_for_two_tasks_and_two_models(self):
        reg = self.env.freeze()
        plan = [run.RunSpec(*s) for s in reg["order"]]
        self.assertEqual(len(plan), 12)
        for task in DEV:
            for model in run.MODELS:
                arms_ = sorted((s.arm, s.rep) for s in plan if s.task_id == task and s.model == model)
                self.assertEqual(arms_, [("bare", 0), ("cs", 0), ("full", 0)])
        self.assertEqual(plan, run.stage1_order(20261027))

    def test_the_plan_hash_is_what_the_go_guard_recomputes(self):
        reg = self.env.freeze()
        plan = [run.RunSpec(*s) for s in json.loads(json.dumps(reg["order"]))]
        self.assertEqual(reg["plan_sha256"], run.plan_sha256(plan))
        go = Path(self._tmp.name) / "go-data"
        go.mkdir()
        (go / run.GO_FILE).write_text(json.dumps({"authorized_by": "operator",
                                                  "plan_sha256": reg["plan_sha256"], **reg["budget"]}))
        budget = run.Budget(reg["budget"]["cap_usd"], 0.0, reg["budget"]["per_call_usd"],
                            reg["budget"]["reserve_usd"])
        run.check_go(go, plan, run.check_budget(budget))  # raises Refused when any binding differs

    def test_changing_one_file_changes_the_registration_hash_through_files_alone(self):
        fixed = lambda tool_dir: "f" * 40   # the same head both times: only `files` can differ
        first = self.env.freeze(head_fn=fixed)
        (self.env.tool / "a.py").write_text("a = 2\n")
        self.env.commit()
        second = self.env.freeze(head_fn=fixed)
        self.assertEqual(first["head"], second["head"])
        self.assertEqual({k: v for k, v in first.items() if k not in ("files", "registration_sha256")},
                         {k: v for k, v in second.items() if k not in ("files", "registration_sha256")})
        self.assertNotEqual(first["files"]["a.py"], second["files"]["a.py"])
        self.assertEqual(first["files"]["b.py"], second["files"]["b.py"])
        self.assertNotEqual(first["registration_sha256"], second["registration_sha256"])
        body = {k: v for k, v in second.items() if k != "registration_sha256"}
        self.assertEqual(second["registration_sha256"], freeze_stage1._digest(body))

    def test_the_registration_file_is_not_hashed_and_its_own_change_is_not_dirt(self):
        first = self.env.freeze()
        self.env.commit("registration")           # now tracked
        (self.env.tool / "stage1-registration.json").write_text("{}\n")  # modified, uncommitted
        second = self.env.freeze()
        self.assertNotIn("stage1-registration.json", second["files"])
        self.assertEqual(first["files"], second["files"])

    def test_the_same_inputs_give_the_same_registration_hash(self):
        a = self.env.freeze()
        b = self.env.freeze()
        self.assertEqual(a["registration_sha256"], b["registration_sha256"])

    def test_a_different_seed_changes_the_plan_and_its_hash(self):
        a = self.env.freeze(seed=1)
        b = self.env.freeze(seed=2)
        self.assertNotEqual(a["order"], b["order"])
        self.assertNotEqual(a["plan_sha256"], b["plan_sha256"])
        self.assertEqual(sorted(a["order"]), sorted(b["order"]))  # same runs, other order

    def test_the_order_holds_only_the_two_development_tasks(self):
        reg = self.env.freeze()
        self.assertEqual({s[0] for s in reg["order"]}, set(DEV))
        self.assertTrue(set(DEV) <= set(run.DEVELOPMENT_TASKS))

    def test_the_host_and_the_arm_image_binaries_are_recorded_apart(self):
        cs = self.env.freeze()["codescout"]
        self.assertEqual(cs["host_binary"]["sha256"], "host-bin")
        self.assertEqual(cs["arm_image_binary"]["sha256"], "arm-bin")
        self.assertEqual(cs["arm_image_binary"]["version_json"]["git_sha_full"], GIT_FULL)
        self.assertEqual(cs["arm_image_binary"]["source"], "tools/setup-ab-eval/task4-live-control.json")
        self.assertEqual(cs["arm_image_binary"]["source_sha256"],
                         freeze_stage1.sha256_file(self.env.tool / "task4-live-control.json"))

    def test_the_manifest_hash_and_path_are_the_runners_copy(self):
        reg = self.env.freeze()
        copy = self.env.run_data / "setup-manifest.json"
        self.assertEqual(reg["manifest_path"], str(copy))
        self.assertEqual(reg["manifest_sha256"], freeze_stage1.sha256_file(copy))


class RefusalTest(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.env = Env(self._tmp.name)
        self.out = self.env.tool / "stage1-registration.json"

    def refuses(self, fragment, **kw):
        with self.assertRaises(freeze_stage1.Refused) as cm:
            self.env.freeze(**kw)
        self._why = str(cm.exception)
        self.assertIn(fragment, self._why)
        self.assertFalse(self.out.exists(), "a refused freeze must not write the registration")

    def test_a_modified_tracked_file_refuses(self):
        (self.env.tool / "a.py").write_text("a = 99\n")
        self.refuses("uncommitted")

    def test_a_staged_uncommitted_file_refuses(self):
        (self.env.tool / "a.py").write_text("a = 99\n")
        git(self.env.repo, "add", "-A")
        self.refuses("uncommitted")

    def test_a_dev_task_in_the_selected_list_refuses(self):
        env = Env(self._tmp.name + "/x", selected=SELECTED + [DEV[1]])
        with self.assertRaises(freeze_stage1.Refused) as cm:
            env.freeze()
        self.assertIn(DEV[1], str(cm.exception))
        self.assertIn("selected", str(cm.exception))
        self.assertFalse((env.tool / "stage1-registration.json").exists())

    def test_verify_reasons_refuse_and_name_the_task(self):
        def bad(ids, root, *, selected=(), inspect=None):
            return {i: (["grader: gold does not pass"] if i == DEV[1] else []) for i in ids}
        self.refuses(DEV[1], verify_fn=bad)

    def test_verify_runs_over_the_stage1_ids_and_the_selected_ids(self):
        self.env.freeze()
        (ids, selected), = self.env.verify_calls
        self.assertEqual(ids, list(DEV) + SELECTED)
        self.assertEqual(selected, SELECTED)

    def test_the_real_verify_refuses_a_task_without_controls(self):
        self.refuses("no controls.json", verify_fn=tasks.verify)

    def test_a_missing_arm_image_refuses(self):
        self.env.inspect.missing.add(f"sab-arm-{DEV[1]}:f9b0")
        self.refuses("arm image")

    def test_an_arm_image_that_is_not_the_exec_only_layer_refuses(self):
        self.env.inspect.bad_label.add(f"sab-arm-{DEV[0]}:f9b0")
        self.refuses("exec-only")

    def test_a_bundle_without_an_arm_image_refuses(self):
        p = self.env.data / "tasks" / DEV[0] / "bundle.json"
        rec = json.loads(p.read_text())
        del rec["arm_image"]
        p.write_text(json.dumps(rec))
        self.refuses("arm image")

    def test_a_bundle_without_a_template_refuses(self):
        p = self.env.data / "tasks" / DEV[1] / "bundle.json"
        rec = json.loads(p.read_text())
        rec["template"] = ""
        p.write_text(json.dumps(rec))
        self.refuses("template")

    def test_a_cli_that_differs_from_the_manifest_refuses(self):
        self.refuses("CLI", run_cli=lambda path: "2.1.999 (Claude Code)\n")

    def main(self):
        err = io.StringIO()
        with contextlib.redirect_stderr(err):
            rc = freeze_stage1.main(["--tool-dir", str(self.env.tool), "--data", str(self.env.data),
                                     "--manifest", str(self.env.manifest),
                                     "--run-data", str(self.env.run_data), "--seed", "1"])
        return rc, err.getvalue()

    def test_a_refusal_has_a_nonzero_exit_and_a_named_reason(self):
        (self.env.tool / "a.py").write_text("a = 99\n")
        rc, err = self.main()
        self.assertEqual(rc, 1)
        self.assertIn("REFUSED:", err)
        self.assertIn("uncommitted changes in ['a.py']", err)

    def test_an_unreadable_manifest_is_a_refusal_not_a_traceback(self):
        self.env.manifest.write_text("{not json")
        rc, err = self.main()
        self.assertEqual(rc, 1)
        self.assertIn("REFUSED:", err)

    def test_an_untracked_file_in_the_tool_dir_refuses(self):
        (self.env.tool / "json.py").write_text("# shadows the stdlib\n")
        self.refuses("untracked", )
        self.assertIn("json.py", self._why)

    def test_the_registration_itself_untracked_is_not_a_refusal(self):
        self.env.freeze()                      # leaves an untracked stage1-registration.json
        self.env.freeze()

    def test_a_skip_worktree_file_cannot_hide_an_edit(self):
        git(self.env.repo, "update-index", "--skip-worktree", "tools/setup-ab-eval/a.py")
        (self.env.tool / "a.py").write_text("a = 99\n")
        self.refuses("a.py")

    def test_a_runner_manifest_copy_that_differs_refuses(self):
        (self.env.run_data / "setup-manifest.json").write_text(json.dumps(dict(MANIFEST, extra=1)))
        self.refuses("setup-manifest.json")

    def test_a_missing_runner_manifest_copy_refuses(self):
        (self.env.run_data / "setup-manifest.json").unlink()
        self.refuses("setup-manifest.json")

    def test_arm_image_and_host_binaries_from_different_commits_refuse(self):
        other = json.loads(json.dumps(LIVE_CONTROL))
        other["image"]["codescout"]["version_json"]["git_sha_full"] = "0" * 40
        other["image"]["codescout"]["sha"] = "0" * 40
        (self.env.tool / "task4-live-control.json").write_text(json.dumps(other))
        self.env.commit()
        self.refuses("git sha")


class NoSpendTest(unittest.TestCase):
    def test_it_writes_no_go_file_and_only_the_registration(self):
        with tempfile.TemporaryDirectory() as tmp:
            env = Env(tmp)
            before = sorted(p.relative_to(env.data) for p in env.data.rglob("*"))
            env.freeze()
            self.assertEqual(sorted(p.relative_to(env.data) for p in env.data.rglob("*")), before)
            self.assertFalse(list(Path(tmp).rglob(run.GO_FILE)))
            self.assertTrue((env.tool / "stage1-registration.json").is_file())


if __name__ == "__main__":
    unittest.main()
