"""Decode verified Codex rollout envelopes; never execute recorded orchestration."""
from decimal import Decimal
import json
import os
from pathlib import Path
import re
from typing import Mapping

from provenance_records import CoverageGap, HistoryBatch, Operation, result_refused
from session_context import principal_key


def _entries(path: Path, root: Path, gaps: list[CoverageGap]) -> list[Path]:
    try:
        return sorted(path.iterdir())
    except FileNotFoundError:
        return []
    except OSError:
        gaps.append(CoverageGap(str(path), "unreadable Codex profile", root))
        return []


def discover_codex_histories(root: Path, environ: Mapping[str, str]) -> tuple[list[Path], list[CoverageGap]]:
    gaps = []
    override = environ.get("FILE_PROVENANCE_CODEX_ROOTS_JSON")
    if override is not None:
        try:
            values = json.loads(override)
            if not isinstance(values, list) or any(not isinstance(p, str) or not Path(p).is_absolute() for p in values):
                raise ValueError("expected absolute history paths")
            directories = [Path(p) for p in values]
        except (ValueError, TypeError):
            return [], [CoverageGap("FILE_PROVENANCE_CODEX_ROOTS_JSON", "malformed Codex history-root override", root)]
    else:
        home = Path(environ.get("HOME", str(Path.home())))
        profiles = [p for p in _entries(home, root, gaps)
                    if p.name.lower() == ".codex" or p.name.lower().startswith(".codex-")]
        if environ.get("CODEX_HOME"):
            profile = Path(environ["CODEX_HOME"])
            if not profile.is_absolute():
                return [], [CoverageGap("CODEX_HOME", "CODEX_HOME must be absolute", root)]
            profiles.append(profile)
        directories = []
        for profile in dict.fromkeys(profiles):
            _entries(profile, root, gaps)
            directories.extend([profile / "sessions", profile / "archived_sessions"])
    paths, seen = [], set()
    for directory in directories:
        # Walk explicitly with onerror: rglob can turn unreadability into an empty match.
        def onerror(error):
            if not isinstance(error, FileNotFoundError):
                gaps.append(CoverageGap(str(error.filename), "unreadable Codex history directory", root))
        if directory.suffix == ".jsonl":
            candidates = [directory]
        else:
            candidates = [Path(parent) / name for parent, _, names in os.walk(directory, onerror=onerror)
                          for name in sorted(names) if name.endswith(".jsonl")]
        for path in candidates:
            try:
                original = path.resolve(strict=True)
            except OSError:
                gaps.append(CoverageGap(str(path), "unreadable Codex history source", root))
                continue
            if original not in seen:
                seen.add(original)
                paths.append(original)
    # Every source is decoded independently; policy filters actual cwd/workspace/targets,
    # including absolute writes into this checkout from a session created elsewhere.
    return sorted(paths), gaps


def _output_refused(output: object) -> bool:
    if result_refused({"content": output}):
        return True
    if isinstance(output, str):
        try:
            body = json.loads(output)
        except ValueError:
            return False
        if isinstance(body, dict):
            return body.get("isError") is True or result_refused(body)
    return False


def _output_failed(output: object) -> bool:
    if _output_refused(output):
        return True
    if isinstance(output, str):
        try:
            value = json.loads(output)
        except ValueError:
            value = None
        if isinstance(value, dict) and type(value.get("exit_code")) is int:
            return value["exit_code"] != 0
        match = re.search(r"(?:Process exited with code|Exit code:)\s*(-?\d+)", output)
        return bool(match and int(match.group(1)) != 0)
    return False


# Known current and legacy Codescout operations. Future/foreign MCP tools may write,
# so accepting their envelope without a supported interpretation is a coverage gap.
KNOWN_CODE_SCOUT_NAMES = frozenset("""
    approve_write call_graph create_file doc edit_code edit_file get_guide grep index
    librarian library memory onboarding read_file references run_command semantic_search
    symbol_at symbols tree workspace
    edit_markdown replace_symbol insert_code remove_symbol artifact artifact_augment
""".split())

def _decode_call(payload: dict, cwd: Path | None):
    name = payload.get("name")
    namespace = payload.get("namespace")
    if namespace not in (None, "functions"):
        return None, "unsupported Codex tool namespace"
    if payload.get("type") == "custom_tool_call":
        raw = payload.get("input")
        if name == "apply_patch" and isinstance(raw, str):
            return ("apply_patch", {"patch": raw}, cwd, False, raw), ""
        return None, "opaque Codex custom-tool wrapper; inner execution is not recoverable"
    raw = payload.get("arguments")
    if not isinstance(raw, str):
        return None, "unsupported Codex function arguments"
    try:
        inp = json.loads(raw, parse_float=Decimal)
    except ValueError:
        return None, "malformed Codex function arguments"
    if not isinstance(inp, dict):
        return None, "unsupported Codex function arguments"
    if name == "exec_command":
        command = inp.get("cmd")
        workdir = inp.get("workdir")
        if not isinstance(command, str) or (workdir is not None and not isinstance(workdir, str)):
            return None, "unsupported Codex shell arguments"
        if workdir is not None:
            if not Path(workdir).is_absolute():
                if cwd is None:
                    return None, "unresolved Codex shell workdir"
                cwd = cwd / workdir
            else:
                cwd = Path(workdir)
        return ("Bash", {**inp, "command": command}, cwd, False, raw), ""
    if isinstance(name, str) and re.fullmatch(r"mcp__[^\s]+__[^\s]+", name):
        if not name.startswith("mcp__codescout__") or name.removeprefix("mcp__codescout__") not in KNOWN_CODE_SCOUT_NAMES:
            return None, "unsupported Codex MCP tool; write effects are not recoverable"
        atomic = name != "mcp__codescout__run_command"
        return (name, inp, cwd, atomic, raw), ""
    # Polling a running shell is read-only only with empty stdin. A nonempty write
    # may drive a process that already owns output handles and unknown target paths.
    if name == "write_stdin" and not inp.get("chars"):
        return None, ""
    return None, "unsupported Codex tool or orchestration wrapper"


def read_codex_history(path: Path) -> HistoryBatch:
    rows, gaps, operations = [], [], []
    try:
        with path.open(encoding="utf-8") as stream:
            for number, line in enumerate(stream, 1):
                try:
                    value = json.loads(line, parse_float=Decimal)
                    if not isinstance(value, dict):
                        raise ValueError("record must be an object")
                    rows.append((number, value))
                except ValueError:
                    gaps.append(CoverageGap(f"{path}:{number}", "malformed Codex history record", None))
    except (OSError, UnicodeError):
        return HistoryBatch([], [CoverageGap(str(path), "unreadable Codex history source", None)])
    metas = [row.get("payload") for _, row in rows if row.get("type") == "session_meta"]
    if not metas or not isinstance(metas[0], dict):
        return HistoryBatch([], gaps + [CoverageGap(str(path), "missing Codex root metadata", None)])
    meta = metas[0]
    sid, thread, native_cwd = meta.get("session_id"), meta.get("id"), meta.get("cwd")
    if (not isinstance(sid, str) or not sid.strip() or not isinstance(thread, str) or not thread.strip()
            or not isinstance(native_cwd, str) or not Path(native_cwd).is_absolute()):
        return HistoryBatch([], gaps + [CoverageGap(str(path), "unsupported Codex root identity or cwd", None)])
    cwd = Path(native_cwd)
    # Attach unattributed syntax gaps to the native workspace once it is known.
    gaps = [CoverageGap(g.source_id, g.reason, cwd, g.paths) for g in gaps]
    if meta.get("history_base") is not None:
        gaps.append(CoverageGap(str(path), "external Codex history prefix is not independently verified", cwd))
    source = meta.get("source")
    child_source = source.get("subagent") if isinstance(source, dict) else None
    spawn = child_source.get("thread_spawn") if isinstance(child_source, dict) else None
    is_child = isinstance(spawn, dict)
    if thread != sid and not is_child:
        return HistoryBatch([], gaps + [CoverageGap(str(path), "unverified Codex child/root relationship", cwd)])
    if is_child and (thread == sid or not isinstance(spawn.get("parent_thread_id"), str)
                     or meta.get("parent_thread_id") not in (None, spawn.get("parent_thread_id"))):
        return HistoryBatch([], gaps + [CoverageGap(str(path), "conflicting Codex child metadata", cwd)])
    boundary = meta.get("subagent_history_start_ordinal") if is_child else None
    paginated = meta.get("history_mode", "legacy") == "paginated"
    if meta.get("history_mode", "legacy") not in ("legacy", "paginated"):
        return HistoryBatch([], gaps + [CoverageGap(str(path), "unsupported Codex history_mode", cwd)])
    if is_child and (type(boundary) is not int or boundary < 0 or not paginated):
        return HistoryBatch([], gaps + [CoverageGap(str(path), "missing verified Codex child history boundary", cwd)])
    ordinals = [row.get("ordinal") for _, row in rows]
    if paginated and any(type(value) is not int or value < 0 for value in ordinals):
        gaps.append(CoverageGap(str(path), "missing or unsupported required Codex ordinal", cwd))
        if is_child:
            return HistoryBatch([], gaps)
    if is_child and max((n for n in ordinals if type(n) is int), default=-1) < boundary - 1:
        return HistoryBatch([], gaps + [CoverageGap(str(path), "incomplete Codex inherited prefix", cwd)])
    if meta.get("forked_from_id") is not None:
        fork_boundary = meta.get("forked_from_ordinal_exclusive")
        if (not isinstance(meta["forked_from_id"], str) or not meta["forked_from_id"]
                or type(fork_boundary) is not int or fork_boundary < 0 or not paginated
                or any(type(value) is not int or value < 0 for value in ordinals)
                or max(ordinals, default=-1) < fork_boundary - 1):
            return HistoryBatch([], gaps + [CoverageGap(str(path), "unverified Codex fork replay boundary", cwd)])
        boundary = fork_boundary
    principal = principal_key("codex", sid)
    agent = thread if is_child else None
    calls, results = [], {}
    for number, row in rows:
        locator = f"{path}:{number}"
        ordinal = row.get("ordinal")
        if boundary is not None and (type(ordinal) is not int or ordinal < boundary):
            continue
        kind, payload = row.get("type"), row.get("payload")
        if kind == "session_meta":
            if payload != meta:
                gaps.append(CoverageGap(locator, "conflicting Codex session metadata replay", cwd))
            continue
        if kind == "turn_context":
            if isinstance(payload, dict) and isinstance(payload.get("cwd"), str) and Path(payload["cwd"]).is_absolute():
                cwd = Path(payload["cwd"])
            else:
                gaps.append(CoverageGap(locator, "unsupported Codex turn cwd", cwd))
            continue
        if kind in ("event_msg", "compacted", "world_state", "world_state_item"):
            continue
        if kind != "response_item" or not isinstance(payload, dict):
            gaps.append(CoverageGap(locator, "unsupported Codex rollout envelope", cwd))
            continue
        item_type = payload.get("type")
        if item_type in ("function_call_output", "custom_tool_call_output"):
            call = payload.get("call_id")
            if isinstance(call, str):
                results.setdefault(call, []).append(payload.get("output"))
            else:
                gaps.append(CoverageGap(locator, "missing Codex result call_id", cwd))
        elif item_type in ("function_call", "custom_tool_call"):
            call = payload.get("call_id")
            if not isinstance(call, str) or not call:
                gaps.append(CoverageGap(locator, "missing Codex call_id", cwd))
                continue
            decoded, reason = _decode_call(payload, cwd)
            if reason:
                gaps.append(CoverageGap(locator, reason, cwd))
            if decoded is not None:
                timestamp = row.get("timestamp") if isinstance(row.get("timestamp"), str) else None
                calls.append((call, timestamp, decoded))
        elif item_type not in ("message", "reasoning", "agent_message", "additional_tools"):
            gaps.append(CoverageGap(locator, "unsupported Codex response item", cwd))
    seen = {}
    for call, timestamp, (name, inp, call_cwd, atomic, raw) in calls:
        outputs = results.get(call, [])
        failed = any(_output_failed(out) for out in outputs)
        refused = any(_output_refused(out) for out in outputs)
        outcome = ("no-write-refused" if atomic else "partial-error") if refused else (
            "partial-error" if failed else "possible-write" if outputs else "missing-result")
        original = f"codex:{sid}/{thread}/{call}"
        if outputs and any(out != outputs[0] for out in outputs[1:]):
            gaps.append(CoverageGap(original, "conflicting Codex result replay", call_cwd))
            outcome = "missing-result"
        op = Operation(original, call, principal, agent, timestamp, call_cwd, name, inp,
                       outcome, atomic, is_child, raw)
        previous = seen.get(original)
        if previous is None:
            seen[original] = op
            operations.append(op)
        elif previous != op:
            gaps.append(CoverageGap(original, "conflicting Codex call replay", call_cwd))
    return HistoryBatch(operations, gaps)
