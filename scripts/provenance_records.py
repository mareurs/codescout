"""Native history decoders emit these records; ownership policy stays host-neutral."""
from dataclasses import dataclass
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
