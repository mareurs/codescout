"""Leakage scanner: no byte an agent can reach may contain the answer.

The answer is two things: the hidden tests' names and the gold patch's added lines. `needles_for`
turns a task into needles; `scan` / `scan_report` look for them in every byte of the given roots
(repository tree, `.codescout/`, baked home state such as a SQLite catalog, the `full` arm's config
copy) and in a Qdrant payload dump.

Needle kinds (decided by the needle string itself, so `scan` takes a plain `list[str]`):
  * name needle: no whitespace. Matched WHOLE-WORD (not preceded or followed by [A-Za-z0-9_]) in
    file bytes and in each file's path relative to its root. Covers test function names and
    repo-relative paths of hidden test files.
  * shingle needle: tokens joined by single spaces, at least `n` tokens. Matched as a window of
    consecutive tokens in the file's token stream; line breaks and spacing are ignored, so
    reformatting a leaked line does not hide it.

Tokenisation (SHINGLE side and FILE side are identical): the bytes/text are split with
`[A-Za-z0-9_]+`; everything else (punctuation, whitespace, newlines) is a separator and dropped.

Shingle construction (`needles_for`): for each ADDED line of a unified diff (`+` but not `+++`),
tokenise; a line with fewer than 8 tokens contributes nothing; otherwise every window of 8
consecutive tokens of that line is a shingle, except a window whose tokens are ALL in
COMMON_TOKENS (case-insensitive: keywords and trivial identifiers). Shingles are per line, never
spanning lines of the patch.

Files are read as raw bytes (SQLite and other binaries included; nothing is decoded). Reports are
bounded: one Hit per (file, needle), at the first occurrence. Offset is a byte offset into the
file, or -1 when the needle matched the file's NAME/relative path. A Qdrant dump, and any
`*.json`/`*.jsonl` file, is additionally scanned through a same-length JSON-unescaped view (`\\n`
becomes space+newline etc.) so a payload string is not hidden by its escapes; `\\uXXXX` escapes
are not decoded. Compressed content (git packs, zip) cannot be seen by a byte scan.

`scan` is strict: it raises EmptyScan if no file was scanned and ScanIncomplete if any file could
not be read, so a scan that saw nothing never reads as "0 hits". `scan_report` returns the counts.
"""
import ast
import json
import os
import re
from pathlib import Path
from typing import Iterable, NamedTuple

SHINGLE_TOKENS = 8

COMMON_TOKENS = frozenset(
    """
    a an the and or not is in of to for if else elif while loop match case return break continue
    let mut const static pub fn def class struct enum impl trait use mod crate super self this
    import from as with new var type where async await move ref dyn box
    true false none null nil some ok err result unit
    i x y n s t e f v
    """.split()
)

_TOKEN = re.compile(rb"[A-Za-z0-9_]+")
_WORD = rb"[A-Za-z0-9_]"
_ESCAPE = re.compile(rb'\\(["\\/bfnrt])')
_ESCAPE_TO = {b"n": b"\n", b"t": b"\t", b"r": b"\r"}


class Hit(NamedTuple):
    path: Path
    needle: str
    offset: int  # byte offset in the file; -1 = matched the file's name / relative path


class ScanReport(NamedTuple):
    hits: list
    files_scanned: int
    bytes_scanned: int
    skipped: list  # (path, reason) for every file that could not be read


class _Acc:
    def __init__(self):
        self.hits, self.skipped = [], []
        self.files_scanned = self.bytes_scanned = 0


class EmptyScan(Exception):
    """No file was scanned: the result says nothing about leakage."""


class ScanIncomplete(Exception):
    """Some files could not be read, so a clean result would be unproven."""


def _tokens(text: str) -> list:
    return _TOKEN.findall(text.encode("utf-8", "replace"))


def _added_lines(diff: str) -> Iterable[str]:
    for line in diff.splitlines():
        if line.startswith("+") and not line.startswith("+++"):
            yield line[1:]


def _line_shingles(line: str, n: int = SHINGLE_TOKENS) -> list:
    toks = [t.decode() for t in _tokens(line)]
    if len(toks) < n:
        return []
    out = []
    for i in range(len(toks) - n + 1):
        window = toks[i:i + n]
        if all(t.lower() in COMMON_TOKENS for t in window):
            continue
        out.append(" ".join(window))
    return out


def _shingles_of_diff(diff: str) -> list:
    return [s for line in _added_lines(diff) for s in _line_shingles(line)]


def _test_name(raw: str) -> str:
    """Final function-name segment of a test id (`a::b::c`, `a.b#c`, `p.py::T::t[x]`, `t (m.C)`)."""
    s = re.sub(r"\[.*\]\s*$", "", raw.strip())
    s = s.split()[0] if s.split() else ""
    parts = [p for p in re.split(r"::|[#./]", s) if p]
    name = parts[-1] if parts else ""
    if not re.fullmatch(r"\w+", name):
        raise ValueError(f"cannot derive a test-function name from {raw!r}")
    return name


def _f2p_list(value) -> list:
    if isinstance(value, str):
        try:  # the real dataset stores a Python repr (single quotes); JSON is accepted too
            value = json.loads(value) if value.strip() else []
        except json.JSONDecodeError:
            value = ast.literal_eval(value)
    return list(value)


def _dedupe(items: Iterable[str]) -> list:
    return list(dict.fromkeys(items))


def needles_for(task: dict, patch_text: str | None = None, extra_texts: Iterable[str] = ()) -> list:
    """Needles for one task. A pure function over already-loaded text (Task 7 loads it).

    S5 task (has `f2p`): `patch_text` is the gold patch (`git diff <parent> <fix>`) and is
    REQUIRED; without it the shingles would silently be missing. Needles: the final segment of each
    `f2p.names`, the repo-relative path of each `hidden.extra` file (these exist nowhere in the
    tree, so a hit is a leak), and shingles of the gold patch's added lines. `hidden.files` are not
    needles: they exist in the tree legitimately (only their hidden test content is secret).
    SWE-bench record (has `test_patch`): `patch_text` is ignored; names come from FAIL_TO_PASS and
    from `def|fn test...` lines the test_patch adds; shingles from both `patch` and `test_patch`.
    `extra_texts`: whole texts (e.g. hidden test file contents) all of whose lines are shingled.
    """
    needles: list = []
    shingles: list = []
    if "f2p" in task:
        if patch_text is None:
            raise ValueError("S5 task: patch_text (the gold patch) is required")
        needles += [_test_name(n) for n in task["f2p"].get("names", [])]
        needles += list(task.get("hidden", {}).get("extra", []))
        shingles += _shingles_of_diff(patch_text)
    elif "test_patch" in task:
        needles += [_test_name(n) for n in _f2p_list(task.get("FAIL_TO_PASS", []))]
        for line in _added_lines(task["test_patch"]):
            m = re.match(r"\s*(?:async\s+)?(?:def|fn)\s+(test\w*)", line)
            if m:
                needles.append(m.group(1))
            m = re.search(r"\b(?:void|public)\b[^(]*?\b(test\w*)\s*\(", line)
            if m:
                needles.append(m.group(1))
        shingles += _shingles_of_diff(task["test_patch"]) + _shingles_of_diff(task.get("patch", ""))
    else:
        raise ValueError("unknown task shape: expected S5 (`f2p`) or SWE-bench (`test_patch`)")
    for text in extra_texts:
        shingles += [s for line in text.splitlines() for s in _line_shingles(line)]
    return _dedupe(needles + shingles)


class _Matcher:
    def __init__(self, needles: list, n: int):
        if not needles:
            raise ValueError("no needles: a scan for nothing proves nothing")
        names, self.shingles = [], {}
        for needle in needles:
            if not needle.strip():
                raise ValueError("empty needle")
            if re.search(r"\s", needle):
                toks = needle.encode().split()
                if len(toks) < n:
                    raise ValueError(f"shingle needle has {len(toks)} tokens, fewer than n={n}: {needle!r}")
                self.shingles[b" ".join(toks)] = needle
            else:
                names.append(needle)
        self.name_needle = {nm.encode(): nm for nm in names}
        self.name_re = None
        if names:
            alt = b"|".join(re.escape(nm.encode()) for nm in sorted(set(names), key=len, reverse=True))
            self.name_re = re.compile(rb"(?<!" + _WORD + rb")(?:" + alt + rb")(?!" + _WORD + rb")")
        self.sizes = sorted({k.count(b" ") + 1 for k in self.shingles})
        self.firsts = {k.split(b" ", 1)[0] for k in self.shingles}

    def search(self, data: bytes) -> dict:
        """needle -> first byte offset."""
        found: dict = {}
        if self.name_re is not None:
            for m in self.name_re.finditer(data):
                found.setdefault(self.name_needle[m.group()], m.start())
        if self.shingles:
            toks = [(m.group(), m.start()) for m in _TOKEN.finditer(data)]
            words = [t for t, _ in toks]
            for k in self.sizes:
                for i, w in enumerate(words):
                    if w in self.firsts and i + k <= len(words):
                        key = b" ".join(words[i:i + k])
                        needle = self.shingles.get(key)
                        if needle is not None:
                            found.setdefault(needle, toks[i][1])
        return found

    def search_path(self, rel: str) -> list:
        if self.name_re is None:
            return []
        return _dedupe(self.name_needle[m.group()] for m in self.name_re.finditer(rel.encode("utf-8", "replace")))


def _unescape_json_view(data: bytes) -> bytes:
    """Same length as `data`: each `\\x` JSON escape becomes space + the decoded char (or space)."""
    return _ESCAPE.sub(lambda m: b" " + _ESCAPE_TO.get(m.group(1), b" "), data)


def _scan_file(path: Path, rel: str, matcher: _Matcher, json_view: bool, report: _Acc) -> None:
    try:
        data = path.read_bytes()
    except OSError as e:
        report.skipped.append((path, f"{type(e).__name__}: {e}"))
        return
    report.hits.extend(Hit(path, nd, -1) for nd in matcher.search_path(rel))
    found = matcher.search(data)
    if json_view and b"\\" in data:
        for nd, off in matcher.search(_unescape_json_view(data)).items():
            found[nd] = min(off, found.get(nd, off))
    report.hits.extend(Hit(path, nd, off) for nd, off in found.items())
    report.files_scanned += 1
    report.bytes_scanned += len(data)


def scan_report(roots: list, qdrant_dump: Path | None, needles: list, n: int = SHINGLE_TOKENS) -> ScanReport:
    """Scan every file under `roots` (and the Qdrant dump) and report hits plus what was covered."""
    matcher = _Matcher(list(needles), n)
    report = _Acc()
    for root in map(Path, roots):
        if not root.exists():
            raise FileNotFoundError(f"scan root does not exist: {root}")
        if root.is_file():
            _scan_file(root, root.name, matcher, root.suffix in (".json", ".jsonl"), report)
            continue
        seen = {os.path.realpath(root)}
        for dirpath, dirnames, filenames in os.walk(root, followlinks=True):
            keep = []
            for d in dirnames:  # a symlink loop or repeated link target is walked once
                real = os.path.realpath(os.path.join(dirpath, d))
                if real not in seen:
                    seen.add(real)
                    keep.append(d)
            dirnames[:] = keep
            for fn in filenames:
                p = Path(dirpath) / fn
                _scan_file(p, p.relative_to(root).as_posix(), matcher, p.suffix in (".json", ".jsonl"), report)
    if qdrant_dump is not None:
        dump = Path(qdrant_dump)
        if not dump.is_file():
            raise FileNotFoundError(f"qdrant dump does not exist: {dump}")
        _scan_file(dump, dump.name, matcher, True, report)
    return ScanReport(report.hits, report.files_scanned, report.bytes_scanned, report.skipped)


def scan(roots: list, qdrant_dump: Path | None, needles: list, n: int = SHINGLE_TOKENS) -> list:
    """Hits for `needles` under `roots` and in the dump. Raises rather than return a vacuous [].

    EmptyScan: zero files scanned. ScanIncomplete: some file was unreadable.
    Use `scan_report` for the files/bytes scanned counts.
    """
    report = scan_report(roots, qdrant_dump, needles, n)
    if report.files_scanned == 0:
        raise EmptyScan("scanned 0 files: nothing was checked, so this is not a clean result")
    if report.skipped:
        raise ScanIncomplete(f"{len(report.skipped)} file(s) unreadable, first: {report.skipped[0]}")
    return report.hits
