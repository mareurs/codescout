"""CLI ownership tests use real dirty files, isolated histories and native namespaces."""
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys

import pytest

from test_harness_history import claude_call, claude_result, codex_meta, codex_call, codex_result, codex_item, history

SCRIPTS = Path(__file__).resolve().parents[1] / "scripts"


@pytest.fixture
def checkout(tmp_path):
    root = tmp_path / "checkout"
    root.mkdir()
    env = dict(os.environ)
    for key in ("CLAUDE_CODE_SESSION_ID", "CODEX_SESSION_ID", "CODEX_THREAD_ID", "CLAUDE_CONFIG_DIR",
                "CODESCOUT_CONTEXT_FILE", "REPO_ROOT"):
        env.pop(key, None)
    home = tmp_path / "home"
    home.mkdir()
    env.update(HOME=str(home), FILE_PROVENANCE_ROOTS=str(tmp_path / "claude"),
               FILE_PROVENANCE_CODEX_ROOTS_JSON=json.dumps([str(tmp_path / "codex")]),
               FILE_PROVENANCE_REGISTRY_ROOTS=str(tmp_path / "registry"))
    subprocess.run(["git", "init", "-q", str(root)], check=True)
    names = ["own.rs", "peer.rs", "shared.rs", "unknown.rs", "partial.rs", "unpaired.rs"]
    for name in names:
        (root / name).write_text("fn base() {}\n")
    subprocess.run(["git", "-C", str(root), "add", "."], check=True)
    subprocess.run(["git", "-C", str(root), "-c", "user.name=Fixture", "-c", "user.email=x@example.invalid",
                    "commit", "-q", "-m", "base"],
                   env=env | {"GIT_AUTHOR_DATE": "2026-10-01T12:00:00+00:00", "GIT_COMMITTER_DATE": "2026-10-01T12:00:00+00:00"},
                   check=True)
    for name in names:
        (root / name).write_text("fn modified(){}\n")
    return root, env


def add_writes(root, env, harness, sid, paths, **patch):
    if harness == "claude-code":
        rows = [claude_call(inp={"file_path": str(root / p)}, call=f"call-{i}", sid=sid, cwd=root, **patch)
                for i, p in enumerate(paths)]
        rows += [claude_result(call=f"call-{i}") for i, _ in enumerate(paths)]
        path = Path(env["FILE_PROVENANCE_ROOTS"]) / (sid + ".jsonl")
    else:
        rows = [codex_meta(root, sid=sid, thread=sid)]
        rows += [codex_call(inp={"path": str(root / p)}, call=f"call-{i}", ordinal=10+i) for i, p in enumerate(paths)]
        rows += [codex_result(call=f"call-{i}", ordinal=20+i) for i, _ in enumerate(paths)]
        path = Path(json.loads(env["FILE_PROVENANCE_CODEX_ROOTS_JSON"])[0]) / (sid + ".jsonl")
    return history(path, rows)


def invoke(root, env, *args):
    return subprocess.run([sys.executable, str(SCRIPTS / "file-provenance.py"), *args],
                          cwd=root, env=env, text=True, capture_output=True)


def verdicts(result):
    assert result.returncode in (0, 1), result.stderr
    return {line.split(maxsplit=1)[1]: line.split()[0] for line in result.stdout.splitlines()
            if line.split() and line.split()[0] in ("MINE", "PEER", "SHARED", "UNKNOWN", "CLEAN")}


@pytest.mark.parametrize("harness,identity", [("claude-code", "CLAUDE_CODE_SESSION_ID"), ("codex", "CODEX_SESSION_ID")])
def test_mixed_native_ownership_and_same_id_cross_host(checkout, harness, identity):
    root, env = checkout
    other = "codex" if harness == "claude-code" else "claude-code"
    add_writes(root, env, harness, "same:root", ["own.rs", "shared.rs"])
    add_writes(root, env, other, "same:root", ["peer.rs", "shared.rs"])
    result = invoke(root, env | {identity: "same:root"}, "own.rs", "peer.rs", "shared.rs", "unknown.rs")
    assert verdicts(result) == {"own.rs": "MINE", "peer.rs": "PEER", "shared.rs": "SHARED", "unknown.rs": "UNKNOWN"}
    assert harness + ":same:roo" in result.stdout
    assert other + ":same:root" in result.stdout
    if other == "codex":
        assert "contact unknown" in result.stdout
        assert "not live" not in result.stdout


def test_known_own_plus_gap_unknown_and_shared_plus_gap_retained(checkout):
    root, env = checkout
    add_writes(root, env, "claude-code", "me", ["own.rs", "shared.rs"])
    add_writes(root, env, "codex", "peer", ["shared.rs"])
    path = Path(json.loads(env["FILE_PROVENANCE_CODEX_ROOTS_JSON"])[0]) / "opaque.jsonl"
    history(path, [codex_meta(root, sid="opaque", thread="opaque"),
                  codex_item({"type": "custom_tool_call", "name": "exec", "call_id": "wrapper",
                              "input": 'await tools.apply_patch("peer.rs")'})])
    result = invoke(root, env | {"CLAUDE_CODE_SESSION_ID": "me"}, "own.rs", "shared.rs")
    assert verdicts(result) == {"own.rs": "UNKNOWN", "shared.rs": "SHARED"}
    assert "coverage" in result.stdout.lower()
    assert "claude-code:me" in result.stdout
    assert "codex:peer" in result.stdout


def test_foreign_workspace_gap_does_not_taint_known_own(checkout, tmp_path):
    root, env = checkout
    add_writes(root, env, "claude-code", "me", ["own.rs"])
    foreign = tmp_path / "foreign"
    foreign.mkdir()
    subprocess.run(["git", "init", "-q", str(foreign)], check=True)
    path = Path(json.loads(env["FILE_PROVENANCE_CODEX_ROOTS_JSON"])[0]) / "foreign.jsonl"
    history(path, [codex_meta(foreign), codex_item({"type": "custom_tool_call", "name": "exec", "call_id": "opaque", "input": "unknown"})])
    assert verdicts(invoke(root, env | {"CLAUDE_CODE_SESSION_ID": "me"}, "own.rs")) == {"own.rs": "MINE"}


def test_atomic_refusal_cannot_add_peer_but_shell_failure_and_unpaired_can(checkout):
    root, env = checkout
    add_writes(root, env, "claude-code", "me", ["own.rs"])
    path = Path(json.loads(env["FILE_PROVENANCE_CODEX_ROOTS_JSON"])[0]) / "peer.jsonl"
    history(path, [codex_meta(root, sid="peer", thread="peer"),
        codex_call(inp={"path": str(root / "own.rs")}, call="refused", ordinal=10),
        codex_result(call="refused", output='{"ok":false}', ordinal=11),
        codex_call("exec_command", {"cmd": "printf x > partial.rs; false"}, call="partial", ordinal=12),
        codex_result(call="partial", output='{"exit_code":1}', ordinal=13),
        codex_call("exec_command", {"cmd": "printf x > unpaired.rs"}, call="unpaired", ordinal=14),
    ])
    result = invoke(root, env | {"CLAUDE_CODE_SESSION_ID": "me"}, "own.rs", "partial.rs", "unpaired.rs")
    assert verdicts(result) == {"own.rs": "MINE", "partial.rs": "PEER", "unpaired.rs": "PEER"}


def test_clean_peer_history_preserves_git_clearance(checkout):
    root, env = checkout
    add_writes(root, env, "codex", "peer", ["peer.rs"])
    subprocess.run(["git", "-C", str(root), "restore", "--", "peer.rs"], check=True)
    result = invoke(root, env | {"CLAUDE_CODE_SESSION_ID": "me"}, "peer.rs")
    assert verdicts(result) == {"peer.rs": "CLEAN"}


def test_explicit_context_and_native_conflict_are_actionable(checkout, tmp_path):
    root, env = checkout
    add_writes(root, env, "codex", "manual-root", ["own.rs"])
    value = dict(schema_version=1, harness="codex", session_id="manual-root", agent_id=None,
                 workspace=str(root), source="explicit", harness_version=None)
    path = tmp_path / "context with spaces.json"
    path.write_text(json.dumps(value))
    assert verdicts(invoke(root, env, "--context", str(path), "own.rs")) == {"own.rs": "MINE"}
    result = invoke(root, env | {"CLAUDE_CODE_SESSION_ID": "other"}, "--context", str(path), "own.rs")
    assert result.returncode == 2
    assert "conflicting" in result.stderr
    missing = invoke(root, env, "own.rs")
    assert missing.returncode == 2
    assert "--context" in missing.stderr or "--check" in missing.stderr


def test_duplicate_discovery_copied_original_event_and_gaps(checkout):
    root, env = checkout
    source = add_writes(root, env, "codex", "me", ["own.rs"])
    history(source.parent / "copy.jsonl", [json.loads(line) for line in source.read_text().splitlines()])
    spec = importlib.util.spec_from_file_location("fp_scan", SCRIPTS / "file-provenance.py")
    fp = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(fp)
    assert hasattr(fp, "scan_provenance"), "mixed-host scan is not implemented"
    scan = fp.scan_provenance(root, env)
    assert scan.owners["own.rs"] == [("codex:me", "2026-10-10T12:01:00Z")]
    assert not scan.gaps
    assert len(fp.scan_with_evidence(root)) == 2


def test_unreadable_codex_source_cannot_hide_behind_claude_scan(checkout, monkeypatch):
    root, env = checkout
    add_writes(root, env, "claude-code", "me", ["own.rs"])
    source = add_writes(root, env, "codex", "peer", ["own.rs"])
    spec = importlib.util.spec_from_file_location("fp_unreadable", SCRIPTS / "file-provenance.py")
    fp = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(fp)
    assert hasattr(fp, "scan_provenance"), "coverage-aware scan is not implemented"
    original = Path.open
    def deny(self, *args, **kwargs):
        if self == source:
            raise PermissionError("fixture unreadable")
        return original(self, *args, **kwargs)
    monkeypatch.setattr(Path, "open", deny)
    scan = fp.scan_provenance(root, env)
    assert scan.owners["own.rs"][0][0] == "claude-code:me"
    assert scan.gaps and any("unreadable" in gap.reason for gap in scan.gaps)

def test_hidden_codex_peer_downgrades_claude_own_dirty_file(checkout):
    root, env = checkout
    add_writes(root, env, "claude-code", "me", ["own.rs"])
    source = add_writes(root, env, "codex", "peer", ["own.rs"])
    rows = [json.loads(line) for line in source.read_text().splitlines()]
    rows[1]["timestamp"] = "2026-09-30T12:00:00Z"
    history(source, rows)
    result = invoke(root, env | {"CLAUDE_CODE_SESSION_ID": "me"}, "own.rs")
    assert verdicts(result) == {"own.rs": "SHARED"}
    assert "predate the window" in result.stdout


def test_absorbed_codex_peer_text_preserves_claude_own_clearance(checkout):
    root, env = checkout
    committed = "pub fn committed_unique_statement_for_absorption() -> &'static str {\n    \"this is a meaningful committed block whose precise bytes distinguish its author\"\n}\n"
    (root / "own.rs").write_text(committed)
    subprocess.run(["git", "-C", str(root), "add", "own.rs"], check=True)
    subprocess.run(["git", "-C", str(root), "-c", "user.name=Fixture", "-c", "user.email=x@example.invalid",
                    "commit", "-q", "-m", "meaningful base"],
                   env=env | {"GIT_AUTHOR_DATE": "2026-10-01T12:00:00+00:00", "GIT_COMMITTER_DATE": "2026-10-01T12:00:00+00:00"},
                   check=True)
    (root / "own.rs").write_text("fn modified(){}\n")
    add_writes(root, env, "claude-code", "me", ["own.rs"])
    source = Path(json.loads(env["FILE_PROVENANCE_CODEX_ROOTS_JSON"])[0]) / "peer.jsonl"
    call = codex_call(inp={"path": str(root / "own.rs"), "new_string": committed})
    call["timestamp"] = "2026-10-01T12:00:00Z"
    history(source, [codex_meta(root, sid="peer", thread="peer"), call, codex_result()])
    result = invoke(root, env | {"CLAUDE_CODE_SESSION_ID": "me"}, "own.rs")
    assert verdicts(result) == {"own.rs": "MINE"}
    assert "already in HEAD" in result.stdout


def test_child_activation_uncertainty_can_be_resolved_by_absolute_workspace_pin(checkout):
    root, env = checkout
    parent = history(Path(env["FILE_PROVENANCE_ROOTS"]) / "me.jsonl", [
        claude_call("mcp__codescout__create_file", {"path": "own.rs", "workspace": str(root)}, sid="me", cwd=root,
                    timestamp="2026-10-10T12:10:00Z"),
    ])
    history(parent.parent / "me/subagents/agent-child.jsonl", [
        claude_call("mcp__codescout__workspace", {"action": "activate", "path": "foreign-id"}, sid="me", cwd=root,
                    timestamp="2026-10-10T12:00:00Z"),
    ])
    assert verdicts(invoke(root, env | {"CLAUDE_CODE_SESSION_ID": "me"}, "own.rs")) == {"own.rs": "MINE"}
    rows = [json.loads(line) for line in parent.read_text().splitlines()]
    del rows[0]["message"]["content"][0]["input"]["workspace"]
    history(parent, rows)
    result = invoke(root, env | {"CLAUDE_CODE_SESSION_ID": "me"}, "own.rs")
    assert verdicts(result) == {"own.rs": "UNKNOWN"}
    assert "ambiguous" in result.stdout
def test_unverified_fork_peer_cannot_leave_sole_own_clearance(checkout):
    root, env = checkout
    add_writes(root, env, "claude-code", "me", ["own.rs"])
    meta = codex_meta(root, sid="fork", thread="fork", forked_from_id="parent",
                      forked_from_ordinal_exclusive=10)
    meta["payload"]["history_mode"] = "legacy"
    call = codex_call(inp={"path": str(root / "own.rs")}, call="new-write")
    meta.pop("ordinal")
    call.pop("ordinal")
    path = Path(json.loads(env["FILE_PROVENANCE_CODEX_ROOTS_JSON"])[0]) / "fork.jsonl"
    history(path, [meta, call])
    result = invoke(root, env | {"CLAUDE_CODE_SESSION_ID": "me"}, "own.rs")
    assert verdicts(result) == {"own.rs": "UNKNOWN"}, result.stdout


@pytest.mark.parametrize("name", ["mcp__filesystem__write_file", "mcp__codescout__future_write"])
def test_unsupported_mcp_writer_preserves_coverage_gap(checkout, name):
    root, env = checkout
    add_writes(root, env, "claude-code", "me", ["own.rs"])
    path = Path(json.loads(env["FILE_PROVENANCE_CODEX_ROOTS_JSON"])[0]) / "peer-mcp.jsonl"
    history(path, [codex_meta(root, sid="peer", thread="peer"),
                   codex_call(name, {"path": str(root / "own.rs"), "content": "peer bytes"})])
    result = invoke(root, env | {"CLAUDE_CODE_SESSION_ID": "me"}, "own.rs")
    assert verdicts(result) == {"own.rs": "UNKNOWN"}, result.stdout
