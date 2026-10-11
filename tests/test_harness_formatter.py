"""Exercise real Rust formatting and compare files, including refused collateral."""
import json
import os
from pathlib import Path
import shutil
import subprocess

import pytest

from test_harness_provenance import add_writes

SCRIPTS = Path(__file__).resolve().parents[1] / "scripts"
FMT = SCRIPTS / "fmt-mine.sh"


@pytest.fixture
def project(tmp_path):
    root = tmp_path / "project with spaces"
    (root / "src").mkdir(parents=True)
    (root / "Cargo.toml").write_text('[package]\nname="context_fixture"\nversion="0.1.0"\nedition="2021"\n')
    (root / "src/lib.rs").write_text("pub mod own;\npub mod peer;\n")
    (root / "src/own.rs").write_text('pub fn own() {\n    let x = 1;\n    println!("{x}");\n}\n')
    (root / "src/peer.rs").write_text('pub fn peer(){let y=2;println!("{y}");}\n')
    subprocess.run(["git", "init", "-q", str(root)], check=True)
    subprocess.run(["git", "-C", str(root), "add", "Cargo.toml", "src/lib.rs", "src/own.rs"], check=True)
    home = tmp_path / "home"
    home.mkdir()
    env = dict(os.environ)
    for key in ("CLAUDE_CODE_SESSION_ID", "CODEX_SESSION_ID", "CODEX_THREAD_ID", "CODESCOUT_CONTEXT_FILE",
                "CLAUDE_CONFIG_DIR", "FMT_MINE_PROVENANCE", "REPO_ROOT"):
        env.pop(key, None)
    env.setdefault("RUSTUP_HOME", str(Path(os.environ["HOME"]) / ".rustup"))
    env.setdefault("CARGO_HOME", str(Path(os.environ["HOME"]) / ".cargo"))
    env.setdefault("RUSTUP_TOOLCHAIN", "1.97.1")
    env.update(HOME=str(home), FILE_PROVENANCE_ROOTS=str(tmp_path / "claude"),
               FILE_PROVENANCE_CODEX_ROOTS_JSON=json.dumps([str(tmp_path / "codex")]),
               FILE_PROVENANCE_REGISTRY_ROOTS=str(tmp_path / "registry"),
               FMT_MINE_PROVENANCE=str(SCRIPTS / "file-provenance.py"))
    subprocess.run(["git", "-C", str(root), "-c", "user.name=Fixture", "-c", "user.email=x@example.invalid",
                    "commit", "-q", "-m", "base"],
                   env=env | {"GIT_AUTHOR_DATE": "2026-10-01T12:00:00+00:00", "GIT_COMMITTER_DATE": "2026-10-01T12:00:00+00:00"}, check=True)
    (root / "src/own.rs").write_text('pub fn own(){let x=1;println!("{x}");}\n')
    return root, env


def run_fmt(root, env, *args):
    return subprocess.run(["bash", str(FMT), *args], cwd=root, env=env, text=True, capture_output=True)


def test_native_claude_fork_prefix_cannot_authorize_formatter_writes(project):
    from test_harness_history import history

    root, env = project
    env["CLAUDE_CODE_SESSION_ID"] = "fork-root"
    path = add_writes(root, env, "claude-code", "fork-root",
                      ["src/own.rs", "src/peer.rs"], timestamp="2026-10-11T07:36:15.141Z")
    copied_rows = [json.loads(line) for line in path.read_text().splitlines()]
    history(path, [
        {"type": "queue-operation", "operation": "enqueue", "sessionId": "fork-root",
         "timestamp": "2026-10-11T07:45:37.860Z"},
        *copied_rows,
    ])
    before = snapshot(root)
    result = run_fmt(root, env)
    assert snapshot(root) == before
    assert result.returncode == 1
    assert "UNKNOWN" in result.stderr



def snapshot(root):
    return {str(p.relative_to(root)): p.read_bytes() for p in (root / "src").glob("*.rs")}


def identity(env, harness):
    return env | {("CODEX_SESSION_ID" if harness == "codex" else "CLAUDE_CODE_SESSION_ID"): "me"}


def test_check_without_identity_and_provenance_is_read_only(project, tmp_path):
    root, env = project
    stub = tmp_path / "must not call provenance"
    marker = tmp_path / "called"
    stub.write_text(f'#!/bin/sh\ntouch "{marker}"\nexit 2\n')
    stub.chmod(0o755)
    before = snapshot(root)
    result = run_fmt(root, env | {"FMT_MINE_PROVENANCE": str(stub)}, "--check")
    assert result.returncode == 1, result.stderr
    assert snapshot(root) == before
    assert not marker.exists()


def test_clean_check_and_standalone_clean_need_no_identity(project):
    root, env = project
    subprocess.run(["cargo", "fmt"], cwd=root, env=env, check=True)
    before = snapshot(root)
    assert run_fmt(root, env, "--check").returncode == 0
    assert run_fmt(root, env | {"CLAUDE_CODE_SESSION_ID": "a", "CODEX_SESSION_ID": "b"}).returncode == 0
    assert snapshot(root) == before


@pytest.mark.parametrize("harness", ["claude-code", "codex"])
def test_mixed_own_format_keeps_untracked_peer_bytes(project, harness):
    root, env = project
    other = "codex" if harness == "claude-code" else "claude-code"
    add_writes(root, env, harness, "me", ["src/own.rs"])
    add_writes(root, env, other, "peer", ["src/peer.rs"])
    before = snapshot(root)
    result = run_fmt(root, identity(env, harness))
    assert result.returncode == 1, result.stderr
    assert (root / "src/own.rs").read_bytes() != before["src/own.rs"]
    assert (root / "src/peer.rs").read_bytes() == before["src/peer.rs"]
    assert "REFUSED" in result.stderr


def test_missing_and_conflicting_context_write_nothing(project):
    root, env = project
    before = snapshot(root)
    missing = run_fmt(root, env)
    assert missing.returncode == 2
    conflict = run_fmt(root, env | {"CLAUDE_CODE_SESSION_ID": "a", "CODEX_SESSION_ID": "b"})
    assert conflict.returncode == 2
    assert "conflicting" in conflict.stderr
    assert snapshot(root) == before


def test_unknown_coverage_refuses_known_own_without_writing(project):
    from test_harness_history import codex_meta, codex_item, history
    root, env = project
    add_writes(root, env, "codex", "me", ["src/own.rs", "src/peer.rs"])
    history(Path(json.loads(env["FILE_PROVENANCE_CODEX_ROOTS_JSON"])[0]) / "opaque.jsonl", [
        codex_meta(root), codex_item({"type": "custom_tool_call", "name": "exec", "call_id": "opaque", "input": "unknown"})])
    before = snapshot(root)
    result = run_fmt(root, identity(env, "codex"))
    assert result.returncode == 1
    assert "UNKNOWN" in result.stderr
    assert snapshot(root) == before


def test_invalid_rust_is_tool_error_in_check_mode(project):
    root, env = project
    (root / "src/own.rs").write_text("pub fn invalid( {\n")
    before = snapshot(root)
    result = run_fmt(root, env, "--check")
    assert result.returncode == 2
    assert snapshot(root) == before


def test_exclusive_checkout_requires_matching_root_and_is_operator_declared(project):
    root, env = project
    before = snapshot(root)
    result = run_fmt(root, env, "--exclusive-checkout", str(root))
    assert result.returncode == 0, result.stderr
    assert snapshot(root) != before
    assert "operator-declared" in result.stderr
    assert "MINE" not in result.stdout


@pytest.mark.parametrize("args", [
    ["--check", "--check"], ["--unknown"], ["--context"], ["--exclusive-checkout", "relative"],
    ["--check", "--exclusive-checkout", "ROOT"], ["--exclusive-checkout", "PARENT"],
])
def test_invalid_modes_write_nothing(project, args):
    root, env = project
    args = [str(root) if value == "ROOT" else str(root.parent) if value == "PARENT" else value for value in args]
    before = snapshot(root)
    assert run_fmt(root, env, *args).returncode == 2
    assert snapshot(root) == before


def test_exclusive_conflicting_context_is_refused(project):
    root, env = project
    before = snapshot(root)
    result = run_fmt(root, env | {"CLAUDE_CODE_SESSION_ID": "a", "CODEX_SESSION_ID": "b"}, "--exclusive-checkout", str(root))
    assert result.returncode == 2
    assert snapshot(root) == before


def test_context_path_spaces_native_codex_owned_files(project, tmp_path):
    root, env = project
    add_writes(root, env, "codex", "me", ["src/own.rs", "src/peer.rs"])
    value = dict(schema_version=1, harness="codex", session_id="me", agent_id=None,
                 workspace=str(root), source="explicit", harness_version=None)
    path = tmp_path / "context with spaces.json"
    path.write_text(json.dumps(value))
    result = run_fmt(root, env, "--context", str(path))
    assert result.returncode == 0, result.stderr
    assert run_fmt(root, env, "--check").returncode == 0


def test_paths_with_spaces_are_literal(project):
    root, env = project
    (root / "src/own.rs").rename(root / "src/own  file.rs")
    (root / "src/peer.rs").rename(root / "src/peer  file.rs")
    (root / "src/lib.rs").write_text('#[path = "own  file.rs"]\npub mod own;\n#[path = "peer  file.rs"]\npub mod peer;\n')
    add_writes(root, env, "codex", "me", ["src/own  file.rs"])
    add_writes(root, env, "claude-code", "peer", ["src/peer  file.rs"])
    before = snapshot(root)
    result = run_fmt(root, identity(env, "codex"))
    assert result.returncode == 1, result.stderr
    assert (root / "src/own  file.rs").read_bytes() != before["src/own  file.rs"]
    assert (root / "src/peer  file.rs").read_bytes() == before["src/peer  file.rs"]


def test_module_root_cannot_rewrite_refused_child(project):
    root, env = project
    (root / "src/lib.rs").write_text("pub mod peer;pub fn root(){let x=3;}\n")
    add_writes(root, env, "codex", "me", ["src/lib.rs"])
    add_writes(root, env, "claude-code", "peer", ["src/peer.rs"])
    before = snapshot(root)
    result = run_fmt(root, identity(env, "codex"))
    assert result.returncode == 1, result.stderr
    assert "descends" in result.stderr
    assert snapshot(root) == before


@pytest.mark.parametrize("via", ["module", "symlink", "clean-module"])
def test_symlink_or_module_cannot_escape_authorized_checkout(project, tmp_path, via):
    root, env = project
    outside = tmp_path / "outside.rs"
    outside.write_text("pub fn external() {}\n" if via == "clean-module" else "pub fn external(){let x=3;}\n")
    if via == "symlink":
        (root / "src/own.rs").unlink()
        (root / "src/own.rs").symlink_to(outside)
    else:
        (root / "src/lib.rs").write_text(f'#[path = "{outside}"]pub mod external;pub fn root(){{let x=4;}}\n')
    before = snapshot(root)
    outside_before = outside.read_bytes()
    result = run_fmt(root, env, "--exclusive-checkout", str(root))
    assert result.returncode == 2, result.stderr
    assert snapshot(root) == before
    assert outside.read_bytes() == outside_before


def test_inherited_foreign_repo_root_cannot_redirect_attribution(project, tmp_path):
    root, env = project
    foreign = tmp_path / "foreign"
    (foreign / "src").mkdir(parents=True)
    (foreign / "src/own.rs").write_text("fn other(){}\n")
    subprocess.run(["git", "init", "-q", str(foreign)], check=True)
    add_writes(root, env, "codex", "me", ["src/own.rs"])
    add_writes(root, env, "claude-code", "peer", ["src/peer.rs"])
    before = snapshot(root)
    foreign_before = (foreign / "src/own.rs").read_bytes()
    result = run_fmt(root, identity(env, "codex") | {"REPO_ROOT": str(foreign)})
    assert result.returncode == 1, result.stderr
    assert (root / "src/own.rs").read_bytes() != before["src/own.rs"]
    assert (root / "src/peer.rs").read_bytes() == before["src/peer.rs"]
    assert (foreign / "src/own.rs").read_bytes() == foreign_before
@pytest.mark.parametrize("clean_child", [False, True])
def test_nested_git_checkout_is_outside_exclusive_authorization(project, clean_child):
    root, env = project
    nested = root / "nested"
    nested.mkdir()
    subprocess.run(["git", "init", "-q", str(nested)], check=True)
    child = nested / "child.rs"
    child.write_text("pub fn child() {}\n" if clean_child else "pub fn child(){let x=3;}\n")
    (root / "src/lib.rs").write_text('#[path = "../nested/child.rs"]pub mod child;pub fn root(){let x=4;}\n')
    before = snapshot(root)
    child_before = child.read_bytes()
    result = run_fmt(root, env, "--exclusive-checkout", str(root))
    assert result.returncode == 2, result.stderr
    assert snapshot(root) == before
    assert child.read_bytes() == child_before
