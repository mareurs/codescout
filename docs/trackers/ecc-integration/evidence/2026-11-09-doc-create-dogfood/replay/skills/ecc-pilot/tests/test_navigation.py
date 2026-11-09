"""Regression checks for the initialized Python fixture and native LSP navigation."""
from pathlib import Path
import os
import shutil
import tempfile
import unittest

from mcp_probe import LSP_TOOLS, McpProbe, copy_fixture

BINARY = os.environ.get("CODESCOUT_BIN") or shutil.which("codescout")


@unittest.skipUnless(BINARY, "installed codescout binary is required")
class NavigationTests(unittest.TestCase):
    def test_unconfigured_fixture_is_a_negative_control(self):
        with tempfile.TemporaryDirectory(prefix="ecc-no-lsp-") as tmp:
            project = copy_fixture(Path(tmp) / "project", configured=False)
            with McpProbe(BINARY, project) as server:
                state = server.call("workspace", action="status")
                self.assertEqual(state["languages"], [])
                self.assertFalse(server.tools() & LSP_TOOLS)

    def test_configured_fixture_advertises_lsp_navigation(self):
        with tempfile.TemporaryDirectory(prefix="ecc-with-lsp-") as tmp:
            project = copy_fixture(Path(tmp) / "project")
            with McpProbe(BINARY, project) as server:
                state = server.call("workspace", action="status")
                self.assertIn("python", state["languages"])
                self.assertTrue(LSP_TOOLS <= server.tools())
    def test_scratch_spec_uses_a_private_catalog(self):
        with tempfile.TemporaryDirectory(prefix="ecc-private-catalog-") as tmp:
            project = copy_fixture(Path(tmp) / "project")
            with McpProbe(BINARY, project) as server:
                created = server.call(
                    "doc", action="create", kind="spec", title="Scratch isolation probe",
                    rel_path="docs/specs/isolation.md", body="# Scratch isolation probe\n",
                    workspace=str(project))
                self.assertNotEqual(created.get("ok"), False, created)
                self.assertTrue((project / "docs/specs/isolation.md").is_file())
                catalog = Path(server.env["LIBRARIAN_DB"])
                self.assertTrue(catalog.is_relative_to(Path(tmp)))
                self.assertTrue(catalog.is_file())
                found = server.call("doc", action="get", id=created["id"],
                                    full=True, workspace=str(project))
                self.assertEqual(found["kind"], "spec")

    @unittest.skipUnless(shutil.which("pyright-langserver"), "pyright-langserver is required")
    def test_navigation_traces_the_sync_caller(self):
        with tempfile.TemporaryDirectory(prefix="ecc-sync-caller-") as tmp:
            project = copy_fixture(Path(tmp) / "project")
            with McpProbe(BINARY, project) as server:
                results = {}
                for name, extra in (("references", {}), ("call_graph", {"direction": "callers"})):
                    result = server.request("tools/call", {
                        "name": name, "arguments": {
                            "symbol": "required_sync", "path": "behavior.py",
                            "workspace": str(project), **extra
                        }})
                    self.assertFalse(result.get("isError", False), result)
                    results[name] = result["content"][0]["text"]
                self.assertRegex(results["references"],
                                 r"(?m)^\s*\d+\s+items = required_sync\(fetch\)\s*$")
                self.assertIn("commit_sync", results["call_graph"])


if __name__ == "__main__":
    unittest.main()
