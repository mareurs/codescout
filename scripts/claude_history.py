"""Decode Claude records and discover every configured profile conservatively."""
from datetime import datetime
import json
from pathlib import Path
import re
import subprocess
from typing import Mapping

from provenance_records import CoverageGap, HistoryBatch, Operation, result_refused
from session_context import principal_key

_NON_ATOMIC = {"Bash", "mcp__codescout__run_command"}


def _slug(root: Path) -> str:
    return re.sub(r"[^A-Za-z0-9]", "-", str(root))


def _main_checkout(root: Path) -> Path | None:
    out = subprocess.run(["git", "-C", str(root), "rev-parse", "--git-common-dir"],
                         capture_output=True, text=True)
    if out.returncode:
        return None
    common = Path(out.stdout.strip())
    if not common.is_absolute():
        common = root / common
    common = common.resolve()
    return common.parent if common.name == ".git" and common.parent != root.resolve() else None


def _entries(path: Path, root: Path, gaps: list[CoverageGap]) -> list[Path]:
    try:
        return sorted(path.iterdir())
    except FileNotFoundError:
        return []
    except OSError:
        gaps.append(CoverageGap(str(path), "unreadable Claude history directory", root))
        return []


def discover_claude_histories(root: Path, environ: Mapping[str, str]) -> tuple[list[Path], list[CoverageGap]]:
    gaps = []
    override = environ.get("FILE_PROVENANCE_ROOTS")
    if override:
        directories = [Path(p) for p in re.split(r"[:,]", override) if p]
    else:
        home = Path(environ.get("HOME", str(Path.home())))
        profiles = [p for p in _entries(home, root, gaps)
                    if p.name == ".claude" or p.name.startswith(".claude-")]
        custom = environ.get("CLAUDE_CONFIG_DIR")
        if custom:
            profiles.append(Path(custom))
        checkouts = [root.resolve()]
        main = _main_checkout(root)
        if main is not None:
            checkouts.append(main)
        directories = []
        for profile in dict.fromkeys(profiles):
            # Probe the profile explicitly: glob/is_dir may silently swallow unreadability.
            _entries(profile, root, gaps)
            directories.extend(profile / "projects" / _slug(checkout) for checkout in checkouts)
    paths, seen = [], set()
    for directory in directories:
        for entry in _entries(directory, root, gaps):
            candidates = [entry] if entry.suffix == ".jsonl" else [
                p for p in _entries(entry / "subagents", root, gaps) if p.suffix == ".jsonl"
            ]
            for path in candidates:
                try:
                    original = path.resolve(strict=True)
                except OSError:
                    gaps.append(CoverageGap(str(path), "unreadable Claude history source", root))
                    continue
                if original not in seen:
                    seen.add(original)
                    paths.append(original)
    return paths, gaps


def _session(path: Path, rec: dict) -> str:
    fallback = path.parent.parent.name if path.parent.name == "subagents" else path.stem
    value = rec.get("sessionId") or rec.get("session_id")
    return value if isinstance(value, str) and value.strip() else fallback


def _timestamp(value: object) -> datetime | None:
    if not isinstance(value, str):
        return None
    try:
        result = datetime.fromisoformat(value)
    except ValueError:
        return None
    return result if result.tzinfo is not None else None



def read_claude_history(path: Path) -> HistoryBatch:
    operations, gaps, calls, results = [], [], [], {}
    is_child = path.parent.name == "subagents"
    agent = path.stem.removeprefix("agent-") if is_child else None
    enqueues = {}
    try:
        with path.open(encoding="utf-8") as stream:
            for line_number, line in enumerate(stream, 1):
                locator = f"{path.resolve()}:{line_number}"
                try:
                    rec = json.loads(line)
                except (ValueError, TypeError):
                    gaps.append(CoverageGap(locator, "malformed Claude history record", None))
                    continue
                if not isinstance(rec, dict):
                    gaps.append(CoverageGap(locator, "unsupported Claude record", None))
                    continue
                if (not is_child and rec.get("type") == "queue-operation"
                        and rec.get("operation") == "enqueue"):
                    principal = principal_key("claude-code", _session(path, rec))
                    queued = _timestamp(rec.get("timestamp"))
                    if principal not in enqueues:
                        enqueues[principal] = queued
                    elif enqueues[principal] is not None:
                        if queued is None or queued < enqueues[principal]:
                            enqueues[principal] = queued
                content = (rec.get("message") or {}).get("content") if isinstance(rec.get("message"), dict) else None
                if not isinstance(content, list):
                    continue
                cwd = Path(rec["cwd"]) if isinstance(rec.get("cwd"), str) and Path(rec["cwd"]).is_absolute() else None
                for block_index, block in enumerate(content):
                    if not isinstance(block, dict):
                        continue
                    if block.get("type") == "tool_result" and isinstance(block.get("tool_use_id"), str):
                        results.setdefault(block["tool_use_id"], []).append(block)
                    if block.get("type") != "tool_use":
                        continue
                    name, inp = block.get("name"), block.get("input")
                    if not isinstance(name, str) or not isinstance(inp, dict):
                        gaps.append(CoverageGap(locator, "unsupported Claude tool arguments", cwd))
                        continue
                    sid = _session(path, rec)
                    call = block.get("id") if isinstance(block.get("id"), str) else None
                    # Event IDs deduplicate within one native root. A fork can rewrite
                    # sessionId on copied calls; the root enqueue boundary is checked below.
                    event = call or rec.get("uuid")
                    producer = agent or "root"
                    source = f"claude-code:{sid}/{producer}/{event}" if isinstance(event, str) else f"{locator}:{block_index}"
                    timestamp = rec.get("timestamp") if isinstance(rec.get("timestamp"), str) else None
                    calls.append((source, call, principal_key("claude-code", sid), agent, timestamp, cwd, name, inp))
    except (OSError, UnicodeError):
        gaps.append(CoverageGap(str(path), "unreadable Claude history source", None))
    seen = {}
    for source, call, principal, agent_id, timestamp, cwd, name, inp in calls:
        if principal in enqueues:
            queued = enqueues[principal]
            called = _timestamp(timestamp)
            if queued is None or called is None or called < queued:
                # Native 2.1.296 forks copy calls with their old timestamps but a new
                # sessionId. This is an uncertainty boundary, not proof of an original
                # owner. Retain a gap even if no original transcript is discoverable.
                pin = inp.get("workspace") if name.startswith("mcp__codescout__") else None
                workspace = Path(pin) if isinstance(pin, str) and Path(pin).is_absolute() else None
                gaps.append(CoverageGap(
                    source, "Claude inherited history has an unverified native root enqueue boundary", workspace))
                continue
        outputs = results.get(call, []) if call else []
        atomic = name not in _NON_ATOMIC
        refused = any(result_refused(output) for output in outputs)
        outcome = ("no-write-refused" if atomic else "partial-error") if refused else (
            "possible-write" if outputs else "missing-result")
        if outputs and any(result_refused(output) != result_refused(outputs[0]) for output in outputs[1:]):
            gaps.append(CoverageGap(source, "conflicting Claude result replay", cwd))
            outcome = "missing-result"
        op = Operation(source, call, principal, agent_id, timestamp, cwd, name, inp,
                       outcome, atomic, is_child, None)
        previous = seen.get(source)
        if previous is None:
            operations.append(op)
            seen[source] = op
        elif previous != op:
            gaps.append(CoverageGap(source, "conflicting Claude call replay", cwd))
    return HistoryBatch(operations, gaps)
