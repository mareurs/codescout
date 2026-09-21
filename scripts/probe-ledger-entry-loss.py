#!/usr/bin/env python3
r"""Ledger entry-loss probe — ids ALLOCATED against entries PRESENT, per ledger/prefix pair.

Ships the derivation behind `architecture-boundary-session-log:F-6` so a reader
re-checks it instead of re-deriving it under a counting rule of their own choosing
(CLAUDE.md § *Observer Blindness*, position 3). The census that produced F-6 died in a
subagent's scratchpad; this is the instrument, not the number.

THE HEADLINE BLIND SPOT — read this before reading any row
===========================================================
The predicate is `entry_high_water_<PREFIX>` (committed frontmatter, monotonic) minus
the count of live entries carrying that prefix. That mark has **exactly one production
writer**:

    src/librarian/frontmatter.rs:387   upsert_int_line
      <- src/librarian/catalog/augmentation.rs:1507   inside allocate_entry_id
         <- src/librarian/tools/append_entry.rs:183   `if a.entry_collection.is_none()`
                                                       -- the PROSE branch, only

`references(upsert_int_line)` returns 8 sites: the definition, 6 test call sites, and
that one production call. The **params** allocator is a different function
(`augmentation.rs:770-771`, `params_next.max(body_max + 1)`) and writes neither the mark
nor a reservation row into frontmatter.

So this probe is **structurally blind to params-backed ledgers** — which is the
population anyone reaching for a loss probe will care about. The proof that it is blind
rather than merely weak is the incident that motivated the ADR this was built to test
(`d66562ed420391a8`): `tool-usage-patterns`' T ledger went from 19 rows to 1 on
2026-08-16, and here it computes as a **negative** gap. A negative gap is reported as
`mark-below-live`, i.e. as a stale instrument — never as loss. The probe reports that
ledger clean.

**An all-zero gap column is therefore NOT evidence that nothing was lost.** It is
evidence about the prose half only. The script says so in its own header on every run;
do not quote a row without it.

WHAT EACH PREDICATE LITERALLY COUNTS
====================================
* "a ledger/prefix PAIR"
    Primary source, `entry_prefix` — a (file, PREFIX) where the file's committed
    frontmatter declares PREFIX through `entry_prefix:` (scalar, flow `[F, W]`, or block
    sequence), unioned with every `entry_high_water_<PREFIX>` key present. This mirrors
    `declared_prefixes_from_frontmatter` (`augmentation.rs:1209`), including its
    acceptance of prefixes LONGER than three characters, and its rejection of a YAML
    `null` value.
    Secondary source, `sidecar+body` — a params-backed ledger that declares no
    `entry_prefix` at all. Its namespace is recovered from the body's own
    allocator-shape claims, because nothing in git names it. A params ledger whose body
    claims nothing is UNENUMERABLE from git and is counted as an artifact with zero
    pairs; the header prints that list rather than letting it vanish.

* "MARK" = the integer value of `entry_high_water_<PREFIX>` in committed frontmatter.
    Absent => the pair is `unmeasurable (no mark)`. Absent and params-backed is the
    normal, expected state, not a defect.

* "LIVE" — TWO definitions, reported side by side because they disagree and the
    disagreement is real. Never merged into one number.
    - `def_re` — what `link_scan` will treat as a DEFINITION of the token, and therefore
      what a citation of it can resolve to: a fence-aware ATX heading whose raw text
      matches `^\s*([A-Z]{1,3}-\d+)\s+[—–-]\s+` (`link_scan/extract.rs:319-322`).
      `[A-Z]{1,3}` is literal: a FOUR-letter prefix such as `DCTX` is invisible to this
      column by construction, always, and reads as total loss
      (filed: `219027f500266ee4`). A heading with no dash-and-title
      (`## A-9 Addendum`) defines nothing. A bold- or link-wrapped token
      (`## **F-3** — a`) does not match either, because `def_re` is applied to the
      heading's raw text.
    - `alloc` — what the ALLOCATOR will refuse to reissue: `body_claimed_indices`
      (`augmentation.rs:2030-2040`), a line-anchored heading OR the leading cell of an
      index-table row, optional `` ` ``/`*`/`[` wrapping, no dash required, any prefix
      length. This column is NOT fence-aware, matching the production regex.

    They disagree by design. `issue-clusters.md`'s IC is the worked case: its entries are
    each defined in a companion file under `docs/trackers/issue-clusters/`, so `def_re`
    sees almost none of them while the body's index table gives `alloc` all of them. A
    large `def_re` gap beside a zero `alloc` gap means "the definitions live elsewhere",
    not "the entries are gone".

* "GAP" = MARK - LIVE, per definition. A negative value is not a gap; it reclassifies
    the pair as `mark-below-live`. That reclassification fires when the mark is below live
    under AT LEAST ONE definition — a disjunction over the two predicates, never a merge
    of the two counts. A mark below any honest live count cannot be a high-water record,
    and the safe direction here is to flag the instrument rather than publish a gap.

* "BACKING" is a property of the ARTIFACT, not of the namespace, and it over-attributes.
    `params` means the artifact's committed sidecar declares an `entry_collection`, so at
    least one of its namespaces routes through the params allocator and writes no mark.
    An artifact can carry both — `docs/research/README.md` has a params `entries`
    collection AND a prose `C-N` namespace whose ids a caller would allocate through the
    prose branch. Such a pair is labelled `params` here and its mark would in fact be
    written. Correcting this needs per-namespace routing information, which exists
    nowhere in git.

* "ARCHIVE-EXPLAINED" = of the missing indices, how many are DEFINED (allocator shape) in
    an archive companion of this ledger: a file at the same rev whose path carries an
    `archive` segment AND whose basename starts with the ledger's stem, or which sits in a
    directory named for that stem. `append_entry`'s own `compaction_note`
    (`append_entry.rs:309`) states this case is expected and "neither is drift", so it is
    reported in its OWN column and never merged with the unexplained count.
    The companion scoping is load-bearing and deliberately narrow: `F` and `W` are shared
    by fourteen session logs here, so an unscoped archive search would "explain" one
    ledger's missing `F-3` with another ledger's archived `F-3`. Ids also found somewhere
    else in the archive corpus are flagged `?<n>` — a hint to look, never an explanation.

* "the corpus" = `git ls-tree -r <rev>` at the named tree, read through `git cat-file`.
    The WORKTREE is deliberately not read: a peer sweep in flight makes worktree and HEAD
    disagree, so two sessions reading correctly at the same instant still differ
    (CLAUDE.md § *Testing Discipline*). The header prints the tree SHA, the instant, and
    how many tracked files the worktree currently differs on.

THE FENCE RULE, AND ITS SELF-CHECK
==================================
`def_re`'s heading scan must skip fenced code. A naive ``` toggle desyncs on this repo's
own `docs/trackers/bug-fix-session-log.md`, where a four-backtick run wraps a three-
backtick block: the toggle closes the outer fence early and every heading after it turns
into a phantom definition or vanishes. The correct rule is CommonMark's, ported from
`src/util/markdown_fence.rs` -- a closer must match the opening CHARACTER, be at least as
LONG, and be followed by nothing but whitespace; a backtick opener's info string may not
contain a backtick.

`--self-check` (on by default) runs BOTH rules over the whole ledger corpus and reports
where they disagree. A regression that quietly turns the CommonMark rule back into a
toggle shows up as `FENCE SELF-CHECK: agree everywhere`, which is reported as a FAILURE,
not as silence -- the fixture is in-tree, so agreement means the instrument broke.

Usage:
    python3 scripts/probe-ledger-entry-loss.py [--rev REV] [--all] [--json]
                                               [--no-self-check]
"""

from __future__ import annotations

import argparse
import datetime as _dt
import json
import re
import subprocess
import sys
from pathlib import Path

# --------------------------------------------------------------------------------------
# git plumbing
# --------------------------------------------------------------------------------------


def repo_root() -> str:
    return subprocess.run(
        ["git", "rev-parse", "--show-toplevel"],
        capture_output=True,
        text=True,
        check=True,
    ).stdout.strip()


class Tree:
    """Every blob at one rev, read once through `git cat-file --batch`.

    Reading at a REV rather than from the worktree is the point, not an optimisation:
    the prior census found 2 tracked files dirty, and a sweep in flight makes the
    worktree and HEAD disagree for readers who are each correct.
    """

    def __init__(self, root: str, rev: str):
        self.root = root
        self.rev = rev
        self.commit = self._git("rev-parse", rev).strip()
        self.tree = self._git("rev-parse", f"{rev}^{{tree}}").strip()
        self.paths = [
            p for p in self._git("ls-tree", "-r", rev, "--name-only").splitlines() if p
        ]
        self._cache: dict[str, str | None] = {}

    def _git(self, *args: str) -> str:
        return subprocess.run(
            ["git", "-C", self.root, *args], capture_output=True, text=True, check=True
        ).stdout

    def read(self, path: str) -> str | None:
        if path in self._cache:
            return self._cache[path]
        proc = subprocess.run(
            ["git", "-C", self.root, "cat-file", "blob", f"{self.rev}:{path}"],
            capture_output=True,
        )
        out = (
            proc.stdout.decode("utf-8", "replace") if proc.returncode == 0 else None
        )
        self._cache[path] = out
        return out

    def dirty_tracked(self) -> list[str]:
        raw = self._git("status", "--porcelain", "--untracked-files=no")
        return [ln[3:] for ln in raw.splitlines() if ln.strip()]


# --------------------------------------------------------------------------------------
# frontmatter
# --------------------------------------------------------------------------------------

ENTRY_PREFIX_KEY = "entry_prefix"
HIGH_WATER_PREFIX = "entry_high_water_"


def split_frontmatter(text: str) -> tuple[str | None, str]:
    """(frontmatter, body). Same delimiter scan `frontmatter::parse` performs.

    The body is what both live-count predicates run over, so a prefix mentioned in
    frontmatter cannot be mistaken for an entry — matching the comment at
    `augmentation.rs:1441`.
    """
    if not (text.startswith("---\n") or text.startswith("---\r\n")):
        return None, text
    nl = text.index("\n") + 1
    rest = text[nl:]
    lines = rest.split("\n")
    for i, ln in enumerate(lines):
        if ln.rstrip("\r") == "---":
            return "\n".join(lines[:i]), "\n".join(lines[i + 1 :])
    return None, text


def frontmatter_keys(fm: str) -> dict[str, object]:
    """Top-level `key: value` pairs, with block sequences collected as lists.

    A deliberate subset of YAML: this reads exactly the two shapes the allocator's own
    two readers agree on (`declared_prefixes_from_frontmatter` and
    `librarian_guard::declared_entry_prefixes`) — scalar, inline flow, and block
    sequence at any indentation. Nested mappings are not descended into, because no
    key this probe reads is ever nested.
    """
    out: dict[str, object] = {}
    lines = fm.split("\n")
    i = 0
    while i < len(lines):
        ln = lines[i].rstrip("\r")
        m = re.match(r"^([A-Za-z0-9_.\-]+):(.*)$", ln)
        if not m:
            i += 1
            continue
        key, val = m.group(1), m.group(2).strip()
        if val == "":
            items: list[str] = []
            j = i + 1
            while j < len(lines):
                s = lines[j].rstrip("\r").lstrip()
                if s.startswith("- "):
                    items.append(s[2:].strip())
                    j += 1
                else:
                    break
            out[key] = items if items else ""
            i = j
        else:
            out[key] = val
            i += 1
    return out


def _clean_prefix(raw: str) -> str | None:
    p = raw.strip().strip("'\"").strip()
    # `declared_prefixes_from_frontmatter` accepts ANY non-empty string, which is why a
    # four-letter prefix allocates. It parses real YAML, so a bare `null` arrives as
    # Value::Null and yields no prefix; this reader has to reject the token by hand.
    if not p or p in {"null", "~"}:
        return None
    return p


def declared_prefixes(fmkeys: dict[str, object]) -> list[str]:
    v = fmkeys.get(ENTRY_PREFIX_KEY)
    if isinstance(v, list):
        return [p for p in (_clean_prefix(x) for x in v) if p]
    if isinstance(v, str) and v:
        s = v.strip()
        if s.startswith("[") and s.endswith("]"):
            return [p for p in (_clean_prefix(x) for x in s[1:-1].split(",")) if p]
        p = _clean_prefix(s)
        return [p] if p else []
    return []


def high_water(fmkeys: dict[str, object]) -> dict[str, int]:
    out: dict[str, int] = {}
    for k, v in fmkeys.items():
        if not k.startswith(HIGH_WATER_PREFIX):
            continue
        if not isinstance(v, str):
            continue
        # Accepts a quoted value: the allocator does too, on the grounds that a
        # hand-written mark should be honoured rather than silently read as absent —
        # reading it as absent is exactly the reissue it exists to prevent.
        s = v.strip().strip("'\"")
        try:
            out[k[len(HIGH_WATER_PREFIX) :]] = int(s)
        except ValueError:
            continue
    return out


# --------------------------------------------------------------------------------------
# fence tracking — a port of src/util/markdown_fence.rs
# --------------------------------------------------------------------------------------


class FenceState:
    """CommonMark-correct fenced-code tracking, ported from `src/util/markdown_fence.rs`.

    Four rules, and the last two are the ones a toggle gets wrong:
      * a closer matches the opening CHARACTER (backtick never closes tilde),
      * a closer is at least as LONG as the opener,
      * a closer is followed by nothing but whitespace,
      * a backtick opener's info string may not contain a backtick.

    Indentation is the caller's business, exactly as in the Rust: `feed` reads the run
    from byte 0 of whatever it is handed and never trims. `headings::parse` passes
    `trim_start()`ed lines, so `heading_scan` below does the same.
    """

    __slots__ = ("open",)

    def __init__(self) -> None:
        self.open: tuple[str, int] | None = None

    def feed(self, line: str) -> bool:
        if not line or line[0] not in ("`", "~"):
            return False
        ch = line[0]
        run = len(line) - len(line.lstrip(ch))
        rest = line[run:]
        if self.open is not None:
            och, orun = self.open
            if ch == och and run >= orun and rest.strip() == "":
                self.open = None
                return True
            return False
        if run < 3:
            return False
        if ch == "`" and "`" in rest:
            return False
        self.open = (ch, run)
        return True

    def in_fence(self) -> bool:
        return self.open is not None


class NaiveToggle:
    """The WRONG rule, kept so the self-check can measure the difference.

    This is what every line-oriented scanner in this tree used before
    `markdown_fence.rs`: flip on any line starting with three backticks. Retained
    deliberately and annotated as inert — it must never be used for a reported count.
    """

    __slots__ = ("_in",)

    def __init__(self) -> None:
        self._in = False

    def feed(self, line: str) -> bool:
        if line.startswith("```"):
            self._in = not self._in
            return True
        return False

    def in_fence(self) -> bool:
        return self._in


def heading_scan(body: str, fence_factory=FenceState) -> list[tuple[int, str]]:
    """(level, raw_text) for every ATX heading outside a fence.

    A port of `librarian::preview::headings::parse` — the same parser `link_scan`'s
    definition pass uses, so the two cannot disagree about what a heading is. `text` is
    the raw remainder after the `#` run, trimmed and NOT inline-parsed, which is what
    `def_re` is applied to at `extract.rs:438`.
    """
    out: list[tuple[int, str]] = []
    fence = fence_factory()
    for line in body.split("\n"):
        trimmed = line.lstrip()
        if fence.feed(trimmed):
            continue
        if fence.in_fence():
            continue
        level = len(trimmed) - len(trimmed.lstrip("#"))
        if level == 0 or level > 6:
            continue
        if len(trimmed) <= level or trimmed[level] != " ":
            continue
        out.append((level, trimmed[level + 1 :].strip()))
    return out


# --------------------------------------------------------------------------------------
# the two live-count predicates
# --------------------------------------------------------------------------------------

# link_scan/extract.rs:321, byte for byte. `[A-Z]{1,3}` is the four-letter blind spot.
DEF_RE = re.compile(r"^\s*([A-Z]{1,3}-\d+)\s+[—–-]\s+")


def def_re_indices(body: str, prefix: str, fence_factory=FenceState) -> set[int]:
    """Indices `link_scan` would treat as DEFINED — i.e. that a citation can resolve to."""
    out: set[int] = set()
    if not re.fullmatch(r"[A-Z]{1,3}", prefix):
        # Not a shortcoming of this ledger: `def_re` cannot express the token at all.
        return out
    for _level, text in heading_scan(body, fence_factory):
        m = DEF_RE.match(text)
        if not m:
            continue
        tok = m.group(1)
        p, _, n = tok.rpartition("-")
        if p == prefix:
            out.add(int(n))
    return out


def alloc_indices(body: str, prefix: str) -> set[int]:
    """`body_claimed_indices` (`augmentation.rs:2030-2040`) — what the allocator sees.

    Deliberately NOT fence-aware, because the production regex is not: a `## F-3 — x`
    inside a fenced block DOES block the allocator from reissuing F-3. Making this
    column fence-aware would make it stop describing the allocator, which is its only
    job.
    """
    esc = re.escape(prefix)
    rx = re.compile(r"(?m)^(?:#{1,6}[ \t]+|\|[ \t]*)[`*\[]*" + esc + r"-(\d+)\b")
    return {int(m.group(1)) for m in rx.finditer(body)}


# --------------------------------------------------------------------------------------
# archive control
# --------------------------------------------------------------------------------------


def archive_paths(tree: Tree) -> list[str]:
    return [
        p
        for p in tree.paths
        if p.endswith(".md") and "archive" in Path(p).parts[:-1]
    ]


def companion_archives(ledger_path: str, arch: list[str]) -> list[str]:
    """Archive files that could hold THIS ledger's compacted entries.

    Scoped by stem on purpose. `F` and `W` are shared by fourteen session logs in this
    repo, so an unscoped search would let one ledger's archived `F-3` "explain" a
    different ledger's missing `F-3` — a confident wrong answer, not a near miss.
    """
    stem = Path(ledger_path).stem
    out = []
    for p in arch:
        pp = Path(p)
        if pp.stem.startswith(stem) or stem in pp.parts[:-1]:
            out.append(p)
    return out


# --------------------------------------------------------------------------------------
# enumeration
# --------------------------------------------------------------------------------------


class Pair:
    __slots__ = (
        "path",
        "prefix",
        "source",
        "backing",
        "mark",
        "live_def",
        "live_alloc",
        "missing_def",
        "missing_alloc",
        "arch_def",
        "arch_alloc",
        "elsewhere_def",
        "elsewhere_alloc",
        "cls",
    )

    def as_dict(self) -> dict:
        return {k: getattr(self, k) for k in self.__slots__}


def collect(tree: Tree) -> tuple[list[Pair], list[str], dict]:
    md = [p for p in tree.paths if p.endswith(".md")]

    # Committed augmentation sidecars are the only git-readable statement of which
    # ledgers are params-backed. The catalog holds the authoritative copy and is
    # machine-local and gitignored, so a ledger whose shape was never exported reads
    # here as prose-backed. That direction of error is the probe's, not the repo's:
    # see `docs/conventions/cross-machine-catalog-resume.md`.
    sidecar_collection: dict[str, str] = {}
    for p in tree.paths:
        if p.startswith("docs/augmentations/") and p.endswith(".yaml"):
            txt = tree.read(p) or ""
            m = re.search(r"(?m)^entry_collection:\s*(\S+)\s*$", txt)
            if m:
                sidecar_collection[p] = m.group(1)

    pairs: list[Pair] = []
    unenumerable: list[str] = []
    artifacts: set[str] = set()
    arch = archive_paths(tree)

    for path in md:
        text = tree.read(path)
        if text is None:
            continue
        if ENTRY_PREFIX_KEY not in text and "expects_augmentation" not in text:
            continue
        fm, body = split_frontmatter(text)
        if fm is None:
            continue
        keys = frontmatter_keys(fm)
        prefixes = declared_prefixes(keys)
        hw = high_water(keys)
        sidecar = keys.get("expects_augmentation")
        sidecar = sidecar.strip().strip("'\"") if isinstance(sidecar, str) else None
        params_backed = bool(sidecar and sidecar in sidecar_collection)

        # A `entry_high_water_X` with no matching declaration is still a pair: the mark
        # is the allocated-id record whatever the declaration now says.
        namespaces = list(dict.fromkeys([*prefixes, *hw.keys()]))
        source = "entry_prefix"

        if not namespaces and params_backed:
            # No committed declaration anywhere. The namespace exists only in the
            # catalog, so recover it from the body's own allocator-shape claims — the
            # one surface that is both in git and in the allocator's input.
            found = re.findall(
                r"(?m)^(?:#{1,6}[ \t]+|\|[ \t]*)[`*\[]*([A-Z]{1,6})-\d+\b", body
            )
            namespaces = sorted(set(found))
            source = "sidecar+body"
            if not namespaces:
                unenumerable.append(path)
                artifacts.add(path)
                continue

        if not namespaces:
            continue

        artifacts.add(path)
        comp = companion_archives(path, arch)
        comp_bodies = []
        for c in comp:
            ct = tree.read(c)
            if ct is not None:
                comp_bodies.append(split_frontmatter(ct)[1])
        other_bodies = None  # lazily built, only when something is unexplained

        for pfx in namespaces:
            pr = Pair()
            pr.path = path
            pr.prefix = pfx
            pr.source = source
            pr.backing = "params" if params_backed else "prose"
            pr.mark = hw.get(pfx)
            d = def_re_indices(body, pfx)
            a = alloc_indices(body, pfx)
            pr.live_def = len(d)
            pr.live_alloc = len(a)

            if pr.mark is None:
                pr.cls = "unmeasurable (no mark)"
            elif pr.mark < max(pr.live_def, pr.live_alloc):
                # Disjunction over the two predicates, not a merge of their counts: a
                # mark below EITHER honest live count cannot be a high-water record.
                # Flagging the instrument is the safe direction — the alternative is
                # publishing a negative number as though it were a gap.
                pr.cls = "mark-below-live (instrument stale)"
            else:
                pr.cls = "measurable"

            if pr.mark is None:
                pr.missing_def = pr.missing_alloc = []
                pr.arch_def = pr.arch_alloc = 0
                pr.elsewhere_def = pr.elsewhere_alloc = 0
                pairs.append(pr)
                continue

            full = set(range(1, pr.mark + 1))
            pr.missing_def = sorted(full - d)
            pr.missing_alloc = sorted(full - a)

            # One pass over the companions, not one per missing id.
            comp_claimed: set[int] = set()
            for b in comp_bodies:
                comp_claimed |= alloc_indices(b, pfx)

            def explained(missing: list[int]) -> tuple[int, list[int]]:
                hits = [n for n in missing if n in comp_claimed]
                left = [n for n in missing if n not in comp_claimed]
                return len(hits), left

            pr.arch_def, left_def = explained(pr.missing_def)
            pr.arch_alloc, left_alloc = explained(pr.missing_alloc)
            pr.missing_def = left_def
            pr.missing_alloc = left_alloc

            # A hit anywhere else in the archive corpus is a HINT, never an explanation:
            # prefix namespaces collide across ledgers.
            pr.elsewhere_def = pr.elsewhere_alloc = 0
            if left_def or left_alloc:
                if other_bodies is None:
                    other_bodies = []
                    for c in arch:
                        if c in comp:
                            continue
                        ct = tree.read(c)
                        if ct is not None:
                            other_bodies.append(split_frontmatter(ct)[1])
                elsewhere = set()
                for b in other_bodies:
                    elsewhere |= alloc_indices(b, pfx)
                pr.elsewhere_def = len(set(left_def) & elsewhere)
                pr.elsewhere_alloc = len(set(left_alloc) & elsewhere)

            pairs.append(pr)

    meta = {
        "artifacts": len(artifacts),
        "sidecars_with_entry_collection": len(sidecar_collection),
    }
    return pairs, unenumerable, meta


# --------------------------------------------------------------------------------------
# fence self-check
# --------------------------------------------------------------------------------------


def fence_self_check(tree: Tree, pairs: list[Pair]) -> dict:
    """Run BOTH fence rules over the ledger corpus and report where they disagree.

    Reported as a FAILURE when they agree everywhere. The disagreeing fixture is
    in-tree (`docs/trackers/bug-fix-session-log.md` carries a four-backtick run wrapping
    a three-backtick block), so agreement means either the fixture went away or the
    CommonMark port regressed into a toggle — and a silent regression is precisely what
    this exists to make visible. The fixture's line is located rather than hardcoded: it
    was at :2909 when the trap was first reported and has since moved.

    Not vacuous by assertion — by observed red. Rebinding `FenceState` to `NaiveToggle`
    in-process (nothing written to the shared tree) flips this from `pass` to `FAIL`,
    which is the exact regression it claims to catch.
    """
    rows = []
    for pr in pairs:
        text = tree.read(pr.path)
        if text is None:
            continue
        body = split_frontmatter(text)[1]
        correct = len(def_re_indices(body, pr.prefix, FenceState))
        naive = len(def_re_indices(body, pr.prefix, NaiveToggle))
        if correct != naive:
            rows.append(
                {
                    "path": pr.path,
                    "prefix": pr.prefix,
                    "commonmark": correct,
                    "naive_toggle": naive,
                    "mark": pr.mark,
                    "phantom_gap": None if pr.mark is None else pr.mark - naive,
                    "real_gap": None if pr.mark is None else pr.mark - correct,
                }
            )

    # Locate the four-backtick fixture wherever it now lives.
    fixture = None
    for p in tree.paths:
        if not p.endswith(".md"):
            continue
        t = tree.read(p)
        if t is None or "````" not in t:
            continue
        for i, ln in enumerate(t.split("\n"), 1):
            if ln.startswith("````") and "```" in ln[4:]:
                fixture = {"path": p, "line": i, "text": ln.strip()}
                break
        if fixture:
            break

    balanced = True
    if fixture:
        st = FenceState()
        for ln in (tree.read(fixture["path"]) or "").split("\n"):
            st.feed(ln.lstrip())
        balanced = not st.in_fence()

    return {
        "status": "pass" if rows else "FAIL",
        "disagreements": rows,
        "fixture": fixture,
        "fixture_fences_balanced_under_commonmark": balanced,
    }


# --------------------------------------------------------------------------------------
# rendering
# --------------------------------------------------------------------------------------


def fmt_ids(ids: list[int], prefix: str, cap: int = 12) -> str:
    if not ids:
        return "—"
    shown = [f"{prefix}-{n}" for n in ids[:cap]]
    if len(ids) > cap:
        shown.append(f"… (+{len(ids) - cap})")
    return " ".join(shown)


def render(tree: Tree, pairs, unenumerable, meta, selfcheck, show_all: bool) -> None:
    now = _dt.datetime.now(_dt.timezone.utc).strftime("%Y-%m-%d %H:%M:%S UTC")
    dirty = tree.dirty_tracked()

    print("probe-ledger-entry-loss — ids ALLOCATED vs entries PRESENT, per ledger/prefix pair")
    print()
    print(f"  rev              {tree.rev}  ->  commit {tree.commit[:12]}")
    print(f"  tree             {tree.tree}")
    print(f"  run at           {now}")
    print(
        f"  worktree         {len(dirty)} tracked file(s) differ from this tree "
        f"— every count below is read from the TREE, not the worktree"
    )
    print()
    print("  WHAT A ZERO GAP DOES NOT MEAN")
    print("    `entry_high_water_<PREFIX>` has one production writer (frontmatter.rs:387 via")
    print("    augmentation.rs:1507), reached only from append_entry.rs:183's PROSE branch. The")
    print("    params allocator (augmentation.rs:770-771) writes no mark. This probe is")
    print("    STRUCTURALLY BLIND to params-backed ledgers. The incident that motivated ADR")
    print("    d66562ed420391a8 — tool-usage-patterns' T ledger dropping 19 rows to 1 on")
    print("    2026-08-16 — computes here as a NEGATIVE gap and is reported clean. An all-zero")
    print("    gap column is evidence about the prose half only.")
    print()

    by_cls: dict[str, int] = {}
    by_back: dict[str, int] = {}
    by_src: dict[str, int] = {}
    for p in pairs:
        by_cls[p.cls] = by_cls.get(p.cls, 0) + 1
        by_back[p.backing] = by_back.get(p.backing, 0) + 1
        by_src[p.source] = by_src.get(p.source, 0) + 1

    print("POPULATION  [unit: ledger/prefix pairs; derived, not stored]")
    print(f"  {len(pairs)} pairs across {meta['artifacts']} artifacts")
    print(
        "  by source:          "
        + " · ".join(f"{k} {v}" for k, v in sorted(by_src.items()))
    )
    print(
        "  by backing:         "
        + " · ".join(f"{k} {v}" for k, v in sorted(by_back.items()))
    )
    print(
        "  by classification:  "
        + " · ".join(f"{k} {v}" for k, v in sorted(by_cls.items()))
    )
    if unenumerable:
        print(
            "  params-backed and UNENUMERABLE from git (no entry_prefix, body claims no token):"
        )
        for p in unenumerable:
            print(f"      {p}")
    print()

    print("FENCE SELF-CHECK  [CommonMark rule vs the naive ``` toggle, over the ledger corpus]")
    if selfcheck["fixture"]:
        f = selfcheck["fixture"]
        print(f"  four-backtick fixture located at {f['path']}:{f['line']}  —  {f['text'][:60]}")
        print(
            f"  that file's fences balance under the CommonMark rule: "
            f"{selfcheck['fixture_fences_balanced_under_commonmark']}"
        )
    else:
        print("  four-backtick fixture: NOT FOUND in this tree")
    if selfcheck["status"] == "pass":
        print(
            f"  status: pass — the two rules disagree on {len(selfcheck['disagreements'])} pair(s):"
        )
        for r in selfcheck["disagreements"]:
            print(
                f"      {r['path']}  {r['prefix']}: commonmark {r['commonmark']} vs "
                f"naive {r['naive_toggle']}  (naive would report gap "
                f"{r['phantom_gap']}, real gap {r['real_gap']})"
            )
    else:
        print(
            "  status: FAIL — the two rules agree EVERYWHERE. Either the four-backtick fixture"
        )
        print(
            "          left the tree, or the CommonMark port regressed into a toggle. Do not"
        )
        print("          trust the def_re column until this is resolved.")
    print()

    print("PAIRS  [gap = mark - live; two live definitions, never merged]")
    hdr = (
        f"  {'ledger':<52} {'pfx':<5} {'back':<6} {'mark':>5} "
        f"{'live_def':>8} {'gap_def':>7} {'live_all':>8} {'gap_all':>7}  class"
    )
    print(hdr)
    print("  " + "-" * (len(hdr) - 2))
    rows = pairs if show_all else [p for p in pairs if _nonzero(p)]
    for p in sorted(rows, key=lambda x: (x.path, x.prefix)):
        gd = "—" if p.mark is None else p.mark - p.live_def
        ga = "—" if p.mark is None else p.mark - p.live_alloc
        print(
            f"  {p.path[-52:]:<52} {p.prefix:<5} {p.backing:<6} "
            f"{('—' if p.mark is None else p.mark):>5} "
            f"{p.live_def:>8} {str(gd):>7} {p.live_alloc:>8} {str(ga):>7}  {p.cls}"
        )
    if not rows:
        print("  (no pair has a non-zero gap under either definition — see the caveat above)")
    print()

    print("MISSING IDS  [archive-explained and unexplained, in SEPARATE columns, never merged]")
    any_missing = False
    for p in sorted(pairs, key=lambda x: (x.path, x.prefix)):
        if p.mark is None:
            continue
        if not (p.missing_def or p.missing_alloc or p.arch_def or p.arch_alloc):
            continue
        any_missing = True
        print(f"  {p.path}  [{p.prefix}, mark {p.mark}, {p.backing}]")
        print(
            f"      def_re     archive-explained {p.arch_def:<4} unexplained "
            f"{len(p.missing_def):<4} {fmt_ids(p.missing_def, p.prefix)}"
            + (f"   [?{p.elsewhere_def} also seen elsewhere in archives]" if p.elsewhere_def else "")
        )
        print(
            f"      allocator  archive-explained {p.arch_alloc:<4} unexplained "
            f"{len(p.missing_alloc):<4} {fmt_ids(p.missing_alloc, p.prefix)}"
            + (f"   [?{p.elsewhere_alloc} also seen elsewhere in archives]" if p.elsewhere_alloc else "")
        )
    if not any_missing:
        print("  (none)")
    print()

    print("WORKED CASE — the ADR's own motivating incident, computed live")
    t = next((p for p in pairs if p.path.endswith("tool-usage-patterns.md")), None)
    if t is None:
        print("  docs/trackers/tool-usage-patterns.md is not in this tree — the worked case is")
        print("  the whole point of the header caveat; re-read it before reading a row.")
    else:
        print(
            f"  {t.path} [{t.prefix}]: mark {t.mark} - live {t.live_alloc} "
            f"= {t.mark - t.live_alloc}  ->  classified `{t.cls}`"
        )
        print(
            "  19 rows were lost from this ledger on 2026-08-16. The predicate returns a"
        )
        print("  non-positive number and this probe reports it CLEAN. That is the blind spot,")
        print("  not a rounding error.")


def _nonzero(p: Pair) -> bool:
    if p.mark is None:
        return False
    return (p.mark - p.live_def) != 0 or (p.mark - p.live_alloc) != 0


# --------------------------------------------------------------------------------------


def main() -> int:
    ap = argparse.ArgumentParser(
        prog="probe-ledger-entry-loss.py",
        description=(
            "Per ledger/prefix pair, the gap between ids ALLOCATED "
            "(entry_high_water_<PREFIX>) and entries PRESENT. Reads a git rev, never the "
            "worktree. STRUCTURALLY BLIND to params-backed ledgers — read the header it "
            "prints on every run before quoting any row."
        ),
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=(
            "Two live-count definitions are reported side by side and never merged:\n"
            "  def_re     what link_scan treats as a DEFINITION (heading + dash + title,\n"
            "             [A-Z]{1,3} only — a four-letter prefix is invisible to it)\n"
            "  allocator  what body_claimed_indices sees (heading OR index-table row, no\n"
            "             dash required, any prefix length)\n"
            "A large def_re gap beside a zero allocator gap means the definitions live in a\n"
            "companion file, not that the entries are gone.\n"
            "\n"
            "--rev takes any git rev, so a published figure can be re-derived at the tree it\n"
            "was published against instead of being reconciled by argument (PROBES.md rule 5).\n"
        ),
    )
    ap.add_argument("--rev", default="HEAD", help="git rev to read (default: HEAD)")
    ap.add_argument(
        "--all", action="store_true", help="print every pair, not only non-zero gaps"
    )
    ap.add_argument("--json", action="store_true", help="machine-readable output")
    ap.add_argument(
        "--no-self-check",
        action="store_true",
        help="skip the fence self-check (it is on by default and its FAIL is load-bearing)",
    )
    args = ap.parse_args()

    tree = Tree(repo_root(), args.rev)
    pairs, unenumerable, meta = collect(tree)
    selfcheck = (
        {"status": "skipped", "disagreements": [], "fixture": None,
         "fixture_fences_balanced_under_commonmark": None}
        if args.no_self_check
        else fence_self_check(tree, pairs)
    )

    if args.json:
        print(
            json.dumps(
                {
                    "rev": args.rev,
                    "commit": tree.commit,
                    "tree": tree.tree,
                    "run_at_utc": _dt.datetime.now(_dt.timezone.utc).isoformat(),
                    "dirty_tracked": tree.dirty_tracked(),
                    "meta": meta,
                    "unenumerable": unenumerable,
                    "self_check": selfcheck,
                    "pairs": [p.as_dict() for p in pairs],
                    "caveat": (
                        "entry_high_water_<PREFIX> is written only by the PROSE allocator "
                        "(frontmatter.rs:387 <- augmentation.rs:1507 <- append_entry.rs:183). "
                        "Params-backed ledgers write no mark; a zero gap is evidence about "
                        "the prose half only."
                    ),
                },
                indent=2,
            )
        )
        return 0

    render(tree, pairs, unenumerable, meta, selfcheck, args.all)
    # Exit non-zero only when the instrument itself is in doubt. A gap is a finding to
    # read, not a build failure; a fence self-check that cannot find its own
    # disagreement is a broken instrument.
    return 1 if selfcheck["status"] == "FAIL" else 0


if __name__ == "__main__":
    sys.exit(main())
