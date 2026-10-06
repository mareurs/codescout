"""Stage 2: the correction miner (Task 8).

Proposes correction CANDIDATES (never confirmed labels -- a later LLM judge confirms each
one) from two sources:

  - operator candidates: a top-level `prompt`/`interrupt`/`rejection` turn with an earlier
    top-level `assistant_text` turn in the same session (R89, R105);
  - commit candidates: a git commit whose subject/body/added-lines match a
    correction-shaped selector (R90), attributed to its origin by blaming the REMOVED
    side of each hunk at the commit's parent (R88, R91) -- never the correcting commit's
    own `Session-Id`, which names who committed, not who wrote the corrected lines
    (docs/issues/archive/2026-09-22-a-session-id-trailer-names-who-committed-not-who-wrote-the-line.md).

Recall-oriented by design: a false candidate costs one judge call later and is not a
bias, so no lexical filter is applied to operator candidates (R89).

Full ruling ledger (R88-R95, and R102/R105/R110 from fix round 1):
.superpowers/sdd/2026-09-26-system1-base-rate-measurement/task-8-context.md and
task-8-fix1-brief.md.
"""
import importlib.util
import re
import sqlite3
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

_HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(_HERE))
import join  # noqa: E402  (sibling module in this same directory, per freeze_and_measure.py's idiom)

# mine_pairs.py lives in a wholly different directory (a data/eval dir, not scripts/measure),
# so R88 requires loading it by path relative to THIS file rather than via sys.path -- a
# home-literal path is forbidden (R39).
_MINE_PAIRS_PATH = Path(__file__).resolve().parents[2] / \
    "docs/evals/data/2026-09-24-rule-tell/stage2/mine_pairs.py"
_mp_spec = importlib.util.spec_from_file_location("mine_pairs", _MINE_PAIRS_PATH)
_mine_pairs = importlib.util.module_from_spec(_mp_spec)
_mp_spec.loader.exec_module(_mine_pairs)
MARKER_RE = _mine_pairs.MARKER_RE
NOTE_RE = _mine_pairs.NOTE_RE


@dataclass(frozen=True)
class Candidate:
    cid: str
    sid: str
    detected_ts: str
    # operator_message | operator_interrupt | operator_rejection | correction_commit |
    # review_commit | retraction
    source: str
    corrector: str  # operator | self | peer-session | review
    origin_hint: dict  # {"uuid"|"sha": ..., "ts": ...}
    text: str


# R90(iii): a lexical subject proxy, never the body.
_CORRECTION_SUBJECT_RE = re.compile(r"\b(?:retract|correct|falsif|withdr[ae]w|overstat)", re.I)
# R90(iv): word-boundary "review" -- "preview" must NOT match.
_REVIEW_RE = re.compile(r"\breview", re.I)
# A trailer-shaped line: "Key: value". Used only to trim the TRAILING contiguous block of
# such lines off a commit body (R90(i): "the body with trailer lines removed") -- a
# "Review: ..." paragraph earlier in the body, separated by a blank line, must survive.
_TRAILER_LINE_RE = re.compile(r"^[A-Za-z][\w-]*:\s+\S")
# "@@ -a,b +c,d @@", b/d optional (implied 1) per unified-diff convention.
_HUNK_RE = re.compile(r"^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@")
# First field of a git blame --porcelain commit-info line.
_BLAME_SHA_RE = re.compile(r"^([0-9a-f]{40})\s")


def _git(repo_path, *args):
    # `errors="replace"`: git commit bodies and diffs are not guaranteed valid UTF-8 (a
    # committer's system encoding, or a diffed file's own bytes, can carry e.g. a Windows-1252
    # smart quote) -- a real commit in this repo's own history (176015f2) triggered a strict-decode
    # UnicodeDecodeError on its diff. Replacing undecodable bytes with U+FFFD keeps every regex
    # match here (subject/body/added-line markers) working on everything ELSE in the text; it can
    # only ever cause a false-negative on the exact undecodable byte span, never a crash.
    return subprocess.run(
        ["git", "-C", str(repo_path), *args], capture_output=True, text=True, check=True,
        errors="replace",
    ).stdout


def _commit_body(repo_path, sha):
    return _git(repo_path, "log", "-1", "--format=%B", sha)


def session_id_of(repo_path, sha):
    """The ONE function both the correcting commit's and the antecedent's Session-Id come
    from (R91), so the two are read the same way.

    R110: the value comes from git's own trailer parser
    (`%(trailers:key=Session-Id,valueonly)`, first non-empty line), never from a regex over
    the body. A prose line that merely STARTS "Session-Id:" above the real trailer block is
    not a trailer, and a first-match search would read it.

    Equals commits.session_id on every commit: join._run_git_log reads the same trailer through
    the same `%(trailers:key=Session-Id,valueonly)` format and takes the same first non-empty
    line (Task 14, R110), merges and prose "Session-Id:" paragraphs included -- the merge
    trailer-as-file-list bug is fixed and archived as 6708cab25f53b797. The miner skips merges
    before it ever reads a Session-Id, so a merge never reaches a candidate either way.
    """
    raw = _git(repo_path, "log", "-1", "--format=%(trailers:key=Session-Id,valueonly)", sha)
    for line in raw.splitlines():
        line = line.strip()
        if line:
            return line
    return None


def _commit_ts_of(repo_path, sha):
    # R110: the AUTHOR date (%aI), never the committer date. `experiments` is rebased after
    # every ship, which restamps committer dates, so %cI says when a commit was last rewritten,
    # not when its lines were written. Used for the antecedent's origin ts.
    raw = _git(repo_path, "log", "-1", "--format=%aI", sha).strip()
    return join._fmt_ts(raw)


def _parents_of(repo_path, sha):
    raw = _git(repo_path, "log", "-1", "--format=%P", sha).strip()
    return raw.split() if raw else []


def _commit_diff(repo_path, parent_sha, sha):
    return _git(repo_path, "diff", "-U0", "--no-color", "--no-renames", parent_sha, sha)


def _strip_trailers(body):
    """Drop only the TRAILING contiguous block of trailer-shaped lines, stopping at the
    first blank (or non-trailer-shaped) line walking backward -- so an earlier "Review: ..."
    paragraph, separated from the trailer block by a blank line, is preserved."""
    lines = body.splitlines()
    while lines and not lines[-1].strip():
        lines.pop()
    i = len(lines)
    while i > 0 and _TRAILER_LINE_RE.match(lines[i - 1]):
        i -= 1
    return "\n".join(lines[:i])


def _parse_diff(diff_text):
    """[(path, [{"a": int, "b": int, "added": [line, ...]}, ...]), ...] for a `git diff -U0`
    (zero-context, no-renames) unified diff. `path` follows mine_pairs.py's own idiom for
    parsing a "diff --git a/<path> b/<path>" line."""
    files = []
    cur_path = None
    cur_hunks = None
    cur_hunk = None
    for line in diff_text.splitlines():
        if line.startswith("diff --git "):
            if cur_path is not None:
                files.append((cur_path, cur_hunks))
            cur_path = line.split(" b/", 1)[-1]
            cur_hunks = []
            cur_hunk = None
        elif line.startswith("@@"):
            m = _HUNK_RE.match(line)
            if m:
                a = int(m.group(1))
                b = int(m.group(2)) if m.group(2) is not None else 1
                cur_hunk = {"a": a, "b": b, "added": []}
                cur_hunks.append(cur_hunk)
        elif cur_hunk is not None and line.startswith("+") and not line.startswith("+++"):
            cur_hunk["added"].append(line[1:])
        # removed-line text itself is never needed: the antecedent is found by blaming the
        # PARENT at the hunk's old-side line range, never by reading the "-" line's text.
    if cur_path is not None:
        files.append((cur_path, cur_hunks))
    return files


def _blame_shas(repo_path, parent_sha, path, start, count, stats):
    """Distinct commit shas (order preserved) attributing lines [start, start+count) of
    `path` at `parent_sha`, via `git blame --porcelain -L start,+count`.

    R102: returns None, never [], when `git blame` itself exits non-zero, and counts it in
    `blame_git_failures`. A failed blame means the antecedent is UNKNOWN, which must never read
    like a pure addition (no antecedent). Reachable without fault injection: a submodule
    pointer (gitlink) shows in `git diff` as a one-line hunk, but blaming it exits 128."""
    out = subprocess.run(
        ["git", "-C", str(repo_path), "blame", "--porcelain", "-L", f"{start},+{count}",
         parent_sha, "--", path],
        capture_output=True, text=True, errors="replace",
    )
    if out.returncode != 0:
        _bump(stats, "blame_git_failures")
        return None
    shas = []
    for line in out.stdout.splitlines():
        m = _BLAME_SHA_RE.match(line)
        if m:
            shas.append(m.group(1))
    return list(dict.fromkeys(shas))


def _is_examined(subject, stripped_body, added_lines, is_review):
    return (
        bool(MARKER_RE.search(subject))
        or bool(MARKER_RE.search(stripped_body))
        or any(MARKER_RE.search(line) for line in added_lines)
        # R90(ii) is INERT (M7): every NOTE_RE word is a MARKER_RE word, so no test can reach it.
        or any(NOTE_RE.match(line) for line in added_lines)
        or bool(_CORRECTION_SUBJECT_RE.search(subject))
        or is_review
    )


def _antecedents_of(repo_path, parent_sha, files, stats):
    """([(antecedent_sha, via_note_only), ...], blame_failed): one entry per (hunk, distinct
    blamed sha), and whether any blame FAILED (R102)."""
    out = []
    blame_failed = False
    for path, hunks in files:
        for h in hunks:
            a, b, added = h["a"], h["b"], h["added"]
            if b > 0:
                shas, via_note = _blame_shas(repo_path, parent_sha, path, a, b, stats), False
            elif b == 0 and a >= 1 and any(NOTE_RE.match(line) for line in added):
                # R91: a pure addition whose added lines match NOTE_RE blames the single
                # line directly before the insertion point -- a retraction of the sentence
                # it annotates.
                shas, via_note = _blame_shas(repo_path, parent_sha, path, a, 1, stats), True
            else:
                continue
            if shas is None:
                blame_failed = True
                continue
            out.extend((asha, via_note) for asha in shas)
    return out, blame_failed


def _sid_by_bare(conn):
    m = {}
    for (sid,) in conn.execute("SELECT DISTINCT sid FROM turns"):
        bare = sid.split("/", 1)[1] if "/" in sid else sid
        m.setdefault(bare, []).append(sid)
    return m


def _bump(stats, key):
    if stats is not None:  # candidates() always passes a dict; a direct helper call may not
        stats[key] = stats.get(key, 0) + 1


def _operator_candidates(conn, stats):
    rows = conn.execute(
        "SELECT rowid, sid, uuid, ts, kind, text FROM turns WHERE agent_path IS NULL "
        "ORDER BY sid, ts, rowid"
    ).fetchall()
    by_sid: dict[str, list] = {}
    for rowid, sid, uuid, ts, kind, text in rows:
        by_sid.setdefault(sid, []).append((rowid, uuid, ts, kind, text))

    cands = []
    for sid, srows in by_sid.items():
        last_assistant = None  # (uuid, ts) of the latest top-level assistant_text so far
        for i, (_rowid, uuid, ts, kind, text) in enumerate(srows):
            if kind == "assistant_text":
                last_assistant = (uuid, ts)
                continue
            if kind not in ("prompt", "interrupt", "rejection"):
                continue  # delegation, meta, tool_result, tool_use, assistant_thinking: never candidates
            if last_assistant is None:
                _bump(stats, "operator_no_prior_assistant_text")
                continue
            origin_hint = {"uuid": last_assistant[0], "ts": last_assistant[1]}
            if kind == "prompt":
                cands.append(Candidate(
                    cid=f"op:{sid}:{uuid}", sid=sid, detected_ts=ts,
                    source="operator_message", corrector="operator",
                    origin_hint=origin_hint, text=text,
                ))
            elif kind == "interrupt":
                next_text = ""
                for j in range(i + 1, len(srows)):
                    if srows[j][3] == "prompt":
                        next_text = srows[j][4]
                        break
                cands.append(Candidate(
                    cid=f"op:{sid}:{uuid}", sid=sid, detected_ts=ts,
                    source="operator_interrupt", corrector="operator",
                    origin_hint=origin_hint, text=next_text,
                ))
            else:  # rejection (R105; join emits it from Task 14): origin as a prompt, own text.
                cands.append(Candidate(
                    cid=f"op:{sid}:{uuid}", sid=sid, detected_ts=ts,
                    source="operator_rejection", corrector="operator",
                    origin_hint=origin_hint, text=text,
                ))
    return cands


def _commit_candidates_for_row(repo_path, repo, sha, ts, subject, sid_map, stats):
    parents = _parents_of(repo_path, sha)
    if len(parents) > 1:
        _bump(stats, "commit_merge")
        return []
    if not parents:
        return []  # a root commit can never be a correction; unreached in practice
    parent_sha = parents[0]

    stripped_body = _strip_trailers(_commit_body(repo_path, sha))
    files = _parse_diff(_commit_diff(repo_path, parent_sha, sha))
    added_all = [line for _, hunks in files for h in hunks for line in h["added"]]

    is_review = bool(_REVIEW_RE.search(subject)) or bool(_REVIEW_RE.search(stripped_body))
    if not _is_examined(subject, stripped_body, added_all, is_review):
        return []

    antecedents, blame_failed = _antecedents_of(repo_path, parent_sha, files, stats)
    if not antecedents:
        # R102: after a FAILED blame the antecedent is unknown, not absent. It is already
        # counted in blame_git_failures, and never as no_antecedent (a pure addition).
        if not blame_failed:
            _bump(stats, "no_antecedent")
        return []

    via_note_only: dict[str, bool] = {}
    for asha, via_note in antecedents:
        via_note_only[asha] = via_note_only.get(asha, True) and via_note

    if is_review:
        source = "review_commit"
    elif via_note_only and all(via_note_only.values()):
        source = "retraction"
    else:
        source = "correction_commit"

    correcting_sid = session_id_of(repo_path, sha)
    if correcting_sid is None:
        _bump(stats, "commit_untrailered")
        return []

    bare = correcting_sid.split("/", 1)[1] if "/" in correcting_sid else correcting_sid
    matches = sid_map.get(bare, [])
    if len(matches) == 0:
        _bump(stats, "commit_session_not_in_corpus")
        return []
    if len(matches) > 1:
        _bump(stats, "commit_session_ambiguous")
        return []
    cand_sid = matches[0]

    out = []
    seen_shas = set()
    for asha, _via_note in antecedents:
        if asha in seen_shas:
            continue
        seen_shas.add(asha)
        antecedent_sid = session_id_of(repo_path, asha)
        if antecedent_sid is None:
            _bump(stats, "antecedent_untrailered")
            continue
        if antecedent_sid in join.SPEC_EXCLUDED_SIDS:
            _bump(stats, "antecedent_excluded")
            continue
        corrector = "review" if is_review else (
            "self" if antecedent_sid == correcting_sid else "peer-session"
        )
        out.append(Candidate(
            cid=f"commit:{repo}:{sha}:{asha}",
            sid=cand_sid,
            detected_ts=ts,
            source=source,
            corrector=corrector,
            origin_hint={"sha": asha, "ts": _commit_ts_of(repo_path, asha)},
            text=subject,
        ))
    return out


def candidates(events_db, repos, stats=None):
    if stats is None:
        stats = {}
    db_uri = Path(events_db).resolve().as_uri() + "?mode=ro"
    conn = sqlite3.connect(db_uri, uri=True)
    try:
        cands = _operator_candidates(conn, stats)
        sid_map = _sid_by_bare(conn)
        commit_rows = conn.execute(
            "SELECT repo, sha, ts, session_id, subject FROM commits"
        ).fetchall()
    finally:
        conn.close()

    for repo, sha, ts, _db_session_id, subject in commit_rows:
        if repo not in repos:
            _bump(stats, "commit_repo_missing")
            continue
        cands.extend(
            _commit_candidates_for_row(repos[repo], repo, sha, ts, subject, sid_map, stats)
        )
    return cands


def _origin_key(cand):
    if "uuid" in cand.origin_hint:
        return ("uuid", cand.origin_hint["uuid"])
    return ("sha", cand.origin_hint["sha"])


def group_by_origin(cands):
    """One group per distinct origin (R93). Keying on ("uuid", u) vs ("sha", s) means an
    operator candidate can never collide with a commit candidate's group, by construction."""
    groups: dict[tuple, list] = {}
    order = []
    for c in cands:
        key = _origin_key(c)
        if key not in groups:
            groups[key] = []
            order.append(key)
        groups[key].append(c)

    result = [sorted(groups[key], key=lambda c: (c.detected_ts, c.cid)) for key in order]
    result.sort(key=lambda g: (g[0].detected_ts, g[0].cid))
    return result
