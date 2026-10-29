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
from unittest import mock

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
        self.fail_rm = set()  # container names whose `rm -f` fails
        self.killed = []

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
            if any(n in self.fail_rm for n in a[2:]):
                rc = 1
            else:
                for n in a[2:]:
                    self.containers.pop(n, None)
        elif a[0] == "kill":
            self.killed.append(a[-1])
            rc = 0 if a[-1] in self.containers else 1
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
           "no_work": False, "subtype": "success", "terminal_reason": "completed"}


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
            recs.append({"type": "result", "subtype": sc["subtype"], "is_error": sc["is_error"],
                         "terminal_reason": sc["terminal_reason"],
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
        self.registration = self.tmp / "registration.json"
        self.registered = set()

    def load(self, task_id):
        return run.TaskInfo(image=f"sab-arm-{task_id}:t", prompt="Fix the bug.\n",
                            template=f"sab-tpl-{task_id}", grader=self.grader)

    def go(self, plan, budget, acks=(), **override):
        """Play the operator: write GO.json for `plan` and `budget` (a Budget), bound to the
        registration (which is made to register `plan` first)."""
        self.register(plan)
        body = {"cap_usd": budget.cap_usd, "per_call_usd": budget.per_call_usd,
                "reserve_usd": budget.reserve_usd, "spent_usd": budget.spent_usd,
                "plan_sha256": run.plan_sha256(plan),
                "registration_sha256": self.registration_sha256(),
                "authorized_by": "operator", "acknowledged_stops": list(acks)}
        body.update(override)
        (self.data / "GO.json").write_text(json.dumps(body) + "\n")

    def register(self, *plans):
        """Play the freeze: a registration that registers every plan given so far. Rewritten
        (a new registration_sha256) only when a plan is new."""
        hashes = {run.plan_sha256(p) for p in plans}
        if hashes <= self.registered:
            return
        self.registered |= hashes
        body = {"stage": "test", "plans": {f"p{i}": {"plan_sha256": h}
                                           for i, h in enumerate(sorted(self.registered))}}
        reg = dict(body, registration_sha256=run.registration_digest(body))
        self.registration.write_text(json.dumps(reg) + "\n")

    def registration_sha256(self):
        return json.loads(self.registration.read_text())["registration_sha256"]

    def ack_last_stop(self, plan, budget):
        """Play the operator after inspecting a stop: acknowledge the latched stop id."""
        latched = run.last_stop(run.read_events(self.data))
        self.go(plan, budget, acks=[latched[0]] if latched else [])
        return latched

    def spent(self, budget=None):
        return run.spent_usd(run.read_rows(self.data), budget or BUDGET,
                             run.read_events(self.data))

    def batch(self, plan, budget, cli=None, go=True, **kw):
        if go:
            self.go(plan, budget)
        else:
            self.register(plan)
        cli = cli or FakeCLI(self.world, self.data)
        kwargs = dict(docker=self.world, exec_cli=cli, load_task=self.load,
                      manifest=self.manifest, token_reader=lambda: TOKEN,
                      registration=self.registration)
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
    def assertRefused(self, plan, budget, pattern=""):
        cli = FakeCLI(self.world, self.data)
        with self.assertRaisesRegex(run.Refused, pattern):
            self.batch(plan, budget, cli=cli, go=False)
        self.assertEqual(cli.calls, [])
        self.assertEqual(self.world.calls, [])
        self.assertFalse((self.data / "rows.jsonl").exists())

    def test_refused_without_the_operator_file(self):
        self.assertRefused([spec()], BUDGET)

    def test_refused_on_cap_mismatch(self):
        plan = [spec()]
        self.go(plan, BUDGET._replace(cap_usd=30.0))
        self.assertRefused(plan, BUDGET)

    def test_refused_on_plan_hash_mismatch(self):
        self.register([spec(rep=1)], [spec()])  # both registered: GO.json alone decides
        self.go([spec(rep=1)], BUDGET)
        self.assertRefused([spec()], BUDGET, "authorizes plan")
    def test_refused_on_spent_mismatch(self):
        # Final review I-5: GO.json binds spent_usd (the spend outside this journal).
        plan = [spec()]
        self.go(plan, BUDGET._replace(spent_usd=10.0))
        self.assertRefused(plan, BUDGET, "spent_usd")
        self.go(plan, BUDGET, spent_usd=None)
        self.assertRefused(plan, BUDGET, "spent_usd")

    def test_refused_on_registration_mismatch(self):
        # Final review I-5 / T10b M6: GO.json names the registration the batch runs under.
        plan = [spec()]
        self.go(plan, BUDGET, registration_sha256="0" * 64)
        self.assertRefused(plan, BUDGET, "authorizes registration")
        self.go(plan, BUDGET, registration_sha256=None)
        self.assertRefused(plan, BUDGET, "authorizes registration")

    def test_refused_without_a_registration_or_with_a_tampered_one(self):
        plan = [spec()]
        self.go(plan, BUDGET)
        cli = FakeCLI(self.world, self.data)
        with self.assertRaisesRegex(run.Refused, "registration"):
            self.batch(plan, BUDGET, cli=cli, go=False, registration=None)
        with self.assertRaisesRegex(run.Refused, "missing"):
            self.batch(plan, BUDGET, cli=cli, go=False, registration=self.tmp / "nope.json")
        # An edit after the freeze breaks the registration's own digest.
        reg = json.loads(self.registration.read_text())
        reg["stage"] = "edited"
        self.registration.write_text(json.dumps(reg))
        with self.assertRaisesRegex(run.Refused, "does not match its content"):
            self.batch(plan, BUDGET, cli=cli, go=False)
        self.assertEqual(cli.calls, [])
        self.assertEqual(self.world.calls, [])

    def test_refused_when_the_registration_does_not_register_the_plan(self):
        self.register([spec(rep=1)])
        body = json.loads(self.registration.read_text())
        plan = [spec()]
        go = {"cap_usd": BUDGET.cap_usd, "per_call_usd": BUDGET.per_call_usd,
              "reserve_usd": BUDGET.reserve_usd, "spent_usd": BUDGET.spent_usd,
              "plan_sha256": run.plan_sha256(plan), "authorized_by": "operator",
              "registration_sha256": body["registration_sha256"]}
        (self.data / "GO.json").write_text(json.dumps(go))
        cli = FakeCLI(self.world, self.data)
        with self.assertRaisesRegex(run.Refused, "does not register"):
            run.run_batch(plan, BUDGET, self.data, dry_run=False, registration=self.registration,
                          docker=self.world, exec_cli=cli, load_task=self.load,
                          manifest=self.manifest, token_reader=lambda: TOKEN)
        self.assertEqual(self.world.calls, [])
        # Stage 1's form (a top-level plan_sha256) and the Stage 3 parts both register.
        self.assertEqual(run.registered_plan_hashes(
            {"plan_sha256": "a", "plans": {"pilot": {"plan_sha256": "b"},
                                           "extension": {"plan_sha256": "c"}}}), {"a", "b", "c"})

    def test_the_start_event_records_the_registration(self):
        self.batch([spec()], BUDGET)
        start = [e for e in self.events() if e["event"] == "start"][-1]
        self.assertEqual(start["registration_sha256"], self.registration_sha256())

    def test_refused_unless_authorized_by_the_operator(self):
        plan = [spec()]
        self.go(plan, BUDGET, authorized_by="controller")
        self.assertRefused(plan, BUDGET)

    def test_refused_on_a_symlinked_or_malformed_file(self):
        plan = [spec()]
        self.go(plan, BUDGET)
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
        # Behavioral (fix round 1, M4/M7): a full fake batch over all three arms leaves GO.json's
        # bytes and mtime as the operator wrote them and creates no other GO* file.
        plan = [spec(arm="bare"), spec(arm="cs"), spec(arm="full")]
        self.go(plan, BUDGET)
        go = self.data / "GO.json"
        before = (go.read_bytes(), go.stat().st_mtime_ns)
        self.batch(plan, BUDGET, go=False)
        self.assertEqual({r["outcome"] for r in self.latest().values()}, {"scored"})
        self.assertEqual((go.read_bytes(), go.stat().st_mtime_ns), before)
        self.assertEqual(sorted(p.name for p in self.data.rglob("*")
                                if p.name.upper().startswith("GO")), ["GO.json"])
        dry = self.tmp / "dry"
        run.run_batch(run.stage1_order(seed=7), BUDGET, dry, dry_run=True)
        self.assertEqual([p.name for p in dry.iterdir()], ["dry-run.json"])


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
                self.assertEqual(self.spent(), 1.25)
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
        self.assertEqual(self.spent(), 0.5)

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
                # The same plan again (after the operator acknowledged any stop): nothing re-runs.
                self.ack_last_stop(plan, BUDGET)
                self.batch(plan, BUDGET, cli=cli, go=False)
                self.assertEqual(len(cli.calls), 1)
                self.assertEqual(len(run.read_rows(self.data)), 2)  # pending + final
                self.assertIn("skip", [e["event"] for e in self.events()])

    def test_a_recovered_unknown_spend_run_is_not_retried(self):
        plan = [spec()]
        pending = run.make_row(plan[0], "s-1", outcome=run.PENDING)
        (self.data / "rows.jsonl").write_text(json.dumps(pending) + "\n")
        # the killed batch's journal: start and call, no stop (the operator acknowledges it)
        run._event(self.data, "start", budget=BUDGET._asdict())
        run._event(self.data, "call", runId=run.run_id(plan[0]))
        self.ack_last_stop(plan, BUDGET)
        cli = self.batch(plan, BUDGET, go=False)
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
        self.assertEqual(self.spent(), 1.25)
        self.assertEqual(self.world.runner_objects(), (set(), set(), set()))

    def test_pending_rows_left_by_a_killed_runner_become_unknown_spend_at_start(self):
        # A hard kill (SIGKILL) writes nothing after the pending row: simulate its journal.
        plan = [spec(rep=0), spec(rep=1)]
        pending = run.make_row(plan[0], "s-1", outcome=run.PENDING)
        (self.data / "rows.jsonl").write_text(json.dumps(pending) + "\n")
        budget = run.Budget(3.5, 0.0, 1.25, 1.0)
        run._event(self.data, "start", budget=budget._asdict())
        run._event(self.data, "call", runId=run.run_id(plan[0]))
        self.ack_last_stop(plan, budget)
        # Charged at per_call: 1.25 + 1.0 + 1.25 = 3.5 <= 3.5 admits rep 1 and nothing more.
        cli = self.batch(plan, budget, go=False)
        rows = self.latest()
        self.assertEqual(rows[run.run_id(plan[0])]["outcome"], "unknown-spend")
        self.assertIsNone(rows[run.run_id(plan[0])]["costUsd"])
        self.assertEqual(len(cli.calls), 1)
        self.assertEqual(cli.calls[0][1][cli.calls[0][1].index("--session-id") + 1] != "s-1", True)
        self.assertEqual(self.spent(budget), 1.25 + 0.5)


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
            self.go(plan, budget)
        else:
            self.register(plan)
        kwargs = dict(docker=spy, exec_cli=cli, load_task=self.load, manifest=self.manifest,
                      token_reader=lambda: TOKEN, registration=self.registration)
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
        fix_round_1 = ["stage"]  # M5: development rows are tagged and never `scored`
        final_review = ["capHit"]  # I-6: the result record says --max-budget-usd ended the run
        for row in rows:
            self.assertEqual(sorted(row), sorted(brief + r6 + fix_round_1 + final_review))
        self.assertEqual(rows[-1]["stage"], "main")
        final = rows[-1]
        self.assertEqual(final["outcome"], "scored")
        self.assertEqual(final["numTurns"], 3)
        self.assertEqual(final["inputTokens"], 115)   # input + cache creation + cache read
        self.assertEqual(final["outputTokens"], 20)
        self.assertEqual(final["effectiveEffort"], "medium")
        self.assertEqual(final["grade"], {"pass": True, "oom_killed": False})
        self.assertTrue(final["integrity"]["ok"])
        self.assertIn(final["outcome"], run.OUTCOMES)
class CapHitTest(RunnerCase):
    """Final review I-6: a run ended by --max-budget-usd is recorded per row (capHit)."""

    def test_a_run_ended_by_the_per_call_cap_is_scored_and_marked(self):
        hit = {"subtype": "error_max_budget_usd", "terminal_reason": "budget_exhausted",
               "is_error": True, "cost": 1.27}
        cli = FakeCLI(self.world, self.data, lambda arm, n: hit if n == 0 else {})
        plan = [spec(rep=0), spec(rep=1)]
        self.batch(plan, BUDGET, cli=cli)
        rows = self.latest()
        capped, normal = rows[run.run_id(plan[0])], rows[run.run_id(plan[1])]
        self.assertEqual(capped["outcome"], "scored")  # it did model work: scored, never dropped
        self.assertIs(capped["capHit"], True)
        self.assertEqual(capped["costUsd"], 1.27)
        self.assertIs(normal["capHit"], False)

    def test_no_result_record_is_unknown(self):
        cli = FakeCLI(self.world, self.data, lambda arm, n: {"timeout": True})
        self.batch([spec()], BUDGET, cli=cli)
        self.assertIsNone(self.latest()[run.run_id(spec())]["capHit"])

    def test_cap_hit_reads_the_result_record(self):
        self.assertIsNone(run.cap_hit(None))
        self.assertFalse(run.cap_hit({"subtype": "success", "terminal_reason": "completed"}))
        self.assertFalse(run.cap_hit({"subtype": "error_max_turns", "is_error": True}))
        self.assertTrue(run.cap_hit({"subtype": "error_max_budget_usd", "is_error": True}))
        self.assertTrue(run.cap_hit({"subtype": "error_during_execution",
                                     "terminal_reason": "budget_exhausted"}))


class Stage1RowsTest(RunnerCase):
    """Stage 1's rows were written before `capHit`; they stay readable in the one data dir."""

    def legacy(self, task):
        row = run.make_row((task, MODEL, "bare", 0), "s", outcome="dev-scored"
                           if run.stage_of(task) == "dev" else "scored",
                           costUsd=0.1, grade={"pass": True})
        return {k: v for k, v in row.items() if k != "capHit"}

    def test_a_stage1_row_without_cap_hit_reads_as_unknown(self):
        (self.data / "rows.jsonl").write_text(json.dumps(self.legacy("burntsushi__ripgrep-2576"))
                                              + "\n")
        rows = run.read_rows(self.data)
        self.assertEqual(len(rows), 1)
        self.assertIsNone(rows[0]["capHit"])
        self.assertEqual(sorted(rows[0]), sorted(run.ROW_FIELDS))

    def test_a_main_row_without_cap_hit_is_refused(self):
        (self.data / "rows.jsonl").write_text(json.dumps(self.legacy("t1")) + "\n")
        with self.assertRaisesRegex(ValueError, "not a runner row"):
            run.read_rows(self.data)

    def test_the_real_stage1_journal_still_reads(self):
        study = run.tasks.DATA / "study"
        if not (study / "rows.jsonl").is_file():
            self.skipTest("no Stage 1 journal on this machine")
        import analyze
        rows = run.read_rows(study)  # read only
        self.assertTrue(rows)
        self.assertTrue(all(r["stage"] == "dev" and r["capHit"] is None for r in rows))
        for r in rows:
            analyze.check_row(r)


class Stage3DryRunTest(unittest.TestCase):
    """Final review I-4: `dry-run --plan` and the Stage 3 reps and budget, at $0."""

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="sab-dry-"))
        self.addCleanup(shutil.rmtree, self.tmp, ignore_errors=True)

    def main(self, *argv):
        import contextlib
        import io
        with contextlib.redirect_stdout(io.StringIO()):
            run.main(list(argv))
        return json.loads((self.tmp / "d" / "dry-run.json").read_text())

    def test_dry_run_accepts_a_plan_file_with_the_stage3_reps(self):
        plan = run.make_order(["org__a-1", "org__a-2"], run.MODELS, 5)
        path = self.tmp / "plan.json"
        path.write_text(json.dumps([list(s) for s in plan]))
        out = self.main("dry-run", "--plan", str(path), "--stage3", "--data", str(self.tmp / "d"))
        self.assertEqual(out["count"], 16)
        self.assertEqual(out["reps"], "stage3")
        self.assertEqual(out["plan_sha256"], run.plan_sha256(plan))
        self.assertEqual(out["specs"], [list(s) for s in plan])
        self.assertEqual(out["budget"], {"cap_usd": 200.0, "spent_usd": 0.0,
                                         "per_call_usd": 5.0, "reserve_usd": 5.0})
        self.assertEqual(out["worst_case_usd"], 80.0)

    def test_dry_run_builds_a_stage3_order_from_tasks(self):
        out = self.main("dry-run", "--stage3", "--seed", "3", "--tasks", "org__a-1",
                        "--data", str(self.tmp / "d"))
        self.assertEqual((out["count"], out["reps"]), (8, "stage3"))
        self.assertEqual(out["plan_sha256"],
                         run.plan_sha256(run.make_order(["org__a-1"], run.MODELS, 3)))

    def test_the_stage1_default_is_unchanged(self):
        out = self.main("dry-run", "--seed", "7", "--data", str(self.tmp / "d"))
        self.assertEqual((out["count"], out["reps"]), (12, "stage1"))
        self.assertEqual(out["plan_sha256"], run.plan_sha256(run.stage1_order(7)))
        self.assertEqual((out["budget"]["cap_usd"], out["budget"]["per_call_usd"]), (25.0, 1.25))

    def test_dry_run_needs_a_plan_or_a_seed(self):
        import contextlib
        import io
        with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
            run.main(["dry-run", "--data", str(self.tmp / "d")])

    def test_plan_reps(self):
        self.assertEqual(run.plan_reps(run.make_order(["a"], run.MODELS, 1)), "stage3")
        self.assertEqual(run.plan_reps(run.stage1_order(1)), "stage1")
        self.assertIsNone(run.plan_reps(run.make_order(["a"], run.MODELS, 1)[:-1]))

    def test_stage3_budget_constants(self):
        self.assertEqual((run.STAGE3_CAP_USD, run.STAGE3_PER_CALL_USD, run.STAGE3_RESERVE_USD),
                         (200.0, 5.0, 5.0))
        run.check_budget(run.Budget(run.STAGE3_CAP_USD, 0.0, run.STAGE3_PER_CALL_USD,
                                    run.STAGE3_RESERVE_USD))


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


# --- fix round 1 ------------------------------------------------------------------------------

class RaisingGrader:
    def __init__(self, exc):
        self.exc, self.calls = exc, []

    def __call__(self, gdir):
        def grade(patch, name):
            self.calls.append(name)
            raise self.exc
        return grade


class PerCallChargeTest(RunnerCase):
    """I1a: an unknown cost is charged at the per_call of the call that spent it."""

    def test_unknown_spend_is_charged_at_the_per_call_of_its_own_call(self):
        b5 = run.Budget(25.0, 0.0, 5.0, 1.0)
        cli = FakeCLI(self.world, self.data, lambda arm, n: {"raise": KeyboardInterrupt()})
        with self.assertRaises(KeyboardInterrupt):
            self.batch([spec()], b5, cli=cli)
        self.assertEqual(self.latest()[run.run_id(spec())]["outcome"], "unknown-spend")
        b1 = run.Budget(25.0, 0.0, 1.25, 1.0)
        plan = [spec(), spec(rep=1)]
        self.ack_last_stop(plan, b1)
        cli2 = FakeCLI(self.world, self.data)
        self.batch(plan, b1, cli=cli2, go=False)
        self.assertEqual(len(cli2.calls), 1)
        self.assertEqual(self.spent(b1), 5.0 + 0.5)  # not 1.25 + 0.5

    def test_charge_lookup_and_fallback(self):
        ev = []
        def add(event, **f):
            ev.append({"event": event, "seq": len(ev) + 1, **f})
        add("start", budget={"per_call_usd": 5.0})
        add("call", runId="a")
        add("stop", reason="error", stopId="stop-3")
        add("start", budget={"per_call_usd": 2.0})
        add("call", runId="b")
        rows = [dict(run.make_row(spec(rep=i), "s", outcome="unknown-spend"), runId=r)
                for i, r in enumerate(("a", "b", "c"))]
        budget = run.Budget(25.0, 0.0, 1.25, 1.0)
        # a at its batch's 5, b at its batch's 2, c (no call event) at the max over starts: 5.
        self.assertEqual(run.spent_usd(rows, budget, ev), 5.0 + 2.0 + 5.0)
        self.assertEqual(run.spent_usd(rows, budget, []), 3 * 1.25)


class GoBindsPerCallTest(RunnerCase):
    """I1b: GO.json binds per_call_usd and reserve_usd too."""

    def test_per_call_or_reserve_mismatch_is_refused(self):
        plan = [spec()]
        for field, value in (("per_call_usd", 2.0), ("reserve_usd", 0.5), ("per_call_usd", None)):
            with self.subTest(field=field, value=value):
                self.go(plan, BUDGET, **{field: value})
                cli = FakeCLI(self.world, self.data)
                with self.assertRaisesRegex(run.Refused, field):
                    self.batch(plan, BUDGET, cli=cli, go=False)
                self.assertEqual(cli.calls, [])
                self.assertEqual(self.world.calls, [])


class LatchTest(RunnerCase):
    """I3: every stop other than `plan complete` holds until the operator acknowledges it."""

    def kinds(self):
        no_work = {"is_error": True, "cost": 0, "duration_api_ms": 0, "text": "Not logged in",
                   "no_work": True}
        return {
            "plan-limit": dict(script=lambda a, n: {"text": "You've hit your limit",
                                                    "is_error": True}),
            "missing cost": dict(script=lambda a, n: {"timeout": True}),
            "no model work": dict(script=lambda a, n: no_work),
            "effort mismatch": dict(script=lambda a, n: {"effort": "high" if a == "cs"
                                                         else "medium"}),
            "grader error": dict(grader=RaisingGrader(RuntimeError("grader broke"))),
            "grader infra": dict(grader=Grader({"pass": False, "infra": "timeout"})),
            "admission": dict(budget=run.Budget(2.0, 0.0, 1.25, 1.0)),
            "error: RuntimeError: boom": dict(script=lambda a, n: {"raise": RuntimeError("boom")},
                                              raises=RuntimeError),
        }

    def test_restart_after_each_stop_kind_is_refused_until_acknowledged(self):
        for reason, k in self.kinds().items():
            with self.subTest(stop=reason):
                self.setUp()
                plan = [spec(arm="bare"), spec(arm="cs"), spec(rep=1)]
                budget = k.get("budget", BUDGET)
                if "grader" in k:
                    self.grader = k["grader"]
                cli = FakeCLI(self.world, self.data, k.get("script"))
                if k.get("raises"):
                    with self.assertRaises(k["raises"]):
                        self.batch(plan, budget, cli=cli)
                else:
                    self.batch(plan, budget, cli=cli)
                stop = self.events()[-1]
                self.assertEqual(stop["event"], "stop")
                self.assertTrue(stop["reason"].startswith(reason), stop["reason"])
                calls = len(cli.calls)
                with self.assertRaisesRegex(run.Refused, stop["stopId"]):
                    self.batch(plan, budget, cli=cli)  # same command, GO without the ack
                self.assertEqual(len(cli.calls), calls)
                self.assertEqual(self.events()[-1], stop)  # refused before writing anything
                self.grader = Grader()
                self.ack_last_stop(plan, budget)
                self.batch(plan, budget, cli=FakeCLI(self.world, self.data), go=False)
                start = [e for e in self.events() if e["event"] == "start"][-1]
                self.assertEqual(start["acknowledged"], stop["stopId"])

    def test_a_batch_that_never_wrote_its_stop_is_latched(self):
        run._event(self.data, "start", budget=BUDGET._asdict())  # then SIGKILL: no stop event
        with self.assertRaisesRegex(run.Refused, "unterminated-1-[0-9a-f]{8}"):
            self.batch([spec()], BUDGET)
        latched = run.last_stop(run.read_events(self.data))
        self.go([spec()], BUDGET, acks=[latched[0]])
        self.batch([spec()], BUDGET, go=False)
        self.assertEqual(self.latest()[run.run_id(spec())]["outcome"], "scored")

    def test_a_completed_batch_does_not_latch(self):
        self.batch([spec()], BUDGET)
        self.assertEqual(self.events()[-1]["reason"], "plan complete")
        self.batch([spec(), spec(rep=1)], BUDGET)
        self.assertEqual(self.events()[-1]["reason"], "plan complete")


class TeardownTest(RunnerCase):
    """I2: the final row and its cost survive a teardown failure or an interrupt in grading."""

    def test_teardown_failure_after_the_cli_keeps_the_row_and_its_cost(self):
        self.world.fail_rm.add(run.AGENT_PREFIX + run._key(run.run_id(spec())))
        with self.assertRaises(RuntimeError):
            self.batch([spec()], BUDGET)
        row = self.latest()[run.run_id(spec())]
        self.assertEqual(row["outcome"], "scored")
        self.assertEqual(row["costUsd"], 0.5)
        record = json.loads((self.data / "runs" / run.run_id(spec()) / "run-record.json")
                            .read_text())
        self.assertTrue(record["teardownErrors"])
        self.assertTrue(self.events()[-1]["reason"].startswith("error:"))

    def test_an_interrupt_during_grading_still_writes_the_row(self):
        self.grader = RaisingGrader(KeyboardInterrupt())
        with self.assertRaises(KeyboardInterrupt):
            self.batch([spec(), spec(rep=1)], BUDGET)
        row = self.latest()[run.run_id(spec())]
        self.assertEqual(row["outcome"], "infra-other")
        self.assertEqual(row["costUsd"], 0.5)
        self.assertEqual(len(self.latest()), 1)
        self.assertEqual(self.world.runner_objects(), (set(), set(), set()))


class GraderSignalTest(RunnerCase):
    """I4: grader infra, anomalies and exceptions are infra and stop the batch."""

    def test_grade_infra_is_infra_other_and_stops(self):
        self.grader = Grader({"pass": False, "infra": "incomplete", "build": "not_run"})
        self.batch([spec(), spec(rep=1)], BUDGET)
        row = self.latest()[run.run_id(spec())]
        self.assertEqual(row["outcome"], "infra-other")
        self.assertIsNone(row["grade"])
        self.assertEqual(len(self.latest()), 1)
        self.assertEqual(self.events()[-1]["reason"], "grader infra")

    def test_a_grader_exception_stops_with_grader_error(self):
        self.grader = RaisingGrader(RuntimeError("docker run of the grader failed"))
        self.batch([spec(), spec(rep=1)], BUDGET)
        row = self.latest()[run.run_id(spec())]
        self.assertEqual((row["outcome"], row["costUsd"]), ("infra-other", 0.5))
        self.assertEqual(len(self.latest()), 1)
        self.assertEqual(self.events()[-1]["reason"], "grader error")

    def test_grade_outcome_signals(self):
        ok = {"infra": None, "graft": "ok", "build": "ok", "pass": True,
              "f2p": {"passed": ["t"], "failed": [], "notRun": []},
              "p2p": {"passed": [], "failed": [], "notRun": []}}
        self.assertEqual(run.grade_outcome(ok), ("scored", None, None))
        self.assertEqual(run.grade_outcome({"pass": False, "failure_kind": "test_failure"}),
                         ("scored", None, None))
        cases = {
            "grader anomaly": [{"pass": False, "error": "missing required tests: ['t']"},
                               dict(ok, build="failed"),            # checked_pass contradiction
                               {"pass": None}, "not a dict"],
            "grader infra": [{"pass": False, "infra": "rm_failed"},
                             {"pass": False, "oom_killed": True}],
        }
        for stop, grades in cases.items():
            for g in grades:
                with self.subTest(grade=g):
                    outcome, got, _ = run.grade_outcome(g)
                    self.assertEqual(got, stop)
                    self.assertIn(outcome, ("infra-other", "infra-oom"))


class FailingKillWorld(World):
    """A daemon whose `docker kill` always fails (exit 1)."""

    def __call__(self, *args, **kw):
        res = super().__call__(*args, **kw)
        if args and str(args[0]) == "kill":
            return SimpleNamespace(returncode=1, stdout="")
        return res


class KillTest(RunnerCase):
    """M1: a timed-out or OOM-hit agent container is killed before usage is read."""

    def run_watching_usage(self, script):
        seen = []
        real = run.run_usage

        def watched(config, session):
            seen.append(list(self.world.killed))
            return real(config, session)
        with mock.patch.object(run, "run_usage", watched):
            self.batch([spec()], BUDGET, cli=FakeCLI(self.world, self.data, script))
        return seen

    def test_a_timed_out_cli_is_killed_before_usage_is_read(self):
        name = run.AGENT_PREFIX + run._key(run.run_id(spec()))
        self.assertEqual(self.run_watching_usage(lambda a, n: {"timeout": True}), [[name]])
        record = json.loads((self.data / "runs" / run.run_id(spec()) / "run-record.json")
                            .read_text())
        self.assertEqual(record["killed"], "timeout")
        self.assertEqual(record["killedRc"], 0)  # the `docker kill` return code is recorded

    def test_the_kill_return_code_is_recorded_even_when_the_kill_fails(self):
        self.world = FailingKillWorld()
        self.batch([spec()], BUDGET, cli=FakeCLI(self.world, self.data,
                                                 lambda a, n: {"timeout": True}))
        record = json.loads((self.data / "runs" / run.run_id(spec()) / "run-record.json")
                            .read_text())
        self.assertEqual((record["killed"], record["killedRc"]), ("timeout", 1))

    def test_a_run_that_is_not_killed_records_no_kill_rc(self):
        self.batch([spec()], BUDGET)
        record = json.loads((self.data / "runs" / run.run_id(spec()) / "run-record.json")
                            .read_text())
        self.assertEqual((record["killed"], record["killedRc"]), (None, None))

    def test_an_oom_container_is_killed_before_usage_is_read(self):
        name = run.AGENT_PREFIX + run._key(run.run_id(spec()))
        self.assertEqual(self.run_watching_usage(lambda a, n: {"oom": True}), [[name]])

    def test_a_normal_run_is_not_killed(self):
        self.assertEqual(self.run_watching_usage(None), [[]])


class StageTest(RunnerCase):
    """M5: development-task rows carry stage "dev" and are `dev-scored`, never `scored`."""

    def test_development_rows_are_tagged_and_never_scored(self):
        dev = spec("astral-sh__ruff-15309")
        self.batch([dev, spec("t1")], BUDGET)
        rows = self.latest()
        self.assertEqual((rows[run.run_id(dev)]["stage"], rows[run.run_id(dev)]["outcome"]),
                         ("dev", "dev-scored"))
        self.assertIsNotNone(rows[run.run_id(dev)]["grade"])
        self.assertEqual((rows[run.run_id(spec("t1"))]["stage"],
                          rows[run.run_id(spec("t1"))]["outcome"]), ("main", "scored"))
        with self.assertRaises(ValueError):
            run.make_row(dev, "s", outcome="scored")
        self.assertTrue(set(run.STAGE1_DEV_TASKS) <= set(run.DEVELOPMENT_TASKS))
        self.assertTrue(all(s.task_id in run.DEVELOPMENT_TASKS for s in run.stage1_order(1)))
        # Effort equality still applies to development rows (Stage 1 runs the gate live).
        self.assertIn(f"astral-sh__ruff-15309|{MODEL}",
                      json.loads((self.data / "task-validity.json").read_text()))


# --- task 10a: pre-go hardening ----------------------------------------------------------------

class JournalTrustTest(RunnerCase):
    """batch.jsonl is the one point of trust for the latch and the per-call charge: rows.jsonl
    without a matching batch.jsonl is refused before any side effect."""

    def seed_row(self, sp=None, outcome="scored", cost=0.4):
        run._append(self.data / run.ROWS, run.make_row(sp or spec(), "s", outcome=outcome,
                                                        costUsd=cost))

    def refused(self, pattern):
        cli = FakeCLI(self.world, self.data)
        before = sorted(p.name for p in self.data.iterdir())
        with self.assertRaisesRegex(run.Refused, pattern):
            self.batch([spec(rep=1)], BUDGET, cli=cli)
        self.assertEqual(cli.calls, [])
        self.assertEqual(self.world.calls, [])
        after = sorted(p.name for p in self.data.iterdir())
        self.assertEqual(after, sorted(set(before) | {"GO.json"}))

    def test_rows_without_a_batch_journal_are_refused(self):
        self.seed_row()
        self.refused("batch.jsonl")

    def test_rows_with_a_journal_that_has_no_start_are_refused(self):
        self.seed_row()
        run._event(self.data, "call", runId=run.run_id(spec()))
        self.refused("start")

    def test_a_graded_row_without_a_call_event_is_refused(self):
        run._event(self.data, "start", plan_sha256="x", budget=BUDGET._asdict())
        run._event(self.data, "stop", reason=run.PLAN_COMPLETE, spent=0)
        self.seed_row()
        self.refused(run.run_id(spec()))

    def test_a_row_with_a_call_event_is_accepted(self):
        self.batch([spec()], BUDGET)
        self.batch([spec(), spec(rep=1)], BUDGET)
        self.assertEqual(self.latest()[run.run_id(spec(rep=1))]["outcome"], "scored")

    def test_a_pending_row_needs_no_call_event(self):
        run._event(self.data, "start", plan_sha256="x", budget=BUDGET._asdict())
        run._event(self.data, "stop", reason=run.PLAN_COMPLETE, spent=0)
        self.seed_row(outcome=run.PENDING, cost=None)
        self.batch([spec(rep=1)], BUDGET)  # recovered as unknown-spend, then runs

    def test_an_old_format_journal_line_is_refused_not_a_keyerror(self):
        (self.data / "batch.jsonl").write_text(json.dumps({"event": "start", "budget": {}}) + "\n")
        with self.assertRaisesRegex(run.Refused, "seq"):
            run.read_events(self.data)
        self.go([spec()], BUDGET)
        with self.assertRaisesRegex(run.Refused, "seq"):
            self.batch([spec()], BUDGET, go=False)

    def test_an_old_format_stop_without_a_stop_id_is_refused(self):
        (self.data / "batch.jsonl").write_text(
            json.dumps({"event": "stop", "seq": 1, "reason": "error"}) + "\n")
        with self.assertRaisesRegex(run.Refused, "stopId"):
            run.read_events(self.data)


class StopIdTest(RunnerCase):
    def test_stop_ids_are_unique_even_at_the_same_seq(self):
        other = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, other, ignore_errors=True)
        a = run._event(self.data, "stop", reason="error")
        b = run._event(other, "stop", reason="error")
        self.assertEqual(a["seq"], b["seq"])
        for e in (a, b):
            self.assertRegex(e["stopId"], r"^stop-1-[0-9a-f]{8}$")
        self.assertNotEqual(a["stopId"], b["stopId"])
        self.assertEqual(run.read_events(self.data)[0]["stopId"], a["stopId"])  # stored, stable

    def test_a_truncated_journal_cannot_reuse_a_stale_acknowledged_id(self):
        plan = [spec()]
        cli = FakeCLI(self.world, self.data, lambda a, n: {"raise": RuntimeError("boom")})
        with self.assertRaises(RuntimeError):
            self.batch(plan, BUDGET, cli=cli)
        stale = run.last_stop(run.read_events(self.data))[0]
        # the operator restored an older journal; the next stop lands on the same seq
        (self.data / "batch.jsonl").unlink()
        (self.data / run.ROWS).unlink()
        shutil.rmtree(self.data / "runs", ignore_errors=True)
        self.world = World()
        self.go(plan, BUDGET, acks=[stale])
        with self.assertRaises(RuntimeError):
            self.batch(plan, BUDGET, cli=FakeCLI(self.world, self.data,
                                                 lambda a, n: {"raise": RuntimeError("boom")}),
                       go=False)
        fresh = run.last_stop(run.read_events(self.data))[0]
        self.assertNotEqual(stale, fresh)
        with self.assertRaisesRegex(run.Refused, fresh):
            self.batch(plan, BUDGET, go=False)  # GO.json still lists only the stale id

    def test_the_unterminated_id_is_deterministic_and_names_its_start_event(self):
        run._event(self.data, "start", budget=BUDGET._asdict())
        first = run.last_stop(run.read_events(self.data))[0]
        self.assertRegex(first, r"^unterminated-1-[0-9a-f]{8}$")
        self.assertEqual(first, run.last_stop(run.read_events(self.data))[0])
        other = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, other, ignore_errors=True)
        e = run._event(other, "start", budget=dict(BUDGET._asdict(), cap_usd=99.0))
        self.assertNotEqual(first, run.last_stop([e])[0])


class RequiredArgsTest(unittest.TestCase):
    def test_spent_usd_requires_the_events(self):
        with self.assertRaises(TypeError):
            run.spent_usd([], BUDGET)

    def test_development_tasks_equal_the_selection_rules_excluded_ids(self):
        rule = json.loads((run.HERE / "selection-rule.json").read_text())
        self.assertEqual(sorted(run.DEVELOPMENT_TASKS), sorted(rule["exclude_ids"]))


if __name__ == "__main__":
    unittest.main()
