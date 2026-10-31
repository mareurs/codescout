---
id: e313cba4b1d3d738
kind: bug
status: open
title: 'BUG: a grep result with matches is silent about the hidden-path skip, so a missing .github/ hit reads as absent'
owners:
- marius
tags:
- cluster/selector-narrower-than-its-population
- grep
- completeness-warning
- false-negative
topic: grep completeness
opened: 2026-10-31
severity: medium
---

# BUG: a grep result with matches is silent about the hidden-path skip, so a missing `.github/` hit reads as absent

## Summary

`grep` skips dot-prefixed paths by default. Since bug `790e34f82ec93c1b`, a result with **zero** matches carries a `completeness_warning` that names the skipped hidden entries. A result with **one or more** matches carries nothing. So a reader who looks for the complete set of files that mention a term gets a partial list that reads as complete, and a hit under `.github/` is silently absent.

## Class fit

IC-18: the default search selector (every non-hidden path) is narrower than the population a reader assumes it names (the whole project), and the result does not state the difference. It differs from bug `790e34f82ec93c1b`, the zero-match member, because here the result is non-empty, so the reader has no empty result to make them doubt it.

## Symptom (Effect)

`grep(pattern="gate-slot", mode="files")` at HEAD `6ef568eae`:

```
37 matches in 23 files
```

No line in the result mentions hidden paths. The same call with `include_hidden=true`:

```
46 matches in 27 files
      4  .github/workflows/ci.yml
```

The CI job `gate-slot-tests` in `.github/workflows/ci.yml` is the only runner of `tests/gate-slot.sh`. The default result therefore supported the false conclusion "nothing runs this suite".

## Reproduction

1. `grep(pattern="gate-slot", mode="files")` → 23 files, no `.github/workflows/ci.yml`, no warning.
2. `grep(pattern="gate-slot", mode="files", include_hidden=true)` → 27 files, including `.github/workflows/ci.yml`.

## Environment

codescout MCP server on `experiments`, 2026-10-31, project root `/home/marius/work/claude/codescout`.

## Root cause

`completeness_warning` is attached only on the zero-match paths. The Fix section of `docs/issues/archive/2026-08-07-grep-zero-match-silent-about-hidden-skip.md` says so: it is attached "on a zero-match result in *both* the `mode=\"files\"` early return and the main tail". The non-zero paths were out of that fix's scope.

measured 2026-10-31: the two calls above, default against `include_hidden=true`. The location in `src/tools/grep.rs` is inferred from the archived Fix section, not read this session.

## Evidence

The two results quoted under Symptom.

## Hypotheses tried

1. **Hypothesis:** the search excluded `.github/` because of a glob. **Test:** the call in step 1 passes no glob. **Verdict:** rejected.
2. **Hypothesis:** the default skip of hidden paths is the cause. **Test:** step 2. **Verdict:** confirmed.

## Fix

Not fixed. A likely shape: attach a short hidden-root note to every result whose walk pruned a hidden root entry that is not excluded (the `.git` and `.codescout` exclusions from `790e34f82ec93c1b` stay). The open design point is noise: most searches never need hidden paths, so the note must be short, for example one line naming the pruned directories.

## Tests added

None yet. A test needs a tree with one visible match and one match under a hidden directory, and must assert the note on the non-zero result.

## Workarounds

Pass `include_hidden=true` whenever the question is "where is X mentioned at all", or search `.github/` with an explicit `path`.

## Resume

Read the `completeness_warning` attachment sites in `src/tools/grep.rs`, then decide whether a non-zero result gets the same warning or a shorter note.

## References

- `docs/issues/archive/2026-08-07-grep-zero-match-silent-about-hidden-skip.md` — the zero-match half, fixed.
- `docs/adrs/2026-08-27-negative-results-name-their-scope.md`.
- Found while planning `docs/superpowers/specs/2026-10-31-gate-ram-tier-design.md`.
