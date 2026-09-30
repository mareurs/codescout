"""Labelled-sample packet: the blinded text an operator reads to label ONE assistant message.

Model-free. A packet shows the operator's last message (or, for a hand-back, the dispatch prompt),
the last CONTEXT_MESSAGES assistant messages before the decision point WITH their tool output, and
the message itself with the tool calls it is about to run. Nothing at or after the decision point
except the message's own entries. What is blinded (replaced by a placeholder) is limited to: uuid-shaped
strings (session and message ids), calendar timestamps, and API ids of the form msg_01... / toolu_01.... A
BARE calendar date is kept everywhere (prose, file names): only a date WITH hours, minutes and seconds, ISO `T`
or space separated, is blinded, because _TIMESTAMP_RE requires HH:MM:SS (`2026-09-20 10:11` is kept). A lone
surrogate (a truncated
emoji) becomes U+FFFD so the text can be hashed as UTF-8. The packet is at most PACKET_CHARS characters
in total, always.

A cut is never silent: a result longer than RESULT_HEAD_CHARS + RESULT_TAIL_CHARS keeps its head and its tail
with `[… N characters not shown]` between them, and a call's arguments or the operator/dispatch message's dropped
end `[… N more characters not shown]` after the kept head (_dropped). A judged message with tool calls but no
prose says so (NO_TEXT_NOTE), and its section is titled JUDGED_HEADING so a long context message cannot be
mistaken for it.

Blinded values are NUMBERED per packet (_Ids): each distinct uuid, timestamp and API id becomes `<uuid-N>`,
`<timestamp-N>` or `<id-N>` by first appearance in reading order, so the same value reads the same everywhere in
one packet and two different values never do. The numbers say nothing across packets.

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
# A result longer than HEAD + TAIL keeps its first HEAD and last TAIL characters with a marker between: the counts
# and headers many tools print first, and the exit or error text they print last (bug 5deb65cf11add6cf).
RESULT_HEAD_CHARS = 500
RESULT_TAIL_CHARS = 1000
ARGS_CHARS = 300
# A call that writes a durable record keeps more of its arguments: the body is what the judgement depends on.
RECORD_ARGS_CHARS = 1500
PACKET_CHARS = 20000
TRIM_MARKER = "[… the earlier part of this message is not shown]"
NONE_BEFORE = "(none before this point)"
NO_CONTEXT = "(no earlier assistant messages)"
NO_RESULT = "(no result before this point)"
NO_TEXT_NOTE = "(no text; the message is only the tool call(s) below)"
JUDGED_HEADING = "The message (the one you judge)"
MINUS = "−"

_TOKEN_RE = re.compile(r"gh[pousr]_[A-Za-z0-9]{36}|github_pat_[A-Za-z0-9_]{50,}")
_EXIT_LINE_RE = re.compile(r"Exit code (-?\d+)")
# No pattern starts or ends with `\b`: that anchor does not exist between two word characters, so a value glued
# to a letter, digit or `_` (`run2026-09-30T12:00:00Z`, `sid_<uuid>`, `idmsg_01...`) would pass unmasked. Each
# forbids only what would make the match a fragment of a longer value of its own kind.
_UUID_RE = re.compile(
    r"(?<![0-9a-fA-F])[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}(?![0-9a-fA-F])")
_TIMESTAMP_RE = re.compile(
    r"(?<!\d)\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}(?!\d)(?:\.\d+)?(?:\s?(?:Z|[+-]\d{2}:?\d{2}))?")
_API_ID_RE = re.compile(r"(?:msg|toolu)_01[A-Za-z0-9]{20,}")
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


class _Ids:
    """The numbering for ONE packet (bug 64f2c7af4c2845fb, operator ruling 2026-09-30). Every distinct uuid,
    timestamp and API id becomes `<uuid-N>` / `<timestamp-N>` / `<id-N>`, N counting from 1 per kind in order of
    first appearance, so the labeller can tell one value reused from two different ones without seeing either.
    One instance is created per build_packet and passed to every piece, and the pieces must be blinded in the
    order they are RENDERED (operator or dispatch prompt, context oldest to newest, the judged message): a
    per-call instance, or another order, would number the same value differently in different sections.

    Blinding runs before any cut or cap, so a value that only occurs in dropped material (the cut middle of a
    result, a context message the cap removed) still holds its number and the packet can show `<uuid-2>` with no
    `<uuid-1>`. That is deliberate: numbering only what survives would need the layout before the numbers,
    and the layout depends on the numbers' width."""

    def __init__(self):
        self._seen = {"uuid": {}, "timestamp": {}, "id": {}}

    def _number(self, kind, value):
        seen = self._seen[kind]
        return f"<{kind}-{seen.setdefault(value, len(seen) + 1)}>"

    def blind(self, s):
        """Replace lone surrogates, then uuid-, timestamp- and API-id-shaped strings quoted inside transcript
        content. A uuid is the same value whatever its case; a timestamp or API id is the same value when its
        text is."""
        s = _clean(s)
        s = _UUID_RE.sub(lambda m: self._number("uuid", m.group(0).lower()), s)
        s = _TIMESTAMP_RE.sub(lambda m: self._number("timestamp", m.group(0)), s)
        return _API_ID_RE.sub(lambda m: self._number("id", m.group(0)), s)





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


def _dropped(n, where=""):
    """The marker for a cut that removed `n` characters: `where` is "earlier" (a kept tail), "more" (a kept
    head) or "" (the gap between a kept head and a kept tail). Distinct from TRIM_MARKER, which is about the
    judged message; without one a partial value reads as the whole (bug 2d514ea5f3113cae). It is not source
    text: the token check judges the kept source range."""
    label = f"{where} " if where else ""
    return f"[… {n:,} {label}character{'' if n == 1 else 's'} not shown]"


def _writes_record(name, inp):
    """True for a call whose arguments ARE the durable record it writes (bug 5deb65cf11add6cf): an edit tool, or
    a catalog tool with a write action. The sets are the sampler's own, imported so the two cannot drift."""
    if name in sampler.EDIT_TOOLS:
        return True
    action = inp.get("action")
    return name in sampler.CATALOG_TOOLS and isinstance(action, str) and action in sampler.CATALOG_WRITE_ACTIONS



def _call(name, inp, ids):
    """('name(args)', leaked) with the JSON arguments cut to RECORD_ARGS_CHARS for a call that writes a record
    and to ARGS_CHARS for any other, a cut marked. `ids` is the packet's _Ids."""
    limit = RECORD_ARGS_CHARS if _writes_record(name, inp) else ARGS_CHARS
    raw = ids.blind(json.dumps(inp, ensure_ascii=False, sort_keys=False))
    args, leaked = _head(raw, limit)
    if len(raw) > len(args):
        args += _dropped(len(raw) - len(args), "more")
    return f"{_clean(name)}({args})", leaked


def _is_shell(name):
    return name.rsplit("__", 1)[-1] in sampler.SHELL_TOOLS


def _result_line(result, shell, ids):
    """('RESULT ...', leaked). The [exit N] prefix is for shell tools only. A result of more than
    RESULT_HEAD_CHARS + RESULT_TAIL_CHARS keeps its head and its tail with the gap marked between them.
    `ids` is the packet's _Ids."""
    if result is None:
        return f"RESULT {NO_RESULT}", False
    text, is_error = result
    text = ids.blind(text)
    prefix = ""
    code = exit_code(text) if shell else None  # from the WHOLE text: the code is often in the dropped middle
    if code is not None:
        prefix += f"[exit {code}] "
    if is_error:
        prefix += "[is_error] "
    keep = RESULT_HEAD_CHARS + RESULT_TAIL_CHARS
    if len(text) <= keep:
        shown, leaked = _tail(text, keep)  # the whole text; the flag still says whether a token is in it
    else:
        head, head_leak = _head(text, RESULT_HEAD_CHARS)
        tail, tail_leak = _tail(text, RESULT_TAIL_CHARS)
        shown = head + "\n" + _dropped(len(text) - keep) + "\n" + tail
        leaked = head_leak or tail_leak
    return f"RESULT {prefix}{shown}", leaked


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


def _context_block(label, ents, results, ids):
    """(block text, leaked) for one context message. `ids` is the packet's _Ids."""
    lines = [f"### {label}"]
    leaked = False
    text = ids.blind(sampler._text(ents))
    if text:
        lines.append(text)
    for tid, name, inp in _tool_calls(ents):
        call, l1 = _call(name, inp, ids)
        res, l2 = _result_line(results.get(tid), _is_shell(name), ids)
        lines.append(f"CALL {call}")
        lines.append(res)
        leaked = leaked or l1 or l2
    return "\n".join(lines), leaked


def _calls_block(lines):
    return "ABOUT TO RUN:\n" + "\n".join(lines)


def _join_body(text, lines):
    # a message with calls but no prose says so, else its section is only `ABOUT TO RUN:` and reads as text
    # that failed to render (bug 3cf36991ec89a020)
    head = [text] if text else ([NO_TEXT_NOTE] if lines else [])
    parts = head + ([_calls_block(lines)] if lines else [])
    return "\n\n".join(parts) or "(no text)"


def _fit(text, calls, budget):
    """(body, leaked) for the unit: its text plus the ABOUT TO RUN block, at most `budget` chars, the
    trim markers counted. Order of sacrifice: the head of the text, then the tail of the call list. A
    text-less message's NO_TEXT_NOTE is presentation, not source: it goes before any call does."""
    lines = [c for c, _ in calls]
    body = _join_body(text, lines)
    if len(body) <= budget:
        return body, any(l for _, l in calls)

    block = _calls_block(lines) if lines else ""
    sep = 2 if block else 0
    avail = budget - len(block) - sep
    if text and avail > len(TRIM_MARKER):  # the calls fit whole beside a trimmed text
        kept, leaked = _tail(text, avail - len(TRIM_MARKER) - 1)
        head = TRIM_MARKER + "\n" + kept
        return head + ("\n\n" + block if block else ""), leaked or any(l for _, l in calls)
    if not text and block and len(block) <= budget:  # only the note did not fit: drop it, keep every call
        return block, any(l for _, l in calls)
    # the calls alone are too big: keep the text (trimmed to half the budget if it needs it) and as
    # many leading calls as fit, then say how many were left out
    half = budget // 2
    leaked = False
    if len(text) <= half:
        head = text or (NO_TEXT_NOTE if lines else "")
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
    return "\n\n".join([f"## {op_title}\n\n{op_text}", f"## Context\n\n{ctx}",
                        f"## {JUDGED_HEADING}\n\n{body}"]) + "\n"


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
    ids = _Ids()  # ONE per packet, and the pieces below are blinded in the order they are RENDERED
    if first:
        blinded = ids.blind(first)
        op_text, op_leak = _head(blinded, OPERATOR_CHARS)
        if len(blinded) > len(op_text):
            op_text += "\n" + _dropped(len(blinded) - len(op_text), "more")
    else:
        op_text, op_leak = NONE_BEFORE, False

    kept = list(prior_ents)
    # the context is blinded BEFORE the judged message: its values are numbered first, as a reader meets them
    built = [_context_block(f"{MINUS}{len(kept) - i}", ents, results, ids) for i, ents in enumerate(kept)]
    unit_text = ids.blind(sampler._text(unit_ents))
    calls = []
    for _, name, inp in _tool_calls(unit_ents):
        call, leaked = _call(name, inp, ids)
        calls.append((f"- {call}", leaked))
    full_body = _join_body(unit_text, [c for c, _ in calls])

    while True:
        blocks = [b for b, _ in built]
        text = _render(op_title, op_text, blocks, full_body)
        if len(text) <= PACKET_CHARS or not kept:
            break
        kept.pop(0)  # oldest first; the unit itself is never dropped
        # relabelled from the mapping the first pass completed: a rebuild changes no number
        built = [_context_block(f"{MINUS}{len(kept) - i}", ents, results, ids) for i, ents in enumerate(kept)]
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
