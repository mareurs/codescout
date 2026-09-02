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
is closed by a MECHANISM, not by vigilance: `the_hook_script_agrees_with_this_gate` runs this
file with `--source=worktree --json` and fails if the two derivations disagree. Change one and
the test reddens until you change the other.
"""

from __future__ import annotations

import json
import pathlib
import subprocess
import sys

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


def _git(*args: str) -> str:
    r = subprocess.run(["git", *args], capture_output=True, text=True)
    if r.returncode != 0:
        raise SystemExit(f"git {' '.join(args)} failed: {r.stderr.strip()}")
    return r.stdout


def class_files(source: str) -> list:
    """Every per-class file, read from the SAME source as the ledger text.

    Source-matching is load-bearing and is the reason this is not a bare glob: the hook's
    whole point is comparing what is STAGED against what is on disk, and a disk glob would
    silently mix the two -- reporting a class file's worktree content against an index-sourced
    ledger. `head` and `index` therefore go through git, and only `worktree` touches disk.

    Tracked-only for the git sources, matching the rest of this hook: an untracked class file
    is invisible here, so a local green defers rather than clears. That is the documented
    posture (see the module header), not an oversight.
    """
    if source == "head":
        out = _git("ls-tree", "-r", "--name-only", "HEAD", LEDGER_DIR)
    elif source == "index":
        out = _git("ls-files", LEDGER_DIR)
    else:
        d = pathlib.Path(LEDGER_DIR)
        return sorted(str(p) for p in d.glob("*.md")) if d.is_dir() else []
    return sorted(p for p in out.splitlines() if p.endswith(".md"))


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
    the WORKTREE figure -- the mode `the_hook_script_agrees_with_this_gate` runs -- while the
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
    """Mirrors `tracked_all_bug_files` -- the INDEX population, so untracked files are excluded."""
    return [
        p
        for p in _git("ls-files", "docs/issues").splitlines()
        if p.endswith(".md") and not p.endswith("_TEMPLATE.md")
    ]


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
    """
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


def main() -> int:
    source = "index"
    as_json = False
    for arg in sys.argv[1:]:
        if arg.startswith("--source="):
            source = arg.split("=", 1)[1]
        elif arg == "--json":
            as_json = True
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
        elif arg == "--fixture-tags":
            # Pure over stdin, so `the_hook_script_agrees_on_both_yaml_tag_styles` can feed
            # both YAML forms directly. The live corpus cannot exercise the inline arm:
            # measured 2026-09-01, ZERO bug files carry a `cluster/` tag in flow style, so a
            # corpus-driven agreement check leaves that branch untested however green it is.
            print(json.dumps(cluster_tags(frontmatter(sys.stdin.read()) or "")))
            return 0
    if source not in ("index", "worktree", "head"):
        raise SystemExit(f"--source must be index|worktree|head, got {source!r}")

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

    # CHECK 2 -- a class that GAINS a member must say something about it.
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
        undocumented.append((slug, sorted(stems)))
    if not undocumented:
        return 0

    print(
        "a class gained a member and its `**Members:**` does not name it:\n  "
        + "\n  ".join(
            f"cluster/{slug} -- expected the field to change and to contain one of: "
            + (", ".join(f"`{x}`" for x in stems) or "(any change)")
            for slug, stems in undocumented
        )
        + "\n\n"
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


if __name__ == "__main__":
    sys.exit(main())
