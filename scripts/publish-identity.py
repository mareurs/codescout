#!/usr/bin/env python3
"""Resolve publish ownership using the shared harness-neutral context authority."""
import os
from pathlib import Path
import re
import subprocess
import sys

from session_context import principal_key, resolve_context


def main() -> int:
    git = subprocess.run(["git", "rev-parse", "--show-toplevel"],
                         capture_output=True, text=True)
    if git.returncode:
        print("publish-identity: not a git repository", file=sys.stderr)
        return 2
    result = resolve_context(Path(git.stdout.strip()), os.environ)
    if result.state == "missing":
        return 0
    if result.state != "available":
        print(f"publish-identity: {result.state} context: {result.reason}", file=sys.stderr)
        return 2
    context = result.context
    if context.harness == "manual":
        return 0
    if not re.fullmatch(r"[A-Za-z0-9-]+", context.session_id):
        print(f"publish-identity: invalid session id {context.session_id} in "
              "CLAUDE_CODE_SESSION_ID, CODEX_SESSION_ID or explicit context; "
              "expected [A-Za-z0-9-]+", file=sys.stderr)
        return 2
    principal = principal_key(context.harness, context.session_id)
    # Existing Claude trailers and acknowledgements remain byte-compatible.
    print(context.session_id if context.harness == "claude-code" else principal)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
