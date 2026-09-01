#!/usr/bin/env python3
"""Find (and optionally strip) a stale SECOND frontmatter block in a librarian artifact.

WHAT IT LOOKS FOR
-----------------
`artifact(action="create")` writes the YAML frontmatter itself from its typed
parameters.  A caller that also puts a `---` block at the top of `body` gets it
written BELOW the catalog's, where nothing reads it: two blocks, the second inert.
That form is refused at the tool boundary since `reject_body_leading_frontmatter`
(`src/librarian/tools/create.rs`, called from both `create` and `update`), so new
corruption cannot arrive through the librarian.  This probe exists for the copies
that predate the guard, and for anything written to disk around it.

THE DISCRIMINATOR — the whole point of this file
------------------------------------------------
Position is NOT the test.  `---` means BOTH "frontmatter delimiter" AND "horizontal
rule", so "a second `---` pair below the first" also describes a perfectly ordinary
callout fenced by two hrules.  A position-only detector reports those, and a repair
built on one silently deletes a real document's rule pair.

Measured 2026-09-02: a position-only scan over this repo returned 9 files; the
content test rejected 1 of them
(`docs/issues/archive/2026-08-31-nested-hook-state-dirs-are-untracked-but-not-ignored.md`,
whose "block" is a `>` blockquote between two hrules).  An 11% false-positive rate,
every member of it a file the repair would have damaged.

So a candidate is accepted only when its interior parses as a YAML **mapping**.
Prose fails `yaml.safe_load` outright or yields a str/list, and is rejected by kind.

WHAT --apply DOES, AND DOES NOT, DO
-----------------------------------
It removes the second block and normalises to one blank line.  It does NOT move keys
into the surviving frontmatter, because a frontmatter write must go through the
catalog or it does not reach it (BL-48).  The report names, per file:

  LIFT      keys present only in the dead block -> re-add with
            artifact(action="update", id=..., patch={"extra": {...}})
  CONFLICT  keys in both -> the catalog's copy wins; the dead one is stale by
            construction (it is what the catalog was built to supersede)
  EMPTY     keys whose value is null/[]/{} -> carry no information, dropped

Usage:  probe-double-frontmatter.py            report only (exit 1 if any found)
        probe-double-frontmatter.py --diff     report + unified diff per file
        probe-double-frontmatter.py --apply    write the repairs
"""

import difflib
import os
import pathlib
import subprocess
import sys

try:
    import yaml
except ImportError:
    sys.exit("needs PyYAML: pip install pyyaml")

REPO = subprocess.run(
    ["git", "rev-parse", "--show-toplevel"], capture_output=True, text=True
).stdout.strip()


def scan():
    """Every tracked .md whose body opens with a real second frontmatter mapping."""
    files = subprocess.run(
        ["git", "ls-files", "*.md"], capture_output=True, text=True, cwd=REPO
    ).stdout.split()
    found = []
    for rel in files:
        path = pathlib.Path(REPO, rel)
        try:
            text = path.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError):
            continue
        lines = text.split("\n")
        if not lines or lines[0].strip() != "---":
            continue
        close1 = next((i for i in range(1, len(lines)) if lines[i].strip() == "---"), None)
        if close1 is None:
            continue
        open2 = close1 + 1
        while open2 < len(lines) and lines[open2].strip() == "":
            open2 += 1
        if open2 >= len(lines) or lines[open2].strip() != "---":
            continue
        close2 = next((k for k in range(open2 + 1, len(lines)) if lines[k].strip() == "---"), None)
        if close2 is None:
            continue
        # THE DISCRIMINATOR: interior must parse as a YAML mapping, or it is prose.
        try:
            fm2 = yaml.safe_load("\n".join(lines[open2 + 1 : close2]))
        except yaml.YAMLError:
            continue
        if not isinstance(fm2, dict) or not fm2:
            continue
        try:
            fm1 = yaml.safe_load("\n".join(lines[1:close1])) or {}
        except yaml.YAMLError:
            fm1 = {}
        found.append((rel, text, lines, close1, open2, close2, fm1, fm2))
    return found


def repaired(lines, close1, close2):
    rest = lines[close2 + 1 :]
    while rest and rest[0].strip() == "":
        rest.pop(0)
    return lines[: close1 + 1] + [""] + rest


def main():
    apply_ = "--apply" in sys.argv
    show_diff = "--diff" in sys.argv or apply_
    hits = scan()

    for rel, text, lines, close1, open2, close2, fm1, fm2 in hits:
        new = repaired(lines, close1, close2)
        lift = {k: v for k, v in fm2.items() if k not in fm1 and v not in (None, [], {})}
        conflict = {k: (fm1[k], fm2[k]) for k in fm2 if k in fm1 and fm1[k] != fm2[k]}
        empty = [k for k in fm2 if k not in fm1 and fm2[k] in (None, [], {})]

        print(f"\n{rel}   (dead block: lines {open2 + 1}-{close2 + 1})")
        print(f"  LIFT     {lift or '-'}")
        print(f"  CONFLICT {conflict or '-'}   (catalog wins)")
        print(f"  EMPTY    {empty or '-'}")
        if show_diff:
            print(
                "\n".join(
                    "  " + d
                    for d in difflib.unified_diff(
                        text.split("\n"), new, fromfile=rel, tofile=rel + " (repaired)",
                        lineterm="", n=1,
                    )
                )
            )
        if apply_:
            pathlib.Path(REPO, rel).write_text("\n".join(new), encoding="utf-8")

    verb = "repaired" if apply_ else "found"
    print(f"\n-- {verb} {len(hits)} file(s) with a stale second frontmatter block")
    if hits and not apply_:
        print("-- re-run with --apply to strip them, then lift LIFT keys via artifact(update)")
    return 1 if hits and not apply_ else 0


if __name__ == "__main__":
    sys.exit(main())
