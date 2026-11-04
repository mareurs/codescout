"""The pure part of write-target extraction: which paths one tool call wrote, from the call alone.

Split out of file-provenance.py so the history cache can key on it (bug 20927ccfe293c34e,
CIW-10): a cached Operation carries `static_targets(...)`, and the ownership policy stays out
of the cache key. The one input that is NOT a pure function of the call, the doc tool's
artifact-id lookup, stays in file-provenance.py (`catalog_targets`) and is read live.

Every name here is re-exported by file-provenance.py, because tests read them there.
"""
from __future__ import annotations

import dataclasses
import re
from typing import Iterator

from provenance_records import HistoryBatch

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

# Every tool name `write_targets` has a branch for. Any other name yields nothing, so the function
# returns at once for it. One scan of the real corpus makes 249,056 calls, 33,017 of them for
# `read_file` (bug 20927ccfe293c34e). These are the ONLY writers: add a name here when you add a
# branch there (`test_the_set_names_every_tool_the_branches_name` reds if you forget). Deliberately
# not named `*_TOOLS`, for the reason `_NON_ATOMIC_NAMES` gives: that gate reads such a constant as
# a list of registered codescout tools, and this one holds the Codex tool `apply_patch`.
_TARGET_NAMES = frozenset(CS_WRITE_TOOLS | NATIVE_WRITE_TOOLS | {
    "apply_patch",
    "Bash",
    "mcp__codescout__run_command",
    "mcp__codescout__doc",
    "mcp__codescout__artifact",          # legacy -> doc: pre-ceb5b57a name
    "mcp__codescout__artifact_augment",  # legacy -> doc: now doc(action="augment")
})

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

def patch_write_targets(patch: str) -> list[str]:
    """Extract native patch headers, preserving literal filenames and move operands."""
    if not isinstance(patch, str):
        return []
    lines = patch.splitlines()
    if not lines or lines[0] != "*** Begin Patch" or lines[-1] != "*** End Patch":
        return []
    paths = []
    for line in lines[1:-1]:
        match = re.fullmatch(r"\*\*\* (?:Add|Update|Delete) File: (.+)", line)
        move = re.fullmatch(r"\*\*\* Move to: (.+)", line)
        if match or move:
            paths.append((match or move).group(1))
    return list(dict.fromkeys(paths))


def static_targets(name: str, inp: object) -> Iterator[str]:
    """Paths this ONE tool call wrote, from the call alone. Empty for every read-only call.

    Everything `write_targets` yields except the doc tool's catalog-id lookups, in the same
    order. `catalog_targets` (file-provenance.py) supplies those, and `write_targets` appends
    them after this.
    """
    if not isinstance(inp, dict):
        return
    if name not in _TARGET_NAMES:
        return
    if name == "apply_patch":
        yield from patch_write_targets(inp.get("patch"))
    elif name in CS_WRITE_TOOLS:
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
            # lookup (`catalog_targets`) can never resolve it. Without this key an archive move
            # -- a bug file's normal end state -- attributes to nobody.
            for key in ("rel_path", "new_rel_path"):
                if isinstance(inp.get(key), str):
                    yield inp[key]
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


def with_targets(batch: HistoryBatch) -> HistoryBatch:
    """`batch` with each operation's `static_targets` stored on it, for the history cache.

    Gaps are untouched. A non-dict `input` (a malformed transcript line) stores `()`, which is
    what `static_targets` yields for it. The doc tool's catalog-id targets are NOT stored: they
    are read live at scan time (`catalog_targets` in file-provenance.py).
    """
    return dataclasses.replace(batch, operations=[
        dataclasses.replace(op, targets=tuple(static_targets(op.name, op.input)))
        for op in batch.operations])
