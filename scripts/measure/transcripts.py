"""Stage 1a — sessions, exclusions, fork detection over a FROZEN corpus.

Never reads a live ~/.claude* directory directly: every function here takes a corpus_dir
produced by archive.freeze(), whose layout is
  <corpus_dir>/transcripts/<NN>-<profile>/<project_slug>/<sid>.jsonl
  <corpus_dir>/transcripts/<NN>-<profile>/<project_slug>/<sid>/subagents/<name>.jsonl
(see archive.freeze()'s dest_project_dir / dest_sub_dir construction — profile_name there is
src_dir.parent.parent.name, e.g. the literal ".claude-kat").

Exclusion reasons (Task 5 brief + controller rulings R1/R22/R23/R24/R26/R27/R29; R28 superseded
R23's fork EXCLUSION — see below):
  sdk-cli                       entrypoint == "sdk-cli" (headless `claude -p`)
  scratchpad-project            project slug names a scratchpad checkout
  excluded-by-spec              sid explicitly named by the caller (e.g. this measurement's
                                 own controlling session, or a session named in the plan)
  duplicate-prefix-of:<copy>    R22: same sessionId in two profiles, this copy's uuid list is
                                 a verified exact prefix of the kept copy's
  divergent-duplicate-of:<copy> R22: same sessionId in two profiles, neither is a prefix of the
                                 other — the longer copy is kept

Every key in the exclusions() dict is a copy id `<profile>/<sid>` (see copy_id()) — never a
bare sid — so the R22 case (one sid, two profiles) cannot collide with itself.

R28: forks are NO LONGER an exclusion reason (a real pair showed R23 excluding a copy with 64
real operator prompts as `fork-of:`, discarding content that was never actually duplicated —
the two transcripts diverge early and each carries its own unique tail). Fork *relationships*
are now reporting-only, via relations() (R23's same detection + orientation, unchanged), and
overlap in the counted operator-message uuids is resolved separately by attribute_entries(),
which assigns each uuid to exactly one owning transcript so a fork pair's shared prefix is
counted once rather than twice.

operator_messages() additionally excludes (R26) harness task-notification entries
(`promptSource == "system"` or `origin.kind == "task-notification"`, or — for older entries
carrying neither field — text starting with `<task-notification>`), (R27) the
`<command-message>...</command-message>` skill-command wrapper (the real order skill commands
are written in, which the original `<command-name>`-only check missed), and (R29) a bare slash
command with no arguments (stripped text matching `^/[a-z][a-z0-9-]*$`, e.g. `/compact`).
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
    # R27: the real skill-command order is "<command-message>X</command-message>\n
    # <command-name>/X</command-name>" — command-message comes FIRST, so without this the
    # startswith(COMMAND_WRAPPER_TAGS) check missed every one of the 66 real entries in that
    # exact order (a `<command-name>`-only check only catches text that happens to START with
    # command-name, which the real wrapper never does).
    "<command-message>",
)

# R26: harness task-notification entries, excluded as a fallback text prefix for OLDER entries
# that carry neither `promptSource` nor `origin.kind` (see _is_task_notification below for the
# primary, field-based check — this is the fallback the primary check cannot reach).
TASK_NOTIFICATION_TAG = "<task-notification>"

# R29: a bare slash command with no arguments (202 `/compact` in the real corpus) is harness
# scaffolding, not an operator prompt. Anchored full-string match (^...$) on the STRIPPED text,
# so "/review this function please" — a real prompt that happens to start with a slash command
# token followed by more words — does not match and stays a prompt.
BARE_SLASH_COMMAND_RE = re.compile(r"^/[a-z][a-z0-9-]*$")

# R25: an entry whose whole text, after strip(), EQUALS exactly one of these literals is an
# operator INTERRUPT, not a prompt — exact equality, never substring, so a real prompt that
# merely quotes one of these strings still counts as a prompt. Measured across all 3 profiles:
# 160 x "[Request interrupted by user]", 38 x "[Request interrupted by user for tool use]".
INTERRUPT_MARKERS = (
    "[Request interrupted by user]",
    "[Request interrupted by user for tool use]",
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


def _is_task_notification(entry):
    """R26: the primary, field-based check for a harness task-notification entry —
    `promptSource == "system"` or `origin.kind == "task-notification"`. Checked before the
    TASK_NOTIFICATION_TAG text-prefix fallback (module docstring), which exists only for
    older entries carrying neither field.
    """
    if entry.get("promptSource") == "system":
        return True
    origin = entry.get("origin")
    if isinstance(origin, dict) and origin.get("kind") == "task-notification":
        return True
    return False


def operator_messages(entries):
    """Keep only type=="user" entries that are real human prompts.

    Excludes (Review Focus 3): isMeta entries (skill-loading injections etc, whether their
    content is a list or a string), compaction summaries (isCompactSummary), tool results
    (message.content a list not leading with a "text" item — so not recognized by
    _message_text at all), messages wrapped in <command-name>, <local-command-stdout>,
    <local-command-caveat> or (R27) <command-message> (slash-command scaffolding, not the
    operator's own words — see COMMAND_WRAPPER_TAGS for why <command-message> had to join
    this tuple rather than being checked separately: it is the FIRST tag in the real
    skill-command wrapper order, so a <command-name>-only check missed every one of it),
    (R25) operator-interrupt markers (see operator_interrupts() — the same text extraction
    and the same strip()-equality check decide both functions, so there is exactly one place
    that decision is made), (R26) harness task-notification entries (structural
    promptSource/origin.kind check via _is_task_notification(), with a text-prefix fallback
    for older entries carrying neither field), and (R29) a bare slash command with no
    arguments (e.g. "/compact" — BARE_SLASH_COMMAND_RE, anchored so a real prompt that
    merely starts with a slash-command-shaped token followed by more words still counts).
    """
    kept = []
    for entry in entries:
        if entry.get("type") != "user":
            continue
        if entry.get("isMeta"):
            continue
        if entry.get("isCompactSummary"):
            continue
        if _is_task_notification(entry):
            continue
        text = _message_text(entry)
        if text is None:
            continue
        stripped = text.strip()
        if stripped in INTERRUPT_MARKERS:
            continue
        if text.startswith(COMMAND_WRAPPER_TAGS):
            continue
        if text.startswith(TASK_NOTIFICATION_TAG):
            continue
        if BARE_SLASH_COMMAND_RE.match(stripped):
            continue
        kept.append(entry)
    return kept


def operator_interrupts(entries):
    """R25: return exactly the entries operator_messages() drops as interrupt markers.

    Same structural filters as operator_messages() (type=="user", not isMeta, not a
    compaction summary) and the same _message_text() extraction — this and
    operator_messages() are the only two places INTERRUPT_MARKERS is consulted, and both
    consult it via strip()-equality, never substring, so a prompt that merely quotes a
    marker is excluded here and kept there.
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
        if text.strip() in INTERRUPT_MARKERS:
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

    R28: forks (formerly stage E, `fork-of:<sid>`) are NO LONGER an exclusion reason — see
    module docstring and relations()/attribute_entries() for the replacement model.
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

    return excl


def _sid_keepers(sessions_list):
    """For each sid, the R22 "keeper" copy — the longest timeline, ties broken by copy_id
    for determinism (same rule exclusions() Stage D applies). relations() needs this
    independently of exclusions(): per R28(ii)'s signature, relations(sessions) takes no
    exclusions map, so when a sid itself has more than one profile copy it must re-derive
    which copy is canonical (the one R22 would keep) rather than being told.
    """
    by_sid = {}
    for s in sessions_list:
        by_sid.setdefault(s.sid, []).append(s)
    keepers = {}
    for sid, copies in by_sid.items():
        if len(copies) == 1:
            keepers[sid] = copies[0]
            continue
        timelines = {copy_id(c): _full_timeline(c.path) for c in copies}
        ordered = sorted(copies, key=lambda c: (-len(timelines[copy_id(c)]), copy_id(c)))
        keepers[sid] = ordered[0]
    return keepers


def relations(sessions_list):
    """R28(ii): fork relationships, reporting-only — forks no longer exclude anything (see
    module docstring and exclusions(), which dropped stage E).

    Returns {copy_id(fork): "fork-of:<copy_id(original)>"}. Detection is unchanged from
    R23: sessions with different sids whose first FORK_PREFIX_LEN uuids are identical are a
    fork group; the ORIGINAL is whichever branch's first entry after the common prefix has
    the earlier timestamp (first_ts ties by construction across a shared prefix, hence not
    used for orientation). When a sid itself has more than one profile copy (R22), the
    canonical representative used for grouping/orientation is that sid's R22 keeper (see
    _sid_keepers) — so a fork reference points at the copy_id that survives R22, matching
    the controller's R24 update ("d8a1f024 now appears in relations() as
    fork-of:<571eb3d6's kept copy_id>").
    """
    representatives = list(_sid_keepers(sessions_list).values())

    groups = {}
    for s in representatives:
        if len(s.first_uuids) < FORK_PREFIX_LEN:
            continue
        key = tuple(s.first_uuids[:FORK_PREFIX_LEN])
        groups.setdefault(key, []).append(s)

    rels = {}
    for key, distinct in groups.items():
        # `representatives` holds exactly one session per sid, so `distinct` already has no
        # same-sid duplicates to collapse — unlike the old stage E, no by_member_sid pass.
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
        # the earlier timestamp. Tie broken by sid only for determinism.
        original = min(distinct, key=lambda s: (_ts_at(s.sid), s.sid))
        original_cid = copy_id(original)
        for s in distinct:
            if s.sid == original.sid:
                continue
            rels[copy_id(s)] = f"fork-of:{original_cid}"

    return rels


def attribute_entries(sessions_list, exclusions_map):
    """R28(iii): map each operator-message uuid to exactly one owning transcript (copy_id),
    over non-excluded transcripts, resolving the double-counting a fork pair's (or any
    other overlapping pair's) shared prefix would otherwise cause.

    Each uuid is attributed to the containing transcript with the MOST operator-message
    uuids; ties are broken by earliest first_ts, then lexically smallest copy_id. This is
    computed as one global sort of candidate sessions by (-count, first_ts, copy_id),
    assigning each of a session's uuids to it only if no earlier (in that order) session
    already claimed it — equivalent to independently picking, per uuid, the max-count
    containing session with the same tie-break, because a total order restricts
    consistently onto any subset (the subset of sessions containing that uuid).
    """
    candidates = [s for s in sessions_list if copy_id(s) not in exclusions_map]

    uuid_sets = {}
    counts = {}
    for s in candidates:
        entries, _skipped = read_jsonl(s.path)
        uuids = {
            e.get("uuid") for e in operator_messages(entries) if e.get("uuid") is not None
        }
        cid = copy_id(s)
        uuid_sets[cid] = uuids
        counts[cid] = len(uuids)

    ordered = sorted(
        candidates, key=lambda s: (-counts[copy_id(s)], s.first_ts or "", copy_id(s))
    )

    attribution = {}
    for s in ordered:
        cid = copy_id(s)
        for u in uuid_sets[cid]:
            if u not in attribution:
                attribution[u] = cid
    return attribution


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
