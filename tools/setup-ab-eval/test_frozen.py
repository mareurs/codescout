"""Tests for frozen: the frozen arm inputs (final review I-7). Temp dirs only; no docker."""
import json
import os
import shutil
import tempfile
import unittest
from pathlib import Path

import arms
import frozen

TOKENISH = "sk-ant-oat01-NOTAREALTOKEN-0123456789abcdef"


class Fixture:
    def __init__(self, root):
        self.root = Path(root)
        profile = self.root / "profile"
        profile.mkdir()
        (profile / "CLAUDE.md").write_text("# rules\n")
        (profile / "settings.json").write_text(json.dumps({
            "outputStyle": "x", "permissions": {"deny": ["Bash"]}, "theme": "dark",
            "env": {"CLAUDE_CODE_OAUTH_TOKEN": TOKENISH, "ANTHROPIC_BASE_URL": "http://localhost:8082"},
            "statusLine": {"command": "/home/u/s.sh"}, "unlistedKey": True,
            "primaryApiKeyHelper": TOKENISH}))
        self.plugins = {}
        for name in ("codescout-companion", "superpowers", "buddy"):
            d = self.root / "cache" / name
            (d / "hooks").mkdir(parents=True)
            (d / "hooks" / "run.mjs").write_text(f"// {name}\n")
            os.chmod(d / "hooks" / "run.mjs", 0o755)
            (d / "README.md").write_text(name)
            self.plugins[name] = d
        blobs = self.plugins["buddy"] / "blobs"
        blobs.mkdir()
        (blobs / "b1").write_text("blob")
        (self.plugins["buddy"] / "snap").mkdir()
        os.symlink("../blobs/b1", self.plugins["buddy"] / "snap" / "config.json")
        self.manifest = {
            "plugins_cs": {"install_path": str(self.plugins["codescout-companion"])},
            "plugins_full": [{"name": "superpowers", "install_path": str(self.plugins["superpowers"])},
                             {"name": "codescout-companion",
                              "install_path": str(self.plugins["codescout-companion"])},
                             {"name": "buddy", "install_path": str(self.plugins["buddy"])}],
            "full_config_files": [str(profile / "CLAUDE.md"), str(profile / "settings.json")]}
        self.dest = self.root / "study" / "frozen"


class FrozenTest(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="sab-frozen-"))
        self.addCleanup(shutil.rmtree, self.tmp, ignore_errors=True)
        self.fx = Fixture(self.tmp)

    def freeze(self):
        return frozen.freeze_inputs(self.fx.manifest, self.fx.dest)

    def test_layout_and_hashes(self):
        rec = self.freeze()
        d = self.fx.dest
        self.assertEqual(sorted(rec["trees"]), sorted(
            ["config/CLAUDE.md", "config/settings.json", "plugins/codescout-companion",
             "plugins/superpowers", "plugins/buddy"]))
        self.assertEqual((d / "config" / "CLAUDE.md").read_text(), "# rules\n")
        for name, src in self.fx.plugins.items():
            self.assertEqual(rec["trees"][f"plugins/{name}"], frozen.tree_sha256(src))
            self.assertEqual(rec["sources"][f"plugins/{name}"], str(src))
        self.assertTrue((d / "plugins" / "buddy" / "snap" / "config.json").is_symlink())
        self.assertEqual(json.loads((d / "frozen.json").read_text())["trees"], rec["trees"])
        frozen.check_trees(d, rec["trees"])  # unchanged: passes
        self.assertEqual(list(self.tmp.joinpath("study").iterdir()), [d])  # no partial dir left

    def test_settings_copy_only_the_stage1_keys_and_never_a_credential(self):
        rec = self.freeze()
        text = (self.fx.dest / "config" / "settings.json").read_text()
        self.assertEqual(sorted(json.loads(text)), ["outputStyle", "permissions", "theme"])
        self.assertNotIn(TOKENISH, text)
        self.assertNotIn("localhost", text)
        self.assertEqual(rec["settings"]["copied_keys"], ["outputStyle", "permissions", "theme"])
        self.assertEqual(rec["settings"]["not_copied_keys"],
                         ["env", "primaryApiKeyHelper", "statusLine", "unlistedKey"])
        self.assertNotIn(TOKENISH, (self.fx.dest / "frozen.json").read_text())
        self.assertTrue(set(frozen.SETTINGS_KEYS) >= {"outputStyle", "permissions",
                                                      "enabledPlugins"})
        self.assertNotIn("env", frozen.SETTINGS_KEYS)

    def test_a_credential_in_a_copied_key_refuses_the_freeze(self):
        with self.assertRaisesRegex(frozen.Mismatch, "credential"):
            frozen.frozen_settings({"permissions": {"allow": [f"Bash(echo {TOKENISH})"]}})

    def test_every_change_changes_the_tree_hash(self):
        self.freeze()
        plugin = self.fx.dest / "plugins" / "superpowers"
        base = frozen.tree_sha256(plugin)
        self.assertEqual(base, frozen.tree_sha256(plugin))  # deterministic
        edits = [
            lambda: (plugin / "README.md").write_text("changed"),
            lambda: os.chmod(plugin / "hooks" / "run.mjs", 0o644),
            lambda: (plugin / "new.txt").write_text(""),
            lambda: (plugin / "hooks" / "empty").mkdir(),
            lambda: (plugin / "README.md").rename(plugin / "README2.md"),
            lambda: os.symlink("hooks/run.mjs", plugin / "link"),
        ]
        for i, edit in enumerate(edits):
            with self.subTest(i):
                shutil.rmtree(plugin)
                shutil.copytree(self.fx.plugins["superpowers"], plugin, symlinks=True)
                self.assertEqual(frozen.tree_sha256(plugin), base)
                edit()
                self.assertNotEqual(frozen.tree_sha256(plugin), base)
        link = self.fx.dest / "plugins" / "buddy" / "snap" / "config.json"
        before = frozen.tree_sha256(self.fx.dest / "plugins" / "buddy")
        link.unlink()
        os.symlink("../blobs/other", link)
        self.assertNotEqual(frozen.tree_sha256(self.fx.dest / "plugins" / "buddy"), before)

    def test_check_trees_refuses_a_change_a_loss_and_no_record(self):
        rec = self.freeze()
        (self.fx.dest / "config" / "CLAUDE.md").write_text("# edited after the freeze\n")
        with self.assertRaisesRegex(frozen.Mismatch, "config/CLAUDE.md"):
            frozen.check_trees(self.fx.dest, rec["trees"])
        (self.fx.dest / "config" / "CLAUDE.md").write_text("# rules\n")
        frozen.check_trees(self.fx.dest, rec["trees"])
        shutil.rmtree(self.fx.dest / "plugins" / "buddy")
        with self.assertRaisesRegex(frozen.Mismatch, "missing"):
            frozen.check_trees(self.fx.dest, rec["trees"])
        for empty in (None, {}):
            with self.assertRaises(frozen.Mismatch):
                frozen.check_trees(self.fx.dest, empty)

    def test_a_freeze_is_made_once_and_never_half(self):
        self.freeze()
        with self.assertRaisesRegex(frozen.Mismatch, "exists"):
            self.freeze()
        # A symlink that leaves its tree refuses, and nothing is left behind.
        os.symlink("/etc/passwd", self.fx.plugins["superpowers"] / "escape")
        dest = self.tmp / "other"
        with self.assertRaisesRegex(frozen.Mismatch, "leaves its plugin tree"):
            frozen.freeze_inputs(self.fx.manifest, dest)
        self.assertFalse(dest.exists())
        self.assertEqual([p for p in self.tmp.iterdir() if p.name.startswith("other")], [])

    def test_frozen_manifest_names_only_frozen_paths(self):
        self.freeze()
        m = frozen.frozen_manifest(self.fx.manifest, self.fx.dest)
        root = self.fx.dest.resolve()
        self.assertEqual(m["frozen_root"], str(root))
        paths = ([m["plugins_cs"]["install_path"]] + [p["install_path"] for p in m["plugins_full"]]
                 + m["full_config_files"])
        self.assertTrue(all(Path(p).is_relative_to(root) and Path(p).exists() for p in paths))
        self.assertEqual(self.fx.manifest["plugins_cs"]["install_path"],
                         str(self.fx.plugins["codescout-companion"]))  # the input is unchanged
        # arms accepts it for every arm, and the full config it copies is the frozen one.
        run_dir = self.tmp / "run-full"
        urls = {"CODESCOUT_QDRANT_URL": "http://sab-qdrant-x:6334"}
        m["mcp_env"] = {"CODESCOUT_QDRANT_URL": "http://127.0.0.1:6334"}
        spec = arms.arm_invocation("full", "claude-sonnet-5-5", "p", dict(m, cli={"path": "/c"}),
                                   urls, run_dir, session_id="s")
        hosts = {t: h for h, t, _ in spec.mounts}
        self.assertTrue(Path(hosts["/opt/plugins/buddy"]).is_relative_to(root))
        self.assertEqual(sorted(json.loads((run_dir / "config" / "settings.json").read_text())),
                         ["outputStyle", "permissions", "theme"])

    def test_two_install_paths_for_one_plugin_refuse(self):
        m = json.loads(json.dumps(self.fx.manifest))
        m["plugins_full"][1]["install_path"] = str(self.fx.plugins["buddy"])
        with self.assertRaisesRegex(frozen.Mismatch, "two install paths"):
            frozen.plugin_sources(m)


if __name__ == "__main__":
    unittest.main()
