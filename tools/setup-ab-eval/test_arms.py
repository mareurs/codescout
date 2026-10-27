"""Tests for arms.py: the three arms differ only in CLI flags and mounts."""
import ast
import json
import tempfile
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
        (profile / "settings.json").write_text('{"outputStyle": "x"}\n')
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
        self.assertTrue(set(pairs(cs.cli_args)) <= set(pairs(full.cli_args)))
        added = [p for p in pairs(full.cli_args) if p not in pairs(cs.cli_args)]
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
        config = Path(next(m[0] for m in full.mounts if m[1] == arms.CONFIG_DIR))
        for src in self.manifest["full_config_files"]:
            self.assertEqual((config / Path(src).name).read_bytes(), Path(src).read_bytes())
        cs_config = Path(next(m[0] for m in cs.mounts if m[1] == arms.CONFIG_DIR))
        self.assertEqual(list(cs_config.iterdir()), [])

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
class FakeDocker:
    """Records docker argv; `exec ... index` exits with `index_rc`."""

    def __init__(self, index_rc=0):
        self.calls, self.index_rc = [], index_rc

    def __call__(self, *args, **kwargs):
        args = [str(a) for a in args]
        self.calls.append(args)
        rc = self.index_rc if (args[0] == "exec" and "index" in args) else 0
        out = "A /home/agent/.codescout\n" if args[0] == "diff" else "log line\n"
        return type("P", (), {"returncode": rc, "stdout": out})()


class PrepareIndexTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.data = Path(self.tmp.name)
        self.env = {"CODESCOUT_QDRANT_URL": "http://sab-qdrant:6334",
                    "CODESCOUT_QUERY_PREFIX": "Represent this query: "}
        self.ticks = iter([100.0, 142.5])

    def tearDown(self):
        self.tmp.cleanup()

    def run_prepare(self, docker):
        return arms.prepare_index("sab-agent-cs-x:1", "sab-run-x:1", "sab-net", self.env,
                                  self.data, docker=docker, clock=lambda: next(self.ticks))

    def test_prepare_index_times_the_index_and_commits_the_result(self):
        docker = FakeDocker()
        self.run_prepare(docker)
        run = next(c for c in docker.calls if c[0] == "run")
        name = run[run.index("--name") + 1]
        self.assertTrue(name.startswith("sab-"))
        self.assertEqual(run[run.index("--network") + 1], "sab-net")
        for flag in arms.RESOURCES:
            self.assertIn(flag, run)
        exec_ = next(c for c in docker.calls if c[0] == "exec")
        self.assertEqual(exec_[-4:], [arms.CODESCOUT_BIN, "index", "-p", "/testbed"])
        self.assertIn("CODESCOUT_QUERY_PREFIX=Represent this query: ", exec_)
        commit = next(c for c in docker.calls if c[0] == "commit")
        self.assertEqual(commit[-2:], [name, "sab-run-x:1"])
        self.assertEqual(docker.calls[-1], ["rm", "-f", name])
        cost = json.loads((self.data / "index-cost.json").read_text())
        self.assertEqual(cost["wall_seconds"], 42.5)
        self.assertEqual(cost["exit_code"], 0)
        self.assertEqual(cost["image"], "sab-agent-cs-x:1")
        self.assertIn("/home/agent/.codescout", (self.data / "index-diff.txt").read_text())

    def test_a_failed_index_is_not_committed_and_the_container_is_removed(self):
        docker = FakeDocker(index_rc=1)
        with self.assertRaises(RuntimeError):
            self.run_prepare(docker)
        self.assertFalse([c for c in docker.calls if c[0] == "commit"])
        self.assertEqual(docker.calls[-1][:2], ["rm", "-f"])
        self.assertEqual(json.loads((self.data / "index-cost.json").read_text())["exit_code"], 1)

    def test_prepare_index_refuses_a_host_url(self):
        with self.assertRaises(ValueError):
            arms.prepare_index("i", "o", "sab-net", {"CODESCOUT_QDRANT_URL": "http://127.0.0.1:6334"},
                               self.data, docker=FakeDocker())


if __name__ == "__main__":
    unittest.main()
