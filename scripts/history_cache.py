"""A per-file cache for the history readers (bug 20927ccfe293c34e).

The red-attribution engine decodes every Claude and Codex history file on the machine on each
call: 9.1 GB on this checkout, about 240 s, against a 30 s ceiling that silences the hint. A
reader's result is a function of one file's bytes and path, so it can be stored and reused
while the file is unchanged. A file that grows is decoded again in full: the readers keep state
across lines (call and result pairing, enqueue boundaries), so there is no incremental form.

Key and invalidation. One entry per history file, replaced when the file changes. The entry
records `(code digest, path, size, mtime_ns)` and is used only when all four match what the
file reports NOW. The digest covers the readers and the modules they import, the Python
version, and the pickle protocol, so changing a reader empties the cache. The file is `stat`ed
BEFORE it is read: a file that grows during the read then stores under its old size and misses
next time, where the other order would store old content under the new size and hit forever.

What is never stored. A result carrying an "unreadable ... source" gap is not a function of
the file's bytes: it depends on permissions, which size and mtime do not show. Storing it would
keep reporting a file as unreadable after it was fixed.

Where. `$FILE_PROVENANCE_CACHE_DIR` when set (empty disables), else
`$XDG_CACHE_HOME/codescout/history-cache`, else `~/.cache/codescout/history-cache`. The entries
are pickles, and loading a pickle runs code, so the directory is used only when it is a real
directory (not a symlink) owned by this user with no group or other access, and each entry is
opened without following links and must be a regular file owned by this user. Anything else, and
any error at all, falls back to reading the file: the cache can only ever cost a stale miss, never
change an answer.

Progress survives a timeout. Each entry is written as soon as its file is read, and the engine
reads files in one fixed order. A cold scan that the caller stops at its ceiling has stored every
file it finished, so the next call starts further along.

Housekeeping. A pass at most once a day removes entries not used for 60 days and temp files older
than an hour (a killed writer leaves one).
"""
from __future__ import annotations

import functools
import hashlib
import os
import pickle
import stat
import sys
import tempfile
import time
from pathlib import Path
from typing import Callable, Mapping

SCHEMA = 1
PICKLE_PROTOCOL = 4

# The modules whose bytes decide what a reader returns. A change to any of them is a new key.
_CODE_FILES = ("history_cache.py", "claude_history.py", "codex_history.py",
               "provenance_records.py", "session_context.py", "write_targets.py")

PRUNE_AFTER_SECONDS = 60 * 24 * 60 * 60
PRUNE_EVERY_SECONDS = 24 * 60 * 60
TEMP_AFTER_SECONDS = 60 * 60
_PRUNE_MARKER = ".pruned"
_TEMP_PREFIX = ".tmp-"


def cache_dir(environ: Mapping[str, str]) -> Path | None:
    """The directory to use, or None when caching is off. Does not create it."""
    override = environ.get("FILE_PROVENANCE_CACHE_DIR")
    if override is not None:
        return Path(override) if override and Path(override).is_absolute() else None
    base = environ.get("XDG_CACHE_HOME")
    if not base or not Path(base).is_absolute():
        base = str(Path(environ.get("HOME", str(Path.home()))) / ".cache")
    if not Path(base).is_absolute():
        return None
    return Path(base) / "codescout" / "history-cache"


@functools.lru_cache(maxsize=1)
def code_digest() -> str:
    """Hash of the reader code, the Python version and the pickle protocol; "" when unreadable."""
    here = Path(__file__).resolve().parent
    digest = hashlib.sha256(f"{SCHEMA}|{sys.version_info[:3]}|{PICKLE_PROTOCOL}".encode())
    for name in _CODE_FILES:
        try:
            body = (here / name).read_bytes()
        except OSError:
            return ""
        digest.update(name.encode() + b"\0" + body + b"\0")
    return digest.hexdigest()


def _usable(directory: Path, create: bool) -> bool:
    """Is `directory` a real directory this user owns, closed to everyone else?"""
    if not hasattr(os, "getuid"):
        return False
    try:
        if create:
            directory.mkdir(mode=0o700, parents=True, exist_ok=True)
        info = os.lstat(directory)
    except OSError:
        return False
    return (stat.S_ISDIR(info.st_mode) and info.st_uid == os.getuid()
            and not info.st_mode & 0o077)


def _entry_path(directory: Path, path: Path) -> Path:
    return directory / (hashlib.sha256(str(path).encode()).hexdigest()[:40] + ".pkl")


def _cacheable(batch) -> bool:
    """False when the result depends on something other than the file's bytes."""
    return not any(gap.reason.startswith("unreadable") for gap in batch.gaps)


def _load(entry: Path, header: tuple):
    """The stored batch when `entry` is a safe file whose header equals `header`, else None."""
    try:
        fd = os.open(entry, os.O_RDONLY | os.O_NOFOLLOW)
    except OSError:
        return None
    try:
        info = os.fstat(fd)
        if not stat.S_ISREG(info.st_mode) or info.st_uid != os.getuid():
            return None
        with os.fdopen(fd, "rb") as stream:
            fd = -1
            stored, batch = pickle.load(stream)
    except Exception:
        return None
    finally:
        if fd >= 0:
            os.close(fd)
    return batch if stored == header else None


def _store(entry: Path, header: tuple, batch) -> None:
    """Write `entry` atomically; a failure is ignored, since the cache is best effort."""
    try:
        fd, temp = tempfile.mkstemp(dir=entry.parent, prefix=_TEMP_PREFIX)
        try:
            with os.fdopen(fd, "wb") as stream:
                pickle.dump((header, batch), stream, protocol=PICKLE_PROTOCOL)
            os.replace(temp, entry)
        except BaseException:
            try:
                os.unlink(temp)
            except OSError:
                pass
            raise
    except Exception:
        pass


def _prune(directory: Path) -> None:
    """Remove stale entries and orphaned temp files, at most once per PRUNE_EVERY_SECONDS."""
    now = time.time()
    marker = directory / _PRUNE_MARKER
    try:
        if now - os.stat(marker).st_mtime < PRUNE_EVERY_SECONDS:
            return
    except OSError:
        pass
    try:
        with os.scandir(directory) as entries:
            for item in entries:
                try:
                    if not item.is_file(follow_symlinks=False):
                        continue
                    age = now - item.stat(follow_symlinks=False).st_mtime
                    stale = ((item.name.startswith(_TEMP_PREFIX) and age > TEMP_AFTER_SECONDS)
                             or (item.name.endswith(".pkl") and age > PRUNE_AFTER_SECONDS))
                    if stale:
                        os.unlink(item.path)
                except OSError:
                    continue
        marker.touch()
    except OSError:
        pass


def caching(reader: Callable[[Path], object], directory: Path | None) -> Callable[[Path], object]:
    """`reader` with a per-file cache in `directory`; `reader` itself when caching is off."""
    digest = code_digest()
    if directory is None or not digest:
        return reader
    ready: list[bool] = []

    def read(path: Path):
        if not ready:
            ready.append(_usable(directory, create=True))
            if ready[0]:
                _prune(directory)
        if not ready[0]:
            return reader(path)
        try:
            info = os.stat(path)
        except OSError:
            return reader(path)
        header = (digest, str(path), info.st_size, info.st_mtime_ns)
        entry = _entry_path(directory, path)
        hit = _load(entry, header)
        if hit is not None:
            try:
                os.utime(entry)
            except OSError:
                pass
            return hit
        batch = reader(path)
        if _cacheable(batch):
            _store(entry, header, batch)
        return batch

    return read
