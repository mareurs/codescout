"""Labelled-sample packet: the blinded text an operator reads to label ONE assistant message.

Model-free. A packet shows the operator's last message (or, for a hand-back, the dispatch prompt),
the last CONTEXT_MESSAGES assistant messages before the decision point WITH their tool output, and
the message itself with the tool calls it is about to run. Nothing at or after the decision point
except the message's own entries. What is blinded (replaced by a placeholder) is limited to: uuid-shaped
strings (session and message ids), calendar timestamps, and API ids of the form msg_01... / toolu_01.... A
BARE calendar date is kept everywhere (prose, file names): only a date WITH a time of day, ISO `T` or
space separated, is blinded, because _TIMESTAMP_RE requires the time. A lone surrogate (a truncated
emoji) becomes U+FFFD so the text can be hashed as UTF-8. The packet is at most PACKET_CHARS characters
in total, always.

A token-shaped string refuses the build (TokenFound). The check is made on the SOURCE of every piece
before any cut (a token half-cut by a tail/head/trim would otherwise render its secret body while the
final text no longer matches the pattern): it refuses when a match OVERLAPS the range that is kept,
and does not refuse for a match wholly inside dropped material (a dropped head, or a context message
removed by the cap), which shows the operator nothing.
"""
import hashlib
import json
import pathlib
import re
import sys
from dataclasses import dataclass

_HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(_HERE))
import sampler  # noqa: E402
import transcripts  # noqa: E402

CONTEXT_MESSAGES = 6
OPERATOR_CHARS = 1500
RESULT_TAIL_CHARS = 1500
ARGS_CHARS = 300
PACKET_CHARS = 20000
TRIM_MARKER = "[… the earlier part of this message is not shown]"
NONE_BEFORE = "(none before this point)"
NO_CONTEXT = "(no earlier assistant messages)"
NO_RESULT = "(no result before this point)"
MINUS = "−"

_TOKEN_RE = re.compile(r"gh[pousr]_[A-Za-z0-9]{36}|github_pat_[A-Za-z0-9_]{50,}")
_EXIT_LINE_RE = re.compile(r"Exit code (-?\d+)")
_UUID_RE = re.compile(r"\b[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}\b")
_TIMESTAMP_RE = re.compile(
    r"\b\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:\s?(?:Z|[+-]\d{2}:?\d{2}))?")
_API_ID_RE = re.compile(r"\b(?:msg|toolu)_01[A-Za-z0-9]{20,}")
_SURROGATE_RE = re.compile("[\ud800-\udfff]")


class TokenFound(Exception):
    """The packet text contains a token-shaped string; the build is refused, the string is not echoed."""


@dataclass(frozen=True)
class Packet:
    case_id: str
    text: str
    sha256: str
    n_context: int
    chars: int
    token_hits: int


def case_id_for(unit, seed):
    return hashlib.sha256(f"{seed}|{unit.case_key}".encode("utf-8")).hexdigest()[:10]


def token_hits(text):
    return len(_TOKEN_RE.findall(text))


def exit_code(result_text):
    """The exit code a SHELL tool result reports, else None. Order: a top-level JSON object whose
    `exit_code` is an int; else the FIRST line `Exit code N` (the harness's own line); else None. Never
    searched for inside the body: a result that merely quotes such text (a file being read, `cat log`)
    is not reporting its own exit. run_command's compact `✗ exit N · ...` summary is deliberately NOT
    recognised: it never reaches a transcript result as a first line, and a program's output that
    happens to begin that way would be tagged with a code the tool never returned."""
    try:
        obj = json.loads(result_text)
    except (ValueError, RecursionError):
        obj = None
    if isinstance(obj, dict):
        v = obj.get("exit_code")
        if isinstance(v, int) and not isinstance(v, bool):
            return v
    first = result_text.split("\n", 1)[0].rstrip("\r")
    m = _EXIT_LINE_RE.fullmatch(first)
    return int(m.group(1)) if m else None


def _clean(s):
    """Replace each LONE surrogate with U+FFFD. Claude Code writes the JSON escape `\\ud83d` alone when a
    truncated output splits an emoji; json.loads keeps it as a lone surrogate, which `str.encode("utf-8")`
    refuses (the packet's sha256 step). A valid pair was already joined into one non-BMP char by json.loads
    and is untouched, as is every string without a surrogate. One char for one char, so offsets are unchanged.
    Applied at the earliest point every piece passes: `_blind` (results, args, operator/dispatch text, unit
    and context text, all before any overlap check or cut) and `_call` (tool names)."""
    return _SURROGATE_RE.sub("�", s)


def _blind(s):
    """Replace lone surrogates, then uuid-, timestamp- and API-id-shaped strings quoted inside transcript content."""
    s = _clean(s)
    s = _UUID_RE.sub("<uuid>", s)
    s = _TIMESTAMP_RE.sub("<timestamp>", s)
    return _API_ID_RE.sub("<id>", s)


def _overlaps(source, start, end):
    """True when a token-shaped match in `source` overlaps [start, end) (wholly or partly inside it)."""
    return any(m.start() < end and m.end() > start for m in _TOKEN_RE.finditer(source))


def _tail(source, n):
    """(last n chars of source, whether a token overlaps them)."""
    start = max(0, len(source) - n)
    return source[start:], _overlaps(source, start, len(source))


def _head(source, n):
    """(first n chars of source, whether a token overlaps them)."""
    end = min(len(source), n)
    return source[:end], _overlaps(source, 0, end)


def _call(name, inp):
    """('name(args)', leaked) with the JSON arguments cut to ARGS_CHARS."""
    args, leaked = _head(_blind(json.dumps(inp, ensure_ascii=False, sort_keys=False)), ARGS_CHARS)
    return f"{_clean(name)}({args})", leaked


def _is_shell(name):
    return name.rsplit("__", 1)[-1] in sampler.SHELL_TOOLS


def _result_line(result, shell):
    """('RESULT ...', leaked). The [exit N] prefix is for shell tools only."""
    if result is None:
        return f"RESULT {NO_RESULT}", False
    text, is_error = result
    text = _blind(text)
    prefix = ""
    code = exit_code(text) if shell else None  # from the WHOLE text: the code is often outside the kept tail
    if code is not None:
        prefix += f"[exit {code}] "
    if is_error:
        prefix += "[is_error] "
    tail, leaked = _tail(text, RESULT_TAIL_CHARS)
    return f"RESULT {prefix}{tail}", leaked


def _tool_calls(entries):
    """[(tool_use_id, name, input)] in entry then block order."""
    out = []
    for e in entries:
        for b in sampler._content(e):
            if b.get("type") == "tool_use":
                inp = b.get("input")
                out.append((b.get("id"), b.get("name") or "", inp if isinstance(inp, dict) else {}))
    return out


def _hashable(x):
    try:
        hash(x)
    except TypeError:
        return False
    return True


def _results(entries):
    """{tool_use_id: (text, is_error)} over the given entries (first block for an id wins). A block whose id is
    unhashable (a malformed transcript) is skipped, exactly as _results_index skips it."""
    out = {}
    for e in entries:
        if e.get("type") != "user":
            continue
        for b in sampler._content(e):
            tid = b.get("tool_use_id")
            if b.get("type") == "tool_result" and _hashable(tid) and tid not in out:
                text = transcripts._tool_result_text(b)
                out[tid] = (text or "", b.get("is_error") is True)
    return out


def _context_block(label, ents, results):
    """(block text, leaked) for one context message."""
    lines = [f"### {label}"]
    leaked = False
    text = _blind(sampler._text(ents))
    if text:
        lines.append(text)
    for tid, name, inp in _tool_calls(ents):
        call, l1 = _call(name, inp)
        res, l2 = _result_line(results.get(tid), _is_shell(name))
        lines.append(f"CALL {call}")
        lines.append(res)
        leaked = leaked or l1 or l2
    return "\n".join(lines), leaked


def _calls_block(lines):
    return "ABOUT TO RUN:\n" + "\n".join(lines)


def _join_body(text, lines):
    parts = ([text] if text else []) + ([_calls_block(lines)] if lines else [])
    return "\n\n".join(parts) or "(no text)"


def _fit(text, calls, budget):
    """(body, leaked) for the unit: its text plus the ABOUT TO RUN block, at most `budget` chars, the
    trim markers counted. Order of sacrifice: the head of the text, then the tail of the call list."""
    lines = [c for c, _ in calls]
    body = _join_body(text, lines)
    if len(body) <= budget:
        return body, any(l for _, l in calls)
    block = _calls_block(lines) if lines else ""
    sep = 2 if block else 0
    avail = budget - len(block) - sep
    if avail > len(TRIM_MARKER):  # the calls fit whole beside a trimmed text
        kept, leaked = _tail(text, avail - len(TRIM_MARKER) - 1)
        head = TRIM_MARKER + "\n" + kept
        return head + ("\n\n" + block if block else ""), leaked or any(l for _, l in calls)
    # the calls alone are too big: keep the text (trimmed to half the budget if it needs it) and as
    # many leading calls as fit, then say how many were left out
    half = budget // 2
    leaked = False
    if len(text) <= half:
        head = text
    else:
        kept, leaked = _tail(text, half - len(TRIM_MARKER) - 1)
        head = TRIM_MARKER + "\n" + kept
    room = budget - len(head) - (2 if head else 0)
    for n in range(len(lines) - 1, -1, -1):
        left = len(lines) - n
        marker = f"[… {left} more tool call{'s' if left != 1 else ''} not shown]"
        block = "ABOUT TO RUN:\n" + "\n".join(lines[:n] + [marker])
        if len(block) <= room:
            leaked = leaked or any(l for _, l in calls[:n])
            return (head + "\n\n" if head else "") + block, leaked
    raise ValueError("packet budget too small for even the call-list marker")


def _render(op_title, op_text, blocks, body):
    ctx = "\n\n".join(blocks) if blocks else NO_CONTEXT
    return "\n\n".join([f"## {op_title}\n\n{op_text}", f"## Context\n\n{ctx}", f"## The message\n\n{body}"]) + "\n"


def _results_index(entries):
    """{tool_use_id: (entry_index, (text, is_error))}, the FIRST block for an id winning, over ALL entries.
    Restricted to entries before an index it equals _results(entries[:index]): the first block overall is
    the first in any prefix that contains it, and no prefix that does not contain it has one."""
    out = {}
    for i, e in enumerate(entries):
        if e.get("type") != "user":
            continue
        for b in sampler._content(e):
            tid = b.get("tool_use_id")
            if b.get("type") == "tool_result" and _hashable(tid) and tid not in out:
                text = transcripts._tool_result_text(b)
                out[tid] = (i, (text or "", b.get("is_error") is True))
    return out


class _PrefixResults:
    """Read-only view of a _results_index limited to entries before `idx`; only .get is used downstream."""

    def __init__(self, index, idx):
        self._index, self._idx = index, idx

    def get(self, tid, default=None):
        hit = self._index.get(tid)
        return hit[1] if hit is not None and hit[0] < self._idx else default


def _transcript_state(cache, path, handback):
    """(entries, pos, msgs, results_index) for one transcript. With cache=None every call parses afresh
    (today's behaviour). With a caller-owned dict, the parsed state is kept under the RESOLVED path, so a
    caller iterating units grouped by transcript parses each file once; the caller clears the dict when the
    transcript changes (memory only: a stale entry is never wrong, since the key is the full path)."""
    key = str(pathlib.Path(path).resolve())
    state = cache.get(key) if cache is not None else None
    if state is None:
        entries, _ = transcripts.read_jsonl(path)
        state = {"entries": entries, "pos": {id(e): i for i, e in enumerate(entries)}, "msgs": {},
                 "results": None}
        if cache is not None:
            cache[key] = state
    skip = not handback
    if skip not in state["msgs"]:
        state["msgs"][skip] = sampler._messages(state["entries"], skip_sidechain=skip)
    if state["results"] is None:
        state["results"] = _results_index(state["entries"])
    return state["entries"], state["pos"], state["msgs"][skip], state["results"]


def build_packet(corpus_dir, unit, case_id, cache=None):
    """The packet for `unit`. Re-reads the transcript with transcripts.read_jsonl so that
    unit.first_entry_index indexes the same parsed entries the sampler counted. `cache` is an optional
    caller-owned dict (see _transcript_state); None keeps the original per-call parse and scan."""
    corpus_dir = pathlib.Path(corpus_dir)
    handback = unit.kind == "handback"
    idx = unit.first_entry_index
    if cache is None:
        entries, _ = transcripts.read_jsonl(corpus_dir / unit.transcript)
        msgs = sampler._messages(entries, skip_sidechain=not handback)
        pos = {id(e): i for i, e in enumerate(entries)}
        results = None
    else:
        entries, pos, msgs, index = _transcript_state(cache, corpus_dir / unit.transcript, handback)
        results = _PrefixResults(index, idx)
    found = [m for m in msgs if m[0] == unit.message_id and m[1] == idx]
    if not found:
        raise ValueError(f"unit {unit.case_key!r} is not a message of {unit.transcript!r} at entry {idx}")
    unit_ents = found[0][2]

    before = entries[:idx]  # nothing at or after the decision point is read past here (the unit's own entries excepted)
    if results is None:
        results = _results(before)
    prior = [m for m in msgs if m[1] < idx][-CONTEXT_MESSAGES:]
    prior_ents = [[e for e in m[2] if pos[id(e)] < idx] for m in prior]

    if handback:
        op_title = "Dispatch prompt"
        first = next((transcripts._message_text(e) for e in before
                      if e.get("type") == "user" and transcripts._message_text(e) is not None), None)
    else:
        op_title = "Operator's last message"
        ops = transcripts.operator_messages(before)
        first = transcripts._message_text(ops[-1]) if ops else None
    if first:
        op_text, op_leak = _head(_blind(first), OPERATOR_CHARS)
    else:
        op_text, op_leak = NONE_BEFORE, False

    unit_text = _blind(sampler._text(unit_ents))
    calls = []
    for _, name, inp in _tool_calls(unit_ents):
        call, leaked = _call(name, inp)
        calls.append((f"- {call}", leaked))
    full_body = _join_body(unit_text, [c for c, _ in calls])

    kept = list(prior_ents)
    while True:
        built = [_context_block(f"{MINUS}{len(kept) - i}", ents, results) for i, ents in enumerate(kept)]
        blocks = [b for b, _ in built]
        text = _render(op_title, op_text, blocks, full_body)
        if len(text) <= PACKET_CHARS or not kept:
            break
        kept.pop(0)  # oldest first; the unit itself is never dropped
    leak = op_leak or any(l for _, l in built)
    # what is left of the budget once the operator section and the kept context are in; the unit
    # is whole when it fits, else trimmed to fit (its trim markers counted)
    budget = PACKET_CHARS - len(_render(op_title, op_text, blocks, ""))
    body, body_leak = _fit(unit_text, calls, budget)
    leak = leak or body_leak
    text = _render(op_title, op_text, blocks, body)

    hits = token_hits(text)
    if leak or hits:
        raise TokenFound(f"case {case_id}: token-shaped string(s) in the packet; refusing to build it")
    return Packet(case_id=case_id, text=text, sha256=hashlib.sha256(text.encode("utf-8")).hexdigest(),
                  n_context=len(kept), chars=len(text), token_hits=hits)
