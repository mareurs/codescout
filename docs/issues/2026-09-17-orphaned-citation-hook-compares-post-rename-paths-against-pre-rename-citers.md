---
id: '0d3426f903c8cd61'
kind: bug
status: open
title: 'BUG: the orphaned-citation hook compares post-rename paths against pre-rename citers, so it warns on the shape it exists to bless'
tags:
- cluster/unclassified
---

# BUG: the orphaned-citation hook compares post-rename paths against pre-rename citers, so it warns on the shape it exists to bless

## Summary

`scripts/pre-commit-orphaned-citations.sh` suppresses its warning when the citing file is in
the same commit. It builds that suppression set from `git diff --cached --name-only`, which
reports **only the destination** of a rename, and its citer list from `git grep … HEAD`, which
returns paths under their **pre-rename** names. Two path namespaces, compared as one.

So a citer that is *itself* being renamed in the same commit can never match, and the hook
warns on the correct shape — the one its own comment says must stay silent.

## Symptom (Effect)

Committing `5a61efee` (two bug files archived together, each citing the other, both repointed
in that same commit):

```
note: this commit MOVES a path that files it is not committing still cite.

  docs/issues/2026-09-16-a-symlinked-instruction-file-…md
      still cited by: docs/issues/2026-09-17-the-reclamation-predicate-…md
  docs/issues/2026-09-17-the-reclamation-predicate-…md
      still cited by: docs/issues/2026-09-16-a-symlinked-instruction-file-…md
```

Both named citers **were in that commit**. Warning only, so nothing was blocked.

## Reproduction

Any commit that renames two files which cite each other, or renames a file whose citer is also
renamed in the same commit. Minimal shape:

```
doc(action="move", …)   # a.md -> archive/a.md, citing b.md
doc(action="move", …)   # b.md -> archive/b.md, citing a.md
git add -- <all four paths> && git commit -- <all four paths>
```

## Environment

`experiments`, hook at `6853c517` (shipped 2026-09-16), observed on `5a61efee` 2026-09-17.

## Root cause

Two inputs, two namespaces:

```
$ git show 5a61efee --name-only --diff-filter=ACMRD
docs/issues/archive/2026-09-16-…md        <- DESTINATION only
docs/issues/archive/2026-09-17-…md

$ git show 5a61efee --name-status -M
R096  docs/issues/2026-09-16-…md  docs/issues/archive/2026-09-16-…md   <- BOTH
R097  docs/issues/2026-09-17-…md  docs/issues/archive/2026-09-17-…md
```

`scripts/pre-commit-orphaned-citations.sh` sets `committed` from the first form. Its citer list
comes from `git grep -l -F "$stem" HEAD`, which reads the **committed tree** — deliberately, and
correctly, since that is the population the check is about. But HEAD predates the rename, so a
renamed citer appears there under its old path.

The suppression test is then a literal path match:

```bash
printf '%s\n' "$committed" | grep -qxF "$citer" && continue
```

`docs/issues/2026-09-17-…md` (citer, HEAD name) is never equal to
`docs/issues/archive/2026-09-17-…md` (committed, destination name). No match, so no suppression.

**The hook's own comment states the discrimination it fails:** *"A citer this commit is ALSO
committing is being repointed in the same commit, which is the correct shape and must stay
silent. This is the whole discrimination: the check is about the COMPLEMENT of the commit."*

## Evidence

The `-M` flag is already used ten lines above, to find the rename sources the check iterates
over. So the information needed to fix this is not merely available — the script already asks
git for it, in the adjacent statement, and then builds the suppression set with a different
command that discards it.

## Hypotheses tried

1. **Hypothesis:** the citers genuinely were outside the commit.
   **Test:** `git show 5a61efee --name-status -M`.
   **Verdict:** rejected — both are `R` rows of that commit, and their destinations carry the
   repointed citations (verified in the index before committing).

## Fix

Build `committed` from `git diff --cached --name-status -M` and include **both** columns of
every `R` row, not `--name-only`'s destination alone.

Not fixed here. It is a warning-only hook and the mis-fire costs a glance, so it does not
warrant a same-session Rust-free detour ahead of whatever is next; filed so it is not
re-discovered.

**Do not fix it by dropping `-M` from the source scan instead.** Without `-M` git reports a
rename as an unrelated add and delete, and the check's whole subject — rename sources — becomes
unreadable. The script's own comment says so.

## Tests added

None. `tests/hooks-discrimination.sh` § 12 covers this hook with 9 cases; none constructs a
**renamed citer**, which is why a hook whose comment names this exact case shipped unable to
handle it. A fix owes a case there: two files renamed in one commit, each citing the other,
asserting silence.

## Workarounds

Read the named citers against `git diff --cached --name-status -M`. If a citer appears as an
`R` source, the warning is spurious.

## Resume

Change the `committed=` assignment in `scripts/pre-commit-orphaned-citations.sh` to parse
`--name-status -M` and emit both columns for `R` rows. Add the mutual-rename case to
`tests/hooks-discrimination.sh` § 12 and confirm it RED before the change — the existing 9
cases all pass against the broken script.

## References

- `scripts/pre-commit-orphaned-citations.sh` — shipped `6853c517`
- `tests/hooks-discrimination.sh` § 12 — the 9 cases that do not reach this
