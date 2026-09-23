#!/usr/bin/env python3
"""Refuse a commit whose issue-cluster Index counts disagree with the corpus it STAGES.

Why this exists as a hook rather than only as `tests/issue_clusters.rs`:
`no_index_row_stores_a_count` reads the WORKING TREE, so it certifies the state on
disk. A partial commit ships a state that never existed on disk, and no re-run of that test
can address it — running the gate before committing answers a different question than the
commit poses, and answers it green. Measured 2026-09-01 (`cluster-promotion-session-log:F-7`,
`reconnaissance-patterns:R-155`): `461c037a` staged the ledger and six of seven retagged bug
files went unstaged, publishing counts of 3/1/2 against ZERO members at HEAD.

This reads the INDEX and nothing else -- `git ls-files` for the population, `git show :<path>`
for content -- so it answers the question the commit actually poses. It deliberately does NOT
depend on pre-commit's unstaged-stash: measured, that stash gives hooks the index version of
tracked files but leaves UNTRACKED files on disk, so a filesystem-reading hook would read the
ledger correctly and over-count the corpus.

The parse logic is duplicated from `tests/issue_clusters.rs` on purpose -- a cargo invocation
in the commit path costs ~7s and blocks unboundedly on the shared `target/` lock. Divergence
is closed by a MECHANISM, not by vigilance: `the_hook_script_agrees_on_the_cluster_parsers` runs
this file with `--source=worktree --json` and fails if the two derivations disagree. Change one
and the test reddens until you change the other.

**That mechanism covered the PARSERS and not the RULE SETS, and the gap shipped.** This hook
enforced a strict subset of what `tests/issue_clusters.rs` does -- it checked stored counts and
member growth, and never the "exactly one `cluster/<slug>` tag per open bug file" invariant -- so
a commit adding a second tag passed here and redded the shared gate for every other session in
the checkout. The test a reader consulted to rule that out compared `parse_index_counts` and
`cluster_tags`, which is parser parity, not rule parity, while its name said otherwise
(`docs/issues/archive/2026-09-09-the-pre-commit-cluster-hook-enforces-a-subset-of-the-gate-it-mirrors.md`).
`HOOK_RULES` below is the closed answer: `--rules` prints it, and the Rust side asserts it equals
its own declaration, where every test must be classified hook-owed or exempted with a reason.
"""

from __future__ import annotations

import json
import pathlib
import subprocess
import sys

# The std streams take their encoding from the LOCALE, and on Windows that locale is cp1252
# rather than UTF-8. Every `--fixture-*` mode below reads its corpus from stdin and this ledger's
# headings carry em dashes (`## IC-7 — ...`), so an unreconfigured stdin hands the parsers `â€”`
# where the file holds `—`. That is a silent mis-parse, not an error: the two derivations agree
# about every rule and disagree only about the bytes one of them was given. Measured 2026-09-14 —
# it reds `the_hook_script_agrees_on_the_index_section_scan` and
# `the_hook_script_agrees_on_the_mechanism_basis_scan` on all three windows-latest lanes while
# every Linux lane stays green. Reproduce the Windows read from anywhere:
#   printf '## IC-7 — t' | PYTHONIOENCODING=cp1252 python3 scripts/pre-commit-ledger-counts.py \
#     --fixture-index-sections
#
# stdout and stderr are reconfigured for the WRITE direction, which fails differently and in two
# ways depending on the character — measured, because the obvious guess is wrong on both counts:
#   `—` IS in cp1252 (0x97), so a refusal naming a heading does not raise. It emits one cp1252
#   byte where the reader expects UTF-8, so the message a committer must act on arrives mangled
#   with nothing reporting that it was.
#   A character cp1252 lacks — `→`, `✓` — raises UnicodeEncodeError on stdout, whose handler is
#   `strict`, killing the hook mid-refusal. stderr's is `backslashreplace` and degrades instead.
# The JSON path is immune either way (`json.dumps` escapes non-ASCII by default), so no fixture
# test can observe this half; it is verified by reading the streams' `.encoding` directly.
#
# File reads already pass `encoding="utf-8"` explicitly (see `read`). The streams are the sites
# that have no call-site parameter to pass one to.
for _stream in (sys.stdin, sys.stdout, sys.stderr):
    if _stream is not None:
        _stream.reconfigure(encoding="utf-8")

# Every rule this hook enforces, as a closed set, emitted by `--rules`.
#
# **The ids are the Rust TEST NAMES on purpose** -- a neutral id would need a translation table
# on one side or the other, and that table is exactly the surface that drifts. A renamed test
# reds `every_cluster_rule_is_hook_owed_or_exempt`, which checks each declared name against the
# `#[test]` functions that actually exist, so a rename cannot silently exempt a rule.
#
# `a_class_gaining_a_member_names_it` has no Rust twin and is declared HOOK-ONLY on the Rust
# side: it compares the INDEX against HEAD, a question no working-tree test can pose.
HOOK_RULES = [
    "a_class_gaining_a_member_names_it",
    "every_open_bug_file_declares_one_known_defect_class",
    "no_class_field_states_a_bare_n",
    "no_index_row_stores_a_count",
    "no_index_row_stores_a_mechanism",
    "every_declared_class_has_an_index_row",
    "no_mechanism_status_is_a_bare_verdict",
    "the_index_file_holds_no_class_sections",
]

LEDGER = "docs/trackers/issue-clusters.md"
# Since 2026-09-02 the ledger is an Index file PLUS one file per class. The split was a pure
# relocation (every section byte-identical) and exists because that one file was the repo's
# contention head -- 16 distinct sessions, 53 commits in a day. See
# docs/adrs/2026-09-02-isolate-what-is-cheap-own-what-is-shared.md.
#
# This mirror stays byte-comparable with tests/issue_clusters.rs::ledger_text(), which
# the_ledger_parsers_agree_on_a_fixture pins. The PARSERS are unchanged -- only what they are
# pointed at moved. Safe because every parser here is line-anchored, so none can straddle the
# join between two concatenated files.
LEDGER_DIR = "docs/trackers/issue-clusters"

# The `--source` this run is judging, set once by `main()`. Read only by
# `_emit_divergence_note`, which must stay silent unless the verdict came from the INDEX --
# under `--source=worktree` there is no second copy to disagree with and the note would be
# false. A module global rather than a parameter THREADED THROUGH SEVEN REFUSAL SITES: a
# parameter that a future check forgets to pass degrades to no note, silently, which is the
# same forgettable-placement failure the note itself exists to close. `None` until `main()`
# runs, so the `--fixture-*` modes -- which call the refusal emitters directly, with no repo
# and no index -- emit nothing.
_SOURCE: str | None = None


def _git(*args: str) -> str:
    r = subprocess.run(["git", *args], capture_output=True, text=True)
    if r.returncode != 0:
        raise SystemExit(f"git {' '.join(args)} failed: {r.stderr.strip()}")
    return r.stdout


def _repo_path(p: pathlib.PurePath) -> str:
    """Render a path the way this repo CITES paths: POSIX, on every platform.

    `class_files`' git sources already return POSIX -- git emits forward slashes whatever
    the host -- while the worktree source builds `Path` objects, which render with the
    NATIVE separator. Nothing reconciled the two, so on Windows the growth refusal cited
    `docs\\trackers\\issue-clusters\\IC-7-....md` from one branch and
    `docs/trackers/issue-clusters.md` (a literal) from the other, in the same message.

    Two costs, and the second is the one that made this worth a helper rather than a cast.
    A backslash path is a string the reader cannot paste into `git`, `grep`, or any of this
    repo's citation tooling, and `audit_doc_refs` keys on backticked path-shaped tokens so
    it cannot resolve one either -- the refusal is documentation-shaped and its whole point
    is that the path is greppable. And `class_file_for` splits on `"/"` to take a basename:
    a backslash path does not split at all, leaving `stem` as the entire path. That routing
    survived only on the `endswith(f"-{slug}")` fallback, i.e. by luck.

    Callers pass the `Path` itself, never `str(p)` -- the object knows its own flavour and
    `as_posix()` is what converts. That is also what makes the defect testable from Linux,
    where `str(p)` and `p.as_posix()` are byte-identical: see `--fixture-repo-path`.
    """
    return p.as_posix()


def class_files(source: str) -> list:
    """Every per-class file, read from the SAME source as the ledger text.

    Source-matching is load-bearing and is the reason this is not a bare glob: the hook's
    whole point is comparing what is STAGED against what is on disk, and a disk glob would
    silently mix the two -- reporting a per-class file's worktree content against an
    index-sourced ledger. `head` and `index` therefore go through git, and only `worktree`
    touches disk.

    Tracked-only for the git sources, matching the rest of this hook: an untracked per-class
    file is invisible here, so a local green defers rather than clears. That is the documented
    posture (see the module header), not an oversight.

    Deduplicated for the git sources: `git ls-files` reports index ENTRIES, so a file unmerged
    in an in-flight merge is listed once per stage. `full_ledger_text` CONCATENATES every path
    this returns, so a duplicate splices one cluster's text into the ledger three times, and
    `parse_bare_n_claims` -- which extends a list rather than a set -- then reports each of its
    violations three times in the refusal. `ls-tree` needs no such guard: a tree lists each path
    once, having no stages.
    """
    if source == "head":
        out = _git("ls-tree", "-r", "--name-only", "HEAD", LEDGER_DIR)
    elif source == "index":
        out = _git("ls-files", LEDGER_DIR)
    else:
        d = pathlib.Path(LEDGER_DIR)
        return sorted(_repo_path(p) for p in d.glob("*.md")) if d.is_dir() else []
    return sorted({p for p in out.splitlines() if p.endswith(".md")})


def class_file_for(slug: str, source: str) -> str:
    """The file whose `**Members:**` line owns `slug` -- a PATH, for the refusal to name.

    Since the per-class split the word "the ledger" denotes two things, and the refusal that
    demands a `**Members:**` edit could name neither. `docs/trackers/issue-clusters.md` is the
    ROSTER: it lists every slug and carries a `**Members:**` for exactly one of them. The other
    22 live one-per-file under `LEDGER_DIR`.

    Falling back to `LEDGER` is CORRECT, not a default: `cluster/unclassified` genuinely has no
    class file and its field genuinely is in the Index. The two branches are the two real cases,
    which is why this returns a path rather than an Optional -- there is no "unknown" outcome to
    represent, and handing the caller a None would put the old silence back one layer down.

    Matched on the basename, not the whole path, so a slug that happens to be a substring of a
    directory name cannot pull the wrong file. `IC-<n>-<slug>.md` is the naming convention;
    matching the stem's tail rather than the whole name keeps the `IC-<n>-` prefix out of it.
    """
    for path in class_files(source):
        stem = path.rsplit("/", 1)[-1].removesuffix(".md")
        if stem == slug or stem.endswith(f"-{slug}"):
            return path
    return LEDGER


def read_ledger(source: str):
    """Index file + every class file, concatenated -- the twin of ledger_text() in Rust.

    Returns None only when the Index itself is absent from `source`, preserving the caller's
    "nothing to check" path. A present Index with zero class files concatenates to just the
    Index, which is the pre-split shape and stays correct rather than erroring.
    """
    head = read(LEDGER, source)
    if head is None:
        return None
    parts = [head]
    for path in class_files(source):
        text = read(path, source)
        if text is not None:
            parts.append(text)
    return "\n".join(parts)


_INDEX_BLOBS: dict[str, str] | None = None


def _prime_index(paths: list[str]) -> None:
    """Read every path from the index in ONE `git cat-file --batch`.

    `git show :<path>` spawns a subprocess per file. Measured 2026-09-01 over the 555-file
    corpus: index mode 1360 ms against worktree mode 103 ms. The config header's "<0.2s" was
    the WORKTREE figure -- the mode `the_hook_script_agrees_on_the_cluster_parsers` runs -- while the
    hook itself runs index mode, so the documented number was right about a different question.

    Runtime is not a comfort metric here. pre-commit fails a hook when the whole-tree diff
    changes across it (`commands/run.py:203-206`, no per-hook opt-out), so a hook's duration IS
    the window in which another session's unrelated write is attributed to this commit. That
    defect is why the pre-push hooks were withdrawn; shortening the window is the only lever
    left. See the block at the top of `.pre-commit-config.yaml`.
    """
    global _INDEX_BLOBS
    stdin = "".join(f":{p}\n" for p in paths)
    r = subprocess.run(
        ["git", "cat-file", "--batch"], input=stdin.encode(), capture_output=True
    )
    blobs: dict[str, str] = {}
    buf, i = r.stdout, 0
    for p in paths:
        nl = buf.find(b"\n", i)
        if nl == -1:
            break
        header = buf[i:nl].decode("utf-8", "replace")
        # A path absent from the index prints `:<path> missing` with no body.
        if header.endswith(("missing", "ambiguous")):
            i = nl + 1
            continue
        size = int(header.rsplit(" ", 1)[1])
        body = nl + 1
        blobs[p] = buf[body : body + size].decode("utf-8", "replace")
        i = body + size + 1  # the newline git writes after the contents
    _INDEX_BLOBS = blobs

def read(path: str, source: str) -> str | None:
    """File content from the index (`git show :path`) or the working tree.

    Index reads consult a cache primed by [`_prime_index`]; the per-file `git show` below is
    the fallback for anything unprimed, so a slip in the batch parser costs speed, never
    correctness.
    """
    if source == "head":
        r = subprocess.run(["git", "show", f"HEAD:{path}"], capture_output=True, text=True)
        return r.stdout if r.returncode == 0 else None
    if source == "index":
        if _INDEX_BLOBS is not None and path in _INDEX_BLOBS:
            return _INDEX_BLOBS[path]
        r = subprocess.run(["git", "show", f":{path}"], capture_output=True, text=True)
        return r.stdout if r.returncode == 0 else None
    try:
        with open(path, encoding="utf-8") as fh:
            return fh.read()
    except OSError:
        return None


def bug_files() -> list[str]:
    """Mirrors `tracked_all_bug_files` -- the INDEX population, so untracked files are excluded.

    Deduplicated, and here that is not cosmetic. `git ls-files` reports index ENTRIES: while a
    merge holds an unresolved conflict the unmerged path is listed once per stage (1/2/3), so one
    bug file arrives three times. `actual_counts` below turns this into a PER-CLASS TALLY, so a
    single conflicted file under `docs/issues/` trebles a class's count and this hook then REFUSES
    the commit over a number nobody can reproduce once the merge resolves. A gate that blocks
    work, not a test that reports -- which is why this site outranks the Rust one it mirrors.

    NEITHER PINNING MECHANISM REACHES IT. `the_hook_script_agrees_on_the_cluster_parsers` runs in a clean
    tree, where the deduplicated and un-deduplicated forms are byte-identical. And
    `probe-caveat-density.py`'s `_self_check` says so about itself: it and this gate SHARE this
    function, so a defect inside it makes both sides agree, which at the point of use is
    indistinguishable from corroboration.
    """
    return sorted(
        {
            p
            for p in _git("ls-files", "docs/issues").splitlines()
            if p.endswith(".md") and not p.endswith("_TEMPLATE.md")
        }
    )


def open_bug_files() -> list[str]:
    """Mirrors `tracked_open_bug_files` -- OPEN bug files only, so `docs/issues/archive/` is out.

    The population is narrower than `bug_files` on purpose and the two are not interchangeable:
    the count rules run over the whole corpus INCLUDING the archive (a fixed bug is still a
    member of its class), while the one-tag rule runs over open files only. An archived file
    predating the closed set would otherwise refuse every commit that touches anything, with no
    action available to the committer -- a guard whose remedy nobody can perform.
    """
    return [
        p
        for p in bug_files()
        if p.startswith("docs/issues/") and "/" not in p[len("docs/issues/") :]
    ]


def bad_tag_declarations(valid: set[str], source: str) -> list[str]:
    """Mirrors `every_open_bug_file_declares_one_known_defect_class` -- exactly one KNOWN tag.

    The Rust `verdict` is the reference and this reproduces its four arms, including the one
    that is easy to drop: a file with NO frontmatter is a defect reported as "no cluster/ tag",
    never a file skipped. Skipping it would make the worst-formed bug file in the corpus the one
    the gate is quietest about.

    Emits the same four strings the Rust assertion does, so a reader who hit one gate recognises
    the other rather than debugging a second, differently-worded refusal.
    """
    out = []
    for rel in open_bug_files():
        content = read(rel, source)
        if content is None:
            continue
        fm = frontmatter(content)
        tags = cluster_tags(fm) if fm is not None else []
        if not tags:
            out.append(f"{rel} -- no cluster/ tag")
        elif len(tags) > 1:
            out.append(f"{rel} -- {len(tags)} cluster/ tags: {tags}")
        elif tags[0] not in valid:
            out.append(f"{rel} -- unknown slug: cluster/{tags[0]}")
    return out


def valid_slugs(ledger: str) -> set[str]:
    """Mirrors `valid_slugs` -- the ledger's `**Slug:**` declarations are the closed set."""
    out = set()
    for line in ledger.splitlines():
        if not line.startswith("**Slug:**"):
            continue
        slug = backticked_cluster_slug(line[len("**Slug:**") :])
        if slug is None:
            continue
        # The ledger's own template declares `cluster/<slug>`; a placeholder is not a class.
        if slug and all(c.islower() and c.isascii() or c == "-" for c in slug):
            out.add(slug)
    return out


def frontmatter(content: str) -> str | None:
    """Mirrors `frontmatter` -- the YAML block without its fences, or None."""
    if not content.startswith("---\n"):
        return None
    rest = content[len("---\n") :]
    end = rest.find("\n---")
    return None if end < 0 else rest[:end]


def cluster_tags(fm: str) -> list[str]:
    """Mirrors `cluster_tags` -- BOTH YAML forms; reading one silently under-reports."""
    out: list[str] = []
    in_block = False
    for line in fm.splitlines():
        if line.startswith("tags:"):
            rest = line[len("tags:") :].strip()
            if rest.startswith("[") and rest.endswith("]"):
                out.extend(t.strip().strip("\"'") for t in rest[1:-1].split(","))
                in_block = False
            else:
                in_block = rest == ""
            continue
        if in_block:
            if line.startswith("-"):
                item = line[1:]
            elif line.lstrip().startswith("- "):
                item = line.lstrip()[1:]
            else:
                # A non-item line ends the block; a later key is not a tag.
                in_block = False
                continue
            out.append(item.strip().strip("\"'"))
    return [t[len("cluster/") :] for t in out if t.startswith("cluster/")]


def backticked_cluster_slug(rest: str) -> str | None:
    """Mirrors `backticked_cluster_slug` -- the slug inside the first backtick pair."""
    a = rest.find("`")
    if a < 0:
        return None
    b = rest.find("`", a + 1)
    if b < 0:
        return None
    inner = rest[a + 1 : b]
    return inner[len("cluster/") :] if inner.startswith("cluster/") else None


def bare_n_values(rest: str) -> list[int]:
    """Mirrors `bare_n_values` -- every n= NOT inside a backtick span.

    The backtick is the escape and is the ONLY signal separating a live claim from a quotation:
    the ledger preserves superseded figures with their derivation, so `**Promotes to:**`
    legitimately carries "took it from `n=2` to `n=27`". Position cannot discriminate there --
    two entries OPEN that field with a historical citation -- so a first-n=-wins rule reddens two
    correct fields. Eleven backticked n= occurrences under the span reading, ten of which would
    be flagged if bare; a tight-token count gives ten, the difference being `n=1 taggable` -- a
    backticked PHRASE, and corpus evidence that the house style already wraps prose.

    SPAN, not adjacency. The first shipped version tested only the byte before `n=`, so an n=
    inside a backticked PHRASE read as a live claim. Complete pairs only: a dangling backtick
    opens nothing, leaving the tail CHECKED, because a false report is loud where a skipped
    claim would be silent.

    `n>=` is deliberately not matched: it is the promotion THRESHOLD in prose, never a count.
    """
    spans: list[tuple[int, int]] = []
    open_at: int | None = None
    for i, ch in enumerate(rest):
        if ch == "`":
            if open_at is None:
                open_at = i
            else:
                spans.append((open_at, i))
                open_at = None

    out: list[int] = []
    i = 0
    while True:
        at = rest.find("n=", i)
        if at < 0:
            return out
        quoted = any(a < at < b for a, b in spans)
        digits = ""
        for ch in rest[at + 2 :]:
            if not ch.isdigit():
                break
            digits += ch
        if digits and not quoted:
            out.append(int(digits))
        i = at + 2


def parse_bare_n_claims(ledger: str, valid: set[str]) -> list[list]:
    """Mirrors `parse_bare_n_claims` -- every bare n= in a class's judgement fields.

    Both fields are read: `**Members:**` states the count, `**Promotes to:**` reasons from it and
    is where the four measured drifts of 2026-09-01 actually did their damage (`0c5bab41`).

    The slug RESETS at every `## IC-` heading and is NOT consumed on use, because it now spans
    two fields -- so the reset is the only clearing mechanism, and a section stating a count
    while declaring no slug is the shape it guards.
    """
    fields = (("**Members:**", "Members"), ("**Promotes to:**", "Promotes to"))
    out: list[list] = []
    slug: str | None = None
    for line in ledger.splitlines():
        if line.startswith("## IC-"):
            slug = None
        elif line.startswith("**Slug:**"):
            cand = backticked_cluster_slug(line[len("**Slug:**") :])
            slug = cand if cand in valid else None
        elif slug is not None:
            for prefix, label in fields:
                if line.startswith(prefix):
                    out.extend([slug, label, n] for n in bare_n_values(line[len(prefix) :]))
    out.sort()
    return out


def parse_index_counts(ledger: str, valid: set[str]) -> dict[str, int]:
    """Mirrors `parse_index_counts` -- the cell IMMEDIATELY AFTER the slug cell.

    Never "the first number in the row": the mechanism column carries digits of its own, so a
    scan-for-a-number parser returns a plausible wrong value rather than failing. A row whose
    count cell does not parse is left ABSENT rather than defaulted to 0.
    """
    out: dict[str, int] = {}
    for line in ledger.splitlines():
        if not line.startswith("| IC-"):
            continue
        cells = [c.strip() for c in line.split("|")]
        for i, cell in enumerate(cells):
            if not (cell.startswith("`") and cell.endswith("`") and len(cell) > 1):
                continue
            inner = cell[1:-1]
            if inner not in valid:
                continue
            if i + 1 < len(cells):
                try:
                    out[inner] = int(cells[i + 1])
                except ValueError:
                    pass
            break
    return out


def parse_index_rows(ledger: str, valid: set[str]) -> set[str]:
    """Mirrors `parse_index_rows` -- which slugs have an Index row AT ALL, count-free.

    The count-free twin of `parse_index_counts` above, and the two deliberately cannot be one
    parser: the emptiness that is a PASS for `no_index_row_stores_a_count` is exactly the failure
    mode this one exists to catch. A table nobody can read returns {} from both, which the count
    rule reads as "no stored counts, all good".

    Scans PAST a backticked cell that is not a known slug rather than stopping at the first
    backtick -- the `promotes to` cell carries backticked ids of its own, and on a row where one
    precedes the slug a stop-at-first parser silently drops the row.
    """
    out: set[str] = set()
    for line in ledger.splitlines():
        if not line.startswith("| IC-"):
            continue
        for cell in (c.strip() for c in line.split("|")):
            if not (cell.startswith("`") and cell.endswith("`") and len(cell) > 1):
                continue
            inner = cell[1:-1]
            if inner in valid:
                out.add(inner)
                break
    return out


def missing_index_rows(valid: set[str], rows: set[str]) -> list[str]:
    """Mirrors `missing_index_rows` -- declared slugs with no row, minus the one exemption.

    `unclassified` is the sanctioned escape hatch. It carries a `**Slug:**`/`**Members:**` pair so
    CHECK 3 can track its growth like any class, but by the ledger's own design it has no numbered
    id and no Index row. The exemption is exactly that one slug and nothing wider; the Rust side
    pins that narrowness with `missing_index_rows_exempts_only_unclassified`, because an exemption
    that quietly grew to "any slug missing a row" would make this rule vacuous.
    """
    return sorted(s for s in valid if s != "unclassified" and s not in rows)


def index_rows_with_extra_cells(ledger: str) -> list[tuple[str, str]]:
    """Mirrors `index_rows_with_extra_cells` -- Index rows carrying a cell past `promotes to`.

    `| a | b | c | d |` splits on `|` into six pieces: a leading and a trailing empty, plus the
    four content cells. Anything longer is a fifth column, which since 2026-09-13 means a
    `mechanism` cell has been refilled. That cell was a second copy of the entry's own
    `**Mechanism status:**` field, sitting in the one file every IC record shares; nothing read
    it, so it drifted on 2 of 23 rows before it was deleted.

    POSITIONAL, not a scan for the word. Two rows' `promotes to` prose legitimately contains
    "mechanism" -- IC-5 quotes a withdrawn "mechanism owed" clause and IC-12 says the remedy is
    knowledge rather than mechanism -- so a word scan reds on correct rows and gets deleted.
    """
    out: list[tuple[str, str]] = []
    for line in ledger.splitlines():
        if not line.startswith("| IC-"):
            continue
        cells = [c.strip() for c in line.split("|")]
        if len(cells) > 6:
            out.append((cells[1], " | ".join(cells[5:-1])))
    return out


def index_header_readvertises_mechanism(ledger: str) -> str | None:
    """The Index header line if it still names the deleted column, else None.

    The rows can be clean while the header invites the next editor to refill them, so this is a
    separate finding rather than a second condition on the row scan.
    """
    for line in ledger.splitlines():
        if line.startswith("| id | class | slug") and "mechanism" in line:
            return line
    return None


def index_class_sections(index_text: str) -> list[str]:
    """`## IC-<digits> ` headings in the Index file -- the twin of `index_class_sections` in
    tests/issue_clusters.rs.

    Takes the INDEX FILE ALONE and never `read_ledger()`'s concatenation. Every class file opens
    with its own `## IC-N —` heading, so the joined text matches once per class on a perfectly
    healthy corpus and this check would refuse every commit in the repo. The caller passing
    `read(LEDGER, source)` rather than the ledger it already holds is the load-bearing half.

    Digits are required because `## IC-N — <the class…>` in the template is not a class section:
    the token grammar is `[A-Z]{1,3}-\\d+`, and `N` is not a digit.

    `isascii()` beside `isdigit()` is what keeps this agreeing with Rust. Python's `str.isdigit()`
    is true for `٣` and `²`; `char::is_ascii_digit` is not, so without it the two sides classify
    the same heading differently and only the fixture would ever say so.
    """
    out = []
    for line in index_text.splitlines():
        if not line.startswith("## IC-"):
            continue
        n, sep, _ = line[len("## IC-") :].partition(" ")
        # `sep` is what distinguishes "no space at all" from "empty head": Rust's
        # `split_once(' ')` yields None for `## IC-8` and `("", rest)` for `## IC- 8`, and both
        # are non-findings. `partition` collapses them without it.
        if sep and n and n.isascii() and n.isdigit():
            out.append(line)
    return out


def mechanism_statuses(ledger: str) -> list[list]:
    """Mirrors `mechanism_statuses` -- `[entry id, body]` for every `**Mechanism status:**`.

    FENCE-AWARE, and that is not a nicety: the `## Template for new entries` block carries a
    specimen field, and a worked example teaching the syntax is not a declaration. Collecting it
    would make this check permanently red on a ledger that is correct -- the same rule
    `**Valid:**` detection uses.

    The id is the first whitespace token after `## IC-` with trailing non-digits trimmed, so
    `## IC-12 — title` keys `IC-12`. `id` is never reset by a non-`IC-` heading, matching Rust:
    a `**Mechanism status:**` under a later prose heading still belongs to the last class.
    """
    out: list[list] = []
    ident: str | None = None
    fenced = False
    for line in ledger.splitlines():
        if line.lstrip().startswith("```"):
            fenced = not fenced
            continue
        if fenced:
            continue
        if line.startswith("## IC-"):
            tok = line[len("## IC-") :].split()
            if tok:
                end = len(tok[0])
                while end > 0 and not (tok[0][end - 1].isascii() and tok[0][end - 1].isdigit()):
                    end -= 1
                ident = "IC-" + tok[0][:end]
            else:
                ident = "IC-"
        elif line.startswith("**Mechanism status:**") and ident is not None:
            out.append([ident, line[len("**Mechanism status:**") :].strip()])
    return out


def is_unbasised(body: str) -> bool:
    """Mirrors `is_unbasised` -- the field states a verdict and gives no way to check it.

    Length of what REMAINS once the verdict prefix is stripped, threshold 20 CHARACTERS (not
    bytes -- the corpus is full of em dashes). Two earlier formulations were refuted by
    measurement and are recorded in the Rust twin's doc comment; do not re-propose them here.

    The verdict list LOCATES a prefix and never DECIDES. An unknown verdict word is simply not
    stripped, the remainder is then the whole field, and the length test still applies -- so a
    novel bare verdict (`unbuilt` -> 7) is caught while a long field passes. The failure mode
    degrades toward flagging rather than toward silence, which is the direction a word-list
    formulation got backwards: a word list is monotone under RENAMING, so re-wording the
    ledger's own `none yet` would have disarmed it rather than failed it.
    """
    verdicts = ("none yet", "not yet", "partial", "designed", "shipped", "none")
    s = body.strip().lstrip("*`").strip()
    low = s.lower()
    # Longest first: "none yet" must win over "none".
    for v in sorted(verdicts, key=len, reverse=True):
        if low.startswith(v):
            s = s[len(v) :]
            break
    # Tolerate a parenthetical qualifier, e.g. `shipped (partial)` -- it qualifies the verdict
    # and is not a way to check it.
    lead = s.lstrip()
    if lead.startswith("("):
        _, sep, after = lead[1:].partition(")")
        if sep:
            s = after
    return len(s.strip().lstrip("*`.—–-").strip()) < 20


def actual_counts(valid: set[str], source: str) -> dict[str, int]:
    """Mirrors `actual_counts` -- seeded at 0 so a class with no members is still compared."""
    out = {s: 0 for s in valid}
    files = bug_files()
    if source == "index":
        # Prime here rather than in `read`: this is the first point the whole population is
        # known, and it is the only caller that reads more than one file.
        _prime_index(files)
    for rel in files:
        content = read(rel, source)
        if content is None:
            continue
        fm = frontmatter(content)
        if fm is None:
            continue
        for tag in cluster_tags(fm):
            if tag in out:
                out[tag] += 1
    return out


def _corpus_paths_diverged() -> list[str]:
    """Corpus paths whose WORKTREE bytes differ from their INDEX bytes.

    `git diff --name-only` with no revision is exactly worktree-vs-index, which is the
    divergence this hook's refusals are computed across -- one call, no per-file `git show`.
    Scoped to this hook's own corpus so an unrelated dirty `.rs` never surfaces in a refusal
    about the clusters ledger.

    Best-effort, and deliberately NOT `_git`: that raises `SystemExit` on a non-zero status,
    which would turn "git was momentarily unavailable" into a crash of a hook whose real
    verdict is already on stderr by the time this runs.
    """
    try:
        r = subprocess.run(
            ["git", "diff", "--name-only", "--", LEDGER, LEDGER_DIR, "docs/issues"],
            capture_output=True,
            text=True,
        )
    except OSError:
        return []
    if r.returncode != 0:
        return []
    return sorted({p for p in r.stdout.splitlines() if p.endswith(".md")})


def _emit_divergence_note() -> None:
    """Name a worktree/index disagreement, because the refusal above cites a file that shows
    the opposite of what it says.

    THE REFUSAL IS CORRECT AND THE READER'S NEXT ACTION IS WRONG, which is the narrow thing
    this closes. Every refusal here names a corpus file and sends the reader to it. When that
    file is dirty the bytes they open are not the bytes the verdict was computed from -- so a
    reader who follows the instruction finds their own slug sitting in the `**Members:**` line
    the refusal just said does not contain it. Measured 2026-09-15 in an isolated repo, same
    grep against both copies: worktree `1`, index `0`. The rational conclusion from that pair
    is "the gate is broken", and the action it licenses is `--no-verify`.

    So this does NOT suppress or soften the verdict -- the verdict is right about the index,
    and the index is what the commit publishes. It supplies the one fact that makes the
    contradiction legible, and it fires only on a refusal: a hook that says this on every
    green commit teaches the same `--no-verify` lesson by a different route.

    Placed in the shared tail rather than at each refusal ON PURPOSE. There are seven
    `return 1` sites and every one already routes through here, so a check added later
    inherits this without its author knowing the hazard exists. A head position would read
    better and would be one more thing to remember -- `CLAUDE.md` § *Observer Blindness*
    position 3 prefers the placement that cannot be omitted.

    Silent unless `--source=index`: under `--source=worktree` the verdict IS computed from the
    bytes on disk, so there is no divergence to warn about and this note would be false.
    """
    if _SOURCE != "index":
        return
    diverged = _corpus_paths_diverged()
    if not diverged:
        return
    print(
        "\nNOTE -- the worktree and the index DISAGREE about "
        f"{len(diverged)} of this hook's corpus file(s):\n  "
        + "\n  ".join(diverged)
        + "\n\n"
        "The refusal above was computed from the INDEX, because the index is what your\n"
        "commit publishes. The paths above read DIFFERENTLY on disk. So opening one to\n"
        "check the refusal can show you the OPPOSITE of what it says -- your slug present\n"
        "in a `**Members:**` line the refusal reports as missing, for instance. That is\n"
        "this divergence, not a contradiction, and not a broken gate.\n\n"
        "If the edit that satisfies the rule is in a path above, it is on disk and unstaged:\n"
        "stage it, or commit it together with what you already staged.\n\n"
        "If you did not write those edits, another session holds them -- and this hook's\n"
        "CODE runs from the working tree, so their edit went live for you the moment they\n"
        "saved it, with no commit in between. Name the owner:\n"
        "    python3 scripts/file-provenance.py <path>\n"
        "then ask them whether that edit is ready to STAGE. Ask that, not \"is it yours\":\n"
        "it has three answers they can actually give -- yes, not yet, or I will revert it --\n"
        "and each one tells you what to do next. Waiting clears nothing on its own: the\n"
        "index moves only when somebody stages.",
        file=sys.stderr,
    )

def _emit_sequence_tail() -> None:
    """Print the shared commit-sequence tail, if it is readable.

    Forward reach. Each hook here teaches only its own rule at its own collision, so the
    sequence was being learned one collision at a time -- nine cross-session messages and
    roughly two hours across two sessions for one two-author commit, measured 2026-09-01.

    Single emitted copy (``scripts/commit-sequence-tail.txt``), shared by all three
    refusing hooks, so the three texts cannot drift apart. The full prose with the
    measurement behind each step lives in
    ``docs/conventions/shared-checkout-commit-sequence.md``; that split is a summary and
    its source, the same shape as CLAUDE.md's gate sentence against
    ``docs/conventions/gate-ordering.md``, not two copies of one text.

    Best-effort by design: a missing or unreadable tail must never turn this hook's own
    verdict into a crash, and that verdict is already on stderr by the time this runs.

    Also carries ``_emit_divergence_note``, and that is the whole reason it is called from
    seven places rather than one: this function is the only line every refusal already
    crosses, so wiring the note here gives it to checks nobody has written yet. See that
    function for why the note exists at all.
    """
    _emit_divergence_note()
    tail = pathlib.Path(__file__).with_name("commit-sequence-tail.txt")
    try:
        print("\n" + tail.read_text(encoding="utf-8"), file=sys.stderr, end="")
    except OSError:
        pass

def members_fields(ledger: str, valid: set[str]) -> dict[str, str]:
    """slug -> its `**Members:**` line, keyed by the `**Slug:**` its own section declares."""
    out: dict[str, str] = {}
    cur: str | None = None
    for line in ledger.splitlines():
        if line.startswith("## IC-"):
            cur = None
        elif line.startswith("**Slug:**"):
            cur = backticked_cluster_slug(line[len("**Slug:**"):])
        elif line.startswith("**Members:**") and cur is not None and cur in valid:
            out[cur] = line
    return out


def _stem(rel: str) -> str:
    """The dateless slug -- a bug file's identity ACROSS a rename.

    `docs/issues/X.md` and `docs/issues/archive/X.md` are the same record; only the path moved.
    Keying membership on the path makes every archive move -- which this repo mandates for every
    verified fix -- read as a brand-new member, so the gate refuses the archive flow. Measured
    2026-09-02 against a peer's live archive commit, minutes after shipping.
    """
    stem = pathlib.Path(rel).stem
    if len(stem) > 11 and stem[4] == "-" and stem[7] == "-" and stem[10] == "-":
        stem = stem[11:]
    return stem


def bug_files_at(source: str) -> list[str]:
    """The bug-file population of ONE tree. `head` lists HEAD's own paths, not the index's."""
    if source != "head":
        return bug_files()
    return [
        p
        for p in _git("ls-tree", "-r", "--name-only", "HEAD", "docs/issues").splitlines()
        if p.endswith(".md") and not p.endswith("_TEMPLATE.md")
    ]


def tags_by_stem(source: str) -> dict[str, set[str]]:
    """stem -> cluster tags, over the population of the named tree."""
    out: dict[str, set[str]] = {}
    for rel in bug_files_at(source):
        content = read(rel, source)
        if content is None:
            continue
        out.setdefault(_stem(rel), set()).update(cluster_tags(frontmatter(content) or ""))
    return out


def added_member_stems(source: str) -> dict[str, set[str]]:
    """slug -> dateless stems of bug files carrying it now that did not carry it at HEAD."""
    now, was = tags_by_stem(source), tags_by_stem("head")
    out: dict[str, set[str]] = {}
    for stem, tags in now.items():
        for slug in tags - was.get(stem, set()):
            out.setdefault(slug, set()).add(stem)
    return out


def emit_growth_refusal(undocumented: list) -> int:
    """Compose and print the growth refusal. Extracted from `main` so it is REACHABLE.

    The refusal's job is to route the author to the `**Members:**` field, and the field moved
    when the ledger split per class. Triggering this from a test would mean staging a bug file
    into a shared index, so the composer is separated from the detection and driven directly by
    `--fixture-growth-refusal`. Detection is already covered; what was untested is the routing.

    Returns 1: this is a refusal, and the caller returns it.
    """
    print(
        "a class gained a member and its `**Members:**` does not name it:\n  "
        + "\n  ".join(
            f"cluster/{slug} -- the field is in `{path}`\n      "
            "expected it to change and to contain one of: "
            + (", ".join(f"`{x}`" for x in stems) or "(any change)")
            for slug, stems, path in undocumented
        )
        + "\n\n"
        "THE PATH ABOVE IS THE POINT -- do not go to the Index looking for the field.\n"
        "Since the per-class split, `docs/trackers/issue-clusters.md` is the ROSTER: it lists\n"
        "every slug and carries a `**Members:**` for exactly ONE of them\n"
        "(`cluster/unclassified`). Grepping your slug there returns 0, and that zero means\n"
        "WRONG FILE, not `no such class` -- while a generic `cluster/` grep returns dozens,\n"
        "which is what makes the wrong file look like the right one.\n\n"
        "The count used to force this edit. It no longer exists, so this asks for the half that\n"
        "was always the valuable one: WHY this instance belongs to this class. The ledger's own\n"
        "shape is `+1: `<slug-without-the-date>`` followed by the derivation.\n\n"
        "This is deliberately NOT `did the line change` -- a trailing space would satisfy that,\n"
        "and the count gate it replaces could not be satisfied by accident.\n\n"
        "If the class gained a member by RETAG rather than by a new file, changing the line is\n"
        "enough and this passes.\n\n"
        "BOTH SIDES OF THIS ARE NAMED ABOVE, and that is deliberate -- the corpus side is your\n"
        "bug file, the ledger side is the `**Members:**` line, and they must land in ONE commit\n"
        "or the gate is red in one direction or the other. If the ledger is contended right now:\n"
        "  - It is a WAIT, not a re-derivation. The edit is a one-line append carrying no number,\n"
        "    so no peer's commit can invalidate it between your writing it and your committing\n"
        "    it. That was not true of the count this replaced.\n"
        "  - A peer mid-archive-move does NOT cause this. This check reads the INDEX\n"
        "    (`git show :path`), never the worktree, so a file deleted from the worktree but\n"
        "    still tracked is read normally. If you are here, a class really did gain a member.\n"
        "  - If you genuinely cannot land both, leaving your bug file UNSTAGED is a legal state\n"
        "    and the gate will pass -- `git ls-files` is the population, so an untracked file is\n"
        "    invisible to it. Said out loud because it is lossy: the evidence stays off the\n"
        "    corpus until you stage it, and nothing will remind you.",
        file=sys.stderr,
    )
    _emit_sequence_tail()
    return 1


def main() -> int:
    source = "index"
    as_json = False
    for arg in sys.argv[1:]:
        if arg.startswith("--source="):
            source = arg.split("=", 1)[1]
        elif arg == "--json":
            as_json = True
        elif arg == "--rules":
            # The closed set of rules this hook enforces, for
            # `the_hook_enforces_every_rule_it_declares` on the Rust side. PRINTED rather than
            # inferred: a test that scraped `main` for check blocks would be asserting about
            # its own re-implementation of this file's structure, which is indistinguishable
            # from coverage until you break the thing that ships.
            print(json.dumps(sorted(HOOK_RULES)))
            return 0
        elif arg == "--fixture-ledger":
            # Pure over stdin: both ledger parsers on a caller-supplied ledger, so
            # `the_ledger_parsers_agree_on_a_fixture` can feed shapes the LIVE corpus does not
            # contain. It does not contain them by construction — every real section declares a
            # slug before its count, so the section-boundary reset is unreachable from the
            # corpus and a corpus-driven agreement check passes with it deleted. Measured.
            fixture = sys.stdin.read()
            fv = valid_slugs(fixture)
            print(json.dumps(
                {
                    "declared": parse_index_counts(fixture, fv),
                    "claimed": parse_bare_n_claims(fixture, fv),
                },
                sort_keys=True,
            ))
            return 0
        elif arg == "--fixture-growth-refusal":
            # Pure over stdin: one slug per line, composed with a synthetic stem. The LIVE
            # corpus cannot exercise this branch -- reaching it needs a bug file STAGED into
            # the index, and a test that stages into a shared checkout's index is a defect of
            # its own. So detection stays corpus-driven and ROUTING is driven from here.
            rows = [
                (s, ["fixture-stem"], class_file_for(s, "worktree"))
                for s in sys.stdin.read().split()
            ]
            emit_growth_refusal(rows)
            return 0
        elif arg == "--fixture-repo-path":
            # Windows-shaped paths over stdin, through the SAME renderer the worktree
            # branch of `class_files` uses.
            #
            # This fixture exists because the defect is INVISIBLE ON POSIX BY
            # CONSTRUCTION: `str(p)` and `p.as_posix()` are byte-identical on Linux and
            # macOS, so no assertion over real `class_files` output can discriminate
            # there, and the Rust test that caught this in CI
            # (`the_growth_refusal_names_the_file_holding_the_members_field`) is green on
            # every developer machine in this project while being red on Windows.
            # `PureWindowsPath` carries the foreign flavour without needing the foreign
            # OS, so the Linux gate can red on the mutation that shipped.
            for line in sys.stdin.read().split():
                print(_repo_path(pathlib.PureWindowsPath(line)))
            return 0
        elif arg == "--fixture-tags":
            # Pure over stdin, so `the_hook_script_agrees_on_both_yaml_tag_styles` can feed
            # both YAML forms directly. The live corpus cannot be trusted to exercise the
            # inline arm: it does carry flow-style `cluster/` tags today (re-derive with
            # `git grep -clE '^tags: *\[.*cluster/' -- docs/issues/*.md docs/issues/archive/*.md`,
            # never a bare count — this file is IC-11's own instance of citing one that decays),
            # but the count moves with every filing, so this fixture is what keeps the inline
            # arm reliably exercised regardless of what the live corpus holds on a given day.
            print(json.dumps(cluster_tags(frontmatter(sys.stdin.read()) or "")))
            return 0
        elif arg == "--fixture-index-mechanism":
            # Pure over stdin, so `the_index_mechanism_scan_discriminates` can feed a table whose
            # answers are known. The live corpus cannot reach the interesting branch AT ALL: the
            # check exists to keep a refilled column out, so a correct ledger has zero findings
            # forever, and asserting over it would be an absence assertion pinned to an absence
            # -- green whether the scan works or is deleted. The fixture is the only surface on
            # which this parser can be shown to return a non-empty answer.
            fx = sys.stdin.read()
            print(
                json.dumps(
                    {
                        "extra": index_rows_with_extra_cells(fx),
                        "header": index_header_readvertises_mechanism(fx),
                    }
                )
            )
            return 0
        elif arg == "--fixture-index-sections":
            # Pure over stdin, so `the_hook_script_agrees_on_the_index_section_scan` can feed
            # headings the live corpus cannot hold. It cannot hold them BY CONSTRUCTION: this
            # check exists to keep class sections out of the Index, so a correct ledger yields
            # zero findings forever and a corpus-driven comparison is two empty lists -- green
            # whether this scan works or has been deleted outright. Same argument as
            # `--fixture-index-mechanism` above, for the same reason.
            print(json.dumps(index_class_sections(sys.stdin.read())))
            return 0
        elif arg == "--fixture-index-rows":
            # Pure over stdin: a mini-ledger (its own `**Slug:**` declarations plus an Index
            # table), so `the_hook_script_agrees_on_the_index_row_scan` can feed rows the live
            # corpus does not contain -- an unbackticked slug cell, a non-slug backtick standing
            # ahead of the real one, a prose line naming a slug. All three are shapes a correct
            # ledger never holds, and each is a way a looser parser silently returns the right
            # answer for the wrong reason.
            fx = sys.stdin.read()
            fv = valid_slugs(fx)
            fr = parse_index_rows(fx, fv)
            print(json.dumps({"rows": sorted(fr), "missing": missing_index_rows(fv, fr)}))
            return 0
        elif arg == "--fixture-mechanism-basis":
            # Pure over stdin: BOTH halves of the rule in one document, because they fail
            # differently and a fixture can hold shapes the corpus cannot. `statuses` exercises
            # the fence-aware scan (a template specimen inside a fence must NOT be collected --
            # the live ledger has exactly one and it is already correct, so the corpus can never
            # show this working). `unbasised` exercises the predicate on verdict wordings the
            # corpus does not contain -- `unbuilt`, `TBD`, `no mechanism yet` -- which are
            # precisely the ones a word-list formulation let through, so a corpus-driven check
            # cannot see the regression that actually happened.
            fx = sys.stdin.read()
            st = mechanism_statuses(fx)
            print(json.dumps(
                {
                    "statuses": st,
                    "unbasised": [ic for ic, body in st if is_unbasised(body)],
                },
                sort_keys=True,
            ))
            return 0
        else:
            raise SystemExit(f"unknown flag {arg!r}")
    if source not in ("index", "worktree", "head"):
        raise SystemExit(f"--source must be index|worktree|head, got {source!r}")

    # AFTER the arg loop, not inside it, and the difference is the whole wiring. The hook
    # invokes this script with NO arguments -- `--source` is never passed on the one path
    # that refuses real commits -- so an assignment guarded by `arg.startswith("--source=")`
    # leaves `_SOURCE` at `None` exactly there, and `_emit_divergence_note` returns early on
    # every hook run while passing every test that drives it with an explicit flag. Caught
    # while writing it; recorded because the green version is indistinguishable from the
    # wired one until you run the hook with no arguments.
    global _SOURCE
    _SOURCE = source

    ledger = read_ledger(source)
    if ledger is None:
        # Not staged and not on disk: nothing to check. Silence is correct -- a commit that
        # touches neither the ledger nor a bug file must not be blocked by this hook.
        return 0

    valid = valid_slugs(ledger)
    declared = parse_index_counts(ledger, valid)
    claimed = parse_bare_n_claims(ledger, valid)
    actual = actual_counts(valid, source)

    if as_json:
        print(json.dumps(
            {"declared": declared, "actual": actual, "claimed": claimed},
            sort_keys=True,
        ))
        return 0

    # CHECK 1 -- no STORED count. Mirrors `no_index_row_stores_a_count` and
    # `no_class_field_states_a_bare_n`. Counts are derived
    # (`scripts/probe-cluster-census.py`); storing them in a file 22 classes share made every
    # bug filer edit it, and a peer's commit staled yours between deriving and committing.
    stored = [
        f"cluster/{slug} -- the Index row stores a count ({n})"
        for slug, n in sorted(declared.items())
    ] + [
        f"cluster/{slug} -- **{field}:** stores a bare n={n}" for slug, field, n in claimed
    ]
    if stored:
        print(
            "this commit stores a DERIVED count in the clusters ledger:\n  "
            + "\n  ".join(stored)
            + "\n\n"
            "Counts are no longer stored. Derive them:\n"
            "  python3 scripts/probe-cluster-census.py\n\n"
            "If you meant to QUOTE a figure -- which the house style encourages, with its\n"
            "derivation -- wrap it in backticks. A backticked `n=N` is a quotation and is\n"
            "deliberately not checked. If you meant to state today's count, cite the probe\n"
            "instead, so the sentence cannot decay.",
            file=sys.stderr,
        )
        _emit_sequence_tail()
        return 1

    # CHECK 4 -- the Index table stores no mechanism status.
    #
    # Mirrors `no_index_row_stores_a_mechanism`. The number is its order of ADDITION, not of
    # execution: it sits above CHECK 3 because CHECK 3 exits early on three paths, so anything
    # placed below it is unreachable on most commits.
    #
    # The cell was deleted 2026-09-13 -- a second copy of the entry's own
    # `**Mechanism status:**` field, in the one file every IC record shares, read by no parser
    # on either side and therefore ungated. It drifted on 2 of 23 rows. Positional rather than a
    # scan for the word: two rows carry "mechanism" in their `promotes to` prose legitimately,
    # and a word scan would red on correct rows and be deleted for it.
    refilled = index_rows_with_extra_cells(ledger)
    stale_header = index_header_readvertises_mechanism(ledger)
    if refilled or stale_header:
        rows = [f"{ic} -- Index row carries a fifth cell: `{cell}`" for ic, cell in refilled]
        if stale_header:
            rows.append(f"the Index header still names the column: {stale_header}")
        print(
            "the Index table stores mechanism status again:\n  "
            + "\n  ".join(rows)
            + "\n\n"
            "IF YOU EDITED THE ROSTER -- delete the cell. That text belongs in the entry's own\n"
            "`**Mechanism status:**` under docs/trackers/issue-clusters/, and is read back with\n"
            "`python3 scripts/probe-cluster-census.py`, which renders it beside the verdict.\n"
            "\n"
            "IF YOU DID NOT TOUCH docs/trackers/issue-clusters.md, THIS IS NOT YOUR DEFECT and\n"
            "the fix above is not yours to make. This check reads the INDEX, so a peer's\n"
            "uncommitted migration of that file is invisible to it and you are refused for\n"
            "their in-flight work. Ask them -- the question has an answer they can give\n"
            "('landing now' or 'backed out'), which is why this sends you to a person and not\n"
            "to a file:\n"
            "    python3 scripts/file-provenance.py docs/trackers/issue-clusters.md\n"
            "Do NOT edit their files. Do NOT reach for --no-verify: this same run carries the\n"
            "one-tag and growth checks your own bug files need, so silencing a refusal that is\n"
            "not yours silences two that are.",
            file=sys.stderr,
        )
        _emit_sequence_tail()
        return 1

    # CHECK 2 -- exactly one KNOWN `cluster/<slug>` tag per OPEN bug file.
    #
    # Mirrors `every_open_bug_file_declares_one_known_defect_class`. It was absent here until
    # 2026-09-11 and its absence was the filed defect: this hook enforced a strict SUBSET of the
    # gate it mirrors, so a commit adding a second cluster tag passed the commit path and redded
    # the shared `cargo test` for every other session in the checkout -- the cost lands on people
    # who did not write it and cannot see why it broke.
    # docs/issues/archive/2026-09-09-the-pre-commit-cluster-hook-enforces-a-subset-of-the-gate-it-mirrors.md
    bad_tags = bad_tag_declarations(valid, source)
    if bad_tags:
        print(
            "this commit stages an open bug file with a bad defect-class declaration:\n  "
            + "\n  ".join(bad_tags)
            + "\n\n"
            "Every open bug carries exactly ONE `cluster/<slug>` tag from the closed set in\n"
            f"{LEDGER}. Write it THROUGH THE CATALOG --\n"
            '  doc(action="update", id=..., patch={"tags": ["cluster/<slug>"]})\n'
            "  codescout doc update <id> --tags cluster/<slug>\n"
            "-- because a direct frontmatter edit does not reach the catalog (BL-48), leaving the\n"
            "tag on disk and invisible to every `find`. If no existing slug fits, add one to the\n"
            "ledger rather than forcing a fit: a wrong declaration corrupts the counts that\n"
            "promotion reads.",
            file=sys.stderr,
        )
        _emit_sequence_tail()
        return 1

    # CHECK 5 -- the Index file holds no class sections.
    #
    # Mirrors `the_index_file_holds_no_class_sections`. Ported 2026-09-14; until then it was a
    # Rust-only rule, so a commit that left the section behind passed the commit path and redded
    # `cargo test --test issue_clusters` for every other session sharing the checkout.
    # docs/issues/archive/2026-09-11-three-ledger-rules-are-tested-but-not-enforced-at-commit-time.md
    #
    # Placed above CHECK 3 for the reason CHECK 4 gives: CHECK 3 exits early on three paths, so
    # anything below it is unreachable on most commits.
    #
    # This is the EXPECTED state right after `append_entry` files a new class -- `PendingSection`
    # splices the section into the artifact's own file, and that artifact is the Index. The
    # correct filing path produces this every time, which is why step 2 of the flow needs a
    # mechanism rather than a filer who remembers it (skill-frictions:SKF-22).
    #
    # Reads the Index file ALONE, not `ledger`: the concatenation carries every class file's own
    # `## IC-N —` heading and would refuse every commit in the repo.
    stray_sections = index_class_sections(read(LEDGER, source) or "")
    if stray_sections:
        print(
            f"{LEDGER} is the Index, and this commit leaves class section(s) in it:\n  "
            + "\n  ".join(stray_sections)
            + "\n\n"
            "IF YOU JUST FILED A CLASS -- this is step 2 of the flow, not a mistake in step 1.\n"
            f"Move the section verbatim into {LEDGER_DIR}/IC-N-<slug>.md with tracker\n"
            "frontmatter, leave the Index row behind, and commit both together. Do NOT silence\n"
            "this by declaring `entry_prefix: IC` in the class file -- that raises link_scan's\n"
            "prefix_conflicts instead of fixing anything.\n"
            "\n"
            f"IF YOU DID NOT TOUCH {LEDGER}, THIS IS NOT YOUR DEFECT and the move is not yours\n"
            "to make -- it is someone else's half-filed class. This check reads the INDEX, so a\n"
            "peer's in-flight filing refuses you for work you cannot see. Ask them; the question\n"
            "has an answer they can give ('landing now' or 'backed out'), which is why this\n"
            "sends you to a person rather than to a file:\n"
            f"    python3 scripts/file-provenance.py {LEDGER}\n"
            "Do NOT edit their files. Do NOT reach for --no-verify: this same run carries the\n"
            "one-tag and growth checks your own bug files need, so silencing a refusal that is\n"
            "not yours silences two that are.",
            file=sys.stderr,
        )
        _emit_sequence_tail()
        return 1

    # CHECK 6 -- every declared class has an Index row.
    #
    # Mirrors `every_declared_class_has_an_index_row`. Ported 2026-09-14 from the same bug file as
    # CHECK 5; until then the hook parsed Index rows only for COUNTS and had no count-free row
    # parser, which is the whole reason this one lagged.
    # docs/issues/archive/2026-09-11-three-ledger-rules-are-tested-but-not-enforced-at-commit-time.md
    #
    # Reads the CONCATENATION, unlike CHECK 5 above: `**Slug:**` declarations live in the per-class
    # files and the rows live in the Index, so this rule is the one place that needs both halves.
    #
    # The second arm is a VACUITY guard, not a second rule. If `parse_index_rows` matched nothing
    # -- a renamed column, a reformatted table, slug cells that stopped being backticked -- then
    # CHECK 1's stored-count scan passes green forever over an empty map, which is zero coverage
    # wearing a passing check's clothes (IC-16). The threshold mirrors the Rust side's.
    index_rows = parse_index_rows(ledger, valid)
    missing_rows = missing_index_rows(valid, index_rows)
    if missing_rows or len(index_rows) <= 10:
        if missing_rows:
            head = (
                "these classes declare a `**Slug:**` but have no parseable Index row:\n  "
                + "\n  ".join(missing_rows)
            )
        else:
            head = (
                f"only {len(index_rows)} Index row(s) parsed out of {LEDGER} -- the table format "
                "moved, and CHECK 1 is now asserting emptiness over a table nobody can read, "
                "which it would pass"
            )
        print(
            head + "\n\n"
            "Either the row is absent, or its slug cell stopped being backticked. The row lives\n"
            f"in {LEDGER}; the declaration lives in {LEDGER_DIR}/IC-N-<slug>.md, and filing a\n"
            "class writes BOTH -- this fires when only one of the two landed.\n"
            "\n"
            "ON A SHARED CHECKOUT THERE IS A THIRD CASE AND IT IS NOT YOUR DEFECT: a peer is\n"
            "mid-write. A section and its Index row are two writes, so a slug that is theirs and\n"
            "in flight appears here until the second lands. This message cannot tell the cases\n"
            "apart -- these two can, and the first is the one to run:\n"
            f"    git diff HEAD -- {LEDGER}\n"
            f"    python3 scripts/file-provenance.py {LEDGER}\n"
            "If it is theirs, wait or ask; do NOT add the row for them and do NOT reach for\n"
            "--no-verify, which also silences the one-tag and growth checks your own files need.",
            file=sys.stderr,
        )
        _emit_sequence_tail()
        return 1

    # CHECK 7 -- no `**Mechanism status:**` is a bare verdict.
    #
    # Mirrors `no_mechanism_status_is_a_bare_verdict`, the last of the three this bug file names.
    # docs/issues/archive/2026-09-11-three-ledger-rules-are-tested-but-not-enforced-at-commit-time.md
    #
    # NOT a truth check -- nothing can gate whether a sentence about the code is true. What a gate
    # CAN require is that the sentence carry a route back to the thing it describes, so a reader
    # re-checks it in a minute instead of re-deriving it.
    #
    # Measured 2026-09-01/02: three fields were checked against their code and three were wrong.
    # Two read `none yet` over a mechanism that had already shipped, one of them over a mechanism
    # that explicitly REFUSED the remedy the field proposed. A bare `none yet` reads as an
    # established absence and was, in every case examined, an unexamined one.
    bare_verdicts = [
        f"{ic} -- `{body}`" for ic, body in mechanism_statuses(ledger) if is_unbasised(body)
    ]
    if bare_verdicts:
        print(
            "a `**Mechanism status:**` states a verdict and gives no basis:\n  "
            + "\n  ".join(bare_verdicts)
            + "\n\n"
            "A bare verdict cannot be re-checked, so it is read as established. Add whatever\n"
            "makes it checkable in one minute -- not a paragraph:\n"
            "  `shipped`/`partial`/`designed` -> name WHERE (a path, a symbol, a SHA, a tool,\n"
            "                                    an ADR)\n"
            "  `none yet`                     -> say when it was last checked against the code,\n"
            "                                    or say that it has not been\n"
            "\n"
            '"Not checked" is a legitimate and useful answer. It is the difference between\n'
            "nobody having looked and there being nothing to find, and this check exists because\n"
            "those two had the same nine characters.\n"
            "\n"
            f"IF THE ENTRY NAMED ABOVE IS NOT YOURS, THIS IS NOT YOUR DEFECT. The field lives in\n"
            f"{LEDGER_DIR}/<entry>.md and only its author knows what was checked and when;\n"
            "writing a basis you did not verify is worse than the bare verdict, because it reads\n"
            "as established for the same reason. Ask them:\n"
            f"    python3 scripts/file-provenance.py {LEDGER_DIR}/\n"
            "Do NOT reach for --no-verify: this same run carries the one-tag and growth checks\n"
            "your own bug files need.",
            file=sys.stderr,
        )
        _emit_sequence_tail()
        return 1

    # CHECK 3 -- a defect class that GAINS a member must say something about it.
    #
    # This replaces the forcing function check 1 used to be, and the replacement is the point.
    # The old count gate made a ledger edit MANDATORY, and that is why per-member derivations
    # exist at all: authors wrote them while satisfying the refusal. Measured on `1b92a7de` --
    # one bug filing added 1,508 characters of hand-authored, non-derivable prose across the
    # three lines it had to touch for the number. Remove the number and nothing asks; a
    # `**Members:**` with 22 members and no derivations reads identically to one with full
    # derivations, to every query. Raised by codescout-17 (sessionId 9716a130), which measured
    # its own commit rather than accepting the premise it was handed.
    #
    # "Did the line change?" was the first form and is REJECTED: a trailing space satisfies it.
    # That is `cluster/assertion-satisfiable-by-accident`, and it would be a real regression --
    # the count gate was not satisfiable by accident, because the number had to equal a derived
    # value. So the assertion is that the field NAMES the new member, in the ledger's own
    # `+1: `<dateless-slug>`` shape. Whitespace cannot satisfy that, and a filer who names it
    # differently gets a refusal saying which stem was looked for, recoverable by reading.
    # Measured 2026-09-02: 7 of 22 classes already cite members this way -- a boundary, not a
    # universal convention -- but this is PROSPECTIVE, firing only on a class that gains a
    # member, so the other 15 are untouched until they do.
    head_ledger = read_ledger("head")
    if head_ledger is None:
        return 0
    before = actual_counts(valid_slugs(head_ledger), "head")

    # Derived from stem identity, NOT from `actual` vs `before` counts: a count comparison
    # cannot tell an archive move (path changed, stem unchanged) from a new member, because the
    # index population and HEAD's population are different path sets.
    new_stems = added_member_stems(source)
    gained = sorted(new_stems)
    if not gained:
        return 0

    members = members_fields(ledger, valid)
    head_members = members_fields(head_ledger, valid)

    undocumented = []
    for slug in gained:
        line = members.get(slug, "")
        changed = line != head_members.get(slug, "")
        stems = new_stems.get(slug, set())
        if changed and (not stems or any(st in line for st in stems)):
            continue
        undocumented.append((slug, sorted(stems), class_file_for(slug, source)))
    if not undocumented:
        return 0

    return emit_growth_refusal(undocumented)


if __name__ == "__main__":
    sys.exit(main())
