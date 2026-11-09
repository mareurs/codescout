"""Package boundaries and native Codex discovery; no model calls or user config writes."""
import hashlib
import json
import os
from pathlib import Path
import queue
import shutil
import subprocess
import tempfile
import threading
import unittest

ROOT = Path(__file__).resolve().parents[1]
EXPECTED_SKILLS = {"ecc-spec-miner", "ecc-silent-failure-review"}


class PackageTests(unittest.TestCase):
    def test_codex_only_and_self_contained(self):
        manifest = json.loads((ROOT / ".codex-plugin/plugin.json").read_text())
        self.assertEqual(manifest["name"], "ecc-codescout")
        self.assertNotIn("hooks", manifest)
        self.assertNotIn("mcpServers", manifest)
        for forbidden in (".claude-plugin", ".mcp.json", "hooks", "agents", "commands"):
            self.assertFalse((ROOT / forbidden).exists(), forbidden)
        skills = (ROOT / manifest["skills"]).resolve()
        self.assertTrue(skills.is_relative_to(ROOT))
        found = {p.parent.name for p in skills.glob("*/SKILL.md")}
        self.assertEqual(found, EXPECTED_SKILLS)
        for name in found:
            text = (skills / name / "SKILL.md").read_text()
            self.assertTrue(text.startswith("---\n"))
            self.assertIn("name: " + name + "\n", text)
            self.assertIn("description: Use when", text)
            self.assertNotIn("tools:", text)
            self.assertNotIn("model:", text)
            self.assertNotIn("/home/", text)
            self.assertNotIn("openspec/specs/", text)

    def test_marketplace_resolves_this_package(self):
        market = json.loads((ROOT / ".agents/plugins/marketplace.json").read_text())
        self.assertEqual(market["name"], "ecc-codescout-pilot")
        self.assertEqual(len(market["plugins"]), 1)
        plugin = market["plugins"][0]
        self.assertEqual(plugin["name"], "ecc-codescout")
        self.assertEqual(plugin["source"], {"source": "local", "path": "./"})

    def test_provenance_and_license(self):
        upstream = json.loads((ROOT / "upstream.json").read_text())
        self.assertEqual(upstream["repository"], "https://github.com/affaan-m/ECC")
        self.assertEqual(len(upstream["commit"]), 40)
        self.assertEqual(set(upstream["files"]), {
            "agents/spec-miner.md", "agents/silent-failure-hunter.md", "LICENSE"
        })
        license_bytes = (ROOT / "LICENSE").read_bytes()
        self.assertEqual(hashlib.sha256(license_bytes).hexdigest(), upstream["files"]["LICENSE"])
        self.assertIn(b"Affaan Mustafa", license_bytes)

    @unittest.skipUnless(shutil.which("codex"), "native Codex CLI is not installed")
    def test_native_install_and_skill_discovery(self):
        with tempfile.TemporaryDirectory(prefix="ecc-codex-smoke-") as tmp:
            home = Path(tmp) / "home"
            home.mkdir()
            project = Path(tmp) / "project"
            project.mkdir()
            env = dict(os.environ, CODEX_HOME=str(home))
            # Plugin state is isolated; ~/.agents user skills may remain discoverable.
            def cli(*args):
                result = subprocess.run(["codex", *args], cwd=project, env=env,
                                        capture_output=True, text=True, timeout=40)
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                return result.stdout
            cli("plugin", "marketplace", "add", str(ROOT), "--json")
            cli("plugin", "add", "ecc-codescout@ecc-codescout-pilot")
            listing = cli("plugin", "list", "--json")
            self.assertIn("ecc-codescout", listing)
            config = (home / "config.toml").read_text()
            self.assertIn("ecc-codescout", config)
            proc = subprocess.Popen(["codex", "app-server"], cwd=project, env=env,
                                    stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                    stderr=subprocess.DEVNULL, text=True)
            messages = queue.Queue()
            def collect():
                for line in proc.stdout:
                    messages.put(line)
            reader = threading.Thread(target=collect, daemon=True)
            reader.start()
            def request(method, params, request_id):
                proc.stdin.write(json.dumps({"id": request_id, "method": method,
                                             "params": params}) + "\n")
                proc.stdin.flush()
                while True:
                    response = json.loads(messages.get(timeout=40))
                    if response.get("id") == request_id:
                        self.assertNotIn("error", response, response)
                        return response["result"]
            try:
                request("initialize", {"clientInfo": {"name": "ecc-pilot-smoke",
                        "version": "0.1.0"}, "capabilities": {"experimentalApi": True}}, 1)
                proc.stdin.write('{"method":"initialized","params":{}}\n')
                proc.stdin.flush()
                result = request("skills/list", {"cwds": [str(project)],
                                                "forceReload": True}, 2)
                for row in result["data"]:
                    self.assertFalse(row["errors"], row["errors"])
                skills = [skill for row in result["data"] for skill in row["skills"]
                          if skill.get("pluginId") == "ecc-codescout@ecc-codescout-pilot"]
                self.assertEqual({s["name"] for s in skills},
                                 {"ecc-codescout:" + name for name in EXPECTED_SKILLS})
                for skill in skills:
                    self.assertTrue(skill["enabled"], skill)
                    installed = Path(skill["path"]).resolve()
                    self.assertTrue(installed.is_relative_to(home), skill)
                    source = ROOT / "skills" / skill["name"].split(":", 1)[1] / "SKILL.md"
                    self.assertEqual(installed.read_bytes(), source.read_bytes())
                    prompt = skill["interface"]["defaultPrompt"]
                    self.assertTrue(prompt.startswith("Use $" + skill["name"] + " "), prompt)
                    metadata = Path("agents/openai.yaml")
                    self.assertEqual((installed.parent / metadata).read_bytes(),
                                     (source.parent / metadata).read_bytes())
            finally:
                proc.terminate()
                proc.communicate(timeout=10)
                reader.join(timeout=1)


if __name__ == "__main__":
    unittest.main()
