"""Verify native journal prefixes. Integrity alone never grants write ownership."""
from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path
import re
import stat

SCHEMA = 1
MAX_STREAM_BYTES = 256 * 1024 * 1024
MAX_EVENT_BYTES = 16 * 1024 * 1024
MAX_RECORD_BYTES = 2 * MAX_EVENT_BYTES + 1024
MAX_EVENTS = 1_000_000
_ID = re.compile(r"[A-Za-z0-9_-]{1,256}\Z")
_HASH = re.compile(r"[a-f0-9]{64}\Z")


class ProofError(ValueError):
    """Missing, unsupported or unverifiable native evidence; preserve UNKNOWN."""


@dataclass(frozen=True)
class VerifiedPrefix:
    identity: dict
    call_id: str
    events: tuple
    sha256: str


def _require(condition, message):
    if not condition:
        raise ProofError(message)


def _object(pairs):
    value = {}
    for key, item in pairs:
        _require(key not in value, "duplicate JSON key in native evidence")
        value[key] = item
    return value


def _json(raw):
    def constant(_):
        raise ProofError("nonfinite JSON value in native evidence")
    try:
        return json.loads(raw, object_pairs_hook=_object, parse_constant=constant)
    except (ValueError, UnicodeError, RecursionError) as error:
        raise ProofError("malformed native evidence") from error


def _regular(path):
    try:
        fd = os.open(path, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_NONBLOCK", 0))
        try:
            _require(stat.S_ISREG(os.fstat(fd).st_mode), "native evidence is not a regular file")
            return os.fdopen(fd, "rb")
        except BaseException:
            os.close(fd)
            raise
    except OSError as error:
        raise ProofError("native evidence cannot be opened safely") from error


def _history(identity, history_path):
    try:
        history = Path(history_path)
        _require(history.is_absolute() and history.resolve(strict=True) == history,
                 "native history path is not canonical")
        _require(identity["history"] == str(history), "native history binding differs")
        length = identity["history_bytes"]
        _require(type(length) is int and 0 < length <= MAX_STREAM_BYTES,
                 "unsupported native history prefix length")
        _require(isinstance(identity["history_sha256"], str)
                 and _HASH.fullmatch(identity["history_sha256"]), "invalid native history digest")
        with _regular(history) as stream:
            prefix = stream.read(length)
        _require(len(prefix) == length, "native history prefix is missing")
        _require(hashlib.sha256(prefix).hexdigest() == identity["history_sha256"],
                 "native history prefix changed")
        _require(prefix.endswith(b"\n"), "native history prefix ends inside a record")
        first = _json(prefix.split(b"\n", 1)[0])
        _require(isinstance(first, dict) and first.get("type") == "session_meta",
                 "native root metadata is missing")
        meta = first.get("payload")
        _require(isinstance(meta, dict) and meta.get("session_id") == identity["root"]
                 and meta.get("id") == identity["thread"], "native root/thread binding differs")
        return history
    except (KeyError, TypeError, OSError, RuntimeError) as error:
        raise ProofError("unsupported native history binding") from error


def _transitions(events, invocation_id):
    calls = {}
    informational = {"cell_start", "cell_callback", "cell_closed", "cell_retired", "lineage", "incomplete"}
    allowed = {"received", "dispatch", "outcome", "cancelled_before_dispatch",
               "cancelled_after_dispatch", "checkpoint", "mcp_dispatch", "mcp_outcome"} | informational
    for event in events[1:]:
        kind, data = event["kind"], event["data"]
        _require(kind in allowed and isinstance(data, dict), "unsupported native event")
        if kind in informational:
            continue
        call_id = data.get("call_id")
        _require(isinstance(call_id, str) and call_id, "native call identity missing")
        state = calls.get(call_id)
        if kind == "received":
            _require(state is None, "native call identity was reused")
            calls[call_id] = "received"
        elif kind in {"dispatch", "cancelled_before_dispatch"}:
            _require(state == "received", "invalid native dispatch transition")
            calls[call_id] = "dispatched" if kind == "dispatch" else "terminal"
        elif kind in {"outcome", "cancelled_after_dispatch"}:
            _require(state == "dispatched", "invalid native outcome transition")
            if kind == "outcome":
                _require(data.get("outcome") in {"completed", "failed", "refused"},
                         "native outcome is unsupported")
                calls[call_id] = "terminal"
        else:
            _require(state == "dispatched", "checkpoint/MCP invocation was not dispatched")
    _require(calls.get(invocation_id) == "dispatched", "checkpoint invocation is not current")


def verify_checkpoint(checkpoint, native_identity, history_path, invocation_id):
    """Return an immutable prefix only for a fully bound, real transport checkpoint.

    Callers must obtain checkpoint/identity/invocation from native MCP metadata,
    never from tool-result text or arbitrary ambient environment. This function
    proves the journal prefix; lifecycle completeness and fencing are separate
    required checks before any opaque-history gap can be discharged.
    """
    try:
        _require(isinstance(checkpoint, dict) and set(checkpoint) ==
                 {"identity", "path", "seq", "sha256", "call_id"}, "checkpoint fields missing or unsupported")
        identity = checkpoint["identity"]
        keys = {"schema", "root", "thread", "epoch", "history", "history_bytes", "history_sha256"}
        _require(isinstance(identity, dict) and set(identity) == keys, "unsupported native identity")
        _require(type(identity["schema"]) is int and identity["schema"] == SCHEMA,
                 "unsupported native journal schema")
        for key in ("root", "thread", "epoch"):
            _require(isinstance(identity[key], str) and _ID.fullmatch(identity[key]),
                     "invalid native identity")
        _require(isinstance(native_identity, dict) and
                 all(native_identity.get(key) == identity[key] for key in ("root", "thread")),
                 "native invocation root/thread differs")
        _require(isinstance(invocation_id, str) and invocation_id
                 and checkpoint["call_id"] == invocation_id, "native invocation ID differs")
        seq = checkpoint["seq"]
        _require(type(seq) is int and 0 < seq < MAX_EVENTS, "unsupported checkpoint ordinal")
        _require(isinstance(checkpoint["sha256"], str) and _HASH.fullmatch(checkpoint["sha256"]),
                 "invalid checkpoint digest")
        history = _history(identity, history_path)
        expected = Path(str(history) + ".provenance-" + identity["epoch"] + ".journal")
        path = Path(checkpoint["path"])
        _require(path == expected and path.resolve(strict=True) == expected,
                 "native journal path differs or is linked")
        events, previous, total = [], "0" * 64, 0
        with _regular(path) as stream:
            for ordinal in range(seq + 1):
                line = stream.readline(MAX_RECORD_BYTES + 1)
                total += len(line)
                _require(line.endswith(b"\n") and len(line) <= MAX_RECORD_BYTES
                         and total <= MAX_STREAM_BYTES, "native journal prefix is truncated or oversized")
                record = _json(line)
                _require(isinstance(record, dict) and set(record) == {"body", "sha256"},
                         "unsupported native journal record")
                body = record["body"]
                _require(isinstance(body, str), "native event body is missing")
                raw = body.encode("utf-8")
                _require(len(raw) <= MAX_EVENT_BYTES, "native event body exceeds limit")
                digest = hashlib.sha256(raw).hexdigest()
                _require(record["sha256"] == digest, "native event digest differs")
                event = _json(body)
                _require(isinstance(event, dict) and set(event) == {"seq", "previous", "kind", "data"},
                         "unsupported native event fields")
                _require(type(event["seq"]) is int and event["seq"] == ordinal
                         and event["previous"] == previous, "native event sequence or linkage differs")
                if ordinal == 0:
                    _require(event["kind"] == "header" and event["data"] == identity,
                             "native journal header differs")
                events.append(event)
                previous = digest
        _require(previous == checkpoint["sha256"], "checkpoint frontier differs")
        _require(events[-1]["kind"] == "checkpoint"
                 and events[-1]["data"] == {"call_id":invocation_id}, "frontier is not the bound checkpoint")
        _transitions(events, invocation_id)
        return VerifiedPrefix(identity.copy(), invocation_id, tuple(events), previous)
    except ProofError:
        raise
    except (OSError, KeyError, TypeError, ValueError, UnicodeError, OverflowError, RecursionError) as error:
        raise ProofError("unsupported or unreadable native checkpoint") from error
