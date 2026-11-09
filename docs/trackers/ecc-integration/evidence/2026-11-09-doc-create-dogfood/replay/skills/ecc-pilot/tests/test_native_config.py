"""Native MCP configuration must carry isolated state explicitly."""
from pathlib import Path
import tempfile
import tomllib
import unittest

from live_smoke import native_config
from mcp_probe import isolated_server_env


class NativeConfigTests(unittest.TestCase):
    def test_native_mcp_receives_explicit_isolated_state(self):
        with tempfile.TemporaryDirectory(prefix="ecc-native-config-") as tmp:
            root = Path(tmp)
            env = isolated_server_env(root / "state")
            config = tomllib.loads(native_config(
                {"model": "existing-model", "model_reasoning_effort": "high"},
                "/usr/bin/codescout", root / "project", env))
            server = config["mcp_servers"]["codescout"]
            self.assertEqual(server["command"], "/usr/bin/codescout")
            self.assertEqual(server["args"], ["start", "--project", str(root / "project")])
            self.assertEqual(config["model"], "existing-model")
            forwarded = server["env"]
            for name in ("LIBRARIAN_DB", "LIBRARIAN_WORKSPACE",
                         "XDG_DATA_HOME", "XDG_STATE_HOME", "XDG_CONFIG_HOME"):
                self.assertTrue(Path(forwarded[name]).is_relative_to(root))
                self.assertEqual(forwarded[name], env[name])
            self.assertEqual(forwarded["CODESCOUT_ARTIFACT_BACKEND"], "sqlite-vec")
            self.assertNotIn("CODESCOUT_ALLOW_TEMP_WORKSPACE", forwarded)
            self.assertNotIn("CODEX_HOME", forwarded)
            self.assertNotIn("OPENAI_API_KEY", forwarded)


if __name__ == "__main__":
    unittest.main()
