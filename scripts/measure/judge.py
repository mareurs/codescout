"""Task 9: the judge -- a Codex-backed labeller, and the validity gate it must pass first.

Spec: docs/superpowers/specs/2026-09-26-system1-base-rate-measurement-design.md § Judge protocol
(Codex), Amendment 1 (A1.3, A1.4) and Amendment 7 (b). Rulings R116-R126 (leakage, bounded
context, lesson dating, the Verdict extension, gate items from documents, the 9a/9b split, the
Codex channel, the lesson INDEX, run.py) are in
.superpowers/sdd/2026-09-26-system1-base-rate-measurement/task-9-context.md; fix round 1's R133-R142
(the jailed channel, the detectability definitions, retries, the live-gate refusals) are in
task-9-fix1-brief.md beside it.

- `lessons_for` (R118/R125): the lesson inventory frozen at a commit, each repo lesson dated by the
  AUTHOR date of the commit that first introduced its anchor line, kept iff dated strictly before
  the decision; the operator's global CLAUDE.md is always listed, `undated`.
- `build_input` (R116/R117/R120): one judge input, from an events DB (a session's preceding
  top-level turns, strictly before the decision) or from a document (a gate item, whose leakage
  protection is structural: the pre-correction blob).
- `Verdict`, `parse_verdict`, `verify_quote` (R119): the typed judge output. A missing or invalid
  field parses to None / "unknown" / {} and is FLAGGED, never defaulted.
- `judge` (R122/R137): three votes, majority per field, every vote, every attempt and the
  disagreement kept; `CodexChannel` (R133) runs codex in a bubblewrap jail that hides the
  repository, and refuses to start until a precondition shows the jail hides it.
- `gate_items`, `score_gate`, `run_gate` (R120/R121): the gate over the 21 RTD cases and the 52
  never-corrected controls, and its dry form (every input built and every prompt rendered, with
  no model call).

Task 9a makes NO model call. `judge` takes an injectable `complete(prompt, log_path)`; only when
none is given does it build the Codex channel, and the dry gate never does.

Run: ~/work/claude/prompt-engineering/.venv/bin/python -m pytest tests/test_measure_judge.py -v
"""
import collections
import concurrent.futures as cf
import dataclasses
import hashlib
import importlib.util
import json
import math
import os
import pathlib
import re
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import threading

_HERE = pathlib.Path(__file__).resolve().parent
REPO_ROOT = _HERE.parents[1]
if str(_HERE) not in sys.path:
    sys.path.insert(0, str(_HERE))
import join  # noqa: E402  (sibling module, per miner.py's idiom)
import lessons as _lessons  # noqa: E402

PROMPT_PATH = _HERE / "judge_prompt.md"
RTD_DOC = "docs/evals/rule-tell-detection.md"
CONTROLS_DOC = "docs/evals/rule-tell-controls.md"
# The operator's global CLAUDE.md (R118's undated source), derived at run time -- never a
# home-literal path in a tracked file (R39).
GLOBAL_CLAUDE_MD = pathlib.Path.home() / ".claude" / "CLAUDE.md"
# R122: the model pin is LOADED BY PATH from the rule-tell campaign, never copied. R133 amends
# R122's "through `codex_complete`": that function's channel can read the answer key, so the
# judge owns its channel (`CodexChannel`), and the campaign's files are left as they are.
_GS_PATH = REPO_ROOT / "docs/evals/data/2026-09-24-rule-tell/stage2/generate_synthetic.py"

MODES = ("correction", "audit")
DETECTABILITY = ("in-trace", "obtainable", "external")
TRI = ("yes", "no", "unknown")
OUTCOMES = ("applied", "missed")
LESSON_SENTINELS = ("uncovered", "abstain")
# Spec § Judge protocol: the gate's detectability mapping.
TEXT_DETECTABLE_TO_DETECTABILITY = {"yes": "in-trace", "partial": "obtainable", "no": "external"}

CONTEXT_MAX_TURNS = 12          # R117
CONTEXT_MAX_CHARS = 20_000      # R117
ORIGIN_CANDIDATES_MAX = 5       # R117
ORIGIN_CANDIDATE_CHARS = 400    # display cap per origin candidate (its full text is in context)
# R120: up to this many chars on EACH side of a document span -- a radius, not a total. Kept at
# 1,500 per side by R142 (review concern 1): under a 750-per-side reading RTD-15's falsifier
# would sit at the window's edge.
DOC_CONTEXT_CHARS = 1_500       # R120 / R142: per side
LESSON_INDEX_ENTRY_MAX = 300    # R124
MIN_QUOTE_CHARS = 12            # phase 1's verify_span MIN_SPAN: shorter "quotes" match anywhere
MAX_CONCURRENT_CALLS = 3        # R122
DATING_WORKERS = 8
CHARS_PER_TOKEN = 4             # the dry gate's token ESTIMATE (ceil(chars / 4)), not a count

# Spec § Judge protocol -- the gate's thresholds, fixed before any gate output exists.
GATE_DETECTABILITY_MIN = (16, 21)   # correction mode: detectability agrees with text_detectable
GATE_PEER_YES_MIN = (3, 4)          # audit mode: flags a mistake on the peer x yes cell
GATE_YES_MIN = (6, 8)               # audit mode: flags a mistake on the text_detectable yes column
GATE_CONTROL_FIRE_MAX = (5, 52)     # audit mode: fires on the never-corrected controls
SPEC_PEER_YES = ("RTD-3", "RTD-8", "RTD-9", "RTD-10")   # spec: the honest target cell
SPEC_NO_LAW = ("RTD-8", "RTD-9", "RTD-15")              # Amendment 7 (b)2: expected `uncovered`
GATE_POPULATION = {"peer_yes": SPEC_PEER_YES, "no_law": SPEC_NO_LAW}


class LeakageError(ValueError):
    """A judge input would carry material from at or after its decision (R116)."""


def _git(repo, *args):
    p = subprocess.run(["git", "-C", str(repo), *args], capture_output=True, text=True,
                       encoding="utf-8", errors="replace")
    if p.returncode != 0:
        raise RuntimeError(f"git {' '.join(args)} failed in {repo}: {p.stderr.strip()[:300]}")
    return p.stdout


def _resolve(repo, rev):
    return _git(repo, "rev-parse", "--verify", f"{rev}^{{commit}}").strip()


def _author_date(repo, sha):
    return _git(repo, "show", "-s", "--format=%aI", sha).strip()


def _norm(s):
    return re.sub(r"\s+", " ", s or "").strip()


def _sha256(path):
    return hashlib.sha256(pathlib.Path(path).read_bytes()).hexdigest()


# --- R118 / R125: dating a lesson by its anchor line ----------------------------------------------

# POSIX ERE metacharacters, escaped with a backslash, which ERE defines for exactly these and
# Python's `re` reads the same way. `]` and `}` are literal in both when unescaped (nothing opens
# them once `[` and `{` are escaped), so the one pattern is valid for `git log -G` and for `re`.
_ERE_META = frozenset("\\.[()*+?{|^$")


def anchor_regex(anchor, full_line):
    """R125: the anchor with each whitespace run as `\\s+` and metacharacters escaped. It is
    anchored at line start (leading whitespace allowed); a HEADING anchor also at line end, so
    `## Notes` never matches `## Notes on X`. A bullet or prose anchor is a prefix match, so a
    rewrap that moves words off the end of its first line keeps the original date."""
    core = r"\s+".join("".join("\\" + ch if ch in _ERE_META else ch for ch in tok)
                       for tok in anchor.split())
    return r"^\s*" + core + (r"\s*$" if full_line else "")


def date_anchor(repo, freeze_sha, path, regex, occurrence=1):
    """R125: the AUTHOR date (%aI) of the first commit reachable from `freeze_sha` whose version
    of `path` holds at least `occurrence` lines matching `regex`, else None.

    Candidates come from `git log --reverse -G<regex> <freeze_sha> -- <path>`: path-limited, so a
    line drafted elsewhere dates from its arrival in the lesson's own source, and a move dates at
    the move. `--topo-order` (added to the ruled command) keeps an ancestor ahead of its
    descendants whatever their committer dates say, which a rebase restamps. Counting copies at
    each candidate is what dates a repeated heading's copy n at its own introduction."""
    out = _git(repo, "log", "--topo-order", "--reverse", "--format=%H%x09%aI", f"-G{regex}",
               freeze_sha, "--", path)
    pat = re.compile(regex)
    for line in out.splitlines():
        if not line.strip():
            continue
        sha, adate = line.split("\t", 1)
        text = _lessons._git_show(repo, sha, path)
        if text is None:
            continue
        if sum(1 for ln in text.splitlines() if pat.search(ln)) >= occurrence:
            return adate.strip()
    return None


def _first_prose_line(lines, indices, fenced):
    """First non-blank line among `indices` that is not a heading (a fenced line is content)."""
    for i in indices:
        if lines[i].strip() and (fenced[i] or _lessons._heading_match(lines[i]) is None):
            return i
    return None


def _claude_md_anchor_lines(text, id_prefix):
    """{bare lesson id: (line index, is_heading)} for every lesson `lessons._claude_md_lessons`
    mints from `text`, mirroring its sectioning and slugging with lessons.py's own helpers, so the
    anchor comes from the SOURCE FILE (R125), never from `Lesson.text`. A bullet's anchor is its
    bold-lead line; a section-prose lesson's is its heading; the `-preamble` prose lesson's is its
    first non-blank non-heading line."""
    L = _lessons
    lines = text.splitlines()
    fenced = L._fence_flags(lines)
    heads = []
    for i, ln in enumerate(lines):
        hm = None if fenced[i] else L._heading_match(ln)
        if hm and 2 <= hm[0] <= 3:
            heads.append((i, hm[1]))
    out = {}
    used = set()

    def emit(slug, start, end, heading_idx):
        body_lines = lines[start:end]
        bold, prose = L._partition_bullets_and_prose("\n".join(body_lines))
        bfl = L._fence_flags(body_lines)
        starts = [j for j, ln in enumerate(body_lines) if not bfl[j] and ln.startswith("- **")]
        used_lead = set()
        continuation = set()
        for j, bullet_text in zip(starts, bold):
            lead = L._dedupe_slug(L._lead_slug(L._bullet_lead_source(bullet_text)), used_lead)
            out[f"{id_prefix}#{slug}/{lead}"] = (start + j, False)
            continuation.update(range(start + j, start + j + bullet_text.count("\n") + 1))
        if "\n".join(prose).strip():
            if heading_idx is not None:
                out[f"{id_prefix}#{slug}"] = (heading_idx, True)
            else:
                idx = _first_prose_line(
                    lines, [i for i in range(start, end) if i not in continuation], fenced)
                if idx is not None:
                    out[f"{id_prefix}#{slug}"] = (idx, False)

    pre_end = heads[0][0] if heads else len(lines)
    pre_bold, pre_prose = L._partition_bullets_and_prose("\n".join(lines[:pre_end]))
    if pre_bold or "\n".join(pre_prose).strip():
        emit(L._dedupe_slug("-preamble", used), 0, pre_end, None)
    for k, (i, heading_text) in enumerate(heads):
        end = heads[k + 1][0] if k + 1 < len(heads) else len(lines)
        emit(L._dedupe_slug(L._slugify(heading_text), used), i + 1, end, i)
    return out


def _memory_anchor_lines(text, id_prefix):
    """The same map for one `.codescout/memories/*.md` file (`lessons._memory_lessons`): a `##`
    section's anchor is its heading; the preamble's, and a section-less file's, is its first
    non-blank non-heading line."""
    L = _lessons
    lines = text.splitlines()
    fenced = L._fence_flags(lines)
    heads = []
    for i, ln in enumerate(lines):
        hm = None if fenced[i] else L._heading_match(ln)
        if hm and hm[0] == 2:
            heads.append((i, hm[1]))
    if not heads:
        idx = _first_prose_line(lines, range(len(lines)), fenced)
        if idx is None:
            idx = next((i for i, ln in enumerate(lines) if ln.strip()), None)
        return {} if idx is None else {id_prefix: (idx, False)}
    out = {}
    used = set()
    if L._memory_preamble_text(text):
        idx = _first_prose_line(lines, range(heads[0][0]), fenced)
        if idx is not None:
            out[f"{id_prefix}#{L._dedupe_slug('-preamble', used)}"] = (idx, False)
    for i, heading_text in heads:
        out[f"{id_prefix}#{L._dedupe_slug(L._slugify(heading_text), used)}"] = (i, True)
    return out


def _entry_anchor_lines(text, id_re, level, id_prefix):
    """The same map for an OP-N / R-N / T-N tracker (`lessons._parse_id_entries`): the entry's
    `## ID ...` heading line."""
    L = _lessons
    lines = text.splitlines()
    fenced = L._fence_flags(lines)
    used = set()
    out = {}
    for i, ln in enumerate(lines):
        hm = None if fenced[i] else L._heading_match(ln)
        if hm and hm[0] == level:
            m = id_re.match(hm[1])
            if m:
                out[f"{id_prefix}#{L._dedupe_slug(m.group(1), used)}"] = (i, True)
    return out


_ENTRY_SOURCES = {
    "docs/trackers/operator-rules.md": (_lessons._OP_ID_RE, 2, "operator-rules.md"),
    "docs/trackers/reconnaissance-patterns.md": (_lessons._R_ID_RE, 2, "reconnaissance-patterns.md"),
    "docs/trackers/tool-usage-patterns.md": (_lessons._T_ID_RE, 3, "tool-usage-patterns.md"),
}


def _anchor_lines_for(path, text):
    if path == "CLAUDE.md":
        return _claude_md_anchor_lines(text, "CLAUDE.md")
    if path in _ENTRY_SOURCES:
        return _entry_anchor_lines(text, *_ENTRY_SOURCES[path])
    prefix = _lessons._MEMORIES_PREFIX
    if path.startswith(prefix) and path.endswith(".md"):
        return _memory_anchor_lines(text, "memory:" + path[len(prefix):-len(".md")])
    return {}


def _lesson_anchor(repo, sha, lesson, file_cache):
    """{path, anchor, regex, occurrence, full_line} for a repo lesson at `sha`, or None when its
    anchor cannot be derived from its source file. `occurrence` is the anchor line's ordinal among
    the file's lines matching its regex, which is how a repeated heading's copy is told apart."""
    repo_name, sep, path = lesson.source.partition(":")
    if not sep or not lesson.id.startswith(repo_name + ":"):
        return None
    bare = lesson.id[len(repo_name) + 1:]
    if path not in file_cache:
        text = _lessons._git_show(repo, sha, path)
        file_cache[path] = (None, {}) if text is None else (text.splitlines(),
                                                             _anchor_lines_for(path, text))
    lines, amap = file_cache[path]
    if bare not in amap:
        return None
    idx, full_line = amap[bare]
    anchor = lines[idx].strip()
    if not anchor:
        return None
    regex = anchor_regex(anchor, full_line)
    pat = re.compile(regex)
    occurrence = sum(1 for ln in lines[:idx + 1] if pat.search(ln))
    return {"path": path, "anchor": anchor, "regex": regex, "occurrence": occurrence,
            "full_line": full_line}


# R118: dates are cached per (repo, freeze sha, lesson id).
_DATE_CACHE = {}
_DATE_LOCK = threading.Lock()


def _dates_for(repo, sha, repo_lessons):
    rkey = str(pathlib.Path(repo).resolve())
    file_cache = {}
    anchors = {l.id: _lesson_anchor(repo, sha, l, file_cache) for l in repo_lessons}
    with _DATE_LOCK:
        todo = [l for l in repo_lessons if (rkey, sha, l.id) not in _DATE_CACHE]

    def work(lesson):
        a = anchors[lesson.id]
        date = None if a is None else date_anchor(repo, sha, a["path"], a["regex"], a["occurrence"])
        return lesson.id, date

    with cf.ThreadPoolExecutor(max_workers=DATING_WORKERS) as ex:
        found = list(ex.map(work, todo))
    with _DATE_LOCK:
        for lid, date in found:
            _DATE_CACHE[(rkey, sha, lid)] = date
        dates = {l.id: _DATE_CACHE[(rkey, sha, l.id)] for l in repo_lessons}
    return anchors, dates


_SENTENCE_END = re.compile(r"[.!?][*_\"'`)\]”’]*(?=\s|$)")


def _first_sentence(text):
    flat = _norm(text)
    m = _SENTENCE_END.search(flat)
    return flat[:m.end()] if m else flat


def _index_entry(lesson, status, anchor, date, undatable=False):
    """One lesson as the judge sees it (R124): id, source, dated|undated, anchor line and first
    sentence -- never the full text."""
    lines = lesson.text.splitlines()
    if anchor is not None:
        anchor_line, is_heading = anchor["anchor"], anchor["full_line"]
    else:  # undated (global CLAUDE.md) or undatable: shown from its own first line
        anchor_line = next((ln.strip() for ln in lines if ln.strip()), "")
        is_heading = _lessons._heading_match(anchor_line) is not None
    # A heading-anchored lesson's text opens with its heading, or with its bare entry id, so its
    # first sentence is its body's; and a heading line (a preamble's `# Title`) is no sentence.
    body_lines = lines[1:] if is_heading else lines
    fenced = _lessons._fence_flags(body_lines)
    body = "\n".join(ln for ln, f in zip(body_lines, fenced)
                     if f or _lessons._heading_match(ln) is None)
    return {"id": lesson.id, "source": lesson.source, "status": status, "date": date,
            "anchor": anchor_line, "first_sentence": _first_sentence(body),
            "undatable": undatable}


def lessons_for(repo, freeze_sha, decision_ts, global_claude_md=GLOBAL_CLAUDE_MD, stats=None):
    """R118: the lessons a decision at `decision_ts` could have applied, as index entries.

    `lessons_at(freeze_sha)`, each dated by R125 and kept iff dated STRICTLY before
    `decision_ts` (compared with `join.utc`), marked `dated`. A repo lesson whose date cannot be
    derived is listed `undated` and counted in `stats["lesson_undatable"]`. Then every lesson of
    the operator's global CLAUDE.md, via `undated_lessons` at the same freeze sha (R101), listed
    `undated`; `global_claude_md=None` omits them (fixtures only)."""
    repo = pathlib.Path(repo)
    sha = _resolve(repo, freeze_sha)
    cutoff = join.utc(decision_ts)
    if stats is None:
        stats = {}
    for key in ("lesson_dated", "lesson_undatable", "lesson_after_decision", "lesson_global"):
        stats.setdefault(key, 0)
    repo_lessons = _lessons.lessons_at(repo, sha)
    anchors, dates = _dates_for(repo, sha, repo_lessons)
    out = []
    for lesson in repo_lessons:
        date = dates[lesson.id]
        if date is None:
            stats["lesson_undatable"] += 1
            out.append(_index_entry(lesson, "undated", anchors[lesson.id], None, undatable=True))
        elif join.utc(date) < cutoff:
            stats["lesson_dated"] += 1
            out.append(_index_entry(lesson, "dated", anchors[lesson.id], date))
        else:
            stats["lesson_after_decision"] += 1
    if global_claude_md is not None:
        for lesson in _lessons.undated_lessons(global_claude_md, repo, sha):
            stats["lesson_global"] += 1
            out.append(_index_entry(lesson, "undated", None, None))
    return out


def _index_desc(entry):
    a = _norm(entry["anchor"])
    s = entry["first_sentence"]
    if not s or s in a:
        return a
    if a and a in s:
        return s
    return f"{a} — {s}" if a else s


def lesson_index_line(entry):
    """≤ LESSON_INDEX_ENTRY_MAX chars: the description is cut to fit; the id never is."""
    head = f"- {entry['id']} ({entry['source']}; {entry['status']}): "
    desc = _index_desc(entry)
    room = LESSON_INDEX_ENTRY_MAX - len(head)
    if len(desc) > room:
        desc = desc[:max(room - 1, 0)].rstrip() + "…"
    return head + desc


def render_lesson_index(entries):
    return "\n".join(lesson_index_line(e) for e in entries)


# --- R116 / R117 / R120: one judge input ----------------------------------------------------------


def _session_part(events_db, item, mode):
    """R117: the same session's preceding TOP-LEVEL turns (rowid = transcript order), newest
    first, up to 12 turns and 20,000 chars, dropping the oldest first; in correction mode also up
    to 5 preceding top-level `assistant_text` turns as origin candidates. R116: every included
    turn must be STRICTLY before the decision (join.utc), else LeakageError -- a guard on what was
    selected by order, never a filter that would hide the violation."""
    cutoff = join.utc(item["decision_ts"])
    uri = pathlib.Path(events_db).resolve().as_uri() + "?mode=ro"
    conn = sqlite3.connect(uri, uri=True)
    try:
        rows = conn.execute(
            "SELECT uuid, ts, role, kind, text FROM turns WHERE sid = ? AND agent_path IS NULL "
            "ORDER BY rowid", (item["sid"],)).fetchall()
    finally:
        conn.close()
    pos = [i for i, r in enumerate(rows) if r[0] == item["decision_uuid"]]
    if len(pos) != 1:
        raise ValueError(f"decision turn {item['decision_uuid']!r} is not exactly one top-level "
                         f"turn of session {item['sid']!r} ({len(pos)} found)")
    k = pos[0]
    d_ts, d_text = rows[k][1], rows[k][4] or ""
    if d_ts is None or join.utc(d_ts) != cutoff:
        raise ValueError(f"item decision_ts {item['decision_ts']!r} disagrees with the events db "
                         f"({d_ts!r}) for turn {item['decision_uuid']!r}")
    preceding = rows[:k]

    context = []
    used = 0
    for uuid, ts, role, kind, text in reversed(preceding):
        if len(context) >= CONTEXT_MAX_TURNS:
            break
        text = text or ""
        if used + len(text) > CONTEXT_MAX_CHARS:
            if not context:  # the newest turn alone is over budget: keep its newest part
                marker = "[… the earlier part of this turn is not shown]\n"
                text = marker + text[-(CONTEXT_MAX_CHARS - len(marker)):]
                context.append({"uuid": uuid, "ts": ts, "role": role, "kind": kind, "text": text})
            break
        used += len(text)
        context.append({"uuid": uuid, "ts": ts, "role": role, "kind": kind, "text": text})

    candidates = []
    if mode == "correction":
        for uuid, ts, _role, kind, text in reversed(preceding):
            if kind == "assistant_text":
                candidates.append({"uuid": uuid, "ts": ts,
                                   "text": (text or "")[:ORIGIN_CANDIDATE_CHARS]})
                if len(candidates) >= ORIGIN_CANDIDATES_MAX:
                    break

    for turn in context + candidates:
        if turn["ts"] is None or join.utc(turn["ts"]) >= cutoff:
            raise LeakageError(
                f"R116: turn {turn['uuid']!r} at {turn['ts']!r} is not strictly before the "
                f"decision at {item['decision_ts']!r}")
    pre = "\n\n".join([t["text"] for t in reversed(context)] + [c["text"] for c in candidates]
                      + [d_text])
    return {"decision": d_text, "context": context, "origin_candidates": candidates,
            "pre_evidence": pre,
            "leakage_guard": "timestamps: every included turn strictly before decision_ts"}


def _document_part(item):
    """R120: a gate item is built from documents and carries NO timestamps. Its leakage
    protection is STRUCTURAL: every excerpt was read from the pre-correction blob its `document`
    names by (sha, path), where the correction does not yet exist. What can be checked here is
    checked: the blob provenance is present, and no text the item lists in `must_not_contain`
    (its own correction) appears anywhere in its context."""
    doc = item.get("document") or {}
    if not doc.get("sha") or not doc.get("path"):
        raise LeakageError(f"R120: document item {item.get('id')!r} names no blob (sha, path), so "
                           "nothing shows its context predates its correction")
    excerpts = list(doc.get("excerpts") or [])
    joined = _norm("\n".join(excerpts))
    for text in item.get("must_not_contain") or []:
        probe = _norm(text)
        if probe and probe in joined:
            raise LeakageError(f"R120: document item {item.get('id')!r} carries its own correction "
                               f"in its context ({doc['sha'][:12]}:{doc['path']})")
    n = len(excerpts)
    context = [{"label": f"excerpt {i + 1} of {n} from {doc['path']}", "text": ex}
               for i, ex in enumerate(excerpts)]
    return {"decision": item["decision"], "context": context, "origin_candidates": [],
            "pre_evidence": "\n\n".join(excerpts + [item["decision"]]),
            "leakage_guard": f"structural: pre-correction blob {doc['sha']}:{doc['path']}"}


def build_input(item, events_db, lessons, mode):
    """One judge input: decision, context, (correction mode) origin candidates and correction,
    the lesson index, `pre_evidence` (what `verify_quote` checks), and the rendered prompt.

    R116: audit mode refuses a correction field; the correction is a separate labelled field and
    is never part of `pre_evidence`. `pre_evidence` is the context, the origin candidates and the
    decision's own text (its own words can show its problem, as `in-trace` allows). Session items
    (`events_db` given) are guarded by timestamps; document items (`events_db` None, the gate's)
    structurally, by the pre-correction blob -- see `_document_part`."""
    if mode not in MODES:
        raise ValueError(f"mode must be one of {MODES}, not {mode!r}")
    correction = item.get("correction")
    if mode == "audit" and correction is not None:
        raise LeakageError(f"R116: item {item.get('id')!r} carries a correction in audit mode")
    if mode == "correction" and not (isinstance(correction, str) and correction.strip()):
        raise ValueError(f"item {item.get('id')!r}: correction mode needs the correction text")
    part = (_session_part(events_db, item, mode) if events_db is not None
            else _document_part(item))
    lessons = list(lessons)
    inp = {
        "id": item["id"], "mode": mode, "decision_ts": item.get("decision_ts"),
        "decision": part["decision"], "context": part["context"],
        "origin_candidates": part["origin_candidates"],
        "offered_uuids": [c["uuid"] for c in part["origin_candidates"]],
        "correction": correction if mode == "correction" else None,
        "lessons": lessons, "offered_lessons": [e["id"] for e in lessons],
        "pre_evidence": part["pre_evidence"], "leakage_guard": part["leakage_guard"],
    }
    inp["prompt"] = render_prompt(inp)
    return inp


_MODE_BLOCK = re.compile(r"<!-- mode: (correction|audit) -->\n?(.*?)<!-- end mode -->\n?", re.S)
_COMMENT = re.compile(r"<!--.*?-->\n?", re.S)
_PLACEHOLDER = re.compile(r"\{\{([A-Z_]+)\}\}")


def _data(text):
    """Material shown between delimiter lines: a line of its own that opens like a delimiter
    gets a leading space, which `verify_quote`'s whitespace normalisation cannot see."""
    return re.sub(r"(?m)^=====", " =====", text)


def _render_context(inp):
    ctx = inp["context"]
    if not ctx:
        return "(none)"
    if "label" in ctx[0]:  # document excerpts
        return "\n\n".join(f"[{c['label']}]\n{_data(c['text'])}" for c in ctx)
    n = len(ctx)
    return "\n\n".join(  # stored newest first (R117); shown oldest first
        f"[turn -{n - i} | {c['ts']} | {c['role']}/{c['kind']} | uuid {c['uuid']}]\n{_data(c['text'])}"
        for i, c in enumerate(reversed(ctx)))


def _render_candidates(inp):
    if not inp["origin_candidates"]:
        return "(none offered: the decision point shown is the origin)"
    return "\n\n".join(f"[uuid {c['uuid']} | {c['ts']}]\n{_data(c['text'])}"
                       for c in inp["origin_candidates"])


def render_prompt(inp, template=None):
    t = PROMPT_PATH.read_text() if template is None else template
    t = _MODE_BLOCK.sub(lambda m: m.group(2) if m.group(1) == inp["mode"] else "", t)
    t = _COMMENT.sub("", t)
    values = {
        "LESSON_COUNT": str(len(inp["lessons"])),
        "LESSON_INDEX": _data(render_lesson_index(inp["lessons"])) or "(no lessons)",
        "CONTEXT": _render_context(inp),
        "DECISION": _data(inp["decision"]),
        "ORIGIN_CANDIDATES": _render_candidates(inp),
        "CORRECTION": _data(inp["correction"] or ""),
    }

    def fill(m):
        if m.group(1) not in values:
            raise KeyError(f"judge_prompt.md names an unknown placeholder {m.group(0)}")
        return values[m.group(1)]

    return _PLACEHOLDER.sub(fill, t)  # one pass: filled-in material is never re-scanned


# --- R119: the Verdict ----------------------------------------------------------------------------


@dataclasses.dataclass
class Verdict:
    mode: str | None = None
    is_correction: bool | None = None       # correction mode
    origin_uuid: str | None = None          # correction mode: one of the offered uuids, or None
    is_decision_point: bool | None = None
    is_mistake: bool | None = None          # audit mode: True = "flags a mistake" / "fires"
    lessons: list | str | None = None       # [ids] | "uncovered" | "abstain"
    detectability: str | None = None        # in-trace | obtainable | external
    quote: str = ""
    evidence_present_before: str = "unknown"  # audit mode (A1.3)
    evidence_used: str = "unknown"            # audit mode (A1.3)
    lesson_outcomes: dict = dataclasses.field(default_factory=dict)  # audit mode (A1.4)
    flags: list = dataclasses.field(default_factory=list)


_MODE_FIELDS = {
    "correction": ("is_correction", "origin_uuid", "is_decision_point", "lessons",
                   "detectability", "quote"),
    "audit": ("is_decision_point", "is_mistake", "lessons", "lesson_outcomes", "detectability",
              "evidence_present_before", "evidence_used", "quote"),
}


def _extract_json(raw):
    s = (raw or "").strip()
    s = re.sub(r"^```(?:json)?\s*", "", s)
    s = re.sub(r"\s*```$", "", s)
    candidates = [s] + [ln.strip() for ln in s.splitlines()]
    i, j = s.find("{"), s.rfind("}")
    if 0 <= i < j:
        candidates.append(s[i:j + 1])
    for c in candidates:
        try:
            obj = json.loads(c)
        except ValueError:
            continue
        if isinstance(obj, dict):
            return obj
    return None


def parse_verdict(raw, mode=None, offered_uuids=None, offered_lessons=None):
    """R119: parse one judge reply. Every field the mode's schema names is read; one that is
    missing parses to None / "unknown" / {} and is flagged `missing:<field>`, and one with a value
    outside its type is flagged `invalid:<field>` -- never defaulted to yes / applied / True. A
    `null` boolean is the prompt's abstention (R139): None, flagged `abstain:<field>`.
    `origin_uuid` must be one of `offered_uuids` (none offered = none valid), else None. With
    `offered_lessons`, a lesson id outside the index is dropped and flagged; a list left empty by
    that becomes "abstain". `mode=None` reads both modes' fields."""
    obj = _extract_json(raw)
    if obj is None:
        return Verdict(mode=mode, flags=["unparseable"])
    fields = set()
    for m in ((mode,) if mode else MODES):
        fields.update(_MODE_FIELDS[m])
    flags = []
    v = Verdict(mode=mode, flags=flags)
    offered_l = None if offered_lessons is None else set(offered_lessons)

    def present(f):
        if f not in obj:
            flags.append(f"missing:{f}")
            return False
        return True

    for f in ("is_correction", "is_decision_point", "is_mistake"):
        if f in fields and present(f):
            if isinstance(obj[f], bool):
                setattr(v, f, obj[f])
            elif obj[f] is None:  # R139: the prompt's abstention, never a type error
                flags.append(f"abstain:{f}")
            else:
                flags.append(f"invalid:{f}")

    if "origin_uuid" in fields and present("origin_uuid"):
        x = obj["origin_uuid"]
        if x is not None:
            if isinstance(x, str) and x in (offered_uuids or ()):
                v.origin_uuid = x
            else:
                flags.append("origin_uuid_not_offered")

    if "lessons" in fields and present("lessons"):
        x = obj["lessons"]
        if isinstance(x, str) and x in LESSON_SENTINELS:
            v.lessons = x
        elif isinstance(x, list) and all(isinstance(i, str) for i in x):
            keep = []
            for lid in x:
                if offered_l is not None and lid not in offered_l:
                    flags.append(f"unknown_lesson:{lid}")
                elif lid not in keep:
                    keep.append(lid)
            v.lessons = "abstain" if (x and not keep) else keep
        else:
            flags.append("invalid:lessons")

    if "detectability" in fields and present("detectability"):
        x = obj["detectability"]
        if x in DETECTABILITY:
            v.detectability = x
        elif x is None:
            if v.is_correction is True or v.is_mistake is True:
                flags.append("null:detectability")
        else:
            flags.append("invalid:detectability")

    for f in ("evidence_present_before", "evidence_used"):
        if f in fields and present(f):
            if obj[f] in TRI:
                setattr(v, f, obj[f])
            else:
                flags.append(f"invalid:{f}")

    if "lesson_outcomes" in fields and present("lesson_outcomes"):
        x = obj["lesson_outcomes"]
        if isinstance(x, dict):
            outcomes = {}
            for lid, val in x.items():
                if offered_l is not None and lid not in offered_l:
                    flags.append(f"unknown_lesson:{lid}")
                elif val in OUTCOMES:
                    outcomes[lid] = val
                else:
                    flags.append(f"invalid:lesson_outcomes:{lid}")
            v.lesson_outcomes = outcomes
        else:
            flags.append("invalid:lesson_outcomes")

    if "quote" in fields and present("quote"):
        if isinstance(obj["quote"], str):
            v.quote = obj["quote"]
        else:
            flags.append("invalid:quote")
    return v


def _verified_span(quote, evidence):
    """Phase 1's `verify_span` idea (scripts/phase1-span-selector.py): only whitespace is
    normalised and surrounding quotes / backticks / emphasis stripped; nothing else is fuzzy."""
    c = _norm(quote)
    for _ in range(3):
        c = c.strip().strip("*_").strip().strip("\"'`“”‘’").strip()
    if len(c) < MIN_QUOTE_CHARS:
        return None
    return c if c in _norm(evidence) else None


def verify_quote(v, pre_evidence):
    """Spec § Judge protocol: an `in-trace` verdict whose quote is not verbatim in the
    pre-decision evidence becomes abstain -- `lessons="abstain"`, detectability None, flagged
    `quote_not_verbatim`. Other verdicts pass through unchanged."""
    if v.detectability != "in-trace" or _verified_span(v.quote, pre_evidence) is not None:
        return v
    return dataclasses.replace(v, lessons="abstain", detectability=None,
                               flags=list(v.flags) + ["quote_not_verbatim"])


# --- R122: the Codex channel, and the vote --------------------------------------------------------


_GS = None


def _gs():
    global _GS
    if _GS is None:
        spec = importlib.util.spec_from_file_location("generate_synthetic", _GS_PATH)
        mod = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(mod)
        _GS = mod
    return _GS


CODEX_CLI_VERSION = "codex-cli 0.154.0"  # R138: a live gate refuses any other `codex --version`
GATE_VOTES = 3                           # R138: a live gate refuses any other vote count
MAX_ATTEMPTS_PER_VOTE = 3                # R137: the first attempt and up to 2 re-issues
RETRY_FAILURES = ("call_failed", "unparseable", "tool_call")  # R137: the kinds re-issued
CODEX_CALL_TIMEOUT = 1200                # seconds per `codex exec` (the campaign's value)
PRECONDITION_TIMEOUT = 120               # seconds per `codex sandbox ... test -e`
_STRIPPED_ENV = ("OPENAI_API_KEY", "CODEX_API_KEY", "OPENAI_BASE_URL")  # subscription only
# R133(b)(c): every flag of the `codex exec` call. The prompt is not an argument: the rendered
# frozen file goes on stdin (the `-` PROMPT), with no wrapper instruction, so it is the whole
# user message.
_CODEX_EXEC_FLAGS = ("--skip-git-repo-check", "--sandbox", "read-only", "-c",
                     "approval_policy=never", "--ephemeral", "--json", "--strict-config")
# R133(d): appended to the fresh CODEX_HOME's config.toml. Both keys were measured recognised by
# codex-cli 0.154.0 under --strict-config (a bogus key in the same position fails with "unknown
# configuration field"), and `codex features list` reads `in_app_updates` false with it.
_CODEX_CONFIG_EXTRA = 'forced_login_method = "chatgpt"\n\n[features]\nin_app_updates = false\n'
# R133(g): the `--json` events that are no exec, tool or file read. codex-cli 0.154.0 emits
# `thread.started`, `turn.started|completed|failed`, `error`, and `item.started|updated|completed`
# whose `item.type` names the item. Anything else -- a command execution, a file change, an MCP,
# collaboration or web-search call, a plan update, an event or item type these lists do not
# know, or a line that is not a JSON object -- makes the vote a `tool_call` failure: fail closed.
_JSON_EVENTS_OK = frozenset({"thread.started", "turn.started", "turn.completed", "turn.failed",
                             "error"})
_JSON_ITEM_EVENTS = frozenset({"item.started", "item.updated", "item.completed"})
_JSON_ITEMS_OK = frozenset({"agent_message", "reasoning", "error"})


class ToolCallError(RuntimeError):
    """R133(g): a vote's `--json` stream shows an exec, tool or file-read event. Carries counts by
    kind only, never an event's content."""

    def __init__(self, counts):
        self.counts = dict(counts)
        super().__init__(json.dumps(self.counts, sort_keys=True))


class JailError(RuntimeError):
    """R133(f): the judge's jail does not hide what it must, or cannot see its own workdir."""


def tool_events(stdout):
    """Counts, by kind, of the `--json` events in `stdout` that the allow-lists above do not
    admit; {} for a vote that used no tool."""
    bad = collections.Counter()
    for line in (stdout or "").splitlines():
        if not line.strip():
            continue
        try:
            ev = json.loads(line)
        except ValueError:
            ev = None
        if not isinstance(ev, dict):
            bad["unparsed_line"] += 1
            continue
        kind = ev.get("type")
        if kind in _JSON_EVENTS_OK:
            continue
        if kind in _JSON_ITEM_EVENTS:
            item = ev.get("item")
            itype = item.get("type") if isinstance(item, dict) else None
            if itype not in _JSON_ITEMS_OK:
                bad[f"item:{itype}"] += 1
            continue
        bad[f"event:{kind}"] += 1
    return dict(bad)


def jail_argv(home_dir, uid, rw_paths):
    """R133(e): the bubblewrap prefix of every codex process the channel starts. The filesystem
    is read-only, the home directory, /tmp and /run/user/<uid> are empty tmpfs mounts (hiding the
    repository with its answer keys and history, every Claude profile, and session scratch
    copies), and exactly `rw_paths` are bound back read-write at their own paths. Measured on
    codex-cli 0.154.0: codex still starts with /run/user/<uid> hidden, so it is hidden."""
    argv = ["bwrap", "--die-with-parent", "--ro-bind", "/", "/", "--dev", "/dev", "--proc", "/proc",
            "--tmpfs", str(home_dir), "--tmpfs", "/tmp", "--tmpfs", f"/run/user/{uid}"]
    for p in rw_paths:
        argv += ["--bind", str(p), str(p)]
    return argv + ["--"]


def codex_exec_argv(work):
    """R133(a)-(c): one vote's `codex exec`; `work` is an empty directory."""
    work = pathlib.Path(work)
    return ["codex", "exec", *_CODEX_EXEC_FLAGS, "-o", str(work / "last.txt"), "-C", str(work), "-"]


def precondition_argv(path):
    """R133(f): a local command under Codex's own read-only sandbox, never a model call."""
    return ["codex", "sandbox", "-c", "sandbox_mode=read-only", "--", "test", "-e", str(path)]


def describe_channel(argv):
    """The header's channel description, derived from an argv the builders above produced (R138):
    never a hand-written string that could drift from what runs."""
    extra = " ".join(_CODEX_CONFIG_EXTRA.split())
    return (" ".join(argv) + " (prompt on stdin); env: CODEX_HOME=<CODEX_HOME>, without "
            + "/".join(_STRIPPED_ENV) + "; config.toml: model, model_reasoning_effort, " + extra)


def _placeholder_argv():
    work = pathlib.Path("<workdir>")
    return (jail_argv("<home>", "<uid>", ("<CODEX_HOME>", "<auth.json target>", work))
            + codex_exec_argv(work))



def _codex_version(runner=None):
    """`codex --version` for every output header (R122). A version query, not a model call."""
    try:
        p = (runner or subprocess.run)(["codex", "--version"], capture_output=True, text=True,
                                       timeout=30, stdin=subprocess.DEVNULL)
    except (OSError, subprocess.TimeoutExpired) as exc:
        return f"unavailable ({type(exc).__name__})"
    if p.returncode != 0:
        return f"unavailable (exit {p.returncode})"
    text = (p.stdout.strip() or p.stderr.strip())
    return text.splitlines()[0] if text else "?"


def new_codex_home(model, effort, auth_source=None):
    """`run_labellers.new_codex_home`'s pattern: a fresh CODEX_HOME holding only an auth.json
    symlink (to `auth_source`, default `~/.codex/auth.json`) and a config.toml that pins the model
    and effort; R133(d) appends `_CODEX_CONFIG_EXTRA` (subscription login only, no in-app
    updates)."""
    home = pathlib.Path(tempfile.mkdtemp(prefix="codex-judge-home-"))
    src = pathlib.Path.home() / ".codex" / "auth.json" if auth_source is None else auth_source
    # Linked to the RESOLVED file, which is what the jail binds back (R133 e): a link to a link
    # would dangle inside the jail, whose home directory is an empty tmpfs.
    (home / "auth.json").symlink_to(os.path.realpath(src))
    (home / "config.toml").write_text(
        f'model = "{model}"\nmodel_reasoning_effort = "{effort}"\n' + _CODEX_CONFIG_EXTRA)
    return home


class CodexChannel:
    """R122 / R133: the judge's own Codex channel. One fresh CODEX_HOME per run (the model and
    effort imported from generate_synthetic.py), subscription only (the API-key variables are
    stripped and `forced_login_method` is set), and every codex process inside `jail_argv`'s
    bubblewrap jail with only the CODEX_HOME, the file its auth link resolves to, and one empty
    workdir bound back. Constructing it RUNS THE JAIL PRECONDITION (`check_jail`), so no vote can
    be cast on a channel whose jail has not been shown to hide the answers. `runner` stands in for
    `subprocess.run` (tests); `home_dir` and `uid` default to the running user's."""

    def __init__(self, runner=None, home_dir=None, uid=None, repo=REPO_ROOT):
        gs = _gs()
        self.model, self.effort = gs.CODEX_MODEL, gs.CODEX_EFFORT
        self._run = runner or subprocess.run
        self.home_dir = pathlib.Path.home() if home_dir is None else pathlib.Path(home_dir)
        self.uid = os.getuid() if uid is None else uid
        self.repo = pathlib.Path(repo)
        self.version = _codex_version(self._run)
        self.home = new_codex_home(self.model, self.effort,
                                   auth_source=self.home_dir / ".codex" / "auth.json")
        try:
            # R133(e): resolved at run time and bound at its own path, so a token refresh can
            # write the real file. Bound, never read.
            self.auth = pathlib.Path(os.path.realpath(self.home / "auth.json"))
            self.precondition = self.check_jail()
        except BaseException:
            self.close()
            raise

    def _env(self):
        env = {k: v for k, v in os.environ.items() if k not in _STRIPPED_ENV}
        env["CODEX_HOME"] = str(self.home)
        return env

    def jail(self, work):
        return jail_argv(self.home_dir, self.uid, (self.home, self.auth, work))

    def hidden_paths(self):
        """R133(f): what the jail must hide -- the repository, the answer key, every Claude
        profile, and the session scratch root."""
        return [self.repo, self.repo / RTD_DOC, self.home_dir / ".claude",
                self.home_dir / ".claude-sdd", self.home_dir / ".claude-kat",
                pathlib.Path(f"/tmp/claude-{self.uid}")]

    def check_jail(self):
        """R133(f): inside the same jail a vote runs in, `test -e` under Codex's read-only
        sandbox must give 1 (absent) for every `hidden_paths` entry and 0 for the workdir;
        anything else raises JailError naming the path. Returns the rows (path, rc, expect)."""
        work = pathlib.Path(tempfile.mkdtemp(prefix="codex-judge-work-"))
        try:
            rows = []
            for path, expect in [(p, 1) for p in self.hidden_paths()] + [(work, 0)]:
                p = self._run(self.jail(work) + precondition_argv(path), capture_output=True,
                              text=True, timeout=PRECONDITION_TIMEOUT, env=self._env(),
                              stdin=subprocess.DEVNULL)
                rows.append({"path": str(path), "rc": p.returncode, "expect": expect})
            bad = [r for r in rows if r["rc"] != r["expect"]]
            if bad:
                raise JailError("R133: the judge's jail refuses to start: " + "; ".join(
                    f"{r['path']} gave rc {r['rc']}, must give {r['expect']} "
                    f"({'hidden' if r['expect'] else 'visible'})" for r in bad))
            return rows
        finally:
            shutil.rmtree(work, ignore_errors=True)

    def describe(self):
        """`describe_channel` over the argv this channel builds, its paths shown as labels."""
        work = pathlib.Path("<workdir>")
        labels = {str(self.home_dir): "<home>", str(self.home): "<CODEX_HOME>",
                  str(self.auth): "<auth.json target>", f"/run/user/{self.uid}": "/run/user/<uid>"}
        return describe_channel([labels.get(a, a) for a in self.jail(work) + codex_exec_argv(work)])

    def complete(self, prompt, log_path=None):
        """One vote: the rendered prompt on stdin, `--json` stdout to `log_path` (with stderr).
        Raises ToolCallError when the stream shows a tool event, RuntimeError on a non-zero exit;
        returns the last agent message."""
        work = pathlib.Path(tempfile.mkdtemp(prefix="codex-judge-work-"))
        try:
            p = self._run(self.jail(work) + codex_exec_argv(work), input=prompt,
                          capture_output=True, text=True, timeout=CODEX_CALL_TIMEOUT,
                          env=self._env())
            if log_path is not None:  # R133(h): under --log-dir only, and read by counts only
                pathlib.Path(log_path).write_text(p.stdout + "\n--- stderr ---\n" + p.stderr)
            bad = tool_events(p.stdout)
            if bad:
                raise ToolCallError(bad)
            if p.returncode != 0:
                raise RuntimeError(f"codex exec exit {p.returncode} (its log holds the rest)")
            return (work / "last.txt").read_text()
        finally:
            shutil.rmtree(work, ignore_errors=True)

    def close(self):
        shutil.rmtree(self.home, ignore_errors=True)


_CHANNEL = None


def codex_channel():
    global _CHANNEL
    if _CHANNEL is None:
        _CHANNEL = CodexChannel()
    return _CHANNEL


_MAJORITY_FIELDS = ("is_correction", "origin_uuid", "is_decision_point", "is_mistake", "lessons",
                    "detectability", "evidence_present_before", "evidence_used")


def _canon(x):
    return json.dumps(sorted(x) if isinstance(x, list) else x, sort_keys=True)


def _majority(verdicts):
    """Per field, the value a STRICT majority of votes gives, else None ("abstain" for lessons);
    `lesson_outcomes` per lesson id. `disagreement` is each field's count of distinct values."""
    n = len(verdicts)
    majority, disagreement = {"mode": verdicts[0].mode if verdicts else None}, {}
    for f in _MAJORITY_FIELDS:
        counts = collections.Counter(_canon(getattr(v, f)) for v in verdicts)
        disagreement[f] = len(counts)
        top, k = counts.most_common(1)[0] if counts else ("null", 0)
        majority[f] = json.loads(top) if 2 * k > n else ("abstain" if f == "lessons" else None)
    per = collections.defaultdict(collections.Counter)
    for v in verdicts:
        for lid, outcome in v.lesson_outcomes.items():
            per[lid][outcome] += 1
    majority["lesson_outcomes"] = {lid: c.most_common(1)[0][0] for lid, c in per.items()
                                   if 2 * c.most_common(1)[0][1] > n}
    disagreement["lesson_outcomes"] = len({_canon(v.lesson_outcomes) for v in verdicts})
    majority["quote"] = next((v.quote for v in verdicts
                              if v.detectability == majority["detectability"] and v.quote), "")
    majority["flags"] = sorted({f for v in verdicts for f in v.flags})
    return majority, disagreement


def _failure(v):
    """R137: the failure kind (one of RETRY_FAILURES) a vote attempt carries, or None."""
    for f in v.flags:
        kind = f.split(":", 1)[0]
        if kind in RETRY_FAILURES:
            return kind
    return None


def judge(item, votes=3, complete=None, log_dir=None):
    """R122: `votes` votes on `item` (a `build_input` result), at most 3 concurrently; every
    attempt has its own log path, is parsed (R119) and quote-checked, and is kept with its raw
    reply, beside the per-field majority and disagreement. R137, pre-registered: an attempt that
    is `call_failed`, `unparseable` or `tool_call` is re-issued, up to MAX_ATTEMPTS_PER_VOTE
    attempts in all; the vote is the first attempt that is none of those, and a vote whose every
    attempt failed stays failed. `complete(prompt, log_path) -> str` is injectable; the default is
    the Codex channel."""
    if complete is None:
        complete = codex_channel().complete
    if log_dir is not None:
        log_dir = pathlib.Path(log_dir)
        log_dir.mkdir(parents=True, exist_ok=True)
    safe = re.sub(r"[^A-Za-z0-9._-]+", "_", item["id"])

    def attempt(k, a):
        log_path = None if log_dir is None else log_dir / f"{safe}.vote{k + 1}.try{a + 1}.log"
        try:
            raw = complete(item["prompt"], log_path)
        except ToolCallError as exc:  # R133(g): a failed vote of its own kind, never dropped
            return Verdict(mode=item["mode"], flags=[f"tool_call:{exc}"]), ""
        except Exception as exc:  # noqa: BLE001 -- recorded on the vote, never silently dropped
            return Verdict(mode=item["mode"],
                           flags=[f"call_failed:{type(exc).__name__}: {str(exc)[:200]}"]), ""
        v = parse_verdict(raw, mode=item["mode"], offered_uuids=item["offered_uuids"],
                          offered_lessons=item["offered_lessons"])
        return verify_quote(v, item["pre_evidence"]), raw

    def one(k):
        tries = []
        for a in range(MAX_ATTEMPTS_PER_VOTE):
            v, raw = attempt(k, a)
            tries.append({"attempt": a + 1, "failure": _failure(v), "flags": list(v.flags),
                          "raw": raw})
            if tries[-1]["failure"] is None:
                break
        return v, raw, tries

    with cf.ThreadPoolExecutor(max_workers=max(1, min(MAX_CONCURRENT_CALLS, votes))) as ex:
        results = list(ex.map(one, range(votes)))
    verdicts = [v for v, _raw, _tries in results]
    majority, disagreement = _majority(verdicts)
    return {"id": item["id"], "mode": item["mode"],
            "votes": [dict(dataclasses.asdict(v), raw=raw, attempts=tries)
                      for v, raw, tries in results],
            "majority": majority, "disagreement": disagreement,
            "split_fields": sorted(f for f, n in disagreement.items() if n > 1)}


# --- R120: gate items, built from the two eval documents ------------------------------------------

_FIELD_RE = re.compile(r"^- \*\*([a-z_]+(?: \([AB]\))?):\*\*(.*)$")
_HEX = r"[0-9a-f]{7,40}"
_MD_PATH_RE = re.compile(r"`((?:[\w.-]+/)+[\w.-]+\.md)`")


def _fenced_blocks(lines):
    """The contents of each ``` / ~~~ fenced block, with lessons.py's fence rule (R85)."""
    out, cur, ch, n = [], None, None, 0
    for ln in lines:
        if cur is not None:
            m = _lessons._FENCE_CLOSE_RE.match(ln)
            if m and m.group(2)[0] == ch and len(m.group(2)) >= n:
                out.append("\n".join(cur))
                cur = None
            else:
                cur.append(ln)
            continue
        m = _lessons._FENCE_OPEN_RE.match(ln)
        if m and not (m.group(2)[0] == "`" and "`" in m.group(3)):
            cur, ch, n = [], m.group(2)[0], len(m.group(2))
    if cur is not None:
        raise ValueError("an unterminated fence in a gate document field")
    return out


def _blocks_by_heading(lines, heading_re):
    """[(match, [lines])] per unfenced heading matching `heading_re`, each block running to the
    next matching heading or any unfenced `## ` heading."""
    fenced = _lessons._fence_flags(lines)
    out, cur = [], None
    for ln, f in zip(lines, fenced):
        m = None if f else heading_re.match(ln)
        if m:
            cur = (m, [])
            out.append(cur)
        elif not f and ln.startswith("## "):
            cur = None
        elif cur is not None:
            cur[1].append(ln)
    return out


def _fields(lines):
    fenced = _lessons._fence_flags(lines)
    out, cur = {}, None
    for ln, f in zip(lines, fenced):
        m = None if f else _FIELD_RE.match(ln)
        if m:
            cur = [m.group(2)]
            out[m.group(1)] = cur
        elif cur is not None:
            cur.append(ln)
    return out


def _flat(lines):
    return _norm("\n".join(lines))


def _first_tick(lines):
    m = re.search(r"`([^`]+)`", "\n".join(lines))
    return m.group(1) if m else None


def _section_lines(text, heading):
    lines = text.splitlines()
    fenced = _lessons._fence_flags(lines)
    start = next((i for i, (ln, f) in enumerate(zip(lines, fenced)) if not f and ln == heading),
                 None)
    if start is None:
        raise ValueError(f"no {heading!r} section")
    end = next((i for i in range(start + 1, len(lines))
                if not fenced[i] and lines[i].startswith("## ")), len(lines))
    return lines[start + 1:end]


def parse_rtd_cases(text):
    """Every `### Case RTD-N — …` of rule-tell-detection.md § Cases, with its fields (§ Case
    format). `positive` / `negative` are each field's FIRST fenced block (RTD-8 and RTD-15 quote a
    falsifier in a second block, which is context, not the decision)."""
    blocks = _blocks_by_heading(_section_lines(text, "## Cases"),
                                re.compile(r"^### Case (RTD-\d+) — (.*)$"))
    cases = []
    for m, lines in blocks:
        f = _fields(lines)
        for name in ("rule", "positive", "negative", "source", "caught_by", "text_detectable"):
            if name not in f:
                raise ValueError(f"{m.group(1)}: no `{name}` field")
        pos, neg = _fenced_blocks(f["positive"]), _fenced_blocks(f["negative"])
        if not pos or not neg:
            raise ValueError(f"{m.group(1)}: a positive or negative with no fenced text")
        td, cb = _first_tick(f["text_detectable"]), _first_tick(f["caught_by"])
        if td not in TEXT_DETECTABLE_TO_DETECTABILITY:
            raise ValueError(f"{m.group(1)}: text_detectable {td!r}")
        if cb not in ("peer", "measurement", "self-reread"):
            raise ValueError(f"{m.group(1)}: caught_by {cb!r}")
        cases.append({"case": m.group(1), "title": m.group(2), "rule": _flat(f["rule"]),
                      "tell": _flat(f.get("tell", [])), "positive": pos[0], "negative": neg[0],
                      "source": _flat(f["source"]), "caught_by": cb, "text_detectable": td})
    return cases


def parse_no_law_cases(text):
    """The cases § Case format records as having no written law that names their tell."""
    m = re.search(r"\((RTD-\d+(?:,\s*RTD-\d+)*)\) have no law", text)
    return set(re.findall(r"RTD-\d+", m.group(1))) if m else set()


def _parse_source(src, last_path, last_adr):
    """(positive rev, side, correction rev, path) from a case's `source:` field. In place (`-`
    side): the positive is read at the correction's PARENT. Appended (`positive from X`): at X
    itself, the positive's own `+` side, where its later retraction does not yet exist. A field
    naming no path means the previous case's file; `same ADR` the previous ADR."""
    m = re.search(rf"positive (?:and its falsifier both )?from `({_HEX})`", src)
    if m:
        n = re.search(rf"negative from `({_HEX})`", src)
        if not n:
            raise ValueError(f"appended source names no negative commit: {src!r}")
        pos_rev, side, corr_rev = m.group(1), "+", n.group(1)
    else:
        h = re.search(rf"`({_HEX})`", src)
        if not h or "`-`" not in src:
            raise ValueError(f"source is neither in place (`-` side) nor appended: {src!r}")
        pos_rev, side, corr_rev = f"{h.group(1)}^", "-", h.group(1)
    pm = _MD_PATH_RE.search(src)
    path = pm.group(1) if pm else (last_adr if "same ADR" in src else last_path)
    if path is None:
        raise ValueError(f"source names no file and no earlier case did: {src!r}")
    return pos_rev, side, corr_rev, path


def _expected_lesson(case, no_law):
    """What lesson-assignment agreement is scored against (R120; reported, never a pass
    condition): `uncovered` for a no-law case; else the rule's surface and section, with a
    lesson-id prefix only where the surface is a lesson source (CLAUDE.md)."""
    if case["case"] in no_law:
        return "uncovered"
    m = re.match(r"(?:`([^`]+)`\s*)?(?:§\s*\*([^*]+)\*)?", case["rule"])
    surface, section = m.group(1), m.group(2)
    prefix = (f"CLAUDE.md#{_lessons._slugify(section)}"
              if surface == "CLAUDE.md" and section else None)
    return {"surface": surface, "section": section, "prefix": prefix}




def _excerpts(blob, spans, radius=DOC_CONTEXT_CHARS):
    """Up to `radius` chars of `blob` on each side of each span (snapped to a line boundary
    inside that budget), windows that touch merged, each span marked in place."""
    windows = []
    for s, e, label in sorted(spans):
        ws, we = max(0, s - radius), min(len(blob), e + radius)
        if ws > 0:
            nl = blob.find("\n", ws, s)
            ws = nl + 1 if nl >= 0 else ws
        if we < len(blob):
            nl = blob.rfind("\n", e, we)
            we = nl if nl >= 0 else we
        if windows and ws <= windows[-1][1]:
            windows[-1][1] = max(windows[-1][1], we)
            windows[-1][2].append((s, e, label))
        else:
            windows.append([ws, we, [(s, e, label)]])
    out = []
    for ws, we, sp in windows:
        parts, cur = [], ws
        for s, e, label in sp:
            tag = "DECISION POINT" + (f" ({label})" if label else "")
            parts += [blob[cur:s], f"<<<{tag}>>>", blob[s:e], f"<<<END {tag}>>>"]
            cur = e
        parts.append(blob[cur:we])
        out.append("".join(parts))
    return out


def _rtd_items(repo, cases, no_law):
    items = []
    last_path = last_adr = None
    for c in cases:
        pos_rev, side, corr_rev, path = _parse_source(c["source"], last_path, last_adr)
        last_path = path
        if path.startswith("docs/adrs/"):
            last_adr = path
        pos_sha, corr_sha = _resolve(repo, pos_rev), _resolve(repo, corr_rev)
        blob = _lessons._git_show(repo, pos_sha, path)
        if blob is None:
            raise RuntimeError(f"{c['case']}: {path} does not exist at {pos_sha}")
        start = blob.find(c["positive"])
        if start < 0:
            raise ValueError(f"{c['case']}: the positive is not verbatim in the pre-correction "
                             f"blob {pos_sha[:12]}:{path}")
        td = c["text_detectable"]
        base = {
            "case": c["case"], "kind": "rtd", "decision": c["positive"], "decision_ts": None,
            "document": {"sha": pos_sha, "path": path,
                         "excerpts": _excerpts(blob, [(start, start + len(c["positive"]), None)])},
            "must_not_contain": [c["negative"]],
            "source": {"side": side, "positive_sha": pos_sha, "correction_sha": corr_sha,
                       "path": path, "occurrences": blob.count(c["positive"]), "raw": c["source"]},
            "expected": {"text_detectable": td,
                         "detectability": TEXT_DETECTABLE_TO_DETECTABILITY[td],
                         "caught_by": c["caught_by"],
                         "peer_yes": c["caught_by"] == "peer" and td == "yes",
                         "lesson": _expected_lesson(c, no_law)},
            "lesson_freeze_sha": pos_sha, "lesson_decision_ts": _author_date(repo, pos_sha),
        }
        items.append(dict(base, id=f"{c['case']}/correction", mode="correction",
                          correction=c["negative"]))
        if td == "yes":
            items.append(dict(base, id=f"{c['case']}/audit", mode="audit"))
    return items


def parse_controls(text):
    """rule-tell-controls.md: its tree, every `### Passage …` with its fields, and the five known
    positives -- the ids § Answer key lists, which must be exactly the passages whose
    `never_corrected` reads `withheld` (§ Five passages below are known positives)."""
    tree = re.search(r"\*\*Tree and instant:\*\* built at `([0-9a-f]{40})`", text)
    if not tree:
        raise ValueError("controls document names no 40-hex tree")
    lines = text.splitlines()
    fenced = _lessons._fence_flags(lines)
    key_at = next((i for i, (ln, f) in enumerate(zip(lines, fenced))
                   if not f and ln == "## Answer key"), None)
    if key_at is None:
        raise ValueError("controls document has no § Answer key")
    passages = []
    for m, plines in _blocks_by_heading(lines[:key_at],
                                        re.compile(r"^### Passage (CTL[0-9X]+-\d+) — (.*)$")):
        f = _fields(plines)
        if "text" in f:
            texts = [("", _fenced_blocks(f["text"])[0])]
        elif "text (A)" in f and "text (B)" in f:
            texts = [("A", _fenced_blocks(f["text (A)"])[0]),
                     ("B", _fenced_blocks(f["text (B)"])[0])]
        else:
            raise ValueError(f"{m.group(1)}: no text field")
        src = "\n".join(f.get("source", []))
        sources = [(p, int(a), int(b or a))
                   for p, a, b in re.findall(r"`([^`:\s]+):(\d+)(?:-(\d+))?`", src)]
        nc = "\n".join(f.get("never_corrected", []))
        passages.append({"id": m.group(1), "title": m.group(2),
                         "for_prompt": _flat(f.get("for_prompt", [])),
                         "texts": texts, "sources": sources,
                         "commits": re.findall(rf"`({_HEX})`\s+\d{{4}}-\d{{2}}-\d{{2}}", nc),
                         "withheld": "withheld" in nc,
                         "shape": ("full-shape" if "full-shape" in _flat(f.get("why_it_resembles", []))
                                   else "near-miss" if "near-miss" in _flat(f.get("why_it_resembles", []))
                                   else None)})
    known = set(re.findall(r"^\|\s*`(CTL[0-9X]+-\d+)`\s*\|", "\n".join(lines[key_at:]), re.M))
    withheld = {p["id"] for p in passages if p["withheld"]}
    if known != withheld:
        raise ValueError(f"answer key {sorted(known)} != withheld passages {sorted(withheld)}")
    return {"tree": tree.group(1), "passages": passages, "known_positives": sorted(known)}


def _locate(blob, text, line_hint):
    """Offset of `text` in `blob`: of its occurrences, the one whose line is nearest the cited
    `line_hint`; None when it does not occur. A cite can drift from the tree it names (measured:
    CTL3-4 and CTL10-4 each sit one line above their cited range, and occur once)."""
    starts, i = [], blob.find(text)
    while i >= 0:
        starts.append(i)
        i = blob.find(text, i + 1)
    if not starts:
        return None
    return min(starts, key=lambda s: abs(blob.count("\n", 0, s) + 1 - line_hint))


def _control_items(repo, parsed):
    tree = _resolve(repo, parsed["tree"])
    items = []
    for p in parsed["passages"]:
        if p["id"] in parsed["known_positives"]:
            continue
        sources = p["sources"]
        if len(sources) == 1 and len(p["texts"]) > 1:  # "two sentences of one line"
            sources = sources * len(p["texts"])
        if len(sources) != len(p["texts"]) or len({s[0] for s in sources}) != 1:
            raise ValueError(f"{p['id']}: expected one cited range per text, all in one file")
        path = sources[0][0]
        blob = _lessons._git_show(repo, tree, path)
        if blob is None:
            raise RuntimeError(f"{p['id']}: {path} does not exist at {tree}")
        spans, drift = [], []
        for (label, text), (_p, a, _b) in zip(p["texts"], sources):
            s = _locate(blob, text, a)
            if s is None:
                raise ValueError(f"{p['id']}: text {label} is not verbatim in {tree[:12]}:{path}")
            spans.append((s, s + len(text), label or None))
            drift.append(blob.count("\n", 0, s) + 1 - a)
        if not p["commits"]:
            raise ValueError(f"{p['id']}: never_corrected names no introducing commit")
        dates = [_author_date(repo, _resolve(repo, c)) for c in p["commits"]]
        decision = (p["texts"][0][1] if len(p["texts"]) == 1
                    else "\n\n".join(f"({label}) {text}" for label, text in p["texts"]))
        items.append({
            "id": f"{p['id']}/audit", "case": p["id"], "kind": "control", "mode": "audit",
            "decision": decision, "decision_ts": None,
            "document": {"sha": tree, "path": path, "excerpts": _excerpts(blob, spans)},
            "must_not_contain": [],
            "source": {"path": path, "tree": tree, "lines": [s[1:] for s in sources],
                       "line_drift": drift, "shape": p["shape"], "for_prompt": p["for_prompt"]},
            "expected": {"text_detectable": None, "peer_yes": False, "lesson": None,
                         "control": True},
            # Never corrected: its lessons are those at the controls' tree, dated before the
            # LATEST commit that introduced its text.
            "lesson_freeze_sha": tree, "lesson_decision_ts": max(dates, key=join.utc),
        })
    return items


def gate_items(repo, rtd_doc=None, controls_doc=None):
    """R120: every gate item -- per RTD case a correction-mode item, plus an audit-mode item for
    each `text_detectable: yes` case; and one audit-mode item per never-corrected control.
    Positives are read from the pre-correction blob, never from HEAD (spec § Judge protocol)."""
    repo = pathlib.Path(repo)
    rtd_text = pathlib.Path(rtd_doc or repo / RTD_DOC).read_text()
    ctl_text = pathlib.Path(controls_doc or repo / CONTROLS_DOC).read_text()
    return (_rtd_items(repo, parse_rtd_cases(rtd_text), parse_no_law_cases(rtd_text))
            + _control_items(repo, parse_controls(ctl_text)))


def _population(items):
    corr = [i for i in items if i["kind"] == "rtd" and i["mode"] == "correction"]
    audit = [i for i in items if i["kind"] == "rtd" and i["mode"] == "audit"]
    return {"correction": corr, "audit_rtd": audit,
            "peer_yes": [i for i in audit if i["expected"]["peer_yes"]],
            "controls": [i for i in items if i["kind"] == "control"]}


def check_population(items, population=GATE_POPULATION):
    """The gate's denominators are the spec's: 21 / 8 / 4 / 52, and the peer x yes cell is
    exactly RTD-3/8/9/10. Anything else raises."""
    pop = _population(items)
    want = {"correction": GATE_DETECTABILITY_MIN[1], "audit_rtd": GATE_YES_MIN[1],
            "peer_yes": GATE_PEER_YES_MIN[1], "controls": GATE_CONTROL_FIRE_MAX[1]}
    got = {k: len(v) for k, v in pop.items()}
    if got != want:
        raise ValueError(f"gate population {got} is not the spec's {want}")
    peer = sorted(i["case"] for i in pop["peer_yes"])
    if peer != sorted(population["peer_yes"]):
        raise ValueError(f"peer x yes cell {peer} is not the spec's {sorted(population['peer_yes'])}")
    return got


def check_no_law(items, population=GATE_POPULATION):
    """The cases whose expected lesson is `uncovered` are exactly the spec's RTD-8/9/15."""
    no_law = sorted(i["case"] for i in _population(items)["correction"]
                    if i["expected"]["lesson"] == "uncovered")
    if no_law != sorted(population["no_law"]):
        raise ValueError(f"no-law cases {no_law} are not the spec's {sorted(population['no_law'])}")


def _lesson_agreement(r):
    exp = r["expected"].get("lesson")
    got = r["majority"].get("lessons")
    if exp is None:
        return None
    if exp == "uncovered":
        return got == "uncovered"
    if not exp.get("prefix"):
        return None  # the rule's surface is no lesson source: not scoreable against the index
    return isinstance(got, list) and any(
        g.split(":", 1)[1].startswith(exp["prefix"]) for g in got if ":" in g)


def score_gate(results, population=GATE_POPULATION):
    """Spec § Judge protocol thresholds. `passed` is the conjunction of the four checks and of
    nothing else: lesson-assignment agreement is REPORTED beside it, never a pass condition
    (R120; Amendment 7 (b)2). Control fires are a firing rate, not a false-positive rate."""
    pop = _population(results)
    if population is not None:
        check_population(results, population)
    agree = sum(1 for r in pop["correction"] if r["majority"].get("detectability")
                == TEXT_DETECTABLE_TO_DETECTABILITY.get(r["expected"]["text_detectable"]))
    yes = sum(1 for r in pop["audit_rtd"] if r["majority"].get("is_mistake") is True)
    peer = sum(1 for r in pop["peer_yes"] if r["majority"].get("is_mistake") is True)
    fires = sum(1 for r in pop["controls"] if r["majority"].get("is_mistake") is True)
    checks = {
        "detectability": {"count": agree, "of": len(pop["correction"]),
                          "rule": f">= {GATE_DETECTABILITY_MIN[0]}",
                          "ok": agree >= GATE_DETECTABILITY_MIN[0]},
        "peer_yes_flags": {"count": peer, "of": len(pop["peer_yes"]),
                           "rule": f">= {GATE_PEER_YES_MIN[0]}", "ok": peer >= GATE_PEER_YES_MIN[0]},
        "yes_flags": {"count": yes, "of": len(pop["audit_rtd"]), "rule": f">= {GATE_YES_MIN[0]}",
                      "ok": yes >= GATE_YES_MIN[0]},
        "control_fires": {"count": fires, "of": len(pop["controls"]),
                          "rule": f"<= {GATE_CONTROL_FIRE_MAX[0]}",
                          "ok": fires <= GATE_CONTROL_FIRE_MAX[0],
                          "reported_as": "a firing rate, not a false-positive rate"},
    }
    rows = [{"id": r["id"], "expected": r["expected"].get("lesson"),
             "got": r["majority"].get("lessons"), "agrees": _lesson_agreement(r)}
            for r in pop["correction"]]
    scored = [x for x in rows if x["agrees"] is not None]
    return {"passed": all(c["ok"] for c in checks.values()), "checks": checks,
            "lesson_assignment": {"agree": sum(1 for x in scored if x["agrees"]),
                                  "scoreable": len(scored), "rows": rows,
                                  "note": "reported only; not a pass condition"}}


# --- R121: run the gate (dry: no model call) ------------------------------------------------------


_GATE_ID_RE = re.compile(r"RTD-\d+|CTL[0-9X]+-\d+|rule-tell", re.I)


def _leak_scan(items, inputs):
    """Mechanical checks a reviewer would otherwise do by eye: a case's own correction must be in
    no pre-decision evidence and in no lesson index; and the count of gate-id tokens
    (`RTD-N`, `CTL…-N`, `rule-tell`) each rendered prompt carries."""
    own_neg = []
    id_tokens = {}
    for it, inp in zip(items, inputs):
        for neg in it.get("must_not_contain") or []:
            probe = _norm(neg)
            if probe and (probe in _norm(inp["pre_evidence"])
                          or probe in _norm(render_lesson_index(inp["lessons"]))):
                own_neg.append(it["id"])
        hits = _GATE_ID_RE.findall(inp["prompt"])
        if hits:
            id_tokens[it["id"]] = len(hits)
    template_hits = len(_GATE_ID_RE.findall(PROMPT_PATH.read_text()))
    return {"own_correction_in_evidence_or_index": own_neg, "gate_id_tokens_by_item": id_tokens,
            "gate_id_tokens_in_template": template_hits}


def _shown(path, repo):
    """A path as the report prints it: repo-relative when inside the repo."""
    p = pathlib.Path(path).resolve()
    try:
        return str(p.relative_to(pathlib.Path(repo).resolve()))
    except ValueError:
        return p.name

def _leak_found(scan):
    """R135(d): True when `_leak_scan` reports anything at all."""
    return bool(scan["own_correction_in_evidence_or_index"] or scan["gate_id_tokens_by_item"]
                or scan["gate_id_tokens_in_template"])


def gate_header(repo, rtd_doc, controls_doc, votes, dry, version=None, channel=None):
    gs = _gs()
    repo = pathlib.Path(repo)
    return {
        "mode": "DRY -- inputs built and prompts rendered; no model call" if dry else "LIVE",
        "codex_version": version if version is not None else _codex_version(),
        "model": gs.CODEX_MODEL, "effort": gs.CODEX_EFFORT,
        "model_source": str(_GS_PATH.relative_to(REPO_ROOT)),
        "channel": channel if channel is not None else describe_channel(_placeholder_argv()),
        "prompt": f"{PROMPT_PATH.relative_to(REPO_ROOT)} sha256 {_sha256(PROMPT_PATH)}",
        "rtd_doc": f"{_shown(rtd_doc, repo)} sha256 {_sha256(rtd_doc)}",
        "controls_doc": f"{_shown(controls_doc, repo)} sha256 {_sha256(controls_doc)}",
        "repo_head": _resolve(repo, "HEAD"),
        "votes_per_item": votes,
        "retry_policy": f"R137: a {'/'.join(RETRY_FAILURES)} attempt is re-issued, up to "
                        f"{MAX_ATTEMPTS_PER_VOTE} attempts per vote; every attempt is recorded",
        "token_estimate": f"ceil(chars / {CHARS_PER_TOKEN}) -- an estimate, not a tokenizer count",
    }


def _refuse_log_dir_inside(log_dir, *roots):
    """R133(h): vote logs live only under --log-dir, OUTSIDE the repository, so no log can be
    committed or read back as data."""
    d = pathlib.Path(log_dir).resolve()
    for root in roots:
        r = pathlib.Path(root).resolve()
        if d == r or r in d.parents:
            raise ValueError(f"R133: --log-dir {d} is inside the repository {r}; vote logs live "
                             "outside it")


def run_gate(complete=None, dry=False, repo=REPO_ROOT, rtd_doc=None, controls_doc=None,
             global_claude_md=GLOBAL_CLAUDE_MD, votes=3, log_dir=None,
             population=GATE_POPULATION):
    """R120/R121. Builds all gate inputs (lessons at each item's own freeze sha and decision
    time) and renders every prompt. `dry=True` stops there and makes NO `complete` call; the calls
    actually made are counted either way. Otherwise every item is judged and scored; `passed`
    False makes the measurement INCONCLUSIVE by rule (plan Task 9 Step 6).

    A live run refuses to start (no vote cast): without `log_dir`, or with it inside the
    repository (R133 h); while the leak scan reports anything (R135 d). A live run on the Codex
    channel -- no `complete` injected -- also refuses any `votes` but GATE_VOTES, any
    `population` but the spec's (R138: the flags stay for dry runs and fixtures), and any
    `codex --version` but CODEX_CLI_VERSION; building the channel runs its jail precondition
    (R133 f) before the first vote."""
    global _CHANNEL
    repo = pathlib.Path(repo)
    if not dry and complete is None and log_dir is None:
        raise ValueError("a live gate on the Codex channel needs log_dir (R122: one log per call)")
    if not dry and log_dir is not None:
        _refuse_log_dir_inside(log_dir, repo, REPO_ROOT)
    own_channel = not dry and complete is None
    if own_channel and votes != GATE_VOTES:
        raise ValueError(f"R138: a live gate casts {GATE_VOTES} votes per item, not {votes}")
    if own_channel and population != GATE_POPULATION:
        raise ValueError("R138: a live gate checks the spec's population; --any-population is "
                         "for dry runs and fixtures only")
    try:
        ch = None
        if own_channel:
            ch = codex_channel()
            if ch.version != CODEX_CLI_VERSION:
                raise ValueError(f"R138: a live gate needs {CODEX_CLI_VERSION}; `codex --version` "
                                 f"gives {ch.version!r}")
        return _gate(ch, complete, dry, repo, rtd_doc, controls_doc, global_claude_md, votes,
                     log_dir, population)
    finally:
        if own_channel and _CHANNEL is not None:
            _CHANNEL.close()
            _CHANNEL = None


def _gate(ch, complete, dry, repo, rtd_doc, controls_doc, global_claude_md, votes, log_dir,
          population):
    rtd_path = pathlib.Path(rtd_doc or repo / RTD_DOC)
    ctl_path = pathlib.Path(controls_doc or repo / CONTROLS_DOC)
    items = gate_items(repo, rtd_path, ctl_path)
    if population is not None:
        population_counts = check_population(items, population)
        check_no_law(items, population)
    else:
        population_counts = {k: len(v) for k, v in _population(items).items()}
    calls = []
    lock = threading.Lock()

    def counted(prompt, log_path=None):
        with lock:
            calls.append(log_path)
        return (complete or ch.complete)(prompt, log_path)

    lesson_cache, lesson_stats, inputs = {}, {}, []
    for it in items:
        key = (it["lesson_freeze_sha"], it["lesson_decision_ts"])
        if key not in lesson_cache:
            stats = {}
            lesson_cache[key] = lessons_for(repo, key[0], key[1], global_claude_md=global_claude_md,
                                            stats=stats)
            lesson_stats[key] = stats
        inputs.append(build_input(it, None, lesson_cache[key], it["mode"]))

    rows = [{"id": it["id"], "mode": it["mode"], "kind": it["kind"], "lessons": len(inp["lessons"]),
             "context_chars": sum(len(c["text"]) for c in inp["context"]),
             "prompt_chars": len(inp["prompt"]),
             "est_tokens": math.ceil(len(inp["prompt"]) / CHARS_PER_TOKEN)}
            for it, inp in zip(items, inputs)]
    largest = max(rows, key=lambda r: r["prompt_chars"]) if rows else None
    totals = {"items": len(rows), "population": population_counts,
              "prompt_chars": sum(r["prompt_chars"] for r in rows),
              "est_tokens": sum(r["est_tokens"] for r in rows), "votes": votes,
              "largest": largest}
    totals["est_tokens_all_votes"] = totals["est_tokens"] * votes
    result = {"dry": dry, "rows": rows, "totals": totals, "leak_scan": _leak_scan(items, inputs),
              "lesson_stats": [{"freeze_sha": k[0], "decision_ts": k[1], **v}
                               for k, v in lesson_stats.items()]}
    result["header"] = gate_header(repo, rtd_path, ctl_path, votes, dry,
                                   version=None if ch is None else ch.version,
                                   channel=None if ch is None else ch.describe())
    if dry:
        result.update(calls=len(calls), passed=None)
        return result
    if _leak_found(result["leak_scan"]):
        raise LeakageError("R135: a live gate refuses to start while the leak scan reports "
                           f"anything: {json.dumps(result['leak_scan'], sort_keys=True)}")
    results = []
    for it, inp in zip(items, inputs):
        r = judge(inp, votes=votes, complete=counted, log_dir=log_dir)
        results.append(dict(r, kind=it["kind"], case=it["case"], expected=it["expected"],
                            for_prompt=it["source"].get("for_prompt")))
    score = score_gate(results, population)
    result.update(results=results, score=score, passed=score["passed"], calls=len(calls))
    return result


# R142 (review concern 2): a transfer limit, printed in every gate report.
CONTEXT_DISCLOSURE = ("gate document items include text after the decision point from the same "
                      "pre-correction blob; session items never do")


def _quote_failure_line(results):
    """R142 (review concern 4), descriptive: how many majority flags (the text_detectable yes
    column's `is_mistake` True) and control fires carry at least one `quote_not_verbatim` vote --
    a quote-failed vote keeps its `is_mistake`, so these still count."""
    def carries(r):
        return any("quote_not_verbatim" in v["flags"] for v in r["votes"])

    pop = _population(results)
    flagged = [r for r in pop["audit_rtd"] if r["majority"].get("is_mistake") is True]
    fires = [r for r in pop["controls"] if r["majority"].get("is_mistake") is True]
    return (f"- quote_not_verbatim (descriptive): {sum(map(carries, flagged))} of "
            f"{len(flagged)} majority flags and {sum(map(carries, fires))} of {len(fires)} "
            "control fires carry at least one such vote")



def format_gate(res):
    """The gate report as text: header, the per-item table, totals, lesson inventory per freeze,
    the leak scan, the call count, and (live only) the four checks, lesson agreement, control
    fires per prompt section, and every item's majority and split fields."""
    h = res["header"]
    out = [f"# Task 9 gate -- {h['mode']}", ""]
    out += [f"{k}: {v}" for k, v in h.items() if k != "mode"]
    out += ["", "| id | mode | kind | lessons | context chars | prompt chars | est tokens |",
            "|---|---|---|---|---|---|---|"]
    out += [f"| {r['id']} | {r['mode']} | {r['kind']} | {r['lessons']} | {r['context_chars']} | "
            f"{r['prompt_chars']} | {r['est_tokens']} |" for r in res["rows"]]
    t = res["totals"]
    out += ["", f"items: {t['items']} {json.dumps(t['population'], sort_keys=True)}",
            f"total prompt chars: {t['prompt_chars']}",
            f"total est tokens (one vote): {t['est_tokens']}",
            f"total est tokens (x{t['votes']} votes): {t['est_tokens_all_votes']}"]
    if t["largest"]:
        big = t["largest"]
        out.append(f"largest item: {big['id']} ({big['prompt_chars']} chars, "
                   f"{big['est_tokens']} est tokens)")
    out += ["", "lesson inventory per (freeze sha, decision ts):",
            "| freeze sha | decision ts | dated kept | after decision | undatable | global undated |",
            "|---|---|---|---|---|---|"]
    out += [f"| {s['freeze_sha'][:12]} | {s['decision_ts']} | {s['lesson_dated']} | "
            f"{s['lesson_after_decision']} | {s['lesson_undatable']} | {s['lesson_global']} |"
            for s in res["lesson_stats"]]
    lk = res["leak_scan"]
    own = lk["own_correction_in_evidence_or_index"]
    out += ["", "leak scan:",
            f"- own correction in pre-evidence or lesson index: {len(own)} {own}",
            f"- gate-id tokens in the template: {lk['gate_id_tokens_in_template']}",
            f"- items whose rendered prompt carries gate-id tokens: "
            f"{len(lk['gate_id_tokens_by_item'])} {json.dumps(lk['gate_id_tokens_by_item'], sort_keys=True)}",
            "", f"context disclosure: {CONTEXT_DISCLOSURE}",
            "", f"complete() calls: {res['calls']}"]
    if not res["dry"]:
        s = res["score"]
        out += ["", f"PASSED: {s['passed']}"]
        out += [f"- {k}: {c['count']}/{c['of']} (need {c['rule']}) -> {'ok' if c['ok'] else 'FAIL'}"
                for k, c in s["checks"].items()]
        la = s["lesson_assignment"]
        out.append(f"- lesson assignment (reported, not a pass condition): "
                   f"{la['agree']}/{la['scoreable']} scoreable")
        by_prompt = collections.Counter(r.get("for_prompt") for r in res["results"]
                                        if r["kind"] == "control"
                                        and r["majority"].get("is_mistake") is True)
        out.append(f"- control fires by prompt section (descriptive): "
                   f"{json.dumps(dict(by_prompt), sort_keys=True)}")
        out.append(_quote_failure_line(res["results"]))
        out += ["", "| id | expected | detectability | is_mistake | lessons | split fields | flags |",
                "|---|---|---|---|---|---|---|"]
        for r in res["results"]:
            m = r["majority"]
            exp = r["expected"].get("detectability") or ("control" if r["kind"] == "control" else "")
            out.append(f"| {r['id']} | {exp} | {m.get('detectability')} | {m.get('is_mistake')} | "
                       f"{json.dumps(m.get('lessons'))} | {','.join(r['split_fields'])} | "
                       f"{','.join(m.get('flags') or [])} |")
    return "\n".join(out) + "\n"
