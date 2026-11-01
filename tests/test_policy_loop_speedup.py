"""The provenance policy loop's speed-up must not change what it answers (bug 20927ccfe293c34e).

Design: docs/superpowers/specs/2026-11-01-policy-loop-speedup-design.md. The old code is kept here as an
oracle, so a change to `normalize` or `write_targets` is compared with what they used to return.
"""
import importlib.util
import inspect
import os
import re
import sys
from pathlib import Path

import pytest

from test_harness_provenance import add_writes, checkout  # noqa: F401  (checkout is a fixture)

SCRIPTS = Path(__file__).resolve().parents[1] / "scripts"
sys.path.insert(0, str(SCRIPTS))


def _load(name="fp_policy_loop"):
    spec = importlib.util.spec_from_file_location(name, SCRIPTS / "file-provenance.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


@pytest.fixture(scope="module")
def fp():
    return _load()


def old_normalize(p, root, base=None):
    """`normalize` as it was before the string prefix test: the oracle."""
    if not p:
        return None
    q = Path(p)
    if not q.is_absolute() and base is not None:
        q = base / q
    if q.is_absolute():
        try:
            return str(q.resolve().relative_to(root))
        except ValueError:
            return None
    return os.path.normpath(p)


def _table(tmp_path, unknown_tree):
    root = tmp_path / "repo"
    (root / "sub").mkdir(parents=True)
    (root / "a.txt").write_text("x")
    outside = tmp_path / "outside"
    outside.mkdir()
    (tmp_path / "repo-other").mkdir()
    (root / "link-out").symlink_to(outside)          # inside the root, points outside
    (outside / "link-in").symlink_to(root)           # outside the root, points inside
    rows = [
        ("inside", str(root / "a.txt"), None),
        ("equal to the root", str(root), None),
        ("outside", str(tmp_path / "elsewhere" / "x"), None),
        ("dot-dot that stays in", str(root / "a" / ".." / "b"), None),
        ("dot-dot that leaves", str(root / ".." / "outside" / "x"), None),
        ("trailing slash", str(root / "sub") + "/", None),
        ("symlinked dir out of the root", str(root / "link-out" / "f"), None),
        ("symlinked dir into the root", str(outside / "link-in" / "a.txt"), None),
        ("sibling sharing the prefix", str(tmp_path / "repo-other" / "x"), None),
        ("relative with a base", "rel/file.txt", root),
        ("relative without a base", "rel/file.txt", None),
        ("relative dot-dot with a base", "../up.txt", root / "sub"),
        ("empty string", "", None),
        ("relative with UNKNOWN_TREE", "x/../y.txt", unknown_tree),
    ]
    return root, rows


def test_normalize_agrees_with_the_old_implementation(fp, tmp_path):
    root, rows = _table(tmp_path, fp.UNKNOWN_TREE)
    for label, p, base in rows:
        assert fp.normalize(p, root, base) == old_normalize(p, root, base), label


def test_normalize_agrees_with_the_old_implementation_for_root_slash(fp):
    for p in ("/etc/hostname", "/", "/usr/bin/../lib/x"):
        assert fp.normalize(p, Path("/")) == old_normalize(p, Path("/")), p


def test_a_sibling_directory_sharing_a_prefix_is_outside(fp, tmp_path):
    root, _ = _table(tmp_path, fp.UNKNOWN_TREE)
    assert fp.normalize(str(root) + "-other/x", root) is None


def test_normalize_does_not_call_path_relative_to(fp, tmp_path, monkeypatch):
    """The speed-up IS not calling it: `Path.relative_to` was 5.8 of 10.8 profiled seconds."""
    root, _ = _table(tmp_path, fp.UNKNOWN_TREE)
    calls = []
    original = Path.relative_to

    def recording(self, *args, **kwargs):
        calls.append(self)
        return original(self, *args, **kwargs)

    monkeypatch.setattr(Path, "relative_to", recording)
    assert fp.normalize(str(root / "a.txt"), root) == "a.txt"
    assert fp.normalize(str(root), root) == "."
    assert fp.normalize(str(tmp_path / "elsewhere" / "x"), root) is None
    assert calls == [], f"normalize called Path.relative_to {len(calls)} times"


def test_the_memo_calls_normalize_once_per_distinct_argument(checkout, monkeypatch):  # noqa: F811
    root, env = checkout
    env = env | {"FILE_PROVENANCE_CACHE_DIR": ""}
    add_writes(root, env, "claude-code", "me", ["own.rs"] * 5 + ["shared.rs"] * 3)
    fp = _load("fp_memo_count")
    seen = []
    real = fp.normalize

    def counting(p, r, base=None):
        seen.append((p, base))
        return real(p, r, base)

    monkeypatch.setattr(fp, "normalize", counting)
    scan = fp.scan_provenance(root, env)
    assert sorted(scan.owners) == ["own.rs", "shared.rs"], "the fixture must produce both paths"
    assert len(seen) == len(set(seen)) == 2, f"{len(seen)} normalize calls for {len(set(seen))} distinct arguments"


def test_the_memo_does_not_outlive_the_call(checkout, tmp_path):  # noqa: F811
    root, env = checkout
    env = env | {"FILE_PROVENANCE_CACHE_DIR": ""}
    inside, outside = root / "real-dir", tmp_path / "outside-dir"
    inside.mkdir()
    outside.mkdir()
    link = root / "link"
    link.symlink_to(inside)
    add_writes(root, env, "claude-code", "me", ["link/f.rs"])
    fp = _load("fp_memo_scope")
    assert "real-dir/f.rs" in fp.scan_provenance(root, env).owners
    retargeted = root / "link.new"
    retargeted.symlink_to(outside)
    os.replace(retargeted, link)
    assert "real-dir/f.rs" not in fp.scan_provenance(root, env).owners, "the second scan reused a stale answer"


# One input carrying every key a `write_targets` branch reads.
RICH = {"path": "p.rs", "file_path": "f.rs", "rel_path": "r.md", "new_rel_path": "n.md",
        "action": "create", "command": "echo x > out.txt",
        "patch": "*** Begin Patch\n*** Add File: patched.rs\n*** End Patch"}

READ_ONLY = [
    "mcp__codescout__read_file", "mcp__codescout__grep", "mcp__codescout__symbols",
    "mcp__codescout__symbol_at", "mcp__codescout__references", "mcp__codescout__call_graph",
    "mcp__codescout__semantic_search", "mcp__codescout__tree", "mcp__codescout__memory",
    "mcp__codescout__workspace", "mcp__codescout__library", "mcp__codescout__librarian",
    "mcp__codescout__index", "mcp__codescout__get_guide", "mcp__codescout__onboarding",
    "mcp__codescout__approve_write", "mcp__researcher__research", "mcp__claude_ai_Claude_Docs__batch",
    "Read", "Glob", "Grep", "WebFetch", "WebSearch", "Agent", "Skill", "ToolSearch", "TodoWrite",
    "TaskCreate", "TaskUpdate", "BashOutput", "KillShell", "NotebookRead", "ExitPlanMode",
    "SendMessage", "ListAgents", "mcp__codescout__no_such_tool", "", "unknown",
]


def _writers(fp):
    return (set(fp.CS_WRITE_TOOLS) | set(fp.NATIVE_WRITE_TOOLS)
            | {"apply_patch", "Bash", "mcp__codescout__run_command", "mcp__codescout__doc",
               "mcp__codescout__artifact", "mcp__codescout__artifact_augment"})


def test_every_tool_that_writes_still_yields_its_targets(fp, tmp_path, monkeypatch):
    monkeypatch.setattr(fp, "catalog_paths", lambda: {})      # `doc` ids read a database
    for name in sorted(_writers(fp)):
        assert list(fp.write_targets(name, RICH, tmp_path)), f"{name} yields nothing for a full input"
    assert "out.txt" in list(fp.write_targets("Bash", RICH, tmp_path))
    assert "out.txt" in list(fp.write_targets("mcp__codescout__run_command", RICH, tmp_path))


def test_a_tool_that_cannot_write_yields_nothing(fp, tmp_path, monkeypatch):
    monkeypatch.setattr(fp, "catalog_paths", lambda: {})
    for name in READ_ONLY:
        assert list(fp.write_targets(name, RICH, tmp_path)) == [], name


def test_the_target_tools_set_is_exactly_the_writers(fp):
    assert fp._TARGET_NAMES == frozenset(_writers(fp))


def test_the_set_names_every_tool_the_branches_name(fp):
    """A branch added to `write_targets` for a new tool name, with the set forgotten, would be
    dropped by the early exit with no failure anywhere else."""
    source = inspect.getsource(fp.write_targets)
    named = set(re.findall(
        r'"(mcp__[a-z_]+__[a-z_]+|Bash|apply_patch|Edit|Write|MultiEdit|NotebookEdit)"', source))
    assert named, "the pattern found no tool name in write_targets: the test is blind"
    assert named <= fp._TARGET_NAMES, sorted(named - fp._TARGET_NAMES)
