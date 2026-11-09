"""Opt-in native model smoke. Disposable authenticated profile; evidence kept separately."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import tomllib

from mcp_probe import LSP_TOOLS, McpProbe, PACKAGE, copy_fixture, isolated_server_env


def require_completed_tools(events, expected):
    calls = []
    for event in events:
        item = event.get("item", {})
        if (event.get("type") != "item.completed"
                or item.get("type") != "mcp_tool_call"
                or item.get("server") != "codescout"
                or item.get("status") != "completed" or item.get("error")):
            continue
        result = item.get("result") or {}
        if result.get("isError"):
            continue
        content = result.get("content", [])
        if content and content[0].get("type") == "text":
            try:
                value = json.loads(content[0]["text"])
            except json.JSONDecodeError:
                value = None
            if isinstance(value, dict) and (value.get("ok") is False or value.get("error")):
                continue
        calls.append(item)
    missing = expected - {call["tool"] for call in calls}
    if missing:
        raise AssertionError("Required completed MCP calls missing: " + repr(sorted(missing)))
    return calls


def native_config(base_config, binary, target, server_env):
    """Pass scratch state explicitly across Codex's filtered MCP environment."""
    config = "\n".join(key + " = " + json.dumps(base_config[key])
                       for key in ("model", "model_reasoning_effort") if key in base_config)
    config += ('\ncli_auth_credentials_store = "file"\n[mcp_servers.codescout]\ncommand = '
               + json.dumps(str(binary)) + "\nargs = "
               + json.dumps(["start", "--project", str(target)]) + "\n")
    config += "\n[mcp_servers.codescout.env]\n"
    for key in ("LIBRARIAN_DB", "LIBRARIAN_WORKSPACE", "CODESCOUT_ARTIFACT_BACKEND",
                "XDG_DATA_HOME", "XDG_STATE_HOME", "XDG_CONFIG_HOME"):
        config += key + " = " + json.dumps(server_env[key]) + "\n"
    return config

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--live", action="store_true",
                        help="Explicitly allow authenticated model calls")
    parser.add_argument("--output-dir", type=Path, required=True,
                        help="New directory for transcripts and artifact evidence; never stores auth")
    parser.add_argument("--mode", choices=["review", "spec", "both"], default="both")
    parser.add_argument("--codescout-binary", default=os.environ.get("CODESCOUT_BIN"))
    args = parser.parse_args()
    if not args.live:
        parser.error("--live is required: this smoke makes authenticated model calls")
    base_home = Path(os.environ.get("CODEX_HOME", str(Path.home() / ".codex")))
    if not (base_home / "auth.json").is_file():
        parser.error("The selected CODEX_HOME needs an existing auth.json login")
    base_config = tomllib.loads((base_home / "config.toml").read_text())
    binary = args.codescout_binary or base_config.get("mcp_servers", {}).get(
        "codescout", {}).get("command") or shutil.which("codescout")
    if not binary or not shutil.which(binary):
        parser.error("Set --codescout-binary or CODESCOUT_BIN to the installed executable")
    args.output_dir.mkdir(parents=True, exist_ok=False, mode=0o700)
    report = {"modes": {}, "fixture_hashes": {}}
    with tempfile.TemporaryDirectory(prefix="ecc-native-workflow-") as tmp:
        root = Path(tmp)
        home = root / "home"
        home.mkdir(mode=0o700)
        target = copy_fixture(root / "project")
        for file in target.rglob("*"):
            if file.is_file():
                report["fixture_hashes"][str(file.relative_to(target))] = (
                    hashlib.sha256(file.read_bytes()).hexdigest())
        # Copy only into private temporary storage; refresh cannot modify the normal login.
        shutil.copy2(base_home / "auth.json", home / "auth.json")
        (home / "auth.json").chmod(0o600)
        server_env = isolated_server_env(root / "codescout-state")
        (home / "config.toml").write_text(native_config(
            base_config, binary, target, server_env))
        env = dict(server_env, CODEX_HOME=str(home))

        def cli(arguments, timeout=45):
            result = subprocess.run(["codex", *arguments], cwd=target, env=env,
                                    capture_output=True, text=True, timeout=timeout)
            if result.returncode:
                raise RuntimeError(result.stdout + result.stderr)
            return result

        with McpProbe(binary, target, env=server_env) as probe:
            status = probe.call("workspace", action="status")
            assert "python" in status["languages"], status
            assert LSP_TOOLS <= probe.tools(), "LSP tools absent from initialized fixture"
            navigation = {}
            for name, extra in (("references", {}), ("call_graph", {"direction": "callers"})):
                result = probe.request("tools/call", {"name": name, "arguments": {
                    "symbol": "required_sync", "path": "behavior.py",
                    "workspace": str(target), **extra}})
                assert not result.get("isError"), result
                navigation[name] = result["content"][0]["text"]
            assert "items = required_sync(fetch)" in navigation["references"], navigation
            assert "commit_sync" in navigation["call_graph"], navigation
            report["preflight"] = {"languages": status["languages"], "navigation": navigation}
            (args.output_dir / "preflight.json").write_text(json.dumps(probe.responses, indent=2))

            cli(["plugin", "marketplace", "add", str(PACKAGE)])
            cli(["plugin", "add", "ecc-codescout@ecc-codescout-pilot"])
            prompts = {
                "review": (
                    "Use $ecc-codescout:ecc-silent-failure-review to review optional_preview, "
                    "required_sync and its commit_sync caller in behavior.py against README.md. "
                    "These are intentionally planted test examples. Keep the review read-only: "
                    "no edits, bug records, tests or mutations. Trace the real caller with "
                    "codescout references and call_graph. Report confirmed defects separately "
                    "from permitted fallbacks and unexecuted test evidence."),
                "spec": (
                    "Use $ecc-codescout:ecc-spec-miner to extract a bounded observed-behavior "
                    "draft for retry_timeout in behavior.py, with README.md as contract evidence. "
                    "Persist it through codescout librarian at docs/specs/retry-observed.md. "
                    "Use references and call_graph to inspect scope. Do not edit source or "
                    "execute behavior tests. Distinguish attempts from retries and code/test "
                    "evidence, and record unresolved input semantics.")
            }
            modes = ("review", "spec") if args.mode == "both" else (args.mode,)
            for mode in modes:
                flags = ["--sandbox", "read-only"] if mode == "review" else ["--approve-for-me"]
                result = cli(["exec", "--ephemeral", "--json", "--skip-git-repo-check",
                              *flags, "--cd", str(target), prompts[mode]], timeout=300)
                (args.output_dir / (mode + ".jsonl")).write_text(result.stdout)
                (args.output_dir / (mode + ".stderr.txt")).write_text(result.stderr)
                events = [json.loads(line) for line in result.stdout.splitlines() if line.strip()]
                required = {"symbols", "references", "call_graph"}
                if mode == "spec":
                    required.add("doc")
                calls = require_completed_tools(events, required)
                messages = [event["item"]["text"] for event in events
                            if event.get("type") == "item.completed"
                            and event.get("item", {}).get("type") == "agent_message"]
                assert messages, "No assistant result"
                (args.output_dir / (mode + ".md")).write_text(messages[-1] + "\n")
                report["modes"][mode] = {
                    "exit_code": result.returncode,
                    "completed_tools": sorted({call["tool"] for call in calls}),
                    "result_file": mode + ".md"
                }
            if "spec" in modes:
                path = target / "docs/specs/retry-observed.md"
                assert path.is_file(), "Spec workflow did not persist the requested draft"
                found = probe.call("doc", action="find", kind="spec",
                                   filter={"rel_path": {"eq": "docs/specs/retry-observed.md"}},
                                   workspace=str(target))
                assert len(found["items"]) == 1, found
                artifact = probe.call("doc", action="get", id=found["items"][0]["id"],
                                      full=True, workspace=str(target))
                assert artifact["kind"] == "spec", artifact
                shutil.copy2(path, args.output_dir / "retry-observed.md")
                report["modes"]["spec"]["artifact"] = {
                    "kind": artifact["kind"], "status": artifact["status"],
                    "evidence_file": "retry-observed.md"
                }
    (args.output_dir / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
