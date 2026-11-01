"""The per-file history cache (bug 20927ccfe293c34e): what it returns, when it misses, what it refuses."""
import importlib.util
import os
import stat
import sys
import time
from pathlib import Path

import pytest

from test_harness_provenance import add_writes, checkout  # noqa: F401  (checkout is a fixture)

SCRIPTS = Path(__file__).resolve().parents[1] / "scripts"
sys.path.insert(0, str(SCRIPTS))

import history_cache as hc  # noqa: E402
from provenance_records import CoverageGap, HistoryBatch  # noqa: E402


def _fp(name="fp_cache"):
    spec = importlib.util.spec_from_file_location(name, SCRIPTS / "file-provenance.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _snapshot(scan):
    return (scan.owners, scan.evidence,
            [(g.source_id, g.reason, g.workspace, g.paths) for g in scan.gaps], scan.streams)


def _counting(fp, monkeypatch):
    calls = {"claude": [], "codex": []}
    claude, codex = fp.read_claude_history, fp.read_codex_history

    def count_claude(path):
        calls["claude"].append(path)
        return claude(path)

    def count_codex(path):
        calls["codex"].append(path)
        return codex(path)

    monkeypatch.setattr(fp, "read_claude_history", count_claude)
    monkeypatch.setattr(fp, "read_codex_history", count_codex)
    return calls


@pytest.fixture
def cached(checkout, tmp_path):  # noqa: F811
    root, env = checkout
    cache = tmp_path / "cache"
    env = env | {"FILE_PROVENANCE_CACHE_DIR": str(cache)}
    claude = add_writes(root, env, "claude-code", "me", ["own.rs", "shared.rs"])
    codex = add_writes(root, env, "codex", "peer", ["shared.rs"])
    return root, env, cache, claude, codex


def test_a_warm_scan_equals_a_cold_scan_and_an_uncached_scan(cached):
    root, env, cache, _, _ = cached
    fp = _fp()
    uncached = _snapshot(fp.scan_provenance(root, env | {"FILE_PROVENANCE_CACHE_DIR": ""}))
    assert not cache.exists()
    cold = _snapshot(fp.scan_provenance(root, env))
    warm = _snapshot(fp.scan_provenance(root, env))
    assert uncached[0], "the fixture must produce owners, or the comparison is empty"
    assert cold == uncached
    assert warm == uncached
    assert sorted(p.suffix for p in cache.iterdir() if p.name != ".pruned") == [".pkl", ".pkl"]
    assert stat.S_IMODE(os.lstat(cache).st_mode) == 0o700


def test_a_warm_scan_does_not_read_the_history_files(cached, monkeypatch):
    root, env, *_ = cached
    fp = _fp()
    calls = _counting(fp, monkeypatch)
    fp.scan_provenance(root, env)
    assert (len(calls["claude"]), len(calls["codex"])) == (1, 1)
    fp.scan_provenance(root, env)
    assert (len(calls["claude"]), len(calls["codex"])) == (1, 1), "the second scan must be served"


def test_a_file_that_changed_is_read_again_and_its_new_writes_are_seen(cached, monkeypatch):
    root, env, cache, claude, _ = cached
    fp = _fp()
    calls = _counting(fp, monkeypatch)
    fp.scan_provenance(root, env)
    add_writes(root, env, "claude-code", "me", ["own.rs", "shared.rs", "peer.rs"])   # new size
    scan = fp.scan_provenance(root, env)
    assert (len(calls["claude"]), len(calls["codex"])) == (2, 1)
    assert [w for w, _ in scan.owners["peer.rs"]] == ["claude-code:me"]
    # The same bytes under a new mtime are a different file to the cache.
    info = os.stat(claude)
    os.utime(claude, ns=(info.st_atime_ns, info.st_mtime_ns + 10**9))
    fp.scan_provenance(root, env)
    assert len(calls["claude"]) == 3


def _batch_reader(calls, reason=None):
    def read(path):
        calls.append(path)
        gaps = [CoverageGap(path.read_text(), reason or "note", None)]
        return HistoryBatch([], gaps)
    return read


def _history_file(tmp_path, text="a"):
    path = tmp_path / "history.jsonl"
    path.write_text(text)
    return path


def test_a_result_with_an_unreadable_source_gap_is_never_stored(tmp_path):
    cache, calls, path = tmp_path / "c", [], _history_file(tmp_path)
    read = hc.caching(_batch_reader(calls, reason="unreadable Claude history source"), cache)
    read(path)
    assert not [p for p in cache.iterdir() if p.suffix == ".pkl"]
    read = hc.caching(_batch_reader(calls), cache)        # permissions fixed: same size, same mtime
    read(path)
    assert len(calls) == 2, "the unreadable answer must not have been served"
    read(path)
    assert len(calls) == 2, "an ordinary result is stored"


def test_a_directory_that_is_not_private_is_not_used(tmp_path):
    path, calls = _history_file(tmp_path), []
    open_dir = tmp_path / "open"
    open_dir.mkdir()
    os.chmod(open_dir, 0o755)
    read = hc.caching(_batch_reader(calls), open_dir)
    read(path); read(path)
    assert len(calls) == 2 and not list(open_dir.iterdir())
    private = tmp_path / "private"
    private.mkdir(mode=0o700)
    link = tmp_path / "link"
    link.symlink_to(private)
    read = hc.caching(_batch_reader(calls), link)
    read(path); read(path)
    assert len(calls) == 4 and not list(private.iterdir()), "a symlinked directory is not a private directory"


def test_a_corrupt_entry_or_a_linked_entry_is_a_miss_and_is_replaced(tmp_path):
    cache, calls, path = tmp_path / "c", [], _history_file(tmp_path)
    read = hc.caching(_batch_reader(calls), cache)
    read(path)
    entry = next(p for p in cache.iterdir() if p.suffix == ".pkl")
    entry.write_bytes(b"not a pickle")
    read(path)
    assert len(calls) == 2
    read(path)
    assert len(calls) == 2, "the corrupt entry was replaced by a good one"
    good = entry.read_bytes()
    entry.unlink()
    elsewhere = tmp_path / "elsewhere.pkl"
    elsewhere.write_bytes(good)
    entry.symlink_to(elsewhere)
    read(path)
    assert len(calls) == 3, "an entry that is a symlink is not followed"


def test_a_change_to_the_reader_code_is_a_miss(tmp_path, monkeypatch):
    cache, calls, path = tmp_path / "c", [], _history_file(tmp_path)
    hc.caching(_batch_reader(calls), cache)(path)
    monkeypatch.setattr(hc, "code_digest", lambda: "another-version-of-the-readers")
    hc.caching(_batch_reader(calls), cache)(path)
    assert len(calls) == 2


def test_a_file_that_cannot_be_statted_is_read_uncached(tmp_path):
    cache, calls = tmp_path / "c", []
    gone = tmp_path / "gone.jsonl"

    def read(path):
        calls.append(path)
        return HistoryBatch([], [CoverageGap(str(path), "unreadable Claude history source", None)])

    batch = hc.caching(read, cache)(gone)
    assert calls == [gone] and batch.gaps


def test_housekeeping_removes_only_stale_entries_and_runs_once_a_day(tmp_path):
    cache, calls = tmp_path / "c", []
    cache.mkdir(mode=0o700)
    now = time.time()
    names = {"old.pkl": 90 * 86400, "fresh.pkl": 86400, ".tmp-old": 2 * 3600, ".tmp-new": 60}
    for name, age in names.items():
        (cache / name).write_bytes(b"x")
        os.utime(cache / name, (now - age, now - age))
    path = _history_file(tmp_path)
    hc.caching(_batch_reader(calls), cache)(path)
    left = {p.name for p in cache.iterdir()}
    assert "old.pkl" not in left and ".tmp-old" not in left
    assert {"fresh.pkl", ".tmp-new", ".pruned"} <= left
    (cache / "old-again.pkl").write_bytes(b"x")
    os.utime(cache / "old-again.pkl", (now - 90 * 86400,) * 2)
    hc.caching(_batch_reader(calls), cache)(path)
    assert (cache / "old-again.pkl").exists(), "the pass ran less than a day ago"


def test_the_cache_location_follows_the_environment(tmp_path):
    home = tmp_path / "home"
    assert hc.cache_dir({"HOME": str(home)}) == home / ".cache" / "codescout" / "history-cache"
    xdg = tmp_path / "xdg"
    assert hc.cache_dir({"HOME": str(home), "XDG_CACHE_HOME": str(xdg)}) == xdg / "codescout" / "history-cache"
    assert hc.cache_dir({"HOME": str(home), "XDG_CACHE_HOME": "relative"}) == home / ".cache" / "codescout" / "history-cache"
    assert hc.cache_dir({"FILE_PROVENANCE_CACHE_DIR": str(tmp_path / "x")}) == tmp_path / "x"
    assert hc.cache_dir({"FILE_PROVENANCE_CACHE_DIR": ""}) is None
    assert hc.cache_dir({"FILE_PROVENANCE_CACHE_DIR": "relative"}) is None


def test_the_digest_covers_files_that_exist():
    for name in hc._CODE_FILES:
        assert (SCRIPTS / name).is_file(), f"{name} is hashed into every key but does not exist"
    assert hc.code_digest()
