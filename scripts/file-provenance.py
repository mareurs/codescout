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
(docs/issues/archive/2026-09-01-un-wired-function-reds-the-shared-build-with-no-author.md).

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
answers. **A SUBAGENT write is NOT a second such blind spot, though this file said it
was until 2026-09-12.** Claude Code 2.1.x does record subagent activity: it writes it to
`<project-dir>/<parent-session-id>/subagents/agent-<id>.jsonl`, one directory below the
parent's own transcript, and each record carries `isSidechain: true`, a timestamp, and
the PARENT's `sessionId`. `scan()` now globs that location, so an `Agent`-written file
attributes to the session that dispatched it.

The superseded claim — *"zero sidechain records across 41 versions and 1,928
dispatches"* — was a true count of a window, not of the disk: every reading of it was
taken through a non-recursive `*.jsonl` glob that could not reach those files, so the
zero reproduced on demand and read as corroboration. It was corrected twice before being
found (substrate → version → neither). Re-derived 2026-09-12 outside the window, one
checkout, three profiles: 756 subagent transcript files, 755 carrying the flag, 192,797
records, newest same-day under 2.1.267 — a version the superseded claim named as
emitting none. **Bash remains the one real blind spot.**

THE OTHER DIRECTION: what SHARED / PEER may claim
-------------------------------------------------
Both assert that another party's UNCOMMITTED bytes are in the path right now, so three things
that look like writes in a transcript but put nothing at risk are NOT counted as that:
a call whose own tool_result says it was REFUSED (a request and a landed edit are the same
token in a tool_use block; a codescout refusal arrives as `{"ok": false}` with `is_error`
absent, a harness refusal sets `is_error`), and a write to a path git reports CLEAN, which is
history already in HEAD and is reported as the verdict `CLEAN` rather than SHARED or PEER.
A third case needs the text, because per-path git state cannot see it: on a DIRTY path, a write
that the path's last commit already took still falls in the window when it was made in that
commit's own second (git dates a commit to the second). Such a peer is dropped, with a note,
only when its text is known, long enough, present in HEAD and not copied again in the
worktree (`absorbed_writes`); a shell write, a short string, a write outside that second and
the asker's own writes always stay counted.
All three narrow the false-POSITIVE side only. A call with no result on record, and every shell
call whatever its exit status, still counts; UNKNOWN is still never "not mine"; and a path git
cannot vouch for (untracked) keeps its transcript verdict. Verdict tokens, first column:
MINE SHARED PEER UNKNOWN CLEAN. scripts/fmt-mine.sh partitions on that column.

USAGE
    ./scripts/file-provenance.py <path> [<path>...]
    ./scripts/file-provenance.py $(git diff --name-only)
"""
from __future__ import annotations

import functools
import json
import os
import re
import subprocess
import sys
from datetime import datetime, timedelta
from pathlib import Path

# Standalone script also supports the established import-by-filename scan API.
sys.path.insert(0, str(Path(__file__).resolve().parent))
from claude_history import discover_claude_histories, read_claude_history
from provenance_records import CoverageGap, HistoryBatch, Operation, ProvenanceScan
from codex_history import discover_codex_histories, read_codex_history
from native_provenance_history import apply_checkpoint
from typing import Iterable
from session_context import principal_key, resolve_context
from history_cache import cache_dir as history_cache_dir, caching as caching_reader
from write_targets import (
    ARTIFACT_WRITE_ACTIONS, BASH_WRITE_PATTERNS, CS_WRITE_TOOLS, NATIVE_WRITE_TOOLS, PY_ASSIGN,
    PY_WRITE_LITERAL, PY_WRITE_VAR, RELOCATORS, _Q, _TARGET_NAMES, _operands,
    patch_write_targets, python_write_targets, relocation_targets, static_targets, with_targets,
)

# Calls whose result says nothing about whether the write landed. A shell command that exits
# non-zero (or that codescout's own shell blocks) may still have written before it failed:
# `sed -i ... && make` writes the file whatever `make` does, and `cat > f <<EOF; false` leaves
# `f` behind. Every OTHER write-capable call is all-or-nothing -- an edit is applied or it is
# refused -- so a refusal on record means nothing was written (see `_result_refused`). Dropping a
# failed shell call as well would convert "the command errored" into "the command wrote
# nothing", which is a false NEGATIVE, the direction this tool refuses to err in. Deliberately
# not named `*_TOOLS`: the registry drift gate reads every such constant as a tool-name list.
_NON_ATOMIC_NAMES = ("Bash", "mcp__codescout__run_command")

# Prefilter for `refused_call_ids`: a tool_result line that CAN be a refusal. `is_error` for the
# harness's own refusals; the escaped `\"ok\": false` for a codescout refusal, which is JSON text
# inside a JSON string; `pending_ack` for the unacknowledged out-of-project write.
_REFUSAL_HINT = re.compile(r'"is_error"\s*:\s*true|\\"ok\\"\s*:\s*false|pending_ack')

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

def repo_root() -> Path:
    env = os.environ.get("REPO_ROOT")
    if env:
        return Path(env).resolve()
    out = subprocess.run(["git", "rev-parse", "--show-toplevel"],
                         capture_output=True, text=True)
    return Path(out.stdout.strip() or ".").resolve()


def profile_dirs(leaf: str) -> list[Path]:
    """Every `<home>/.claude*/<leaf>` directory that EXISTS.

    Discovered, never hardcoded — and the distinction is not theoretical. Measured
    2026-09-08 on this machine: 7 `.claude*` directories, 5 of them carrying a
    `sessions/`, against the 3 a fixed list named. A hardcoded set is the
    per-profile-subset hazard this whole file exists to defeat, reappearing one
    level in, inside the instrument — and it reports as complete.

    Mirrors default_profile_dirs() in src/librarian/session_registry.rs, which is
    the same decision taken in Rust for the same reason: the set is per-machine.
    """
    try:
        entries = sorted(Path.home().iterdir())
    except OSError:
        return []
    return [p / leaf for p in entries
            if (p.name == ".claude" or p.name.startswith(".claude-"))
            and (p / leaf).is_dir()]


def claude_slug(path: Path) -> str:
    """The directory name Claude Code gives a cwd under `<profile>/projects/`.

    Every non-alphanumeric byte becomes `-`, not only `/`: `.worktrees/x` becomes
    `-worktrees-x`. Measured 2026-09-24: 11 project dirs across every profile on this
    machine, none holding a character outside [A-Za-z0-9-], and every live session's
    registry `cwd` slugged this way named an existing dir. Replacing `/` alone missed
    any cwd containing a dot -- a worktree under `.worktrees/`, or a `mktemp` dir.
    """
    return re.sub(r"[^A-Za-z0-9]", "-", str(path))


def main_checkout(root: Path) -> Path | None:
    """The main checkout's root when `root` is a linked worktree, else None."""
    out = subprocess.run(
        ["git", "-C", str(root), "rev-parse", "--path-format=absolute", "--git-common-dir"],
        capture_output=True, text=True)
    common = Path(out.stdout.strip()) if out.returncode == 0 and out.stdout.strip() else None
    if common is None or common.name != ".git":
        return None
    main = common.parent.resolve()
    return main if main != root.resolve() else None


def transcript_roots(root: Path) -> list[Path]:
    """Every profile's transcript directory that can hold writes to this checkout.

    One profile's directory is a SUBSET and looks complete; see BL-58.

    Transcripts are filed under a session's CWD, not under the tree it edits. In a linked
    worktree the sessions doing the work usually sit in the MAIN checkout and reach the
    worktree by activating it, so its transcript dir is read too; `scan()` then decides,
    write by write, which tree a relative path named.
    docs/issues/archive/2026-09-24-file-provenance-reads-no-transcripts-inside-a-worktree.md
    """
    env = os.environ.get("FILE_PROVENANCE_ROOTS")
    if env:
        return [Path(p) for p in re.split(r"[:,]", env) if p]
    checkouts = [root]
    main = main_checkout(root)
    if main is not None:
        checkouts.append(main)
    dirs: list[Path] = []
    for checkout in checkouts:
        for d in profile_dirs(f"projects/{claude_slug(checkout)}"):
            if d not in dirs:
                dirs.append(d)
    return dirs


def registry_roots() -> list[Path]:
    """Every profile's session-registry directory.

    Same per-profile subset hazard as transcript_roots(), one layer over: a single
    profile's registry is exactly what ListAgents reads, and it presents that subset
    as the whole population with nothing marking it a subset
    (docs/issues/2026-08-30-listagents-omits-cross-profile-sessions-in-the-same-checkout.md).
    """
    env = os.environ.get("FILE_PROVENANCE_REGISTRY_ROOTS")
    if env:
        return [Path(p) for p in re.split(r"[:,]", env) if p]
    return profile_dirs("sessions")


def _pid_alive_windows(pid: object) -> bool:
    """The Windows half of `_pid_alive`: ask for a handle, never send a signal.

    `os.kill(pid, 0)` is the POSIX existence probe and is NOT one here. `signal.CTRL_C_EVENT`
    is 0, so the call asks the console to deliver Ctrl+C: every process attached to the
    caller's console receives it (measured 2026-11-07: the probed process AND the caller), and
    it does not raise for a pid that is gone, so a dead pid read as alive
    (docs/issues/2026-11-07-pid-alive-probe-os-kill-0-sends-ctrl-c-to-the-whole-console-on-windows.md).

    `OpenProcess` with the narrowest right that still allows `GetExitCodeProcess` answers
    without touching the process. Access denied means it exists and is not ours: alive, the
    same third value the POSIX branch gets from `PermissionError`. One residual: a process that
    exited with code 259 (`STILL_ACTIVE`) reads as alive, which Windows documents and no
    pid-only probe can tell apart.
    """
    import ctypes
    from ctypes import wintypes

    try:
        number = int(pid)
    except (TypeError, ValueError):
        return False
    # ctypes truncates an out-of-range integer silently; a pid is a positive DWORD.
    if not 0 < number <= 0xFFFFFFFF:
        return False
    process_query_limited_information = 0x1000
    still_active = 259
    error_access_denied = 5
    kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel32.OpenProcess.restype = wintypes.HANDLE
    kernel32.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
    kernel32.GetExitCodeProcess.restype = wintypes.BOOL
    kernel32.GetExitCodeProcess.argtypes = [wintypes.HANDLE, ctypes.POINTER(wintypes.DWORD)]
    kernel32.CloseHandle.argtypes = [wintypes.HANDLE]
    handle = kernel32.OpenProcess(process_query_limited_information, False, number)
    if not handle:
        return ctypes.get_last_error() == error_access_denied
    try:
        code = wintypes.DWORD()
        if not kernel32.GetExitCodeProcess(handle, ctypes.byref(code)):
            return True
        return code.value == still_active
    finally:
        kernel32.CloseHandle(handle)

def _pid_alive(pid: object) -> bool:
    """True when a process with this pid exists.

    PermissionError means it exists and is owned by someone else — alive, not absent.
    Collapsing that to False would report a live foreign-profile session as exited.

    Windows takes its own branch: `os.kill(pid, 0)` there sends Ctrl+C to the whole console
    instead of probing (see `_pid_alive_windows`).
    """
    if sys.platform == "win32":
        return _pid_alive_windows(pid)
    try:
        os.kill(int(pid), 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        return True
    except (OSError, TypeError, ValueError):
        return False
    return True


def live_sessions() -> tuple[dict[str, list[dict]], list[dict]]:
    """`(sessionId -> live registry rows, rows that could not be READ)`, every profile.

    A registry row carries sessionId, pid, name, cwd and messagingSocketPath in ONE
    record, so a sid resolves to a reachable address offline, with nothing to ask.
    On disk the rows are keyed by PID, so this is the reverse index.

    LIVENESS IS CHECKED, and that is the load-bearing part. A registry file outlives
    the session that wrote it, so the file's existence is not liveness; rendering a
    dead session as reachable sends the reader to a socket that ENOENTs, and the
    error it returns is byte-identical to a cross-profile name refusal
    (skills/reaching-peer-sessions § "Two readings to get right"), so the reader
    mis-diagnoses it as an addressing mistake and retries.

    AN UNREADABLE ROW IS NOT AN ABSENT SESSION, which is why there is a second return
    value rather than a `continue`. The row is a derived cache of state the process
    already holds, rewritten in place, so a write interrupted by a full disk leaves it
    ZERO BYTES with the session still running -- pid alive, socket open, mid-tool-call.
    `json.loads("")` raises, and folding that into "not live" is wrong in the DANGEROUS
    direction: the reader concludes the session exited and its resources are
    reclaimable, and disk exhaustion is exactly when someone goes looking for
    reclaimable per-session state. Measured 2026-09-17, on the session that was reading.
    docs/issues/archive/2026-09-17-a-full-disk-truncates-a-live-sessions-registry-row-so-provenance-reports-it-dead.md

    The FILENAME is what rescues it. Rows are keyed `<pid>.json`, so the pid survives
    the bytes being destroyed -- enough to say *someone is running here and I cannot say
    who*, which is the honest third value `_pid_alive` above already models for its own
    case (`PermissionError` means alive, not absent).

    NEVER CACHE THIS. Measured 2026-09-07: a peer's pid AND registry name both moved
    in a single hop while a message was in flight. The sessionId is the only durable
    component; every other field here is valid at its instant and no longer.
    """
    out: dict[str, list[dict]] = {}
    unreadable: list[dict] = []
    for d in registry_roots():
        # ~/.claude/sessions -> ".claude". A fixture root that is not named
        # "sessions" labels itself, so tests read as their own directory names.
        prof = d.parent.name if d.name == "sessions" else d.name
        try:
            files = sorted(d.glob("*.json"))
        except OSError:
            continue
        for f in files:
            try:
                rec = json.loads(f.read_text(encoding="utf-8", errors="replace"))
            except (OSError, ValueError):
                rec = None
            sid = rec.get("sessionId") if isinstance(rec, dict) else None
            if not sid:
                # The row cannot name its session. Whether that MATTERS is decided by
                # the pid, read from the filename rather than from the bytes: a dead
                # pid's corrupt row is ordinary garbage and must stay silent, or every
                # stale file on disk shouts and the one row that means something is
                # buried in the noise.
                if _pid_alive(f.stem):
                    unreadable.append({"pid": f.stem, "profile": prof or str(d)})
                continue
            sock = rec.get("messagingSocketPath") or ""
            # A KNOWN sid whose socket is gone is a session we can still NAME -- it is
            # simply unreachable, which the dead-session path below already reports.
            # Only a row that cannot say who it is belongs in `unreadable`.
            if not sock:
                continue
            if not Path(sock).exists() or not _pid_alive(rec.get("pid")):
                continue
            out.setdefault(sid, []).append({
                "name": rec.get("name") or "?",
                "pid": rec.get("pid"),
                "profile": prof or str(d),
                "socket": sock,
                "cwd": rec.get("cwd") or "?",
                "status": rec.get("status") or "?",
            })
    return out, unreadable


def address_lines(sid: str, live: dict[str, list[dict]], indent: str) -> tuple[str, list[str]]:
    """Render a sessionId as something the reader can ACT on, or say they cannot.

    Returns (suffix, extra_lines) — the suffix marks the `written by` line itself and
    the extra lines expand a reachable address beneath it.

    THE ASYMMETRY IS DELIBERATE. A dead session collapses to one inline marker; only
    a live one expands. Measured 2026-09-08 on this repo: `--all` over
    docs/trackers/issue-clusters.md names 35 lifetime authors of which 1 is
    reachable, so giving all 35 equal vertical weight buries the single row the
    reader came for.

    The `uds:` form is given rather than the name because it is the one address that
    works from every profile; a bare name resolves only within the sender's own, and
    a name is re-minted by compaction, resume or a restart under another profile
    while the sessionId is not. The name here is a label, never the address.
    """
    rows = live.get(sid) or []
    if not rows:
        return ("  [not live — cannot be asked]", [])
    out = []
    if len(rows) > 1:
        # A sid in two live registry rows is the no-disambiguator half of IC-6:
        # picking either silently addresses a coin flip. Name them all instead.
        out.append(f"{indent}{len(rows)} live rows carry this sessionId — they are "
                   f"not interchangeable; address one explicitly")
    for r in rows:
        out.append(f"{indent}{r['name']} [{r['status']}] — pid {r['pid']}, "
                   f"profile {r['profile']}, cwd {r['cwd']}")
        out.append(f'{indent}  ask it: SendMessage to="uds:{r["socket"]}"')
    return ("  [LIVE]", out)
def owner_contact(sid: str, live: dict[str, list[dict]], indent: str) -> tuple[str, list[str]]:
    if not sid.startswith("claude-code:"):
        return ("  [contact unknown; liveness unknown]", [])
    return address_lines(sid, live, indent)



def normalize(p: str, root: Path, base: Path | None = None) -> str | None:
    """Repo-relative form, or None if the path lies outside this checkout.

    `base` is the directory a RELATIVE `p` was written against, when the transcript says
    (see `scan()`). Without it a relative path is taken to name this checkout, which is
    only right while every transcript read belongs to it -- false once a linked worktree
    also reads the main checkout's transcripts.
    """
    if not p:
        return None
    q = Path(p)
    if not q.is_absolute() and base is not None:
        q = base / q
    if q.is_absolute():
        # A string test on the resolved path, not `Path.relative_to`: that call was 5.8 of this
        # function's 10.8 profiled seconds over the real corpus (bug 20927ccfe293c34e). The
        # separator is part of the prefix, so a sibling `<root>-other` is outside, not a remainder.
        resolved, top = str(q.resolve()), str(root)
        if resolved == top:
            return "."
        prefix = top if top.endswith(os.sep) else top + os.sep
        # Another tree entirely says nothing about here. The remainder is the git form, which
        # is `/` on every platform: `git status` and `git log` print `src/a.rs`, and a caller
        # compares this result with those, so `src\a.rs` would never match on Windows.
        return resolved[len(prefix):].replace(os.sep, "/") if resolved.startswith(prefix) else None
    return os.path.normpath(p).replace(os.sep, "/")


def catalog_targets(name: str, inp: object):
    """The doc tool's artifact-id lookups: ids in `inp` resolved to paths through the live
    catalog. Never cached: a catalog row can move between two scans, and `static_targets`
    (write_targets.py) holds everything that is a pure function of the call."""
    if not isinstance(inp, dict):
        return
    if name not in _TARGET_NAMES:
        return
    if name in ("mcp__codescout__doc",
                "mcp__codescout__artifact",          # legacy -> doc: pre-ceb5b57a name
                "mcp__codescout__artifact_augment"):  # legacy -> doc: now doc(action="augment")
        if inp.get("action") in ARTIFACT_WRITE_ACTIONS or name.endswith("_augment"):
            for key in ("id", "src_id", "dst_id", "into_id", "artifact_id"):
                got = catalog_paths().get(inp.get(key))
                if got:
                    yield got


def write_targets(name: str, inp: dict, root: Path):
    """Paths this ONE tool call wrote. Empty for every read-only call.

    The static part (pure function of the call, `static_targets`) first, then the live
    catalog part (`catalog_targets`). `root` is unused, as it always was.
    """
    yield from static_targets(name, inp)
    yield from catalog_targets(name, inp)


# The active tree after a `workspace(action="activate")` whose path this script cannot resolve.
# A bare token with no separator is a workspace PROJECT ID, the server's own rule for telling an id
# from a path, and only the workspace manifest can map an id to a root. That manifest is
# gitignored, per machine, and not exhaustive (auto-discovered projects are absent from it), so
# the script does not guess: a relative codescout write made while the tree is this value is
# credited to nobody, which is the answer `fmt-mine` refuses on rather than acts on. Compared by
# IDENTITY and never resolved, joined or normalised: it is not a path.
UNKNOWN_TREE = Path("<unresolvable workspace id>")


def is_workspace_id(value: str) -> bool:
    """True when `value` is a bare token the server reads as a project id, not a path.

    The server's rule (get_guide `workspace-state`): no `/` means a project id, anything else a
    path. `.` and `..` are spelled without a separator and are unmistakably paths, so they are
    carved out; an empty string is not a token at all.
    """
    return bool(value) and value not in (".", "..") and "/" not in value and "\\" not in value



def write_base(name, inp, active: Path | None, cwd: Path | None) -> Path | None:
    """The directory a relative path in this tool call was written against.

    codescout tools resolve against the ACTIVE project -- `run_command` included, since it
    runs in the project root, or in its `cwd=` subdirectory when the call names one -- unless
    the call pins a project with `workspace=`. Native `Bash` runs in the session's cwd. Native
    Write/Edit take absolute paths, so the base is moot.
    """
    if isinstance(name, str) and name.startswith("mcp__codescout__"):
        base = active
        if isinstance(inp, dict) and isinstance(inp.get("workspace"), str):
            base = Path(inp["workspace"])
        if (name == "mcp__codescout__run_command" and base is not None
                and base is not UNKNOWN_TREE
                and isinstance(inp, dict) and isinstance(inp.get("cwd"), str)):
            base = base / inp["cwd"]
        return base
    return cwd


@functools.lru_cache(maxsize=None)
def checkout_root(cwd: str) -> Path:
    """The git checkout containing `cwd`: where codescout resolves a relative path.

    A session's cwd can be a subdirectory, but the active project is the checkout root, so
    starting the active tree at the raw cwd lost every relative codescout write from such a
    session. Falls back to `cwd` itself when git cannot answer (a deleted dir, not a repo).
    """
    out = subprocess.run(["git", "-C", cwd, "rev-parse", "--show-toplevel"],
                         capture_output=True, text=True)
    top = out.stdout.strip() if out.returncode == 0 else ""
    return Path(top) if top else Path(cwd)


def activated_tree(name, inp, active: Path | None) -> Path | None:
    """`active` after this call: a `workspace(action="activate", path=...)` moves it.

    Read from the REQUEST; a refused activation would be misread as a move. Accepted:
    activation is rarely refused.

    The tool takes a project path OR a workspace project id, and an id is not a directory, so
    reading every value as a path credited a later relative write to `<active>/<id>`: a path
    that exists in no tree. An id yields `UNKNOWN_TREE` instead, and so does a relative path
    taken from a tree that is already unknown. Anything else moves the tree as it always did,
    so a wrong base here can still mis-file a relative path between two trees, but it no longer
    names a tree that is not there.
    """
    if name != "mcp__codescout__workspace" or not isinstance(inp, dict):
        return active
    if inp.get("action") != "activate" or not isinstance(inp.get("path"), str):
        return active
    target = Path(inp["path"])
    if target.is_absolute():
        return target
    if is_workspace_id(inp["path"]) or active is UNKNOWN_TREE:
        return UNKNOWN_TREE
    return active / target if active is not None else target


def transcript_files(root: Path) -> list[Path]:
    """Every transcript file that can hold writes to this checkout, own and subagent."""
    files: list[Path] = []
    for d in transcript_roots(root):
        if not d.is_dir():
            continue
        # `<project-dir>/*.jsonl` is a session's OWN transcript. Claude Code 2.1.x
        # puts a SUBAGENT's records in `<project-dir>/<parent-session-id>/subagents/
        # agent-<id>.jsonl` — one directory down, which the non-recursive glob this
        # replaces never reached.
        #
        # That omission is why three successive readings of "are there subagent
        # records?" all returned zero: every one was taken through this same glob, so
        # each described the WINDOW rather than the disk, and each agreed with the
        # last. Re-derived 2026-09-12 outside it, for one checkout across 3 profiles:
        # 756 subagent transcript files, 755 carrying `isSidechain: true`, 192,797
        # such records — the newest written that day by 2.1.267, a version the
        # superseded comment named as emitting none.
        files.extend(sorted(list(d.glob("*.jsonl")) + list(d.glob("*/subagents/*.jsonl"))))
    return files


def file_session_id(f: Path) -> str:
    """The session a transcript file belongs to, when its records do not say.

    For a subagent file the stem is `agent-<id>`, which addresses no session and no human. The
    directory two levels up IS the parent session id — a party who can actually be asked — so
    the fallback comes from the path, not the filename.
    """
    return f.parent.parent.name if f.parent.name == "subagents" else f.stem


def tool_records(f: Path):
    """Yield (record, tool_use blocks) for each record of `f` that carries a tool call."""
    try:
        fh = open(f, errors="replace")
    except OSError:
        return
    with fh:
        for line in fh:
            if '"tool_use"' not in line:
                continue        # cheap prefilter; most lines are not tool calls
            try:
                rec = json.loads(line)
            except Exception:
                continue
            content = (rec.get("message") or {}).get("content")
            if not isinstance(content, list):
                continue
            blocks = [b for b in content if isinstance(b, dict) and b.get("type") == "tool_use"]
            if blocks:
                yield rec, blocks
def _result_refused(block: dict) -> bool:
    """Did this tool_result say the call did NOT take effect?

    Two shapes, and the second is the one that matters here because it is the one a reader
    would not guess. A harness refusal (a hook denial, a read-only project, a native
    `Edit` whose `old_string` is absent) sets `is_error: true`. A codescout REFUSAL does
    NOT: `RecoverableError` maps to `isError: false` on purpose, so that a sibling parallel
    call is not aborted, and the refusal arrives as ordinary text,
    `{"ok": false, "error": "old_string not found ..."}`. Measured 2026-10-05 over this
    project's 80 newest transcripts: 262 `edit_file` and 45 `edit_code` results of that shape, every
    one with `is_error` absent. Reading `is_error` alone would have fixed the native tools
    and left the codescout case this bug was filed on untouched.

    `pending_ack` is the out-of-project-write envelope: nothing was written, the caller must
    re-invoke with the `@ack_*` handle, and THAT re-invocation is its own call with its own
    result.
    """
    if block.get("is_error") is True:
        return True
    content = block.get("content")
    if isinstance(content, list):
        text = " ".join(p.get("text", "") for p in content
                        if isinstance(p, dict) and isinstance(p.get("text"), str))
    elif isinstance(content, str):
        text = content
    else:
        return False
    text = text.strip()
    if not text.startswith("{"):
        return False
    try:
        body = json.loads(text)
    except Exception:
        return False
    return isinstance(body, dict) and (body.get("ok") is False or "pending_ack" in body)


def refused_call_ids(f: Path) -> set[str]:
    """tool_use ids in transcript `f` whose own result says the call did not take effect.

    Paired by `tool_use_id`, never by position: parallel calls return their results in
    whatever order they finish, in later records than the calls.

    A call with NO result on record is NOT in this set. A missing result is a gap in the
    transcript (an interrupted session, a truncated file, every synthetic fixture), and
    reading it as a refusal would convert a substrate gap into an exoneration, which is the
    direction this tool refuses to err in.
    """
    ids: set[str] = set()
    try:
        fh = open(f, errors="replace")
    except OSError:
        return ids
    with fh:
        for line in fh:
            # Cheap prefilter. Only a line that CAN carry a refusal is parsed; a miss here
            # leaves a refused call counted as a write, which is the pre-existing behaviour
            # and the safe direction. The escaped quotes match a refusal that is JSON text
            # inside a JSON string, which is how a codescout result is serialised.
            if '"tool_result"' not in line or not _REFUSAL_HINT.search(line):
                continue
            try:
                rec = json.loads(line)
            except Exception:
                continue
            content = (rec.get("message") or {}).get("content")
            if not isinstance(content, list):
                continue
            for b in content:
                if (isinstance(b, dict) and b.get("type") == "tool_result"
                        and isinstance(b.get("tool_use_id"), str) and _result_refused(b)):
                    ids.add(b["tool_use_id"])
    return ids


def record_session(rec: dict, fallback: str) -> str:
    # `sessionId` over the filename, and for a SUBAGENT record that field holds the PARENT's
    # id — the session a human can actually be asked about.
    #
    # This comment has been wrong twice, in the same direction both times, and the shape is
    # worth more than either claim was. It said the substrate carried no subagent records;
    # corrected, it said Claude Code 2.1.x emitted none. Each correction narrowed the blame —
    # substrate, then version — and neither questioned the instrument, because every
    # re-derivation ran through the same non-recursive glob and returned the same zero. Two
    # agreeing counts over one blind spot is one blind spot counted twice, which at the point
    # of use is indistinguishable from corroboration. The records were one directory down the
    # whole time.
    # docs/issues/archive/2026-09-10-subagent-writes-leave-no-transcript-record-so-provenance-and-fmt-mine-refuse-them.md
    return rec.get("sessionId") or rec.get("session_id") or fallback


def operation_activations(operations: list[Operation]) -> dict[str, dict]:
    """Track shared-server activation uncertainty using normalized operations."""
    facts: dict[str, dict] = {}
    roots: dict[tuple[str | None, str | None], Path | None] = {}
    for op in operations:
        stream = (op.principal, op.agent_id)
        if roots.get(stream) is None and op.cwd is not None:
            roots[stream] = checkout_root(str(op.cwd))
        root_of = roots.get(stream)
        if activated_tree(op.name, op.input, None) is None:
            continue
        target = activated_tree(op.name, op.input, root_of)
        fs = facts.setdefault(op.principal,
                              {"parent_activated": False, "sub_activated_at": None})
        if op.is_child:
            ts = op.timestamp or ""
            if fs["sub_activated_at"] is None or ts < fs["sub_activated_at"]:
                fs["sub_activated_at"] = ts
        elif root_of is None or target.resolve() != root_of.resolve():
            fs["parent_activated"] = True
    return facts


def session_activations(files: list[Path]) -> dict[str, dict]:
    """Compatibility entry point for activation facts from Claude histories."""
    return operation_activations([op for path in files
                                  for op in read_claude_history(path).operations])



# The shortest text a write may carry and still be tested for "already committed" (see
# `absorbed_writes`). Below this a string is as likely to occur in a file by coincidence as by
# the write, and "it occurs in HEAD" would then prove nothing about THIS write.
MIN_ABSORB_CHARS = 24


def write_texts(name: str, inp: dict) -> list[str] | None:
    """The text this ONE all-or-nothing write put on disk, or None when that is not knowable.

    None is the safe answer and the common one: a shell command, a `doc()` call (addressed by
    id, patching a rendered body), a deletion (empty replacement) and a rename carry no text
    that can be looked up in a file. A write whose text is unknown is never treated as already
    committed (see `absorbed_writes`), so every None keeps the write counted.

    Also None when ANY piece is shorter than MIN_ABSORB_CHARS: one piece that proves nothing
    must not be outvoted by the others.
    """
    if not isinstance(inp, dict):
        return None
    if name in CS_WRITE_TOOLS:
        keys = ("new_string", "content", "body", "new_body")
    elif name in NATIVE_WRITE_TOOLS:
        keys = ("new_string", "content", "new_source")
    else:
        return None
    found: list[str] = []
    pieces = [inp]
    edits = inp.get("edits")
    if isinstance(edits, list):
        pieces.extend(e for e in edits if isinstance(e, dict))
    for piece in pieces:
        for key in keys:
            if key in piece:
                if not isinstance(piece[key], str):
                    return None
                found.append(piece[key])
    if not found or any(len(t.strip()) < MIN_ABSORB_CHARS for t in found):
        return None
    return found

def merge_history_batches(batches: Iterable[HistoryBatch]) -> HistoryBatch:
    """Deduplicate original native events, retaining every incompatible replay as a gap."""
    operations, gaps, seen = [], [], {}
    for batch in batches:
        gaps.extend(batch.gaps)
        for op in batch.operations:
            previous = seen.get(op.source_id)
            if previous is None:
                seen[op.source_id] = op
                operations.append(op)
            elif previous != op:
                gaps.append(CoverageGap(op.source_id, "conflicting original-event replay", op.cwd))
    return HistoryBatch(operations, gaps)


def _modified_since(paths: list[Path], floor: float | None) -> list[Path]:
    """Keep the history files that may hold a record at or after `floor` (epoch seconds).

    A history file is append-only, so no record in it is newer than its mtime. A file last
    written before `floor` therefore cannot add a record inside a window that starts at
    `floor`, and reading it is wasted work. `floor=None` keeps everything. A file whose mtime
    cannot be read is KEPT: the cut may only ever drop a file it has measured. Measured
    2026-11-01 over 7656 Claude and Codex history files on this machine: none had a record
    in the last 200 lines newer than its mtime plus 2 s (596 had no timestamp there and were
    not judged).
    """
    if floor is None:
        return paths
    kept = []
    for path in paths:
        try:
            if os.path.getmtime(path) < floor:
                continue
        except OSError:
            pass
        kept.append(path)
    return kept


def scan_provenance(root: Path, environ: dict[str, str], modified_since: float | None = None) -> ProvenanceScan:
    """One policy consumes both hosts' normalized history and explicit coverage gaps.

    `modified_since` (epoch seconds) skips history files last written before it: see
    `_modified_since`. Leave it unset for a lifetime answer.
    """
    claude_files, claude_gaps = discover_claude_histories(root, environ)
    codex_files, codex_gaps = discover_codex_histories(root, environ)
    claude_files = _modified_since(claude_files, modified_since)
    codex_files = _modified_since(codex_files, modified_since)
    # Each file's decoded history is stored once and reused while the file is unchanged: see
    # `history_cache`. The cache is keyed on this call's `environ`, so a test that hands in its
    # own HOME gets its own cache. The reader stores each operation's `static_targets` with it
    # (`with_targets`), so a warm scan does not re-derive them; the doc tool's catalog-id
    # targets are not stored, and `catalog_targets` reads them live below.
    cache = history_cache_dir(environ)
    read_claude = caching_reader(lambda p: with_targets(read_claude_history(p)), cache)
    read_codex = caching_reader(lambda p: with_targets(read_codex_history(p)), cache)
    batch = merge_history_batches([
        HistoryBatch([], claude_gaps + codex_gaps),
        *(read_claude(path) for path in claude_files),
        *(apply_checkpoint(path, read_codex(path), environ, root) for path in codex_files),
    ])
    owners, evidence, gaps = {}, {}, list(batch.gaps)
    # `normalize` answers for THIS call only. It resolves symlinks, which is the filesystem as it
    # is now, so the memo is a local and never a module global: a second scan must see a retargeted
    # link. 27,707 of 76,207 calls over the real corpus had distinct arguments (bug 20927ccfe293c34e).
    memo: dict = {}
    streams = {}
    operations = sorted(batch.operations, key=lambda op: op.timestamp or "")
    facts = operation_activations(operations)
    no_facts = {"parent_activated": False, "sub_activated_at": None}
    active_by_stream = {}
    for op in operations:
        who, when = op.principal, op.timestamp
        stream = (who, op.agent_id)
        active = active_by_stream.get(stream)
        if active is None and op.cwd is not None:
            active = checkout_root(str(op.cwd))
        fs = facts.get(who, no_facts)
        sub_at = fs["sub_activated_at"]
        unknowable = (fs["parent_activated"] or sub_at is not None) if op.is_child else (
            sub_at is not None and (when is None or when >= sub_at))
        base = write_base(op.name, op.input, active, op.cwd)
        relative_cs = op.name.startswith("mcp__codescout__")
        pin = op.input.get("workspace")
        pinned = isinstance(pin, str) and Path(pin).is_absolute()
        targets = [] if op.atomic and op.outcome == "no-write-refused" else (
            list(op.targets if op.targets is not None else static_targets(op.name, op.input))
            + list(catalog_targets(op.name, op.input)))
        capable = (op.name in CS_WRITE_TOOLS or op.name in NATIVE_WRITE_TOOLS
                   or op.name == "apply_patch"
                   or op.name in ("mcp__codescout__doc",
                               "mcp__codescout__artifact",          # legacy -> doc
                               "mcp__codescout__artifact_augment")  # legacy -> doc
                   and (op.input.get("action") in ARTIFACT_WRITE_ACTIONS or op.name.endswith("_augment")))
        if capable and not targets and op.outcome != "no-write-refused":
            gaps.append(CoverageGap(op.source_id, "write target is not recoverable", base if base is not UNKNOWN_TREE else None))
        texts = write_texts(op.name, op.input) if len(targets) == 1 else None
        for raw in targets:
            if ((unknowable and not pinned) or base is UNKNOWN_TREE) and relative_cs and not Path(raw).is_absolute():
                gaps.append(CoverageGap(op.source_id, "relative write has ambiguous shared MCP workspace", None))
                continue
            if (raw, base) not in memo:
                memo[(raw, base)] = normalize(raw, root, base)
            rel = memo[(raw, base)]
            if rel:
                if who is None:
                    gaps.append(CoverageGap(op.source_id, "native writer identity is unresolved", root, (rel,)))
                    continue
                owners.setdefault(rel, []).append((who, when))
                streams.setdefault(rel, {}).setdefault(who, set()).add(op.agent_id)
                key = (rel, who, when)
                if texts is None or evidence.get(key, []) is None:
                    evidence[key] = None
                else:
                    evidence.setdefault(key, []).extend(texts)
        active_by_stream[stream] = activated_tree(op.name, op.input, active)
    return ProvenanceScan(owners, evidence, gaps, streams)


def relevant_gaps(scan: ProvenanceScan, rel: str, root: Path) -> list[CoverageGap]:
    """Unknown workspace can affect any path; known unrelated workspaces cannot."""
    result = []
    for gap in scan.gaps:
        if gap.paths is not None:
            if not any(normalize(path, root, gap.workspace) == rel for path in gap.paths):
                continue
        elif gap.workspace is not None and checkout_root(str(gap.workspace)).resolve() != root.resolve():
            continue
        result.append(gap)
    return result


def scan_with_evidence(root: Path, modified_since: float | None = None) -> tuple[
        dict[str, list[tuple[str, str | None]]],
        dict[tuple[str, str, str | None], list[str] | None]]:
    """Compatibility two-value API; use scan_provenance for coverage information."""
    result = scan_provenance(root, os.environ, modified_since)
    return result.owners, result.evidence


def scan(root: Path, modified_since: float | None = None) -> dict[str, list[tuple[str, str | None]]]:
    """path (repo-relative) -> [(session id, ISO timestamp or None), ...]. See `scan_with_evidence`."""
    return scan_with_evidence(root, modified_since)[0]


def last_commit_time(path: str, root: Path) -> str | None:
    """When this path was last committed. Writes older than that are baked into HEAD."""
    out = subprocess.run(
        ["git", "-C", str(root), "log", "-1", "--format=%cI", "--", path],
        capture_output=True, text=True)
    return out.stdout.strip() or None


def worktree_is_dirty(path: str, root: Path) -> bool | None:
    """Does the worktree hold uncommitted bytes for this path? None when git cannot say.

    Deliberately THREE-valued, and the None is load-bearing: the UNKNOWN prose keys a
    DISPOSITIVE clearance off False, so a wrong False replaces the omission this answers
    with a louder false claim.

    TWO distinct ways git cannot say, and the second was found by a mutation-driven case
    rather than by reading. Outside a repo `status` fails plainly. But for a path the repo
    does not TRACK it SUCCEEDS and prints nothing — byte-identical to a clean tracked
    file — so a bare `status` reading licenses a clearance about a path git holds no
    baseline for. `ls-files --error-unmatch` is the discriminator, and it answers both
    cases, which is why it is first.

    What makes the reading worth stating at all is INDEPENDENCE: it shares no blind spot
    with the transcript heuristics, so a Bash write they miss still dirties the tree, and
    `clean` therefore settles what `no record` cannot.
    """
    tracked = subprocess.run(
        ["git", "-C", str(root), "ls-files", "--error-unmatch", "--", path],
        capture_output=True, text=True)
    if tracked.returncode != 0:
        return None
    out = subprocess.run(
        ["git", "-C", str(root), "status", "--porcelain", "--", path],
        capture_output=True, text=True)
    # Defensive, and annotated as such: no input is known to reach a tracked path whose
    # `status` then fails, so this None is not credited with coverage. It is here because
    # the alternative on an unexpected failure is a fabricated clearance.
    if out.returncode != 0:
        return None
    return bool(out.stdout.strip())


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


def _when_dt(iso: str | None) -> datetime | None:
    """A transcript or git timestamp as an aware UTC datetime; None when it will not parse."""
    if not iso:
        return None
    try:
        return datetime.fromisoformat(_key(iso))
    except ValueError:
        return None


def absorbed_writes(rel: str, root: Path, floor: str | None,
                    records: list[tuple[str, str | None]],
                    evidence: dict[tuple[str, str, str | None], list[str] | None],
                    me: str) -> set[tuple[str, str | None]]:
    """Writes to a DIRTY path that the path's last commit already took, so nothing is at risk.

    The window's floor is the path's last commit time, and git gives that to the SECOND. A write
    committed by that very commit can therefore only fall inside the window when it was made in
    the commit's own second: an edit and the commit that took it, one second apart or less.
    That write is history, not co-ownership, but the path is dirty (someone else's bytes are
    in it), so the CLEAN verdict cannot discount it and per-path git state cannot tell the two
    apart (docs/issues/archive/2026-09-13-file-provenance-conflates-touched-once-with-bytes-at-risk.md,
    instance 2: a session whose create was committed at bc933c01 was named as a live co-writer
    of a file the asker had since added 99 lines to).

    What can tell them apart is the text. A write is ABSORBED only when ALL of these hold:
      - it is dated, and falls in [floor, floor + 1 s), the one second the floor cannot resolve;
      - its text is known and long enough to mean something (`write_texts`);
      - every piece of that text occurs in HEAD's copy of the path;
      - the worktree holds no MORE copies of it than HEAD does, so no uncommitted copy of the
        text exists for it to be the author of.
    Anything else stays counted. That includes a shell write, a `doc()` write, a short string, a
    write outside the boundary second, and the asker's own writes (`me`): this narrows the
    FALSE-POSITIVE direction only, so the cost of being wrong is a name that should have been
    dropped and was not, never a live co-writer that was.

    Known residue, accepted: an UNCOMMITTED edit that merely MOVES a block already in HEAD
    leaves the copy count unchanged, so its author reads as absorbed. It needs the move to
    land in the commit's own second as well.
    """
    floor_dt = _when_dt(floor)
    if floor_dt is None:
        return set()
    end = floor_dt + timedelta(seconds=1)
    candidates = []
    for who, when in set(records):
        dt = _when_dt(when)
        if who != me and dt is not None and floor_dt <= dt < end:
            candidates.append((who, when))
    if not candidates:
        return set()
    shown = subprocess.run(["git", "-C", str(root), "show", f"HEAD:{rel}"],
                           capture_output=True, text=True, errors="replace")
    if shown.returncode != 0:
        return set()
    head = shown.stdout
    try:
        work = (root / rel).read_text(errors="replace")
    except OSError:
        return set()
    out: set[tuple[str, str | None]] = set()
    for who, when in candidates:
        texts = evidence.get((rel, who, when))
        if not texts:
            continue
        if all(head.count(t) > 0 and work.count(t) <= head.count(t) for t in texts):
            out.add((who, when))
    return out



def session_grain_note(scan: ProvenanceScan, rel: str, principal: str, indent: str) -> list[str]:
    """The caveat a path needs when a SUBAGENT lies behind the session that "wrote" it, else nothing.

    Every subagent a session spawns runs under its parent's session id, so `owners` can only say
    "this session wrote it". That sentence is true of a path the root wrote, and equally true of one
    a sibling subagent wrote, though a reader who is a different subagent of the same session is
    being told something quite different. The verdict does not change (`fmt-mine.sh` partitions on
    its token, and a parent rightly owns what it dispatched): what changes is that the answer stops
    reading as precise. A path only the root wrote gets no note.
    The asker's own agent id is not available here for Claude Code, whose native environment carries
    none (only Codex's `CODEX_THREAD_ID` and an explicit context file do), so the note names the
    writers and leaves "was that me?" to the reader.
    docs/issues/2026-09-19-file-provenance-answers-at-session-grain-so-sibling-subagents-are-one-writer.md
    """
    agents = scan.streams.get(rel, {}).get(principal, set())
    if not any(agent is not None for agent in agents):
        return []
    names = ["root" if agent is None else f"agent {agent}"
            for agent in sorted(agents, key=lambda a: (a is not None, a or ""))]
    return [f"{indent}session-grain: written under this one session id by {', '.join(names)}. "
            f"Attribution names the SESSION, not the agent, so a sibling subagent's write reads as "
            f"this session's; which agent wrote which bytes is not recorded."]



def short_principal(principal: str) -> str:
    """Qualify the native id before abbreviating it; never abbreviate the host name."""
    harness, _, native = principal.partition(":")
    return f"{harness}:{native[:8]}"


def main(argv: list[str]) -> int:
    since = None
    unbounded = False
    paths = []
    context_path = None
    i = 0
    while i < len(argv):
        a = argv[i]
        if a == "--context":
            if context_path is not None or i + 1 >= len(argv):
                print("context usage: --context <absolute-json-path>", file=sys.stderr)
                return 2
            context_path = Path(argv[i + 1]); i += 2; continue
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
    resolution = resolve_context(root, os.environ, context_path)
    if resolution.state != "available":
        print(f"context {resolution.state}: {json.dumps(resolution.reason)}", file=sys.stderr)
        return 2
    me = principal_key(resolution.context.harness, resolution.context.session_id)
    scan_result = scan_provenance(root, os.environ)
    owners, evidence = scan_result.owners, scan_result.evidence
    # Resolved ONCE per invocation, deliberately: a snapshot the whole run shares is
    # honest about being an instant, where a per-path re-read would silently mix two.
    native_live, unreadable = live_sessions()
    live = {principal_key("claude-code", sid): rows for sid, rows in native_live.items()}
    named_sids: set[str] = set()

    unknown = 0
    for arg in paths:
        rel = normalize(arg, root) or arg
        records = owners.get(rel, [])
        gaps = relevant_gaps(scan_result, rel, root)

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
        # Records the window excluded -- present regardless of verdict, because a hidden
        # write is exactly as real on a MINE path as on an UNKNOWN one. Equals len(records)
        # whenever who_set is empty, which is what makes this a drop-in for the count the
        # UNKNOWN branch used to compute only for itself. Counted BEFORE the absorbed writes
        # below are discounted: those are in the window, just already in HEAD.
        hidden = len(records) - len(in_window) - len(undated)
        # Instance 2 of docs/issues/archive/2026-09-13-file-provenance-conflates-touched-once-with-bytes-
        # at-risk.md: on a DIRTY path, a write committed by the path's last commit still lands
        # in the window when it was made in that commit's own second (git's resolution). Only the
        # DEFAULT window has such a floor; --since and --all name their own.
        absorbed_sids: set[str] = set()
        if in_window and not unbounded and not since and floor is not None \
                and worktree_is_dirty(rel, root):
            absorbed = absorbed_writes(rel, root, floor, records, evidence, me)
            for who in set(in_window):
                its = [(w, t) for w, t in records
                       if w == who and t is not None and _key(t) >= floor]
                if who not in undated and its and all(r in absorbed for r in its):
                    absorbed_sids.add(who)
            in_window = [w for w in in_window if w not in absorbed_sids]
        who_set = set(in_window) | set(undated)
        absorbed_note = (
            f"          ({len(absorbed_sids)} session(s) "
            f"[{', '.join(sorted(short_principal(s) for s in absorbed_sids))}] wrote this path in the last "
            f"commit's own second, and the text they wrote is already in HEAD with no further "
            f"copy in the worktree: they hold no uncommitted bytes here, so they are not named)"
        ) if absorbed_sids else None

        if not who_set:
            unknown += 1
            print(f"UNKNOWN   {rel}")
            for gap in gaps:
                print(f"          coverage gap: {json.dumps(gap.reason)}")
            # The window, printed here for the same reason `hidden` was lifted above the
            # branch: this is the one verdict whose entire meaning IS the window, and the
            # only one that was not naming it. `if floor` is the correct guard unchanged --
            # an untracked path has no floor, and its silence is already right.
            if floor:
                print(f"          window: writes at or after {floor}")
            if absorbed_note:
                print(absorbed_note)
            # Name the cause the reader is most likely looking at BEFORE the caveats.
            # Inside this branch `hidden == len(records)` is a TAUTOLOGY -- an empty
            # who_set means in_window and undated are both empty -- so `records` is the
            # whole condition. Do not "restore" the count as a guard here; it cannot
            # discriminate. With no records at all we know nothing, and say nothing.
            if floor and records:
                dirty = worktree_is_dirty(rel, root)
                if dirty is False:
                    print("          LIKELY CAUSE: the worktree is CLEAN for this path, "
                          "so no session holds uncommitted bytes in it. That is a "
                          "dispositive clearance rather than a coverage gap — git "
                          "cleanliness shares no blind spot with the heuristics below.")
                elif dirty is True and absorbed_sids:
                    print("          LIKELY CAUSE: the worktree is DIRTY for this path and the "
                          "only writes on record in the window are already in the last "
                          "commit, so WHO holds the uncommitted bytes is not recorded — NOT "
                          "evidence that nobody does. A write this tool cannot read made "
                          "them.")
                elif dirty is True:
                    print("          LIKELY CAUSE: the worktree is DIRTY for this path "
                          "and every write on record predates the window, so the window "
                          "is too narrow for the question you asked — NOT evidence that "
                          "nobody owns it. Re-run with --all before concluding anything.")
            print("          no record of any session writing this path in the window. "
                  "That absence is a statement about coverage, NOT about ownership — Bash "
                  "writes this tool's heuristics miss look identical. Do not read it as "
                  "'not mine'.")
            print("          One blind spot is a Bash write: this tool's heuristics "
                  "can miss one, so an owner may exist and not be recorded. A SUBAGENT "
                  "write is NOT one — since 2026-09-12 this tool reads "
                  "<project-dir>/<session-id>/subagents/*.jsonl and attributes an "
                  "Agent's writes to the session that dispatched it, which is a party "
                  "you can reach. An older copy of this message said to stop looking "
                  "for a subagent's owner; that rested on a count taken through a glob "
                  "which could not reach those files.")
            if hidden:
                print(f"          ({hidden} write(s) exist but predate the window; "
                      f"re-run with --all to see them)")
            continue

        mine = bool(me) and me in who_set
        peers = sorted(w for w in who_set if w != me)
        # Item 3 of d567a429109f6fd8: a window-excluded write is invisible to the
        # verdict a reader ACTS on. A hunk-split commit of a shared file can move the
        # floor past a still-uncommitted peer write, so a dated peer write outside the
        # window downgrades MINE to SHARED -- but only when the path is actually dirty
        # right now, so a synthetic/never-touched fixture (no real bytes on disk) is
        # unaffected. Gated on worktree_is_dirty() rather than unconditionally, because
        # a hidden write against a CLEAN path is already baked into HEAD and answers
        # nothing about live uncommitted bytes.
        hidden_peer = mine and not peers and any(
            w != me for w, when in records
            if when is not None and floor is not None and _key(when) < floor)
        # Git cleanliness, read once and only when a verdict could change on it. Three-valued
        # (see worktree_is_dirty): None -- untracked, or git cannot say -- never licenses
        # anything, so it falls through to the transcript verdict unchanged.
        dirty = (worktree_is_dirty(rel, root)
                 if gaps or hidden_peer or not (mine and not peers) else None)
        if hidden_peer and dirty:
            verdict = "SHARED"
        else:
            verdict = "MINE" if mine and not peers else ("SHARED" if mine else "PEER")
        # SHARED and PEER both assert that ANOTHER party's UNCOMMITTED bytes are in this path
        # right now -- that is the claim a reader acts on (fmt-mine refuses, a commit waits for
        # an answer). A path git reports CLEAN holds no uncommitted bytes from anyone, so a
        # session's write to it is history already absorbed into HEAD and the claim is false
        # (docs/issues/archive/2026-09-13-file-provenance-conflates-touched-once-with-bytes-at-risk.md).
        # This is the FALSE-POSITIVE direction only: UNKNOWN is still never rendered as "not
        # mine", MINE is untouched, and a dirty or untracked path keeps its transcript verdict.
        # `CLEAN` is a NEW first-column token; scripts/fmt-mine.sh partitions on that column.
        if verdict in ("SHARED", "PEER") and dirty is False:
            verdict = "CLEAN"
        if gaps and dirty is not False and verdict != "SHARED":
            verdict = "UNKNOWN"
            unknown += 1
        print(f"{verdict:9} {rel}")
        for gap in gaps:
            print(f"          coverage gap: {json.dumps(gap.reason)}")
        if floor:
            print(f"          window: writes at or after {floor}")
        if absorbed_note:
            print(absorbed_note)
        if hidden:
            # MINE is the verdict a reader acts on to license a commit, so this caveat
            # matters most exactly here -- a peer write the window hid is still a peer
            # write. Previously only the UNKNOWN branch printed it.
            print(f"          ({hidden} write(s) also exist but predate the window; "
                  f"re-run with --all to see them)")
        if verdict == "CLEAN":
            print("          the worktree is CLEAN for this path: git holds no uncommitted "
                  "bytes in it at this instant, so nothing is at risk and there is no one's "
                  "work to overwrite. The writes on record are history already in git, not "
                  "co-ownership of the working tree — there is nobody to ask about it.")
            writers = sorted(who_set)
            print("          recorded writer(s), committed history: "
                  + ", ".join(short_principal(w) + (" (THIS session)" if w == me else "") for w in writers))
            continue
        if mine:
            print(f"          written by THIS session ({short_principal(me)})")
            for note in session_grain_note(scan_result, rel, me, " " * 10):
                print(note)
        for w in peers:
            mark = "  [undated — could not be placed in the window]" if (
                w in undated and w not in in_window) else ""
            if w.startswith("claude-code:"):
                named_sids.add(w)
            suffix, extra = owner_contact(w, live, " " * 21)
            print(f"          written by {w}{mark}{suffix}")
            for note in session_grain_note(scan_result, rel, w, " " * 10):
                print(note)
            for line in extra:
                print(line)

    if named_sids:
        # Unit, scope and INSTANT, all three. The instant reads as decoration and is
        # the one that gets dropped: two honest enumerations hours apart share almost
        # no pids, so an unstamped count makes ordinary churn present as a tooling
        # defect — sending the reader to go debug a working instrument.
        n_live = sum(1 for s in named_sids if live.get(s))
        print(f"-- registry: {n_live} of {len(named_sids)} named session(s) live at "
              f"{datetime.now().astimezone().isoformat(timespec='seconds')}, across "
              f"{len(registry_roots())} profile(s). pid, name and socket decay "
              f"continuously; the sessionId does not — re-derive at use.")

    # Printed OUTSIDE the block above, and that placement is the point: the verdict
    # with no session line is UNKNOWN, and UNKNOWN plus a live-but-unnameable session
    # is exactly when "nobody owns this file" is the wrong conclusion to draw.
    if unreadable:
        where = ", ".join(f"pid {u['pid']} ({u['profile']})" for u in unreadable)
        print(f"-- {len(unreadable)} unreadable registry row(s) for LIVE pid(s): "
              f"{where}. A session is running there whose identity could not be read "
              f"— an empty or malformed row, which is what a rewrite interrupted by a "
              f"full disk leaves behind. Every count and every absence above is "
              f"therefore a LOWER BOUND; resolve these by hand before concluding a "
              f"file is unowned or a session has exited.")
    return 1 if unknown == len(paths) else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
