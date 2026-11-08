"""Harness-discriminating publish-hold integration, using only disposable Git refs."""
import json
import os
from pathlib import Path
import subprocess

import pytest

ROOT = Path(__file__).resolve().parents[1]
SID = "aaaaaaaa-1111-2222-3333-444444444444"
OTHER = "bbbbbbbb-5555-6666-7777-888888888888"


@pytest.fixture
def repo(tmp_path):
    checkout = tmp_path / "repo"
    checkout.mkdir()
    home = tmp_path / "home"
    home.mkdir()
    env = {k: v for k, v in os.environ.items()
           if not k.startswith(("GIT_", "CLAUDE_", "CODEX_", "CODESCOUT_"))}
    env.update(HOME=str(home), GIT_CONFIG_NOSYSTEM="1",
               GIT_CONFIG_GLOBAL=os.devnull)

    def run(*args, harness="codex", sid=SID, extra=None, input=None,
            check=True, cwd=None):
        current = dict(env)
        if harness and sid:
            key = "CODEX_SESSION_ID" if harness == "codex" else "CLAUDE_CODE_SESSION_ID"
            current[key] = sid
        current.update(extra or {})
        result = subprocess.run(args, cwd=cwd or checkout, env=current,
                                input=input, capture_output=True, text=True, timeout=30)
        if check:
            assert result.returncode == 0, result.stdout + result.stderr
        return result

    run("git", "init", "-q", "-b", "main")
    run("git", "config", "user.name", "Hold test")
    run("git", "config", "user.email", "hold@example.invalid")
    run("git", "-c", "core.hooksPath=/dev/null", "commit", "--allow-empty", "-qm", "base")
    (checkout / "scripts").symlink_to(ROOT / "scripts", target_is_directory=True)
    hooks = checkout / ".git/hooks"
    for name, script in (("pre-commit", "pre-commit-run.sh"),
                         ("prepare-commit-msg", "prepare-commit-msg-session-id.sh")):
        path = hooks / name
        path.write_text(f'#!/usr/bin/env bash\nexec scripts/{script} "$@"\n')
        path.chmod(0o755)
    run("git", "config", "core.hooksPath", str(hooks))
    return checkout, run


def hold(run, *args, **kwargs):
    return run("bash", str(ROOT / "scripts/hold-publish.sh"), *args, **kwargs)


def push(run, base, tip, ref="refs/heads/main", **kwargs):
    return run("bash", str(ROOT / "scripts/pre-push-foreign-session-guard.sh"),
               "origin", "example.invalid:unused",
               input=f"{tip} {tip} {ref} {base}\n", **kwargs)


def test_codex_hold_refresh_list_and_harness_specific_release(repo):
    checkout, run = repo
    hold(run, "set", "pending review")
    codex_ref = f"refs/holds/_codex/{SID}"
    original = run("git", "cat-file", "-p", codex_ref).stdout
    assert original.splitlines()[0] == "reason: pending review"
    assert len(original.splitlines()) == 3
    run("git", "-c", "core.hooksPath=/dev/null", "commit", "--allow-empty", "-qm", "later")
    hold(run, "set")
    refreshed = run("git", "cat-file", "-p", codex_ref).stdout
    assert refreshed.splitlines()[:2] == original.splitlines()[:2]
    assert refreshed.splitlines()[2] == "head: " + run("git", "rev-parse", "HEAD").stdout.strip()

    hold(run, "set", "Claude same UUID", harness="claude-code")
    listing = hold(run, "list").stdout
    assert f"codex:{SID}\t?\t" in listing
    assert "pending review" in listing
    assert f"{SID}\t" in listing
    hold(run, "release")
    assert run("git", "show-ref", "--exists", codex_ref, check=False).returncode == 2
    run("git", "show-ref", "--exists", f"refs/holds/{SID}")
    released = hold(run, "release", SID)
    assert "another session" in released.stdout
    assert hold(run, "list", harness=None).stdout == ""


@pytest.mark.parametrize("form", ["empty", "index", "pathspec", "all", "amend"])
def test_codex_hold_refuses_commit_forms_without_mutating_refs(repo, form):
    checkout, run = repo
    (checkout / "owned.txt").write_text("before")
    run("git", "add", "owned.txt")
    run("git", "commit", "-qm", "tracked")
    (checkout / "owned.txt").write_text("after")
    run("git", "add", "owned.txt")
    hold(run, "set", "withheld")
    before = run("git", "rev-parse", "HEAD").stdout
    index = run("git", "write-tree").stdout
    ref = run("git", "rev-parse", f"refs/holds/_codex/{SID}").stdout
    args = {"empty": ["--allow-empty"], "index": [], "pathspec": ["--", "owned.txt"],
            "all": ["-a"], "amend": ["--amend", "--no-edit"]}[form]
    result = run("git", "commit", "-m", "blocked", *args, check=False)
    assert result.returncode != 0
    assert "PUBLISH HOLD" in result.stderr
    assert f"codex:{SID}" in result.stderr
    assert run("git", "rev-parse", "HEAD").stdout == before
    assert run("git", "write-tree").stdout == index
    assert run("git", "rev-parse", f"refs/holds/_codex/{SID}").stdout == ref


def test_codex_commit_allowed_after_release_and_ignores_matching_claude_uuid(repo):
    _, run = repo
    hold(run, "set", "Claude only", harness="claude-code")
    run("git", "commit", "--allow-empty", "-qm", "codex is independent")
    hold(run, "set", "Codex")
    assert run("git", "commit", "--allow-empty", "-qm", "held", check=False).returncode != 0
    hold(run, "release", f"codex:{SID}", harness=None)
    run("git", "commit", "--allow-empty", "-qm", "released")
    run("git", "show-ref", "--exists", f"refs/holds/{SID}")


def test_native_codex_root_survives_child_thread_and_linked_worktree(repo, tmp_path):
    _, run = repo
    child = {"CODEX_THREAD_ID": OTHER}
    hold(run, "set", "child uses root", extra=child)
    run("git", "show-ref", "--exists", f"refs/holds/_codex/{SID}")
    tree = tmp_path / "linked"
    run("git", "worktree", "add", "-q", "-b", "linked", str(tree))
    (tree / "scripts").symlink_to(ROOT / "scripts", target_is_directory=True)
    result = run("git", "commit", "--allow-empty", "-qm", "held", cwd=tree,
                 extra=child, check=False)
    assert result.returncode != 0
    assert "PUBLISH HOLD" in result.stderr


@pytest.mark.parametrize("operation", ["set", "release", "commit", "push", "stamp"])
@pytest.mark.parametrize("conflict", ["two-native", "explicit-mismatch"])
def test_conflicting_identity_fails_closed(repo, tmp_path, operation, conflict):
    checkout, run = repo
    extra = {"CLAUDE_CODE_SESSION_ID": SID}
    if conflict == "explicit-mismatch":
        context = tmp_path / "context.json"
        context.write_text(json.dumps({
            "schema_version": 1, "harness": "codex", "session_id": OTHER,
            "agent_id": None, "workspace": str(checkout), "source": "explicit",
            "harness_version": None}))
        extra = {"CODESCOUT_CONTEXT_FILE": str(context)}
    before = run("git", "rev-parse", "HEAD").stdout.strip()
    if operation in ("set", "release"):
        result = hold(run, operation, extra=extra, check=False)
    elif operation == "commit":
        result = run("git", "commit", "--allow-empty", "-qm", "conflict",
                     extra=extra, check=False)
    elif operation == "stamp":
        msg = checkout / "message"
        msg.write_text("conflict\n")
        result = run("bash", str(ROOT / "scripts/prepare-commit-msg-session-id.sh"),
                     str(msg), extra=extra, check=False)
        assert msg.read_text() == "conflict\n"
    else:
        result = push(run, before, before, extra=extra, check=False)
    assert result.returncode != 0
    assert "conflict" in result.stderr.lower()
    assert run("git", "rev-parse", "HEAD").stdout.strip() == before
    assert hold(run, "list", harness=None).stdout == ""


@pytest.mark.parametrize("sid", ["a/b", "a.b", "a b", "a\nb", "../x"])
def test_codex_invalid_ref_identity_fails_closed(repo, sid):
    _, run = repo
    assert hold(run, "set", sid=sid, check=False).returncode != 0
    assert run("git", "commit", "--allow-empty", "-qm", "invalid",
               sid=sid, check=False).returncode != 0
    assert hold(run, "list", harness=None).stdout == ""


def test_explicit_codex_context_and_thread_only_manual_behavior(repo, tmp_path):
    checkout, run = repo
    context = tmp_path / "context.json"
    context.write_text(json.dumps({
        "schema_version": 1, "harness": "codex", "session_id": SID,
        "agent_id": OTHER, "workspace": str(checkout), "source": "explicit",
        "harness_version": None}))
    hold(run, "set", harness=None, extra={"CODESCOUT_CONTEXT_FILE": str(context)})
    run("git", "show-ref", "--exists", f"refs/holds/_codex/{SID}")
    run("git", "commit", "--allow-empty", "-qm", "manual", harness=None,
        extra={"CODEX_THREAD_ID": OTHER})
    assert run("git", "log", "-1", "--format=%(trailers:key=Session-Id,valueonly)").stdout.strip() == ""


def test_codex_trailer_stamps_root_and_preserves_replayed_author(repo):
    checkout, run = repo
    run("git", "commit", "--allow-empty", "-qm", "native",
        extra={"CODEX_THREAD_ID": OTHER, "CODESCOUT_INDEX_ACK": f"codex:{OTHER}"})
    message = run("git", "log", "-1", "--format=%B").stdout
    assert f"Session-Id: codex:{SID}" in message
    assert f"Co-Authored-Session-Id: codex:{OTHER}" in message
    msg = checkout / "message"
    msg.write_text(message)
    script = str(ROOT / "scripts/prepare-commit-msg-session-id.sh")
    run("bash", script, str(msg))
    assert msg.read_text().count(f"Session-Id: codex:{SID}") == 1
    msg.write_text(f"replayed\n\nSession-Id: {OTHER}\n")
    run("bash", script, str(msg))
    assert f"Session-Id: {OTHER}" in msg.read_text()
    assert f"Session-Id: codex:{SID}" not in msg.read_text()


def stamp_message(repo, text):
    checkout, run = repo
    msg = checkout / "message"
    msg.write_text(text)
    result = run("bash", str(ROOT / "scripts/prepare-commit-msg-session-id.sh"), str(msg))
    return msg.read_text(), result


def test_a_bare_co_author_token_is_named_on_stderr_and_left_in_place(repo):
    # 867546ab6 carried a Codex thread id here with no `codex:` prefix, and the guard reads a
    # bare token as a Claude session. The hook cannot tell the harness from the uuid, so it
    # names the token and edits nothing (docs/issues/2026-11-06-codex-coauthor-trailer-missing-namespace.md).
    text, result = stamp_message(
        repo, f"work\n\nCo-Authored-Session-Id: {OTHER}\n")
    assert f"Co-Authored-Session-Id: {OTHER}" in text
    assert OTHER in result.stderr
    assert "codex:" in result.stderr


@pytest.mark.parametrize("token", [f"codex:{OTHER}", f"claude-code:{OTHER}"])
def test_a_qualified_co_author_token_is_silent(repo, token):
    text, result = stamp_message(repo, f"work\n\nCo-Authored-Session-Id: {token}\n")
    assert f"Co-Authored-Session-Id: {token}" in text
    assert "Co-Authored-Session-Id" not in result.stderr


@pytest.mark.parametrize("pusher", ["codex", "claude-code"])
@pytest.mark.parametrize("ref", ["refs/heads/main", "refs/tags/v1", "refs/wip/snapshot"])
def test_codex_held_push_refuses_all_ack_from_either_harness(repo, pusher, ref):
    _, run = repo
    base = run("git", "rev-parse", "HEAD").stdout.strip()
    run("git", "commit", "--allow-empty", "-qm", "Codex authored")
    tip = run("git", "rev-parse", "HEAD").stdout.strip()
    hold(run, "set", "withheld")
    result = push(run, base, tip, ref, harness=pusher, sid=OTHER,
                  extra={"CODESCOUT_PUSH_ACK": "all"}, check=False)
    assert result.returncode != 0
    assert "PUBLISH HOLD" in result.stderr
    assert f"codex:{SID}" in result.stderr
    assert "An ack does not clear a hold" in result.stderr


def test_codex_pusher_owns_qualified_token_but_not_same_uuid_claude(repo):
    _, run = repo
    base = run("git", "rev-parse", "HEAD").stdout.strip()
    run("git", "commit", "--allow-empty", "-qm", "own Codex")
    own = run("git", "rev-parse", "HEAD").stdout.strip()
    push(run, base, own)
    run("git", "commit", "--allow-empty", "-qm", "Claude same UUID", harness="claude-code")
    tip = run("git", "rev-parse", "HEAD").stdout.strip()
    refused = push(run, own, tip, check=False)
    assert refused.returncode != 0
    assert "PUBLISH HOLD" not in refused.stderr
    assert "REFUSING THE PUSH" in refused.stderr
    push(run, own, tip, extra={"CODESCOUT_PUSH_ACK": SID})
    hold(run, "set", "Codex only")
    push(run, own, tip, extra={"CODESCOUT_PUSH_ACK": SID})


def test_codex_coauthor_hold_is_respected(repo):
    _, run = repo
    base = run("git", "rev-parse", "HEAD").stdout.strip()
    run("git", "commit", "--allow-empty", "-qm", "coauthored",
        extra={"CODESCOUT_INDEX_ACK": f"codex:{OTHER}"})
    tip = run("git", "rev-parse", "HEAD").stdout.strip()
    hold(run, "set", "coauthor withheld", sid=OTHER)
    result = push(run, base, tip, extra={"CODESCOUT_PUSH_ACK": "all"}, check=False)
    assert result.returncode != 0
    assert "PUBLISH HOLD" in result.stderr
    assert f"codex:{OTHER}" in result.stderr


def test_codex_liveness_never_borrows_a_matching_claude_registry_row(repo):
    checkout, run = repo
    registry = checkout.parent / "home/.claude-test/sessions"
    registry.mkdir(parents=True)
    (registry / "same.json").write_text(json.dumps({
        "sessionId": SID, "pid": 999999, "procStart": "1",
        "messagingSocketPath": str(checkout / "absent.sock")}))
    hold(run, "set", "Claude", harness="claude-code")
    hold(run, "set", "Codex")
    rows = {row.split("\t")[0]: row.split("\t")
            for row in hold(run, "list").stdout.splitlines()}
    assert rows[SID][1] == "gone"
    assert rows[f"codex:{SID}"][1] == "?"
    resolved = run("bash", "-c", 'source "$1"; resolve_sids "$2" "$3"', "test",
                   str(ROOT / "scripts/resolve-sids.sh"), f"codex:{SID}",
                   f"claude-code:{SID}").stdout.splitlines()
    assert resolved[0] == f"codex:{SID}\t?\t"
    assert resolved[1] == f"claude-code:{SID}\tgone\t"


def test_codex_hold_namespace_does_not_collide_with_legacy_id_codex(repo):
    _, run = repo
    hold(run, "set", "legacy id codex", harness="claude-code", sid="codex")
    hold(run, "set", "native Codex")
    run("git", "show-ref", "--exists", "refs/holds/codex")
    run("git", "show-ref", "--exists", f"refs/holds/_codex/{SID}")
    hold(run, "release", "claude-code:codex", harness=None)
    run("git", "show-ref", "--exists", f"refs/holds/_codex/{SID}")


def test_explicit_claude_alias_release_preserves_same_uuid_codex_hold(repo):
    _, run = repo
    hold(run, "set", "Claude", harness="claude-code")
    hold(run, "set", "Codex")
    hold(run, "release", f"claude-code:{SID}")
    assert run("git", "show-ref", "--exists", f"refs/holds/{SID}",
               check=False).returncode == 2
    run("git", "show-ref", "--exists", f"refs/holds/_codex/{SID}")
    hold(run, "release")
    assert hold(run, "list", harness=None).stdout == ""


@pytest.mark.parametrize("operation", ["push", "release"])
def test_corrupt_codex_hold_refuses_push_and_release_without_deleting_it(repo, operation):
    checkout, run = repo
    base = run("git", "rev-parse", "HEAD").stdout.strip()
    run("git", "commit", "--allow-empty", "-qm", "Codex authored")
    tip = run("git", "rev-parse", "HEAD").stdout.strip()
    hold(run, "set", "withheld")
    ref = checkout / f".git/refs/holds/_codex/{SID}"
    ref.write_text("broken\n")
    if operation == "push":
        result = push(run, base, tip, sid=OTHER,
                      extra={"CODESCOUT_PUSH_ACK": "all"}, check=False)
        assert result.returncode != 0
        assert "cannot check publish hold" in result.stderr.lower()
        assert f"codex:{SID}" in result.stderr
    else:
        result = hold(run, "release", check=False)
        assert result.returncode != 0
        assert "could not inspect" in result.stderr
        assert "no hold" not in result.stdout
    assert ref.read_text() == "broken\n"
