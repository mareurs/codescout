"""Tests for run: the sequential runner. No model call, no token, no container.

Every docker call goes to `World.docker`, a fake that keeps the containers, volumes and networks
it was given or that the runner created; every CLI call goes to `FakeCLI`, which writes the
stream and the session transcript a real run would leave. Plan-limit detection and transcript
usage run the vendored .mjs copies through the real `node`, as the runner does.
"""
import hashlib
import json
import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace

import run

TOKEN = "fake-token-not-real-0123456789"
MODEL = "claude-sonnet-5-5"
FOREIGN_CONTAINERS = ("codescout-qdrant", "pbr27-worker", "rae-20261023-proxy", "sab-prep-cs-p04",
                      "sab-dense", "sab-sparse", "sab-qdrant")
FOREIGN_VOLUMES = ("codescout-retrieval_model_cache", "sab-qv-index", "sab-model-cache",
                   "sab-tpl-t1", "sab-tpl-t2", "sab-tpl-t3")
FOREIGN_NETWORKS = ("sab-tasks-net", "bridge")
PATCH = "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1 +1 @@\n-a\n+b\n"


class World:
    """A fake docker daemon: objects by exact name, every argv recorded."""

    def __init__(self, containers=FOREIGN_CONTAINERS, volumes=FOREIGN_VOLUMES,
                 networks=FOREIGN_NETWORKS, uid="1000"):
        self.containers = {n: {"mounts": {}, "oom": False, "env": None, "image": None}
                           for n in containers}
        self.volumes = set(volumes)
        self.networks = set(networks)
        self.calls = []
        self.uid = uid
        self.changed = ["src/lib.rs", ".codescout/usage.db", "crates/x/.buddy/state.json"]
        self.started = []  # (name, image, argv) of every `docker run`

    def __call__(self, *args, check=True, timeout=None, data=None):
        a = [str(x) for x in args]
        self.calls.append(a)
        out, rc = "", 0
        if a[:2] == ["ps", "-a"]:
            out = "".join(n + "\n" for n in self.containers)
        elif a[:2] == ["volume", "ls"]:
            out = "".join(v + "\n" for v in sorted(self.volumes))
        elif a[:2] == ["volume", "inspect"]:
            rc = 0 if a[2] in self.volumes else 1
        elif a[:2] == ["volume", "create"]:
            rc = 1 if a[-1] in self.volumes else 0
            self.volumes.add(a[-1])
        elif a[:2] == ["volume", "rm"]:
            for v in a[2:]:
                if v not in self.volumes:
                    rc = 1
                self.volumes.discard(v)
        elif a[:2] == ["network", "create"]:
            rc = 1 if a[-1] in self.networks else 0
            self.networks.add(a[-1])
        elif a[:2] == ["network", "inspect"]:
            if a[-1] not in self.networks:
                rc = 1
            elif "{{.Internal}}" in a:
                out = "true\n"
            else:
                out = "172.30.5.0/24 \n"
        elif a[:2] == ["network", "rm"]:
            rc = 0 if a[-1] in self.networks else 1
            self.networks.discard(a[-1])
        elif a[:2] == ["network", "connect"]:
            pass
        elif a[0] == "run":
            name = a[a.index("--name") + 1]
            image = self._image(a)
            self.started.append((name, image, a))
            if name in self.containers:
                rc = 1
            elif "--rm" not in a:
                mounts, env = {}, None
                for i, x in enumerate(a):
                    if x == "-v":
                        host, target = a[i + 1].split(":")[:2]
                        mounts[target] = host
                    if x == "--env-file":
                        env = Path(a[i + 1]).read_text()
                self.containers[name] = {"mounts": mounts, "oom": False, "env": env,
                                         "image": image}
        elif a[:2] == ["rm", "-f"]:
            for n in a[2:]:
                self.containers.pop(n, None)
        elif a[0] == "exec":
            i = 1
            while a[i].startswith("-"):
                i += 2 if a[i] in ("-w", "-e") else 1
            name, cmd = a[i], a[i + 1:]
            if name not in self.containers:
                rc = 1
            elif cmd == ["id", "-u"]:
                out = self.uid + "\n"
            elif cmd[:3] == ["git", "rev-parse", "HEAD"]:
                out = "base0\n"
            elif cmd[:3] == ["git", "diff", "--cached"]:
                names = self.changed
                if "--" in cmd:
                    names = [n for n in names if not run.is_excluded_path(n)]
                out = PATCH if "--binary" in cmd else "\0".join(names) + "\0"
        elif a[0] == "inspect":
            c = self.containers.get(a[-1])
            if c is None:
                rc = 1
            elif "OOMKilled" in " ".join(a):
                out = "true\n" if c["oom"] else "false\n"
            else:
                out = "true\n"
        elif a[:2] == ["image", "inspect"]:
            out = "sha256:" + hashlib.sha256(a[2].encode()).hexdigest() + "\n"
        if check and rc:
            raise RuntimeError(f"docker {a[:3]} failed (exit {rc})")
        return SimpleNamespace(returncode=rc, stdout=out)

    @staticmethod
    def _image(a):
        i = a.index("--name") + 2
        while i < len(a):
            if a[i] in ("--network", "--cpus", "--memory", "--env-file", "--cap-drop",
                        "--security-opt", "-v", "--entrypoint", "--device", "--group-add",
                        "-e", "--user", "--name"):
                i += 2
            elif a[i].startswith("-"):
                i += 1
            else:
                return a[i]
        return None

    def runner_objects(self):
        return ({n for n in self.containers if n not in FOREIGN_CONTAINERS},
                {v for v in self.volumes if v not in FOREIGN_VOLUMES},
                {n for n in self.networks if n not in FOREIGN_NETWORKS})


def arm_of(argv):
    if "user,local" in argv:
        return "full"
    return "cs" if "--mcp-config" in argv else "bare"


DEFAULT = {"effort": "medium", "cost": 0.5, "text": "Fixed the bug.", "is_error": False,
           "codescout_call": True, "oom": False, "timeout": False, "no_result": False,
           "dup": False, "bad_tool_use": False, "raise": None, "duration_api_ms": 1200,
           "no_work": False}


class FakeCLI:
    """Plays one CLI run per call; `script(arm, n)` returns overrides of DEFAULT."""

    def __init__(self, world, data, script=None):
        self.world, self.data, self.script = world, Path(data), script or (lambda arm, n: {})
        self.calls = []
        self.row_at_call = []

    def __call__(self, container, argv, timeout):
        arm = arm_of(argv)
        sc = dict(DEFAULT, **self.script(arm, len(self.calls)))
        self.calls.append((container, list(argv), arm))
        assert TOKEN not in " ".join(argv)
        rows = run.read_rows(self.data)
        self.row_at_call.append(rows[-1] if rows else None)
        if sc["raise"]:
            raise sc["raise"]
        session = argv[argv.index("--session-id") + 1]
        config = Path(self.world.containers[container]["mounts"]["/run/config"])
        tdir = config / "projects" / "-testbed"
        tdir.mkdir(parents=True, exist_ok=True)
        rec = {"type": "assistant", "effort": sc["effort"],
               "message": {"id": "msg_1", "role": "assistant", "model": MODEL,
                           "usage": {"input_tokens": 100, "cache_creation_input_tokens": 10,
                                     "cache_read_input_tokens": 5, "output_tokens": 20},
                           "content": [{"type": "text", "text": sc["text"]}]}}
        if sc["effort"] is None:
            del rec["effort"]
        (tdir / f"{session}.jsonl").write_text(json.dumps(rec) + "\n")
        if sc["oom"]:
            self.world.containers[container]["oom"] = True
        tools = ["Bash", "Read", "Edit", "Write", "Glob", "Grep"]
        recs = []
        if arm != "bare":
            tools += ["mcp__codescout__references", "mcp__codescout__symbol_at",
                      "mcp__codescout__symbols"]
            recs.append({"type": "system", "subtype": "hook_response",
                         "hook_name": "SessionStart:startup", "exit_code": 0,
                         "output": "PROJECT BOOTSTRAP: As your FIRST codescout action ..."})
        recs.insert(0, {"type": "system", "subtype": "init", "tools": tools,
                        "skills": ["superpowers:brainstorming"] if arm == "full" else []})
        blocks = [{"type": "tool_use", "id": "toolu_1", "name": "Bash",
                   "input": {"command": "cargo test --offline"}}]
        if sc["no_work"]:
            blocks = []
        if arm != "bare" and sc["codescout_call"] and not sc["no_work"]:
            blocks.append({"type": "tool_use", "id": "toolu_2", "name": "mcp__codescout__symbols",
                           "input": {"name": "x"}})
        assistant = {"type": "assistant", "parent_tool_use_id": None,
                     "message": {"content": blocks}}
        recs.append(assistant)
        if sc["dup"]:
            recs.append(json.loads(json.dumps(assistant)))
        if sc["bad_tool_use"]:
            recs.append({"type": "assistant", "parent_tool_use_id": None,
                         "message": {"content": [{"type": "tool_use", "name": "Read",
                                                  "input": {}}]}})
        if not (sc["timeout"] or sc["no_result"]):
            recs.append({"type": "result", "subtype": "success", "is_error": sc["is_error"],
                         "num_turns": 3, "duration_api_ms": sc["duration_api_ms"],
                         "total_cost_usd": sc["cost"],
                         "usage": ({"input_tokens": 0, "output_tokens": 0} if sc["no_work"]
                                   else {"input_tokens": 100, "output_tokens": 20}),
                         "result": sc["text"]})
        stdout = "".join(json.dumps(r) + "\n" for r in recs)
        return stdout, "", (None if sc["timeout"] else (137 if sc["no_result"] else 0))


class Grader:
    def __init__(self, result=None):
        self.calls = []
        self.result = result or {"pass": True, "oom_killed": False}

    def __call__(self, gdir):
        def grade(patch, name):
            self.calls.append((patch, name))
            return dict(self.result)
        return grade


class RunnerCase(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="sab-run-test-"))
        self.addCleanup(shutil.rmtree, self.tmp, ignore_errors=True)
        self.data = self.tmp / "data"
        self.data.mkdir()
        cli = self.tmp / "fake-cli"
        cli.write_bytes(b"#!fake cli, never executed\n")
        (self.tmp / "settings.json").write_text("{}\n")
        for d in ("companion", "superpowers"):
            (self.tmp / d).mkdir()
        self.manifest = {
            "cli": {"path": str(cli), "sha256": hashlib.sha256(cli.read_bytes()).hexdigest()},
            "mcp_env": {"CODESCOUT_QDRANT_URL": "http://127.0.0.1:6334",
                        "CODESCOUT_EMBEDDER_URL": "http://127.0.0.1:48081",
                        "CODESCOUT_SPARSE_EMBEDDER_URL": "http://127.0.0.1:48084",
                        "CODESCOUT_RERANKER_URL": "http://127.0.0.1:48083",
                        "CODESCOUT_MODEL_DIM": "768"},
            "plugins_cs": {"install_path": str(self.tmp / "companion")},
            "plugins_full": [{"name": "superpowers", "install_path": str(self.tmp / "superpowers")}],
            "full_config_files": [str(self.tmp / "settings.json")],
            "retrieval": {"qdrant": True, "dense": True, "sparse": True, "reranker": False},
        }
        self.world = World()
        self.grader = Grader()

    def load(self, task_id):
        return run.TaskInfo(image=f"sab-arm-{task_id}:t", prompt="Fix the bug.\n",
                            template=f"sab-tpl-{task_id}", grader=self.grader)

    def go(self, plan, cap, **override):
        body = {"cap_usd": cap, "plan_sha256": run.plan_sha256(plan), "authorized_by": "operator"}
        body.update(override)
        (self.data / "GO.json").write_text(json.dumps(body) + "\n")

    def batch(self, plan, budget, cli=None, go=True, **kw):
        if go:
            self.go(plan, budget.cap_usd)
        cli = cli or FakeCLI(self.world, self.data)
        kwargs = dict(docker=self.world, exec_cli=cli, load_task=self.load,
                      manifest=self.manifest, token_reader=lambda: TOKEN)
        kwargs.update(kw)
        run.run_batch(plan, budget, self.data, dry_run=False, **kwargs)
        return cli

    def latest(self):
        return run.latest_rows(run.read_rows(self.data))

    def events(self):
        p = self.data / "batch.jsonl"
        return [json.loads(l) for l in p.read_text().splitlines()] if p.exists() else []


BUDGET = run.Budget(cap_usd=25.0, spent_usd=0.0, per_call_usd=1.25, reserve_usd=1.0)


def spec(task="t1", arm="bare", rep=0, model=MODEL):
    return run.RunSpec(task, model, arm, rep)


class AdmissionTest(RunnerCase):
    def test_admission_refuses_when_spent_plus_reserve_exceeds_cap(self):
        # 8 + 1 (reserve) + 1.25 (this call's worst case) = 10.25 > 10: refused before any docker
        # object or CLI call exists.
        cli = self.batch([spec()], run.Budget(10.0, 8.0, 1.25, 1.0))
        self.assertEqual(cli.calls, [])
        self.assertEqual(self.world.started, [])
        self.assertEqual(self.latest(), {})
        self.assertEqual(self.events()[-1]["event"], "stop")
        self.assertEqual(self.events()[-1]["reason"], "admission")

    def test_admission_boundary_and_journal_spend(self):
        self.assertTrue(run.admit(run.Budget(10.0, 0.0, 1.25, 1.0), spent=7.75))   # == cap
        self.assertFalse(run.admit(run.Budget(10.0, 0.0, 1.25, 1.0), spent=7.76))
        # The first run reports $1.60; 1.60 + 1.0 + 1.25 = 3.85 > 3.5 refuses the second.
        cli = FakeCLI(self.world, self.data, lambda arm, n: {"cost": 1.6})
        self.batch([spec(rep=0), spec(rep=1)], run.Budget(3.5, 0.0, 1.25, 1.0), cli=cli)
        self.assertEqual(len(cli.calls), 1)
        self.assertEqual(self.events()[-1]["reason"], "admission")

    def test_budget_cap_above_the_study_total_is_refused(self):
        with self.assertRaisesRegex(ValueError, "200"):
            run.check_budget(run.Budget(200.01, 0.0, 1.25, 1.0))
        with self.assertRaisesRegex(ValueError, "per_call"):
            run.check_budget(run.Budget(25.0, 0.0, 0.0, 1.0))

    def test_proxy_config_allows_only_the_api_host(self):
        conf, filt = run.render_proxy_config(run.PROXY_TEMPLATE.read_text(), ["172.30.5.0/24"])
        self.assertEqual(filt, "^api\\.anthropic\\.com$\n")
        self.assertIn("\nAllow 172.30.5.0/24\n", conf)
        self.assertIn("FilterDefaultDeny Yes", conf)
        self.assertIn("ConnectPort 443", conf)
        self.assertNotIn("\n@@ALLOW@@\n", conf)
        self.assertNotIn("\n@@FILTER@@\n", conf)
        with self.assertRaises(ValueError):
            run.render_proxy_config(run.PROXY_TEMPLATE.read_text(), ["not-a-subnet"])


class GoGuardTest(RunnerCase):
    def assertRefused(self, plan, budget):
        cli = FakeCLI(self.world, self.data)
        with self.assertRaises(run.Refused):
            self.batch(plan, budget, cli=cli, go=False)
        self.assertEqual(cli.calls, [])
        self.assertEqual(self.world.calls, [])
        self.assertFalse((self.data / "rows.jsonl").exists())

    def test_refused_without_the_operator_file(self):
        self.assertRefused([spec()], BUDGET)

    def test_refused_on_cap_mismatch(self):
        plan = [spec()]
        self.go(plan, 30.0)
        self.assertRefused(plan, BUDGET)

    def test_refused_on_plan_hash_mismatch(self):
        self.go([spec(rep=1)], BUDGET.cap_usd)
        self.assertRefused([spec()], BUDGET)

    def test_refused_unless_authorized_by_the_operator(self):
        plan = [spec()]
        self.go(plan, BUDGET.cap_usd, authorized_by="controller")
        self.assertRefused(plan, BUDGET)

    def test_refused_on_a_symlinked_or_malformed_file(self):
        plan = [spec()]
        self.go(plan, BUDGET.cap_usd)
        real = self.tmp / "elsewhere.json"
        (self.data / "GO.json").rename(real)
        (self.data / "GO.json").symlink_to(real)
        self.assertRefused(plan, BUDGET)
        (self.data / "GO.json").unlink()
        (self.data / "GO.json").write_text("{not json")
        self.assertRefused(plan, BUDGET)

    def test_accepted_only_when_all_match(self):
        cli = self.batch([spec()], BUDGET)
        self.assertEqual(len(cli.calls), 1)
        self.assertEqual(self.latest()[run.run_id(spec())]["outcome"], "scored")

    def test_the_runner_never_writes_the_go_file(self):
        plan = run.stage1_order(seed=7)
        run.run_batch(plan, BUDGET, self.data, dry_run=True)
        self.assertFalse((self.data / "GO.json").exists())
        self.assertNotIn("GO_FILE).write", Path(run.__file__).read_text())


class PlanLimitTest(RunnerCase):
    def test_plan_limit_text_is_neutral_and_stops_the_batch(self):
        refusal = "You've hit your limit · resets 3pm"
        cli = FakeCLI(self.world, self.data,
                      lambda arm, n: {"text": refusal, "is_error": True, "cost": 0.0})
        plan = [spec(arm="bare"), spec(arm="cs"), spec(arm="full")]
        self.batch(plan, BUDGET, cli=cli)
        self.assertEqual(len(cli.calls), 1)
        row = self.latest()[run.run_id(plan[0])]
        self.assertEqual(row["outcome"], "plan-limit")
        self.assertIsNone(row["grade"])
        self.assertEqual(self.grader.calls, [])
        self.assertEqual(self.events()[-1]["reason"], "plan-limit")
        self.assertEqual(set(self.latest()), {run.run_id(plan[0])})

    def test_detection_is_the_vendored_module_through_node(self):
        self.assertEqual(run.PLAN_LIMIT_MJS.name, "plan-limit.mjs")
        self.assertTrue(run.PLAN_LIMIT_MJS.is_relative_to(run.HERE / "vendor"))
        hit = run.plan_limit(envelope={"result": "You've hit your limit · resets 3pm"})
        self.assertEqual(hit["source"], "envelope_result")
        self.assertEqual(hit["resetHint"], "resets 3pm")
        self.assertIsNone(run.plan_limit(envelope={
            "result": "Approaching your 5-hour usage limit — Claude will wrap up."}))
        self.assertIsNone(run.plan_limit(envelope={"result": "x" * 10 + "You've hit your limit"}))
        typed = run.plan_limit(envelope={"api_error": "usage_limit_reached"})
        self.assertEqual(typed["source"], "envelope_api_error")


class InfraOutcomeTest(RunnerCase):
    def test_oom_kill_is_infra_not_a_failure(self):
        cli = FakeCLI(self.world, self.data, lambda arm, n: {"oom": n == 0})
        plan = [spec(arm="cs"), spec(arm="bare")]
        self.batch(plan, BUDGET, cli=cli)
        rows = self.latest()
        oom = rows[run.run_id(plan[0])]
        self.assertEqual(oom["outcome"], "infra-oom")
        self.assertIsNone(oom["grade"])
        self.assertEqual(oom["costUsd"], 0.5)
        # Not a failure: nothing graded for it, and the batch went on (its cost is known).
        self.assertEqual(len(self.grader.calls), 1)
        self.assertEqual(rows[run.run_id(plan[1])]["outcome"], "scored")

    def test_timeout_or_kill_without_a_result_record_is_never_scored(self):
        for case in ({"timeout": True}, {"no_result": True}):
            with self.subTest(case=case):
                self.setUp()
                cli = FakeCLI(self.world, self.data, lambda arm, n, c=case: c)
                plan = [spec(rep=0), spec(rep=1)]
                self.batch(plan, BUDGET, cli=cli)
                row = self.latest()[run.run_id(plan[0])]
                self.assertEqual(row["outcome"], "infra-other")
                self.assertIsNone(row["costUsd"])
                self.assertIsNone(row["grade"])
                self.assertEqual(len(cli.calls), 1)  # missing cost stops the batch
                self.assertEqual(self.events()[-1]["reason"], "missing cost")
                self.assertEqual(run.spent_usd(run.read_rows(self.data), BUDGET), 1.25)
                record = json.loads((self.data / "runs" / run.run_id(plan[0]) /
                                     "run-record.json").read_text())
                self.assertEqual(record["why"], "timeout" if "timeout" in case
                                 else "no result record (exit 137)")

    def test_a_run_that_did_no_model_work_is_infra_and_stops(self):
        cli = FakeCLI(self.world, self.data, lambda arm, n: {
            "is_error": True, "cost": 0, "duration_api_ms": 0, "text": "Not logged in",
            "no_work": True})
        self.batch([spec(), spec(rep=1)], BUDGET, cli=cli)
        self.assertEqual(self.latest()[run.run_id(spec())]["outcome"], "infra-other")
        self.assertEqual(len(cli.calls), 1)


class InvalidArmTest(RunnerCase):
    def test_invalid_arm_is_charged_and_not_scored(self):
        cli = FakeCLI(self.world, self.data, lambda arm, n: {"codescout_call": False})
        plan = [spec(arm="cs")]
        self.batch(plan, BUDGET, cli=cli)
        row = self.latest()[run.run_id(plan[0])]
        self.assertEqual(row["outcome"], "invalid-arm")
        self.assertEqual(row["costUsd"], 0.5)
        self.assertIsNone(row["grade"])
        self.assertEqual(self.grader.calls, [])
        self.assertFalse(row["integrity"]["ok"])
        self.assertIn("cs arm made zero codescout calls", row["integrity"]["reasons"])
        self.assertEqual(run.spent_usd(run.read_rows(self.data), BUDGET), 0.5)

    def test_duplicate_tool_use_blocks_count_once(self):
        cli = FakeCLI(self.world, self.data, lambda arm, n: {"dup": True})
        self.batch([spec(arm="cs")], BUDGET, cli=cli)
        row = self.latest()[run.run_id(spec(arm="cs"))]
        self.assertEqual(row["outcome"], "scored")
        self.assertEqual(row["integrity"]["codescout_calls"], 1)
        self.assertEqual(row["integrity"]["native_calls"], 1)
        self.assertEqual(row["integrity"]["duplicateToolUses"], 2)
        self.assertEqual(row["bashCommands"], ["cargo test --offline"])

    def test_a_malformed_tool_use_block_invalidates_the_arm(self):
        cli = FakeCLI(self.world, self.data, lambda arm, n: {"bad_tool_use": True})
        self.batch([spec(arm="bare")], BUDGET, cli=cli)
        row = self.latest()[run.run_id(spec())]
        self.assertEqual(row["outcome"], "invalid-arm")
        self.assertTrue(row["integrity"]["toolUseShapeErrors"])

    def test_dedupe_keeps_unparseable_lines_for_integrity(self):
        lines = ['{"type":"assistant","message":{"content":[{"type":"tool_use","id":"a",'
                 '"name":"Read","input":{}}]}}', "not json",
                 '{"type":"assistant","message":{"content":[{"type":"tool_use","id":"a",'
                 '"name":"Read","input":{}}]}}']
        out, dups, errors = run.dedupe_tool_uses(lines)
        self.assertEqual(dups, 1)
        self.assertEqual(errors, [])
        self.assertEqual(out[1], "not json")
        self.assertEqual(json.loads(out[2])["message"]["content"], [])


class EffortTest(RunnerCase):
    def test_effort_mismatch_across_arms_of_a_task_marks_the_task_invalid(self):
        cli = FakeCLI(self.world, self.data,
                      lambda arm, n: {"effort": "high" if arm == "cs" else "medium"})
        plan = [spec("t1", "bare"), spec("t1", "cs"), spec("t2", "bare")]
        self.batch(plan, BUDGET, cli=cli)
        self.assertEqual(len(cli.calls), 2)  # stopped before t2
        rows = self.latest()
        self.assertEqual(rows[run.run_id(plan[0])]["effectiveEffort"], "medium")
        self.assertEqual(rows[run.run_id(plan[1])]["effectiveEffort"], "high")
        validity = json.loads((self.data / "task-validity.json").read_text())
        self.assertFalse(validity[f"t1|{MODEL}"]["valid"])
        self.assertEqual(self.events()[-1]["reason"], "effort mismatch")

    def test_task_validity_rule(self):
        def row(rid, effort, outcome="scored", task="t", model=MODEL):
            return {"runId": rid, "taskId": task, "model": model, "effectiveEffort": effort,
                    "outcome": outcome}
        v = run.task_validity([row("a", "medium"), row("b", "medium"),
                               row("c", "xhigh", outcome="invalid-arm")])
        self.assertTrue(v[f"t|{MODEL}"]["valid"])        # non-scored rows do not vote
        v = run.task_validity([row("a", "medium"), row("b", None)])
        self.assertFalse(v[f"t|{MODEL}"]["valid"])       # unknown effort cannot be equal
        v = run.task_validity([row("a", None), row("b", None)])
        self.assertFalse(v[f"t|{MODEL}"]["valid"])       # not even to another unknown
        v = run.task_validity([row("a", "medium"), row("b", "high", model="claude-opus-5-5")])
        self.assertTrue(all(x["valid"] for x in v.values()))  # per task AND model
        self.assertEqual(run.effective_effort({"efforts": ["medium"]}), "medium")
        self.assertIsNone(run.effective_effort({"efforts": ["medium", None]}))
        self.assertIsNone(run.effective_effort({"efforts": []}))


class NoRetryTest(RunnerCase):
    def test_no_automatic_retry_after_any_outcome(self):
        cases = {"scored": {}, "invalid-arm": {"bad_tool_use": True},
                 "plan-limit": {"text": "You've hit your limit", "is_error": True},
                 "infra-oom": {"oom": True}, "infra-other": {"timeout": True}}
        for outcome, behaviour in cases.items():
            with self.subTest(outcome=outcome):
                self.setUp()
                cli = FakeCLI(self.world, self.data, lambda arm, n, b=behaviour: b)
                plan = [spec()]
                self.batch(plan, BUDGET, cli=cli)
                self.assertEqual(self.latest()[run.run_id(plan[0])]["outcome"], outcome)
                self.batch(plan, BUDGET, cli=cli)  # the same plan again: nothing re-runs
                self.assertEqual(len(cli.calls), 1)
                self.assertEqual(len(run.read_rows(self.data)), 2)  # pending + final
                self.assertIn("skip", [e["event"] for e in self.events()])

    def test_a_recovered_unknown_spend_run_is_not_retried(self):
        plan = [spec()]
        pending = run.make_row(plan[0], "s-1", outcome=run.PENDING)
        (self.data / "rows.jsonl").write_text(json.dumps(pending) + "\n")
        cli = self.batch(plan, BUDGET)
        self.assertEqual(cli.calls, [])
        self.assertEqual(self.latest()[run.run_id(plan[0])]["outcome"], "unknown-spend")


class PendingTest(RunnerCase):
    def test_pending_row_is_written_before_the_cli_call(self):
        cli = FakeCLI(self.world, self.data, lambda arm, n: {"raise": KeyboardInterrupt()})
        with self.assertRaises(KeyboardInterrupt):
            self.batch([spec()], BUDGET, cli=cli)
        at_call = cli.row_at_call[0]
        self.assertEqual(at_call["runId"], run.run_id(spec()))
        self.assertEqual(at_call["outcome"], run.PENDING)
        # The interruption leaves `unknown-spend`, never a silent gap, and it is charged.
        row = self.latest()[run.run_id(spec())]
        self.assertEqual(row["outcome"], "unknown-spend")
        self.assertEqual(run.spent_usd(run.read_rows(self.data), BUDGET), 1.25)
        self.assertEqual(self.world.runner_objects(), (set(), set(), set()))

    def test_pending_rows_left_by_a_killed_runner_become_unknown_spend_at_start(self):
        # A hard kill (SIGKILL) writes nothing after the pending row: simulate its journal.
        plan = [spec(rep=0), spec(rep=1)]
        pending = run.make_row(plan[0], "s-1", outcome=run.PENDING)
        (self.data / "rows.jsonl").write_text(json.dumps(pending) + "\n")
        budget = run.Budget(3.5, 0.0, 1.25, 1.0)
        # Charged at per_call: 1.25 + 1.0 + 1.25 = 3.5 <= 3.5 admits rep 1 and nothing more.
        cli = self.batch(plan, budget)
        rows = self.latest()
        self.assertEqual(rows[run.run_id(plan[0])]["outcome"], "unknown-spend")
        self.assertIsNone(rows[run.run_id(plan[0])]["costUsd"])
        self.assertEqual(len(cli.calls), 1)
        self.assertEqual(cli.calls[0][1][cli.calls[0][1].index("--session-id") + 1] != "s-1", True)
        self.assertEqual(run.spent_usd(run.read_rows(self.data), budget), 1.25 + 0.5)


class CleanupTest(RunnerCase):
    def test_cleanup_removes_only_sab_objects(self):
        plan = [spec(arm="bare"), spec(arm="cs"), spec(arm="full")]
        self.batch(plan, BUDGET)
        self.assertEqual({r["outcome"] for r in self.latest().values()}, {"scored"})
        # Every foreign object survives, including the background job's sab-prep-*/sab-dense/
        # sab-sparse/sab-qdrant and the templates; nothing the runner made is left.
        for name in FOREIGN_CONTAINERS:
            self.assertIn(name, self.world.containers)
        self.assertTrue(set(FOREIGN_VOLUMES) <= self.world.volumes)
        self.assertTrue(set(FOREIGN_NETWORKS) <= self.world.networks)
        self.assertEqual(self.world.runner_objects(), (set(), set(), set()))
        # Removal was by exact name only: every rm names objects in the ownership ledger.
        owned = {e["name"] for e in map(json.loads, (self.data / "owned.jsonl").read_text()
                                        .splitlines()) if e["event"] == "claim"}
        for argv in self.world.calls:
            if argv[:2] in (["rm", "-f"], ["volume", "rm"]):
                self.assertTrue(set(argv[2:]) <= owned, argv)
            if argv[:2] == ["network", "rm"]:
                self.assertIn(argv[-1], owned)
        self.assertTrue(all(n.startswith("sab-") for n in owned))

    def test_a_preexisting_runner_name_is_refused_not_adopted(self):
        self.world.networks.add(run.NET)
        cli = FakeCLI(self.world, self.data)
        with self.assertRaises(run.Refused):
            self.batch([spec()], BUDGET, cli=cli)
        self.assertIn(run.NET, self.world.networks)
        self.assertEqual(cli.calls, [])

    def test_cleanup_on_error_mid_run(self):
        cli = FakeCLI(self.world, self.data, lambda arm, n: {"raise": RuntimeError("boom")})
        with self.assertRaisesRegex(RuntimeError, "boom"):
            self.batch([spec(arm="cs")], BUDGET, cli=cli)
        self.assertEqual(self.world.runner_objects(), (set(), set(), set()))
        self.assertEqual(self.latest()[run.run_id(spec(arm="cs"))]["outcome"], "unknown-spend")


class ContainerTest(RunnerCase):
    def test_env_file_no_proxy_covers_every_study_host(self):
        self.batch([spec(arm="cs"), spec(arm="bare")], BUDGET)
        agents = [(n, img, a) for n, img, a in self.world.started
                  if n.startswith(run.AGENT_PREFIX)]
        self.assertEqual(len(agents), 2)
        cs_dir = self.data / "runs" / run.run_id(spec(arm="cs"))
        mcp_env = json.loads((cs_dir / "sab" / "mcp.json").read_text())["mcpServers"]["codescout"]["env"]
        hosts = {run.url_host(v) for v in mcp_env.values() if "://" in v}
        self.assertTrue(hosts and all(h.startswith("sab-") for h in hosts))
        for name, _, _ in agents:
            env = dict(l.split("=", 1) for l in self.world_env(name).splitlines())
            no_proxy = set(env["NO_PROXY"].split(","))
            self.assertEqual(env["no_proxy"], env["NO_PROXY"])
            self.assertTrue({"127.0.0.1", "localhost", run.DENSE, run.SPARSE} <= no_proxy)
            self.assertEqual(env["HTTPS_PROXY"], f"http://{run.PROXY}:{run.PROXY_PORT}")
            self.assertEqual(env["CLAUDE_CODE_OAUTH_TOKEN"], TOKEN)
        cs_name = agents[0][0]
        cs_no_proxy = set(dict(l.split("=", 1) for l in self.world_env(cs_name).splitlines())
                          ["NO_PROXY"].split(","))
        self.assertTrue(hosts <= cs_no_proxy)
        # The env file held the token only until the container started.
        self.assertEqual(list((self.data / "runs").rglob("private.env")), [])
        for path in (self.data / "runs").rglob("*"):
            if path.is_file():
                self.assertNotIn(TOKEN, path.read_text(errors="replace"), path)

    def world_env(self, name):
        return self.world.env_seen[name]

    def test_arm_runs_start_from_the_verified_arm_image_only(self):
        root = self.tmp / "tasks"
        (root / "t1").mkdir(parents=True)
        out = root / "t1" / "build"
        out.mkdir()
        (out / "prompt.txt").write_text("Fix it.\n")
        (root / "t1" / "bundle.json").write_text(json.dumps({
            "kind": "foreign", "out": str(out), "run_image": "sab-run-t1:x",
            "arm_image": "sab-arm-t1:x", "image": "sab-arm-t1:x", "template": "sab-tpl-t1"}))
        ids = {"sab-run-t1:x": ("sha256:run", None), "sab-arm-t1:x": ("sha256:arm", "sha256:run")}
        inspect = lambda image: ids.get(image, (None, None))
        made = []
        def grader_for(rec, image, gdir):
            made.append(image)
            return lambda patch, name: {"pass": True}
        info = run.load_task_from_bundle("t1", root, inspect=inspect, grader_for=grader_for)
        self.assertEqual(info.image, "sab-arm-t1:x")
        self.assertEqual(info.prompt, "Fix it.\n")
        load = lambda t: run.load_task_from_bundle(t, root, inspect=inspect, grader_for=grader_for)
        self.batch([spec(arm="bare")], BUDGET, load_task=load)
        agents = [img for n, img, a in self.world.started if n.startswith(run.AGENT_PREFIX)]
        self.assertEqual(agents, ["sab-arm-t1:x"])
        self.assertEqual(made, ["sab-arm-t1:x"])  # graded on the image the agent ran in
        self.assertNotIn("sab-run-t1:x", json.dumps(self.world.calls))
        # An arm image that is not the exec-only layer of the current run image is refused.
        ids["sab-arm-t1:x"] = ("sha256:arm", "sha256:stale")
        with self.assertRaisesRegex(RuntimeError, "exec-only layer"):
            run.load_task_from_bundle("t1", root, inspect=inspect, grader_for=grader_for)

    def test_agent_must_run_as_uid_1000(self):
        self.world.uid = "0"
        cli = FakeCLI(self.world, self.data)
        with self.assertRaisesRegex(RuntimeError, "uid"):
            self.batch([spec()], BUDGET, cli=cli)
        self.assertEqual(cli.calls, [])
        self.assertEqual(self.world.runner_objects(), (set(), set(), set()))

    def test_cli_argv_is_the_arm_invocation_with_stream_output_and_the_per_call_cap(self):
        self.batch([spec(arm="full")], BUDGET)
        argv = self.world_cli_argv()
        sep = argv.index("--")
        flags = argv[:sep]
        self.assertNotIn("--effort", flags)
        self.assertEqual(flags[flags.index("--max-budget-usd") + 1], "1.25")
        self.assertEqual(flags[flags.index("--output-format") + 1], "stream-json")
        self.assertIn("--verbose", flags)
        self.assertIn("--include-hook-events", flags)
        self.assertEqual(argv[sep + 1:], ["Fix the bug.\n"])
        with self.assertRaises(ValueError):
            run.cli_argv(["/opt/claude", "-p", "--effort", "low", "--output-format", "json",
                          "--max-budget-usd", "1.25", "--", "x"], 1.25)
        # A prompt that mentions a flag is not mistaken for one.
        args = run.cli_argv(["/opt/claude", "-p", "--output-format", "json", "--max-budget-usd",
                             "1.25", "--", "use --effort high"], 2.0)
        self.assertEqual(args[-1], "use --effort high")
        self.assertEqual(args[args.index("--max-budget-usd") + 1], "2.0")

    def world_cli_argv(self):
        return self.last_cli.calls[-1][1]

    def batch(self, plan, budget, cli=None, go=True, **kw):
        self.world.env_seen = {}
        real_call = self.world.__class__.__call__
        world = self.world

        def spy(*args, **kwargs):
            a = [str(x) for x in args]
            if a and a[0] == "run" and "--env-file" in a:
                world.env_seen[a[a.index("--name") + 1]] = Path(a[a.index("--env-file") + 1]).read_text()
            return real_call(world, *args, **kwargs)
        cli = cli or FakeCLI(world, self.data)
        self.last_cli = cli
        if go:
            self.go(plan, budget.cap_usd)
        kwargs = dict(docker=spy, exec_cli=cli, load_task=self.load, manifest=self.manifest,
                      token_reader=lambda: TOKEN)
        kwargs.update(kw)
        run.run_batch(plan, budget, self.data, dry_run=False, **kwargs)
        return cli


class PatchTest(unittest.TestCase):
    def test_patch_extraction_excludes_tool_state_at_any_depth(self):
        repo = Path(tempfile.mkdtemp(prefix="sab-patch-"))
        self.addCleanup(shutil.rmtree, repo, ignore_errors=True)

        def git(*a):
            return subprocess.run(["git", "-C", str(repo), *a], check=True, capture_output=True,
                                  text=True).stdout
        git("init", "-q")
        git("config", "user.email", "t@example.invalid")
        git("config", "user.name", "t")
        for p in ("src/lib.rs", ".codescout/project.toml"):
            (repo / p).parent.mkdir(parents=True, exist_ok=True)
            (repo / p).write_text("a\n")
        git("add", "-A")
        git("commit", "-qm", "base")
        base = git("rev-parse", "HEAD").strip()
        for p, text in {"src/lib.rs": "b\n", "src/new.rs": "new\n",
                        ".codescout/project.toml": "changed\n", ".codescout/usage.db": "db\n",
                        "crates/x/.buddy/state.json": "{}\n", ".superpowers/sdd/x.md": "x\n",
                        "docs/superpowers.md": "kept: not a .superpowers dir\n"}.items():
            (repo / p).parent.mkdir(parents=True, exist_ok=True)
            (repo / p).write_text(text)
        patch, edited, excluded = run.extract_patch(git, base)
        self.assertEqual(sorted(edited), ["docs/superpowers.md", "src/lib.rs", "src/new.rs"])
        self.assertEqual(sorted(excluded), [".codescout/project.toml", ".codescout/usage.db",
                                            ".superpowers/sdd/x.md", "crates/x/.buddy/state.json"])
        self.assertIn("src/new.rs", patch)
        for d in (".codescout/", ".buddy/", ".superpowers/"):
            self.assertNotIn(d, patch)
        self.assertEqual(run.PATCH_EXCLUDED_DIRS, (".codescout", ".buddy", ".superpowers"))


class RowTest(RunnerCase):
    def test_row_records_edited_and_excluded_files(self):
        self.batch([spec()], BUDGET)
        row = self.latest()[run.run_id(spec())]
        self.assertEqual(row["editedFiles"], ["src/lib.rs"])
        record = json.loads((self.data / "runs" / run.run_id(spec()) / "run-record.json")
                            .read_text())
        self.assertEqual(record["patchExcluded"], [".codescout/usage.db",
                                                   "crates/x/.buddy/state.json"])
        self.assertEqual((self.data / "runs" / run.run_id(spec()) / "agent.patch").read_text(),
                         PATCH)

    def test_rows_carry_exactly_the_brief_fields_plus_r6(self):
        self.batch([spec(arm="cs")], BUDGET)
        rows = run.read_rows(self.data)
        brief = ["runId", "taskId", "model", "arm", "rep", "sessionId", "costUsd", "wallMs",
                 "numTurns", "usage", "effectiveEffort", "grade", "integrity", "outcome"]
        r6 = ["bashCommands", "editedFiles", "inputTokens", "outputTokens"]
        for row in rows:
            self.assertEqual(sorted(row), sorted(brief + r6))
        final = rows[-1]
        self.assertEqual(final["outcome"], "scored")
        self.assertEqual(final["numTurns"], 3)
        self.assertEqual(final["inputTokens"], 115)   # input + cache creation + cache read
        self.assertEqual(final["outputTokens"], 20)
        self.assertEqual(final["effectiveEffort"], "medium")
        self.assertEqual(final["grade"], {"pass": True, "oom_killed": False})
        self.assertTrue(final["integrity"]["ok"])
        self.assertIn(final["outcome"], run.OUTCOMES)


class OrderTest(unittest.TestCase):
    def test_make_order_composition_and_determinism(self):
        order = run.make_order(["a", "b"], list(run.MODELS), seed=11)
        self.assertEqual(len(order), 16)
        for task in ("a", "b"):
            for model in run.MODELS:
                block = [s for s in order if s.task_id == task and s.model == model]
                self.assertEqual(sorted((s.arm, s.rep) for s in block),
                                 [("bare", 0), ("bare", 1), ("cs", 0), ("full", 0)])
        self.assertEqual(order, run.make_order(["a", "b"], list(run.MODELS), seed=11))
        orders = {tuple(run.make_order(["a", "b"], list(run.MODELS), seed=s)) for s in range(20)}
        self.assertGreater(len(orders), 1)
        # Blocks stay contiguous: task, then model, in the given order.
        self.assertEqual([(s.task_id, s.model) for s in order[::4]],
                         [("a", run.MODELS[0]), ("a", run.MODELS[1]),
                          ("b", run.MODELS[0]), ("b", run.MODELS[1])])

    def test_stage1_is_twelve_specs(self):
        order = run.stage1_order(seed=3)
        self.assertEqual(len(order), 12)
        self.assertEqual(len({s.task_id for s in order}), 2)
        self.assertEqual({s.model for s in order}, set(run.MODELS))
        self.assertEqual({(s.arm, s.rep) for s in order}, {("bare", 0), ("cs", 0), ("full", 0)})

    def test_plan_hash_is_order_sensitive_and_canonical(self):
        a = run.make_order(["a"], [MODEL], seed=1)
        self.assertEqual(run.plan_sha256(a), run.plan_sha256([tuple(s) for s in a]))
        self.assertNotEqual(run.plan_sha256(a), run.plan_sha256(list(reversed(a))))


class DryRunTest(unittest.TestCase):
    def test_dry_run_invokes_nothing(self):
        data = Path(tempfile.mkdtemp(prefix="sab-dry-"))
        self.addCleanup(shutil.rmtree, data, ignore_errors=True)
        calls = []
        def forbidden(*a, **k):
            calls.append(a)
            raise AssertionError("dry run touched docker or the CLI")
        plan = run.stage1_order(seed=5)
        run.run_batch(plan, BUDGET, data, dry_run=True, docker=forbidden, exec_cli=forbidden,
                      load_task=forbidden, token_reader=forbidden)
        self.assertEqual(calls, [])
        out = json.loads((data / "dry-run.json").read_text())
        self.assertEqual(len(out["specs"]), 12)
        self.assertEqual(out["plan_sha256"], run.plan_sha256(plan))
        self.assertFalse((data / "rows.jsonl").exists())


class VendoredTest(unittest.TestCase):
    def test_vendored_js_matches_provenance(self):
        prov = json.loads((run.HERE / "PROVENANCE.json").read_text())["vendored"]
        self.assertEqual(set(prov["files"]), {
            "vendor/claude-plugins/effort-steering/spikes/plan-limit.mjs",
            "vendor/claude-plugins/effort-steering/spikes/s4-rows.mjs",
            "vendor/claude-plugins/effort-steering/spikes/s5/usage.mjs",
            "vendor/claude-plugins/effort-steering/spikes/s5/images/tinyproxy.conf.tmpl"})
        for rel, meta in prov["files"].items():
            got = hashlib.sha256((run.HERE / rel).read_bytes()).hexdigest()
            self.assertEqual(got, meta["sha256"], rel)
        repo = Path(prov["source_repo"])
        if (repo / ".git").exists():
            for rel, meta in prov["files"].items():
                blob = subprocess.run(["git", "-C", str(repo), "show",
                                       f"{prov['source_commit']}:{meta['source_path']}"],
                                      capture_output=True, check=True).stdout
                self.assertEqual(hashlib.sha256(blob).hexdigest(), meta["sha256"], rel)
        self.assertEqual(run.USAGE_MJS, run.HERE / "vendor/claude-plugins/effort-steering/spikes/s5/usage.mjs")


if __name__ == "__main__":
    unittest.main()
