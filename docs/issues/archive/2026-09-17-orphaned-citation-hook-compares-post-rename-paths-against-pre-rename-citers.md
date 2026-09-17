---
id: bfe07abae27a3dc1
kind: bug
status: fixed
title: 'BUG: the orphaned-citation hook compares post-rename paths against pre-rename citers, so it warns on the shape it exists to bless'
tags:
- cluster/unclassified
closed: 2026-09-17
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

## Fix provenance

- **SHA:** `a6e06961` (`experiments`)
- **patch-id:** `5f8e8bbcd335d88d770f5ac7136bfe0646aee589`

`committed` now comes from `git diff --cached --name-status -M`, emitting **both** columns of
every `R` row.

Built and `bash -n`'d in a copy before installing. pre-commit.com is retired here, so this
script is live for every session in the checkout the moment it is saved, and a parse error
fails `pre-commit-run.sh` and blocks their commits. The copy was validated against the defect
case (silent) **and** a control where the citer is genuinely outside the commit (906 bytes,
naming the citer) before it was moved in.

## Tests added

`tests/hooks-discrimination.sh` § *orphaned citations* — a new case renaming the citer in the
same commit, asserting silence in **bytes** (exit 0 is true of a warning too).

**THE FIRST VERSION OF THAT CASE PASSED AGAINST THE BROKEN SCRIPT, and that is the finding
worth keeping.** A one-line citer whose only line is rewritten scores **below git's ~50%
similarity cutoff**, so git reports it as `D`+`A` rather than `R` — which puts the old path
back into `--name-only`'s output and suppresses the warning *for the wrong reason*. The defect
is invisible below the threshold and live above it.

That is `doc(action="move")`'s `stage_hint` caveat — *`R` is a **similarity** verdict, not a
staging or a content one* — reaching a test fixture: the same logical change is one row or two
depending on how big the file is. Forty filler lines put the fixture at **R090**; the two real
archives that hit this at `5a61efee` were **R096** and **R097**.

The case therefore carries its own precondition — `fixture: the citer's rename IS detected` —
so it cannot silently go vacuous again if similarity ever drifts back under the cutoff.

**Observed RED before the fix:** `a renamed citer is suppressed like a modified one` FAIL, with
the precondition PASS beside it, so the red was the defect and not a broken fixture.

Suite: `tests/hooks-discrimination.sh` **145/0**; `tests/hook_config.rs` **12/0**. No mutation
probe — it renders INCONCLUSIVE on a shell runner by construction (`6213a09765698cfa`), so the
evidence here is the observed red against a known-clean baseline.

## Workarounds

Read the named citers against `git diff --cached --name-status -M`. If a citer appears as an
`R` source, the warning is spurious.

## Resume

N/A — fixed.

## References

- `scripts/pre-commit-orphaned-citations.sh` — shipped `6853c517`
- `tests/hooks-discrimination.sh` § 12 — the 9 cases that do not reach this
