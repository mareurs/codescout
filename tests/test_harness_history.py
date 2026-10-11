"""Native readers preserve producer identity and uncertainty before policy runs."""
import importlib
import json
from pathlib import Path
import subprocess
import sys

import pytest

SCRIPTS = Path(__file__).resolve().parents[1] / "scripts"
sys.path.insert(0, str(SCRIPTS))


def claude_call(name="Edit", inp=None, call="call-1", sid="root-1", cwd=None, timestamp="2026-10-10T12:00:00Z"):
    rec = {"type": "assistant", "sessionId": sid, "message": {"content": [{
        "type": "tool_use", "name": name, "input": inp or {"file_path": "own.rs"}, "id": call,
    }]}}
    if timestamp is not None:
        rec["timestamp"] = timestamp
    if cwd is not None:
        rec["cwd"] = str(cwd)
    return rec


def claude_result(call="call-1", **patch):
    return {"type": "user", "message": {"content": [{
        "type": "tool_result", "tool_use_id": call, "content": "done", **patch,
    }]}}


def history(path, rows):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("".join(json.dumps(row) + "\n" for row in rows))
    return path


@pytest.fixture
def claude():
    assert (SCRIPTS / "claude_history.py").is_file(), "Claude normalized reader is absent"
    return importlib.import_module("claude_history")


def test_claude_root_and_child_share_principal(claude, tmp_path):
    parent = history(tmp_path / "root-1.jsonl", [claude_call(cwd=tmp_path)])
    child = history(tmp_path / "root-1/subagents/agent-child.jsonl", [claude_call(cwd=tmp_path)])
    root_op = claude.read_claude_history(parent).operations[0]
    child_op = claude.read_claude_history(child).operations[0]
    assert root_op.principal == child_op.principal == "claude-code:root-1"
    assert root_op.agent_id is None
    assert child_op.agent_id == "child"
    assert not root_op.is_child and child_op.is_child
    assert root_op.source_id != child_op.source_id


@pytest.mark.parametrize("result", [
    {"is_error": True},
    {"content": '{"ok":false,"error":"old string missing"}'},
    {"content": [{"type": "text", "text": '{"pending_ack":"@ack_fixture"}'}]},
])
def test_claude_confirmed_atomic_refusal(claude, tmp_path, result):
    path = history(tmp_path / "root-1.jsonl", [claude_call(), claude_result(**result)])
    op = claude.read_claude_history(path).operations[0]
    assert op.outcome == "no-write-refused"
    assert op.atomic


def test_claude_failed_shell_remains_possible_write(claude, tmp_path):
    path = history(tmp_path / "root-1.jsonl", [
        claude_call("Bash", {"command": "printf changed > own.rs; false"}),
        claude_result(is_error=True),
    ])
    op = claude.read_claude_history(path).operations[0]
    assert op.outcome == "partial-error"
    assert not op.atomic
    assert op.input["command"] == "printf changed > own.rs; false"


def test_claude_unpaired_and_undated_calls_survive(claude, tmp_path):
    path = history(tmp_path / "root-1.jsonl", [claude_call(timestamp=None)])
    op = claude.read_claude_history(path).operations[0]
    assert op.outcome == "missing-result"
    assert op.timestamp is None


def test_claude_reordered_results_join_by_call_id(claude, tmp_path):
    path = history(tmp_path / "root-1.jsonl", [
        claude_call(call="first"), claude_call(call="second"),
        claude_result(call="second", is_error=True), claude_result(call="first"),
    ])
    assert [(op.call_id, op.outcome) for op in claude.read_claude_history(path).operations] == [
        ("first", "possible-write"), ("second", "no-write-refused"),
    ]


def test_claude_truncated_record_is_coverage_gap(claude, tmp_path):
    path = history(tmp_path / "root-1.jsonl", [claude_call()])
    with path.open("a") as out:
        out.write('{"message":{"content":[{"type":"tool_use"')
    batch = claude.read_claude_history(path)
    assert len(batch.operations) == 1
    assert batch.gaps
    assert "malformed" in batch.gaps[0].reason


def test_claude_unreadable_history_not_absent(claude, tmp_path, monkeypatch):
    path = history(tmp_path / "root-1.jsonl", [claude_call()])
    original = Path.open
    def deny(self, *args, **kwargs):
        if self == path:
            raise PermissionError("fixture unreadable")
        return original(self, *args, **kwargs)
    monkeypatch.setattr(Path, "open", deny)
    batch = claude.read_claude_history(path)
    assert not batch.operations
    assert batch.gaps
    assert "unreadable" in batch.gaps[0].reason


def test_claude_profiles_custom_dir_and_linked_main(claude, tmp_path):
    root = tmp_path / "repo"
    root.mkdir()
    subprocess.run(["git", "init", "-q", str(root)], check=True)
    subprocess.run(["git", "-C", str(root), "-c", "user.name=Fixture", "-c", "user.email=x@example.invalid",
                    "commit", "-q", "--allow-empty", "-m", "base"], check=True)
    linked = tmp_path / "linked"
    subprocess.run(["git", "-C", str(root), "worktree", "add", "-q", "--detach", str(linked)], check=True)
    home = tmp_path / "home"
    slug = __import__("re").sub(r"[^A-Za-z0-9]", "-", str(root.resolve()))
    a = history(home / ".claude/projects" / slug / "a.jsonl", [claude_call(sid="a")])
    b = history(home / ".claude-sdd/projects" / slug / "b/subagents/agent-b.jsonl", [claude_call(sid="b")])
    c = history(tmp_path / "custom-profile/projects" / slug / "c.jsonl", [claude_call(sid="c")])
    paths, gaps = claude.discover_claude_histories(linked, {"HOME": str(home), "CLAUDE_CONFIG_DIR": str(c.parents[2])})
    assert set(paths) == {a.resolve(), b.resolve(), c.resolve()}
    assert not gaps


def test_claude_unreadable_profile_not_absent(claude, tmp_path, monkeypatch):
    home = tmp_path / "home"
    profile = home / ".claude"
    profile.mkdir(parents=True)
    original = Path.iterdir
    def deny(self):
        if self == profile:
            raise PermissionError("fixture unreadable profile")
        return original(self)
    monkeypatch.setattr(Path, "iterdir", deny)
    paths, gaps = claude.discover_claude_histories(tmp_path / "repo", {"HOME": str(home)})
    assert not paths
    assert gaps
    assert any("unreadable" in gap.reason for gap in gaps)


def test_claude_alias_discovery_deduplicates_source_not_equal_inputs(claude, tmp_path):
    records = tmp_path / "history"
    path = history(records / "root-1.jsonl", [claude_call(call="a"), claude_call(call="b")])
    alias = tmp_path / "alias"
    alias.symlink_to(records, target_is_directory=True)
    paths, gaps = claude.discover_claude_histories(tmp_path, {
        "FILE_PROVENANCE_ROOTS": str(records) + ":" + str(alias),
    })
    assert paths == [path.resolve()]
    assert not gaps
    assert [op.call_id for op in claude.read_claude_history(path).operations] == ["a", "b"]


def test_claude_same_call_id_in_two_children_is_not_same_producer(claude, tmp_path):
    a = history(tmp_path / "root-1/subagents/agent-a.jsonl", [claude_call()])
    b = history(tmp_path / "root-1/subagents/agent-b.jsonl", [claude_call()])
    assert claude.read_claude_history(a).operations[0].source_id != claude.read_claude_history(b).operations[0].source_id

def codex_meta(root, sid="root-1", thread="root-1", **patch):
    return {"timestamp": "2026-10-10T12:00:00Z", "ordinal": 0, "type": "session_meta",
            "payload": {"session_id": sid, "id": thread, "cwd": str(root),
                        "source": "exec", "cli_version": "0.160.0",
                        "history_mode": "paginated", **patch}}


def codex_item(payload, ordinal=10, timestamp="2026-10-10T12:01:00Z"):
    row = {"type": "response_item", "payload": payload, "ordinal": ordinal}
    if timestamp is not None:
        row["timestamp"] = timestamp
    return row


def codex_call(name="mcp__codescout__edit_file", inp=None, call="call-1", ordinal=10, **patch):
    return codex_item({"type": "function_call", "name": name, "call_id": call,
                       "arguments": json.dumps(inp or {"path": "own.rs", "new_string": "updated"}),
                       **patch}, ordinal)


def codex_result(call="call-1", output="done", ordinal=11, **patch):
    return codex_item({"type": "function_call_output", "call_id": call,
                       "output": output, **patch}, ordinal)


@pytest.fixture
def codex():
    assert (SCRIPTS / "codex_history.py").is_file(), "Codex normalized reader is absent"
    return importlib.import_module("codex_history")


def test_codex_function_arguments_native_cwd_and_raw_precision(codex, tmp_path):
    raw = '{"path":"own.rs","ratio":0.123456789123456789,"new_string":"updated"}'
    path = history(tmp_path / "rollout.jsonl", [codex_meta(tmp_path),
        codex_call(arguments=raw), codex_result()])
    op = codex.read_codex_history(path).operations[0]
    assert op.principal == "codex:root-1"
    assert op.name == "mcp__codescout__edit_file"
    assert op.cwd == tmp_path
    assert op.raw_input == raw
    assert str(op.input["ratio"]) == "0.123456789123456789"
    assert op.outcome == "possible-write"


@pytest.mark.parametrize("output", [
    '{"ok":false,"error":"missing old string"}',
    [{"type": "input_text", "text": '{"pending_ack":"@ack_fixture"}'}],
])
def test_codex_atomic_refusals_in_native_outputs(codex, tmp_path, output):
    path = history(tmp_path / "rollout.jsonl", [
        codex_meta(tmp_path), codex_call(), codex_result(output=output),
    ])
    assert codex.read_codex_history(path).operations[0].outcome == "no-write-refused"


def test_codex_shell_failure_retains_authoritative_workdir(codex, tmp_path):
    cwd = tmp_path / "subdir"
    path = history(tmp_path / "rollout.jsonl", [
        codex_meta(tmp_path),
        codex_call("exec_command", {"cmd": "printf x > own.rs; false", "workdir": str(cwd)}),
        codex_result(output='{"exit_code":1}'),
    ])
    op = codex.read_codex_history(path).operations[0]
    assert op.name == "Bash"
    assert op.input["command"] == "printf x > own.rs; false"
    assert op.cwd == cwd
    assert not op.atomic
    assert op.outcome == "partial-error"


def test_codex_child_inherited_prefix_uses_ordinals_not_line_count(codex, tmp_path):
    source = {"subagent": {"thread_spawn": {"parent_thread_id": "root-1", "depth": 1}}}
    path = history(tmp_path / "child.jsonl", [
        codex_meta(tmp_path, thread="child-1", source=source, parent_thread_id="root-1",
                   subagent_history_start_ordinal=100),
        codex_call(call="parent-write", ordinal=90),
        codex_item({"type": "message", "role": "assistant", "content": []}, ordinal=99),
        codex_call(call="child-write", ordinal=100),
        codex_result(call="child-write", ordinal=101),
    ])
    batch = codex.read_codex_history(path)
    assert {op.call_id for op in batch.operations} == {"child-write"}
    op = batch.operations[0]
    assert op.principal == "codex:root-1" and op.agent_id == "child-1"
    assert op.is_child
    assert not batch.gaps


@pytest.mark.parametrize("patch,rows", [
    ({}, [codex_call(ordinal=90)]),
    ({}, [dict(type="response_item", payload=codex_call()["payload"])]),
    ({"subagent_history_start_ordinal": None}, [codex_call(ordinal=100)]),
])
def test_codex_incomplete_child_boundary_yields_gap(codex, tmp_path, patch, rows):
    source = {"subagent": {"thread_spawn": {"parent_thread_id": "root-1", "depth": 1}}}
    meta_fields = {"source": source, "parent_thread_id": "root-1",
                   "subagent_history_start_ordinal": 100, **patch}
    path = history(tmp_path / "child.jsonl", [
        codex_meta(tmp_path, thread="child-1", **meta_fields), *rows,
    ])
    batch = codex.read_codex_history(path)
    assert batch.gaps
    assert not batch.operations


def test_codex_different_thread_alone_does_not_manufacture_child(codex, tmp_path):
    path = history(tmp_path / "fork.jsonl", [
        codex_meta(tmp_path, thread="different"), codex_call(),
    ])
    batch = codex.read_codex_history(path)
    assert batch.gaps
    assert not batch.operations


def test_codex_fork_copied_prefix_is_not_new_root_authorship(codex, tmp_path):
    path = history(tmp_path / "fork.jsonl", [
        codex_meta(tmp_path, sid="fork-root", thread="fork-root", forked_from_id="parent-root",
                   forked_from_ordinal_exclusive=100),
        codex_call(call="parent-write", ordinal=90),
        codex_call(call="fork-write", ordinal=100),
        codex_result(call="fork-write", ordinal=101),
    ])
    batch = codex.read_codex_history(path)
    assert [(op.call_id, op.principal, op.is_child) for op in batch.operations] == [
        ("fork-write", "codex:fork-root", False),
    ]
@pytest.mark.parametrize("case", ["legacy", "missing-ordinal", "incomplete-prefix"])
def test_codex_unverified_fork_boundary_is_gap(codex, tmp_path, case):
    meta = codex_meta(tmp_path, sid="fork", thread="fork", forked_from_id="parent",
                      forked_from_ordinal_exclusive=10)
    call = codex_call(call="new-write", ordinal=10)
    if case == "legacy":
        meta["payload"]["history_mode"] = "legacy"
        meta.pop("ordinal")
        call.pop("ordinal")
    elif case == "missing-ordinal":
        call.pop("ordinal")
    else:
        call["ordinal"] = 2
    batch = codex.read_codex_history(history(tmp_path / "fork.jsonl", [meta, call]))
    assert not batch.operations
    assert batch.gaps
    assert any("fork" in gap.reason for gap in batch.gaps)


def test_codex_results_join_within_producer(codex, tmp_path):
    source = {"subagent": {"thread_spawn": {"parent_thread_id": "root-1", "depth": 1}}}
    def child(name, output):
        return history(tmp_path / (name + ".jsonl"), [
            codex_meta(tmp_path, thread=name, source=source, parent_thread_id="root-1",
                       subagent_history_start_ordinal=0),
            codex_call(call="same"), codex_result(call="same", output=output),
        ])
    a = codex.read_codex_history(child("a", "done")).operations[0]
    b = codex.read_codex_history(child("b", '{"ok":false}')).operations[0]
    assert a.source_id != b.source_id
    assert a.outcome == "possible-write"
    assert b.outcome == "no-write-refused"
    reordered = history(tmp_path / "root.jsonl", [codex_meta(tmp_path),
        codex_call(call="a", ordinal=10), codex_call(call="b", ordinal=11),
        codex_result(call="b", output='{"ok":false}', ordinal=12),
        codex_result(call="a", ordinal=13)])
    assert [(op.call_id, op.outcome) for op in codex.read_codex_history(reordered).operations] == [
        ("a", "possible-write"), ("b", "no-write-refused"),
    ]


def test_codex_conflicting_replay_is_unknown(codex, tmp_path):
    path = history(tmp_path / "root.jsonl", [codex_meta(tmp_path),
        codex_call(inp={"path": "a.rs"}), codex_call(inp={"path": "b.rs"}, ordinal=12)])
    assert codex.read_codex_history(path).gaps


def test_codex_unpaired_undated_call_survives(codex, tmp_path):
    call = codex_call()
    call.pop("timestamp")
    path = history(tmp_path / "root.jsonl", [codex_meta(tmp_path), call])
    op = codex.read_codex_history(path).operations[0]
    assert op.outcome == "missing-result" and op.timestamp is None


@pytest.mark.parametrize("patch", [{"session_id": None}, {"id": None}, {"cwd": "relative"}])
def test_codex_missing_root_metadata_is_gap(codex, tmp_path, patch):
    path = history(tmp_path / "root.jsonl", [codex_meta(tmp_path, **patch), codex_call()])
    batch = codex.read_codex_history(path)
    assert batch.gaps and not batch.operations


def test_codex_bad_arguments_and_truncation_are_gaps(codex, tmp_path):
    path = history(tmp_path / "root.jsonl", [codex_meta(tmp_path), codex_call(arguments="{broken")])
    with path.open("a") as stream:
        stream.write('{"type":"response_item"')
    batch = codex.read_codex_history(path)
    assert len(batch.gaps) >= 2
    assert not batch.operations


def test_codex_captured_opaque_wrapper_is_not_inferred_execution(codex, tmp_path):
    fixture = Path(__file__).parent / "fixtures/native-histories/codex-root-refused-wrapper.jsonl"
    path = tmp_path / "capture.jsonl"
    path.write_text(fixture.read_text().replace("${WORKSPACE}", str(tmp_path)))
    batch = codex.read_codex_history(path)
    assert not batch.operations
    assert any("wrapper" in gap.reason for gap in batch.gaps)


def test_codex_patch_custom_call_and_read_message(codex, tmp_path):
    patch = "*** Begin Patch\n*** Update File: old name.rs\n*** Move to: new name.rs\n@@\n-old\n+new\n*** Delete File: gone.rs\n*** Add File: added.rs\n+x\n*** End Patch"
    path = history(tmp_path / "root.jsonl", [codex_meta(tmp_path),
        codex_item({"type": "message", "role": "assistant", "content": [{"type": "output_text", "text": patch}]}),
        codex_item({"type": "custom_tool_call", "call_id": "patch", "name": "apply_patch", "input": patch}, ordinal=11)])
    batch = codex.read_codex_history(path)
    assert len(batch.operations) == 1
    assert batch.operations[0].name == "apply_patch"
    assert batch.operations[0].input["patch"] == patch
    assert not batch.operations[0].atomic


def test_codex_discovery_profiles_override_and_alias(codex, tmp_path):
    home = tmp_path / "home"
    a = history(home / ".codex/sessions/date/a.jsonl", [codex_meta(tmp_path)])
    b = history(home / ".Codex-sdd/sessions/date/b.jsonl", [codex_meta(tmp_path)])
    c = history(home / ".Codex-kat/archived_sessions/c.jsonl", [codex_meta(tmp_path)])
    custom = tmp_path / "custom"
    d = history(custom / "sessions/date/d.jsonl", [codex_meta(tmp_path)])
    paths, gaps = codex.discover_codex_histories(tmp_path, {"HOME": str(home), "CODEX_HOME": str(custom)})
    assert set(paths) == {a.resolve(), b.resolve(), c.resolve(), d.resolve()}
    assert not gaps
    alias = tmp_path / "alias"
    alias.symlink_to(custom / "sessions", target_is_directory=True)
    paths, gaps = codex.discover_codex_histories(tmp_path, {
        "FILE_PROVENANCE_CODEX_ROOTS_JSON": json.dumps([str(custom / "sessions"), str(alias)]),
    })
    assert paths == [d.resolve()] and not gaps
    assert codex.discover_codex_histories(tmp_path, {"FILE_PROVENANCE_CODEX_ROOTS_JSON": "[]"}) == ([], [])


@pytest.mark.parametrize("override", ["broken", "{}", '["relative"]'])
def test_codex_bad_root_override_does_not_fall_back_to_live_profiles(codex, tmp_path, override):
    paths, gaps = codex.discover_codex_histories(tmp_path, {"FILE_PROVENANCE_CODEX_ROOTS_JSON": override})
    assert not paths and gaps

def test_codex_patch_target_extraction_preserves_both_move_ends_and_spaces(tmp_path):
    import importlib.util
    spec = importlib.util.spec_from_file_location("patch_provenance", SCRIPTS / "file-provenance.py")
    fp = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(fp)
    assert hasattr(fp, "patch_write_targets"), "native patch targeting is not implemented"
    patch = "*** Begin Patch\n*** Update File: old name.rs\n*** Move to: new name.rs\n@@\n-old\n+new\n*** Delete File: gone.rs\n*** Add File: added file.rs\n+x\n*** End Patch"
    expected = ["old name.rs", "new name.rs", "gone.rs", "added file.rs"]
    assert fp.patch_write_targets(patch) == expected
    assert list(fp.write_targets("apply_patch", {"patch": patch}, tmp_path)) == expected
    assert list(fp.write_targets("mcp__codescout__read_file", {"path": "note.md", "content": patch}, tmp_path)) == []
    assert fp.patch_write_targets("merely mentions *** Update File: peer.rs") == []


def test_codex_unreadable_profile_is_coverage_gap(codex, tmp_path, monkeypatch):
    home = tmp_path / "home"
    profile = home / ".Codex"
    profile.mkdir(parents=True)
    original = Path.iterdir
    def deny(self):
        if self == profile:
            raise PermissionError("fixture unreadable")
        return original(self)
    monkeypatch.setattr(Path, "iterdir", deny)
    paths, gaps = codex.discover_codex_histories(tmp_path, {"HOME": str(home)})
    assert not paths
    assert gaps and any("unreadable" in gap.reason for gap in gaps)


def test_codex_unreadable_rollout_and_unknown_response_are_gaps(codex, tmp_path, monkeypatch):
    path = history(tmp_path / "root.jsonl", [codex_meta(tmp_path),
        codex_item({"type": "local_shell_call", "action": {"command": ["sh", "-c", "touch own.rs"]}})])
    assert codex.read_codex_history(path).gaps
    original = Path.open
    def deny(self, *args, **kwargs):
        if self == path:
            raise PermissionError("fixture unreadable")
        return original(self, *args, **kwargs)
    monkeypatch.setattr(Path, "open", deny)
    batch = codex.read_codex_history(path)
    assert not batch.operations and batch.gaps

def test_codex_unresolved_external_prefix_is_coverage_gap(codex, tmp_path):
    path = history(tmp_path / "root.jsonl", [codex_meta(tmp_path, history_base={
        "thread_id": "missing-prefix", "end_ordinal_exclusive": 90, "end_byte_offset": 500,
    }), codex_call(ordinal=100)])
    batch = codex.read_codex_history(path)
    assert any("prefix" in gap.reason for gap in batch.gaps)
    assert batch.operations[0].principal == "codex:root-1"
