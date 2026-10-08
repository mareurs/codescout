#!/usr/bin/env python3
"""Run the light, dependency-heavy, or complete top-level Python test inventory."""
import argparse
import importlib.util
import os
from pathlib import Path
import subprocess
import sys

HEAVY_TESTS = frozenset({
    "tests/test_phase1b_step4.py",
    "tests/test_phase1b_step5.py",
    "tests/test_phase1b_training.py",
})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--lane", choices=("light", "heavy", "all"), default="light")
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1],
                        help="repository root (defaults to this script's repository)")
    args = parser.parse_args()
    root = args.root.resolve()
    inventory = sorted(path.relative_to(root).as_posix()
                       for path in (root / "tests").glob("test_*.py") if path.is_file())
    missing = sorted(HEAVY_TESTS - set(inventory))
    if missing:
        print("Python test inventory is missing classified heavy files: " +
              ", ".join(missing), file=sys.stderr)
        return 2

    selected = [path for path in inventory
                if args.lane == "all" or (path in HEAVY_TESTS) == (args.lane == "heavy")]
    excluded = [path for path in inventory if path not in selected]
    print(f"Python lane: {args.lane}", flush=True)
    for label, paths in (("Selected", selected), ("Excluded", excluded)):
        print(f"{label} ({len(paths)} files):", flush=True)
        for path in paths:
            print(f"  {path}", flush=True)
    if not selected:
        print("No test files selected; refusing an empty Python lane.", file=sys.stderr)
        return 2

    modules = ["pytest"] if args.lane == "light" else ["pytest", "torch", "sklearn"]
    missing_modules = [module for module in modules if importlib.util.find_spec(module) is None]
    if missing_modules:
        requirements = root / ("requirements-python-light.txt" if args.lane == "light"
                               else "requirements-python-heavy.txt")
        print("Missing Python test dependencies: " + ", ".join(missing_modules) +
              f". Install with: {sys.executable} -m pip install -r {requirements}",
              file=sys.stderr)
        return 2
    env = os.environ.copy()
    env["PYTHONDONTWRITEBYTECODE"] = "1"
    return subprocess.run(
        [sys.executable, "-m", "pytest", "-p", "no:cacheprovider", "-q", *selected],
        cwd=root, env=env,
    ).returncode


if __name__ == "__main__":
    raise SystemExit(main())
