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
from datetime import datetime
from pathlib import Path

# Tool calls whose input names a write TARGET. Read tools are deliberately absent:
# including one would make every reader an author.
CS_WRITE_TOOLS = {
    "mcp__codescout__edit_file",
    "mcp__codescout__edit_code",
    "mcp__codescout__create_file",
    "mcp__codescout__edit_markdown",    # legacy -> edit_file: folded into edit_file
    "mcp__codescout__replace_symbol",   # legacy -> edit_code: name, still in older transcripts
    "mcp__codescout__insert_code",      # legacy -> edit_code:
    "mcp__codescout__remove_symbol",    # legacy -> edit_code:
}
NATIVE_WRITE_TOOLS = {"Edit", "Write", "MultiEdit", "NotebookEdit"}

# doc() writes markdown but is addressed by `id`, so its calls carry no path at all. In this
# repo trackers and bug files go through it almost exclusively -- omitting it made the tool
# report UNKNOWN for three files its own author had just edited.
#
# LOAD-BEARING: `augment` is in this set because it was once a SEPARATE TOOL
# (artifact_augment), caught in write_targets by a `name.endswith("_augment")` test. As a
# doc() ACTION that test never fires, so removing it here silently un-covers every
# augmentation write and no other assertion notices.
# `event_create` is deliberately absent: it writes a catalog row, not the file.
ARTIFACT_WRITE_ACTIONS = {"create", "update", "move", "delete", "graft", "link",
                          "append_entry", "update_entry", "augment"}

# Python snippets run through Bash or run_command. The target lives inside the script
# body -- as a literal, or as a variable bound ANYWHERE in the snippet. The binder is a
# whole-snippet `re.search` and has never had a line bound.
#
# This comment used to say "assigned one line earlier", which was never true of the code
# beside it. It cost a real diagnosis on 2026-09-08: a session reading it reported that
# distance was the limitation, and the fix it implied ("look back further") would have
# changed nothing while testing green against the cases that already passed. The two real
# narrowings were the CALL shape and the quote form. Doc-vs-code drift is the defect class
# where reading carefully is the failure mode -- a precise wrong comment closes the
# question that a vague one would have left open.
#
# The quote may arrive backslash-escaped: `python3 -c "... Path(\\"p\\").write_text(x)"` is
# how a shell embeds a quoted string inside a double-quoted -c argument, and it is the
# common form in real transcripts. `_Q` therefore belongs in every quote position here --
# including the binder, which went without it until 2026-09-08.
_Q = r"""\\?['"]"""
PY_WRITE_LITERAL = [
    re.compile(r"""open\(\s*%s([^'"\\]+)%s\s*,\s*['"][wax]""" % (_Q, _Q)),
    re.compile(r"""Path\(\s*%s([^'"\\]+)%s\s*\)\s*\.write_""" % (_Q, _Q)),
]
PY_WRITE_VAR = [
    re.compile(r"""open\(\s*([A-Za-z_]\w*)\s*,\s*%s[wax]""" % _Q),
    re.compile(r"""Path\(\s*([A-Za-z_]\w*)\s*\)\s*\.write_"""),
    # A variable ALREADY holding a Path, written directly: `F.write_text(x)`. The two
    # patterns above both require the name to be wrapped at the call site, which is not
    # how anyone writes a multi-line snippet -- so this is the common form, not the
    # exotic one. `\.write_` and not `\.write` deliberately: a bare `fh.write(x)` is a
    # file OBJECT, and the `open(...,"w")` that produced it is already matched above,
    # so accepting it would only add ways to be wrong. `Path(p).write_text` cannot
    # double-match here -- the character before `.write_` is `)`, not a word char.
    re.compile(r"""\b([A-Za-z_]\w*)\s*\.write_"""),
]

# What binds a variable to a path. The search is over the WHOLE snippet, and always
# was -- distance has never been the limitation, despite the residual twice being
# described as "a variable assigned one line earlier". `Path("lit")` and
# `pathlib.Path("lit")` count alongside a bare `"lit"`, because a snippet that writes
# through a variable has almost always built it with Path().
#
# DELIBERATELY NOT WIDENED to "any assignment": binding is not writing. A path the
# snippet only ever read_text()s must stay out, or this collapses into the
# mention-as-authorship failure the whole tool exists to refuse -- which returns a
# confident WRONG name rather than a vague one. The discriminator is the CALL; this
# resolver only says where a name points once a write call has already selected it.
#
# `_Q`, not a bare quote: a transcript records a shell-quoted snippet, so the quotes
# around the path routinely arrive BACKSLASH-ESCAPED (`Path(\"x\")`). PY_WRITE_LITERAL
# has always known that; this resolver did not, so `p=\"lit\"` bound nothing -- a
# narrowing that predates the Path-valued widening and is fixed with it.
PY_ASSIGN = (r"""\b%s\s*=\s*(?:(?:\w+\.)?Path\(\s*)?"""
             + _Q + r"""([^'"\\]+)""" + _Q)


def python_write_targets(cmd: str):
    """Paths a python snippet writes. Only the direct operand of a write construct --
    a quoted string elsewhere in the snippet is very often the file being READ."""
    for pat in PY_WRITE_LITERAL:
        for m in pat.finditer(cmd):
            yield m.group(1)
    for pat in PY_WRITE_VAR:
        for m in pat.finditer(cmd):
            var = m.group(1)
            assign = re.search(PY_ASSIGN % re.escape(var), cmd)
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
#
# Two bounds carry the weight, and both are about MENTION rather than position.
# `(?=\s)`, not `\b`: a word boundary holds between `v` and `.`, so `\bmv\b` matches
# inside the filename `mv.rs` -- and this repo HAS src/librarian/tools/mv.rs, so every
# read-only command naming it was recorded as a write of something. A real invocation
# always has whitespace before its operands, so nothing legitimate is lost.
# `\n` in the negated class: the tail must stop at the end of ITS OWN command. Unbounded,
# it runs into the next line and _operands() honours a `--` belonging to a different
# command, lifting that command's path. That is how one read yields the harmless fragment
# `.rs` while two reads yield a real file -- the second `--` is what completes the defect.
#
# A third bound, and it is about POSITION rather than shape, which is why neither of the
# other two can stand in for it: a verb surrounded by real whitespace still need not be a
# COMMAND. `(?:^|[;&|(\n])\s*` requires it to open one. Without that, `cargo install X`
# attributes a write of `X` -- the verb is a SUBCOMMAND of another program -- and a HEREDOC
# BODY is scanned as command text, which yielded a path with the surrounding Python syntax
# still attached (`src/beta.rs")]`). A heredoc exists precisely to mean "this is data, not
# syntax", and it is the fifth construct in this process to be misread that way.
#
# KNOWN LOSS, deliberate: a wrapper word hides the verb behind it exactly as `cargo` does --
# `sudo mv a b` no longer resolves -- because nothing separates a wrapper from a program
# with subcommands without a list of one or the other, and a list is the same no-escape
# problem one level in. The miss degrades to UNKNOWN, which is this file's safe direction; a
# confident wrong name is what it exists to avoid. Asserted in the suite so that widening it
# is a deliberate edit.
#
# RESIDUE this does not close: a relocation at the START of a line inside a heredoc body is,
# to a line-oriented scanner, indistinguishable from a command. Closing that needs heredoc
# extent tracking -- a parser, not a tighter pattern.
#
# Measured 2026-09-15; every clause above is guarded in tests/file-provenance.sh, one case
# per bound, because each bound alone prevents the others' symptom and they would otherwise
# guard the conjunction and no single site.
RELOCATORS = re.compile(
    r"(?:^|[;&|(\n])\s*"
    r"(git\s+mv|git\s+checkout|git\s+restore|mv|cp|install)(?=\s)([^;&|\n]*)")


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


def transcript_roots(root: Path) -> list[Path]:
    """Every profile's transcript directory for this project.

    One profile's directory is a SUBSET and looks complete; see BL-58.
    """
    env = os.environ.get("FILE_PROVENANCE_ROOTS")
    if env:
        return [Path(p) for p in re.split(r"[:,]", env) if p]
    slug = "-" + str(root).lstrip("/").replace("/", "-")
    return profile_dirs(f"projects/{slug}")


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


def _pid_alive(pid: object) -> bool:
    """True when a process with this pid exists.

    PermissionError means it exists and is owned by someone else — alive, not absent.
    Collapsing that to False would report a live foreign-profile session as exited.
    """
    try:
        os.kill(int(pid), 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        return True
    except (OSError, TypeError, ValueError):
        return False
    return True


def live_sessions() -> dict[str, list[dict]]:
    """sessionId -> live registry rows, across every profile.

    A registry row carries sessionId, pid, name, cwd and messagingSocketPath in ONE
    record, so a sid resolves to a reachable address offline, with nothing to ask.
    On disk the rows are keyed by PID, so this is the reverse index.

    LIVENESS IS CHECKED, and that is the load-bearing part. A registry file outlives
    the session that wrote it, so the file's existence is not liveness; rendering a
    dead session as reachable sends the reader to a socket that ENOENTs, and the
    error it returns is byte-identical to a cross-profile name refusal
    (skills/reaching-peer-sessions § "Two readings to get right"), so the reader
    mis-diagnoses it as an addressing mistake and retries.

    NEVER CACHE THIS. Measured 2026-09-07: a peer's pid AND registry name both moved
    in a single hop while a message was in flight. The sessionId is the only durable
    component; every other field here is valid at its instant and no longer.
    """
    out: dict[str, list[dict]] = {}
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
                continue
            sid, sock = rec.get("sessionId"), rec.get("messagingSocketPath") or ""
            if not sid or not sock:
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
    return out


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
    elif name in ("mcp__codescout__doc",
                  "mcp__codescout__artifact",          # legacy -> doc: pre-ceb5b57a name
                  "mcp__codescout__artifact_augment"):  # legacy -> doc: now doc(action="augment")
        if inp.get("action") in ARTIFACT_WRITE_ACTIONS or name.endswith("_augment"):
            # `new_rel_path` is move's destination, and it is load-bearing rather than
            # thorough: a move re-keys the artifact (id = sha256(abs_path)), so the `id`
            # recorded in the transcript names a row the move itself deleted and the catalog
            # lookup below can never resolve it. Without this key an archive move — a bug
            # file's normal end state — attributes to nobody.
            for key in ("rel_path", "new_rel_path"):
                if isinstance(inp.get(key), str):
                    yield inp[key]
            for key in ("id", "src_id", "dst_id", "into_id", "artifact_id"):
                got = catalog_paths().get(inp.get(key))
                if got:
                    yield got
    elif name in NATIVE_WRITE_TOOLS:
        if isinstance(inp.get("file_path"), str):
            yield inp["file_path"]
    # run_command is codescout's own shell and carries the command under the same key, so a
    # write issued through it is the same event as a Bash write. Keying on "Bash" alone made
    # every redirect / sed -i / rm through codescout invisible to this tool
    # (docs/issues/archive/2026-09-07-file-provenance-reads-bash-but-not-codescouts-own-shell.md).
    elif name in ("Bash", "mcp__codescout__run_command"):
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
        for f in sorted(
            list(d.glob("*.jsonl")) + list(d.glob("*/subagents/*.jsonl"))
        ):
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
            #
            # For a subagent file the stem is `agent-<id>`, which addresses no session and
            # no human. The directory two levels up IS the parent session id — a party who
            # can actually be asked — so the fallback comes from the path, not the
            # filename. `who` below still prefers the record's own `sessionId`, which real
            # subagent records carry and which holds that same parent id.
            sid = f.parent.parent.name if f.parent.name == "subagents" else f.stem
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
                    # `sessionId` over the filename, and for a SUBAGENT record that field
                    # holds the PARENT's id — the session a human can actually be asked
                    # about. The glob above now reaches those records; see its comment for
                    # where they live and how three readings missed them.
                    #
                    # This comment has been wrong twice, in the same direction both times,
                    # and the shape is worth more than either claim was. It said the
                    # substrate carried no subagent records; corrected, it said Claude Code
                    # 2.1.x emitted none. Each correction narrowed the blame — substrate,
                    # then version — and neither questioned the instrument, because every
                    # re-derivation ran through the same non-recursive glob and returned
                    # the same zero. Two agreeing counts over one blind spot is one blind
                    # spot counted twice, which at the point of use is indistinguishable
                    # from corroboration. The records were one directory down the whole
                    # time.
                    # docs/issues/archive/2026-09-10-subagent-writes-leave-no-transcript-record-so-provenance-and-fmt-mine-refuse-them.md
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
    # Resolved ONCE per invocation, deliberately: a snapshot the whole run shares is
    # honest about being an instant, where a per-path re-read would silently mix two.
    live = live_sessions()
    named_sids: set[str] = set()

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
        # Records the window excluded -- present regardless of verdict, because a hidden
        # write is exactly as real on a MINE path as on an UNKNOWN one. Equals len(records)
        # whenever who_set is empty, which is what makes this a drop-in for the count the
        # UNKNOWN branch used to compute only for itself.
        hidden = len(records) - len(in_window) - len(undated)

        if not who_set:
            unknown += 1
            print(f"UNKNOWN   {rel}")
            # The window, printed here for the same reason `hidden` was lifted above the
            # branch: this is the one verdict whose entire meaning IS the window, and the
            # only one that was not naming it. `if floor` is the correct guard unchanged --
            # an untracked path has no floor, and its silence is already right.
            if floor:
                print(f"          window: writes at or after {floor}")
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
        verdict = "MINE" if mine and not peers else ("SHARED" if mine else "PEER")
        print(f"{verdict:9} {rel}")
        if floor:
            print(f"          window: writes at or after {floor}")
        if hidden:
            # MINE is the verdict a reader acts on to license a commit, so this caveat
            # matters most exactly here -- a peer write the window hid is still a peer
            # write. Previously only the UNKNOWN branch printed it.
            print(f"          ({hidden} write(s) also exist but predate the window; "
                  f"re-run with --all to see them)")
        if mine:
            print(f"          written by THIS session ({me[:8]})")
        for w in peers:
            mark = "  [undated — could not be placed in the window]" if (
                w in undated and w not in in_window) else ""
            named_sids.add(w)
            suffix, extra = address_lines(w, live, " " * 21)
            print(f"          written by {w}{mark}{suffix}")
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
    return 1 if unknown == len(paths) else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
