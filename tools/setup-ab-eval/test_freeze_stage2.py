"""Tests for freeze_stage2: the Stage 3 registration. A temp git repo stands in for the checkout;
fake docker, inspect, verify and `claude` stand in for the host. No model, token, container or
real data dir is touched."""
import contextlib
import hashlib
import io
import json
import subprocess
import tempfile
import unittest
from pathlib import Path

import analyze
import analyze_study
import freeze_stage1
import freeze_stage2
import frozen
import run
import stage3

PRIVATE = [f"cs-p{i:02d}" for i in (1, 4, 7, 8, 15, 19)]
FOREIGN = ["tokio-rs__axum-691", "uutils__coreutils-6575", "nushell__nushell-13831",
           "astral-sh__ruff-15626", "sharkdp__bat-1892", "tokio-rs__tokio-6603"]
SELECTED = [t for pair in zip(PRIVATE, FOREIGN) for t in pair]
CLI_VERSION = "2.1.309 (Claude Code)"
GIT_FULL = "f9b0fc9f867a0b6f5b6bb0d0b436fb303e1c5248"
ARM_SHA = "06c8167b707913b08bc8403e04a4cbb9f80c83c0f63a9d76eeef3eda15f3c3c7"
LIVE_CONTROL = {"image": {"codescout": {
    "sha": GIT_FULL, "binary_sha256": ARM_SHA,
    "version_json": {"version": "0.15.0", "git_sha": "f9b0fc9", "git_sha_full": GIT_FULL}}}}


def git(repo, *args):
    return subprocess.run(["git", "-C", str(repo), *args], check=True, capture_output=True,
                          text=True).stdout


def image_id(tag):
    return "sha256:" + hashlib.sha256(tag.encode()).hexdigest()


class Inspect:
    """inspect(image) -> (id, exec-only label); arm images carry their run image's id."""

    def __init__(self):
        self.missing = set()

    def __call__(self, image):
        if image in self.missing:
            return None, None
        if image.startswith("sab-arm-"):
            return image_id(image), image_id(image.replace("sab-arm-", "sab-run-"))
        return image_id(image), None


class Docker:
    """The two read-only `docker run --rm` checks of stage3."""

    def __init__(self):
        self.calls = []
        self.binary = {}          # image -> sha override
        self.empty_template = set()

    def __call__(self, *args, check=True, timeout=None):
        a = [str(x) for x in args]
        self.calls.append(a)
        assert a[:2] == ["run", "--rm"] and a[a.index("--network") + 1] == "none", a
        if a[-1] == "/opt/sab/sha256.txt":
            image = a[-2]
            out = f"{self.binary.get(image, ARM_SHA)}  /out/bin/codescout\nab  /out/bin/ra\n"
        else:
            tpl = next(x for x in a if x.endswith(":/tpl:ro")).split(":")[0]
            out = "" if tpl in self.empty_template else f"aa  ./{tpl}/segment\n"
        from types import SimpleNamespace
        return SimpleNamespace(returncode=0, stdout=out)


class Env:
    def __init__(self, tmp):
        self.tmp = Path(tmp)
        self.repo = self.tmp / "repo"
        self.tool = self.repo / "tools" / "setup-ab-eval"
        self.tool.mkdir(parents=True)
        (self.tool / "a.py").write_text("a = 1\n")
        (self.tool / "selection-rule.json").write_text('{"rule": 1}\n')
        (self.tool / "task4-live-control.json").write_text(json.dumps(LIVE_CONTROL))
        (self.tool / "stage1-registration.json").write_text('{"stage": 1}\n')
        git(self.repo, "init", "-q")
        git(self.repo, "config", "user.email", "t@t")
        git(self.repo, "config", "user.name", "t")
        self.commit("init")
        self.data = self.tmp / "data"
        for t in SELECTED:
            self.task(t, "private" if t.startswith("cs-") else "foreign")
        (self.data / "selection").mkdir()
        self.select(SELECTED)
        cli = self.tmp / "claude"
        cli.write_bytes(b"#!fake cli\n")
        profile = self.tmp / "profile"
        profile.mkdir()
        (profile / "CLAUDE.md").write_text("# rules\n")
        (profile / "settings.json").write_text(json.dumps({"permissions": {"deny": ["Bash"]}}))
        plugins = {}
        for name in ("codescout-companion", "superpowers"):
            d = self.tmp / "cache" / name
            d.mkdir(parents=True)
            (d / "README.md").write_text(name)
            plugins[name] = d
        self.manifest_body = {
            "cli": {"path": str(cli), "version": CLI_VERSION,
                    "sha256": hashlib.sha256(cli.read_bytes()).hexdigest()},
            "codescout": {"binary_sha256": "host-bin",
                          "version_json": {"version": "0.15.0", "git_sha": "f9b0fc9",
                                           "git_sha_full": GIT_FULL}},
            "plugins_cs": {"install_path": str(plugins["codescout-companion"])},
            "plugins_full": [{"name": "superpowers", "install_path": str(plugins["superpowers"])}],
            "full_config_files": [str(profile / "CLAUDE.md"), str(profile / "settings.json")]}
        self.manifest = self.tmp / "setup-manifest.json"
        self.manifest.write_text(json.dumps(self.manifest_body))
        self.run_data = self.tmp / "study"
        self.run_data.mkdir()
        (self.run_data / "setup-manifest.json").write_text(json.dumps(self.manifest_body))
        self.frozen = self.tmp / "study" / "frozen"
        frozen.freeze_inputs(self.manifest_body, self.frozen)
        self.inspect = Inspect()
        self.docker = Docker()
        self.verify_calls = []
        self.verify_reasons = {}

    def task(self, t, kind):
        d = self.data / "tasks" / t
        (d / "controls").mkdir(parents=True)
        out = d / "build"
        if kind == "private":
            (out / "s5" / "tasks" / t).mkdir(parents=True)
            (out / "s5" / "tasks" / t / "prompt.txt").write_text(f"fix {t}\n")
            (d / "prescan").mkdir()
            (d / "prescan" / "prescan.json").write_text(json.dumps(
                {"id": t, "control": "pass", "hits": [], "reasons": []}))
        else:
            out.mkdir()
            (out / "prompt.txt").write_text(f"fix {t}\n")
        (d / "controls" / "controls.json").write_text(json.dumps({"id": t}))
        (d / "controls" / "qdrant-dump.jsonl").write_text(f"{t}\n")
        (d / "bundle.json").write_text(json.dumps({
            "id": t, "kind": kind, "run_image": f"sab-run-{t}:f9b0",
            "arm_image": f"sab-arm-{t}:f9b0", "image": f"sab-arm-{t}:f9b0",
            "template": f"sab-tpl-{t}", "out": str(out)}))

    def select(self, ids):
        (self.data / "selection" / "selection.json").write_text(json.dumps({"selected": ids}))

    def commit(self, msg="change"):
        git(self.repo, "add", "-A")
        git(self.repo, "commit", "-q", "-m", msg)

    def verify(self, ids, root, *, selected=(), inspect=None):
        self.verify_calls.append((list(ids), list(selected)))
        return {i: list(self.verify_reasons.get(i, [])) for i in ids}

    def freeze(self, **kw):
        args = dict(tool_dir=self.tool, data=self.data, manifest_path=self.manifest,
                    run_data=self.run_data, bootstrap_seed=20261033, head_fn=lambda d: "f" * 40,
                    inspect=self.inspect, run_cli=lambda path: CLI_VERSION + "\n",
                    verify_fn=self.verify, docker=self.docker,
                    python_version=lambda: "3.14.8", frozen_root=self.frozen)
        args.update(kw)
        with contextlib.redirect_stdout(io.StringIO()):
            return freeze_stage2.freeze_stage2(**args)


class FreezeStage2Test(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.env = Env(self._tmp.name)

    def refused(self, pattern, **kw):
        with self.assertRaisesRegex(freeze_stage2.Refused, pattern):
            self.env.freeze(**kw)
        self.assertFalse((self.env.tool / freeze_stage2.REGISTRATION).exists())
        for part in freeze_stage2.PARTS:  # every check runs before anything is written
            self.assertFalse(freeze_stage2.plan_file(self.env.run_data, part).exists())

    # -- what it records ------------------------------------------------------------------------

    def test_the_registration_is_what_the_runner_and_the_analysis_read(self):
        reg = self.env.freeze()
        path = self.env.tool / freeze_stage2.REGISTRATION
        on_disk = json.loads(path.read_text())
        self.assertEqual(dict(reg, go_templates=None), dict(on_disk, go_templates=None))
        self.assertNotIn("go_templates", on_disk)
        self.assertNotIn("plan_sha256", on_disk)  # no Stage 1 top-level plan form
        manifest = json.loads((self.env.run_data / "setup-manifest.json").read_text())
        for part in freeze_stage2.PARTS:
            plan = run.load_plan(freeze_stage2.plan_file(self.env.run_data, part))
            self.assertEqual(run.plan_sha256(plan), on_disk["plans"][part]["plan_sha256"])
            loaded = run.load_registration(path, plan)
            run.check_registered_budget(loaded, plan, stage3.stage3_budget())
            # The printed GO.json template passes check_go as is.
            go_dir = self.env.tmp / f"go-{part}"
            go_dir.mkdir()
            (go_dir / run.GO_FILE).write_text(json.dumps(reg["go_templates"][part]["GO.json"]))
            run.check_go(go_dir, plan, stage3.stage3_budget(), loaded["registration_sha256"])
        m = run._frozen_inputs(on_disk, manifest)
        self.assertEqual(m["frozen_root"], str(self.env.frozen.resolve()))
        # analyze_study --registration accepts it and takes its seed.
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            analyze_study.main(["--data", str(self.env.run_data), "--tasks-root",
                                str(self.env.data / "tasks"), "--registration", str(path)],
                               verify=self.env.verify)
        self.assertEqual(json.loads(buf.getvalue())["seed"], 20261033)
        self.assertFalse((self.env.run_data / run.GO_FILE).exists())  # never writes GO.json

    def test_files_versions_and_stage1_evidence(self):
        reg = self.env.freeze()
        self.assertEqual(sorted(reg["files"]), ["a.py", "selection-rule.json",
                                                "stage1-registration.json",
                                                "task4-live-control.json"])
        self.assertEqual(reg["stage1_registration_sha256"],
                         freeze_stage1.sha256_file(self.env.tool / "stage1-registration.json"))
        self.assertEqual(reg["manifest_sha256"],
                         freeze_stage1.sha256_file(self.env.run_data / "setup-manifest.json"))
        self.assertEqual((reg["python_version"], reg["claude_cli_version"]),
                         ("3.14.8", CLI_VERSION))
        self.assertEqual(reg["codescout"]["arm_image_binary"]["sha256"], ARM_SHA)
        self.assertEqual(reg["head"], "f" * 40)

    def test_twelve_tasks_with_images_templates_and_binary(self):
        reg = self.env.freeze()
        self.assertEqual([t["task_id"] for t in reg["tasks"]], SELECTED)
        by = {t["task_id"]: t for t in reg["tasks"]}
        e = by["cs-p08"]
        self.assertEqual(e["kind"], "private")
        self.assertEqual(e["run_image"], {"tag": "sab-run-cs-p08:f9b0",
                                          "id": image_id("sab-run-cs-p08:f9b0")})
        self.assertEqual(e["arm_image"]["tag"], "sab-arm-cs-p08:f9b0")
        self.assertEqual(e["arm_binary_sha256"], ARM_SHA)
        self.assertEqual(e["template"], "sab-tpl-cs-p08")
        self.assertEqual(e["template_volume"]["files"], 1)
        self.assertEqual(e["template_dump_sha256"], stage3.template_dump_sha256(
            "cs-p08", self.env.data / "tasks"))
        self.assertEqual(reg["registered_tasks"],
                         {t: ("private" if t.startswith("cs-") else "foreign") for t in SELECTED})
        self.assertEqual(self.env.verify_calls, [(SELECTED, SELECTED)])
        self.assertEqual(reg["instrument_gate"]["prescan_reasons"], {})

    def test_plans_seeds_budgets_and_rules(self):
        reg = self.env.freeze()
        self.assertEqual(reg["draw_seed"], 20261030)
        self.assertEqual(reg["split"]["seed"], 20261030)
        self.assertEqual(reg["split"]["rule"], stage3.SPLIT_RULE)
        for part, seed in (("pilot", 20261031), ("extension", 20261032)):
            p = reg["plans"][part]
            self.assertEqual((p["seed"], len(p["order"])), (seed, 48))
            self.assertEqual(p["budget"], {"cap_usd": 200.0, "spent_usd": 0.0,
                                           "per_call_usd": 5.0, "reserve_usd": 5.0})
            self.assertEqual(json.loads(freeze_stage2.plan_file(self.env.run_data, part)
                                        .read_text()), p["order"])
        self.assertEqual(sorted(reg["plans"]["pilot"]["tasks"] + reg["plans"]["extension"]["tasks"]),
                         sorted(SELECTED))
        rule = reg["decision_rule"]
        self.assertEqual((reg["registered_min"], rule["registered_min"]), (10, 10))
        self.assertEqual(reg["stratum_min"], {"foreign": 5, "private": 5})
        self.assertEqual(rule["tolerance"], "a headline needs both strata to decide, so the "
                         "real tolerance is 1 invalid task per stratum")
        self.assertEqual(rule["resamples"], analyze.RESAMPLES)
        self.assertEqual(rule["arm_integrity"], analyze.ARM_INTEGRITY_RULE)
        self.assertEqual(rule["agent_resources"],
                         ["--cpus", "4", "--memory", "16g", "--memory-swap", "16g"])
        self.assertEqual((rule["per_call_usd"], rule["run_timeout_s"], rule["bootstrap_seed"]),
                         (5.0, run.RUN_TIMEOUT_S, 20261033))
        self.assertEqual(rule["comparisons"], {"cs - bare": "codescout", "full - bare": "the setup"})
        self.assertIn("max(bare0_i, bare1_i) / min(bare0_i, bare1_i)", rule["floor_f"])

    def test_go_templates_and_commands(self):
        reg = self.env.freeze()
        go = reg["go_templates"]["pilot"]
        self.assertEqual(go["GO.json"], {
            "cap_usd": 200.0, "per_call_usd": 5.0, "reserve_usd": 5.0, "spent_usd": 0.0,
            "plan_sha256": reg["plans"]["pilot"]["plan_sha256"],
            "registration_sha256": reg["registration_sha256"], "authorized_by": "operator",
            "acknowledged_stops": []})
        cmd = go["command"].split()
        self.assertEqual(cmd[2:4], ["run", "--plan"])
        for flag, value in (("--cap", "200.0"), ("--spent", "0.0"), ("--per-call", "5.0"),
                            ("--reserve", "5.0"),
                            ("--registration", str(self.env.tool / freeze_stage2.REGISTRATION))):
            self.assertEqual(cmd[cmd.index(flag) + 1], value)
        self.assertIn("GO.pilot.json", reg["go_templates"]["extension"]["note"])

    def test_frozen_inputs_recorded(self):
        reg = self.env.freeze()
        rec = json.loads((self.env.frozen / frozen.RECORD).read_text())
        self.assertEqual(reg["frozen"]["trees"], rec["trees"])
        self.assertEqual(reg["frozen"]["root"], str(self.env.frozen.resolve()))
        self.assertEqual(reg["frozen"]["secret_scan"]["hits"], 0)

    # -- refusals -------------------------------------------------------------------------------

    def test_a_changed_or_untracked_tool_file_refuses(self):
        (self.env.tool / "a.py").write_text("a = 2\n")
        self.refused("uncommitted changes")
        self.env.commit()
        (self.env.tool / "json.py").write_text("")
        self.refused("untracked")

    def test_the_registered_file_hashes_are_the_bytes(self):
        reg = self.env.freeze()
        (self.env.tool / "a.py").write_text("a = 3\n")
        self.env.commit()
        self.assertNotEqual(self.env.freeze()["files"]["a.py"], reg["files"]["a.py"])
        self.assertNotEqual(self.env.freeze()["registration_sha256"], reg["registration_sha256"])
        self.assertEqual(reg["files"]["a.py"],
                         hashlib.sha256(b"a = 1\n").hexdigest())

    def test_wrong_python_refuses(self):
        self.refused("3.14.8", python_version=lambda: "3.14.7")

    def test_a_development_task_in_the_selection_refuses(self):
        self.env.select(SELECTED[:11] + ["burntsushi__ripgrep-2576"])
        self.refused("development task")

    def test_verify_or_prescan_reasons_refuse(self):
        self.env.verify_reasons = {"cs-p08": ["leak hits: 1"]}
        self.refused("verify")
        self.env.verify_reasons = {}
        (self.env.data / "tasks" / "cs-p04" / "prescan" / "prescan.json").write_text(
            json.dumps({"id": "cs-p04", "control": "pass", "hits": [{"p": 1}], "reasons": []}))
        self.refused("prescan")

    def test_a_missing_image_refuses(self):
        self.env.inspect.missing.add("sab-arm-cs-p08:f9b0")
        self.refused("cs-p08")

    def test_a_per_image_binary_mismatch_refuses(self):
        self.env.docker.binary["sab-arm-sharkdp__bat-1892:f9b0"] = "1" * 64
        self.refused("sharkdp__bat-1892.*holds codescout")

    def test_a_template_without_a_content_hash_refuses(self):
        self.env.docker.empty_template.add("sab-tpl-cs-p19")
        self.refused("cs-p19")

    def test_frozen_inputs_changed_or_carrying_a_secret_refuse(self):
        (self.env.frozen / "plugins" / "superpowers" / "late.md").write_text("x")
        self.refused("frozen inputs")
        (self.env.frozen / "plugins" / "superpowers" / "late.md").unlink()
        self.env.freeze()  # restored: passes
        (self.env.tool / freeze_stage2.REGISTRATION).unlink()
        for part in freeze_stage2.PARTS:
            freeze_stage2.plan_file(self.env.run_data, part).unlink()
        (self.env.frozen / "frozen.json").write_text(json.dumps(dict(
            json.loads((self.env.frozen / "frozen.json").read_text()))))
        secret = self.env.frozen / "plugins" / "superpowers" / "README.md"
        secret.write_text("PGPASSWORD=s3cr3t-pw psql\n")
        rec = json.loads((self.env.frozen / "frozen.json").read_text())
        rec["trees"]["plugins/superpowers"] = frozen.tree_sha256(secret.parent)
        (self.env.frozen / "frozen.json").write_text(json.dumps(rec))
        self.refused("secret scan")

    def test_plans_that_overlap_miss_a_task_or_change_the_budget_refuse(self):
        def overlapping(*a, **k):
            p = stage3.registration_pieces(*a, **k)
            p["plans"]["extension"]["tasks"][0] = p["plans"]["pilot"]["tasks"][0]
            return p
        self.refused("overlap", pieces_fn=overlapping)

        def short(*a, **k):
            p = stage3.registration_pieces(*a, **k)
            p["plans"]["pilot"]["order"] = p["plans"]["pilot"]["order"][:-1]
            return p
        self.refused("47 runs", pieces_fn=short)

        def dear(*a, **k):
            p = stage3.registration_pieces(*a, **k)
            p["plans"]["pilot"]["budget"] = dict(p["plans"]["pilot"]["budget"], per_call_usd=2.0)
            return p
        self.refused("budget", pieces_fn=dear)

    def test_the_cli_must_match_the_manifest(self):
        self.refused("reports", run_cli=lambda path: "2.1.310 (Claude Code)\n")
        Path(self.env.manifest_body["cli"]["path"]).write_bytes(b"#!another cli\n")
        self.refused("changed after freeze")

    def test_main_prints_refused_and_exits_1(self):
        (self.env.tool / "a.py").write_text("dirty\n")
        err = io.StringIO()
        with contextlib.redirect_stderr(err):
            rc = freeze_stage2.main(["--tool-dir", str(self.env.tool), "--data",
                                     str(self.env.data), "--manifest", str(self.env.manifest),
                                     "--run-data", str(self.env.run_data),
                                     "--bootstrap-seed", "1"])
        self.assertEqual(rc, 1)
        self.assertTrue(err.getvalue().startswith("REFUSED: "))


if __name__ == "__main__":
    unittest.main()
