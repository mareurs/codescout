"""Tests for smoke: the $0 checks run with no token, no network and sab-smoke-* names only."""
import json
import shutil
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

import smoke

FIX = Path(__file__).resolve().parent / "fixtures"


class FakeDocker:
    def __init__(self):
        self.calls = []

    def __call__(self, *args, check=True, timeout=None):
        a = [str(x) for x in args]
        self.calls.append(a)
        out = ""
        if a[:2] == ["exec", a[1]] and a[-1] == "env":
            out = "PATH=/usr/bin\nCLAUDE_CONFIG_DIR=/run/config\n"
        return SimpleNamespace(returncode=0, stdout=out)


class SmokeTest(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="sab-smoke-test-"))
        self.addCleanup(shutil.rmtree, self.tmp, ignore_errors=True)
        for d in ("companion", "superpowers", "buddy"):
            (self.tmp / d).mkdir()
        (self.tmp / "CLAUDE.md").write_text("x\n")
        (self.tmp / "settings.json").write_text("{}\n")
        self.manifest = {
            "cli": {"path": "/c"}, "mcp_env": {"CODESCOUT_QDRANT_URL": "http://127.0.0.1:6334"},
            "plugins_cs": {"install_path": str(self.tmp / "companion")},
            "plugins_full": [{"name": "superpowers", "install_path": str(self.tmp / "superpowers")},
                             {"name": "buddy", "install_path": str(self.tmp / "buddy")}],
            "full_config_files": [str(self.tmp / "CLAUDE.md"), str(self.tmp / "settings.json")]}

    def test_summarize_reads_the_init_record(self):
        text = (FIX / "full_valid.jsonl").read_text()
        s = smoke.summarize(text + json.dumps({"type": "result", "subtype": "success",
                                               "is_error": True, "result": "Not logged in"}))
        self.assertTrue(s["init_emitted"])
        self.assertEqual(s["native_tools"], ["Glob", "Grep", "Read", "Skill", "Task"])
        self.assertEqual(len(s["mcp_tools"]), 21)
        self.assertEqual(s["last"]["result"], "Not logged in")

    def test_the_container_has_no_network_no_token_and_a_sab_name(self):
        fake = FakeDocker()
        cli = SimpleNamespace(returncode=1, stdout="", stderr="")
        with mock.patch.object(smoke, "docker", fake), \
                mock.patch.object(smoke.subprocess, "run", return_value=cli) as run:
            rec = smoke.init_capture("full", "sab-arm-x:t", self.manifest, self.tmp / "out")
        started = next(c for c in fake.calls if c[0] == "run")
        self.assertEqual(started[started.index("--network") + 1], "none")
        name = started[started.index("--name") + 1]
        self.assertTrue(name.startswith("sab-smoke-full-"))
        env = Path(started[started.index("--env-file") + 1]).read_text()
        self.assertEqual(sorted(l.split("=")[0] for l in env.splitlines()),
                         ["CLAUDE_CODE_MAX_OUTPUT_TOKENS", "CLAUDE_CONFIG_DIR",
                          "DISABLE_AUTOUPDATER"])  # arms.CONTAINER_ENV only: no token, no proxy
        argv = run.call_args[0][0]
        self.assertEqual(argv[:5], ["docker", "exec", "-w", "/testbed", name])
        self.assertEqual(argv[argv.index("--tools") + 1],
                         "Bash,Read,Write,Edit,Glob,Grep,Skill,Agent")
        self.assertEqual(argv[argv.index("--output-format") + 1], "stream-json")
        self.assertEqual(fake.calls[-1], ["rm", "-f", name])
        self.assertEqual(rec["rm"], 0)

    def test_a_credential_in_the_container_env_stops_before_the_cli(self):
        fake = FakeDocker()
        real = fake.__call__

        def leaky(*args, **kw):
            r = real(*args, **kw)
            if args[-1] == "env":
                r.stdout += "CLAUDE_CODE_OAUTH_TOKEN=x\n"
            return r
        with mock.patch.object(smoke, "docker", leaky), \
                mock.patch.object(smoke.subprocess, "run") as run:
            with self.assertRaisesRegex(RuntimeError, "credential"):
                smoke.init_capture("cs", "sab-arm-x:t", self.manifest, self.tmp / "out")
        run.assert_not_called()
        self.assertEqual(fake.calls[-1][:2], ["rm", "-f"])


if __name__ == "__main__":
    unittest.main()
