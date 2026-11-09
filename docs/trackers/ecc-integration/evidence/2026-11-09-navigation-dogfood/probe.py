"""Record private-MCP navigation controls; leaves interpretation to adjudication."""
import argparse
import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile

HELPER = Path("/home/marius/work/claude/claude-plugins.worktrees/ecc-codex-codescout/ecc-codescout/tests/mcp_probe.py")
p = argparse.ArgumentParser()
p.add_argument("--binary", required=True)
p.add_argument("--output", required=True)
a = p.parse_args()
output = Path(a.output)
if output.exists():
    raise SystemExit("Refusing to overwrite evidence")
spec = importlib.util.spec_from_file_location("ecc_mcp_probe", HELPER)
helper = importlib.util.module_from_spec(spec)
spec.loader.exec_module(helper)
record = {"binary": str(Path(a.binary).resolve()), "calls": []}
with tempfile.TemporaryDirectory(prefix="ecc-navigation-") as tmp:
    root = Path(tmp)
    project = root / "project"
    project.mkdir()
    subprocess.run(["git", "init", "-q", str(project)], check=True)
    config = project / ".codescout" / "project.toml"
    config.parent.mkdir()
    config.write_text('[project]\nname = "ecc-navigation-dogfood"\nlanguages = ["python"]\n')
    (project / "sample.py").write_text("def leaf():\n    return 1\n\nclass Marker:\n    pass\n\ndef build():\n    return Marker()\n")
    with helper.McpProbe(a.binary, project, env=helper.isolated_server_env(root / "state")) as mcp:
        for name, arguments in [
            ("symbols", {"path": "sample.py"}),
            ("call_graph", {"symbol": "leaf", "path": "sample.py", "direction": "callees", "max_depth": 1}),
            ("call_graph", {"symbol": "Marker", "path": "sample.py", "direction": "both", "max_depth": 1}),
            ("references", {"symbol": "leaf", "path": "sample.py"}),
            ("references", {"symbol": "Marker", "path": "sample.py"}),
        ]:
            arguments["workspace"] = str(project)
            response = mcp.request("tools/call", {"name": name, "arguments": arguments}, timeout=90)
            record["calls"].append({"tool": name, "arguments": arguments, "response": response})
            output.parent.mkdir(parents=True, exist_ok=True)
            output.write_text(json.dumps(record, indent=2) + "\n")
record["transport_completed"] = True
output.write_text(json.dumps(record, indent=2) + "\n")
print(json.dumps({"output": str(output), "tool_calls_recorded": len(record["calls"])}))
