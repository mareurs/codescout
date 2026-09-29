"""Labelled-sample packet: the blinded text an operator reads to label ONE assistant message.

Model-free. A packet shows the operator's last message (or, for a hand-back, the dispatch prompt),
the last CONTEXT_MESSAGES assistant messages before the decision point WITH their tool output, and
the message itself with the tool calls it is about to run. Nothing at or after the decision point
except the message's own entries, and no identifier or timestamp, ever appears in it. A token-shaped
string in the packet refuses the build (TokenFound) rather than being shown.
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
_EXIT_JSON_RE = re.compile(r'"exit_code"\s*:\s*(-?\d+)')
_EXIT_LINE_RE = re.compile(r"^Exit code (-?\d+)$", re.MULTILINE)
_UUID_RE = re.compile(r"\b[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}\b")
_TIMESTAMP_RE = re.compile(r"\b\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:?\d{2})?")
_API_ID_RE = re.compile(r"\b(?:msg|toolu)_[A-Za-z0-9]+")


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
    """The exit code a tool result reports anywhere in its text: the JSON `"exit_code": N` form, else
    a line `Exit code N`; None otherwise."""
    m = _EXIT_JSON_RE.search(result_text) or _EXIT_LINE_RE.search(result_text)
    return int(m.group(1)) if m else None


def _blind(s):
    """Replace uuid-, timestamp- and API-id-shaped strings quoted inside transcript content."""
    s = _UUID_RE.sub("<uuid>", s)
    s = _TIMESTAMP_RE.sub("<timestamp>", s)
    return _API_ID_RE.sub("<id>", s)


def _call(name, inp):
    args = _blind(json.dumps(inp, ensure_ascii=False, sort_keys=False))[:ARGS_CHARS]
    return f"{name}({args})"


def _result_line(result):
    if result is None:
        return f"RESULT {NO_RESULT}"
    text, is_error = result
    text = _blind(text)
    prefix = ""
    code = exit_code(text)  # the WHOLE text: the code is often outside the kept tail
    if code is not None:
        prefix += f"[exit {code}] "
    if is_error:
        prefix += "[is_error] "
    return f"RESULT {prefix}{text[-RESULT_TAIL_CHARS:]}"


def _tool_calls(entries):
    """[(tool_use_id, name, input)] in entry then block order."""
    out = []
    for e in entries:
        for b in sampler._content(e):
            if b.get("type") == "tool_use":
                inp = b.get("input")
                out.append((b.get("id"), b.get("name") or "", inp if isinstance(inp, dict) else {}))
    return out


def _results(entries):
    """{tool_use_id: (text, is_error)} over the given entries (first block for an id wins)."""
    out = {}
    for e in entries:
        if e.get("type") != "user":
            continue
        for b in sampler._content(e):
            if b.get("type") == "tool_result" and b.get("tool_use_id") not in out:
                text = transcripts._tool_result_text(b)
                out[b.get("tool_use_id")] = (text or "", b.get("is_error") is True)
    return out


def _context_block(label, ents, results):
    lines = [f"### {label}"]
    text = _blind(sampler._text(ents))
    if text:
        lines.append(text)
    for tid, name, inp in _tool_calls(ents):
        lines.append(f"CALL {_call(name, inp)}")
        lines.append(_result_line(results.get(tid)))
    return "\n".join(lines)


def _unit_body(ents):
    text = _blind(sampler._text(ents))
    calls = _tool_calls(ents)
    parts = [text] if text else []
    if calls:
        parts.append("ABOUT TO RUN:\n" + "\n".join(f"- {_call(n, i)}" for _, n, i in calls))
    body = "\n\n".join(parts) or "(no text)"
    if len(body) > PACKET_CHARS:
        body = TRIM_MARKER + "\n" + body[-PACKET_CHARS:]
    return body


def _render(op_title, op_text, blocks, body):
    ctx = "\n\n".join(blocks) if blocks else NO_CONTEXT
    return "\n\n".join([f"## {op_title}\n\n{op_text}", f"## Context\n\n{ctx}", f"## The message\n\n{body}"]) + "\n"


def build_packet(corpus_dir, unit, case_id):
    """The packet for `unit`. Re-reads the transcript with transcripts.read_jsonl so that
    unit.first_entry_index indexes the same parsed entries the sampler counted."""
    corpus_dir = pathlib.Path(corpus_dir)
    entries, _ = transcripts.read_jsonl(corpus_dir / unit.transcript)
    handback = unit.kind == "handback"
    idx = unit.first_entry_index
    msgs = sampler._messages(entries, skip_sidechain=not handback)
    found = [m for m in msgs if m[0] == unit.message_id and m[1] == idx]
    if not found:
        raise ValueError(f"unit {unit.case_key!r} is not a message of {unit.transcript!r} at entry {idx}")
    unit_ents = found[0][2]

    before = entries[:idx]  # nothing at or after the decision point is read past here (the unit's own entries excepted)
    results = _results(before)
    pos = {id(e): i for i, e in enumerate(entries)}
    prior = [m for m in msgs if m[1] < idx][-CONTEXT_MESSAGES:]
    prior_ents = [[e for e in m[2] if pos[id(e)] < idx] for m in prior]

    if handback:
        op_title = "Dispatch prompt"
        first = next((transcripts._message_text(e) for e in entries
                      if e.get("type") == "user" and transcripts._message_text(e) is not None), None)
        op_text = _blind(first)[:OPERATOR_CHARS] if first else NONE_BEFORE
    else:
        op_title = "Operator's last message"
        ops = transcripts.operator_messages(before)
        last = transcripts._message_text(ops[-1]) if ops else None
        op_text = _blind(last)[:OPERATOR_CHARS] if last else NONE_BEFORE

    body = _unit_body(unit_ents)
    kept = list(prior_ents)
    while True:
        blocks = [_context_block(f"{MINUS}{len(kept) - i}", ents, results) for i, ents in enumerate(kept)]
        text = _render(op_title, op_text, blocks, body)
        if len(text) <= PACKET_CHARS or not kept:
            break
        kept.pop(0)  # oldest first; the unit's own body is never dropped

    hits = token_hits(text)
    if hits:
        raise TokenFound(f"case {case_id}: {hits} token-shaped string(s) in the packet; refusing to build it")
    return Packet(case_id=case_id, text=text, sha256=hashlib.sha256(text.encode("utf-8")).hexdigest(),
                  n_context=len(kept), chars=len(text), token_hits=hits)
