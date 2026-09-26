"""Stage 1a — sessions, exclusions, fork detection over a FROZEN corpus.

Never reads a live ~/.claude* directory directly: every function here takes a corpus_dir
produced by archive.freeze(), whose layout is
  <corpus_dir>/transcripts/<NN>-<profile>/<project_slug>/<sid>.jsonl
  <corpus_dir>/transcripts/<NN>-<profile>/<project_slug>/<sid>/subagents/<name>.jsonl
(see archive.freeze()'s dest_project_dir / dest_sub_dir construction — profile_name there is
src_dir.parent.parent.name, e.g. the literal ".claude-kat").

Exclusion reasons (Task 5 brief + controller rulings R1/R22/R23/R24):
  sdk-cli                       entrypoint == "sdk-cli" (headless `claude -p`)
  scratchpad-project            project slug names a scratchpad checkout
  excluded-by-spec              sid explicitly named by the caller (e.g. this measurement's
                                 own controlling session, or a session named in the plan)
  duplicate-prefix-of:<copy>    R22: same sessionId in two profiles, this copy's uuid list is
                                 a verified exact prefix of the kept copy's
  divergent-duplicate-of:<copy> R22: same sessionId in two profiles, neither is a prefix of the
                                 other — the longer copy is kept
  fork-of:<sid>                 R23: different sessionId, first FORK_PREFIX_LEN uuids equal
                                 another transcript's; the later-diverging copy is excluded

Every key in the exclusions() dict is a copy id `<profile>/<sid>` (see copy_id()) — never a
bare sid — so the R22 case (one sid, two profiles) cannot collide with itself.
"""
import json
import pathlib
import re
from dataclasses import dataclass, field

FORK_PREFIX_LEN = 5

COMMAND_WRAPPER_TAGS = (
    "<command-name>",
    "<local-command-stdout>",
    "<local-command-caveat>",
)

_PROFILE_DIR_RE = re.compile(r"^\d+-(.+)$")


def read_jsonl(path):
    """Read a transcript file line by line. Returns (entries, skipped_count).

    A malformed line (JSON decode failure — e.g. a process killed mid-write leaves a
    truncated final line) is counted and skipped rather than raising, per Review Focus 4.
    Blank lines are skipped without counting as malformed.
    """
    entries = []
    skipped = 0
    with open(path, "r") as f:
        for raw_line in f:
            line = raw_line.strip()
            if not line:
                continue
            try:
                entries.append(json.loads(line))
            except json.JSONDecodeError:
                skipped += 1
    return entries, skipped


def _message_text(entry):
    """Return the entry's message text, or None if it has none we recognize.

    Content qualifies only if it is a string, or a list whose first item is a {"type":
    "text", ...} dict (the shape a real prompt takes when it carries attachments, e.g. an
    imagePasteIds sibling key) — per Review Focus 3 / fact 2.
    """
    message = entry.get("message")
    if not isinstance(message, dict):
        return None
    content = message.get("content")
    if isinstance(content, str):
        return content
    if isinstance(content, list) and content:
        first = content[0]
        if isinstance(first, dict) and first.get("type") == "text":
            return first.get("text", "")
    return None


def operator_messages(entries):
    """Keep only type=="user" entries that are real human prompts.

    Excludes (Review Focus 3): isMeta entries (skill-loading injections etc, whether their
    content is a list or a string), compaction summaries (isCompactSummary), tool results
    (message.content a list not leading with a "text" item — so not recognized by
    _message_text at all), and messages wrapped in <command-name>, <local-command-stdout>
    or <local-command-caveat> (slash-command scaffolding, not the operator's own words).
    """
    kept = []
    for entry in entries:
        if entry.get("type") != "user":
            continue
        if entry.get("isMeta"):
            continue
        if entry.get("isCompactSummary"):
            continue
        text = _message_text(entry)
        if text is None:
            continue
        if text.startswith(COMMAND_WRAPPER_TAGS):
            continue
        kept.append(entry)
    return kept


@dataclass
class Session:
    sid: str
    path: pathlib.Path
    profile: str
    entrypoint: str
    first_uuids: list = field(default_factory=list)
    subagent_paths: list = field(default_factory=list)
    first_ts: str = None
    last_ts: str = None


def copy_id(session):
    """The R22-safe key: <profile>/<sid>. Never key exclusions by bare sid — the same
    sessionId can legitimately appear once per profile."""
    return f"{session.profile}/{session.sid}"


def _profile_from_dir(dirname):
    m = _PROFILE_DIR_RE.match(dirname)
    return m.group(1) if m else dirname


def sessions(corpus_dir):
    """Enumerate every top-level (non-subagent) session transcript in a frozen corpus.

    One Session per <sid>.jsonl found directly under a <profile>/<project_slug> dir; its
    subagents/*.jsonl siblings (if any) are attached as subagent_paths, not returned as
    separate sessions — they belong to the parent (fact 3).
    """
    corpus_dir = pathlib.Path(corpus_dir)
    transcripts_root = corpus_dir / "transcripts"
    result = []
    if not transcripts_root.is_dir():
        return result

    for profile_dir in sorted(p for p in transcripts_root.iterdir() if p.is_dir()):
        profile = _profile_from_dir(profile_dir.name)
        for project_dir in sorted(p for p in profile_dir.iterdir() if p.is_dir()):
            for jsonl in sorted(project_dir.glob("*.jsonl")):
                sid = jsonl.stem
                entries, _skipped = read_jsonl(jsonl)

                entrypoint = None
                first_ts = None
                last_ts = None
                first_uuids = []
                for entry in entries:
                    if entrypoint is None and entry.get("entrypoint"):
                        entrypoint = entry.get("entrypoint")
                    ts = entry.get("timestamp")
                    if ts:
                        if first_ts is None:
                            first_ts = ts
                        last_ts = ts
                    u = entry.get("uuid")
                    if u is not None and len(first_uuids) < FORK_PREFIX_LEN:
                        first_uuids.append(u)

                subagents_dir = project_dir / sid / "subagents"
                subagent_paths = (
                    sorted(subagents_dir.glob("*.jsonl")) if subagents_dir.is_dir() else []
                )

                result.append(
                    Session(
                        sid=sid,
                        path=jsonl,
                        profile=profile,
                        entrypoint=entrypoint,
                        first_uuids=first_uuids,
                        subagent_paths=subagent_paths,
                        first_ts=first_ts,
                        last_ts=last_ts,
                    )
                )
    return result


def _full_timeline(path):
    """Re-read a transcript in full: [(uuid, timestamp), ...] in file order.

    Session.first_uuids is bounded to FORK_PREFIX_LEN by design (detection heuristic only);
    R22/R23 verification needs the true full-length comparison, so this reads the file again
    on demand rather than trusting the bounded field.
    """
    entries, _skipped = read_jsonl(path)
    return [(e.get("uuid"), e.get("timestamp")) for e in entries if e.get("uuid") is not None]


def exclusions(sessions_list, excluded_sids):
    """Build the exclusion map for a list of Session objects.

    Returns {copy_id(session): reason}. Staged, highest priority first, so a copy already
    excluded by an earlier stage is never re-labelled by a later one:
      A excluded-by-spec  B sdk-cli  C scratchpad-project  D R22 same-sid duplicates
      E R23 forks
    """
    excl = {}

    # --- A: excluded-by-spec (explicit override; wins over anything else) ---
    for s in sessions_list:
        if s.sid in excluded_sids:
            excl[copy_id(s)] = "excluded-by-spec"

    # --- B: sdk-cli (headless) ---
    for s in sessions_list:
        cid = copy_id(s)
        if cid in excl:
            continue
        if s.entrypoint == "sdk-cli":
            excl[cid] = "sdk-cli"

    # --- C: scratchpad-project (project slug names a scratchpad checkout) ---
    for s in sessions_list:
        cid = copy_id(s)
        if cid in excl:
            continue
        project_slug = s.path.parent.name
        if "scratchpad" in project_slug.lower():
            excl[cid] = "scratchpad-project"

    # --- D: R22 same sessionId across profiles ---
    remaining = [s for s in sessions_list if copy_id(s) not in excl]
    by_sid = {}
    for s in remaining:
        by_sid.setdefault(s.sid, []).append(s)
    for sid, copies in by_sid.items():
        if len(copies) < 2:
            continue
        timelines = {copy_id(c): _full_timeline(c.path) for c in copies}
        # Longest copy is the tentative keeper; every shorter copy is checked against it.
        ordered = sorted(copies, key=lambda c: len(timelines[copy_id(c)]), reverse=True)
        keeper = ordered[0]
        keeper_id = copy_id(keeper)
        keeper_tl = timelines[keeper_id]
        for other in ordered[1:]:
            other_id = copy_id(other)
            other_tl = timelines[other_id]
            n = len(other_tl)
            if other_tl == keeper_tl[:n]:
                excl[other_id] = f"duplicate-prefix-of:{keeper_id}"
            else:
                excl[other_id] = f"divergent-duplicate-of:{keeper_id}"

    # --- E: R23 forks (different sid, first FORK_PREFIX_LEN uuids equal) ---
    remaining = [s for s in sessions_list if copy_id(s) not in excl]
    groups = {}
    for s in remaining:
        if len(s.first_uuids) < FORK_PREFIX_LEN:
            continue
        key = tuple(s.first_uuids[:FORK_PREFIX_LEN])
        groups.setdefault(key, []).append(s)

    for key, members in groups.items():
        # One representative copy per sid — same-sid duplicates were already resolved in D.
        by_member_sid = {}
        for m in members:
            by_member_sid.setdefault(m.sid, m)
        distinct = list(by_member_sid.values())
        if len(distinct) < 2:
            continue

        timelines = {s.sid: _full_timeline(s.path) for s in distinct}
        common_len = min(len(tl) for tl in timelines.values())
        divergence_idx = common_len
        for i in range(FORK_PREFIX_LEN, common_len):
            uuids_at_i = {tl[i][0] for tl in timelines.values()}
            if len(uuids_at_i) > 1:
                divergence_idx = i
                break

        def _ts_at(sid):
            tl = timelines[sid]
            if divergence_idx < len(tl):
                return tl[divergence_idx][1]
            return tl[-1][1] if tl else ""

        # R23: the ORIGINAL is whichever branch's first entry after the common prefix has
        # the earlier timestamp (first_ts ties by construction, hence not used here). Tie
        # broken by sid only for determinism; a real tie at this granularity is not expected.
        original = min(distinct, key=lambda s: (_ts_at(s.sid), s.sid))
        for s in distinct:
            if s.sid == original.sid:
                continue
            cid = copy_id(s)
            if cid in excl:
                continue
            excl[cid] = f"fork-of:{original.sid}"

    return excl


def write_exclusions(corpus_dir, exclusions_map):
    """R1: patch a frozen corpus's manifest.json in place, writing `exclusions` as a list of
    {"sid": <copy_id>, "reason": ...} sorted by sid, preserving every other manifest key.

    Safe post-freeze because archive.verify() never hashes manifest.json itself (R1). Note
    the field is literally named "sid" per the controller ruling, but holds a profile-
    qualified copy id (<profile>/<sid>), not a bare sessionId — see module docstring.
    """
    corpus_dir = pathlib.Path(corpus_dir)
    manifest_path = corpus_dir / "manifest.json"
    manifest = json.loads(manifest_path.read_text())
    manifest["exclusions"] = [
        {"sid": sid, "reason": reason} for sid, reason in sorted(exclusions_map.items())
    ]
    manifest_path.write_text(json.dumps(manifest, indent=2, sort_keys=True))
