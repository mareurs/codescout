#!/usr/bin/env python3
"""Resolve invocation identity without pointers, PID inference or host precedence."""
from dataclasses import asdict, dataclass
import json
import os
from pathlib import Path
import subprocess
import sys
from typing import Mapping


@dataclass(frozen=True)
class SessionContext:
    schema_version: int
    harness: str
    session_id: str
    agent_id: str | None
    workspace: Path
    source: str
    harness_version: str | None

    def to_dict(self) -> dict:
        value = asdict(self)
        value["workspace"] = str(self.workspace)
        return value


@dataclass(frozen=True)
class ContextResolution:
    state: str
    context: SessionContext | None
    reason: str

    def to_dict(self) -> dict:
        return {"state": self.state,
                "context": self.context.to_dict() if self.context else None,
                "reason": self.reason}


def principal_key(harness: str, session_id: str) -> str:
    return harness + ":" + session_id


def _failure(state: str, reason: str) -> ContextResolution:
    return ContextResolution(state, None, reason)


def _nonempty(value: object) -> bool:
    return isinstance(value, str) and bool(value.strip())


def validate_context(value: object, workspace: Path) -> ContextResolution:
    if not isinstance(value, dict):
        return _failure("unsupported", "context must be an object")
    fields = ("schema_version", "harness", "session_id", "agent_id",
              "workspace", "source", "harness_version")
    for field in fields:
        if field not in value:
            return _failure("unsupported", f"missing context field {field}")
    version = value["schema_version"]
    if type(version) not in (int, float) or version != 1:
        return _failure("unsupported", "unsupported schema_version")
    if value["harness"] not in ("claude-code", "codex", "manual"):
        return _failure("unsupported", "unsupported harness")
    if value["source"] not in ("native-hook", "native-env", "explicit"):
        return _failure("unsupported", "unsupported source")
    if not _nonempty(value["session_id"]):
        return _failure("unsupported", "session_id must be a nonempty string")
    for field in ("agent_id", "harness_version"):
        if value[field] is not None and not _nonempty(value[field]):
            return _failure("unsupported", f"{field} must be null or a nonempty string")
    if not _nonempty(value["workspace"]) or not Path(value["workspace"]).is_absolute():
        return _failure("unsupported", "workspace must be absolute")
    try:
        actual = workspace.resolve(strict=True)
        supplied = Path(value["workspace"]).resolve(strict=True)
        if actual != supplied:
            return _failure("conflicting", "context workspace differs from invocation checkout")
    except (OSError, RuntimeError):
        return _failure("unsupported", "workspace cannot be resolved")
    return ContextResolution("available", SessionContext(
        1, value["harness"], value["session_id"], value["agent_id"],
        actual, value["source"], value["harness_version"]), "")


def _native_context(workspace: Path, environ: Mapping[str, str]) -> ContextResolution:
    claude = environ.get("CLAUDE_CODE_SESSION_ID")
    codex = environ.get("CODEX_SESSION_ID")
    if claude and codex:
        return _failure("conflicting", "both CLAUDE_CODE_SESSION_ID and CODEX_SESSION_ID are present")
    if not claude and not codex:
        return _failure("missing", "no native root identity; use --context or --check")
    harness, root = ("claude-code", claude) if claude else ("codex", codex)
    thread = environ.get("CODEX_THREAD_ID") if codex else None
    agent = thread if thread and thread != root else None
    version = environ.get("CODEX_VERSION") if codex else None
    return validate_context({
        "schema_version": 1, "harness": harness, "session_id": root,
        "agent_id": agent, "workspace": str(workspace), "source": "native-env",
        "harness_version": version or None,
    }, workspace)


def resolve_context(workspace: Path, environ: Mapping[str, str],
                    context_path: Path | None = None) -> ContextResolution:
    native = _native_context(workspace, environ)
    inherited = environ.get("CODESCOUT_CONTEXT_FILE")
    paths = [Path(p) for p in (context_path, inherited) if p is not None and str(p)]
    if not paths:
        return native
    if any(not p.is_absolute() for p in paths):
        return _failure("unsupported", "context file path must be absolute")
    try:
        resolved = [p.resolve(strict=True) for p in paths]
    except (OSError, RuntimeError):
        return _failure("unsupported", "context file cannot be resolved")
    if len(set(resolved)) > 1:
        return _failure("conflicting", "CLI and CODESCOUT_CONTEXT_FILE identify different context files")
    try:
        explicit = validate_context(json.loads(resolved[0].read_text()), workspace)
    except (OSError, UnicodeError, ValueError):
        return _failure("unsupported", "context file is unreadable or malformed JSON")
    if explicit.state != "available":
        return explicit
    if native.state in ("conflicting", "unsupported"):
        return native
    if native.context:
        a, b = explicit.context, native.context
        if (a.harness, a.session_id, a.workspace) != (b.harness, b.session_id, b.workspace):
            return _failure("conflicting", "explicit context disagrees with native harness, root or workspace")
        if a.agent_id is not None and b.agent_id is not None and a.agent_id != b.agent_id:
            return _failure("conflicting", "explicit agent_id disagrees with native child identity")
    return explicit


def main(argv: list[str] | None = None) -> int:
    try:
        args = list(sys.argv[1:] if argv is None else argv)
        options = {}
        while args:
            flag = args.pop(0)
            if flag not in ("--workspace", "--context") or flag in options or not args:
                raise ValueError("usage: --workspace <absolute-root> [--context <absolute-json-path>]")
            options[flag] = args.pop(0)
        root = Path(options.get("--workspace", ""))
        if "--workspace" not in options or not root.is_absolute():
            raise ValueError("absolute --workspace required")
        git = subprocess.run(["git", "rev-parse", "--show-toplevel"], capture_output=True, text=True)
        if git.returncode:
            raise ValueError("invocation is not in a Git checkout")
        if Path(git.stdout.strip()).resolve(strict=True) != root.resolve(strict=True):
            result = _failure("conflicting", "supplied workspace differs from actual Git checkout")
        else:
            path = Path(options["--context"]) if "--context" in options else None
            result = resolve_context(root, os.environ, path)
    except (ValueError, OSError, RuntimeError) as error:
        result = _failure("unsupported", str(error))
    print(json.dumps(result.to_dict()))
    return 0 if result.state == "available" else 2


if __name__ == "__main__":
    raise SystemExit(main())
