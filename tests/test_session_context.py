"""Contract tests: identity conflicts must never silently authorize another root."""
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys

import pytest

SCRIPT = Path(__file__).resolve().parents[1] / "scripts" / "session_context.py"


@pytest.fixture
def ctx():
    assert SCRIPT.is_file(), "invocation context resolver is not implemented"
    spec = importlib.util.spec_from_file_location("session_context", SCRIPT)
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


@pytest.fixture
def root(tmp_path):
    checkout = tmp_path / "repo with spaces"
    checkout.mkdir()
    subprocess.run(["git", "init", "-q", str(checkout)], check=True)
    return checkout


def context(root, **patch):
    value = dict(schema_version=1, harness="codex", session_id="same:root",
                 agent_id=None, workspace=str(root), source="explicit",
                 harness_version=None)
    return value | patch


def write_context(root, name="context with spaces.json", **patch):
    path = root.parent / name
    path.write_text(json.dumps(context(root, **patch)))
    return path


def clean_env(**patch):
    env = dict(os.environ)
    for key in ("CLAUDE_CODE_SESSION_ID", "CODEX_SESSION_ID", "CODEX_THREAD_ID", "CODESCOUT_CONTEXT_FILE"):
        env.pop(key, None)
    return env | patch


def test_principals_are_host_qualified_and_keep_opaque_colons(ctx):
    assert ctx.principal_key("codex", "same:id") == "codex:same:id"
    assert ctx.principal_key("claude-code", "same:id") == "claude-code:same:id"


@pytest.mark.parametrize("env,state,harness,agent", [
    ({}, "missing", None, None),
    ({"CODEX_THREAD_ID": "child"}, "missing", None, None),
    ({"CLAUDE_CODE_SESSION_ID": "root"}, "available", "claude-code", None),
    ({"CODEX_SESSION_ID": "root"}, "available", "codex", None),
    ({"CODEX_SESSION_ID": "root", "CODEX_THREAD_ID": "root"}, "available", "codex", None),
    ({"CODEX_SESSION_ID": "root", "CODEX_THREAD_ID": "child"}, "available", "codex", "child"),
    ({"CLAUDE_CODE_SESSION_ID": "root", "CODEX_SESSION_ID": "other"}, "conflicting", None, None),
    ({"CLAUDE_CODE_SESSION_ID": "root", "CODEX_SESSION_ID": "root"}, "conflicting", None, None),
])
def test_native_resolution(ctx, root, env, state, harness, agent):
    result = ctx.resolve_context(root, env)
    assert result.state == state
    if state == "available":
        assert result.context.harness == harness
        assert result.context.session_id == "root"
        assert result.context.agent_id == agent
        assert result.context.workspace == root.resolve()
        assert result.context.source == "native-env"
        assert result.context.harness_version is None
    else:
        assert result.context is None
        assert result.reason


def test_codex_native_version_and_opaque_identity(ctx, root):
    result = ctx.resolve_context(root, {"CODEX_SESSION_ID": "opaque:id", "CODEX_VERSION": "0.160.0"})
    assert result.context.session_id == "opaque:id"
    assert result.context.harness_version == "0.160.0"


@pytest.mark.parametrize("version", [2, 1.5, "1", True, None])
def test_unsupported_schema(ctx, root, version):
    result = ctx.validate_context(context(root, schema_version=version), root)
    assert result.state == "unsupported"
    assert "schema_version" in result.reason


def test_integer_valued_json_version_normalizes_to_int(ctx, root):
    result = ctx.validate_context(context(root, schema_version=1.0), root)
    assert type(result.context.schema_version) is int
    assert result.context.schema_version == 1


@pytest.mark.parametrize("patch", [
    {"harness": "other"}, {"source": "pointer"}, {"session_id": ""}, {"session_id": 9},
    {"agent_id": ""}, {"agent_id": 9}, {"harness_version": False},
    {"workspace": "relative"}, {"workspace": 9},
])
def test_bad_field(ctx, root, patch):
    result = ctx.validate_context(context(root, **patch), root)
    assert result.state == "unsupported"
    assert next(iter(patch)) in result.reason


@pytest.mark.parametrize("field", ["schema_version", "harness", "session_id", "agent_id", "workspace", "source", "harness_version"])
def test_all_fields_required(ctx, root, field):
    value = context(root)
    del value[field]
    assert ctx.validate_context(value, root).state == "unsupported"


@pytest.mark.parametrize("native", [
    {"CODEX_SESSION_ID": "other"},
    {"CLAUDE_CODE_SESSION_ID": "same:root"},
    {"CLAUDE_CODE_SESSION_ID": "same:root", "CODEX_SESSION_ID": "same:root"},
])
def test_explicit_context_cannot_override_native_conflict(ctx, root, native):
    path = write_context(root)
    assert ctx.resolve_context(root, native, path).state == "conflicting"


def test_explicit_matching_context_and_path_aliases(ctx, root):
    path = write_context(root)
    alias = path.parent / "alias.json"
    alias.symlink_to(path)
    value = ctx.resolve_context(root, {"CODEX_SESSION_ID": "same:root", "CODESCOUT_CONTEXT_FILE": str(alias)}, path)
    assert value.state == "available"
    assert value.context.source == "explicit"
    assert value.context.session_id == "same:root"
    other = write_context(root, "other.json")
    conflict = ctx.resolve_context(root, {"CODESCOUT_CONTEXT_FILE": str(other)}, path)
    assert conflict.state == "conflicting"
    assert "context" in conflict.reason


def test_manual_explicit_context_requires_no_native_env(ctx, root):
    path = write_context(root, harness="manual", session_id="operator")
    result = ctx.resolve_context(root, {}, path)
    assert result.state == "available"
    assert result.context.harness == "manual"


def test_bad_explicit_file_and_relative_path(ctx, root):
    path = root.parent / "bad.json"
    path.write_text("{broken")
    assert ctx.resolve_context(root, {}, path).state == "unsupported"
    assert ctx.resolve_context(root, {}, Path("relative.json")).state == "unsupported"
    assert ctx.resolve_context(root, {}, root.parent / "absent.json").state == "unsupported"


def test_canonical_checkout_not_string_prefix(ctx, root):
    sibling = root.parent / (root.name + "-other")
    sibling.mkdir()
    assert ctx.validate_context(context(sibling), root).state == "conflicting"
    nested = root / "nested"
    nested.mkdir()
    subprocess.run(["git", "init", "-q", str(nested)], check=True)
    assert ctx.validate_context(context(nested), root).state == "conflicting"
    alias = root.parent / "repo-alias"
    alias.symlink_to(root, target_is_directory=True)
    assert ctx.validate_context(context(alias), root).context.workspace == root.resolve()
    subprocess.run(["git", "-C", str(root), "-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid",
                    "commit", "-q", "--allow-empty", "-m", "base"], check=True)
    worktree = root.parent / "linked tree"
    subprocess.run(["git", "-C", str(root), "worktree", "add", "-q", "--detach", str(worktree)], check=True)
    assert ctx.validate_context(context(worktree), root).state == "conflicting"
    assert ctx.validate_context(context(worktree), worktree).state == "available"


def test_cli_checks_actual_invocation_checkout(root):
    path = write_context(root)
    result = subprocess.run([sys.executable, str(SCRIPT), "--workspace", str(root), "--context", str(path)],
                            cwd=root, env=clean_env(), text=True, capture_output=True)
    assert result.returncode == 0, result.stderr
    assert json.loads(result.stdout)["context"] == context(root)
    nested = root / "nested"
    nested.mkdir()
    subprocess.run(["git", "init", "-q", str(nested)], check=True)
    wrong = subprocess.run([sys.executable, str(SCRIPT), "--workspace", str(root), "--context", str(path)],
                           cwd=nested, env=clean_env(), text=True, capture_output=True)
    assert wrong.returncode == 2
    assert json.loads(wrong.stdout)["state"] == "conflicting"


def test_shared_expected_objects(ctx, root):
    pack = json.loads((Path(__file__).parent / "fixtures/session-context-v1.json").read_text()
                      .replace("${WORKSPACE}", str(root)))
    for case in pack["cases"]:
        result = ctx.validate_context(case["expected"]["context"], root)
        assert result.state == case["expected"]["state"]
        assert result.context.to_dict() == case["expected"]["context"]
