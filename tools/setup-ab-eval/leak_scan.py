"""Leakage scanner: no byte an agent can reach may contain the answer.

The answer is: the hidden tests' names, the repo paths of hidden test files, and the gold patch's
added lines (plus the hidden test files' contents). The flow for one task:

    raw = needles_for(task, gold_patch, hidden_texts)       # candidates, NOT yet usable
    ns = baseline_needles(raw, parent_tree)                 # drop what the clean parent already has
    report = scan(roots, qdrant_dump, ns)                   # strict; raises rather than read clean
    report = positive_control(roots, raw, parent_tree)      # same, plus a planted-needle self-test

BASELINE (required). A gold patch routinely adds lines that already exist in the parent commit
(moves into archive dirs, repeated idioms), and an overlay hidden test file shares most lines with
its parent version. Such needles hit a perfectly clean tree, so the gate could never read 0.
`baseline_needles` searches the clean parent tree (a directory, or a callable yielding
`(relative_path, bytes)`) with the SAME matcher the scan uses and drops every needle found there;
it reports kept/dropped counts per kind. `scan` refuses a raw list of needles (NoBaseline) unless
the caller passes `allow_unbaselined=True` (synthetic needles only). A hidden.extra path needle
therefore survives only if the parent has no such file (nor any content mentioning the path).

Needle kinds (decided by the needle string itself):
  * name: no whitespace, no "/". WHOLE-WORD (not adjacent to [A-Za-z0-9_]) in file bytes and in
    each file's path relative to its root.
  * path: no whitespace, contains "/". Same matching; a repo-relative path of a hidden test file.
  * shingle: tokens joined by single spaces, at least `n` tokens, every token fully matching the
    token regex (else ValueError). Matched as a window of consecutive tokens in the file's token
    stream; line breaks and spacing are ignored, so reformatting a leaked line does not hide it.

Tokenisation (patch side and file side identical): split on `[A-Za-z0-9_]+`; everything else is a
separator and dropped.

Shingle construction: a RUN is a maximal sequence of consecutive added lines of a diff (`+` but
not `+++`; a context line, a removed line or a hunk header ends the run), or the whole text of a
hidden file. Shingles SPAN the lines of a run: tokens of the run are concatenated and every window
of 8 consecutive tokens is a shingle, except a window whose tokens are ALL in COMMON_TOKENS
(case-insensitive keywords and trivial identifiers). A run with fewer than 8 tokens contributes
none, so `}` or `Ok(())` alone never become needles. (Spanning, because the file side already
ignores line breaks: a leaked statement wrapped differently still matches.)

Refusals: needles_for raises if the gold patch has added tokens but yields zero shingles (unless
`allow_no_shingles=True`); baseline_needles raises the same way if the baseline removed every
shingle of such a task; hidden.extra non-empty requires `hidden_texts` for every extra path.

Files are read as raw bytes (nothing decoded). One Hit per (file, needle), first occurrence;
offset is a byte offset, or -1 when the needle matched the file's name / relative path. Every
file whose bytes contain a backslash is also searched through a same-length JSON-unescaped view
(`\\n` becomes space+newline) so a payload string is not hidden by its escapes (`\\uXXXX` is not
decoded). Non-regular files (FIFO, socket, device), unreadable files and unreadable directories
are recorded in `skipped`; `scan` then raises ScanIncomplete. Known blind spots: compressed
content (git packs, zip), UTF-16, and SQLite values that straddle an overflow-page boundary.
"""
import ast
import json
import os
import re
import stat
import tempfile
from pathlib import Path
from typing import Callable, Iterable, Iterator, NamedTuple

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
_TOKEN_STR = re.compile(r"[A-Za-z0-9_]+")
_WORD = rb"[A-Za-z0-9_]"
_ESCAPE = re.compile(rb'\\(["\\/bfnrt])')
_ESCAPE_TO = {b"n": b"\n", b"t": b"\t", b"r": b"\r"}
KINDS = ("name", "path", "shingle")


class Hit(NamedTuple):
    path: Path
    needle: str
    offset: int  # byte offset in the file; -1 = matched the file's name / relative path


class NeedleSet(NamedTuple):
    """Needles that have been checked against the clean parent. `scan` accepts this."""
    needles: list
    kept: dict  # kind -> count
    dropped: dict  # kind -> count removed because the clean parent already contains them
    baseline_files: int
    baseline_bytes: int


class ScanReport(NamedTuple):
    hits: list
    files_scanned: int
    bytes_scanned: int
    skipped: list  # (path, reason)
    per_root: list  # (label, files, bytes), one per root, then the dump
    needle_counts: dict  # kind -> count of needles scanned for
    dropped: dict  # kind -> count dropped by the baseline


class EmptyScan(Exception):
    """A root (or the whole scan) covered 0 files: the result says nothing about leakage."""


class ScanIncomplete(Exception):
    """Some files or directories could not be read, so a clean result would be unproven."""


class NoBaseline(Exception):
    """Needles were not checked against the clean parent tree."""


class ControlFailed(Exception):
    """The planted positive-control needle was not found exactly: the scanner is not trustworthy."""


class _Acc:
    def __init__(self):
        self.hits, self.skipped = [], []
        self.files_scanned = self.bytes_scanned = 0


def kind_of(needle: str) -> str:
    if re.search(r"\s", needle):
        return "shingle"
    return "path" if "/" in needle else "name"


def _count_kinds(needles: Iterable[str]) -> dict:
    out = dict.fromkeys(KINDS, 0)
    for nd in needles:
        out[kind_of(nd)] += 1
    return out


def _tokens(text: str) -> list:
    return _TOKEN_STR.findall(text)


def _runs(diff: str) -> Iterator[list]:
    run: list = []
    for line in diff.splitlines():
        if line.startswith("+") and not line.startswith("+++"):
            run.append(line[1:])
        elif run:
            yield run
            run = []
    if run:
        yield run


def _window_shingles(toks: list, n: int = SHINGLE_TOKENS) -> list:
    out = []
    for i in range(len(toks) - n + 1):
        window = toks[i:i + n]
        if not all(t.lower() in COMMON_TOKENS for t in window):
            out.append(" ".join(window))
    return out


def _diff_shingles(diff: str) -> tuple:
    """(shingles, added_token_count) over the runs of added lines of a diff."""
    shingles, total = [], 0
    for run in _runs(diff):
        toks = _tokens("\n".join(run))
        total += len(toks)
        shingles += _window_shingles(toks)
    return shingles, total


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


def needles_for(task: dict, patch_text: str | None = None, hidden_texts: dict | None = None,
                allow_no_shingles: bool = False) -> list:
    """Candidate needles for one task (pure function over loaded text). NOT yet baselined.

    S5 task (has `f2p`): `patch_text` is the gold patch (`git diff parent fix`) and is REQUIRED.
    Needles: final segment of each `f2p.names`; each `hidden.extra` repo-relative path; shingles of
    the gold patch's added-line runs. When `hidden.extra` is non-empty, `hidden_texts` (path ->
    file content) must cover every extra path (the hidden test body is not in the fix diff) and
    each text is shingled as one run; overlap with the parent version is removed by the baseline.
    `hidden.files` are not needles: they exist in the tree legitimately.
    SWE-bench record (has `test_patch`): names from FAIL_TO_PASS and from `def|fn test*` / Java
    `void test*` lines added by test_patch; shingles from `patch` and `test_patch`.
    Raises ValueError for an unknown shape, a missing patch/hidden text, or (unless
    `allow_no_shingles`) a patch with added tokens that yields no shingle at all.
    """
    needles: list = []
    shingles: list = []
    added_tokens = 0
    if "f2p" in task:
        if patch_text is None:
            raise ValueError("S5 task: patch_text (the gold patch) is required")
        needles += [_test_name(n) for n in task["f2p"].get("names", [])]
        extra = list(task.get("hidden", {}).get("extra", []))
        missing = [p for p in extra if p not in (hidden_texts or {})]
        if missing:
            raise ValueError(f"hidden.extra contents are required, missing: {missing}")
        needles += extra
        sh, total = _diff_shingles(patch_text)
        shingles += sh
        added_tokens += total
        for p in extra:
            toks = _tokens(hidden_texts[p])
            shingles += _window_shingles(toks)
            added_tokens += len(toks)
    elif "test_patch" in task:
        needles += [_test_name(n) for n in _f2p_list(task.get("FAIL_TO_PASS", []))]
        for run in _runs(task["test_patch"]):
            for line in run:
                m = re.match(r"\s*(?:async\s+)?(?:def|fn)\s+(test\w*)", line)
                if m:
                    needles.append(m.group(1))
                m = re.search(r"\b(?:void|public)\b[^(]*?\b(test\w*)\s*\(", line)
                if m:
                    needles.append(m.group(1))
        for diff in (task["test_patch"], task.get("patch", "")):
            sh, total = _diff_shingles(diff)
            shingles += sh
            added_tokens += total
    else:
        raise ValueError("unknown task shape: expected S5 (`f2p`) or SWE-bench (`test_patch`)")
    if added_tokens and not shingles and not allow_no_shingles:
        raise ValueError("the gold patch has added code but yields no shingle needle "
                         "(pass allow_no_shingles=True to accept names/paths only)")
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
                toks = needle.split()
                bad = [t for t in toks if not _TOKEN_STR.fullmatch(t)]
                if bad:
                    raise ValueError(f"shingle needle token(s) {bad[:3]} are not [A-Za-z0-9_]+: {needle!r}")
                if len(toks) < n:
                    raise ValueError(f"shingle needle has {len(toks)} tokens, fewer than n={n}: {needle!r}")
                self.shingles[" ".join(toks).encode()] = needle
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
                        needle = self.shingles.get(b" ".join(words[i:i + k]))
                        if needle is not None:
                            found.setdefault(needle, toks[i][1])
        return found

    def search_path(self, rel: str) -> list:
        if self.name_re is None:
            return []
        return _dedupe(self.name_needle[m.group()] for m in self.name_re.finditer(rel.encode("utf-8", "replace")))

    def matches(self, rel: str, data: bytes) -> list:
        """[(needle, offset)] for one file: path hits (-1), then byte hits (raw and JSON-unescaped views)."""
        out = [(nd, -1) for nd in self.search_path(rel)]
        found = self.search(data)
        if b"\\" in data:
            for nd, off in self.search(_unescape_json_view(data)).items():
                found[nd] = min(off, found.get(nd, off))
        return out + list(found.items())


def _unescape_json_view(data: bytes) -> bytes:
    """Same length as `data`: each `\\x` JSON escape becomes space + the decoded char (or space)."""
    return _ESCAPE.sub(lambda m: b" " + _ESCAPE_TO.get(m.group(1), b" "), data)


def _walk(root: Path, acc: _Acc) -> Iterator[Path]:
    def onerror(e):  # an unreadable directory is not "clean"
        acc.skipped.append((Path(e.filename or root), f"{type(e).__name__}: {e.strerror or e}"))

    seen = {os.path.realpath(root)}
    for dirpath, dirnames, filenames in os.walk(root, followlinks=True, onerror=onerror):
        keep = []
        for d in dirnames:  # a symlink loop or repeated link target is walked once
            real = os.path.realpath(os.path.join(dirpath, d))
            if real not in seen:
                seen.add(real)
                keep.append(d)
        dirnames[:] = keep
        for fn in filenames:
            yield Path(dirpath) / fn


def _read(path: Path, acc: _Acc) -> bytes | None:
    try:
        if not stat.S_ISREG(os.stat(path).st_mode):  # a FIFO would hang read_bytes
            acc.skipped.append((path, "not a regular file"))
            return None
        return path.read_bytes()
    except OSError as e:
        acc.skipped.append((path, f"{type(e).__name__}: {e}"))
        return None


def _scan_tree(root: Path, matcher: _Matcher, acc: _Acc) -> tuple:
    files = size = 0
    items = [(root, root.name)] if root.is_file() else ((p, p.relative_to(root).as_posix()) for p in _walk(root, acc))
    for path, rel in items:
        data = _read(path, acc)
        if data is None:
            continue
        acc.hits.extend(Hit(path, nd, off) for nd, off in matcher.matches(rel, data))
        files += 1
        size += len(data)
    acc.files_scanned += files
    acc.bytes_scanned += size
    return files, size


def baseline_needles(raw: list, parent: "Path | Callable[[], Iterable[tuple]]", n: int = SHINGLE_TOKENS,
                     allow_no_shingles: bool = False) -> NeedleSet:
    """Drop every needle the clean parent tree already contains; report counts per kind.

    `parent`: a directory (e.g. a `git archive <parent>` export) or a callable returning an iterable
    of `(relative_path, bytes)`. Raises EmptyScan if the parent yields no file, ScanIncomplete if
    part of a parent directory is unreadable, ValueError if a task that had shingles keeps none
    (unless `allow_no_shingles`).
    """
    matcher = _Matcher(list(raw), n)
    acc = _Acc()
    present: set = set()
    files = size = 0
    if callable(parent):
        for rel, data in parent():
            present.update(nd for nd, _ in matcher.matches(rel, data))
            files += 1
            size += len(data)
    else:
        parent = Path(parent)
        for path in _walk(parent, acc):
            data = _read(path, acc)
            if data is None:
                continue
            present.update(nd for nd, _ in matcher.matches(path.relative_to(parent).as_posix(), data))
            files += 1
            size += len(data)
    if acc.skipped:
        raise ScanIncomplete(f"baseline tree: {len(acc.skipped)} unreadable, first: {acc.skipped[0]}")
    if files == 0:
        raise EmptyScan("the baseline tree has 0 files: nothing was baselined")
    kept = [nd for nd in raw if nd not in present]
    dropped = [nd for nd in raw if nd in present]
    if (any(kind_of(nd) == "shingle" for nd in raw) and not allow_no_shingles
            and not any(kind_of(nd) == "shingle" for nd in kept)):
        raise ValueError("the baseline removed every shingle needle (pass allow_no_shingles=True to accept)")
    return NeedleSet(kept, _count_kinds(kept), _count_kinds(dropped), files, size)


def _needle_set(needles, allow_unbaselined: bool) -> NeedleSet:
    if isinstance(needles, NeedleSet):
        return needles
    if not allow_unbaselined:
        raise NoBaseline("needles were not baselined against the clean parent tree "
                         "(use baseline_needles; allow_unbaselined=True is for synthetic needles only)")
    needles = list(needles)
    return NeedleSet(needles, _count_kinds(needles), dict.fromkeys(KINDS, 0), 0, 0)


def scan(roots: list, qdrant_dump: Path | None, needles, n: int = SHINGLE_TOKENS,
         allow_unbaselined: bool = False) -> ScanReport:
    """Scan every file under `roots` (and the Qdrant dump); return the checked ScanReport.

    `needles`: a NeedleSet from `baseline_needles`. A raw list raises NoBaseline unless
    `allow_unbaselined=True`. Strict: raises EmptyScan if no file was scanned or ANY root yielded 0
    files, ScanIncomplete if any file/directory was unreadable or non-regular, FileNotFoundError
    for a missing root or dump. `report.per_root` gives files/bytes per root and for the dump.
    """
    ns = _needle_set(needles, allow_unbaselined)
    matcher = _Matcher(list(ns.needles), n)
    acc = _Acc()
    per_root = []
    for root in map(Path, roots):
        if not root.exists():
            raise FileNotFoundError(f"scan root does not exist: {root}")
        per_root.append((str(root),) + _scan_tree(root, matcher, acc))
    if qdrant_dump is not None:
        dump = Path(qdrant_dump)
        if not dump.is_file():
            raise FileNotFoundError(f"qdrant dump does not exist: {dump}")
        per_root.append((f"qdrant:{dump}",) + _scan_tree(dump, matcher, acc))
    report = ScanReport(acc.hits, acc.files_scanned, acc.bytes_scanned, acc.skipped, per_root,
                        ns.kept if isinstance(needles, NeedleSet) else _count_kinds(ns.needles), ns.dropped)
    empty = [label for label, files, _ in per_root if files == 0]
    if report.files_scanned == 0 or empty:
        raise EmptyScan(f"scanned 0 files in {empty or 'all roots'}: not a clean result")
    if report.skipped:
        raise ScanIncomplete(f"{len(report.skipped)} path(s) unreadable or non-regular, first: {report.skipped[0]}")
    return report


def positive_control(roots: list, needles, baseline, qdrant_dump: Path | None = None,
                     n: int = SHINGLE_TOKENS) -> ScanReport:
    """`scan`, plus proof the scanner can find a planted needle in the same run.

    `needles`: raw candidates (baselined here against `baseline`, a dir or callable as in
    `baseline_needles`) or an already-baselined NeedleSet (then `baseline` is not consulted). For
    each kind present in the kept set, the first needle is planted in its own file in a temp root
    that is scanned alongside the real roots (a shingle is planted split across lines). The scan
    must report exactly one hit per planted needle in that temp root, else ControlFailed.
    Returns the report for the REAL roots (control root and its hits removed).
    """
    ns = needles if isinstance(needles, NeedleSet) else baseline_needles(list(needles), baseline, n)
    with tempfile.TemporaryDirectory(prefix="leak-control-") as td:
        control = Path(td) / "control"
        control.mkdir()
        planted = {}
        for kind in KINDS:
            nd = next((x for x in ns.needles if kind_of(x) == kind), None)
            if nd is None:
                continue
            if kind == "path":
                p = control / nd
                p.parent.mkdir(parents=True, exist_ok=True)
                p.write_text("control\n")
            else:
                p = control / f"control_{kind}.txt"
                p.write_text("x " + ("\n   ".join(nd.split()) if kind == "shingle" else nd + "(y)") + " z\n")
            planted[nd] = p
        if not planted:
            raise ControlFailed("no needle to plant")
        report = scan(list(roots) + [control], qdrant_dump, ns, n)
    in_control = [h for h in report.hits if h.path.is_relative_to(control)]
    real = [h for h in report.hits if not h.path.is_relative_to(control)]
    got = sorted(h.needle for h in in_control)
    if got != sorted(planted) or any(h.path != planted[h.needle] for h in in_control):
        raise ControlFailed(f"planted {sorted(planted)} but the scanner reported {[(h.needle, str(h.path)) for h in in_control]}")
    idx = next(i for i, (label, _, _) in enumerate(report.per_root) if label == str(control))
    _, cf, cb = report.per_root[idx]
    rest = report.per_root[:idx] + report.per_root[idx + 1:]
    return report._replace(hits=real, files_scanned=report.files_scanned - cf,
                           bytes_scanned=report.bytes_scanned - cb, per_root=rest)
