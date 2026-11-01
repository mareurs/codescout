"""The Live leak sweep: what a process running as the agent user can read anywhere in the image.

    python3 live_sweep.py <input.json> [root]

Runs INSIDE the agent image, as the image user (uid 1000), with `leak_scan.py` next to it. The
input is {"needles": [...], "literals": [...], "excludes": [...]}. It walks `root` (default `/`)
without crossing filesystems (the `find -xdev` rule) and without following links, skipping only
the `excludes` (the pseudo filesystems and the toolchain dirs, each recorded in the report), and:

  * a file up to TOKEN_MAX bytes goes through `leak_scan._Matcher` (names, paths, 8-token
    shingles, as the host scans do);
  * every file of any size is searched, in windows, for each literal (a gold-only added line that
    is not in the base tree): compiled output keeps those lines verbatim, and tokenising
    gigabytes of object files in Python is not feasible, so this is the check for `target/`;
  * a file whose name looks like a patch, a diff, an eval script or a gold file is listed.

The unreadable directories and files are counted and the first few named (the agent cannot read
them either). The report is one JSON object on stdout; the exit code is 0 when the sweep finished,
whatever it found: the caller decides. Stdlib only.
"""
import json
import os
import re
import stat
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

TOKEN_MAX = 8 * 1024 * 1024
WINDOW = 64 * 1024 * 1024
MAX_HITS = 500
MAX_LISTED = 200
SUSPICIOUS = re.compile(r"(\.(patch|diff|rej|orig)$|^eval.*\.sh$|(^|[_.-])(gold|test_patch|eval_script)([_.-]|$))",
                        re.I)
# where a stray patch is not an operating-system file
SYSTEM_PREFIXES = ("/usr/", "/lib/", "/lib64/", "/etc/", "/var/lib/", "/var/cache/", "/sys/", "/proc/")


def _literal_hits(path, size, literals, window=WINDOW):
    """[(literal, offset)] for the literals found in the file, reading `window` bytes at a time
    with an overlap of the longest literal, so a literal across a window edge is still found."""
    found = {}
    overlap = max(len(x) for x in literals) - 1
    with open(path, "rb") as f:
        base = 0
        tail = b""
        while True:
            chunk = f.read(window)
            if not chunk:
                break
            data = tail + chunk
            for lit in literals:
                if lit not in found:
                    i = data.find(lit)
                    if i >= 0:
                        found[lit] = base - len(tail) + i
            base += len(chunk)
            tail = data[-overlap:] if overlap > 0 else b""
    return [(lit.decode("utf-8", "replace"), off) for lit, off in found.items()]


def sweep(root, needles, literals, excludes, *, token_max=TOKEN_MAX, window=WINDOW):
    import leak_scan
    matcher = leak_scan._Matcher(list(needles), leak_scan.SHINGLE_TOKENS) if needles else None
    lits = [x.encode() for x in literals if x]
    root = os.path.abspath(root)
    root_dev = os.lstat(root).st_dev
    excl = {os.path.normpath(os.path.join(root, e.lstrip("/"))): e for e in excludes}
    report = {"root": root, "files": 0, "bytes": 0, "token_scanned": 0, "literal_only": 0,
              "hits": [], "suspicious": [], "unreadable": [], "unreadable_count": 0,
              "pruned": [], "excluded": {e: os.path.lexists(os.path.join(root, e.lstrip("/"))) for e in excludes}}

    def unreadable(path, why):
        report["unreadable_count"] += 1
        if len(report["unreadable"]) < MAX_LISTED:
            report["unreadable"].append({"path": path, "why": why})

    started = time.monotonic()
    for dirpath, dirnames, filenames in os.walk(root, followlinks=False, onerror=lambda e: unreadable(
            str(e.filename), f"{type(e).__name__}: {e.strerror or e}")):
        keep = []
        for d in dirnames:
            p = os.path.join(dirpath, d)
            if os.path.normpath(p) in excl:
                continue
            try:
                st = os.lstat(p)
            except OSError as e:
                unreadable(p, f"{type(e).__name__}: {e.strerror or e}")
                continue
            if stat.S_ISLNK(st.st_mode):
                continue
            if st.st_dev != root_dev:
                report["pruned"].append(p)
                continue
            keep.append(d)
        dirnames[:] = keep
        for fn in filenames:
            p = os.path.join(dirpath, fn)
            rel = os.path.relpath(p, root)
            if SUSPICIOUS.search(fn) and len(report["suspicious"]) < MAX_LISTED:
                report["suspicious"].append("/" + rel)
            try:
                st = os.lstat(p)
                if not stat.S_ISREG(st.st_mode):
                    continue
                report["files"] += 1
                report["bytes"] += st.st_size
                hits = []
                if matcher is not None and st.st_size <= token_max:
                    with open(p, "rb") as f:
                        hits += matcher.matches("/" + rel, f.read())
                    report["token_scanned"] += 1
                else:
                    report["literal_only"] += 1
                    if matcher is not None:
                        hits += [(nd, -1) for nd in matcher.search_path("/" + rel)]
                if lits:
                    hits += _literal_hits(p, st.st_size, lits, window)
            except OSError as e:
                unreadable(p, f"{type(e).__name__}: {e.strerror or e}")
                continue
            for needle, off in hits:
                if len(report["hits"]) < MAX_HITS:
                    report["hits"].append({"path": "/" + rel, "needle": needle, "offset": off})
    report["seconds"] = round(time.monotonic() - started, 1)
    return report


def main(argv):
    spec = json.load(open(argv[1]))
    root = argv[2] if len(argv) > 2 else "/"
    print(json.dumps(sweep(root, spec.get("needles", []), spec.get("literals", []),
                           spec.get("excludes", []))))


if __name__ == "__main__":
    main(sys.argv)
