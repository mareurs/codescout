"""Write targets stored in the history cache (bug 20927ccfe293c34e, CIW-10).

`GOLDEN` was generated once from `scripts/file-provenance.py` BEFORE the extraction into
`scripts/write_targets.py`; it pins the old output of `write_targets` for every branch.
"""
from __future__ import annotations

import importlib.util
import pickle
import shutil
import sys
from pathlib import Path

import pytest

from test_harness_history import claude_call, claude_result, history
from test_harness_provenance import add_writes, checkout  # noqa: F401  (checkout is a fixture)

SCRIPTS = Path(__file__).resolve().parent.parent / "scripts"
sys.path.insert(0, str(SCRIPTS))

import history_cache as hc  # noqa: E402
from provenance_records import CoverageGap, HistoryBatch, Operation  # noqa: E402

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
def _snapshot(scan):
    return (scan.owners, scan.evidence,
            [(g.source_id, g.reason, g.workspace, g.paths) for g in scan.gaps], scan.streams)


@pytest.fixture
def warmed(checkout, tmp_path):  # noqa: F811
    root, env = checkout
    cache = tmp_path / "cache"
    env = env | {"FILE_PROVENANCE_CACHE_DIR": str(cache)}
    add_writes(root, env, "claude-code", "me", ["own.rs", "shared.rs"])
    return root, env, cache


def _doc_history(root, env, inp):
    rows = [claude_call(name="mcp__codescout__doc", inp=inp, call="d1", sid="me", cwd=root),
            claude_result(call="d1")]
    history(Path(env["FILE_PROVENANCE_ROOTS"]) / "me.jsonl", rows)


def test_a_cached_batch_carries_targets(fp, wt, warmed):
    root, env, cache = warmed
    fp.scan_provenance(root, env)
    ops = []
    for entry in (p for p in cache.iterdir() if p.suffix == ".pkl"):
        with open(entry, "rb") as stream:
            _header, batch = pickle.load(stream)
        ops += batch.operations
    assert ops and any(op.targets for op in ops), "the fixture must store writes with targets"
    for op in ops:
        assert op.targets == tuple(wt.static_targets(op.name, op.input)), op


def _digest(tmp_path, name, edited=None):
    copy = tmp_path / name
    copy.mkdir()
    for f in {*hc._CODE_FILES, "file-provenance.py", "write_targets.py"}:
        shutil.copy(SCRIPTS / f, copy / f)
    if edited:
        with open(copy / edited, "a") as stream:
            stream.write("\n# edited\n")
    return _load(str(copy / "history_cache.py"), "hc_" + name).code_digest()


def test_the_key_changes_with_write_targets_and_not_with_file_provenance(tmp_path):
    assert "write_targets.py" in hc._CODE_FILES
    assert "file-provenance.py" not in hc._CODE_FILES
    base = _digest(tmp_path, "base")
    assert _digest(tmp_path, "wt", edited="write_targets.py") != base
    assert _digest(tmp_path, "fp", edited="file-provenance.py") == base


def test_a_doc_id_target_follows_the_catalog_between_warm_scans(fp, checkout, tmp_path, monkeypatch):  # noqa: F811
    root, env = checkout
    cache = tmp_path / "cache"
    env = env | {"FILE_PROVENANCE_CACHE_DIR": str(cache)}
    _doc_history(root, env, {"action": "update", "id": "fixture-artifact-id"})
    catalog = {"fixture-artifact-id": str(root / "docs" / "x.md")}
    monkeypatch.setattr(fp, "catalog_paths", lambda: catalog)
    first = fp.scan_provenance(root, env)
    assert "docs/x.md" in first.owners
    assert any(p.suffix == ".pkl" for p in cache.iterdir()), "the second scan must be warm"
    catalog["fixture-artifact-id"] = str(root / "docs" / "y.md")
    second = fp.scan_provenance(root, env)
    assert "docs/y.md" in second.owners and "docs/x.md" not in second.owners


def test_an_operation_without_stored_targets_is_recomputed(fp, warmed, monkeypatch):
    root, env, _cache = warmed
    off = env | {"FILE_PROVENANCE_CACHE_DIR": ""}
    filled = _snapshot(fp.scan_provenance(root, off))
    seen, real = [], fp.read_claude_history

    def spy(path):
        batch = real(path)
        seen.extend(op.targets for op in batch.operations)
        return batch

    monkeypatch.setattr(fp, "read_claude_history", spy)
    monkeypatch.setattr(fp, "with_targets", lambda batch: batch)
    bare = _snapshot(fp.scan_provenance(root, off))
    assert seen and all(t is None for t in seen), "the readers must yield bare operations here"
    assert filled[0], "the fixture must produce owners, or the comparison is empty"
    assert bare == filled


def test_a_refused_atomic_write_still_contributes_nothing(fp, checkout, tmp_path, monkeypatch):  # noqa: F811
    root, env = checkout
    refused = Operation("src", "c1", "claude-code:me", None, "2026-10-10T12:00:00Z", root,
                        "mcp__codescout__edit_file", {"path": str(root / "own.rs")},
                        "no-write-refused", True, False, None, (str(root / "own.rs"),))
    monkeypatch.setattr(fp, "discover_claude_histories", lambda r, e: ([tmp_path / "h.jsonl"], []))
    monkeypatch.setattr(fp, "read_claude_history", lambda p: HistoryBatch([refused], []))
    monkeypatch.setattr(fp, "with_targets", lambda batch: batch)
    scan = fp.scan_provenance(root, env | {"FILE_PROVENANCE_CACHE_DIR": ""})
    assert scan.owners == {}, "a refused atomic write must not attribute whatever targets are stored"
    assert not [g for g in scan.gaps if g.reason == "write target is not recoverable"]


def test_filling_targets_tolerates_a_non_dict_input(wt):
    ops = [Operation("s", "c", "p", None, None, None, "Bash", bad, "unpaired", False, False, None)
           for bad in ("junk", None, 5, ["x"])]
    batch = HistoryBatch(ops, [CoverageGap("s", "note", None)])
    filled = wt.with_targets(batch)
    assert [op.targets for op in filled.operations] == [()] * 4
    assert filled.gaps == batch.gaps


def test_static_targets_are_normalized_before_catalog_targets(fp, checkout, monkeypatch):  # noqa: F811
    root, env = checkout
    env = env | {"FILE_PROVENANCE_CACHE_DIR": ""}
    static, catalog = str(root / "docs" / "a.md"), str(root / "docs" / "x.md")
    _doc_history(root, env, {"action": "update", "rel_path": static, "id": "fixture-artifact-id"})
    monkeypatch.setattr(fp, "catalog_paths", lambda: {"fixture-artifact-id": catalog})
    seen, real = [], fp.normalize

    def spy(raw, *rest):
        seen.append(raw)
        return real(raw, *rest)

    monkeypatch.setattr(fp, "normalize", spy)
    fp.scan_provenance(root, env)
    assert seen == [static, catalog]


def test_a_warm_scan_uses_the_stored_targets_and_does_not_recompute_them(fp, warmed, monkeypatch):
    root, env, _cache = warmed
    fp.scan_provenance(root, env)                      # cold: fills the cache
    recomputed, real = [], fp.static_targets

    def spy(name, inp):
        recomputed.append(name)
        return real(name, inp)

    monkeypatch.setattr(fp, "static_targets", spy)
    warm = fp.scan_provenance(root, env)
    assert warm.owners, "the fixture must produce owners, or the check is empty"
    assert recomputed == [], "a warm scan re-derived targets that the cache holds"
