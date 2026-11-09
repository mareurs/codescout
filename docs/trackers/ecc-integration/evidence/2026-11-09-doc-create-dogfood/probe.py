"""Exercise real MCP doc(create) against a disposable catalog and filesystem."""
import argparse
import importlib.util
import json
from pathlib import Path
import sqlite3
import subprocess
import tempfile

HELPER = Path("/home/marius/work/claude/claude-plugins.worktrees/ecc-codex-codescout/ecc-codescout/tests/mcp_probe.py")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", required=True)
    parser.add_argument("--output", required=True)
    args = parser.parse_args()
    output = Path(args.output)
    if output.exists():
        raise SystemExit("Refusing to overwrite existing evidence: " + str(output))
    spec = importlib.util.spec_from_file_location("ecc_mcp_probe", HELPER)
    helper = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(helper)
    record = {"binary": str(Path(args.binary).resolve()), "cases": []}
    with tempfile.TemporaryDirectory(prefix="ecc-doc-create-dogfood-") as tmp:
        root = Path(tmp)
        project = root / "project"
        project.mkdir()
        subprocess.run(["git", "init", "-q", str(project)], check=True)
        config = project / ".codescout" / "project.toml"
        config.parent.mkdir()
        config.write_text('[project]\nname = "ecc-doc-create-dogfood"\nlanguages = []\n')
        blocker = project / "blocked"
        blocker.write_text("preserve this existing file")
        env = helper.isolated_server_env(root / "private-state")
        catalog_path = Path(env["LIBRARIAN_DB"])
        record["isolation"] = {"project": str(project), "catalog": str(catalog_path)}
        with helper.McpProbe(args.binary, project, env=env) as mcp:
            mcp.call("workspace", action="status")
            with sqlite3.connect(catalog_path) as cat:
                cat.execute("CREATE TRIGGER dogfood_fail_delete BEFORE DELETE ON artifact BEGIN SELECT RAISE(ABORT, 'dogfood delete failure'); END")
            for delete_fails in (True, False):
                if not delete_fails:
                    with sqlite3.connect(catalog_path) as cat:
                        cat.execute("DROP TRIGGER dogfood_fail_delete")
                rel = "blocked/delete-fails.md" if delete_fails else "blocked/delete-works.md"
                response = mcp.request("tools/call", {"name": "doc", "arguments": {
                    "action": "create", "kind": "note", "title": "Dogfood failure",
                    "rel_path": rel, "body": "isolated probe",
                    "augment": {"prompt": "p", "params": {}}, "workspace": str(project)}})
                text = "\n".join(b.get("text", "") for b in response.get("content", []) if b.get("type") == "text")
                with sqlite3.connect(catalog_path) as cat:
                    rows = cat.execute("SELECT id FROM artifact WHERE abs_path = ?", (str(project / rel),)).fetchall()
                    augmentations = cat.execute("SELECT count(*) FROM artifact_augmentation WHERE artifact_id IN (SELECT id FROM artifact WHERE abs_path = ?)", (str(project / rel),)).fetchone()[0]
                case = {"delete_fails": delete_fails, "response": response,
                        "residual_artifact_rows": len(rows), "residual_augmentation_rows": augmentations}
                record["cases"].append(case)
                # Write raw evidence even if an assertion below exposes an unexpected seam.
                output.parent.mkdir(parents=True, exist_ok=True)
                output.write_text(json.dumps(record, indent=2) + "\n")
                assert response.get("isError") is True, response
                assert "could not write" in text, text
                assert "cleanup was attempted" in text and "may remain" in text, text
                assert "catalog row was taken back" not in text, text
                assert len(rows) == (1 if delete_fails else 0), case
                assert augmentations == (1 if delete_fails else 0), case
                assert blocker.read_text() == "preserve this existing file"
            success = mcp.call("doc", action="create", kind="note", title="Dogfood success",
                               rel_path="docs/ok.md", body="ok", workspace=str(project))
            assert success.get("id") and (project / "docs/ok.md").is_file(), success
            record["successful_create"] = success
            record["raw_responses"] = mcp.responses
    record["passed"] = True
    output.write_text(json.dumps(record, indent=2) + "\n")
    print(json.dumps({"passed": True, "failure_cases": len(record["cases"]), "successful_create": True, "output": str(output)}))


if __name__ == "__main__":
    main()
