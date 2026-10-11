#!/usr/bin/env python3
"""Parse rustfmt paths literally and reject targets outside the actual checkout."""
from __future__ import annotations
import re
import subprocess
import sys
from pathlib import Path


def confined_path(value: str, root: Path) -> str:
    if not value or any(c in value for c in "\r\n\0"):
        raise ValueError("unrepresentable formatter path")
    path = Path(value)
    if not path.is_absolute():
        path = root / path
    resolved = path.resolve(strict=True)
    if not resolved.is_file():
        raise ValueError("formatter target is not a file")
    resolved.relative_to(root)
    checkout = subprocess.run(["git", "-C", str(resolved.parent), "rev-parse", "--show-toplevel"],
                              capture_output=True, text=True)
    if checkout.returncode != 0 or Path(checkout.stdout.strip()).resolve(strict=True) != root:
        raise ValueError("formatter target belongs to a different Git checkout")
    # Preserve the spelling given to provenance, including spaces and symlinks.
    return str(path.relative_to(root))


def parse_paths(mode: str, text: str, root: Path, targets: list[str]) -> list[str]:
    if mode in ("allow", "refuse"):
        verdicts = {}
        for line in text.splitlines():
            match = re.match(r"^(MINE|CLEAN|PEER|SHARED|UNKNOWN)\s+(.+)$", line)
            if not match:
                continue
            verdict, path = match.groups()
            if path not in targets or path in verdicts:
                raise ValueError("ambiguous or unexpected provenance target")
            confined_path(path, root)
            verdicts[path] = verdict
        if set(verdicts) != set(targets):
            raise ValueError("provenance did not return exactly one verdict per target")
        return [p for p in targets if (verdicts[p] in ("MINE", "CLEAN")) == (mode == "allow")]
    pattern = r"^Diff in (.*):[0-9]+:$" if mode == "diff" else r"^(/.*):$"
    found = []
    for line in text.splitlines():
        match = re.match(pattern, line)
        if match:
            path = confined_path(match.group(1), root)
            if path not in found:
                found.append(path)
    if mode == "emit" and any(confined_path(t, root) not in found for t in targets):
        raise ValueError("rustfmt did not enumerate every permitted module root")
    if mode == "diff" and not found:
        raise ValueError("formatting check named no diff targets")
    return found


def main() -> int:
    try:
        mode, root_arg, *targets = sys.argv[1:]
        if mode not in ("diff", "emit", "allow", "refuse"):
            raise ValueError("unknown path parser mode")
        root = Path(root_arg).resolve(strict=True)
        paths = parse_paths(mode, sys.stdin.read(), root, targets)
        for path in paths:
            print(path)
        return 0
    except (ValueError, OSError, RuntimeError) as error:
        print(f"fmt-mine: cannot safely resolve formatter targets: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
