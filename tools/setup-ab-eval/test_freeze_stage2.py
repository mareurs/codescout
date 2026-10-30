"""Tests for freeze_stage2 and registration: the Stage 3 registration and its re-checks. A temp
git repo stands in for the checkout; fake docker, inspect, verify and `claude`, a temp S5 dir and
a temp study dir holding a Stage 1 journal stand in for the host. No model, token, container or
real data dir is touched."""
import contextlib
import hashlib
import io
import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

import analyze
import analyze_study
import freeze_stage1
import freeze_stage2
import frozen
import registration
import run
import stage3

PRIVATE = [f"cs-p{i:02d}" for i in (1, 4, 7, 8, 15, 19)]
FOREIGN = ["tokio-rs__axum-691", "uutils__coreutils-6575", "nushell__nushell-13831",
           "astral-sh__ruff-15626", "sharkdp__bat-1892", "tokio-rs__tokio-6603"]
SELECTED = [t for pair in zip(PRIVATE, FOREIGN) for t in pair]
CLI_VERSION = "2.1.309 (Claude Code)"
GIT_FULL = "f9b0fc9f867a0b6f5b6bb0d0b436fb303e1c5248"
ARM_SHA = "06c8167b707913b08bc8403e04a4cbb9f80c83c0f63a9d76eeef3eda15f3c3c7"
PROBE_ID = "sha256:" + "0" * 64
LIVE_CONTROL = {"image": {"codescout": {
    "sha": GIT_FULL, "binary_sha256": ARM_SHA,
    "version_json": {"version": "0.15.0", "git_sha": "f9b0fc9", "git_sha_full": GIT_FULL}}}}
PYTHON = str(Path(sys.executable).resolve())
STAGE1_ORDER = run.stage1_order(20261027)


def git(repo, *args):
    return subprocess.run(["git", "-C", str(repo), *args], check=True, capture_output=True,
                          text=True).stdout


def image_id(tag, gen=0):
    return "sha256:" + hashlib.sha256(f"{tag}#{gen}".encode()).hexdigest()


class Inspect:
    """inspect(image) -> (id, exec-only label); arm images carry their run image's id."""

    def __init__(self):
        self.missing = set()
        self.rebuilt = set()   # tags that now resolve to another id (a retag)

    def __call__(self, image):
        if image in self.missing:
            return None, None
        gen = 1 if image in self.rebuilt else 0
        if image.startswith("sab-arm-"):
            return image_id(image, gen), image_id(image.replace("sab-arm-", "sab-run-"))
        return image_id(image, gen), None


class Docker:
    """stage3's read-only docker checks: binary cat, volume and image inspect, volume listing."""

    def __init__(self):
        self.calls = []
        self.binary = {}           # image -> sha override
        self.empty_template = set()
        self.missing_volume = set()
        self.listing_gen = 0

    def __call__(self, *args, check=True, timeout=None):
        a = [str(x) for x in args]
        self.calls.append(a)
        if a[:2] == ["volume", "inspect"]:
            return SimpleNamespace(returncode=int(a[2] in self.missing_volume), stdout="")
        if a[:2] == ["image", "inspect"]:
            return SimpleNamespace(returncode=0, stdout=PROBE_ID + "\n")
        assert a[:2] == ["run", "--rm"] and a[a.index("--network") + 1] == "none", a
        if a[-1] == "/opt/sab/sha256.txt":
            out = f"{self.binary.get(a[-2], ARM_SHA)}  /out/bin/codescout\nab  /out/bin/ra\n"
        else:
            tpl = next(x for x in a if x.endswith(":/tpl:ro")).split(":")[0]
            out = ("" if tpl in self.empty_template
                   else f"aa  ./{tpl}/segment{self.listing_gen}\n")
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
        (self.tool / "stage1-registration.json").write_text(json.dumps(
            {"stage": 1, "order": [list(s) for s in STAGE1_ORDER]}))
        git(self.repo, "init", "-q")
        git(self.repo, "config", "user.email", "t@t")
        git(self.repo, "config", "user.name", "t")
        self.commit("init")
        self.s5 = self.tmp / "plugins" / "effort-steering" / "spikes" / "s5"
        (self.s5.parent.parent / "core").mkdir(parents=True)
        self.s5.mkdir(parents=True)
        (self.s5 / "grade.mjs").write_text(
            "import { a } from './docker.mjs';\nimport {\n  b\n} from '../../core/decide.mjs';\n")
        (self.s5 / "docker.mjs").write_text("export const a = 1;\n")
        (self.s5.parent.parent / "core" / "decide.mjs").write_text("export const b = 2;\n")
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
        self.stage1_journal(self.run_data)
        self.frozen = self.tmp / "study" / "frozen"
        frozen.freeze_inputs(self.manifest_body, self.frozen)
        self.inspect = Inspect()
        self.docker = Docker()
        self.verify_calls = []
        self.verify_reasons = {}

    def stage1_journal(self, data, order=STAGE1_ORDER, stop="plan complete"):
        """Stage 1's journal as run_batch wrote it: start, call+rows per run, stop."""
        run._event(data, "start", plan_sha256=run.plan_sha256(order), budget={
            "cap_usd": 25.0, "spent_usd": 0.0, "per_call_usd": 2.0, "reserve_usd": 2.0})
        for s in order:
            run._event(data, "call", runId=run.run_id(s))
            run._append(Path(data) / run.ROWS, run.make_row(s, "s", outcome=run.PENDING))
            run._append(Path(data) / run.ROWS, run.make_row(
                s, "s", outcome="dev-scored", costUsd=0.39, grade={"pass": True}))
        run._event(data, "stop", reason=stop, spent=4.68)

    def task(self, t, kind):
        d = self.data / "tasks" / t
        (d / "controls").mkdir(parents=True)
        out = d / "build"
        rec = {"id": t, "kind": kind, "run_image": f"sab-run-{t}:f9b0",
               "arm_image": f"sab-arm-{t}:f9b0", "image": f"sab-arm-{t}:f9b0",
               "template": f"sab-tpl-{t}", "out": str(out)}
        if kind == "private":
            s5_out = out / "s5" / "tasks" / t
            (s5_out / "hidden" / "tests").mkdir(parents=True)
            (s5_out / "hidden" / "tests" / "it.rs").write_text(f"// hidden {t}\n")
            (s5_out / "manifest.json").write_text(json.dumps({"id": t}))
            (s5_out / "prompt.txt").write_text(f"fix {t}\n")
            (d / "prescan").mkdir()
            (d / "prescan" / "prescan.json").write_text(json.dumps(
                {"id": t, "control": "pass", "hits": [], "reasons": []}))
        else:
            out.mkdir()
            (out / "prompt.txt").write_text(f"fix {t}\n")
            (out / "record.json").write_text(json.dumps({"instance_id": t, "test_patch": "x"}))
            rec["record_path"] = str(out / "record.json")
        (d / "controls" / "controls.json").write_text(json.dumps({"id": t}))
        (d / "controls" / "qdrant-dump.jsonl").write_text(f"{t}\n")
        (d / "bundle.json").write_text(json.dumps(rec))

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
                    python_version=lambda: "3.14.8", executable=sys.executable,
                    python_on_path=lambda: PYTHON, frozen_root=self.frozen, s5_dir=self.s5)
        args.update(kw)
        with contextlib.redirect_stderr(io.StringIO()):
            return freeze_stage2.freeze_stage2(**args)

    def written(self):
        """Every file the freeze could leave: the registration, the plan files, temp files."""
        return sorted(str(p) for d in (self.tool, self.run_data) for p in d.iterdir()
                      if p.name.startswith((".stage3", "stage3")))

    def check(self, reg, **kw):
        args = dict(data=self.run_data, tool_dir=self.tool, inspect=self.inspect, s5_dir=self.s5,
                    docker=self.docker, tasks_root=self.data / "tasks")
        args.update(kw)
        with contextlib.redirect_stderr(io.StringIO()):
            registration.check(reg, **args)


class Case(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.env = Env(self._tmp.name)

    def refused(self, pattern, **kw):
        before = {p: Path(p).read_bytes() for p in self.env.written()}
        with self.assertRaisesRegex(freeze_stage2.Refused, pattern):
            self.env.freeze(**kw)
        # A refusal writes nothing and leaves what existed byte-identical (round 1, I-4).
        self.assertEqual({p: Path(p).read_bytes() for p in self.env.written()}, before)

    def reg(self):
        return json.loads((self.env.tool / freeze_stage2.REGISTRATION).read_text())


class RecordsTest(Case):
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
            go_dir = self.env.tmp / f"go-{part}"
            go_dir.mkdir()
            (go_dir / run.GO_FILE).write_text(json.dumps(reg["go_templates"][part]["GO.json"]))
            run.check_go(go_dir, plan, stage3.stage3_budget(), loaded["registration_sha256"])
        m = run._frozen_inputs(on_disk, manifest)
        self.assertEqual(m["frozen_root"], str(self.env.frozen.resolve()))
        self.env.check(on_disk, deep=True)  # the fresh registration matches the host
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            analyze_study.main(["--data", str(self.env.run_data), "--tasks-root",
                                str(self.env.data / "tasks"), "--registration", str(path)],
                               verify=self.env.verify, inspect=self.env.inspect,
                               tool_dir=self.env.tool)
        self.assertEqual(json.loads(buf.getvalue())["seed"], 20261033)
        self.assertFalse((self.env.run_data / run.GO_FILE).exists())  # never writes GO.json

    def test_files_versions_journal_and_stage1_evidence(self):
        reg = self.env.freeze()
        self.assertEqual(sorted(reg["files"]), ["a.py", "selection-rule.json",
                                                "stage1-registration.json",
                                                "task4-live-control.json"])
        self.assertEqual(reg["files"]["a.py"], hashlib.sha256(b"a = 1\n").hexdigest())
        self.assertEqual(reg["stage1_registration_sha256"],
                         freeze_stage1.sha256_file(self.env.tool / "stage1-registration.json"))
        self.assertEqual(reg["manifest_sha256"],
                         freeze_stage1.sha256_file(self.env.run_data / "setup-manifest.json"))
        self.assertEqual(reg["run_data"], str(self.env.run_data.resolve()))
        self.assertTrue(Path(reg["manifest_path"]).is_absolute())
        self.assertEqual(reg["stage1_journal"], {"runs": 12, "spend_usd": round(12 * 0.39, 6)})
        self.assertEqual((reg["python_version"], reg["python_executable"],
                          reg["claude_cli_version"]), ("3.14.8", PYTHON, CLI_VERSION))
        self.assertEqual(reg["codescout"]["arm_image_binary"]["sha256"], ARM_SHA)
        self.assertEqual(reg["head"], "f" * 40)

    def test_twelve_tasks_with_images_templates_binary_and_grader_inputs(self):
        reg = self.env.freeze()
        self.assertEqual([t["task_id"] for t in reg["tasks"]], SELECTED)
        e = {t["task_id"]: t for t in reg["tasks"]}["cs-p08"]
        self.assertEqual(e["kind"], "private")
        self.assertEqual(e["run_image"], {"tag": "sab-run-cs-p08:f9b0",
                                          "id": image_id("sab-run-cs-p08:f9b0")})
        self.assertEqual(e["arm_binary_sha256"], ARM_SHA)
        self.assertEqual(e["template_volume"]["probe_image_id"], PROBE_ID)
        self.assertEqual(e["template_dump_sha256"], stage3.template_dump_sha256(
            "cs-p08", self.env.data / "tasks"))
        g = reg["grader_inputs"]
        self.assertEqual(g["s5_dir"], str(self.env.s5.resolve()))
        self.assertEqual(sorted(Path(p).name for p in g["grade_mjs"]),
                         ["decide.mjs", "docker.mjs", "grade.mjs"])  # the import closure
        s5_out = self.env.data / "tasks" / "cs-p08" / "build" / "s5" / "tasks" / "cs-p08"
        self.assertEqual(g["tasks"]["cs-p08"], {
            "hidden_tree": frozen.tree_sha256(s5_out / "hidden"),
            "manifest_json": freeze_stage1.sha256_file(s5_out / "manifest.json")})
        rec = self.env.data / "tasks" / FOREIGN[0] / "build" / "record.json"
        self.assertEqual(g["tasks"][FOREIGN[0]], {"record_json": freeze_stage1.sha256_file(rec)})
        self.assertEqual(reg["registered_tasks"],
                         {t: ("private" if t.startswith("cs-") else "foreign") for t in SELECTED})
        self.assertEqual(self.env.verify_calls, [(SELECTED, SELECTED)])

    def test_plans_seeds_budgets(self):
        reg = self.env.freeze()
        self.assertEqual((reg["draw_seed"], reg["split"]["seed"]), (20261030, 20261030))
        self.assertEqual(reg["split"]["rule"], stage3.SPLIT_RULE)
        for part, seed in (("pilot", 20261031), ("extension", 20261032)):
            p = reg["plans"][part]
            self.assertEqual((p["seed"], len(p["order"])), (seed, 48))
            self.assertEqual(p["budget"], {"cap_usd": 200.0, "spent_usd": 0.0,
                                           "per_call_usd": 5.0, "reserve_usd": 5.0})
            self.assertEqual(json.loads(freeze_stage2.plan_file(self.env.run_data, part)
                                        .read_text()), p["order"])

    def test_the_whole_decision_rule(self):
        rule = self.env.freeze()["decision_rule"]
        self.assertEqual(rule, {
            "registered_min": 10,
            "stratum_min_rule": analyze.STRATUM_MIN_RULE,
            "tolerance": "a headline needs both strata to decide, so the real tolerance is 1 "
                         "invalid task per stratum",
            "floor_f": freeze_stage2.F_FORMULA,
            "resamples": 10_000,
            "interval": freeze_stage2.INTERVAL_RULE,
            "bootstrap_seed": 20261033,
            "comparisons": {"cs - bare": "codescout", "full - bare": "the setup"},
            "comparison_note": "per model; full - bare is 'the setup', never 'codescout'",
            "order": freeze_stage2.ORDER_RULE,
            "non_inferiority": freeze_stage2.NON_INFERIORITY_RULE,
            "arm_integrity": analyze.ARM_INTEGRITY_RULE,
            "instrument": freeze_stage2.INSTRUMENT_RULE,
            "zero_codescout_calls": "a result, reported as zero_codescout_runs; never invalid",
            "admission": analyze.ADMISSION_RULE,
            "re_runs": freeze_stage2.NO_RERUN_RULE,
            "headline": freeze_stage2.HEADLINE_RULE,
            "agent_resources": ["--cpus", "4", "--memory", "16g", "--memory-swap", "16g"],
            "per_call_usd": 5.0,
            "run_timeout_s": run.RUN_TIMEOUT_S,
            "planned_reps": [["bare", 2], ["cs", 1], ["full", 1]],
            "models": list(run.MODELS)})
        self.assertIn("max(bare0_i, bare1_i) / min(bare0_i, bare1_i)", rule["floor_f"])
        self.assertIn("2026-10-29", rule["floor_f"])
        self.assertIn("both to agree", rule["headline"])
        self.assertIn("never run again", rule["re_runs"])

    def test_go_templates_and_commands(self):
        reg = self.env.freeze()
        go = reg["go_templates"]["pilot"]
        self.assertEqual(go["GO.json"], {
            "cap_usd": 200.0, "per_call_usd": 5.0, "reserve_usd": 5.0, "spent_usd": 0.0,
            "plan_sha256": reg["plans"]["pilot"]["plan_sha256"],
            "registration_sha256": reg["registration_sha256"], "authorized_by": "operator",
            "acknowledged_stops": []})
        cmd = go["command"].split()
        self.assertEqual(cmd[0], PYTHON)  # the resolved interpreter (round 1, M-5)
        self.assertEqual(cmd[2:4], ["run", "--plan"])
        for flag, value in (("--cap", "200.0"), ("--spent", "0.0"), ("--per-call", "5.0"),
                            ("--reserve", "5.0"), ("--data", str(self.env.run_data.resolve())),
                            ("--registration",
                             str(self.env.tool.resolve() / freeze_stage2.REGISTRATION))):
            self.assertEqual(cmd[cmd.index(flag) + 1], value)
        self.assertIn("keep Stage 1's GO.json history: replace GO.json, do not delete "
                      "rows.jsonl", go["note"])
        self.assertIn("GO.pilot.json", reg["go_templates"]["extension"]["note"])


class RefusalTest(Case):
    def test_a_changed_or_untracked_tool_file_refuses(self):
        (self.env.tool / "a.py").write_text("a = 2\n")
        self.refused("uncommitted changes")
        self.env.commit()
        (self.env.tool / "json.py").write_text("")
        self.refused("untracked")

    def test_wrong_python_or_another_python3_on_path_refuses(self):
        self.refused("3.14.8", python_version=lambda: "3.14.7")
        self.refused("python3` on PATH", python_on_path=lambda: "/usr/bin/python3.12")
        self.refused("python3` on PATH", python_on_path=lambda: None)

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

    def test_a_template_that_is_empty_or_missing_refuses_without_creating_it(self):
        self.env.docker.empty_template.add("sab-tpl-cs-p19")
        self.refused("cs-p19")
        self.env.docker.empty_template.clear()
        self.env.docker.missing_volume.add("sab-tpl-cs-p07")
        n = len(self.env.docker.calls)
        self.refused("cs-p07.*never created")
        self.assertFalse([c for c in self.env.docker.calls[n:] if "sab-tpl-cs-p07:/tpl:ro" in c])

    def test_an_unreadable_grader_input_refuses(self):
        (self.env.s5 / "docker.mjs").unlink()
        self.refused("grader inputs")

    def test_frozen_inputs_changed_uncovered_or_carrying_a_secret_refuse(self):
        late = self.env.frozen / "plugins" / "superpowers" / "late.md"
        late.write_text("x")
        self.refused("frozen inputs")
        # Refused before any `docker run` (the frozen check is cheap and comes first).
        self.assertFalse([c for c in self.env.docker.calls if c[:1] == ["run"]])
        late.unlink()
        # The manifest names a plugin the frozen copy does not have.
        body = dict(self.env.manifest_body, plugins_full=self.env.manifest_body["plugins_full"]
                    + [{"name": "legible", "install_path": str(self.env.tmp / "legible")}])
        for p in (self.env.manifest, self.env.run_data / "setup-manifest.json"):
            p.write_text(json.dumps(body))
        self.refused("do not cover")
        for p in (self.env.manifest, self.env.run_data / "setup-manifest.json"):
            p.write_text(json.dumps(self.env.manifest_body))
        secret = self.env.frozen / "plugins" / "superpowers" / "README.md"
        secret.write_text("PGPASSWORD=s3cr3t-pw psql\n")
        rec = json.loads((self.env.frozen / "frozen.json").read_text())
        rec["trees"]["plugins/superpowers"] = frozen.tree_sha256(secret.parent)
        (self.env.frozen / "frozen.json").write_text(json.dumps(rec))
        self.refused("secret scan")

    def test_plans_that_overlap_mix_kinds_mis_hash_miss_a_run_or_change_the_budget_refuse(self):
        def edit(f):
            def pieces(*a, **k):
                p = stage3.registration_pieces(*a, **k)
                f(p)
                return p
            return pieces

        def overlap(p):
            p["plans"]["extension"]["tasks"][0] = p["plans"]["pilot"]["tasks"][0]

        def kinds(p):  # still a 6 + 6 partition, but 4 foreign + 2 private in the pilot
            pil, ext, m = p["plans"]["pilot"]["tasks"], p["plans"]["extension"]["tasks"], \
                p["registered_tasks"]
            i = next(i for i, t in enumerate(pil) if m[t] == "private")
            j = next(j for j, t in enumerate(ext) if m[t] == "foreign")
            pil[i], ext[j] = ext[j], pil[i]

        def hashed(p):
            p["plans"]["pilot"]["plan_sha256"] = "0" * 64

        def short(p):
            p["plans"]["pilot"]["order"] = p["plans"]["pilot"]["order"][:-1]

        def dear(p):
            p["plans"]["pilot"]["budget"] = dict(p["plans"]["pilot"]["budget"], per_call_usd=2.0)

        for f, pattern in ((overlap, "overlap"), (kinds, "3 foreign \\+ 3 private"),
                           (hashed, "does not hash its order"), (short, "47 runs"),
                           (dear, "budget")):
            with self.subTest(f.__name__):
                self.refused(pattern, pieces_fn=edit(f))

    def test_the_cli_must_match_the_manifest(self):
        self.refused("reports", run_cli=lambda path: "2.1.310 (Claude Code)\n")
        Path(self.env.manifest_body["cli"]["path"]).write_bytes(b"#!another cli\n")
        self.refused("changed after freeze")

    def test_the_study_dir_must_hold_exactly_stage1(self):
        # Round 1 (I-2): a fresh dir would give the extension its own $200.
        fresh = self.env.tmp / "fresh"
        fresh.mkdir()
        shutil.copy(self.env.run_data / "setup-manifest.json", fresh)
        self.refused("not exactly Stage 1", run_data=fresh)
        for order, stop, pattern in ((STAGE1_ORDER[:-1], "plan complete", "not exactly"),
                                     (STAGE1_ORDER, "admission", "latched")):
            with self.subTest(pattern):
                d = self.env.tmp / f"study-{pattern}"
                d.mkdir()
                shutil.copy(self.env.run_data / "setup-manifest.json", d)
                self.env.stage1_journal(d, order, stop)
                self.refused(pattern, run_data=d)
        extra = self.env.tmp / "study-extra"
        extra.mkdir()
        shutil.copy(self.env.run_data / "setup-manifest.json", extra)
        self.env.stage1_journal(extra)
        spec = run.RunSpec(SELECTED[0], run.MODELS[0], "bare", 0)
        run._event(extra, "start", plan_sha256="x", budget={"per_call_usd": 5.0})
        run._event(extra, "call", runId=run.run_id(spec))
        run._append(extra / run.ROWS, run.make_row(spec, "s", outcome="scored", costUsd=1.0,
                                                   grade={"pass": True}, capHit=False,
                                                   oomKills=0))
        run._event(extra, "stop", reason="plan complete")
        self.refused("not exactly Stage 1", run_data=extra)

    def test_main_prints_refused_and_exits_1(self):
        (self.env.tool / "a.py").write_text("dirty\n")
        err = io.StringIO()
        with contextlib.redirect_stderr(err), contextlib.redirect_stdout(io.StringIO()):
            rc = freeze_stage2.main(["--tool-dir", str(self.env.tool), "--data",
                                     str(self.env.data), "--manifest", str(self.env.manifest),
                                     "--run-data", str(self.env.run_data),
                                     "--bootstrap-seed", "1"])
        self.assertEqual(rc, 1)
        self.assertTrue(err.getvalue().startswith("REFUSED: "))


class WriteOnceTest(Case):
    """Round 1 (I-4): an existing registration is replaced only with --replace, through a temp
    file the self-check reads; a refusal never touches what existed."""

    def test_an_existing_registration_refuses_without_replace(self):
        self.env.freeze()
        first = (self.env.tool / freeze_stage2.REGISTRATION).read_bytes()
        self.refused("exists", head_fn=lambda d: "e" * 40)
        self.assertEqual((self.env.tool / freeze_stage2.REGISTRATION).read_bytes(), first)
        reg = self.env.freeze(replace=True, head_fn=lambda d: "e" * 40)
        self.assertEqual(reg["head"], "e" * 40)
        self.assertEqual(self.reg()["head"], "e" * 40)

    def test_a_tracked_registration_missing_from_disk_still_refuses(self):
        self.env.freeze()
        self.env.commit("register")
        (self.env.tool / freeze_stage2.REGISTRATION).unlink()
        with self.assertRaisesRegex(freeze_stage2.Refused, "tracked"):
            self.env.freeze()

    def test_a_failed_self_check_leaves_the_previous_registration_and_plans(self):
        self.env.freeze()
        before = {p: Path(p).read_bytes() for p in self.env.written()}
        self.assertEqual(len(before), 3)
        for exc in (run.Refused("no"), TypeError("malformed body")):
            with self.subTest(type(exc).__name__), \
                    mock.patch.object(run, "load_registration", side_effect=exc):
                with self.assertRaisesRegex(freeze_stage2.Refused, "self-check"):
                    self.env.freeze(replace=True, head_fn=lambda d: "e" * 40)
            self.assertEqual({p: Path(p).read_bytes() for p in self.env.written()}, before)

    def test_the_self_check_runs_every_reader(self):
        calls = []
        real = registration.check

        def spy(reg, **kw):
            calls.append(kw)
            return real(reg, **kw)
        with mock.patch.object(registration, "check", side_effect=spy):
            self.env.freeze()
        self.assertEqual(len(calls), 1)
        self.assertEqual(calls[0]["data"], self.env.run_data.resolve())
        self.assertFalse(calls[0]["files"])


class RecheckTest(Case):
    """Round 1 (I-1, I-2, I-3): what run_batch, analyze_study and --check re-verify."""

    def setUp(self):
        super().setUp()
        self.env.freeze()
        self.env.commit("register")
        self.registration = self.reg()

    def run_batch(self, part="pilot", data=None):
        plan = run.load_plan(freeze_stage2.plan_file(self.env.run_data, part))
        with contextlib.redirect_stderr(io.StringIO()):
            run.run_batch(plan, stage3.stage3_budget(), data or self.env.run_data,
                          dry_run=False, registration=self.env.tool / freeze_stage2.REGISTRATION,
                          inspect=self.env.inspect, tool_dir=self.env.tool, s5_dir=self.env.s5)

    def assertBatchRefused(self, pattern, **kw):
        with self.assertRaisesRegex(run.Refused, pattern):
            self.run_batch(**kw)

    def test_a_matching_host_passes_to_the_go_check(self):
        self.assertBatchRefused("GO.json is missing")  # every registration check passed
        self.env.check(self.registration, deep=True)

    def test_a_changed_manifest_copy_refuses(self):
        p = self.env.run_data / "setup-manifest.json"
        p.write_text(p.read_text() + " ")
        self.assertBatchRefused("setup-manifest.json hashes")

    def test_a_retagged_image_refuses(self):
        self.env.inspect.rebuilt.add("sab-arm-cs-p08:f9b0")
        self.assertBatchRefused("sab-arm-cs-p08:f9b0 is .*registered")

    def test_a_changed_tool_file_refuses(self):
        (self.env.tool / "a.py").write_text("a = 9\n")
        self.env.commit()
        self.assertBatchRefused("tool file a.py changed")
        (self.env.tool / "b.py").write_text("")
        self.env.commit()
        self.assertBatchRefused("differ from the registered set")

    def test_an_uncommitted_or_untracked_tool_file_refuses(self):
        (self.env.tool / "json.py").write_text("")
        self.assertBatchRefused("untracked")

    def test_another_data_dir_refuses(self):
        other = self.env.tmp / "study-ext"
        shutil.copytree(self.env.run_data, other)
        self.assertBatchRefused("not the registered run_data", data=other)

    def test_changed_grader_inputs_refuse(self):
        hidden = (self.env.data / "tasks" / "cs-p08" / "build" / "s5" / "tasks" / "cs-p08"
                  / "hidden" / "tests" / "it.rs")
        hidden.write_text("// edited\n")
        self.assertBatchRefused("cs-p08: grader inputs changed")
        hidden.write_text("// hidden cs-p08\n")
        (self.env.s5.parent.parent / "core" / "decide.mjs").write_text("export const b = 3;\n")
        self.assertBatchRefused("decide.mjs changed")
        (self.env.s5.parent.parent / "core" / "decide.mjs").write_text("export const b = 2;\n")
        rec = self.env.data / "tasks" / FOREIGN[2] / "build" / "record.json"
        rec.write_text(rec.read_text() + " ")
        self.assertBatchRefused(f"{FOREIGN[2]}: grader inputs changed")

    def test_analyze_study_rechecks_code_and_inputs(self):
        path = self.env.tool / freeze_stage2.REGISTRATION

        def main():
            with contextlib.redirect_stdout(io.StringIO()):
                analyze_study.main(["--data", str(self.env.run_data), "--tasks-root",
                                    str(self.env.data / "tasks"), "--registration", str(path)],
                                   verify=self.env.verify, inspect=self.env.inspect,
                                   tool_dir=self.env.tool)
        main()
        (self.env.tool / "a.py").write_text("RESAMPLES = 10\n")
        self.env.commit()
        with self.assertRaisesRegex(SystemExit, "tool file a.py changed"):
            main()

    def test_the_check_mode_rechecks_deeply_and_writes_nothing(self):
        def check():
            before = sorted(str(p) for p in self.env.tmp.rglob("*"))
            with contextlib.redirect_stdout(io.StringIO()), \
                    contextlib.redirect_stderr(io.StringIO()) as err:
                rc = freeze_stage2.main(["--check", "--tool-dir", str(self.env.tool),
                                         "--data", str(self.env.data),
                                         "--run-data", str(self.env.run_data)],
                                        inspect=self.env.inspect, docker=self.env.docker,
                                        s5_dir=self.env.s5)
            self.assertEqual(sorted(str(p) for p in self.env.tmp.rglob("*")), before)
            return rc, err.getvalue()
        self.assertEqual(check()[0], 0)
        self.env.docker.listing_gen = 1  # the template volume's bytes changed
        rc, err = check()
        self.assertEqual(rc, 1)
        self.assertIn("template_volume differs", err)
        self.env.docker.listing_gen = 0
        self.env.docker.binary["sab-arm-cs-p01:f9b0"] = "2" * 64
        rc, err = check()
        self.assertIn("arm_binary_sha256 differs", err)


if __name__ == "__main__":
    unittest.main()
