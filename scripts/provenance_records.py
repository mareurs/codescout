"""Native history decoders emit these records; ownership policy stays host-neutral."""
from dataclasses import dataclass, field
import json
from pathlib import Path


@dataclass(frozen=True)
class Operation:
    source_id: str
    call_id: str | None
    principal: str | None
    agent_id: str | None
    timestamp: str | None
    cwd: Path | None
    name: str
    input: dict[str, object]
    outcome: str
    atomic: bool
    is_child: bool
    raw_input: str | None
    # The write targets that are a pure function of this call (`static_targets`), filled by the
    # reader wrapper so the history cache stores them (bug 20927ccfe293c34e, CIW-10). None means
    # "not computed": the scan derives them itself, so an entry written without them still reads
    # right. The doc tool's catalog-id targets are never stored here: they follow a live catalog.
    targets: tuple[str, ...] | None = None


@dataclass(frozen=True)
class CoverageGap:
    source_id: str
    reason: str
    workspace: Path | None
    paths: tuple[str, ...] | None = None


@dataclass
class HistoryBatch:
    operations: list[Operation]
    gaps: list[CoverageGap]


@dataclass
class ProvenanceScan:
    owners: dict[str, list[tuple[str, str | None]]]
    evidence: dict[tuple[str, str, str | None], list[str] | None]
    gaps: list[CoverageGap]
    # path -> principal -> the agent ids (None = the root session itself) that wrote it. `owners`
    # names the SESSION only, because every subagent runs under its parent's session id; this is
    # what lets a reader see that more than one writer lies behind one session's name. Defaulted so
    # every consumer that builds or unpacks a scan without it is unaffected.
    streams: dict[str, dict[str, set[str | None]]] = field(default_factory=dict)


def result_refused(block: dict) -> bool:
    """Native is_error and Codescout's recoverable refusals both mean no atomic write."""
    if block.get("is_error") is True:
        return True
    content = block.get("content")
    if isinstance(content, list):
        text = " ".join(p.get("text", "") for p in content
                        if isinstance(p, dict) and isinstance(p.get("text"), str))
    elif isinstance(content, str):
        text = content
    else:
        return False
    try:
        body = json.loads(text.strip())
    except (ValueError, TypeError):
        return False
    return isinstance(body, dict) and (body.get("ok") is False or "pending_ack" in body)
