"""scripts/resident-scratch.sh and scripts/resident-scratch-migrate.sh, in a disposable repo.

Both scripts run with HOME and XDG_CACHE_HOME inside the test's tmp dir, so no test reads or
writes the real ~/.cache or the real .codescout/resident-private.
"""
import os
from pathlib import Path
import stat
import subprocess
import time

import pytest

ROOT = Path(__file__).resolve().parents[1]
SCRATCH = ROOT / "scripts/resident-scratch.sh"
MIGRATE = ROOT / "scripts/resident-scratch-migrate.sh"
LONG_AGO = time.time() - 7200


@pytest.fixture
def world(tmp_path):
    home = tmp_path / "home"
    home.mkdir()
    repo = tmp_path / "work" / "myrepo"
    repo.mkdir(parents=True)
    env = {k: v for k, v in os.environ.items()
           if not k.startswith(("GIT_", "CLAUDE_", "CODEX_", "CODESCOUT_", "RESIDENT_", "XDG_"))}
    env.update(HOME=str(home), XDG_CACHE_HOME=str(home / ".cache"),
               GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL=os.devnull)

    def run(*args, cwd=None, extra=None):
        current = dict(env)
        current.update(extra or {})
        return subprocess.run(args, cwd=cwd or repo, env=current,
                              capture_output=True, text=True, timeout=60)

    for cmd in (("git", "init", "-q", "-b", "main"),
                ("git", "-c", "user.name=t", "-c", "user.email=t@example.invalid",
                 "-c", "core.hooksPath=/dev/null", "commit", "--allow-empty", "-qm", "base")):
        assert run(*cmd).returncode == 0
    return repo, home / ".cache" / "codescout-resident", run


def age(path):
    """Make a file, directory or link look untouched for two hours."""
    os.utime(path, (LONG_AGO, LONG_AGO), follow_symlinks=False)


def make_dir(base, name, files=("data.txt",)):
    d = base / name
    d.mkdir(parents=True)
    for f in files:
        (d / f).write_text("x")
    return d


def age_tree(d):
    for p in sorted(d.rglob("*"), reverse=True):
        age(p)
    age(d)


def test_a_named_scratch_directory_is_created_outside_the_checkout_and_printed(world):
    repo, cache, run = world
    result = run("bash", str(SCRATCH), "exp-1")
    assert result.returncode == 0, result.stderr
    path = Path(result.stdout.strip())
    assert path == (cache / "myrepo" / "exp-1").resolve() or path == cache / "myrepo" / "exp-1"
    assert path.is_dir()
    assert stat.S_IMODE(path.stat().st_mode) == 0o700, "scratch is private to its writer"
    assert not str(path).startswith(str(repo)), "the point is to be outside the checkout"


def test_no_name_prints_the_repo_scratch_root(world):
    _, cache, run = world
    result = run("bash", str(SCRATCH))
    assert result.returncode == 0, result.stderr
    assert Path(result.stdout.strip()) == cache / "myrepo"


@pytest.mark.parametrize("name", ["../x", "a/b", ".hidden", "-x", "a b", ".."])
def test_a_name_that_is_not_one_plain_component_is_refused_and_creates_nothing(world, name):
    _, cache, run = world
    result = run("bash", str(SCRATCH), name)
    assert result.returncode == 2, result.stdout + result.stderr
    assert not cache.exists() or not any(cache.rglob("*")), "a refusal must leave no directory"


def test_a_linked_worktree_shares_the_main_checkouts_scratch_root(world):
    repo, cache, run = world
    worktree = repo.parent / "linked-wt"
    assert run("git", "worktree", "add", "-q", str(worktree), "-b", "side").returncode == 0
    result = run("bash", str(SCRATCH), "exp-1", cwd=worktree)
    assert result.returncode == 0, result.stderr
    assert Path(result.stdout.strip()) == cache / "myrepo" / "exp-1", (
        "keyed on the main checkout, so scratch is not split per worktree")


def test_a_root_inside_the_checkout_is_refused(world):
    repo, _, run = world
    result = run("bash", str(SCRATCH), "exp-1",
                 extra={"CODESCOUT_RESIDENT_ROOT": str(repo / "scratch")})
    assert result.returncode == 2
    assert "inside the checkout" in result.stderr
    assert not (repo / "scratch").exists()


def test_the_old_in_tree_directory_is_named_unless_quiet(world):
    repo, _, run = world
    (repo / ".codescout/resident-private").mkdir(parents=True)
    loud = run("bash", str(SCRATCH), "exp-1")
    assert "still exists inside the repository" in loud.stderr
    quiet = run("bash", str(SCRATCH), "exp-1", extra={"RESIDENT_SCRATCH_QUIET": "1"})
    assert quiet.stderr == ""


def scratch_tree(repo):
    """old and old2 are closed, and old links to old2. live is being written. ref is named by a
    memory. A link in live points at old, so it dangles once old moves."""
    base = repo / ".codescout/resident-private"
    old = make_dir(base, "old")
    old2 = make_dir(base, "old2")
    (old / "to-old2").symlink_to(old2)
    (old / "to-live").symlink_to(base / "live")
    age_tree(old)
    age_tree(old2)
    ref = make_dir(base, "ref")
    age_tree(ref)
    live = make_dir(base, "live")
    (live / "to-old").symlink_to(old)
    memories = repo / ".codescout/memories"
    memories.mkdir(parents=True)
    (memories / "note.md").write_text("Active dir: .codescout/resident-private/ref\n")
    return base


def test_a_dry_run_changes_nothing_and_says_what_would_move_and_stay(world):
    repo, cache, run = world
    base = scratch_tree(repo)
    before = sorted(str(p) for p in base.rglob("*"))
    result = run("bash", str(MIGRATE))
    assert result.returncode == 0, result.stderr
    out = result.stdout
    assert "  move  old\n" in out and "  move  old2\n" in out, out
    assert "stay  live — live:" in out, out
    assert "stay  ref — referenced:" in out, out
    assert "links left dangling in directories that stay: 1" in out, out
    assert "dry run: nothing was changed" in out
    assert sorted(str(p) for p in base.rglob("*")) == before
    assert not (cache / "myrepo").exists() or not any((cache / "myrepo").iterdir())


def test_apply_moves_closed_directories_retargets_links_and_leaves_pointer_files(world):
    repo, cache, run = world
    base = scratch_tree(repo)
    result = run("bash", str(MIGRATE), "--apply")
    assert result.returncode == 0, result.stdout + result.stderr
    dest = cache / "myrepo"
    assert (dest / "old/data.txt").is_file() and (dest / "old2/data.txt").is_file()
    assert not (base / "old").exists() and not (base / "old2").exists()
    pointer = base / "old.moved"
    assert pointer.is_file() and not pointer.is_symlink(), "a link would bring the walk back"
    assert pointer.read_text().strip() == str(dest / "old")
    # A link between two moved directories follows the move.
    assert os.readlink(dest / "old/to-old2") == str(dest / "old2")
    assert (dest / "old/to-old2").is_dir()
    # A link out of a moved directory into one that stays is left as it was, and still works.
    assert os.readlink(dest / "old/to-live") == str(base / "live")
    # Directories that stay are untouched; a link from one into a moved directory now dangles.
    assert (base / "live/data.txt").is_file() and (base / "ref/data.txt").is_file()
    assert not (base / "live/to-old").exists()


def test_only_restricts_the_move_to_the_named_directory(world):
    repo, cache, run = world
    base = scratch_tree(repo)
    result = run("bash", str(MIGRATE), "--apply", "--only", "old2")
    assert result.returncode == 0, result.stdout + result.stderr
    assert (cache / "myrepo/old2").is_dir()
    assert (base / "old").is_dir() and not (cache / "myrepo/old").exists()


def test_an_existing_destination_is_never_overwritten(world):
    repo, cache, run = world
    base = scratch_tree(repo)
    keep = make_dir(cache / "myrepo", "old", files=("precious.txt",))
    result = run("bash", str(MIGRATE), "--apply")
    assert result.returncode == 0, result.stdout + result.stderr
    assert (keep / "precious.txt").is_file()
    assert (base / "old/data.txt").is_file(), "the source stays when the destination exists"
    assert "stay  old — exists:" in result.stdout


def test_force_referenced_moves_a_directory_a_memory_names(world):
    repo, cache, run = world
    base = scratch_tree(repo)
    result = run("bash", str(MIGRATE), "--apply", "--only", "ref", "--force-referenced")
    assert result.returncode == 0, result.stdout + result.stderr
    assert (cache / "myrepo/ref/data.txt").is_file() and not (base / "ref").exists()
