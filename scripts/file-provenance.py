#!/usr/bin/env python3
"""file-provenance.py — answer "is this working-tree file mine?" on a shared checkout.

WHY THIS EXISTS
---------------
Several Claude Code sessions share one checkout here. When a file appears modified —
or an un-wired function reds the shared build — git can say WHAT changed but nothing
can say WHOSE it is: git's author field is a constant across sessions, and uncommitted
state has no commit to carry a Session-Id trailer. On 2026-09-01 that gap produced
three wrong authorship answers in one evening, from three parties who were each
actively reasoning about attribution at the time
(docs/issues/2026-09-01-un-wired-function-reds-the-shared-build-with-no-author.md).

SUBSTRATE, and why it is the transcripts rather than the obvious candidate
-------------------------------------------------------------------------
The bug file first proposed `.buddy/<sid>/cs_tool_log.jsonl`. Measured 2026-09-01,
that file cannot carry this: it is a rolling 50-entry window (196 of 297 logs sit
exactly at the cap), it is DELETED on compact/resume/clear (hook_helpers.py:244,420),
and its `args` field is capped at 200 characters with unordered keys, so 11.5% of
write records carry no `path=` at all and 147 more carry a truncated one
(`src/serve`, `src/lsp/m`). One session measured here made 370 codescout calls and
its log retained 1.

Claude Code's own transcripts have none of those limits: one file per session, append
only, full tool inputs, and they capture native `Edit`/`Write` and `Bash` writes that
never reach an MCP log at all. They live per PROFILE, so all three are read
(~/.claude, ~/.claude-sdd, ~/.claude-kat) — a single profile's directory is a subset
and reports as if it were complete, which is BL-58's shape one layer down.

WHAT IT REFUSES TO DO
---------------------
Mentions are not authorship. A transcript names a path when it reads, greps, discusses
or errors on it. Ranking this project's transcripts by raw mention count puts two
non-authors above the known author of the file that motivated this tool. Only a tool
call whose INPUT names the path as a write TARGET counts.

And `UNKNOWN` is never rendered as "not mine". A Bash write this tool's heuristics miss
is indistinguishable from no write at all, so absence is a statement about coverage,
not about ownership — the precise substitution that produced the evening's three wrong
answers.

USAGE
    ./scripts/file-provenance.py <path> [<path>...]
    ./scripts/file-provenance.py $(git diff --name-only)
"""
from __future__ import annotations

import json
import os
import re
import subprocess
import sys
from pathlib import Path

# Tool calls whose input names a write TARGET. Read tools are deliberately absent:
# including one would make every reader an author.
CS_WRITE_TOOLS = {
    "mcp__codescout__edit_file",
    "mcp__codescout__edit_code",
    "mcp__codescout__edit_markdown",
    "mcp__codescout__create_file",
    "mcp__codescout__replace_symbol",   # legacy name, still in older transcripts
    "mcp__codescout__insert_code",      # legacy
    "mcp__codescout__remove_symbol",    # legacy
}
NATIVE_WRITE_TOOLS = {"Edit", "Write", "MultiEdit", "NotebookEdit"}

# artifact() writes markdown but is addressed by `id`, so its calls carry no path at all.
# In this repo trackers and bug files go through it almost exclusively -- omitting it made
# the tool report UNKNOWN for three files its own author had just edited.
ARTIFACT_WRITE_ACTIONS = {"create", "update", "move", "delete", "graft", "link",
                          "append_entry", "update_entry"}

# Python snippets run through Bash. The target lives inside the script body, either as a
# literal or as a variable assigned one line earlier.
# The quote may arrive backslash-escaped: `python3 -c "... Path(\\"p\\").write_text(x)"` is
# how a shell embeds a quoted string inside a double-quoted -c argument, and it is the
# common form in real transcripts.
_Q = r"""\\?['"]"""
PY_WRITE_LITERAL = [
    re.compile(r"""open\(\s*%s([^'"\\]+)%s\s*,\s*['"][wax]""" % (_Q, _Q)),
    re.compile(r"""Path\(\s*%s([^'"\\]+)%s\s*\)\s*\.write_""" % (_Q, _Q)),
]
PY_WRITE_VAR = [
    re.compile(r"""open\(\s*([A-Za-z_]\w*)\s*,\s*['"][wax]"""),
    re.compile(r"""Path\(\s*([A-Za-z_]\w*)\s*\)\s*\.write_"""),
]


def python_write_targets(cmd: str):
    """Paths a python snippet writes. Only the direct operand of a write construct --
    a quoted string elsewhere in the snippet is very often the file being READ."""
    for pat in PY_WRITE_LITERAL:
        for m in pat.finditer(cmd):
            yield m.group(1)
    for pat in PY_WRITE_VAR:
        for m in pat.finditer(cmd):
            var = m.group(1)
            assign = re.search(r"""\b%s\s*=\s*['"]([^'"]+)['"]""" % re.escape(var), cmd)
            if assign:
                yield assign.group(1)


_CATALOG: dict[str, str] | None = None


def catalog_paths() -> dict[str, str]:
    """artifact id -> abs_path, read once, read-only. Empty on any failure: an
    unresolvable id must degrade to UNKNOWN, never to a guess."""
    global _CATALOG
    if _CATALOG is not None:
        return _CATALOG
    _CATALOG = {}
    db = os.environ.get("FILE_PROVENANCE_CATALOG") or str(
        Path.home() / ".local/share/librarian/catalog.db")
    try:
        import sqlite3
        con = sqlite3.connect(f"file:{db}?mode=ro", uri=True)
        _CATALOG = {i: p for i, p in con.execute(
            "SELECT id, abs_path FROM artifact WHERE abs_path IS NOT NULL")}
        con.close()
    except Exception:
        pass
    return _CATALOG

# Shell constructs that WRITE their operand. Each must capture the target so a command
# that writes one file and merely reads another does not attribute both.
BASH_WRITE_PATTERNS = [
    re.compile(r">>?\s*([^\s;&|<>]+)"),                       # > f   >> f
    re.compile(r"\bsed\s+(?:-[^\s]+\s+)*-i[^\s]*\s+(?:-[^\s]+\s+)*"
               r"(?:'[^']*'|\"[^\"]*\"|[^\s]+)\s+([^\s;&|<>]+)"),
    re.compile(r"\btee\s+(?:-a\s+)?([^\s;&|<>]+)"),
    re.compile(r"\brm\s+(?:-[^\s]+\s+)*([^\s;&|<>]+)"),
    re.compile(r"\btouch\s+([^\s;&|<>]+)"),
]

# Relocating verbs need operand POSITION, not a single capture: `cp a b` writes only b,
# while `mv a b` writes b AND empties a. A pattern that yielded every operand would make
# an author of everyone who ever copied FROM a file.
RELOCATORS = re.compile(
    r"\b(git\s+mv|git\s+checkout|git\s+restore|mv|cp|install)\b([^;&|]*)")


def _operands(rest: str) -> list[str]:
    """Non-flag tokens of a command tail, with a `--` separator honoured."""
    toks = [t for t in rest.split() if t]
    if "--" in toks:
        toks = toks[toks.index("--") + 1:]
    return [t for t in toks if not t.startswith("-")]


def relocation_targets(cmd: str):
    """Paths a relocating verb writes. Source is included only when it disappears."""
    for m in RELOCATORS.finditer(cmd):
        verb = " ".join(m.group(1).split())
        ops = _operands(m.group(2))
        if not ops:
            continue
        if verb in ("git checkout", "git restore"):
            # Rewrites every named path from the index/a tree.
            yield from ops
        elif verb in ("mv", "git mv"):
            yield from ops            # destination written, source(s) removed
        else:                          # cp, install -- destination only
            yield ops[-1]


def repo_root() -> Path:
    env = os.environ.get("REPO_ROOT")
    if env:
        return Path(env).resolve()
    out = subprocess.run(["git", "rev-parse", "--show-toplevel"],
                         capture_output=True, text=True)
    return Path(out.stdout.strip() or ".").resolve()


def transcript_roots(root: Path) -> list[Path]:
    """Every profile's transcript directory for this project.

    One profile's directory is a SUBSET and looks complete; see BL-58.
    """
    env = os.environ.get("FILE_PROVENANCE_ROOTS")
    if env:
        return [Path(p) for p in re.split(r"[:,]", env) if p]
    slug = "-" + str(root).lstrip("/").replace("/", "-")
    return [Path.home() / prof / "projects" / slug
            for prof in (".claude", ".claude-sdd", ".claude-kat")]


def normalize(p: str, root: Path) -> str | None:
    """Repo-relative form, or None if the path lies outside this checkout."""
    if not p:
        return None
    q = Path(p)
    if q.is_absolute():
        try:
            return str(q.resolve().relative_to(root))
        except ValueError:
            return None          # another tree entirely — says nothing about here
    return os.path.normpath(p)


def write_targets(name: str, inp: dict, root: Path):
    """Paths this ONE tool call wrote. Empty for every read-only call."""
    if not isinstance(inp, dict):
        return
    if name in CS_WRITE_TOOLS:
        for key in ("path", "file_path"):
            if isinstance(inp.get(key), str):
                yield inp[key]
    elif name in ("mcp__codescout__artifact", "mcp__codescout__artifact_augment"):
        if inp.get("action") in ARTIFACT_WRITE_ACTIONS or name.endswith("_augment"):
            if isinstance(inp.get("rel_path"), str):
                yield inp["rel_path"]
            for key in ("id", "src_id", "dst_id", "into_id", "artifact_id"):
                got = catalog_paths().get(inp.get(key))
                if got:
                    yield got
    elif name in NATIVE_WRITE_TOOLS:
        if isinstance(inp.get("file_path"), str):
            yield inp["file_path"]
    elif name == "Bash":
        cmd = inp.get("command")
        if isinstance(cmd, str):
            for pat in BASH_WRITE_PATTERNS:
                for m in pat.finditer(cmd):
                    yield m.group(1)
            yield from relocation_targets(cmd)
            yield from python_write_targets(cmd)


def scan(root: Path) -> dict[str, list[tuple[str, str | None]]]:
    """path (repo-relative) -> [(session id, ISO timestamp or None), ...].

    Timestamps are kept rather than filtered here: a record with no timestamp
    cannot be placed in or out of any window, and dropping it would convert a
    substrate gap into a clean exoneration.
    """
    owners: dict[str, list[tuple[str, str | None]]] = {}
    for d in transcript_roots(root):
        if not d.is_dir():
            continue
        for f in sorted(d.glob("*.jsonl")):
            sid = f.stem
            try:
                fh = open(f, errors="replace")
            except OSError:
                continue
            with fh:
                for line in fh:
                    if '"tool_use"' not in line:
                        continue        # cheap prefilter; most lines are not tool calls
                    try:
                        rec = json.loads(line)
                    except Exception:
                        continue
                    msg = rec.get("message") or {}
                    content = msg.get("content")
                    if not isinstance(content, list):
                        continue
                    # The record's own sessionId beats the filename: a sidechain
                    # (subagent) record carries the PARENT's id, which is the session a
                    # human can actually be asked about.
                    who = rec.get("sessionId") or rec.get("session_id") or sid
                    when = rec.get("timestamp")
                    for b in content:
                        if not isinstance(b, dict) or b.get("type") != "tool_use":
                            continue
                        for raw in write_targets(b.get("name"), b.get("input"), root):
                            rel = normalize(raw, root)
                            if rel:
                                owners.setdefault(rel, []).append((who, when))
    return owners


def last_commit_time(path: str, root: Path) -> str | None:
    """When this path was last committed. Writes older than that are baked into HEAD."""
    out = subprocess.run(
        ["git", "-C", str(root), "log", "-1", "--format=%cI", "--", path],
        capture_output=True, text=True)
    return out.stdout.strip() or None


def _key(iso: str) -> str:
    """Sortable UTC key. Tolerates 'Z', '+03:00' and naive forms alike."""
    from datetime import datetime, timezone
    t = iso.strip().replace("Z", "+00:00")
    try:
        d = datetime.fromisoformat(t)
    except ValueError:
        return iso
    if d.tzinfo is None:
        d = d.replace(tzinfo=timezone.utc)
    return d.astimezone(timezone.utc).isoformat()


def main(argv: list[str]) -> int:
    since = None
    unbounded = False
    paths = []
    i = 0
    while i < len(argv):
        a = argv[i]
        if a == "--since" and i + 1 < len(argv):
            since = argv[i + 1]; i += 2; continue
        if a == "--all":
            unbounded = True; i += 1; continue
        if a == "--":
            paths.extend(argv[i + 1:]); break
        paths.append(a); i += 1

    if not paths:
        print(__doc__.strip().split("USAGE")[-1].strip(), file=sys.stderr)
        return 2

    root = repo_root()
    me = os.environ.get("CLAUDE_CODE_SESSION_ID", "")
    owners = scan(root)

    unknown = 0
    for arg in paths:
        rel = normalize(arg, root) or arg
        records = owners.get(rel, [])

        # The window. Default is "since this path was last committed", because the
        # question is about the UNCOMMITTED delta -- a months-old file legitimately has
        # many lifetime authors and naming all of them answers a question nobody asked.
        if unbounded:
            floor = None
        elif since:
            floor = _key(since)
        else:
            lc = last_commit_time(rel, root)
            floor = _key(lc) if lc else None

        in_window, undated = [], []
        for who, when in records:
            if when is None:
                undated.append(who)
            elif floor is None or _key(when) >= floor:
                in_window.append(who)
        who_set = set(in_window) | set(undated)

        if not who_set:
            unknown += 1
            print(f"UNKNOWN   {rel}")
            print("          no record of any session writing this path in the window. "
                  "That is a statement about coverage, NOT about ownership — Bash writes "
                  "this tool's heuristics miss look identical. Do not read it as 'not mine'.")
            if records:
                print(f"          ({len(records)} write(s) exist but predate the window; "
                      f"re-run with --all to see them)")
            continue

        mine = bool(me) and me in who_set
        peers = sorted(w for w in who_set if w != me)
        verdict = "MINE" if mine and not peers else ("SHARED" if mine else "PEER")
        print(f"{verdict:9} {rel}")
        if floor:
            print(f"          window: writes at or after {floor}")
        if mine:
            print(f"          written by THIS session ({me[:8]})")
        for w in peers:
            mark = "  [undated — could not be placed in the window]" if (
                w in undated and w not in in_window) else ""
            print(f"          written by {w}{mark}")
    return 1 if unknown == len(paths) else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
