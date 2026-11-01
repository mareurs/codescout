---
id: 80fee58db6c3a95d
kind: bug
status: fixed
title: 'BUG: a grep result with matches is silent about the hidden-path skip, so a missing .github/ hit reads as absent'
owners:
- marius
tags:
- cluster/selector-narrower-than-its-population
- grep
- completeness-warning
- false-negative
topic: grep completeness
closed: 2026-11-01
opened: 2026-10-31
severity: medium
unverified: 'Not yet seen live: the running codescout server predates 21229c864, so a real grep from this repo''s root was not checked for the note after a rebuild and /mcp.'
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

`21229c864`. A result with matches now carries `hidden_skipped`: one line naming up to four hidden root DIRECTORIES and the `include_hidden=true` remedy. `format_grep` prints it below the count header, in the same place as the zero warning. Both producers set it: the files-mode early return and the main tail.

- The builder is `hidden_skip_note` in `src/tools/grep_hidden_note.rs`. It reuses the zero warning's two predicates, `WalkAudit::hidden_at_root` and `WalkAudit::globs_can_reach_a_root_dot_entry`, so the two halves agree on what was hidden and a glob anchored under a visible directory stays bare.
- It is a separate key from `completeness_warning`, whose text opens with "this zero" and would be false on a non-zero result.
- **Hidden files are left out**, unlike the zero warning. Nearly every repository has a root `.gitignore`, so counting dotfiles made the note ride almost every search. The first version did exactly that in its own bare-tree test. A pruned directory hides a whole subtree, which is the `.github/` case the note exists for.
- On this repo the note still rides most searches from the root, because several hidden directories sit there (`.github/`, `.buddy/`, `.claude/` and more). That is the cost the record accepted: one short line.
- Written as text edits, with the user's approval, while rust-analyzer's watcher spun on the `resident-private` symlinks (`35bcf086deeb7cbe`) and `edit_code` could not run. `edit_file` refuses new definitions, so the builder and its tests sit in `#[path]` modules beside `grep.rs`, the pattern `grep_route_path_tests.rs` already used.

## Tests added

`src/tools/grep_hidden_note_tests.rs`, two tests:

- `a_result_with_matches_names_the_hidden_root_entries_it_skipped`: content and files mode both carry the note naming `.github/` and `include_hidden`, no `completeness_warning`, and the rendered text holds the note. Positive twin: `include_hidden=true` finds 2 matches and carries no note.
- `a_result_with_matches_stays_bare_when_nothing_hidden_was_reachable`: a root with only `.git`, `.gitignore` and `.env`, and a glob anchored under `src/`.

Red first: the first test failed on the missing key. Mutants with `scripts/mutation-probe.sh`, each killed:

| Mutant | Killed by |
|---|---|
| drop the directories-only filter | the bare test |
| drop the glob-reach check | the bare test |
| drop the renderer line | the first test |
| drop the files-mode producer | the first test |

`cargo test --lib tools::grep::` 67 of 67. Gate: `PYTHON_LIGHT=0 CLIPPY=0 LEAN=0 DEFAULT=0`, `FMT=1 ownership-refused` on one line of `grep_hidden_note_tests.rs`, which I then formatted; `rustfmt --check` passes on every file I touched. The gate was not rerun for that whitespace change.

## Workarounds

Pass `include_hidden=true` whenever the question is "where is X mentioned at all", or search `.github/` with an explicit `path`.

## Resume

Fixed in `21229c864`. Not yet seen on a rebuilt server (see `unverified`).

## Fix provenance

- **SHA:** `21229c864` (`experiments`)
- **patch-id:** `deefcbd015a24aa55f1a7480bbfa9eeec21d0861`

## References

- `docs/issues/archive/2026-08-07-grep-zero-match-silent-about-hidden-skip.md` — the zero-match half, fixed.
- `docs/adrs/2026-08-27-negative-results-name-their-scope.md`.
- Found while planning `docs/superpowers/specs/2026-10-31-gate-ram-tier-design.md`.
