"""Exercise actual paginated McpToolCall envelope shape with bounded synthetic writes."""
import importlib
import json
from pathlib import Path
import sys

import pytest

from test_harness_provenance import checkout, invoke, verdicts
from test_harness_formatter import project, run_fmt, snapshot

SCRIPTS = Path(__file__).resolve().parents[1] / "scripts"
sys.path.insert(0, str(SCRIPTS))
codex = importlib.import_module("codex_history")


def meta(root, **patch):
    return {"type":"session_meta", "ordinal":0, "timestamp":"2026-10-11T15:42:17.771Z",
            "payload":{"id":"root-1", "session_id":"root-1", "cwd":str(root),
                       "source":"vscode", "cli_version":"0.160.0", "history_mode":"paginated", **patch}}


def mcp_item(root, *, call="exec-write-1", tool="create_file", arguments=None,
             result=True, ordinal=10, thread="root-1", **patch):
    item = {"type":"McpToolCall", "id":call, "server":"codescout", "tool":tool,
            "arguments":arguments if arguments is not None else {"path":"own.rs", "content":"fn own() {}"},
            "status":"completed", "result":{"content":[{"type":"text", "text":'{"ok":true}'}], "isError":False}
                if result is True else result, "error":None, **patch}
    return {"type":"event_msg", "ordinal":ordinal, "timestamp":"2026-10-11T15:42:32.363Z",
            "payload":{"type":"item_completed", "thread_id":thread, "turn_id":"turn-1",
                       "started_at_ms":1791733352363, "completed_at_ms":1791733352363, "item":item}}


def read(tmp_path, rows):
    path = tmp_path / "native.jsonl"
    path.write_text("".join(json.dumps(row) + "\n" for row in rows))
    return codex.read_codex_history(path)


def test_native_mcp_write_retains_native_call_root_arguments_and_result(tmp_path):
    batch = read(tmp_path, [meta(tmp_path), mcp_item(tmp_path)])
    assert not batch.gaps
    assert len(batch.operations) == 1
    op = batch.operations[0]
    assert (op.call_id, op.principal, op.agent_id, op.name, op.cwd, op.outcome, op.atomic) == (
        "exec-write-1", "codex:root-1", None, "mcp__codescout__create_file", tmp_path, "possible-write", True)
    assert op.input == {"path":"own.rs", "content":"fn own() {}"}
    assert op.timestamp == "2026-10-11T15:42:32.363Z"


def test_native_child_parent_copy_uses_native_child_boundary_not_fork_boundary(tmp_path):
    source = {"subagent":{"thread_spawn":{"parent_thread_id":"root-1", "depth":1}}}
    batch = read(tmp_path, [
        meta(tmp_path, id="child-1", source=source, parent_thread_id="root-1",
             forked_from_id="root-1", subagent_history_start_ordinal=10),
        mcp_item(tmp_path, call="parent-write", ordinal=1),
        mcp_item(tmp_path, call="child-write", ordinal=10, thread="child-1"),
    ])
    assert not batch.gaps
    assert [op.call_id for op in batch.operations] == ["child-write"]
    assert (batch.operations[0].principal, batch.operations[0].agent_id, batch.operations[0].is_child) == (
        "codex:root-1", "child-1", True)


@pytest.mark.parametrize("patch", [
    {"id":"child-1", "source":{"subagent":{"thread_spawn":{"parent_thread_id":"parent-1"}}},
     "parent_thread_id":"parent-1", "forked_from_id":"other-parent", "subagent_history_start_ordinal":10},
    {"id":"child-1", "source":{"subagent":{"thread_spawn":{"parent_thread_id":"root-1"}}},
     "parent_thread_id":"root-1", "forked_from_id":"root-1", "subagent_history_start_ordinal":None},
])
def test_native_child_conflict_or_missing_boundary_cannot_claim_authorship(tmp_path, patch):
    batch = read(tmp_path, [meta(tmp_path, **patch), mcp_item(tmp_path, thread="child-1")])
    assert not batch.operations
    assert batch.gaps


@pytest.mark.parametrize("tool,body,outcome,atomic", [
    ("create_file", {"ok":False, "error":"refused"}, "no-write-refused", True),
    ("run_command", {"exit_code":1, "stdout":"partial write"}, "partial-error", False),
    ("run_command", {"ok":False, "error":"refused"}, "partial-error", False),
])
def test_native_mcp_refusal_and_failed_shell_keep_different_write_outcomes(tmp_path, tool, body, outcome, atomic):
    result = {"content":[{"type":"text", "text":json.dumps(body)}], "isError":False}
    batch = read(tmp_path, [meta(tmp_path), mcp_item(tmp_path, tool=tool,
        arguments={"command":"touch partial.rs"} if tool=="run_command" else None, result=result)])
    assert len(batch.operations) == 1
    assert (batch.operations[0].outcome, batch.operations[0].atomic) == (outcome, atomic)


def test_native_mcp_missing_result_retains_possible_writer(tmp_path):
    batch = read(tmp_path, [meta(tmp_path), mcp_item(tmp_path, result=None)])
    assert len(batch.operations) == 1
    assert batch.operations[0].outcome == "missing-result"


def test_native_mcp_duplicate_keeps_one_call_but_distinct_ids_keep_equal_inputs(tmp_path):
    same = mcp_item(tmp_path)
    batch = read(tmp_path, [meta(tmp_path), same, same, mcp_item(tmp_path, call="exec-write-2", ordinal=11)])
    assert not batch.gaps
    assert [op.call_id for op in batch.operations] == ["exec-write-1", "exec-write-2"]


def test_native_mcp_conflicting_call_result_retains_gap(tmp_path):
    batch = read(tmp_path, [meta(tmp_path), mcp_item(tmp_path), mcp_item(tmp_path, ordinal=11, result=None)])
    assert batch.gaps
    assert len(batch.operations) == 1


@pytest.mark.parametrize("patch", [{"server":"foreign"}, {"tool":"future_write"}, {"arguments":"wrong"}, {"id":None}])
def test_unknown_or_malformed_native_mcp_has_coverage_gap(tmp_path, patch):
    batch = read(tmp_path, [meta(tmp_path), mcp_item(tmp_path, **patch)])
    assert not batch.operations
    assert batch.gaps


def test_native_mcp_thread_mismatch_does_not_borrow_root_identity(tmp_path):
    batch = read(tmp_path, [meta(tmp_path), mcp_item(tmp_path, thread="other-thread")])
    assert not batch.operations
    assert batch.gaps


def test_recovered_inner_call_does_not_clear_opaque_wrapper_coverage(tmp_path):
    wrapper = {"type":"response_item", "ordinal":9, "timestamp":"2026-10-11T15:42:31Z",
               "payload":{"type":"custom_tool_call", "name":"exec", "namespace":"functions",
                          "call_id":"outer-call", "input":"await tools.mcp__codescout__create_file(...);"}}
    batch = read(tmp_path, [meta(tmp_path), wrapper, mcp_item(tmp_path)])
    assert len(batch.operations) == 1
    assert batch.operations[0].principal == "codex:root-1"
    assert any("wrapper" in gap.reason for gap in batch.gaps)
def native_writes(root, env, owner, names, *, opaque=False):
    path = Path(json.loads(env["FILE_PROVENANCE_CODEX_ROOTS_JSON"])[0]) / (owner + ".jsonl")
    path.parent.mkdir(parents=True, exist_ok=True)
    rows = [meta(root, id=owner, session_id=owner)]
    for index, name in enumerate(names, 1):
        rows.append(mcp_item(root, call=f"exec-{owner}-{index}", ordinal=index, thread=owner,
                             arguments={"path":name, "content":(root / name).read_text()}))
    if opaque:
        rows.append({"type":"response_item", "ordinal":len(rows), "timestamp":"2026-10-11T15:42:32Z",
                     "payload":{"type":"custom_tool_call", "name":"exec", "call_id":"wrapper",
                                "namespace":"functions", "input":"unrecoverable orchestration"}})
    path.write_text("".join(json.dumps(row) + "\n" for row in rows))
    return path


def test_native_mcp_provenance_cli_distinguishes_own_and_peer(checkout):
    root, env = checkout
    native_writes(root, env, "root-1", ["own.rs"])
    native_writes(root, env, "peer-root", ["peer.rs"])
    result = invoke(root, env | {"CODEX_SESSION_ID":"root-1"}, "own.rs", "peer.rs")
    assert verdicts(result) == {"own.rs":"MINE", "peer.rs":"PEER"}
    assert "codex:peer-root" in result.stdout


def test_native_mcp_provenance_cli_retains_known_owner_with_wrapper_gap(checkout):
    root, env = checkout
    native_writes(root, env, "root-1", ["own.rs"], opaque=True)
    result = invoke(root, env | {"CODEX_SESSION_ID":"root-1"}, "own.rs")
    assert verdicts(result) == {"own.rs":"UNKNOWN"}
    assert "codex:root-1" in result.stdout
    assert "coverage" in result.stdout.lower()


def test_native_mcp_formatter_formats_own_and_preserves_peer_bytes(project):
    root, env = project
    native_writes(root, env, "root-1", ["src/own.rs"])
    native_writes(root, env, "peer-root", ["src/peer.rs"])
    peer_before = (root / "src/peer.rs").read_bytes()
    result = run_fmt(root, env | {"CODEX_SESSION_ID":"root-1"})
    assert result.returncode == 1, result.stderr
    assert (root / "src/own.rs").read_text() == 'pub fn own() {\n    let x = 1;\n    println!("{x}");\n}\n'
    assert (root / "src/peer.rs").read_bytes() == peer_before
    assert "PEER" in result.stderr


def test_native_mcp_formatter_keeps_known_own_bytes_when_wrapper_coverage_is_unknown(project):
    root, env = project
    native_writes(root, env, "root-1", ["src/own.rs", "src/peer.rs"], opaque=True)
    before = snapshot(root)
    result = run_fmt(root, env | {"CODEX_SESSION_ID":"root-1"})
    assert result.returncode == 1, result.stderr
    assert "UNKNOWN" in result.stderr
    assert snapshot(root) == before

def test_native_source_id_separates_equal_call_ids_across_root_and_child(tmp_path):
    root_batch = read(tmp_path, [meta(tmp_path), mcp_item(tmp_path, call="same-call")])
    child_batch = read(tmp_path, [
        meta(tmp_path, id="child-1",
             source={"subagent":{"thread_spawn":{"parent_thread_id":"root-1"}}},
             parent_thread_id="root-1", forked_from_id="root-1", subagent_history_start_ordinal=10),
        mcp_item(tmp_path, call="same-call", thread="child-1"),
    ])
    assert not root_batch.gaps and not child_batch.gaps
    assert [op.source_id for op in root_batch.operations + child_batch.operations] == [
        "codex:root-1/root-1/same-call", "codex:root-1/child-1/same-call"]
    assert root_batch.operations[0].principal == child_batch.operations[0].principal == "codex:root-1"


@pytest.mark.parametrize("patch", [
    {"result":{"content":[], "isError":True}},
    {"status":"failed"},
    {"error":{"message":"native failure"}},
    {"result":None, "error":{"message":"native failure"}},
    {"result":None, "status":"failed"},
])
def test_native_failure_fields_independently_retain_partial_writer(tmp_path, patch):
    batch = read(tmp_path, [meta(tmp_path), mcp_item(tmp_path, **patch)])
    assert not batch.gaps
    assert len(batch.operations) == 1
    assert batch.operations[0].outcome == "partial-error"
    assert batch.operations[0].atomic is True
    assert batch.operations[0].principal == "codex:root-1"


@pytest.mark.parametrize("kind", ["token_usage_record", "inter_agent_communication_metadata"])
def test_observed_native_metadata_envelopes_do_not_add_coverage_gaps(tmp_path, kind):
    batch = read(tmp_path, [meta(tmp_path),
        {"type":kind, "ordinal":1, "payload":{}}, mcp_item(tmp_path)])
    assert not batch.gaps
    assert [op.call_id for op in batch.operations] == ["exec-write-1"]


def test_native_object_arguments_preserve_raw_decimal_precision(tmp_path):
    path = tmp_path / "native.jsonl"
    item = json.dumps(mcp_item(tmp_path, arguments={"path":"own.rs", "ratio":"DECIMAL_LITERAL"}))
    item = item.replace('"DECIMAL_LITERAL"', "0.123456789012345678901234567890123456789")
    path.write_text(json.dumps(meta(tmp_path)) + "\n" + item + "\n")
    batch = codex.read_codex_history(path)
    assert not batch.gaps
    assert str(batch.operations[0].input["ratio"]) == "0.123456789012345678901234567890123456789"


def test_native_turn_context_changes_relative_write_cwd(tmp_path):
    later = tmp_path / "later"
    batch = read(tmp_path, [meta(tmp_path),
        {"type":"turn_context", "ordinal":1, "payload":{"cwd":str(later)}}, mcp_item(tmp_path)])
    assert not batch.gaps
    op = batch.operations[0]
    assert op.cwd == later
    assert op.input["path"] == "own.rs"
    assert (op.principal, op.agent_id) == ("codex:root-1", None)


def test_native_turn_context_relative_writer_is_attributed_in_current_checkout(checkout):
    root, env = checkout
    path = native_writes(root, env, "root-1", ["own.rs"])
    rows = [json.loads(line) for line in path.read_text().splitlines()]
    rows[0]["payload"]["cwd"] = str(root.parent / "startup-elsewhere")
    rows.insert(1, {"type":"turn_context", "ordinal":0, "payload":{"cwd":str(root)}})
    path.write_text("".join(json.dumps(row) + "\n" for row in rows))
    result = invoke(root, env | {"CODEX_SESSION_ID":"root-1"}, "own.rs")
    assert result.returncode == 0, result.stderr
    assert verdicts(result) == {"own.rs":"MINE"}


def unsupported_native_item(root, kind):
    if kind == "McpToolCall":
        return mcp_item(root, call="unsupported", ordinal=20, status="in_progress")
    return {"type":"event_msg", "ordinal":20,
            "payload":{"type":"item_completed", "thread_id":"root-1",
                       "item":{"type":kind, "id":"unsupported"}}}


@pytest.mark.parametrize("kind", ["FileChange", "CommandExecution", "DynamicToolCall", "FutureWrite"])
def test_unsupported_native_completed_effect_keeps_known_write_and_gap(tmp_path, kind):
    batch = read(tmp_path, [meta(tmp_path), mcp_item(tmp_path), unsupported_native_item(tmp_path, kind)])
    assert [op.call_id for op in batch.operations] == ["exec-write-1"]
    assert any("unsupported native Codex completed item" in gap.reason for gap in batch.gaps)


@pytest.mark.parametrize("status", ["in_progress", "cancelled", "future-status"])
def test_unsupported_native_mcp_status_cannot_claim_authorship(tmp_path, status):
    batch = read(tmp_path, [meta(tmp_path), mcp_item(tmp_path, status=status)])
    assert not batch.operations
    assert any("arguments or status" in gap.reason for gap in batch.gaps)


@pytest.mark.parametrize("kind", ["FileChange", "McpToolCall"])
def test_unsupported_native_effect_downgrades_known_own_cli_to_unknown(checkout, kind):
    root, env = checkout
    path = native_writes(root, env, "root-1", ["own.rs"])
    with path.open("a") as stream:
        stream.write(json.dumps(unsupported_native_item(root, kind)) + "\n")
    result = invoke(root, env | {"CODEX_SESSION_ID":"root-1"}, "own.rs")
    assert result.returncode == 1, result.stderr
    assert verdicts(result) == {"own.rs":"UNKNOWN"}
    assert "codex:root-1" in result.stdout


@pytest.mark.parametrize("kind", ["FileChange", "McpToolCall"])
def test_unsupported_native_effect_prevents_formatter_changes(project, kind):
    root, env = project
    path = native_writes(root, env, "root-1", ["src/own.rs", "src/peer.rs"])
    with path.open("a") as stream:
        stream.write(json.dumps(unsupported_native_item(root, kind)) + "\n")
    before = snapshot(root)
    result = run_fmt(root, env | {"CODEX_SESSION_ID":"root-1"})
    assert result.returncode == 1, result.stderr
    assert "UNKNOWN" in result.stderr
    assert snapshot(root) == before
