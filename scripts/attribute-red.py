#!/usr/bin/env python3
"""attribute-red.py — when a build red names a file, say WHOSE uncommitted file it is.

WHY THIS EXISTS
---------------
On a shared checkout, uncommitted WIP that does not compile reds both test lanes for
every session in the tree. The reader sees a compile error in a file they never touched
and has no channel to the author, so routing goes by proximity — which is anti-evidence
when three sessions touched the tree in an hour. Measured 2026-09-08: the identifier was
sitting in a bug file's `claimed_by` twenty minutes before the red and still went unread.
Full record: docs/issues/2026-09-08-a-claimed-bug-file-names-the-author-of-the-wip-that-reds-the-build.md
(option 3, `IC-10`'s `H`-target).

TWO-STAGE BY CONSTRUCTION, because the resolution is not cheap
--------------------------------------------------------------
Measured 2026-09-08 on this machine: the transcript scan costs **7.0 s** and does not
cache (two consecutive runs, 6.98 s / 7.02 s — the bug file recorded 5.0 s, so the cost
has grown with the corpus; re-derive it rather than citing either number). `git status
--porcelain` costs **0.01 s**. So the cheap question is asked first and the expensive one
only when it can pay:

    1. is anything dirty at all?                        0.01 s  — usually stops here
    2. does the red name one of those dirty paths?      free    — pure text
    3. only then: who wrote it, and can they be asked?  7.0 s

A red that names no dirty file is silent. That is the common case and it must stay free.

WHAT IT REFUSES TO DO
---------------------
`UNKNOWN` is never rendered as "not theirs" or "yours". A write this tool's heuristics
miss is indistinguishable from no write at all, so absence is a statement about coverage.
This is the same refusal `file-provenance.py` makes, and it matters more here: this output
is read at a moment when someone is looking for somebody to blame.

REACHABILITY CEILING — READ THIS BEFORE TRUSTING SILENCE
--------------------------------------------------------
Wired into `run_command`'s failure path. **Native `Bash` bypasses `run_command`
entirely**, so a session that runs its gate through `Bash` gets no hint and cannot tell
that from "nothing to report". An alarm nothing reaches is exactly as informative as no
alarm (CLAUDE.md § Testing Discipline). The ceiling is named here, at the site, and in
`docs/PROBES.md` — closing it means a PostToolUse hook in codescout-companion, which is
cross-repo and inert until its version bumps in all three profiles.

USAGE
    cargo test 2>&1 | ./scripts/attribute-red.py          # reads the red from stdin
    ./scripts/attribute-red.py --paths src/foo.rs         # or name the paths directly
    ./scripts/attribute-red.py --explain                  # why it stayed silent
"""
from __future__ import annotations

import importlib.util
import os
import re
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent


def _load_provenance():
    """Import file-provenance.py as a module.

    Imported rather than shelled out: a subprocess would re-run the 7 s transcript scan
    and re-read every registry, and the two tools must agree about liveness by sharing
    the code rather than by both being careful.
    """
    path = HERE / "file-provenance.py"
    spec = importlib.util.spec_from_file_location("_fp", path)
    if spec is None or spec.loader is None:
        return None
    mod = importlib.util.module_from_spec(spec)
    try:
        spec.loader.exec_module(mod)
    except Exception:
        return None
    return mod


# Where a toolchain names a file in a failure. Deliberately NOT a bare path-shaped-token
# matcher: this text also contains the command line, the crate graph and any prose the
# test printed, and a token that merely APPEARS is not a token the red is about.
DIAGNOSTIC_PATH = [
    re.compile(r"^\s*-->\s+([^\s:]+):\d+:\d+", re.M),          # rustc span
    re.compile(r"panicked at '?([^\s:']+\.rs):\d+", re.M),      # panic site
    re.compile(r"panicked at ([^\s:]+\.rs):\d+", re.M),
    re.compile(r"^error(?:\[[^\]]+\])?:.*?\b([\w./\\-]+\.rs)\b", re.M),
    re.compile(r"^\s*Error:\s+([\w./\\-]+\.(?:rs|py|sh|md))\b", re.M),
]


def named_paths(text: str) -> list[str]:
    """Paths a failure text points AT, in first-seen order."""
    seen, out = set(), []
    for pat in DIAGNOSTIC_PATH:
        for m in pat.finditer(text):
            p = m.group(1).replace("\\", "/")
            if p not in seen:
                seen.add(p)
                out.append(p)
    return out


def dirty_paths(root: Path) -> set[str]:
    """Repo-relative paths with uncommitted content — index OR worktree.

    Both columns, deliberately. A peer mid-`git add` shows ` M` in one and `M ` in the
    other within the same second, and the question "is this file uncommitted right now"
    does not care which side of the index it is on.
    """
    try:
        out = subprocess.run(["git", "-C", str(root), "status", "--porcelain"],
                             capture_output=True, text=True, timeout=20)
    except (OSError, subprocess.SubprocessError):
        return set()
    paths = set()
    for line in out.stdout.splitlines():
        if len(line) < 4:
            continue
        p = line[3:]
        if " -> " in p:          # rename: the destination is the live path
            p = p.split(" -> ", 1)[1]
        paths.add(p.strip().strip('"'))
    return paths


def main(argv: list[str]) -> int:
    explain = "--explain" in argv
    explicit: list[str] = []
    if "--paths" in argv:
        explicit = [a for a in argv[argv.index("--paths") + 1:] if not a.startswith("-")]

    fp = _load_provenance()
    if fp is None:
        if explain:
            print("attribute-red: file-provenance.py could not be imported; no attribution "
                  "is possible. This is a tooling failure, NOT evidence that the red is "
                  "nobody's.", file=sys.stderr)
        return 0

    root = fp.repo_root()

    # --- stage 1: 0.01 s. Usually stops here. -------------------------------------
    dirty = dirty_paths(root)
    if not dirty:
        if explain:
            print("attribute-red: working tree is clean, so no uncommitted state can be "
                  "behind this red.", file=sys.stderr)
        return 0

    # --- stage 2: free. Pure text against the dirty set. --------------------------
    if explicit:
        candidates = [fp.normalize(p, root) or p for p in explicit]
    else:
        text = "" if sys.stdin.isatty() else sys.stdin.read()
        candidates = [fp.normalize(p, root) or p for p in named_paths(text)]
    hits = [p for p in dict.fromkeys(candidates) if p in dirty]
    if not hits:
        if explain:
            print(f"attribute-red: the red names no dirty path "
                  f"({len(dirty)} dirty, {len(candidates)} named). Silent by design — "
                  f"this says nothing about whether the red is yours.", file=sys.stderr)
        return 0

    # --- stage 3: 7.0 s, and only now. --------------------------------------------
    owners = fp.scan(root)
    live = fp.live_sessions()
    me = os.environ.get("CLAUDE_CODE_SESSION_ID", "")

    lines, foreign_any = [], False
    for rel in hits:
        # THE WINDOW, and it is not optional here. `scan()` returns a path's LIFETIME
        # authors; the question at a red is who made the UNCOMMITTED delta. Skipping it
        # names everyone who ever touched the file — measured while building this: a bug
        # file this session had just edited attributed to two peers who wrote it days
        # ago, and neither to the actual editor. That is the worst possible failure for
        # a tool read at the moment someone is looking for a party to blame, so the
        # floor is derived per path exactly as file-provenance's own main() does.
        lc = fp.last_commit_time(rel, root)
        floor = fp._key(lc) if lc else None
        records = owners.get(rel, [])
        in_window, undated = [], []
        for w, when in records:
            if when is None:
                undated.append(w)          # unplaceable: kept, and marked below
            elif floor is None or fp._key(when) >= floor:
                in_window.append(w)
        who = sorted(set(in_window) | set(undated))
        if not who:
            lines.append(f"  {rel}")
            lines.append("      no session on record wrote this since it was last "
                         "committed. That is a statement about")
            lines.append("      COVERAGE, not ownership — a Bash write the heuristic "
                         "misses looks identical.")
            continue
        peers = [w for w in who if w != me]
        if not peers:
            lines.append(f"  {rel}")
            lines.append(f"      written by THIS session ({me[:8]}) — your own "
                         f"uncommitted work, so this red is likely yours.")
            continue
        foreign_any = True
        lines.append(f"  {rel}")
        for w in peers:
            mark = "  [undated — could not be placed in the window]" if (
                w in undated and w not in in_window) else ""
            suffix, extra = fp.address_lines(w, live, " " * 8)
            lines.append(f"      written by {w}{mark}{suffix}")
            lines.extend(extra)

    if not lines:
        return 0
    print("\n[codescout] this failure names files with UNCOMMITTED changes:\n")
    print("\n".join(lines))
    if foreign_any:
        print("\n  Another session is holding at least one of these. Ask before editing, "
              "reverting\n  or formatting it — and do not attribute the red to them until "
              "they confirm: this\n  names who WROTE the file, never who broke the build.")
    print("\n  Scope: uncommitted state only, from Claude transcripts across every "
          "discovered\n  profile. Native `Bash` bypasses run_command, so a peer working "
          "through Bash can be\n  invisible here — silence is not 'nobody'.")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
