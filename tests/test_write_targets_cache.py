"""Write targets stored in the history cache (bug 20927ccfe293c34e, CIW-10).

`GOLDEN` was generated once from `scripts/file-provenance.py` BEFORE the extraction into
`scripts/write_targets.py`; it pins the old output of `write_targets` for every branch.
"""
from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

import pytest

SCRIPTS = Path(__file__).resolve().parent.parent / "scripts"

# The catalog fixture id is deliberately not 16-hex: a 16-hex literal in a committed doc reads
# as a dead artifact citation.
CATALOG = {"fixture-artifact-id": "docs/x.md"}

_PATCH = ("*** Begin Patch\n*** Add File: a.txt\n*** Update File: b.txt\n*** Delete File: c.txt\n"
          "*** Move to: d.txt\n*** End Patch")

CASES = [
    ("apply_patch", {"patch": _PATCH}),
    ("apply_patch", {"patch": "not a patch"}),
    ("apply_patch", {"patch": 7}),
    ("mcp__codescout__edit_file", {"path": "p1.md"}),
    ("mcp__codescout__edit_file", {"path": "p2.md", "file_path": "p3.md"}),
    ("mcp__codescout__edit_code", {"path": "src/a.rs"}),
    ("mcp__codescout__create_file", {"path": "src/new.rs", "content": "x"}),
    ("mcp__codescout__edit_markdown", {"path": "m.md"}),
    ("mcp__codescout__replace_symbol", {"path": "r.rs"}),
    ("mcp__codescout__insert_code", {"path": "i.rs"}),
    ("mcp__codescout__remove_symbol", {"path": "x.rs"}),
    ("mcp__codescout__edit_file", {"path": 5}),
    ("Edit", {"file_path": "e.py"}),
    ("Write", {"file_path": "w.py"}),
    ("MultiEdit", {"file_path": "me.py"}),
    ("NotebookEdit", {"file_path": "n.ipynb"}),
    ("Edit", {"file_path": None}),
    ("mcp__codescout__doc", {"action": "update", "id": "fixture-artifact-id"}),
    ("mcp__codescout__doc", {"action": "update", "rel_path": "docs/a.md",
                             "new_rel_path": "docs/b.md", "id": "fixture-artifact-id"}),
    ("mcp__codescout__doc", {"action": "move", "src_id": "fixture-artifact-id",
                             "dst_id": "unknown-id", "into_id": "fixture-artifact-id"}),
    ("mcp__codescout__doc", {"action": "append_entry", "artifact_id": "fixture-artifact-id"}),
    ("mcp__codescout__doc", {"action": "augment", "id": "fixture-artifact-id"}),
    ("mcp__codescout__doc", {"action": "find", "rel_path": "docs/a.md",
                             "id": "fixture-artifact-id"}),
    ("mcp__codescout__doc", {"action": "update", "id": "unknown-id"}),
    ("mcp__codescout__artifact", {"action": "create", "rel_path": "docs/l.md"}),
    ("mcp__codescout__artifact_augment", {"id": "fixture-artifact-id"}),
    ("Bash", {"command": "echo hi > out.txt"}),
    ("Bash", {"command": "echo hi >> log.txt && cat in.txt"}),
    ("Bash", {"command": "sed -i 's/a/b/' f.txt"}),
    ("Bash", {"command": "ls | tee -a t.txt"}),
    ("Bash", {"command": "rm -rf gone.txt"}),
    ("Bash", {"command": "touch made.txt"}),
    ("Bash", {"command": "git mv old.md new.md"}),
    ("Bash", {"command": "mv a b"}),
    ("Bash", {"command": "cp src dst"}),
    ("Bash", {"command": "git checkout -- one two"}),
    ("Bash", {"command": "cargo install thing"}),
    ("Bash", {"command": 'python3 -c "open(\'lit.txt\', \'w\').write(\'x\')"'}),
    ("Bash", {"command": 'p = "var.txt"\nPath(p).write_text("x")'}),
    ("Bash", {"command": 'F = Path("pv.txt")\nF.write_text("x")'}),
    ("Bash", {"command": 5}),
    ("mcp__codescout__run_command", {"command": "echo hi > rc.txt"}),
    ("mcp__codescout__run_command", {"command": "cp a rc2 ; mv x y"}),
    ("mcp__codescout__read_file", {"path": "readonly.md"}),
    ("Read", {"file_path": "readonly.py"}),
    ("mcp__codescout__doc", "not a dict"),
    ("Bash", None),
    ("unknown_tool", {"path": "z"}),
]

GOLDEN = [
    ['a.txt', 'b.txt', 'c.txt', 'd.txt'],
    [],
    [],
    ['p1.md'],
    ['p2.md', 'p3.md'],
    ['src/a.rs'],
    ['src/new.rs'],
    ['m.md'],
    ['r.rs'],
    ['i.rs'],
    ['x.rs'],
    [],
    ['e.py'],
    ['w.py'],
    ['me.py'],
    ['n.ipynb'],
    [],
    ['docs/x.md'],
    ['docs/a.md', 'docs/b.md', 'docs/x.md'],
    ['docs/x.md', 'docs/x.md'],
    ['docs/x.md'],
    ['docs/x.md'],
    [],
    [],
    ['docs/l.md'],
    ['docs/x.md'],
    ['out.txt'],
    ['log.txt'],
    ['f.txt'],
    ['t.txt'],
    ['gone.txt'],
    ['made.txt'],
    ['old.md', 'new.md'],
    ['a', 'b'],
    ['dst'],
    ['one', 'two'],
    [],
    ['lit.txt'],
    ['var.txt'],
    ['pv.txt'],
    [],
    ['rc.txt'],
    ['rc2', 'x', 'y'],
    [],
    [],
    [],
    [],
    [],
]


def _load(path_name: str, mod_name: str):
    spec = importlib.util.spec_from_file_location(mod_name, SCRIPTS / path_name)
    module = importlib.util.module_from_spec(spec)
    sys.modules[mod_name] = module
    spec.loader.exec_module(module)
    return module


@pytest.fixture
def fp(monkeypatch):
    module = _load("file-provenance.py", "fp_write_targets_cache")
    monkeypatch.setattr(module, "catalog_paths", lambda: dict(CATALOG))
    return module


@pytest.fixture
def wt():
    sys.path.insert(0, str(SCRIPTS))
    try:
        import write_targets
        return write_targets
    finally:
        sys.path.remove(str(SCRIPTS))


def test_write_targets_matches_the_golden_table(fp, tmp_path):
    for (name, inp), want in zip(CASES, GOLDEN, strict=True):
        assert list(fp.write_targets(name, inp, tmp_path)) == want, (name, inp)
def test_static_then_catalog_is_write_targets(fp, wt, tmp_path):
    for (name, inp), want in zip(CASES, GOLDEN, strict=True):
        got = list(wt.static_targets(name, inp)) + list(fp.catalog_targets(name, inp))
        assert got == want, (name, inp)


def test_static_targets_never_reads_the_catalog(fp, wt, monkeypatch):
    def boom(*_a, **_k):
        raise AssertionError("static_targets read the catalog")

    monkeypatch.setattr(fp, "catalog_paths", boom)
    monkeypatch.setattr(wt, "catalog_paths", boom, raising=False)
    for name, inp in CASES:
        list(wt.static_targets(name, inp))


def test_the_moved_names_are_still_attributes_of_file_provenance(fp, wt):
    for attr in ("CS_WRITE_TOOLS", "NATIVE_WRITE_TOOLS", "ARTIFACT_WRITE_ACTIONS",
                 "_TARGET_NAMES", "_Q", "PY_WRITE_LITERAL", "PY_WRITE_VAR", "PY_ASSIGN",
                 "BASH_WRITE_PATTERNS", "RELOCATORS", "python_write_targets",
                 "patch_write_targets", "relocation_targets", "_operands"):
        assert getattr(fp, attr) is getattr(wt, attr), attr
