"""Tests for freeze_setup: the manifest is a frozen record of the operator's setup."""
import json
import tempfile
import unittest
from pathlib import Path

import freeze_setup

VERSION_JSON = {"version": "0.15.0", "git_sha": "abc", "git_sha_full": "abc123", "git_dirty": True,
                "codescout_dirty": 1}
ENV = {
    "CODESCOUT_QDRANT_URL": "http://127.0.0.1:6334",
    "CODESCOUT_EMBEDDER_URL": "http://127.0.0.1:48081",
    "CODESCOUT_SPARSE_EMBEDDER_URL": "http://127.0.0.1:48084",
    "CODESCOUT_RERANKER_URL": "http://127.0.0.1:48083",
}


def fake_runner(argv):
    if argv[-1] == "version":
        return json.dumps(VERSION_JSON)
    return "2.1.309 (Claude Code)\n"


class Fixture:
    def __init__(self, tmp: Path, plugins=None, settings_env=None, mcp_env=ENV, records_extra=None):
        self.root = tmp
        self.profile = tmp / "profile"
        self.profile.mkdir()
        (self.profile / "CLAUDE.md").write_text("# claude\n")
        for d in ("projects", "memory", "todos", "sessions"):
            (self.profile / d).mkdir()
            (self.profile / d / "x.jsonl").write_text("{}")
        (self.profile / "history.jsonl").write_text("{}")
        self.cli = tmp / "claude-bin"
        self.cli.write_bytes(b"cli")
        self.cs_bin = tmp / "codescout-bin"
        self.cs_bin.write_bytes(b"cs")
        plugins = plugins or {"codescout-companion@mk": {"mcp": False}}
        enabled, records = {}, {}
        for key, spec in plugins.items():
            name = key.split("@")[0]
            path = Path(spec.get("path") or self.profile / "plugins/cache/mk" / name / "1.0")
            path.mkdir(parents=True, exist_ok=True)
            if spec.get("mcp"):
                (path / ".mcp.json").write_text(json.dumps({"mcpServers": {name: {"command": "x"}}}))
            enabled[key] = True
            records[key] = [{"scope": "user", "installPath": str(path), "version": "1.0"}]
        records.update(records_extra or {})
        settings = {"enabledPlugins": enabled}
        if settings_env is not None:
            settings["env"] = settings_env
        (self.profile / "settings.json").write_text(json.dumps(settings))
        (self.profile / "plugins").mkdir(exist_ok=True)
        (self.profile / "plugins/installed_plugins.json").write_text(
            json.dumps({"version": 2, "plugins": records}))
        self.claude_json = self.profile / ".claude.json"
        self.claude_json.write_text(json.dumps({"mcpServers": {"codescout": {
            "command": str(self.cs_bin), "args": ["start"], "env": mcp_env}}}))

    def freeze(self, probe=lambda url: True):
        return freeze_setup.freeze(self.profile, self.claude_json, probe, cli_path=self.cli,
                                   runner=fake_runner, out_path=self.root / "setup-manifest.json")


class FreezeTests(unittest.TestCase):
    def setUp(self):
        self._t = tempfile.TemporaryDirectory()
        self.tmp = Path(self._t.name)
        self.addCleanup(self._t.cleanup)

    def test_reranker_without_listener_is_recorded_absent(self):
        m = Fixture(self.tmp).freeze(probe=lambda url: ":48083" not in url)
        self.assertEqual(m["retrieval"], {"qdrant": True, "dense": True, "sparse": True,
                                          "reranker": False})

    def test_full_plugins_resolve_inside_the_profile(self):
        outside = self.tmp / "elsewhere" / "evil" / "1.0"
        fx = Fixture(self.tmp, {"evil@mk": {"path": outside}})
        with self.assertRaises(ValueError) as cm:
            fx.freeze()
        self.assertIn("evil@mk", str(cm.exception))

    def test_memory_and_transcript_dirs_are_never_listed(self):
        m = Fixture(self.tmp).freeze()
        files = m["full_config_files"]
        self.assertEqual(sorted(Path(f).name for f in files), ["CLAUDE.md", "settings.json"])
        for f in files:
            for bad in ("projects", "memory", "todos", ".jsonl"):
                self.assertNotIn(bad, f)

    def test_codescout_dirty_flag_is_recorded_not_gated(self):
        m = Fixture(self.tmp).freeze()
        self.assertEqual(m["codescout"]["version_json"], VERSION_JSON)
        self.assertEqual(m["codescout"]["version_json"]["codescout_dirty"], 1)

    def test_browser_mcp_plugin_is_excluded_with_reason(self):
        fx = Fixture(self.tmp, {
            "codescout-companion@mk": {"mcp": False},
            "chrome-devtools-mcp@mk": {"mcp": True},
            "other-mcp@mk": {"mcp": True},
        })
        m = fx.freeze()
        full = {p["name"]: p for p in m["plugins_full"]}
        self.assertNotIn("chrome-devtools-mcp", full)
        self.assertTrue(full["other-mcp"]["has_mcp"])
        self.assertFalse(full["codescout-companion"]["has_mcp"])
        excl = {p["name"]: p for p in m["plugins_full_excluded"]}
        self.assertIn("browser", excl["chrome-devtools-mcp"]["reason"])

    def test_manifest_written_and_cs_plugin_and_cli_recorded(self):
        fx = Fixture(self.tmp)
        m = fx.freeze()
        on_disk = json.loads((self.tmp / "setup-manifest.json").read_text())
        self.assertEqual(on_disk, m)
        self.assertEqual(m["mcp_env"], ENV)
        self.assertEqual(m["plugins_cs"]["version"], "1.0")
        self.assertEqual(m["cli"]["version"], "2.1.309 (Claude Code)")
        self.assertEqual(len(m["cli"]["sha256"]), 64)
        self.assertEqual(m["profile_dir"], str(fx.profile))

    def test_missing_service_url_is_an_error_not_an_absent_service(self):
        fx = Fixture(self.tmp)
        fx.claude_json.write_text(json.dumps({"mcpServers": {"codescout": {
            "command": str(fx.cs_bin), "env": {}}}}))
        with self.assertRaises(ValueError):
            fx.freeze()


    def test_env_inherited_from_settings_json(self):
        fx = Fixture(self.tmp, settings_env=dict(ENV, CODESCOUT_EMBEDDER_PROTOCOL="llama-server"),
                     mcp_env={})
        m = fx.freeze()
        self.assertEqual(m["mcp_env"]["CODESCOUT_EMBEDDER_PROTOCOL"], "llama-server")
        self.assertEqual(m["mcp_env_source"]["CODESCOUT_EMBEDDER_URL"], "settings")

    def test_mcp_env_overrides_settings_env(self):
        fx = Fixture(self.tmp, settings_env=dict(ENV, CODESCOUT_EMBEDDER_PROTOCOL="llama-server"),
                     mcp_env={"CODESCOUT_EMBEDDER_PROTOCOL": "openai"})
        m = fx.freeze()
        self.assertEqual(m["mcp_env"]["CODESCOUT_EMBEDDER_PROTOCOL"], "openai")
        self.assertEqual(m["mcp_env_source"]["CODESCOUT_EMBEDDER_PROTOCOL"], "claude_json")
        self.assertEqual(m["mcp_env_source"]["CODESCOUT_QDRANT_URL"], "settings")

    def test_non_codescout_settings_env_keys_never_recorded(self):
        fx = Fixture(self.tmp, settings_env=dict(ENV, CLAUDE_CODE_OAUTH_TOKEN="sk-secret",
                                                 LIBRARIAN_ENABLED="1", TMPDIR="/x"))
        m = fx.freeze()
        self.assertNotIn("CLAUDE_CODE_OAUTH_TOKEN", m["mcp_env"])
        self.assertNotIn("TMPDIR", m["mcp_env"])
        self.assertEqual(m["mcp_env"]["LIBRARIAN_ENABLED"], "1")
        self.assertNotIn("sk-secret", (self.tmp / "setup-manifest.json").read_text())

    def test_mismatched_profile_and_claude_json_raises(self):
        fx = Fixture(self.tmp)
        other = self.tmp / "other.json"
        other.write_text(fx.claude_json.read_text())
        with self.assertRaises(ValueError):
            freeze_setup.freeze(fx.profile, other, lambda u: True, cli_path=fx.cli,
                                runner=fake_runner, out_path=self.tmp / "m.json")

    def test_claude_json_defaults_to_profile_dot_claude_json(self):
        fx = Fixture(self.tmp)
        m = freeze_setup.freeze(fx.profile, None, lambda u: True, cli_path=fx.cli,
                                runner=fake_runner, out_path=self.tmp / "m.json")
        self.assertEqual(m["mcp_env"], ENV)

    def test_user_scope_record_wins_over_later_project_record(self):
        fx = Fixture(self.tmp)
        key = "codescout-companion@mk"
        recs = json.loads((fx.profile / "plugins/installed_plugins.json").read_text())
        proj = self.tmp / "elsewhere"
        proj.mkdir()
        recs["plugins"][key].append({"scope": "project", "projectPath": "/p",
                                     "installPath": str(proj), "version": "9.9"})
        (fx.profile / "plugins/installed_plugins.json").write_text(json.dumps(recs))
        m = fx.freeze()
        self.assertEqual(m["plugins_cs"]["version"], "1.0")

    def test_missing_user_scope_record_raises(self):
        fx = Fixture(self.tmp)
        key = "codescout-companion@mk"
        recs = json.loads((fx.profile / "plugins/installed_plugins.json").read_text())
        recs["plugins"][key][0]["scope"] = "project"
        (fx.profile / "plugins/installed_plugins.json").write_text(json.dumps(recs))
        with self.assertRaises(ValueError):
            fx.freeze()

    def test_missing_companion_raises(self):
        fx = Fixture(self.tmp, {"other@mk": {}})
        with self.assertRaises(ValueError) as cm:
            fx.freeze()
        self.assertIn("codescout-companion", str(cm.exception))


    def test_cli_has_no_default_profile(self):
        with self.assertRaises(SystemExit):
            freeze_setup.main(["--cli", "x"])


if __name__ == "__main__":
    unittest.main()
