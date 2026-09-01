#!/usr/bin/env python3
"""Refuse a commit whose issue-cluster Index counts disagree with the corpus it STAGES.

Why this exists as a hook rather than only as `tests/issue_clusters.rs`:
`every_index_count_matches_the_corpus` reads the WORKING TREE, so it certifies the state on
disk. A partial commit ships a state that never existed on disk, and no re-run of that test
can address it — running the gate before committing answers a different question than the
commit poses, and answers it green. Measured 2026-09-01 (`cluster-promotion-session-log:F-7`,
`reconnaissance-patterns:R-155`): `461c037a` staged the ledger and six of seven retagged bug
files went unstaged, publishing counts of 3/1/2 against ZERO members at HEAD.

This reads the INDEX and nothing else -- `git ls-files` for the population, `git show :<path>`
for content -- so it answers the question the commit actually poses. It deliberately does NOT
depend on pre-commit's unstaged-stash: measured, that stash gives hooks the index version of
tracked files but leaves UNTRACKED files on disk, so a filesystem-reading hook would read the
ledger correctly and over-count the corpus.

The parse logic is duplicated from `tests/issue_clusters.rs` on purpose -- a cargo invocation
in the commit path costs ~7s and blocks unboundedly on the shared `target/` lock. Divergence
is closed by a MECHANISM, not by vigilance: `the_hook_script_agrees_with_this_gate` runs this
file with `--source=worktree --json` and fails if the two derivations disagree. Change one and
the test reddens until you change the other.
"""

from __future__ import annotations

import json
import subprocess
import sys

LEDGER = "docs/trackers/issue-clusters.md"


def _git(*args: str) -> str:
    r = subprocess.run(["git", *args], capture_output=True, text=True)
    if r.returncode != 0:
        raise SystemExit(f"git {' '.join(args)} failed: {r.stderr.strip()}")
    return r.stdout


def read(path: str, source: str) -> str | None:
    """File content from the index (`git show :path`) or the working tree."""
    if source == "index":
        r = subprocess.run(["git", "show", f":{path}"], capture_output=True, text=True)
        return r.stdout if r.returncode == 0 else None
    try:
        with open(path, encoding="utf-8") as fh:
            return fh.read()
    except OSError:
        return None


def bug_files() -> list[str]:
    """Mirrors `tracked_all_bug_files` -- the INDEX population, so untracked files are excluded."""
    return [
        p
        for p in _git("ls-files", "docs/issues").splitlines()
        if p.endswith(".md") and not p.endswith("_TEMPLATE.md")
    ]


def valid_slugs(ledger: str) -> set[str]:
    """Mirrors `valid_slugs` -- the ledger's `**Slug:**` declarations are the closed set."""
    out = set()
    for line in ledger.splitlines():
        if not line.startswith("**Slug:**"):
            continue
        rest = line[len("**Slug:**"):]
        a = rest.find("`")
        if a < 0:
            continue
        b = rest.find("`", a + 1)
        if b < 0:
            continue
        inner = rest[a + 1 : b]
        if not inner.startswith("cluster/"):
            continue
        slug = inner[len("cluster/") :]
        # The ledger's own template declares `cluster/<slug>`; a placeholder is not a class.
        if slug and all(c.islower() and c.isascii() or c == "-" for c in slug):
            out.add(slug)
    return out


def frontmatter(content: str) -> str | None:
    """Mirrors `frontmatter` -- the YAML block without its fences, or None."""
    if not content.startswith("---\n"):
        return None
    rest = content[len("---\n") :]
    end = rest.find("\n---")
    return None if end < 0 else rest[:end]


def cluster_tags(fm: str) -> list[str]:
    """Mirrors `cluster_tags` -- BOTH YAML forms; reading one silently under-reports."""
    out: list[str] = []
    in_block = False
    for line in fm.splitlines():
        if line.startswith("tags:"):
            rest = line[len("tags:") :].strip()
            if rest.startswith("[") and rest.endswith("]"):
                out.extend(t.strip().strip("\"'") for t in rest[1:-1].split(","))
                in_block = False
            else:
                in_block = rest == ""
            continue
        if in_block:
            if line.startswith("-"):
                item = line[1:]
            elif line.lstrip().startswith("- "):
                item = line.lstrip()[1:]
            else:
                # A non-item line ends the block; a later key is not a tag.
                in_block = False
                continue
            out.append(item.strip().strip("\"'"))
    return [t[len("cluster/") :] for t in out if t.startswith("cluster/")]


def parse_index_counts(ledger: str, valid: set[str]) -> dict[str, int]:
    """Mirrors `parse_index_counts` -- the cell IMMEDIATELY AFTER the slug cell.

    Never "the first number in the row": the mechanism column carries digits of its own, so a
    scan-for-a-number parser returns a plausible wrong value rather than failing. A row whose
    count cell does not parse is left ABSENT rather than defaulted to 0.
    """
    out: dict[str, int] = {}
    for line in ledger.splitlines():
        if not line.startswith("| IC-"):
            continue
        cells = [c.strip() for c in line.split("|")]
        for i, cell in enumerate(cells):
            if not (cell.startswith("`") and cell.endswith("`") and len(cell) > 1):
                continue
            inner = cell[1:-1]
            if inner not in valid:
                continue
            if i + 1 < len(cells):
                try:
                    out[inner] = int(cells[i + 1])
                except ValueError:
                    pass
            break
    return out


def actual_counts(valid: set[str], source: str) -> dict[str, int]:
    """Mirrors `actual_counts` -- seeded at 0 so a class with no members is still compared."""
    out = {s: 0 for s in valid}
    for rel in bug_files():
        content = read(rel, source)
        if content is None:
            continue
        fm = frontmatter(content)
        if fm is None:
            continue
        for tag in cluster_tags(fm):
            if tag in out:
                out[tag] += 1
    return out


def main() -> int:
    source = "index"
    as_json = False
    for arg in sys.argv[1:]:
        if arg.startswith("--source="):
            source = arg.split("=", 1)[1]
        elif arg == "--json":
            as_json = True
        elif arg == "--fixture-tags":
            # Pure over stdin, so `the_hook_script_agrees_on_both_yaml_tag_styles` can feed
            # both YAML forms directly. The live corpus cannot exercise the inline arm:
            # measured 2026-09-01, ZERO bug files carry a `cluster/` tag in flow style, so a
            # corpus-driven agreement check leaves that branch untested however green it is.
            print(json.dumps(cluster_tags(frontmatter(sys.stdin.read()) or "")))
            return 0
    if source not in ("index", "worktree"):
        raise SystemExit(f"--source must be index|worktree, got {source!r}")

    ledger = read(LEDGER, source)
    if ledger is None:
        # Not staged and not on disk: nothing to check. Silence is correct -- a commit that
        # touches neither the ledger nor a bug file must not be blocked by this hook.
        return 0

    valid = valid_slugs(ledger)
    declared = parse_index_counts(ledger, valid)
    actual = actual_counts(valid, source)

    if as_json:
        print(json.dumps({"declared": declared, "actual": actual}, sort_keys=True))
        return 0

    drift = [
        f"cluster/{slug} — table says {n}, staged corpus has {actual.get(slug, 0)}"
        for slug, n in sorted(declared.items())
        if actual.get(slug, 0) != n
    ]
    if not drift:
        return 0

    print(
        "the Index table's `n` column disagrees with the corpus THIS COMMIT STAGES:\n  "
        + "\n  ".join(drift)
        + "\n\n"
        "This reads the INDEX, so it is about what you are SHIPPING, not what is on disk.\n"
        "`cargo test --test issue_clusters` can be green at the same moment — it reads the\n"
        "working tree, and a partial commit ships a state that never existed there.\n\n"
        "Most likely: you staged the ledger and not the bug files whose tags it counts.\n"
        "  git status --short docs/issues     # what is not staged\n"
        "  git add <the bug file(s)>          # then re-commit\n\n"
        "If the count is genuinely wrong, re-derive rather than adjust by the delta:\n"
        "  git grep -clE '^[[:space:]]*-[[:space:]]*cluster/<slug>[[:space:]]*$' "
        "-- 'docs/issues/*.md' | wc -l\n"
        "and move the `**Members:**` and `**Promotes to:**` fields of BOTH classes in the\n"
        "same pass — a count and the judgement reading it move independently.",
        file=sys.stderr,
    )
    return 1


if __name__ == "__main__":
    sys.exit(main())
