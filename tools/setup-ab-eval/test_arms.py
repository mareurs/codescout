"""Tests for arms.py: the three arms differ only in CLI flags and mounts."""
import ast
import json
import subprocess
import tempfile
import tomllib
import unittest
from pathlib import Path

import arms

HERE = Path(__file__).resolve().parent
SESSION = "00000000-0000-4000-8000-000000000000"
MODEL = "claude-sonnet-5-5"
PROMPT = "Fix the bug.\n"


def codex_args():
    """The `args = [...]` literal of pilot.run_pilot, read from the provenance-pinned source.

    Non-literal elements come back as placeholders named after their source text, so the test
    compares against the Codex list itself rather than a retyped copy of it.
    """
    tree = ast.parse((HERE / "pilot.py").read_text())
    run_pilot = next(n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name == "run_pilot")
    for node in ast.walk(run_pilot):
        if (isinstance(node, ast.Assign) and len(node.targets) == 1
                and isinstance(node.targets[0], ast.Name) and node.targets[0].id == "args"):
            return [e.value if isinstance(e, ast.Constant) else ("<expr>", ast.unparse(e))
                    for e in node.value.elts]
    raise AssertionError("pilot.run_pilot has no `args = [...]` literal")


def pairs(args):
    """(flag, value) pairs before `--`; bare switches pair with None."""
    head = args[:args.index("--")]
    out, i = [], 0
    while i < len(head):
        if i + 1 < len(head) and not head[i + 1].startswith("--"):
            out.append((head[i], head[i + 1]))
            i += 2
        else:
            out.append((head[i], None))
            i += 1
    return out
def targets(spec):
    return {(m[1], m[2]) for m in spec.mounts}


def host_of(spec, target):
    (host,) = [m[0] for m in spec.mounts if m[1] == target]
    return host


class ArmsTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        root = Path(self.tmp.name)
        self.plugins = {}
        for name in ("codescout-companion", "superpowers", "buddy"):
            p = root / "plugins" / name
            p.mkdir(parents=True)
            self.plugins[name] = p
        profile = root / "profile"
        profile.mkdir()
        (profile / "CLAUDE.md").write_text("# frozen user CLAUDE.md\n")
        # The shapes of the operator's real keys (measured in ~/.claude/settings.json).
        self.user_settings = {
            "outputStyle": "legible:Explanatory STE",
            "enabledPlugins": {"superpowers@superpowers-marketplace": True},
            "permissions": {"allow": ["Read(/home/marius/.claude/plugins/**)", "Bash(ls:*)",
                                      "Bash(git clone https://github.com/apache/x.git)",
                                      "WebFetch(domain:127.0.0.1)"]},
            "env": {"ANTHROPIC_BASE_URL": "http://localhost:8082",
                    "CODESCOUT_QDRANT_URL": "http://127.0.0.1:6334", "TMPDIR": "/tmp"},
            "hooks": {"UserPromptSubmit": [{"hooks": [
                {"type": "command", "command": "node /home/marius/work/x/user-prompt.mjs"}]}],
                "Stop": [{"hooks": [{"type": "command", "command": "true"}]}]},
            "statusLine": {"type": "command", "command": "bash /home/marius/statusline.sh"},
            "extraKnownMarketplaces": {"m": {"source": {"source": "directory",
                                                        "path": "/home/marius/work/m"}}},
            "autoMode": {"environment": ["dev Langfuse at `http://localhost:3000`"]},
            "effortLevel": "medium",
            "modelSettings": {"claude-sonnet-5-5": {"effortLevel": "medium"}},
            "theme": "dark",
            "verbose": True,
            "autoMemoryEnabled": False,
            "fileCheckpointingEnabled": False,
            "lanService": {"endpoint": "http://192.168.1.20:3000"},
            "skillOverrides": {"vet-ad-offer": "user-invocable-only"},
        }
        (profile / "settings.json").write_text(json.dumps(self.user_settings) + "\n")
        cli = root / "claude-cli"
        cli.write_text("binary")
        self.manifest = {
            "cli": {"path": str(cli), "version": "2.1.309 (Claude Code)", "sha256": "0" * 64},
            "mcp_env": {
                "CODESCOUT_QDRANT_URL": "http://127.0.0.1:6334",
                "CODESCOUT_EMBEDDER_URL": "http://127.0.0.1:48081",
                "CODESCOUT_EMBEDDER_PROTOCOL": "llama-server",
                "CODESCOUT_EMBEDDER_MODEL_NAME": "CodeRankEmbed-Q4_K_M.gguf",
                "CODESCOUT_QUERY_PREFIX": "Represent this query for searching relevant code: ",
                "CODESCOUT_SPARSE_EMBEDDER_URL": "http://127.0.0.1:48084",
                "CODESCOUT_RERANKER_URL": "http://127.0.0.1:48083",
                "CODESCOUT_RERANKER_PROTOCOL": "llama-server",
                "CODESCOUT_MODEL_DIM": "768",
                "CODESCOUT_BM25_BOOST": "3.0",
            },
            "plugins_cs": {"install_path": str(self.plugins["codescout-companion"]),
                           "version": "1.20.21"},
            "plugins_full": [
                {"name": "superpowers", "install_path": str(self.plugins["superpowers"])},
                {"name": "codescout-companion",
                 "install_path": str(self.plugins["codescout-companion"])},
                {"name": "buddy", "install_path": str(self.plugins["buddy"])},
            ],
            "full_config_files": [str(profile / "CLAUDE.md"), str(profile / "settings.json")],
        }
        # The shape stack_up returns, with the per-run Qdrant URL from qdrant_for_run.
        self.urls = {
            "CODESCOUT_QDRANT_URL": "http://sab-qdrant-r1:6334",
            "CODESCOUT_EMBEDDER_URL": "http://sab-dense:8080",
            "CODESCOUT_EMBED_URL": "http://sab-dense:8080/v1",
            "LIBRARIAN_EMBED_URL": "http://sab-dense:8080/v1",
            "CODESCOUT_SPARSE_EMBEDDER_URL": "http://sab-sparse:80",
        }
        self.root = root

    def tearDown(self):
        self.tmp.cleanup()

    def spec(self, arm, manifest=None, urls=None):
        run_dir = self.root / "runs" / arm
        return arms.arm_invocation(arm, MODEL, PROMPT, manifest or self.manifest,
                                   self.urls if urls is None else urls, run_dir,
                                   session_id=SESSION)

    def test_bare_args_equal_codex_args_without_effort(self):
        expected = []
        source = codex_args()
        i = 0
        while i < len(source):
            if source[i] == "--effort":
                i += 2  # drop the flag and its value
                continue
            expected.append(source[i])
            i += 1
        # Resolve the three non-literal elements of the Codex list.
        expected[expected.index("claude-sonnet-5-5")] = MODEL
        session = expected.index(("<expr>", "session"))
        expected[session] = SESSION
        self.assertEqual(expected[-1][0], "<expr>")  # the task prompt, read from task.txt
        expected[-1] = PROMPT
        self.assertEqual(self.spec("bare").cli_args, expected)

    def test_bare_takes_the_model_argument(self):
        spec = arms.arm_invocation("bare", "claude-opus-5-5", PROMPT, self.manifest, self.urls,
                                   self.root / "runs" / "o", session_id=SESSION)
        self.assertEqual(spec.cli_args[spec.cli_args.index("--model") + 1], "claude-opus-5-5")

    def test_no_arm_passes_effort(self):
        for arm in arms.ARMS:
            args = self.spec(arm).cli_args
            self.assertFalse([a for a in args[:args.index("--")] if a.startswith("--effort")],
                             arm)

    def test_cs_is_bare_plus_mcp_and_companion_only(self):
        bare, cs = self.spec("bare"), self.spec("cs")
        added = [p for p in pairs(cs.cli_args) if p not in pairs(bare.cli_args)]
        self.assertEqual(added, [("--mcp-config", "/opt/sab/mcp.json"),
                                 ("--plugin-dir", "/opt/plugins/codescout-companion")])
        # Removing the added pairs gives the bare list back, order included.
        stripped = [p for p in pairs(cs.cli_args) if p not in added]
        self.assertEqual(stripped, pairs(bare.cli_args))
        self.assertEqual(cs.cli_args[cs.cli_args.index("--"):],
                         bare.cli_args[bare.cli_args.index("--"):])
        self.assertEqual(cs.env, bare.env)
        # Run-dir host paths differ per run; compare mounts by container target and mode, and
        # check every manifest-sourced host path as it is.
        self.assertEqual(targets(cs) - targets(bare),
                         {("/opt/sab/mcp.json", "ro"),
                          ("/opt/plugins/codescout-companion", "ro")})
        self.assertTrue(targets(bare) <= targets(cs))
        self.assertEqual(host_of(cs, "/opt/claude"), self.manifest["cli"]["path"])
        self.assertEqual(host_of(bare, "/opt/claude"), self.manifest["cli"]["path"])
        for spec in (bare, cs):
            self.assertEqual(Path(host_of(spec, "/opt/settings.json")).read_text(), "{}\n")
        companion = next(m for m in cs.mounts if m[1] == "/opt/plugins/codescout-companion")
        self.assertEqual(companion[0], self.manifest["plugins_cs"]["install_path"])
        mcp = json.loads(Path(next(m[0] for m in cs.mounts if m[1] == "/opt/sab/mcp.json"))
                         .read_text())
        self.assertEqual(list(mcp["mcpServers"]), ["codescout"])
        server = mcp["mcpServers"]["codescout"]
        self.assertEqual([server["command"], *server["args"]],
                         [arms.CODESCOUT_BIN, "start", "--debug"])
        # bare: no MCP config and no plugin anywhere.
        self.assertNotIn("--mcp-config", bare.cli_args)
        self.assertNotIn("--plugin-dir", bare.cli_args)

    def test_full_is_superset_of_cs(self):
        cs, full = self.spec("cs"), self.spec("full")
        # C2 ruling: full alone loads the user setting source; every other pair is kept.
        swap = {("--setting-sources", "local"): ("--setting-sources", "user,local")}
        self.assertTrue({swap.get(p, p) for p in pairs(cs.cli_args)} <= set(pairs(full.cli_args)))
        added = [p for p in pairs(full.cli_args) if swap.get(p, p) not in
                 [swap.get(q, q) for q in pairs(cs.cli_args)]]
        # One --plugin-dir per plugins_full entry; the companion is already in cs, not twice.
        self.assertEqual(added, [("--plugin-dir", "/opt/plugins/superpowers"),
                                 ("--plugin-dir", "/opt/plugins/buddy")])
        self.assertEqual(full.cli_args.count("/opt/plugins/codescout-companion"), 1)
        self.assertEqual(full.env, cs.env)
        self.assertTrue(targets(cs) <= targets(full))
        self.assertEqual(targets(full) - targets(cs),
                         {("/opt/plugins/superpowers", "ro"), ("/opt/plugins/buddy", "ro")})
        self.assertEqual(host_of(full, "/opt/plugins/superpowers"),
                         str(self.plugins["superpowers"]))
        self.assertEqual(host_of(full, "/opt/plugins/buddy"), str(self.plugins["buddy"]))
        self.assertEqual(host_of(full, "/opt/plugins/codescout-companion"),
                         host_of(cs, "/opt/plugins/codescout-companion"))
        self.assertEqual(Path(host_of(full, "/opt/sab/mcp.json")).read_bytes(),
                         Path(host_of(cs, "/opt/sab/mcp.json")).read_bytes())
        config = Path(host_of(full, arms.CONFIG_DIR))
        claude_md = self.manifest["full_config_files"][0]
        self.assertEqual((config / "CLAUDE.md").read_bytes(), Path(claude_md).read_bytes())
        self.assertEqual(sorted(p.name for p in config.iterdir()),
                         [".claude.json", "CLAUDE.md", "settings.json"])
        cs_config = Path(host_of(cs, arms.CONFIG_DIR))
        self.assertEqual(sorted(p.name for p in cs_config.iterdir()), [".claude.json"])

    def test_cs_and_full_seed_claude_json_with_the_same_server(self):
        # C1 ruling: the companion's detect.mjs finds codescout only in the config dir's
        # .claude.json (or the repo), never through --mcp-config.
        for arm in ("cs", "full"):
            spec = self.spec(arm)
            mcp = json.loads(Path(host_of(spec, "/opt/sab/mcp.json")).read_text())
            seeded = json.loads((Path(host_of(spec, arms.CONFIG_DIR)) / ".claude.json")
                                .read_text())
            self.assertEqual(seeded, {"mcpServers": mcp["mcpServers"]}, arm)
            self.assertEqual(list(seeded["mcpServers"]), ["codescout"])
        bare = self.spec("bare")
        self.assertEqual(list(Path(host_of(bare, arms.CONFIG_DIR)).iterdir()), [])

    def test_full_settings_drop_env_hooks_statusline_and_host_values(self):
        full = self.spec("full")
        settings = json.loads((Path(host_of(full, arms.CONFIG_DIR)) / "settings.json")
                              .read_text())
        # Kept by name, whatever their values hold (C2 ruling).
        for key in ("outputStyle", "enabledPlugins", "permissions"):
            self.assertEqual(settings[key], self.user_settings[key], key)
        # Kept because no host path or URL is in the value.
        self.assertEqual(settings["theme"], "dark")
        self.assertEqual(settings["skillOverrides"], self.user_settings["skillOverrides"])
        # The host-path hook is dropped; a hook without one survives.
        self.assertEqual(settings["hooks"], {"Stop": self.user_settings["hooks"]["Stop"]})
        for key in ("env", "statusLine", "extraKnownMarketplaces", "autoMode",
                    "effortLevel", "modelSettings", "verbose"):
            self.assertNotIn(key, settings, key)
        rest = {k: v for k, v in settings.items() if k not in arms.USER_SETTINGS_KEPT_BY_NAME}
        for marker in ("/home/", "127.0.0.1", "localhost"):
            self.assertNotIn(marker, json.dumps(rest))
        # Every dropped key is recorded with a reason in the run's arm record.
        record = json.loads((self.root / "runs" / "full" / "arm-record.json").read_text())
        self.assertEqual(record["arm"], "full")
        self.assertEqual(sorted(record["settings_dropped"]),
                         ["autoMode", "effortLevel", "env", "extraKnownMarketplaces",
                          "hooks.UserPromptSubmit", "lanService", "modelSettings", "statusLine",
                          "verbose"])
        self.assertTrue(all(record["settings_dropped"].values()))
        # The same function the record came from is importable for the manifest.
        kept, dropped = arms.sanitize_user_settings(self.user_settings)
        self.assertEqual(kept, settings)
        self.assertEqual(dropped, record["settings_dropped"])
        # env, statusLine and the effort keys go by name, even with no host value in them.
        kept, dropped = arms.sanitize_user_settings(
            {"env": {"FOO": "1"}, "statusLine": {"type": "command", "command": "true"},
             "effortLevel": "medium", "modelSettings": {}, "verbose": True, "theme": "dark"})
        self.assertEqual(kept, {"theme": "dark"})
        self.assertEqual(dropped, {k: arms.USER_SETTINGS_DROPPED[k] for k in
                                   ("env", "statusLine", "effortLevel", "modelSettings",
                                    "verbose")})
        # Round-3 ruling: harness-behaviour keys (effort, verbose) are pinned as dropped.
        for key in ("effortLevel", "modelSettings", "verbose"):
            self.assertIn(key, arms.USER_SETTINGS_DROPPED)
            self.assertTrue(arms.USER_SETTINGS_DROPPED[key])

    def test_only_full_uses_user_setting_source(self):
        values = {arm: self.spec(arm).cli_args for arm in arms.ARMS}
        source = {arm: a[a.index("--setting-sources") + 1] for arm, a in values.items()}
        self.assertEqual(source, {"bare": "local", "cs": "local", "full": "user,local"})
        for arm, args in values.items():
            self.assertEqual(args.count("--setting-sources"), 1, arm)
            record = json.loads((self.root / "runs" / arm / "arm-record.json").read_text())
            self.assertEqual(record["setting_sources"], source[arm])
            if arm != "full":
                self.assertEqual(record["settings_dropped"], {})
    def test_only_study_hosts_pass_the_allowlist(self):
        # Minor 2: every URL host must start with sab-; a denylist would miss each of these.
        bad = ["http://192.168.1.20:8080", "http://172.17.0.1:6334", "http://[::1]:6334",
               "http://qdrant:6334", "localhost:8082"]
        for n, value in enumerate(bad):
            leftover = dict(self.manifest, mcp_env=dict(self.manifest["mcp_env"],
                                                        CODESCOUT_EXTRA_URL=value))
            with self.assertRaises(ValueError, msg=value):
                arms.arm_invocation("cs", MODEL, PROMPT, leftover, self.urls,
                                    self.root / "runs" / f"left{n}", session_id=SESSION)
            with self.assertRaises(ValueError, msg=value):
                arms.arm_invocation("cs", MODEL, PROMPT, self.manifest,
                                    dict(self.urls, CODESCOUT_QDRANT_URL=value),
                                    self.root / "runs" / f"study{n}", session_id=SESSION)
            # A settings key with such a URL is dropped from full's settings...
            kept, dropped = arms.sanitize_user_settings({"svc": {"url": value}, "theme": "x"})
            self.assertEqual(kept, {"theme": "x"}, value)
            self.assertIn("svc", dropped)
            # ...and a key kept by name may not carry one (permissions patterns excepted).
            with self.assertRaises(ValueError, msg=value):
                arms.sanitize_user_settings({"outputStyle": f"style at {value}"})
        self.assertEqual(arms.foreign_hosts("http://sab-dense:8080/v1 http://sab-qdrant-r1:6334"),
                         [])
        kept, _ = arms.sanitize_user_settings(
            {"permissions": {"allow": ["Bash(git clone https://github.com/a/b.git)"]}})
        self.assertIn("permissions", kept)

    def test_arm_record_lists_kept_settings_with_their_effects(self):
        # Minor 3: every kept key is listed, and these three name their effect.
        self.spec("full")
        record = json.loads((self.root / "runs" / "full" / "arm-record.json").read_text())
        kept, _ = arms.sanitize_user_settings(self.user_settings)
        self.assertEqual(sorted(record["settings_kept"]), sorted(kept))
        for key in ("autoMemoryEnabled", "fileCheckpointingEnabled", "skillOverrides",
                    "outputStyle", "enabledPlugins", "permissions"):
            effect = record["settings_kept"][key]
            self.assertTrue(effect and not effect.startswith("unknown"), key)
        self.assertIn("# Memory", record["settings_kept"]["autoMemoryEnabled"])
        self.assertTrue(all(isinstance(v, str) and v for v in record["settings_kept"].values()))

    def test_the_arm_tool_set_is_the_daily_codescout_set(self):
        # I1: daily use advertises 21 codescout tools; references, symbol_at and call_graph are
        # gated on has_lsp (src/tools/core/types.rs Availability::RequiresLsp).
        self.assertEqual(len(arms.CODESCOUT_TOOLS), 21)
        for tool in arms.LSP_GATED_TOOLS:
            self.assertIn(tool, arms.CODESCOUT_TOOLS)
        self.assertEqual(set(arms.LSP_GATED_TOOLS), {"references", "symbol_at", "call_graph"})
        expected = arms.expected_mcp_tools("cs")
        self.assertIn("mcp__codescout__references", expected)
        self.assertIn("mcp__codescout__symbol_at", expected)
        self.assertEqual(expected, arms.expected_mcp_tools("full"))
        self.assertEqual(arms.expected_mcp_tools("bare"), [])

    def test_mcp_env_urls_are_rewritten_to_study_services(self):
        cs = self.spec("cs")
        raw = Path(next(m[0] for m in cs.mounts if m[1] == "/opt/sab/mcp.json")).read_text()
        for marker in ("127.0.0.1", "localhost", "host.docker.internal"):
            self.assertNotIn(marker, raw)
        env = json.loads(raw)["mcpServers"]["codescout"]["env"]
        self.assertEqual(env["CODESCOUT_QDRANT_URL"], "http://sab-qdrant-r1:6334")
        self.assertEqual(env["CODESCOUT_EMBEDDER_URL"], "http://sab-dense:8080")
        self.assertEqual(env["CODESCOUT_SPARSE_EMBEDDER_URL"], "http://sab-sparse:80")
        self.assertNotIn("CODESCOUT_RERANKER_URL", env)  # absent in daily use: dropped
        # Only the intersection is rewritten: keys the manifest lacks are not invented.
        self.assertNotIn("CODESCOUT_EMBED_URL", env)
        self.assertNotIn("LIBRARIAN_EMBED_URL", env)
        for key in ("CODESCOUT_EMBEDDER_PROTOCOL", "CODESCOUT_MODEL_DIM", "CODESCOUT_QUERY_PREFIX",
                    "CODESCOUT_RERANKER_PROTOCOL", "CODESCOUT_BM25_BOOST"):
            self.assertEqual(env[key], self.manifest["mcp_env"][key])
        # The bare arm carries no codescout env at all.
        self.assertFalse([k for k in self.spec("bare").env if k.startswith("CODESCOUT_")])

    def test_a_host_url_with_no_study_url_is_refused(self):
        manifest = dict(self.manifest)
        manifest["mcp_env"] = dict(self.manifest["mcp_env"],
                                   CODESCOUT_EXTRA_URL="http://localhost:9999")
        with self.assertRaises(ValueError):
            self.spec("cs", manifest=manifest)
        urls = dict(self.urls)
        del urls["CODESCOUT_SPARSE_EMBEDDER_URL"]  # stack started no sparse: key stays host
        with self.assertRaises(ValueError):
            self.spec("cs", urls=urls)

    def test_a_host_study_url_is_refused(self):
        with self.assertRaises(ValueError):
            self.spec("cs", urls=dict(self.urls, CODESCOUT_QDRANT_URL="http://127.0.0.1:6334"))

    def test_unknown_arm_is_refused(self):
        with self.assertRaises(ValueError):
            self.spec("cs-lite")

    def test_resource_flags_identical_across_arms(self):
        argvs = {}
        for arm in arms.ARMS:
            argv = arms.docker_run_args(f"sab-{arm}", "img", "sab-net", self.spec(arm),
                                        Path("/x/private.env"))
            self.assertIn("--cpus", argv)
            self.assertIn("--memory", argv)
            # Drop every mount; what is left (resources, network, env, image) must be equal.
            rest, i = [], 0
            while i < len(argv):
                if argv[i] == "-v":
                    i += 2
                    continue
                rest.append(argv[i] if argv[i] != f"sab-{arm}" else "<name>")
                i += 1
            argvs[arm] = rest
        self.assertEqual(argvs["bare"], argvs["cs"])
        self.assertEqual(argvs["cs"], argvs["full"])
# The project.toml codescout's own onboarding() wrote in the ruff task container (measured
# 2026-10-27, controls/round4/onboarding-probe/project.toml), embeddings as the hardware
# recommendation gave them: the shape prepare_index then edits.
ONBOARDED_TOML = """[embeddings]
model = "local:AllMiniLML6V2Q"

[ignored_paths]
force_include = []

[libraries]
auto_fetch_sources = false
auto_index = false
fetch_timeout_secs = 300

[libraries.version_overrides]

[lsp]

[memory]
protected = ["gotchas"]
semantic_anchor_min_similarity = 0.10000000149011612
semantic_anchor_top_n = 10
staleness_drift_threshold = 0.30000001192092896

[project]
encoding = "utf-8"
languages = [
    "bash",
    "markdown",
    "python",
    "rust",
]
name = "testbed"
system_prompt_sha256 = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
tool_timeout_secs = 60
"""


class FakeDocker:
    """Records docker argv. `exec ... index` exits with `index_rc`; the in-container MCP probe
    answers its onboarding() call (with an error when `onboarding_error`)."""

    def __init__(self, index_rc=0, onboarding_error=False):
        self.calls, self.index_rc, self.onboarding_error = [], index_rc, onboarding_error

    def __call__(self, *args, **kwargs):
        args = [str(a) for a in args]
        self.calls.append(args)
        rc = self.index_rc if (args[0] == "exec" and "index" in args) else 0
        out = "A /home/agent/.codescout\n" if args[0] == "diff" else "log line\n"
        if args[0] == "exec" and any(a.endswith("mcp_probe.py") for a in args):
            result = ({"content": [{"type": "text", "text": "onboarding failed"}],
                       "isError": True} if self.onboarding_error else
                      {"content": [{"type": "text", "text": "{\"languages\": [\"rust\"]}"}]})
            out = json.dumps({"tools": ["onboarding"], "calls": [
                {"tool": "onboarding", "arguments": {}, "result": result, "error": None,
                 "probe_error": None}]})
        if args[0] == "exec" and args[-1].endswith("/.codescout/project.toml") and "cat" in args:
            out = ONBOARDED_TOML
        return type("P", (), {"returncode": rc, "stdout": out})()


class PrepareIndexTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.data = Path(self.tmp.name)
        self.env = {"CODESCOUT_QDRANT_URL": "http://sab-qdrant:6334",
                    "CODESCOUT_EMBEDDER_URL": "http://sab-dense:8080",
                    "CODESCOUT_QUERY_PREFIX": "Represent this query: "}
        self.ticks = iter([100.0, 142.5])

    def tearDown(self):
        self.tmp.cleanup()

    def run_prepare(self, docker):
        return arms.prepare_index("sab-agent-cs-x:1", "sab-run-x:1", "sab-net", self.env,
                                  self.data, docker=docker, clock=lambda: next(self.ticks))

    @staticmethod
    def at(docker, pred):
        return next(i for i, c in enumerate(docker.calls) if pred(c))

    def test_prepare_index_times_the_index_and_commits_the_result(self):
        docker = FakeDocker()
        self.run_prepare(docker)
        run = next(c for c in docker.calls if c[0] == "run")
        name = run[run.index("--name") + 1]
        self.assertTrue(name.startswith("sab-"))
        self.assertEqual(run[run.index("--network") + 1], "sab-net")
        for flag in arms.RESOURCES:
            self.assertIn(flag, run)
        index = next(c for c in docker.calls if c[0] == "exec" and "index" in c)
        self.assertEqual(index[-4:], [arms.CODESCOUT_BIN, "index", "-p", "/testbed"])
        self.assertIn("CODESCOUT_QUERY_PREFIX=Represent this query: ", index)
        commit = next(c for c in docker.calls if c[0] == "commit")
        self.assertEqual(commit[-2:], [name, "sab-run-x:1"])
        self.assertEqual(docker.calls[-1], ["rm", "-f", name])
        cost = json.loads((self.data / "index-cost.json").read_text())
        self.assertEqual(cost["wall_seconds"], 42.5)
        self.assertEqual(cost["exit_code"], 0)
        self.assertEqual(cost["image"], "sab-agent-cs-x:1")
        self.assertIn("/home/agent/.codescout", (self.data / "index-diff.txt").read_text())

    def test_prepare_onboards_through_codescout_before_indexing(self):
        # Round-4 ruling I1: the project config (languages -> has_lsp -> references, symbol_at,
        # call_graph) comes from codescout's own onboarding(), run before the index so the
        # index chunks with the configured model.
        docker = FakeDocker()
        self.run_prepare(docker)
        self.assertEqual(json.loads((self.data / "prepare-calls.json").read_text()),
                         [["onboarding", {}]])
        mcp = json.loads((self.data / "prepare-mcp.json").read_text())["mcpServers"]["codescout"]
        self.assertEqual([mcp["command"], *mcp["args"]], [arms.CODESCOUT_BIN, *arms.CODESCOUT_ARGS])
        self.assertEqual(mcp["env"], self.env)
        probe = self.at(docker, lambda c: c[0] == "exec" and
                        any(a.endswith("mcp_probe.py") for a in c))
        self.assertEqual(docker.calls[probe][docker.calls[probe].index("-w") + 1], "/testbed")
        embed = self.at(docker, lambda c: c[0] == "exec" and
                        any(a.endswith("set_embeddings.py") for a in c))
        cleanup = self.at(docker, lambda c: c[0] == "exec" and c[-1] == arms.PREPARE_CLEANUP)
        index = self.at(docker, lambda c: c[0] == "exec" and "index" in c)
        git = self.at(docker, lambda c: c[0] == "exec" and "git add -f" in c[-1])
        self.assertTrue(probe < embed < cleanup < git < index)
        record = json.loads((self.data / "prepare.json").read_text())
        self.assertTrue(record["onboarding"]["ok"])

    def test_a_failed_onboarding_is_refused_and_nothing_is_committed(self):
        docker = FakeDocker(onboarding_error=True)
        with self.assertRaises(RuntimeError):
            self.run_prepare(docker)
        self.assertFalse([c for c in docker.calls if c[0] == "commit"])
        self.assertFalse([c for c in docker.calls if c[0] == "exec" and "index" in c])
        self.assertEqual(docker.calls[-1][:2], ["rm", "-f"])

    def test_prepare_sets_the_daily_embeddings_in_the_committed_config(self):
        docker = FakeDocker()
        self.run_prepare(docker)
        embed = next(c for c in docker.calls if c[0] == "exec" and
                     any(a.endswith("set_embeddings.py") for a in c))
        self.assertEqual(embed[-3:], ["/testbed/.codescout/project.toml",
                                      arms.DAILY_EMBED_MODEL, "http://sab-dense:8080/v1"])
        # The script the container runs, applied here to the TOML codescout's onboarding wrote.
        scope = {"__name__": "set_embeddings"}
        exec(arms.SET_EMBEDDINGS_PY, scope)
        rewritten = tomllib.loads(scope["rewrite"](ONBOARDED_TOML, arms.DAILY_EMBED_MODEL,
                                                   "http://sab-dense:8080/v1"))
        original = tomllib.loads(ONBOARDED_TOML)
        self.assertEqual(rewritten["embeddings"], {"model": "CodeRankEmbed",
                                                   "url": "http://sab-dense:8080/v1"})
        self.assertEqual({k: v for k, v in rewritten.items() if k != "embeddings"},
                         {k: v for k, v in original.items() if k != "embeddings"})
        record = json.loads((self.data / "prepare.json").read_text())
        self.assertEqual(record["embeddings"]["set"], {"model": "CodeRankEmbed",
                                                       "url": "http://sab-dense:8080/v1"})
        self.assertEqual(record["embeddings"]["onboarding_wrote"],
                         {"model": "local:AllMiniLML6V2Q"})
        self.assertIn("daily_effective", record["embeddings"])

    def test_prepare_cleanup_removes_memories_and_runtime_files_but_keeps_config(self):
        root = self.data / "proj"
        cs = root / ".codescout"
        for rel in ("project.toml", "workspace.toml", "index-state.json", "write.lock",
                    "write.lock.holder", "usage.db", "debug.log", "diagnostic-1a2b.log",
                    "memories/onboarding.md", "memories/.history/onboarding.md/x.md.bak",
                    "private-memories/p.md", "projects/ruff/memories/onboarding.md",
                    "projects/ruff/memories/language-patterns.md", "tmp/onboarding-prompt.md"):
            (cs / rel).parent.mkdir(parents=True, exist_ok=True)
            (cs / rel).write_text("x")
        subprocess.run(["sh", "-c", arms.PREPARE_CLEANUP], cwd=root, check=True)
        left = sorted(str(p.relative_to(cs)) for p in cs.rglob("*") if p.is_file())
        self.assertEqual(left, ["index-state.json", "project.toml", "workspace.toml",
                                "write.lock"])

    def test_prepare_commits_index_artifacts_but_not_tmp_lock(self):
        # Round-4 order: the config (project.toml, .gitignore) is committed BEFORE the index, so
        # the index records that commit as last_indexed_commit == HEAD; the index state itself
        # stays untracked but excluded (.git/info/exclude), so an agent's `git add -A` patch
        # carries none of it and the companion never sees the index behind HEAD.
        docker = FakeDocker()
        self.run_prepare(docker)
        kinds = [c[0] for c in docker.calls]
        execs = [c for c in docker.calls if c[0] == "exec"]
        index_at = next(i for i, c in enumerate(docker.calls) if c[0] == "exec" and "index" in c)
        git = [c for c in execs if any("git add -f" in a for a in c)]
        self.assertEqual(len(git), 1)
        script = git[0][-1]
        self.assertIn(".codescout", script)
        self.assertIn(".gitignore", script)
        self.assertIn("commit", script)
        self.assertNotIn("/tmp", script)
        self.assertEqual(git[0][git[0].index("-w") + 1], "/testbed")
        state = [c for c in execs if c[-1] == arms.INDEX_STATE_EXCLUDE]
        self.assertEqual(len(state), 1)
        self.assertEqual(state[0][state[0].index("-w") + 1], "/testbed")
        lock = [c for c in execs if any("/tmp/codescout-" in a for a in c)]
        self.assertEqual(len(lock), 1)
        self.assertIn("rm -f", lock[0][-1])
        self.assertIn("/tmp/sab-prepare", lock[0][-1])
        commit_at = kinds.index("commit")
        git_at = docker.calls.index(git[0])
        state_at = docker.calls.index(state[0])
        lock_at = docker.calls.index(lock[0])
        diff_at = kinds.index("diff")
        # config commit, index, index-state exclusion (+ HEAD and clean-tree checks), lock
        # removal, recorded diff, image commit.
        self.assertTrue(git_at < index_at < state_at < lock_at < diff_at < commit_at)

    def test_index_state_exclude_leaves_a_clean_tree_at_head(self):
        repo = self.data / "repo"
        cs = repo / ".codescout"
        cs.mkdir(parents=True)
        git = ["git", "-C", str(repo), "-c", "user.name=t", "-c", "user.email=t@example.invalid"]
        subprocess.run(["git", "init", "-q", str(repo)], check=True)
        (cs / "project.toml").write_text("[project]\nname = \"r\"\n")
        subprocess.run([*git, "add", "-A"], check=True)
        subprocess.run([*git, "commit", "-qm", "config"], check=True)
        head = subprocess.run([*git, "rev-parse", "HEAD"], capture_output=True, text=True,
                              check=True).stdout.strip()
        (cs / "write.lock").write_text("")
        (cs / "index-state.json").write_text(json.dumps({"last_indexed_commit": head}, indent=2))
        r = subprocess.run(["sh", "-c", arms.INDEX_STATE_EXCLUDE], cwd=repo, capture_output=True,
                           text=True)
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        status = subprocess.run([*git, "status", "--porcelain"], capture_output=True, text=True)
        self.assertEqual(status.stdout, "")
        self.assertTrue((cs / "index-state.json").exists())
        # Idempotent: a second run adds no duplicate lines.
        subprocess.run(["sh", "-c", arms.INDEX_STATE_EXCLUDE], cwd=repo, check=True)
        lines = (repo / ".git" / "info" / "exclude").read_text().splitlines()
        self.assertEqual(lines.count("/.codescout/index-state.json"), 1)
        # An index behind HEAD is refused.
        (cs / "index-state.json").write_text(json.dumps({"last_indexed_commit": "0" * 40}))
        r = subprocess.run(["sh", "-c", arms.INDEX_STATE_EXCLUDE], cwd=repo, capture_output=True,
                           text=True)
        self.assertNotEqual(r.returncode, 0)
        # So is a dirty tree.
        (cs / "index-state.json").write_text(json.dumps({"last_indexed_commit": head}))
        (repo / "stray.txt").write_text("x")
        r = subprocess.run(["sh", "-c", arms.INDEX_STATE_EXCLUDE], cwd=repo, capture_output=True,
                           text=True)
        self.assertNotEqual(r.returncode, 0)

    def test_a_failed_index_is_not_committed_and_the_container_is_removed(self):
        docker = FakeDocker(index_rc=1)
        with self.assertRaises(RuntimeError):
            self.run_prepare(docker)
        self.assertFalse([c for c in docker.calls if c[0] == "commit"])
        self.assertEqual(docker.calls[-1][:2], ["rm", "-f"])
        self.assertEqual(json.loads((self.data / "index-cost.json").read_text())["exit_code"], 1)

    def test_prepare_index_refuses_a_host_url(self):
        # Minor 2: an allowlist (host must start with sab-), not a denylist of host names.
        for bad in ("http://127.0.0.1:6334", "http://192.168.1.20:6334", "http://172.17.0.1:6334",
                    "http://[::1]:6334", "http://qdrant:6334"):
            with self.assertRaises(ValueError, msg=bad):
                arms.prepare_index("i", "o", "sab-net", dict(self.env, CODESCOUT_QDRANT_URL=bad),
                                   self.data, docker=FakeDocker())


if __name__ == "__main__":
    unittest.main()
