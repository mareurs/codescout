---
status: open
opened: 2026-09-24
closed:
severity: low
owner: marius
related: []
tags: [cluster/selector-narrower-than-its-population]
kind: bug
---

# BUG: IL-3 refuses git plumbing whose output is bounded by its argument count

## Summary
`git_subcommand_is_single_line` knows plumbing that prints one line total (`rev-parse`,
`merge-base`, …) but not plumbing that prints at most one line **per argument**, so
`git check-ignore -v <path> | tail -1` is refused as a context-flood risk.

## Symptom (Effect)
```
git ls-files --error-unmatch Cargo.toml | tail -2
→ IL3 violation — piped `git ls-files --error-unmatch Cargo.toml | tail -2` to a log-trimmer. BLOCKED.
```
Same for `git diff HEAD~3..HEAD -- Cargo.toml | head -3`.

## Reproduction
Live binary at `506924f2`; `run_command` either command above.

## Root cause
`git_subcommand_is_single_line` (`src/util/path_security.rs:1475`) is an allowlist of
O(1)-line subcommands; everything else falls to `git_output_is_bounded`'s limiter-flag test.
measured 2026-09-24: the refusals above.

**The report's proposed remedy is too wide, and two of its three examples must stay refused:**
- `git diff -- <one path>` — one file's diff has no size bound.
- `git ls-files --error-unmatch <path>` — expands a directory pathspec:
  measured 2026-09-24, `git ls-files --error-unmatch src | wc -l` → **340**.

Only per-argument-bounded plumbing qualifies: `check-ignore` (not `--stdin`) and
`cat-file -t/-s/-e` (not `-p`, not `--batch*`). measured 2026-09-24:
`git check-ignore -v target Cargo.toml .worktrees | wc -l` → 1; `git cat-file -t HEAD` → 1 line.

## Fix

`git_subcommand_is_argument_bounded` (`src/util/path_security.rs`), OR-ed beside
`git_subcommand_is_single_line` in `git_output_is_bounded`. A sibling rather than a widening, so
the existing function's name stays true. `check-ignore` without `--stdin`; `cat-file` only with
`-t`/`-s`/`-e`. An explicit `-p`/`--batch*` exclusion was written and then removed: a mutation
run showed it could never fire, because cat-file's modes are mutually exclusive and the positive
requirement had already refused. Both refusal texts updated to name the new class and the two
commands that stay refused (`detect_il3_violation`, and `refusal_predicate` in
`src/prompts/mod.rs`).

**Partial by design:** the report's own `ls-files` and `diff` examples still refuse, correctly.

Fix SHA / patch-id: _recorded at commit time_.
## Tests added

`src/util/path_security.rs` tests: `il3_allows_git_plumbing_bounded_by_its_argument_count`,
`il3_still_blocks_per_argument_plumbing_in_its_unbounded_modes`,
`il3_still_blocks_single_path_git_commands_that_are_not_argument_bounded`. Mutations: unwired,
`--stdin` allowed, positive requirement removed — all killed.
## Workarounds
Run bare and query the `@cmd_*` buffer.

## Resume

N/A once committed. Tag through the catalog after merge.
## References
- Source report: `codescout-lessons.md` § 6.2.
