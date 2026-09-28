"""Task 7 — the lesson inventory frozen at a commit (spec A1.4, Outcome 3 / transfer).

`lessons_at(repo, sha)` answers "what lessons existed at this commit" so Task 9's judge can
decide whether a decision point could have applied one. Every source is read with
`git show <sha>:<path>` or `git ls-tree -r <sha>` — NEVER the working tree — so the function is a
pure function of the commit. `undated_lessons(path)` reads the operator's private global
`CLAUDE.md` instead; those lessons carry `dated=False` and never count as "existing at origin"
(they have no commit to be frozen at).

Run: ~/work/claude/prompt-engineering/.venv/bin/python -m pytest tests/test_measure_lessons.py -v
"""
import dataclasses
import pathlib
import re
import subprocess

REPO_ROOT = pathlib.Path(__file__).resolve().parents[2]

_PROMOTED_STATUSES = {"promoted", "promoted-to-permanent-docs"}


@dataclasses.dataclass
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


def _split_sections(text, min_level, max_level):
    """Split `text` into (heading_text, body_text) at each heading whose level is in
    [min_level, max_level]. Content before the first such heading is discarded (callers that
    want a whole-file fallback check `bool(sections)` themselves). A heading OUTSIDE the range
    (shallower or deeper) does not start a new section here -- it is absorbed as body content,
    which is what lets a "####"-level sub-note ride along inside its enclosing "##" section.
    """
    lines = text.splitlines()
    sections = []
    heading = None
    body = []
    for line in lines:
        hm = _heading_match(line)
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
    """
    lines = text.splitlines()
    entries = {}
    order = []
    current_id = None
    body = []

    def flush():
        if current_id is not None:
            entries[current_id] = "\n".join(body)

    for line in lines:
        hm = _heading_match(line)
        if hm is not None:
            level, heading_text = hm
            if level == entry_level:
                m = id_re.match(heading_text)
                if m:
                    flush()
                    current_id = m.group(1)
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


# --- CLAUDE.md (R80) --------------------------------------------------------------------------

_BULLET_START_RE = re.compile(r"^- ")
_BULLET_BOLD_RE = re.compile(r"^- \*\*(.+?)\*\*")
_WORD_RE = re.compile(r"[A-Za-z0-9']+")


def _split_bullets(body):
    lines = body.splitlines()
    bullets = []
    current = []
    for line in lines:
        if _BULLET_START_RE.match(line):
            if current:
                bullets.append("\n".join(current))
            current = [line]
        elif current:
            current.append(line)
    if current:
        bullets.append("\n".join(current))
    return bullets


def _lead_slug(lead_text, n=8):
    words = _WORD_RE.findall(lead_text)[:n]
    return _slugify(" ".join(words))


def _claude_md_lessons(text, source_label):
    """One lesson per `- **`-led bullet, one per `##`/`###` section with no such bullets.

    R80: a bullet's id is `<source_label>#<heading-slug>/<lead-slug>`, where lead-slug is a
    slug of the bullet's bold lead (its first 8 words). `-<n>` is appended only on a lead-slug
    collision WITHIN THE SAME SECTION, counted in document order -- never the bullet's index,
    so inserting an unrelated bullet earlier in the section cannot re-key a later one.
    """
    lessons = []
    for heading_text, body in _split_sections(text, 2, 3):
        heading_slug = _slugify(heading_text)
        bullets = _split_bullets(body)
        bold = [(b, _BULLET_BOLD_RE.match(b)) for b in bullets]
        bold = [(b, m) for b, m in bold if m]
        if not bold:
            lessons.append(Lesson(
                id=f"{source_label}#{heading_slug}",
                source=source_label,
                text=f"## {heading_text}\n{body}".strip(),
                dated=True,
            ))
            continue
        seen = {}
        for bullet_text, m in bold:
            slug = _lead_slug(m.group(1))
            seen[slug] = seen.get(slug, 0) + 1
            n = seen[slug]
            suffix = "" if n == 1 else f"-{n}"
            lessons.append(Lesson(
                id=f"{source_label}#{heading_slug}/{slug}{suffix}",
                source=source_label,
                text=bullet_text.strip(),
                dated=True,
            ))
    return lessons


# --- docs/trackers/operator-rules.md (R81) ----------------------------------------------------

_OP_TABLE_ROW_RE = re.compile(r"^\|\s*(OP-\d+)\s*\|(.*)\|\s*$")
_OP_ID_RE = re.compile(r"^(OP-\d+)\b")


def _parse_op_table_statuses(text):
    statuses = {}
    for line in text.splitlines():
        m = _OP_TABLE_ROW_RE.match(line)
        if not m:
            continue
        cells = m.group(2).split("|")
        last = cells[-1].strip() if cells else ""
        statuses[m.group(1)] = last.strip("*").strip().lower()
    return statuses


def _operator_rules_lessons(text):
    """R81: an OP-N lesson exists iff its `## OP-N` section exists AND its index-table row (at
    this same text/commit) reads exactly "active". A row reading anything else (e.g. "retired"),
    and a `## OP-N` section with NO index row at all, are both treated as ABSENT (documented
    choice, per task-7-context.md's "absent or raises, state which") -- not raised, because a
    row-less section is exactly the R81 "iff" failing on its status half.
    """
    statuses = _parse_op_table_statuses(text)
    entries, order = _parse_id_entries(text, _OP_ID_RE, 2)
    lessons = []
    for op_id in order:
        if statuses.get(op_id) != "active":
            continue
        lessons.append(Lesson(
            id=f"operator-rules.md#{op_id}",
            source="operator-rules.md",
            text=f"## {op_id}\n{entries[op_id]}".strip(),
            dated=True,
        ))
    return lessons


# --- docs/trackers/reconnaissance-patterns.md (R-N) and tool-usage-patterns.md (T-N) — R82 -----

_R_ID_RE = re.compile(r"^(R-\d+)\b")
_T_ID_RE = re.compile(r"^(T-\d+)\b")
_STATUS_LINE_RE = re.compile(r"^\*\*Status:\*\*\s*([A-Za-z][A-Za-z0-9_-]*)")


def _entry_status(body):
    """The entry's status token: the WORD right after `**Status:**`, stopping at the first
    whitespace/punctuation boundary -- e.g. "promoted -- verdict ..." reads as "promoted", and
    "promoted-to-permanent-docs" (a hyphenated compound with no internal whitespace) reads as
    one token. Entries with no `**Status:**` line (most T-N entries use `**Verdict:**` instead)
    return None.
    """
    for line in body.splitlines():
        m = _STATUS_LINE_RE.match(line)
        if m:
            return m.group(1).lower()
    return None


def _tracker_lessons(text, id_re, entry_level, source_name):
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
            id=f"{source_name}#{entry_id}",
            source=source_name,
            text=f"{entry_id}\n{entries[entry_id]}".strip(),
            dated=True,
        ))
    return lessons


# --- .codescout/memories/**/*.md (R83) ---------------------------------------------------------

_MEMORIES_PREFIX = ".codescout/memories/"


def _memory_lessons(repo, sha):
    """R83: each `##` section of each TRACKED (`git ls-tree`, never the working tree)
    `.codescout/memories/**/*.md` file is one lesson, id `memory:<rel-path-without-.md>#<heading-
    slug>`. A file with no `##` section is one lesson, id `memory:<rel-path-without-.md>` (whole
    file as its own text). Non-`.md` files (e.g. `*.anchors.toml`) are skipped.
    """
    lessons = []
    for path in sorted(_git_ls_tree(repo, sha, ".codescout/memories")):
        if not path.endswith(".md"):
            continue
        content = _git_show(repo, sha, path)
        if content is None:
            continue
        rel = path[len(_MEMORIES_PREFIX):-len(".md")] if path.startswith(_MEMORIES_PREFIX) else path[:-len(".md")]
        sections = _split_sections(content, 2, 2)
        if not sections:
            lessons.append(Lesson(
                id=f"memory:{rel}",
                source=path,
                text=content.strip(),
                dated=True,
            ))
            continue
        for heading_text, body in sections:
            lessons.append(Lesson(
                id=f"memory:{rel}#{_slugify(heading_text)}",
                source=path,
                text=f"## {heading_text}\n{body}".strip(),
                dated=True,
            ))
    return lessons


# --- public: lessons_at -------------------------------------------------------------------------


def lessons_at(repo, sha):
    """All lessons that existed at `sha` -- a pure function of the commit (every read goes
    through `git show`/`git ls-tree`, never the working tree)."""
    repo = pathlib.Path(repo)
    lessons = []

    claude_md = _git_show(repo, sha, "CLAUDE.md")
    if claude_md is not None:
        lessons.extend(_claude_md_lessons(claude_md, "CLAUDE.md"))

    op_rules = _git_show(repo, sha, "docs/trackers/operator-rules.md")
    if op_rules is not None:
        lessons.extend(_operator_rules_lessons(op_rules))

    recon = _git_show(repo, sha, "docs/trackers/reconnaissance-patterns.md")
    if recon is not None:
        lessons.extend(_tracker_lessons(recon, _R_ID_RE, 2, "reconnaissance-patterns.md"))

    tool_usage = _git_show(repo, sha, "docs/trackers/tool-usage-patterns.md")
    if tool_usage is not None:
        lessons.extend(_tracker_lessons(tool_usage, _T_ID_RE, 3, "tool-usage-patterns.md"))

    lessons.extend(_memory_lessons(repo, sha))

    return lessons


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


def _default_op_rule_bodies():
    """Best-effort reference set for the text-equality detector, read from THIS repo's current
    working-tree `docs/trackers/operator-rules.md` -- reasonable for `undated_lessons`'s real
    caller (there is no commit to pin an "undated" lesson to), and overridable by tests via the
    `op_rule_bodies` parameter so unit tests never depend on this repo's live tracker content.
    """
    op_path = REPO_ROOT / "docs" / "trackers" / "operator-rules.md"
    try:
        text = op_path.read_text()
    except OSError:
        return []
    entries, _order = _parse_id_entries(text, _OP_ID_RE, 2)
    return [body.strip() for body in entries.values() if body.strip()]


def undated_lessons(path, op_rule_bodies=None):
    """Lessons from the operator's private global CLAUDE.md, `dated=False` -- they never count
    as "existing at origin" (spec A1.4; there is no commit to freeze them at).

    R79: the generated operator-rules block (which duplicates dated OP-N rules already counted
    once, from operator-rules.md, by `lessons_at`) is excluded so it is never double-counted as
    undated. Detected by BEGIN/END markers first; where markers are absent, by exact text
    equality (after whitespace normalization) against a reference OP-N rule body -- NOT by
    resemblance, so a rule that has merely been paraphrased nearby still counts as the operator's
    own (dated-elsewhere but here undated) content.

    `op_rule_bodies` is not part of the brief's stated signature; it is an optional injection
    seam so tests can supply synthetic reference bodies instead of this repo's real
    docs/trackers/operator-rules.md. Real callers omit it and get `_default_op_rule_bodies()`.
    """
    path = pathlib.Path(path)
    text = path.read_text()
    if op_rule_bodies is None:
        op_rule_bodies = _default_op_rule_bodies()

    stripped, had_markers = _strip_marked_block(text)
    if not had_markers:
        stripped = _strip_equal_paragraphs(stripped, op_rule_bodies)

    lessons = []
    for lesson in _claude_md_lessons(stripped, str(path)):
        lessons.append(dataclasses.replace(lesson, dated=False))
    return lessons
