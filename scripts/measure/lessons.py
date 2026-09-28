"""Task 7 — the lesson inventory frozen at a commit (spec A1.4, Outcome 3 / transfer).

`lessons_at(repo, sha)` answers "what lessons existed at this commit" so Task 9's judge can
decide whether a decision point could have applied one. Every source is read with
`git show <sha>:<path>` or `git ls-tree -r <sha>` — NEVER the working tree — so the function is a
pure function of the commit. `undated_lessons(path, repo, sha)` reads the operator's private
global CLAUDE.md instead; those lessons carry `dated=False` and never count as "existing at
origin" (they have no commit to be frozen at). `repo`/`sha` there are used only to derive the R79
equality-fallback reference set (R84) — never to read `path` itself.

Every markdown parser in this module (heading, bullet-start and Status-line detection) is
fence-aware (R85): a ``` or ~~~ fence is closed only by a fence of the SAME character and AT
LEAST the same length, and every line inside an open fence is opaque content, never structure.

Run: ~/work/claude/prompt-engineering/.venv/bin/python -m pytest tests/test_measure_lessons.py -v
"""
import collections
import dataclasses
import pathlib
import re
import subprocess

_PROMOTED_STATUSES = {"promoted", "promoted-to-permanent-docs"}


@dataclasses.dataclass(frozen=True)
class Lesson:
    id: str
    source: str
    text: str
    dated: bool


# --- git-object reads (never the working tree) -----------------------------------------------


def _git_show(repo, sha, path):
    """Return `path`'s text at `sha`, or None if it does not exist there.

    A pure read of the git object -- deliberately never the working tree. `git show <sha>:<path>`
    exits non-zero when the path is absent at that commit (added later, deleted, or never
    existed); that is not an error here, every source this module reads is optional per-commit.
    """
    proc = subprocess.run(
        ["git", "show", f"{sha}:{path}"],
        cwd=str(repo),
        capture_output=True,
        text=True,
    )
    if proc.returncode != 0:
        return None
    return proc.stdout


def _git_ls_tree(repo, sha, path):
    """Return tracked file paths (repo-relative) under `path` at `sha`.

    `git ls-tree -r <sha> --name-only -- <path>` reads the commit's tree object -- never
    `git ls-files`, which reads the working tree's index and would leak untracked or
    since-modified content (R83's tracked-vs-working-tree rule).
    """
    proc = subprocess.run(
        ["git", "ls-tree", "-r", sha, "--name-only", "--", path],
        cwd=str(repo),
        capture_output=True,
        text=True,
    )
    if proc.returncode != 0:
        return []
    return [line for line in proc.stdout.splitlines() if line]


# --- fence-awareness (R85) ----------------------------------------------------------------------

_FENCE_OPEN_RE = re.compile(r"^(\s*)(`{3,}|~{3,})(.*)$")
_FENCE_CLOSE_RE = re.compile(r"^(\s*)(`{3,}|~{3,})\s*$")


def _fence_flags(lines):
    """Per-line: True iff the line is inside (or is the opener/closer of) a ``` or ~~~ fence.

    A fence closes only on a line with the SAME fence character and AT LEAST the same run
    length (R85) -- a shorter or different-character fence line is opaque fence content, not a
    closer. CommonMark also refuses a backtick fence whose info string itself contains a
    backtick (a "runaway fence" would otherwise never close and silently swallow the rest of
    the file, item 9/fix round 2) -- tilde fences have no such restriction, since a tilde info
    string may contain backticks freely.
    """
    flags = []
    in_fence = False
    fence_char = None
    fence_len = 0
    for line in lines:
        if in_fence:
            flags.append(True)
            m = _FENCE_CLOSE_RE.match(line)
            if m and m.group(2)[0] == fence_char and len(m.group(2)) >= fence_len:
                in_fence = False
            continue
        m = _FENCE_OPEN_RE.match(line)
        if m and not (m.group(2)[0] == "`" and "`" in m.group(3)):
            flags.append(True)
            in_fence = True
            fence_char = m.group(2)[0]
            fence_len = len(m.group(2))
            continue
        flags.append(False)
    return flags


# --- generic markdown helpers ----------------------------------------------------------------

_HEADING_RE = re.compile(r"^(#{1,6})\s+(.*)$")


def _heading_match(line):
    """Return (level, heading_text) for a markdown heading line, else None."""
    m = _HEADING_RE.match(line)
    if not m:
        return None
    return len(m.group(1)), m.group(2).strip()


_SLUG_STRIP_RE = re.compile(r"[^a-z0-9]+")


def _slugify(text):
    text = text.strip().lower().replace("`", "")
    slug = _SLUG_STRIP_RE.sub("-", text).strip("-")
    return slug or "section"


def _dedupe_slug(slug, used_ids):
    """Return a slug guaranteed unique against `used_ids` (mutated in place to add whichever
    slug is returned): `slug` itself if unused, else `slug-2`, `slug-3`, ... until one is free.

    Re-derives against the FULL used-ids set on every call -- not a per-natural-slug counter --
    so a disambiguated id minted for an earlier collision (e.g. `notes-2`) cannot itself collide
    with a THIRD heading whose OWN natural slug is `notes-2` (R87 fix round 2; the prior
    per-slug-counter form minted exactly that duplicate for `## Notes`, `## Notes`, `## Notes 2`).
    """
    if slug not in used_ids:
        used_ids.add(slug)
        return slug
    n = 2
    while f"{slug}-{n}" in used_ids:
        n += 1
    candidate = f"{slug}-{n}"
    used_ids.add(candidate)
    return candidate


def _split_sections(text, min_level, max_level):
    """Split `text` into (heading_text, body_text) at each heading whose level is in
    [min_level, max_level]. Content before the first such heading is discarded (callers that
    want a whole-file fallback check `bool(sections)` themselves). A heading OUTSIDE the range
    (shallower or deeper) does not start a new section here -- it is absorbed as body content,
    which is what lets a "####"-level sub-note ride along inside its enclosing "##" section.

    Fence-aware (R85): a line inside a fenced code block is never treated as a heading, even if
    it is shaped like one (`## Not a heading` inside a ``` block stays body content).
    """
    lines = text.splitlines()
    fenced = _fence_flags(lines)
    sections = []
    heading = None
    body = []
    for i, line in enumerate(lines):
        hm = None if fenced[i] else _heading_match(line)
        if hm and min_level <= hm[0] <= max_level:
            if heading is not None:
                sections.append((heading, "\n".join(body)))
            heading = hm[1]
            body = []
        else:
            if heading is not None:
                body.append(line)
    if heading is not None:
        sections.append((heading, "\n".join(body)))
    return sections


def _parse_id_entries(text, id_re, entry_level):
    """Split `text` into (entry_id -> body) for headings at exactly `entry_level` whose text
    matches `id_re` (group 1 = id). Returns (entries dict, ids in document order).

    A heading at or shallower than `entry_level` that does NOT match `id_re` (a tracker's own
    prose section header, e.g. "## History") ends the current entry's body without starting a
    new one, so that prose is never misattributed as part of an entry. A heading DEEPER than
    `entry_level` (e.g. a "#### Correction ..." addendum written into an existing entry) is
    absorbed into the current entry's body instead of ending it.

    Fence-aware (R85): a fenced line shaped like a heading (e.g. a `# a shell comment` inside a
    ```bash block) never ends or starts an entry -- it is body content of whichever entry is
    open, so a Status line that follows a fenced comment is still reached.

    R99 (fix round 3): a REPEATED entry id (e.g. two `## R-5` headings) is disambiguated via
    `_dedupe_slug` against a used-ids set scoped to this one call, so the second occurrence keys
    in as `"R-5-2"` -- not the same key overwriting the first entry's body in `entries`, and not
    the same id emitted twice in `order`. The disambiguated id is what every caller (and the
    fixed-up `entries[op_id]`/`entries[r_id]` lookup) sees; a row-less disambiguated id (no
    matching index-table entry, e.g. an OP-N's second occurrence) is naturally excluded by the
    existing "row-less section is ABSENT" rule those callers already apply -- no special-casing.
    """
    lines = text.splitlines()
    fenced = _fence_flags(lines)
    entries = {}
    order = []
    used_ids = set()
    current_id = None
    body = []

    def flush():
        if current_id is not None:
            entries[current_id] = "\n".join(body)

    for i, line in enumerate(lines):
        hm = None if fenced[i] else _heading_match(line)
        if hm is not None:
            level, heading_text = hm
            if level == entry_level:
                m = id_re.match(heading_text)
                if m:
                    flush()
                    current_id = _dedupe_slug(m.group(1), used_ids)
                    order.append(current_id)
                    body = []
                    continue
            if level <= entry_level:
                flush()
                current_id = None
                body = []
                continue
        if current_id is not None:
            body.append(line)
    flush()
    return entries, order


def _first_field_line(body, field_re):
    """The value (group 1) of the FIRST non-fenced line of `body` matching `field_re`, else
    None. Fence-aware (R85): a line shaped like the field but sitting inside a fenced code block
    is never a match. Used for both Status-line detection (R82/R87 "first Status line wins") and
    Imperative-line extraction (R84) -- one fence-aware "first matching field line" primitive.
    """
    lines = body.splitlines()
    fenced = _fence_flags(lines)
    for i, line in enumerate(lines):
        if fenced[i]:
            continue
        m = field_re.match(line)
        if m:
            return m.group(1).strip()
    return None


# --- CLAUDE.md (R80, R86) ----------------------------------------------------------------------

_BULLET_START_RE = re.compile(r"^- ")
_BULLET_BOLD_RE = re.compile(r"^- \*\*(.+?)\*\*", re.DOTALL)
_WORD_RE = re.compile(r"[A-Za-z0-9']+")


def _lead_slug(lead_text, n=8):
    words = _WORD_RE.findall(lead_text)[:n]
    return _slugify(" ".join(words))


def _bullet_lead_source(bullet_text):
    """The text to derive a bold bullet's lead-slug from: the closed `**...**` span (which may
    itself span multiple physical lines) when the bold closes, or -- Critical fix -- the
    bullet's own text (marker stripped) when the bold NEVER closes, so a `- **`-led bullet is
    never silently dropped just because its emphasis span is malformed.
    """
    m = _BULLET_BOLD_RE.match(bullet_text)
    if m:
        return m.group(1)
    return _BULLET_START_RE.sub("", bullet_text, count=1)


def _line_indented(line):
    return bool(line) and line[0] in (" ", "\t")


def _bullet_continuation_split(span_lines):
    """Partition one bullet's line span (span_lines[0] is its `- ` marker line, up to but not
    including the next bullet start or end of section) into (continuation_lines, prose_lines).

    R86: "A bullet's continuation is its indented or directly following lines, up to a blank
    line followed by non-indented, non-bullet text." Concretely: every line is continuation
    until we hit a blank line whose next non-blank line (skipping further blanks) is NOT
    indented -- from there to the end of the span is prose, not the bullet's own text. This is
    an exact partition of `span_lines`: every line lands in exactly one of the two outputs.
    """
    n = len(span_lines)
    continuation = []
    prose = []
    in_continuation = True
    i = 0
    while i < n:
        line = span_lines[i]
        if not in_continuation:
            prose.append(line)
            i += 1
            continue
        if line.strip() == "":
            j = i + 1
            while j < n and span_lines[j].strip() == "":
                j += 1
            if j < n and not _line_indented(span_lines[j]):
                in_continuation = False
            continuation.append(line)
            i += 1
            continue
        continuation.append(line)
        i += 1
    return continuation, prose


def _partition_bullets_and_prose(body):
    """Fence-aware (R85) partition of one CLAUDE.md section's body into:
    - `bold_bullets`: one string per `- **`-led bullet-start line, in document order, holding
      exactly that bullet's own continuation text (R86) -- emitted even when its bold span
      never closes (Critical fix).
    - `prose_lines`: every OTHER line of `body`, in original order and exactly once each: prose
      before the first bullet, whole plain (non-`- **`-led) bullet spans, and each bold
      bullet's own trailing lines that are not its continuation.

    Together these are an exact partition of `body`'s lines -- the R86 coverage invariant.
    """
    lines = body.splitlines()
    fenced = _fence_flags(lines)
    starts = [i for i, ln in enumerate(lines) if not fenced[i] and _BULLET_START_RE.match(ln)]
    if not starts:
        return [], list(lines)
    bold_bullets = []
    prose_lines = []
    prev_end = 0
    for idx, start in enumerate(starts):
        end = starts[idx + 1] if idx + 1 < len(starts) else len(lines)
        prose_lines.extend(lines[prev_end:start])
        span = lines[start:end]
        if span[0].startswith("- **"):
            continuation, trailing = _bullet_continuation_split(span)
            bold_bullets.append("\n".join(continuation))
            prose_lines.extend(trailing)
        else:
            prose_lines.extend(span)
        prev_end = end
    return bold_bullets, prose_lines


def _leading_preamble(text, min_level, max_level):
    """Text before the first heading whose level is in [min_level, max_level] (fence-aware, same
    rule `_split_sections` uses to find its first section). `_split_sections` documents this
    span as discarded; CLAUDE.md routinely opens with a title line and description prose before
    its first `##`, and R86's invariant is stated over the WHOLE file, so a CLAUDE.md-specific
    caller must recover it rather than silently drop it (R86 fix round 1)."""
    lines = text.splitlines()
    fenced = _fence_flags(lines)
    for i, line in enumerate(lines):
        hm = None if fenced[i] else _heading_match(line)
        if hm and min_level <= hm[0] <= max_level:
            return "\n".join(lines[:i])
    return text


def _emit_partitioned_lessons(
    lessons, id_prefix, source_label, heading_id_slug, heading_display, bold_bullets, prose_lines,
):
    """Append one lesson per bullet in `bold_bullets` plus (if non-empty) one section-prose
    lesson for `prose_lines`, under `heading_id_slug`. `heading_display` is the heading text to
    prefix onto the section-prose lesson's `.text` (e.g. "## Some Heading"), or None for a span
    with no real heading (the CLAUDE.md preamble), whose prose lesson carries no such prefix.
    Shared by `_claude_md_lessons`'s per-section loop and its preamble span (R86 fix round 1).

    R99 (fix round 3): a bullet's lead-slug is disambiguated via `_dedupe_slug` against a FRESH
    `used_lead_ids` set, scoped to this one call (i.e. per section/preamble span) -- not a
    per-natural-slug counter. The counter form minted the SAME suffix for two different natural
    slugs in document order (`Foo bar.`, `Foo bar.`, `Foo bar 2.` -> `foo-bar`, `foo-bar-2`,
    `foo-bar-2`: the counter never checked whether `foo-bar-2` was already itself a natural
    slug); `_dedupe_slug`'s full-set re-check is the same fix R87 already made for heading slugs.
    """
    used_lead_ids = set()
    for bullet_text in bold_bullets:
        slug = _lead_slug(_bullet_lead_source(bullet_text))
        lead_id_slug = _dedupe_slug(slug, used_lead_ids)
        lessons.append(Lesson(
            id=f"{id_prefix}#{heading_id_slug}/{lead_id_slug}",
            source=source_label,
            text=bullet_text.strip(),
            dated=True,
        ))

    prose_text = "\n".join(prose_lines).strip()
    if prose_text:
        text_value = (
            f"## {heading_display}\n{prose_text}".strip() if heading_display is not None
            else prose_text
        )
        lessons.append(Lesson(
            id=f"{id_prefix}#{heading_id_slug}",
            source=source_label,
            text=text_value,
            dated=True,
        ))


def _claude_md_lessons(text, id_prefix, source_label):
    """One lesson per `- **`-led bullet, PLUS one section-prose lesson per section holding every
    non-bullet-continuation line (R86) -- never a heading-only (empty-body) lesson. The span
    before the first `##`/`###` heading (a CLAUDE.md title + description, if any) is its own
    "preamble" pseudo-section under the same rules, so the R86 invariant holds over the WHOLE
    file, not only from the first heading onward (R86 fix round 1).

    R80: a bullet's id is `<id_prefix>#<heading-slug>/<lead-slug>`, lead-slug from the bullet's
    bold lead (its first 8 words); `-<n>` on a lead-slug collision WITHIN THE SAME SECTION only,
    counted in document order. R87: a HEADING-slug collision is disambiguated across the WHOLE
    document via `_dedupe_slug`, which checks the full set of ids already minted -- not merely a
    per-natural-slug counter -- so a disambiguated slug from an earlier collision cannot itself
    collide with a third heading's own natural slug (fix round 2; the prior per-slug counter
    minted a duplicate id for `## Notes`, `## Notes`, `## Notes 2`).

    The synthetic preamble pseudo-section's id is the UNCONDITIONAL literal slug `-preamble`,
    never plain `preamble`: `_slugify` strips a leading `-` from any real heading's slug, so no
    real `## Preamble` heading can ever produce `-preamble`, and the synthetic id is therefore
    stable across every commit -- never displaced by, and never displacing, a real section of
    that name (fix round 2, item 8; previously the two shared the same slug and a real
    "## Preamble" section added later could take over `#preamble` out from under the synthetic
    one, breaking R80's cross-commit identity stability).

    The section-prose lesson's id is the bare (possibly disambiguated) heading-slug, no
    `/lead-slug` suffix, so it can never collide with a bullet lesson's id in the same section.
    """
    lessons = []
    used_ids = set()

    preamble = _leading_preamble(text, 2, 3)
    pre_bullets, pre_prose = _partition_bullets_and_prose(preamble)
    if pre_bullets or "\n".join(pre_prose).strip():
        preamble_id_slug = _dedupe_slug("-preamble", used_ids)
        _emit_partitioned_lessons(
            lessons, id_prefix, source_label, preamble_id_slug, None, pre_bullets, pre_prose,
        )

    for heading_text, body in _split_sections(text, 2, 3):
        heading_slug = _slugify(heading_text)
        heading_id_slug = _dedupe_slug(heading_slug, used_ids)

        bold_bullets, prose_lines = _partition_bullets_and_prose(body)
        _emit_partitioned_lessons(
            lessons, id_prefix, source_label, heading_id_slug, heading_text, bold_bullets, prose_lines,
        )
    return lessons


# --- docs/trackers/operator-rules.md (R81) ----------------------------------------------------

_OP_TABLE_ROW_RE = re.compile(r"^\|\s*(OP-\d+)\s*\|(.*)\|\s*$")
_OP_ID_RE = re.compile(r"^(OP-\d+)\b")
_IMPERATIVE_LINE_RE = re.compile(r"^\*\*Imperative:\*\*\s*(.+)$")


def _parse_op_table_statuses(text):
    """The Index-table Status cell for each `OP-N` row, keyed by id, lowercased -- fence-aware
    (R85 fix round 2): an index-table-shaped row that appears inside a fenced example block
    (a documentation illustration of the table syntax, not a live row) is opaque content and
    must not update `statuses`, since `_operator_rules_lessons`'s existence gate reads this map
    directly.
    """
    statuses = {}
    lines = text.splitlines()
    fenced = _fence_flags(lines)
    for i, line in enumerate(lines):
        if fenced[i]:
            continue
        m = _OP_TABLE_ROW_RE.match(line)
        if not m:
            continue
        cells = m.group(2).split("|")
        last = cells[-1].strip() if cells else ""
        statuses[m.group(1)] = last.strip("*").strip().lower()
    return statuses


def _operator_rules_lessons(text, source_label):
    """R81: an OP-N lesson exists iff its `## OP-N` section exists AND its index-table row (at
    this same text/commit) reads exactly "active". A row reading anything else (e.g. "retired"),
    and a `## OP-N` section with NO index row at all, are both treated as ABSENT -- not raised,
    because a row-less section is exactly the R81 "iff" failing on its status half. The id this
    helper mints is bare (`operator-rules.md#OP-N`); `lessons_at` repo-qualifies BOTH `id` and
    `source` afterward, in its own single post-processing step over every source's already-built
    lessons (R87/R96(a)). So ids ARE repo-qualified in the value `lessons_at` actually returns --
    only this helper's own, pre-post-processing return value is not.
    """
    statuses = _parse_op_table_statuses(text)
    entries, order = _parse_id_entries(text, _OP_ID_RE, 2)
    lessons = []
    for op_id in order:
        if statuses.get(op_id) != "active":
            continue
        lessons.append(Lesson(
            id=f"operator-rules.md#{op_id}",
            source=source_label,
            text=f"## {op_id}\n{entries[op_id]}".strip(),
            dated=True,
        ))
    return lessons


def _op_imperative_bodies(repo, sha):
    """R84: the R79 equality-fallback reference set is the `**Imperative:**` line of each ACTIVE
    `## OP-N` section of docs/trackers/operator-rules.md AT `sha` -- never the working tree,
    never a default. "Active" here means the same thing R81's own existence gate means:
    `_parse_op_table_statuses(text)[op_id] == "active"` (R97, fix round 2). A retired rule's
    Imperative is therefore not a reference-set member, so a hand-written paragraph equal to a
    RETIRED rule's Imperative is counted once as undated (R81 already excludes it as dated), never
    zero times -- the pre-fix behaviour put every `## OP-N` section's Imperative into the set
    regardless of status, which could make such a paragraph match on the dated side too and be
    silently dropped from both.

    The generator (undated_lessons's real caller) emits only that one line per rule, so comparing
    whole section bodies (the pre-fix behaviour) could never match it.

    Raises FileNotFoundError if the file does not exist at `sha`. Raises ValueError if the file
    exists but the ACTIVE-only reference set comes out empty: there is no silent
    empty-reference-set fallback either way, because an empty reference set makes the whole R79
    detector inert without anyone being told (fix round 2 extends this from the file-absent case
    to the zero-active-imperatives case).
    """
    text = _git_show(repo, sha, "docs/trackers/operator-rules.md")
    if text is None:
        raise FileNotFoundError(
            f"docs/trackers/operator-rules.md not found at {sha!r} in {repo!r} -- R84 requires "
            "it to derive the R79 equality-fallback reference set; there is no default and no "
            "working-tree read."
        )
    statuses = _parse_op_table_statuses(text)
    entries, order = _parse_id_entries(text, _OP_ID_RE, 2)
    bodies = []
    for op_id in order:
        if statuses.get(op_id) != "active":
            continue
        imperative = _first_field_line(entries[op_id], _IMPERATIVE_LINE_RE)
        if imperative:
            bodies.append(imperative)
    if not bodies:
        raise ValueError(
            f"docs/trackers/operator-rules.md at {sha!r} in {repo!r} exists but yields zero "
            "ACTIVE **Imperative:** references -- R84's reference set must not be silently empty."
        )
    return bodies


# --- docs/trackers/reconnaissance-patterns.md (R-N) and tool-usage-patterns.md (T-N) — R82 -----

_R_ID_RE = re.compile(r"^(R-\d+)\b")
_T_ID_RE = re.compile(r"^(T-\d+)\b")
_STATUS_LINE_RE = re.compile(r"^\*\*Status:\*\*\s*([A-Za-z][A-Za-z0-9_-]*)")


def _entry_status(body):
    """The entry's status token: the WORD right after the FIRST (R87: "first Status line wins")
    non-fenced `**Status:**` line, stopping at the first whitespace/punctuation boundary -- e.g.
    "promoted -- verdict ..." reads as "promoted", and "promoted-to-permanent-docs" (a hyphenated
    compound with no internal whitespace) reads as one token. An entry with no `**Status:**` line
    (most T-N entries use `**Verdict:**` instead) returns None. Exact-token match only (R82):
    the regex anchors on the word right after the marker, so "open — ... not yet promoted" reads
    as "open", never as a substring hit on "promoted".
    """
    val = _first_field_line(body, _STATUS_LINE_RE)
    return val.lower() if val is not None else None


def _tracker_lessons(text, id_re, entry_level, id_prefix, source_label):
    """R82: an R-N/T-N lesson is an entry whose status is EXACTLY "promoted" or
    "promoted-to-permanent-docs". Never deduped across sources -- each Lesson keeps its own
    `source`, so an R-N and a T-N entry can never collide even if their bodies happened to match.
    """
    entries, order = _parse_id_entries(text, id_re, entry_level)
    lessons = []
    for entry_id in order:
        status = _entry_status(entries[entry_id])
        if status not in _PROMOTED_STATUSES:
            continue
        lessons.append(Lesson(
            id=f"{id_prefix}#{entry_id}",
            source=source_label,
            text=f"{entry_id}\n{entries[entry_id]}".strip(),
            dated=True,
        ))
    return lessons


# --- .codescout/memories/**/*.md (R83) ---------------------------------------------------------

_MEMORIES_PREFIX = ".codescout/memories/"


def _memory_lessons(repo, sha, repo_name):
    """R83: each `##` section (never `###` -- a "###"-level sub-note is absorbed into its
    enclosing "##" section's body, same rule `_split_sections` always applied) of each TRACKED
    (`git ls-tree`, never the working tree) `.codescout/memories/**/*.md` file is one lesson, id
    `memory:<rel-path-without-.md>#<heading-slug>`. A file with no `##` section is one lesson, id
    `memory:<rel-path-without-.md>` (whole file as its own text). Non-`.md` files (e.g.
    `*.anchors.toml`) are skipped. R87: `_dedupe_slug` disambiguates repeated `##` headings
    within one file (fresh `used_ids` per file, checking the full set rather than a per-slug
    counter, so a disambiguated id cannot itself collide with a third heading's own natural
    slug -- fix round 2). `source` is repo-qualified here, at construction; `id` is repo-qualified
    later, by `lessons_at`'s own post-processing step over every source's lessons -- so both `id`
    and `source` ARE repo-qualified in what `lessons_at` returns (R87/R96(a)).
    """
    lessons = []
    for path in sorted(_git_ls_tree(repo, sha, ".codescout/memories")):
        if not path.endswith(".md"):
            continue
        content = _git_show(repo, sha, path)
        if content is None:
            continue
        rel = path[len(_MEMORIES_PREFIX):-len(".md")] if path.startswith(_MEMORIES_PREFIX) else path[:-len(".md")]
        source_label = f"{repo_name}:{path}"
        sections = _split_sections(content, 2, 2)
        if not sections:
            lessons.append(Lesson(
                id=f"memory:{rel}",
                source=source_label,
                text=content.strip(),
                dated=True,
            ))
            continue
        used_ids = set()
        for heading_text, body in sections:
            slug = _slugify(heading_text)
            slug_id = _dedupe_slug(slug, used_ids)
            lessons.append(Lesson(
                id=f"memory:{rel}#{slug_id}",
                source=source_label,
                text=f"## {heading_text}\n{body}".strip(),
                dated=True,
            ))
    return lessons


# --- public: lessons_at -------------------------------------------------------------------------


def _repo_name(repo):
    """The repo name used to qualify `lessons_at`'s ids and sources (R96(b), fix round 2):
    the basename of the directory CONTAINING the git common dir
    (`git rev-parse --path-format=absolute --git-common-dir`), never the repo path argument's
    own basename. Worktree-safe -- a `git worktree add` checkout's own directory can have any
    basename (e.g. `mutation-slot-0`) while its common dir still lives inside the main
    checkout's `.git`, so this resolves to the MAIN repo's name in both places, keeping
    `lessons_at(main, sha)` and `lessons_at(worktree, sha)` identical. For an ordinary
    (non-worktree) repo the common dir's parent IS the repo root, so this agrees with
    `repo.resolve().name` there -- the two computations diverge only under a worktree.
    Falls back to the repo path's own basename if git cannot answer at all (not a git
    repository).

    R100 (fix round 3, accepted known limit -- no code change): a submodule, a
    `--separate-git-dir` checkout, or a bare repository can each place the common dir's PARENT
    at something other than the checkout a human would call "the repo" (a submodule's common
    dir parent is the superproject's `.git/modules/<name>` entry; `--separate-git-dir` and a
    bare repo have no working-tree directory at all to name), so the basename this function
    returns in those layouts need not equal the checkout's own name. This pipeline's repos are
    ordinary checkouts and worktrees, where the guarantee above holds; the limit is accepted
    rather than generalized against, since neither case occurs in this pipeline's inputs.
    """
    try:
        out = subprocess.run(
            ["git", "rev-parse", "--path-format=absolute", "--git-common-dir"],
            cwd=str(repo),
            capture_output=True,
            text=True,
            check=True,
        ).stdout.strip()
        return pathlib.Path(out).resolve().parent.name
    except (subprocess.CalledProcessError, OSError):
        return pathlib.Path(repo).resolve().name



def _require_commit(repo, sha):
    """R98 (fix round 3): raise ValueError if `sha` does not resolve to a commit in `repo`.

    Covers a non-git directory, an unknown sha, and a typo -- every case where the CALLER passed
    something that cannot be a commit. Without this, `lessons_at` fell through every `_git_show`
    call (each returns None on a non-zero `git show`, the same signal a genuinely-absent-at-this-
    commit path gives) and silently returned `[]`: a typo'd sha would have zeroed Task 9's
    denominator with nothing anywhere to say why.
    """
    proc = subprocess.run(
        ["git", "-C", str(repo), "rev-parse", "--verify", f"{sha}^{{commit}}"],
        capture_output=True,
        text=True,
    )
    if proc.returncode != 0:
        raise ValueError(
            f"{sha!r} does not resolve to a commit in {repo!r} (R98): {proc.stderr.strip()}"
        )


def _require_unique_ids(lessons, context):
    """R99 (fix round 3): raise ValueError naming any id that repeats in `lessons`.

    Every disambiguating site upstream (heading slugs via `_dedupe_slug` in `_claude_md_lessons`
    and `_memory_lessons`, lead slugs in `_emit_partitioned_lessons`, tracker entry ids in
    `_parse_id_entries`) prevents a collision it can itself see -- but each disambiguates against
    only ITS OWN used-ids set, scoped to one heading, one file, or one tracker. This is the
    return-time backstop over the WHOLE assembled list, which is the only place a collision
    between two independent SOURCES becomes visible: e.g. a memory file `foo.md`'s own `## Bar`
    heading (`memory:foo#bar`) and a second memory file literally named `foo#bar.md` (also
    `memory:foo#bar`) -- neither disambiguator's used-ids set ever contains the other's
    candidate, so nothing upstream of this guard can catch it.
    """
    counts = collections.Counter(l.id for l in lessons)
    dupes = sorted(i for i, n in counts.items() if n > 1)
    if dupes:
        raise ValueError(f"{context}: duplicate lesson ids {dupes!r} (R99)")
    return lessons



def lessons_at(repo, sha):
    """All lessons that existed at `sha` -- a pure function of the commit (every read goes
    through `git show`/`git ls-tree`, never the working tree).

    R87/R96(a): `source` AND `id` are both repo-qualified (`"<repo-name>:<...>"`, repo name via
    `_repo_name`, worktree-safe -- R96(b)). Applied as a single post-processing step over every
    source's already-built ids, so no internal helper's own id-construction logic changes; only
    `lessons_at`'s own return value gains the prefix (fix round 2 -- R87's header already said
    ids are repo-qualified, its sub-bullets omitted it by drafting error, and round 1's docstring
    here said the opposite: "id stays the bare, unqualified form it always was").

    R98 (fix round 3): raises ValueError if `sha` does not resolve to a commit in `repo` -- a
    non-git directory, an unknown sha, or a typo would otherwise fall through every `_git_show`
    call and silently return `[]`, zeroing Task 9's denominator with nothing to say why.

    R99 (fix round 3): raises ValueError if the assembled list contains a duplicate id (see
    `_require_unique_ids`) just before returning.
    """
    repo = pathlib.Path(repo)
    _require_commit(repo, sha)
    repo_name = _repo_name(repo)
    lessons = []

    claude_md = _git_show(repo, sha, "CLAUDE.md")
    if claude_md is not None:
        lessons.extend(_claude_md_lessons(claude_md, "CLAUDE.md", f"{repo_name}:CLAUDE.md"))

    op_rules = _git_show(repo, sha, "docs/trackers/operator-rules.md")
    if op_rules is not None:
        lessons.extend(_operator_rules_lessons(
            op_rules, f"{repo_name}:docs/trackers/operator-rules.md"))

    recon = _git_show(repo, sha, "docs/trackers/reconnaissance-patterns.md")
    if recon is not None:
        lessons.extend(_tracker_lessons(
            recon, _R_ID_RE, 2, "reconnaissance-patterns.md",
            f"{repo_name}:docs/trackers/reconnaissance-patterns.md"))

    tool_usage = _git_show(repo, sha, "docs/trackers/tool-usage-patterns.md")
    if tool_usage is not None:
        lessons.extend(_tracker_lessons(
            tool_usage, _T_ID_RE, 3, "tool-usage-patterns.md",
            f"{repo_name}:docs/trackers/tool-usage-patterns.md"))

    lessons.extend(_memory_lessons(repo, sha, repo_name))

    lessons = [dataclasses.replace(l, id=f"{repo_name}:{l.id}") for l in lessons]
    return _require_unique_ids(lessons, f"lessons_at({repo}, {sha!r})")


# --- public: undated_lessons ---------------------------------------------------------------------

_MARKER_RE = re.compile(
    r"<!--\s*BEGIN operator-rules.*?-->.*?<!--\s*END operator-rules\s*-->\n?",
    re.DOTALL,
)


def _strip_marked_block(text):
    stripped, n = _MARKER_RE.subn("", text)
    return stripped, n > 0


def _normalize_ws(s):
    return " ".join(s.split()).strip()


def _strip_equal_paragraphs(text, reference_bodies):
    normalized_refs = {_normalize_ws(b) for b in reference_bodies if b.strip()}
    if not normalized_refs:
        return text
    paragraphs = re.split(r"\n[ \t]*\n", text)
    kept = [p for p in paragraphs if _normalize_ws(p) not in normalized_refs]
    return "\n\n".join(kept)


def undated_lessons(path, repo, sha):
    """Lessons from the operator's private global CLAUDE.md at `path`, `dated=False` -- they
    never count as "existing at origin" (spec A1.4; there is no commit to freeze them at).

    R79/R84: the generated operator-rules block (which duplicates dated OP-N rules already
    counted once, from operator-rules.md, by `lessons_at`) is excluded so it is never
    double-counted as undated. Detected by BEGIN/END markers first; where markers are absent, by
    exact text equality (after whitespace normalization) against each ACTIVE OP-N section's
    `**Imperative:**` line (R97 -- a retired rule's Imperative is not in this reference set), read
    via `git show <sha>:docs/trackers/operator-rules.md` in `repo` -- NEVER the working tree, and
    never a default (`repo`/`sha` are required; see `_op_imperative_bodies`).

    R87: `id` and `source` are the fixed literal "global-CLAUDE.md" -- never `path`'s actual
    filesystem location -- so the SAME content read from different profile directories
    (`~/.claude`, `~/.claude-sdd`, `~/.claude-kat`) yields byte-identical `Lesson.id`s.

    R98 (fix round 3): `_op_imperative_bodies` -> `_git_show` already raises (FileNotFoundError)
    when `sha` does not resolve at all, for the same reason a genuinely-absent-at-that-sha path
    raises -- `git show <sha>:<path>` cannot tell the two apart, and this function does not need
    its own separate sha-resolution check the way `lessons_at` does (R98's own guard lives there).

    R99 (fix round 3): raises ValueError if the assembled list contains a duplicate id (see
    `_require_unique_ids`) just before returning.
    """
    path = pathlib.Path(path)
    text = path.read_text()
    op_rule_bodies = _op_imperative_bodies(repo, sha)

    stripped, had_markers = _strip_marked_block(text)
    if not had_markers:
        stripped = _strip_equal_paragraphs(stripped, op_rule_bodies)

    lessons = []
    for lesson in _claude_md_lessons(stripped, "global-CLAUDE.md", "global-CLAUDE.md"):
        lessons.append(dataclasses.replace(lesson, dated=False))
    return _require_unique_ids(lessons, f"undated_lessons({path}, {repo}, {sha!r})")
