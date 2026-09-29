"""Labelled-sample frame: which assistant messages exist, which stratum each is in, and a seeded draw.

Model-free. The frame is every assistant API message (all transcript entries sharing one
message.id) in the KEPT top-level sessions of a frozen corpus, plus the last text-bearing message
of every subagent file of a kept session (the hand-back). A message is `substantive` when it makes
a claim or takes a consequential action (REASONS), else `routine`; a hand-back is always
substantive.
"""
import pathlib
import random
import re
import sys
from dataclasses import dataclass

_HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(_HERE))
import join  # noqa: E402
import transcripts  # noqa: E402

CLAIM_PATTERNS = {
    "completion": r"\b(?:done|fixed|passes|passing|verified|confirmed|committed|green|completed?|resolved|implemented|landed|merged|pushed|shipped|works now|now works|all set)\b",
    "test_result": r"\b\d[\d,]*\s+(?:passed|failed|passing|failing|tests? pass(?:ed)?|tests? fail(?:ed)?|ignored|skipped)\b|\b0\s+(?:failed|failures|errors|warnings)\b|test result:\s*ok|\ball\s+(?:tests?\s+)?(?:pass|green)\b|\b\d+\s*/\s*\d+\s+(?:pass|tests?)\b|\bexit(?:ed)?(?: code)?\s*[=:]?\s*0\b",
    "count_noun": r"\b\d[\d,]*\s+(?:[A-Za-z][A-Za-z-]*\s+){0,2}?(?:files?|tests?|lines?|messages?|sessions?|entries|rows?|commits?|bugs?|findings?|hits?|matches|occurrences?|functions?|symbols?|tools?|items?|calls?|errors?|instances?|cases?|sites?|callers?|references?|turns?|bytes|chars|characters|tokens|docs?|artifacts?|trackers?|crates?|modules?|failures?|warnings?|assertions?|mutations?|packets?|samples?|strata|percent|%)\b",
    "absence": r"\b(?:no|none|never|nothing|zero|nowhere|nobody|neither)\b|\bnot found\b|\bno such\b|\bnot (?:present|exist|there|used|reached|called|set)\b|\b(?:doesn't|does not|don't|do not|didn't|did not|isn't|is not|aren't|are not|cannot|can't|won't|will not)\s+(?:exist|appear|contain|occur|match|find|show|carry|reach|fire|hold|have)\b|\bwithout any\b",
}
EDIT_TOOLS = {"edit_file", "edit_code", "create_file", "Write", "Edit", "MultiEdit", "NotebookEdit", "edit_markdown"}
DISPATCH_TOOLS = {"Agent", "Task"}
SHELL_TOOLS = {"run_command", "Bash"}
CONSEQ_CMD = r"\bgit\s+(commit|push|reset|rebase|checkout|stash|clean)\b|\brm\s+-|\bcargo\s+rb\b|\brb\.sh\b"
CATALOG_TOOLS = {"doc", "memory"}
CATALOG_WRITE_ACTIONS = {"create", "update", "move", "delete", "graft", "link", "append_entry", "update_entry",
                         "rekey_prefix", "event_create", "augment", "write", "remember", "forget"}
REASONS = ("claim_marker", "end_turn", "edit", "git_or_rm_or_release", "dispatch", "catalog_write", "handback")

STRATA = ("substantive", "routine")

_CLAIM_RES = [re.compile(p, re.IGNORECASE) for p in CLAIM_PATTERNS.values()]
_CONSEQ_RE = re.compile(CONSEQ_CMD)


@dataclass(frozen=True)
class Unit:
    case_key: str
    copy_id: str
    transcript: str
    message_id: str
    kind: str
    stratum: str
    reasons: tuple
    first_entry_index: int
    decision_ts: str


def _content(entry):
    content = (entry.get("message") or {}).get("content")
    if isinstance(content, str):
        return [{"type": "text", "text": content}]
    return [b for b in (content or []) if isinstance(b, dict)]


def _text(entries):
    parts = []
    for e in entries:
        for b in _content(e):
            if b.get("type") == "text" and isinstance(b.get("text"), str):
                parts.append(b["text"])
    return "\n".join(parts)


def _tool_uses(entries):
    """[(bare_tool_name, input_dict)]; a name is compared after its last `__`."""
    out = []
    for e in entries:
        for b in _content(e):
            if b.get("type") == "tool_use":
                inp = b.get("input")
                out.append(((b.get("name") or "").rsplit("__", 1)[-1], inp if isinstance(inp, dict) else {}))
    return out


def classify(message_entries, handback=False):
    """(stratum, reasons) for one assistant message given as its entries. Reasons follow REASONS order."""
    text = _text(message_entries)
    tools = _tool_uses(message_entries)
    found = set()
    if any(r.search(text) for r in _CLAIM_RES):
        found.add("claim_marker")
    if text.strip() and any((e.get("message") or {}).get("stop_reason") == "end_turn" for e in message_entries):
        found.add("end_turn")
    for name, inp in tools:
        if name in EDIT_TOOLS:
            found.add("edit")
        if name in SHELL_TOOLS and _CONSEQ_RE.search(str(inp.get("command") or "")):
            found.add("git_or_rm_or_release")
        if name in DISPATCH_TOOLS:
            found.add("dispatch")
        if name in CATALOG_TOOLS and inp.get("action") in CATALOG_WRITE_ACTIONS:
            found.add("catalog_write")
    if handback:
        found.add("handback")
    reasons = tuple(r for r in REASONS if r in found)
    return ("substantive" if reasons else "routine"), reasons


def _messages(entries, skip_sidechain):
    """[(message_id, first_entry_index, [entries])] for assistant messages, in first-appearance order.
    Messages that are synthetic or API errors are dropped whole."""
    order = []
    groups = {}
    for i, e in enumerate(entries):
        if e.get("type") != "assistant":
            continue
        if skip_sidechain and e.get("isSidechain"):
            continue
        mid = (e.get("message") or {}).get("id") or e.get("uuid")
        if mid not in groups:
            groups[mid] = (i, [])
            order.append(mid)
        groups[mid][1].append(e)
    out = []
    for mid in order:
        idx, ents = groups[mid]
        if any((e.get("message") or {}).get("model") == "<synthetic>" or e.get("isApiErrorMessage") for e in ents):
            continue
        out.append((mid, idx, ents))
    return out


def _make_unit(cid, rel, mid, kind, idx, ents, handback):
    stratum, reasons = classify(ents, handback=handback)
    return Unit(case_key=f"{cid}|{rel}|{mid}", copy_id=cid, transcript=rel, message_id=mid, kind=kind,
                stratum=stratum, reasons=reasons, first_entry_index=idx,
                decision_ts=ents[0].get("timestamp") or "")


def frame(corpus_dir, excluded_sids=None):
    """Every unit of the kept sessions of `corpus_dir`. None means join.SPEC_EXCLUDED_SIDS."""
    corpus_dir = pathlib.Path(corpus_dir)
    if excluded_sids is None:
        excluded_sids = join.SPEC_EXCLUDED_SIDS
    all_sessions = transcripts.sessions(corpus_dir)
    excl = transcripts.exclusions(all_sessions, excluded_sids)
    units = []
    for s in all_sessions:
        cid = transcripts.copy_id(s)
        if cid in excl:
            continue
        rel = s.path.relative_to(corpus_dir).as_posix()
        entries, _ = transcripts.read_jsonl(s.path)
        for mid, idx, ents in _messages(entries, skip_sidechain=True):
            units.append(_make_unit(cid, rel, mid, "top", idx, ents, False))
        for sub in s.subagent_paths:
            sub_entries, _ = transcripts.read_jsonl(sub)
            with_text = [m for m in _messages(sub_entries, skip_sidechain=False) if _text(m[2]).strip()]
            if with_text:
                mid, idx, ents = with_text[-1]
                units.append(_make_unit(cid, sub.relative_to(corpus_dir).as_posix(), mid, "handback",
                                        idx, ents, True))
    return units


def draw(units, sizes, seed, exclude=frozenset()):
    """Seeded stratified sample without replacement: per stratum (substantive, then routine) sort by
    case_key, remove `exclude`, take random.Random(seed).sample of the requested size."""
    unknown = set(sizes) - set(STRATA)
    if unknown:
        raise ValueError(f"unknown strata in sizes: {sorted(unknown)}")
    rng = random.Random(seed)
    out = []
    for stratum in STRATA:
        if stratum not in sizes:
            continue
        pool = sorted((u for u in units if u.stratum == stratum and u.case_key not in exclude),
                      key=lambda u: u.case_key)
        if sizes[stratum] > len(pool):
            raise ValueError(f"stratum {stratum!r}: asked for {sizes[stratum]}, only {len(pool)} candidates")
        out.extend(rng.sample(pool, sizes[stratum]))
    return out


def frame_counts(units):
    """{stratum: {kind: n}} over the units given."""
    counts = {}
    for u in units:
        by_kind = counts.setdefault(u.stratum, {})
        by_kind[u.kind] = by_kind.get(u.kind, 0) + 1
    return counts
