---
kind: bug
status: fixed
title: 'BUG: the member gate keyed identity on the path, so every archive move read as a new member'
tags:
- cluster/selector-narrower-than-its-population
- issue-clusters
- pre-commit
- rename
closed: 2026-09-02
last_observed: 2026-09-02
last_verified: 2026-09-02
---

# BUG: the member gate keyed identity on the path, so every archive move read as a new member

## Summary

`scripts/pre-commit-ledger-counts.py`'s member-documentation check compared a bug file's cluster
tags **per path**, against the same path at `HEAD`. An archive move is a rename, so the new path
does not exist at `HEAD`, the "before" read returned `None`, and every tag on the file read as
newly added. The gate then demanded a `**Members:**` edit naming a member the field **already
named**.

Shipped at `1b3ac36b` and hit within the hour, on the normal end state of every bug record —
CLAUDE.md mandates archiving each verified fix, so this fired on the one operation the process
requires most.

## Symptom (Effect)

From a peer's live commit `81710e49`:

```
rename docs/issues/{ => archive}/2026-09-02-read-only-true-is-inert-at-every-root.md (99%)

a class gained a member and its `**Members:**` does not name it:
  cluster/accepted-parameter-silently-dropped -- expected the field to change
    and to contain one of: `read-only-true-is-inert-at-every-root`
```

`**Members:**` already contained that slug. It had been put there an hour earlier at `1559daa5`,
in the commit the same gate passed.

## Reproduction

Any `git mv docs/issues/X.md docs/issues/archive/X.md` where `X` carries a `cluster/` tag, staged
and committed. Deterministic, not load-sensitive.

## Root cause

```python
for rel in bug_files():          # CURRENT index paths
    now = read(rel, source)
    was = read(rel, "head")      # the SAME path at HEAD  <- the defect
    ...
    for slug in now_tags - was_tags: out[slug].add(stem)
```

Two compounding parts:

1. **Identity was the path.** `docs/issues/X.md` and `docs/issues/archive/X.md` are one record
   whose path moved; the comparison treated them as unrelated files.
2. **`HEAD` was interrogated with the index's path list**, not its own. The two trees disagree
   about paths *precisely when a rename is in flight*, which is the only time this check matters.

The dateless stem — the identity that *is* stable across the move — was computed two lines below,
and used only to build the message. The comparison never touched it.

The deletion side of the rename was invisible because the loop walks only paths that exist **now**.
`gained` was additionally derived from an `actual` vs `before` count comparison, which structurally
cannot separate a move from a gain, because the two populations are different path sets.

## Evidence

Found independently, from opposite directions, within minutes: `codescout-cc` (sessionId
`953b5e77`) read the loop and named the mechanism; this session read the same refusal off
`--source=index` exit=1 against the peer's staged rename. The convergence is worth recording
because the two instruments were genuinely different — source reading versus running the tool —
rather than two views of the same scope.

`cc` also **declined to file it**, reporting it instead so the author could judge whether it was a
defect or a deliberate cost of the per-path form. And it worked around the false refusal by adding
*real* information — the archive path and the re-keyed artifact id `421564ab7890f8f8` (was
`114306f5be948990`) — rather than a trailing space. That is a useful datapoint about the
accident-proofing itself: the gate extracted genuine content while firing for the wrong reason,
which is the harder test of a check that refuses whitespace.

## Hypotheses tried

1. **Hypothesis:** read `git diff --cached --raw`'s rename status (`R`) and pair the two sides.
   **Verdict:** rejected, though it is exact where the adopted fix is heuristic. It inherits git's
   similarity threshold, so a bug file edited heavily in the same commit that archives it drops
   below the threshold and reappears as a gain. **A false positive that returns only sometimes is
   worse than the one being fixed** — this one was at least deterministic.

## Fix

Fixed at `610fe141`, patch-id `d40ee445623c44305491000598cc53c812f7d103`.

Identity is the dateless stem, compared as **sets** across the two trees; `HEAD` gets its own
population via `git ls-tree -r --name-only HEAD docs/issues`; and `gained` derives from that same
stem identity rather than from a count comparison.

## Fix provenance

- **SHA:** `610fe141` (`experiments`) — positional; does not survive a rebase of `experiments`.
- **patch-id:** `d40ee445623c44305491000598cc53c812f7d103` — content hash of the diff; survives rebase and cherry-pick.

Structured because `structured_fix_pointers` in `src/librarian/tools/doctor.rs` reads
`- **SHA:**` / `- **patch-id:**` list items and nothing else, so the accurate prose form in
§ *Fix* above read as **no anchor declared** — and this file's prose carries five commit-like
hashes, of which only this one is the fix. Verified 2026-09-02 before archiving: the SHA
resolves to a commit contained in `experiments`, and `git show 610fe141 | git patch-id --stable`
reproduces the patch-id above.
## Tests added

Four mutations, and the fourth is what makes the other three mean anything:

| | scenario | expected |
|---|---|---|
| A | archive move (rename) | **passes** — the defect |
| B | genuinely new bug file with a class tag | refused, names the stem |
| C | retag an existing file into a new class | refused, names the stem |
| D | archive move **and** a new member in one commit | refused **for the new one only** |

Without **D**, the fix could have been a blanket *"renames are invisible"* — a silent hole rather
than a repair. A commit that archives one record while adding another is exactly where a blanket
exemption stops discriminating, and it is the shape this repo produces constantly.

## Resume

Nothing outstanding. The durable claim is narrower than "handle renames": **when a comparison
needs an identity, ask which operations the system mandates and whether the identity survives
them.** Here the process requires archiving every verified fix, so a path-keyed identity was
guaranteed to break — not by an edge case, but by the workflow's normal end state.

## References

- Fixed at `610fe141` (patch-id `d40ee445623c44305491000598cc53c812f7d103`); introduced at
  `1b3ac36b`.
- Peer's triggering commit: `81710e49`; the earlier passing commit whose work it duplicated:
  `1559daa5`.
- `docs/trackers/issue-clusters.md` — `IC-18`, `cluster/selector-narrower-than-its-population`.
- Judged **not** an instance of `bug-fix-session-log:F-100`, by the reporter and accepted by the
  author: the window here excludes one *side of a paired event*, not a refuting observation.
