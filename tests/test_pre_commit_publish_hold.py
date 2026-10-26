"""Publish holds must block real commits through the native dispatcher.

Removing the dispatch, looking up another session, or ignoring the shared ref
lets a held commit advance HEAD; the refusal tests observe that change.
"""
import os
from pathlib import Path
import subprocess

import pytest

ROOT = Path(__file__).resolve().parents[1]
AUTHOR = "aaaaaaaa-1111-2222-3333-444444444444"
OTHER = "bbbbbbbb-5555-6666-7777-888888888888"


@pytest.fixture
def checkout(tmp_path):
    repo = tmp_path / "repo"
    repo.mkdir()
    home = tmp_path / "home"
    home.mkdir()
    env = {key: value for key, value in os.environ.items()
           if not key.startswith(("GIT_", "CLAUDE_", "CODEX_", "CODESCOUT_"))}
    env.update(HOME=str(home), GIT_CONFIG_NOSYSTEM="1",
               GIT_CONFIG_GLOBAL=os.devnull, CLAUDE_CODE_SESSION_ID=AUTHOR)

    def run(*args, sid=AUTHOR, cwd=None, check=True, extra=None):
        current = dict(env)
        if sid is None:
            current.pop("CLAUDE_CODE_SESSION_ID", None)
        else:
            current["CLAUDE_CODE_SESSION_ID"] = sid
        current.update(extra or {})
        result = subprocess.run(args, cwd=cwd or repo, env=current,
                                text=True, capture_output=True, timeout=30)
        if check:
            assert result.returncode == 0, result.stdout + result.stderr
        return result

    run("git", "init", "-q")
    run("git", "config", "user.name", "Publish hold test")
    run("git", "config", "user.email", "hold@example.invalid")
    run("git", "-c", "core.hooksPath=/dev/null", "commit", "--allow-empty",
        "-qm", "base")
    (repo / "scripts").symlink_to(ROOT / "scripts", target_is_directory=True)
    hook = repo / ".git/hooks/pre-commit"
    hook.write_text("#!/usr/bin/env bash\nexec scripts/pre-commit-run.sh\n")
    hook.chmod(0o755)
    run("git", "config", "core.hooksPath", str(hook.parent))
    return repo, run


def set_hold(run, sid=AUTHOR):
    run("bash", str(ROOT / "scripts/hold-publish.sh"), "set",
        "waiting for operator", sid=sid)
    return run("git", "rev-parse", f"refs/holds/{sid}").stdout.strip()


@pytest.mark.parametrize("form", ["index", "pathspec", "all", "amend", "empty"])
def test_held_commit_is_refused_without_changing_head_index_or_hold(checkout, form):
    repo, run = checkout
    hold = set_hold(run)
    before = run("git", "rev-parse", "HEAD").stdout
    if form != "empty":
        (repo / "payload.txt").write_text("held content\n")
        run("git", "add", "payload.txt")
    index_before = run("git", "write-tree").stdout
    forms = {"index": [], "pathspec": ["--", "payload.txt"], "all": ["-a"],
             "amend": ["--amend"], "empty": ["--allow-empty"]}
    result = run("git", "commit", "-qm", "held change", *forms[form],
                 check=False, extra={"CODESCOUT_PUSH_ACK": "all"})
    assert result.returncode != 0, "an active hold must refuse the commit"
    assert "PUBLISH HOLD" in result.stdout + result.stderr
    assert AUTHOR in result.stdout + result.stderr
    assert "waiting for operator" in result.stdout + result.stderr
    assert f"hold-publish.sh release {AUTHOR}" in result.stdout + result.stderr
    assert run("git", "rev-parse", "HEAD").stdout == before
    assert run("git", "write-tree").stdout == index_before
    assert run("git", "rev-parse", f"refs/holds/{AUTHOR}").stdout.strip() == hold


def test_release_allows_the_previously_refused_commit(checkout):
    repo, run = checkout
    set_hold(run)
    (repo / "payload.txt").write_text("ready after release\n")
    run("git", "add", "payload.txt")
    before = run("git", "rev-parse", "HEAD").stdout
    refused = run("git", "commit", "-qm", "held change", check=False)
    assert refused.returncode != 0
    assert run("git", "rev-parse", "HEAD").stdout == before
    run("bash", str(ROOT / "scripts/hold-publish.sh"), "release")
    run("git", "commit", "-qm", "released change")
    assert run("git", "rev-parse", "HEAD").stdout != before
    assert run("git", "show", "HEAD:payload.txt").stdout == "ready after release\n"


def test_another_sessions_hold_does_not_block_this_session(checkout):
    _, run = checkout
    hold = set_hold(run, sid=OTHER)
    before = run("git", "rev-parse", "HEAD").stdout
    run("git", "commit", "--allow-empty", "-qm", "unheld author")
    assert run("git", "rev-parse", "HEAD").stdout != before
    assert run("git", "rev-parse", f"refs/holds/{OTHER}").stdout.strip() == hold


@pytest.mark.parametrize("sid", [AUTHOR, None])
def test_unheld_or_manual_commit_still_works(checkout, sid):
    _, run = checkout
    if sid is None:
        set_hold(run)
    before = run("git", "rev-parse", "HEAD").stdout
    run("git", "commit", "--allow-empty", "-qm", "allowed", sid=sid)
    assert run("git", "rev-parse", "HEAD").stdout != before


@pytest.mark.parametrize("sid", ["bad/session", "bad.name", "has space", "bad\nname"])
def test_invalid_session_id_refuses_without_changing_head_or_index(checkout, sid):
    _, run = checkout
    before = run("git", "rev-parse", "HEAD").stdout
    index_before = run("git", "write-tree").stdout
    result = run("git", "commit", "--allow-empty", "-qm", "invalid identity",
                 sid=sid, check=False)
    assert result.returncode != 0, result.stdout + result.stderr
    assert "invalid session id" in result.stderr
    assert sid in result.stderr
    assert run("git", "rev-parse", "HEAD").stdout == before
    assert run("git", "write-tree").stdout == index_before


def test_hold_in_main_checkout_refuses_a_linked_worktree_commit(checkout, tmp_path):
    _, run = checkout
    set_hold(run)
    tree = tmp_path / "linked"
    run("git", "worktree", "add", "-qb", "other-worktree", str(tree))
    (tree / "scripts").symlink_to(ROOT / "scripts", target_is_directory=True)
    before = run("git", "rev-parse", "HEAD", cwd=tree).stdout
    result = run("git", "commit", "--allow-empty", "-qm", "held in worktree",
                 cwd=tree, check=False)
    assert result.returncode != 0
    assert "PUBLISH HOLD" in result.stdout + result.stderr
    assert run("git", "rev-parse", "HEAD", cwd=tree).stdout == before


def test_hold_refuses_the_first_commit_in_an_unborn_checkout(checkout):
    repo, run = checkout
    run("git", "checkout", "--orphan", "unborn")
    set_hold(run)
    (repo / "payload.txt").write_text("first commit is held\n")
    run("git", "add", "payload.txt")
    result = run("git", "commit", "-qm", "first", check=False)
    assert result.returncode != 0
    assert "PUBLISH HOLD" in result.stdout + result.stderr
    assert run("git", "rev-parse", "--verify", "HEAD", check=False).returncode != 0


def test_unreadable_hold_ref_refuses_instead_of_treating_it_as_absent(checkout):
    repo, run = checkout
    ref = repo / ".git/refs/holds" / AUTHOR
    ref.parent.mkdir(parents=True)
    ref.write_text("not-an-object-id\n")
    before = run("git", "rev-parse", "HEAD").stdout
    result = run("git", "commit", "--allow-empty", "-qm", "uncertain hold",
                 check=False)
    assert result.returncode != 0
    assert "cannot check publish hold" in result.stdout + result.stderr
    assert AUTHOR in result.stdout + result.stderr
    assert run("git", "rev-parse", "HEAD").stdout == before
