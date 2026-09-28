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
  fork-of-excluded:<copy>       R106: shares a FORK_PREFIX_LEN uuid prefix with an
                                 excluded-by-spec copy (a fork inherits its parent's entries)
  duplicate-prefix-of:<copy>    R22: same sessionId in two profiles, this copy's uuid list is
                                 a verified exact prefix of the kept copy's
  divergent-duplicate-of:<copy> R22: same sessionId in two profiles, neither is a prefix of the
                                 other — the longer copy is kept

Every key in the exclusions() dict is a copy id `<profile>/<sid>` (see copy_id()) — never a
bare sid — so the R22 case (one sid, two profiles) cannot collide with itself.

R28: forks are NO LONGER an exclusion reason (a real pair showed R23 excluding a copy with 47
real operator prompts as `fork-of:`, discarding content that was never actually duplicated —
the two transcripts diverge early and each carries its own unique tail). Fork *relationships*
are now reporting-only, via relations() (R23's same detection + orientation, unchanged), and
overlap in the counted entry uuids (R31: every entry, not just operator messages) is resolved
separately by attribute_entries(), which assigns each uuid to exactly one owning transcript so
a fork pair's shared prefix is counted once rather than twice.

operator_messages() additionally excludes (R26) harness task-notification entries
(`promptSource == "system"` or `origin.kind == "task-notification"`, or — for older entries
carrying neither field — text starting with `<task-notification>`), (R27) the
`<command-message>...</command-message>` skill-command wrapper (the real order skill commands
are written in, which the original `<command-name>`-only check missed), and (R29) a bare slash
command with no arguments (stripped text matching `^/[a-z][a-z0-9-]*$`, e.g. `/compact`).

Task 14 (spec Amendment 7): operator_messages() also keeps (R103) a `queued_command`
attachment the operator typed mid-turn, and identifies the operator POSITIVELY (R104) wherever an
entry carries an `origin`; operator_rejections() (R105) returns tool-rejection feedback;
exclusions() propagates the spec exclusion to forks (R106); sessions() flags a changed entrypoint
(R107); read_jsonl() survives a torn UTF-8 line (R113).
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

# R104: POSITIVE identification. A type:user entry carrying an `origin` dict is an operator
# message only if origin.kind == "human" (measured 2026-09-28: all 2592 kept prompts carry it).
# These leading tags are the FALLBACK for origin-less (older) entries only -- machine channels
# the harness writes as type:user text. Several are open tags that carry attributes
# (`<cross-session-message from=...>`), hence no closing ">" on those.
ORIGINLESS_FALLBACK_TAGS = (
    "<cross-session-message",
    "<teammate-message",
    "<agent-message",
    "<bash-input>",
    "<bash-stdout>",
    "<bash-stderr>",
    "<local-command-stderr>",
    "<system-reminder>",
)

# R103: a message the operator types while the agent runs is written as a `type:"attachment"`
# entry whose attachment.type is this. Measured 2026-09-28 on the Task 14 freeze (138 sessions):
# 2095 such attachments; commandMode "prompt" 1262 (attachment origin human 171, peer 1091),
# "task-notification" 833; every attachment.prompt a str; no entry-level origin on any.
QUEUED_COMMAND_TYPE = "queued_command"

# R105: the harness marker a tool_result carries when the operator rejects a tool use and types
# feedback. Measured 2026-09-28: 10 blocks in 9 kept sessions, one block per entry.
REJECTION_MARKER = "the user said:"

_PROFILE_DIR_RE = re.compile(r"^\d+-(.+)$")


def read_jsonl(path, stats=None):
    """Read a transcript file line by line. Returns (entries, skipped_count).

    A malformed line (JSON decode failure — e.g. a process killed mid-write leaves a
    truncated final line) is counted and skipped rather than raising, per Review Focus 4.
    Blank lines are skipped without counting as malformed.

    R113: the file is decoded as UTF-8 with errors="replace", so a torn multi-byte sequence
    becomes U+FFFD instead of raising UnicodeDecodeError and aborting a whole build. The return
    shape is unchanged; the documented extension is `stats`: pass a dict and this ADDS to it
    (so one dict can accumulate across files) `skipped` (the same count as the return value)
    and `replaced_lines` -- lines that contain U+FFFD after decoding. That count cannot tell a
    decode replacement from a U+FFFD the writer put there itself, so it is an upper bound on
    torn lines, never a lower one.
    """
    entries = []
    skipped = 0
    replaced = 0
    with open(path, "r", encoding="utf-8", errors="replace") as f:
        for raw_line in f:
            line = raw_line.strip()
            if not line:
                continue
            if "�" in line:
                replaced += 1
            try:
                entries.append(json.loads(line))
            except json.JSONDecodeError:
                skipped += 1
    if stats is not None:
        stats["skipped"] = stats.get("skipped", 0) + skipped
        stats["replaced_lines"] = stats.get("replaced_lines", 0) + replaced
    return entries, skipped


def _message_text(entry):
    """Return the entry's message text, or None if it has none we recognize.

    Content qualifies only if it is a string, or a list whose first item is a {"type":
    "text", ...} dict (the shape a real prompt takes when it carries attachments, e.g. an
    imagePasteIds sibling key) — per Review Focus 3 / fact 2.

    R103: a `queued_command` attachment (a message typed while the agent ran) carries no
    `message`; its text is `attachment.prompt` when that is a str (all 2095 measured are), so
    join records a queued prompt through this same function. Any other attachment is None.
    """
    message = entry.get("message")
    if not isinstance(message, dict):
        if entry.get("type") == "attachment":
            att = entry.get("attachment")
            if isinstance(att, dict) and att.get("type") == QUEUED_COMMAND_TYPE:
                prompt = att.get("prompt")
                if isinstance(prompt, str):
                    return prompt
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


def _normalize_ws(text):
    """R103's dedupe key: whitespace-normalized text (runs of whitespace -> one space)."""
    return " ".join(text.split())


def _is_operator_user_entry(entry):
    """The type:"user" half of operator_messages() -- see its docstring for every exclusion."""
    if entry.get("type") != "user":
        return False
    # R104: the old-layout top-level agent-<id>.jsonl opens with the parent model's brief as an
    # isSidechain type:user entry; a sidechain entry is never the operator speaking.
    if entry.get("isSidechain"):
        return False
    if entry.get("isMeta"):
        return False
    if entry.get("isCompactSummary"):
        return False
    if _is_task_notification(entry):
        return False
    # R104: positive identification wherever the harness says who spoke.
    origin = entry.get("origin")
    has_origin = isinstance(origin, dict)
    if has_origin and origin.get("kind") != "human":
        return False
    text = _message_text(entry)
    if text is None:
        return False
    stripped = text.strip()
    if stripped in INTERRUPT_MARKERS:
        return False
    if text.startswith(COMMAND_WRAPPER_TAGS):
        return False
    if text.startswith(TASK_NOTIFICATION_TAG):
        return False
    if not has_origin and text.startswith(ORIGINLESS_FALLBACK_TAGS):
        return False
    if BARE_SLASH_COMMAND_RE.match(stripped):
        return False
    return True


def _is_queued_operator_prompt(entry):
    """R103: a `queued_command` attachment the operator typed mid-turn. ALL must hold:
    attachment.type == "queued_command", attachment.commandMode == "prompt", and the
    ATTACHMENT's own attachment.origin.kind == "human" -- never an entry-level origin, which
    no measured attachment carries: every prompt-mode attachment carries an attachment origin
    (1091 peer + 171 human), so reading the wrong field would admit every peer message. An
    attachment with no origin never counts. Not isMeta (the ledger's R103) and not isSidechain
    (R104). Its text must be a str (_message_text)."""
    if entry.get("type") != "attachment":
        return False
    if entry.get("isMeta") or entry.get("isSidechain"):
        return False
    att = entry.get("attachment")
    if not isinstance(att, dict):
        return False
    if att.get("type") != QUEUED_COMMAND_TYPE or att.get("commandMode") != "prompt":
        return False
    origin = att.get("origin")
    if not (isinstance(origin, dict) and origin.get("kind") == "human"):
        return False
    return _message_text(entry) is not None


def operator_messages(entries):
    """Keep only the entries that are the operator speaking, in file order (R2: the ONE
    definition -- join's turns.kind='prompt' is exactly this set).

    A type=="user" entry is kept unless it is: (R104) an isSidechain entry; an isMeta entry
    (skill-loading injections etc, whether their content is a list or a string); a compaction
    summary (isCompactSummary); (R26) a harness task-notification (structural
    promptSource/origin.kind check via _is_task_notification(), with the TASK_NOTIFICATION_TAG
    text-prefix fallback); (R104) an entry whose `origin` dict names anyone but the human
    (origin.kind != "human"); a tool result (message.content a list not leading with a "text"
    item -- so not recognized by _message_text at all); (R25) an operator-interrupt marker (see
    operator_interrupts() -- the same text extraction and the same strip()-equality check decide
    both functions); a message wrapped in <command-name>, <local-command-stdout>,
    <local-command-caveat> or (R27) <command-message> (slash-command scaffolding -- see
    COMMAND_WRAPPER_TAGS for why <command-message> had to join that tuple); (R104) for an
    ORIGIN-LESS entry only, text starting with one of ORIGINLESS_FALLBACK_TAGS (machine channels
    written as user text); or (R29) a bare slash command with no arguments (e.g. "/compact" --
    BARE_SLASH_COMMAND_RE, anchored so a real prompt that merely starts with a
    slash-command-shaped token followed by more words still counts).

    R103: a `queued_command` attachment the operator typed mid-turn is kept too (see
    _is_queued_operator_prompt), UNLESS its whitespace-normalized text equals that of a kept
    type:user operator message LATER in the same `entries` list -- the same message, echoed as
    a prompt once the turn ended; the user entry is kept and the attachment dropped. A kept
    user message EARLIER in the list never dedupes it.
    """
    kept_reversed = []
    later_user_texts = set()
    for entry in reversed(entries):
        if _is_operator_user_entry(entry):
            kept_reversed.append(entry)
            later_user_texts.add(_normalize_ws(_message_text(entry)))
        elif _is_queued_operator_prompt(entry):
            if _normalize_ws(_message_text(entry)) in later_user_texts:
                continue
            kept_reversed.append(entry)
    kept_reversed.reverse()
    return kept_reversed


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


def _tool_result_text(block):
    """A tool_result block's text: its `content` when a str, else the joined text items of a
    list-shaped `content`; None for any other shape."""
    content = block.get("content")
    if isinstance(content, str):
        return content
    if isinstance(content, list):
        return "\n".join(
            item["text"]
            for item in content
            if isinstance(item, dict) and item.get("type") == "text"
            and isinstance(item.get("text"), str)
        )
    return None


def operator_rejections(entries):
    """R105: tool-rejection feedback, the operator's most explicit correction. Returns, in file
    order, one {"uuid", "ts", "text"} per type:"user" entry whose message.content LIST holds a
    tool_result block whose text contains REJECTION_MARKER; "text" is everything after the
    marker's first occurrence, stripped. A marker inside a plain text block (not a tool_result)
    never counts. Every measured entry holds exactly one such block; if one ever held several,
    their texts are joined with a blank line, so the entry still yields ONE dict (one turns row).

    Known latent shape, measured 2026-09-28: a tool_result that merely QUOTES the marker (e.g. a
    grep over these docs) matches too -- 1 such block, in the review session 82cff72e, which R106
    excludes; the 10 genuine blocks are all is_error=True and carry the harness's "The user
    doesn't want to proceed with this tool use" prefix. The rule as ruled does not check either.
    """
    out = []
    for entry in entries:
        if entry.get("type") != "user":
            continue
        message = entry.get("message")
        content = message.get("content") if isinstance(message, dict) else None
        if not isinstance(content, list):
            continue
        texts = []
        for block in content:
            if not (isinstance(block, dict) and block.get("type") == "tool_result"):
                continue
            text = _tool_result_text(block)
            if text and REJECTION_MARKER in text:
                texts.append(text.split(REJECTION_MARKER, 1)[1].strip())
        if texts:
            out.append(
                {"uuid": entry.get("uuid"), "ts": entry.get("timestamp"), "text": "\n\n".join(texts)}
            )
    return out


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
    # R107: the LAST non-empty entrypoint in the file, and whether it differs from the first
    # (`entrypoint`). The session's class stays first-entry; the flag only makes a change visible.
    last_entrypoint: str = None
    entrypoint_changed: bool = False


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
                last_entrypoint = None
                first_ts = None
                last_ts = None
                first_uuids = []
                for entry in entries:
                    if entry.get("entrypoint"):
                        if entrypoint is None:
                            entrypoint = entry.get("entrypoint")
                        last_entrypoint = entry.get("entrypoint")
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
                        last_entrypoint=last_entrypoint,
                        entrypoint_changed=entrypoint != last_entrypoint,
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


def _prefix_key(session):
    """The fork-detection key: the tuple of a copy's first FORK_PREFIX_LEN uuids, or None when
    it has fewer. Two copies share a prefix of at least FORK_PREFIX_LEN iff their keys are equal.
    The ONE definition behind relations()' fork groups and exclusions()' R106 stage A2."""
    if len(session.first_uuids) < FORK_PREFIX_LEN:
        return None
    return tuple(session.first_uuids[:FORK_PREFIX_LEN])


def exclusions(sessions_list, excluded_sids):
    """Build the exclusion map for a list of Session objects.

    Returns {copy_id(session): reason}. Staged, highest priority first, so a copy already
    excluded by an earlier stage is never re-labelled by a later one:
      A excluded-by-spec  A2 fork-of-excluded:<copy> (R106)  B sdk-cli  C scratchpad-project
      D R22 same-sid duplicates

    R28: forks (formerly stage E, `fork-of:<sid>`) are NO LONGER an exclusion reason — see
    module docstring and relations()/attribute_entries() for the replacement model.
    """
    excl = {}

    # --- A: excluded-by-spec (explicit override; wins over anything else) ---
    for s in sessions_list:
        if s.sid in excluded_sids:
            excl[copy_id(s)] = "excluded-by-spec"

    # --- A2: R106 fork propagation -- fork-of-excluded:<copy> ---
    # A copy sharing a FORK_PREFIX_LEN uuid prefix (the relations() key, _prefix_key) with an
    # excluded-by-spec copy inherits that copy's entries, so it is excluded too. Seeded from the
    # SPEC reason only, never from every excluded copy: an R22 duplicate shares its prefix with
    # its own KEPT keeper by construction, and sdk-cli/scratchpad forks are R28's
    # reporting-only relations. The match is symmetric ("shares a prefix"), so a kept copy that
    # an excluded session was itself forked FROM is excluded too -- R106's whole-session grain,
    # disclosed; measured 2026-09-28, no copy shares a prefix with either spec sid.
    spec_prefixes = {}
    for s in sorted(sessions_list, key=copy_id):
        key = _prefix_key(s)
        if key is not None and excl.get(copy_id(s)) == "excluded-by-spec":
            spec_prefixes.setdefault(key, copy_id(s))
    for s in sessions_list:
        cid = copy_id(s)
        if cid in excl:
            continue
        key = _prefix_key(s)
        if key is not None and key in spec_prefixes:
            excl[cid] = f"fork-of-excluded:{spec_prefixes[key]}"

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
    # R32/R36: the keeper is decided by _sid_keepers() — the SAME function relations() uses,
    # and (since R36/F4) over the SAME non-excluded population this stage builds `remaining`
    # from — so the two cannot disagree about which copy of a tied-length sid is canonical.
    # Before this fix, this stage sorted independently by length alone, no copy_id
    # tie-break at all, so a length TIE fell to Python's stable sort / remaining's
    # iteration order (which follows profile-directory on-disk name, numeric prefix
    # included) while _sid_keepers() broke the same tie by copy_id. On a genuine tie the
    # two sorts could and did pick different keepers, so this stage could exclude a copy
    # that relations() still named as a fork's kept original.
    remaining = [s for s in sessions_list if copy_id(s) not in excl]
    by_sid = {}
    for s in remaining:
        by_sid.setdefault(s.sid, []).append(s)
    keepers = _sid_keepers(remaining)
    for sid, copies in by_sid.items():
        if len(copies) < 2:
            continue
        keeper = keepers[sid]
        keeper_id = copy_id(keeper)
        timelines = {copy_id(c): _full_timeline(c.path) for c in copies}
        keeper_tl = timelines[keeper_id]
        for other in copies:
            other_id = copy_id(other)
            if other_id == keeper_id:
                continue
            other_tl = timelines[other_id]
            n = len(other_tl)
            if other_tl == keeper_tl[:n]:
                excl[other_id] = f"duplicate-prefix-of:{keeper_id}"
            else:
                excl[other_id] = f"divergent-duplicate-of:{keeper_id}"

    return excl


def _sid_keepers(sessions_list):
    """For each sid, the R22 "keeper" copy — the longest timeline, ties broken by copy_id
    for determinism. relations() needs this independently of exclusions(): even though it
    now (R36/F4) takes an exclusions_map of its own and is called over the non-excluded
    population, when a sid itself has more than one surviving profile copy it must still
    re-derive which copy is canonical (the one R22 would keep) rather than being told.

    R32: exclusions() Stage D now calls this same function rather than re-implementing its
    own sort. R36 additionally makes relations() call it over the SAME non-excluded
    population Stage D uses (not the raw, unfiltered sessions_list) — so the two callers
    cannot pick different keepers for the same sid.
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


def relations(sessions_list, exclusions_map):
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

    R36: computed only over NON-excluded copies — sessions whose copy_id is in
    exclusions_map (any reason: R22 duplicate, sdk-cli, scratchpad, ...) are dropped before
    grouping, and _sid_keepers() itself runs over that same filtered population. So an
    excluded copy can never be named as a fork's original, and can never own a relation
    entry of its own. When a sid's would-be keeper is excluded by some OTHER stage (e.g. a
    longer sdk-cli copy dropped at Stage B), the surviving copy of that sid stands in as the
    representative; if every copy of a sid is excluded, that sid contributes no
    representative at all and its fork group produces no relation for it.
    """
    non_excluded = [s for s in sessions_list if copy_id(s) not in exclusions_map]
    representatives = list(_sid_keepers(non_excluded).values())

    groups = {}
    for s in representatives:
        key = _prefix_key(s)
        if key is None:
            continue
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
    """R28(iii)/R31: map each ENTRY uuid — every entry in the transcript (prompts, assistant
    messages, tool results, meta entries, all of them), not only operator-message uuids — to
    exactly one owning transcript (copy_id), over non-excluded transcripts, resolving the
    double-counting a fork pair's (or any other overlapping pair's) shared prefix would
    otherwise cause.

    R31: counting and attributing every entry (not just operator messages) matters because
    the downstream audit samples ASSISTANT messages — restricting this map to operator-
    message uuids would silently omit every assistant-message uuid from it entirely, and
    could pick the wrong owner for a fork pair's shared prefix, whose decision points are
    exactly what the audit is sampling.

    Each uuid is attributed to the containing transcript with the MOST total uuids; ties
    are broken by earliest first_ts, then lexically smallest copy_id. This is computed as
    one global sort of candidate sessions by (-count, first_ts, copy_id), assigning each of
    a session's uuids to it only if no earlier (in that order) session already claimed it —
    equivalent to independently picking, per uuid, the max-count containing session with the
    same tie-break, because a total order restricts consistently onto any subset (the subset
    of sessions containing that uuid).
    """
    candidates = [s for s in sessions_list if copy_id(s) not in exclusions_map]

    uuid_sets = {}
    counts = {}
    for s in candidates:
        entries, _skipped = read_jsonl(s.path)
        uuids = {e.get("uuid") for e in entries if e.get("uuid") is not None}
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


def read_exclusions(corpus_dir):
    """R106: the inverse of write_exclusions() -- manifest.json's `exclusions` as the
    {copy_id: reason} map exclusions() returns, so a reader (observability.coverage) compares
    the recorded set with its own recomputation. A manifest without the key RAISES KeyError."""
    manifest = json.loads((pathlib.Path(corpus_dir) / "manifest.json").read_text())
    return {rec["sid"]: rec["reason"] for rec in manifest["exclusions"]}


def entrypoint_changed_count(sessions_list, exclusions_map):
    """R107: the number of KEPT copies (copy_id not in exclusions_map) whose first and last
    entrypoint differ (Session.entrypoint_changed). Measured 2026-09-28: 0."""
    return sum(
        1 for s in sessions_list
        if copy_id(s) not in exclusions_map and s.entrypoint_changed
    )
