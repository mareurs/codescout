---
status: open
opened: 2026-08-06
closed:
severity: high
owner: marius
related: []
tags: [windows, librarian, catalog, doctor, retrieval, ci]
kind: bug
---

# BUG: nine tests fail on windows-latest — catalog rehome/prune, like_escape guard, index lock

## Summary

The `Test (windows-latest / default)` CI cell fails with 9 failures, all in code
this cohort added: the catalog GC/`rehome` repair modes, the `like_escape` idiom
guard, and the new per-project retrieval index lock. Linux and macOS pass the
same config, so these are Windows-specific — almost certainly path semantics
(separator, drive prefix, or canonicalization) rather than logic.

Blocks a green promotion to `master`: `master` and `experiments` both run CI on
push, so merging carries the red cell over.

## Symptom (Effect)

From CI run 30852803569 (2026-08-03), job `Test (windows-latest / default)`:

```
test result: FAILED. 3250 passed; 9 failed; 11 ignored; 0 measured; 0 filtered out

librarian::tools::doctor::tests::prune_missing_batch_confirm_prunes_dead_roots_only
librarian::tools::doctor::tests::prune_missing_batch_dry_run_excludes_worktree_covered_root_from_totals
librarian::tools::doctor::tests::prune_missing_batch_dry_run_lists_dead_roots_without_deleting
librarian::tools::doctor::tests::run_fix_rehome_dry_run_then_confirm_migrates_rows
librarian::tools::doctor::tests::run_fix_rehome_errors_when_no_rows_under_old_root
librarian::tools::doctor::tests::run_fix_rehome_via_surfaced_old_root_arg_dry_runs
librarian::tools::doctor::tests::validate_rehome_gates
librarian::util::tests::like_escape_idiom_is_not_inlined_outside_helper
retrieval::index_lock::tests::lock_path_is_not_sited_in_bare_temp_dir
```

The `local-embed` Windows cell shows the same 9 against a smaller total
(`3238 passed; 9 failed; ... 12 filtered out`), so the failures are in the
librarian/retrieval core rather than feature-gated paths.

Individual assertion messages were not captured — the CI log excerpt collected
lists the test lines but not their panic output.

## Reproduction

Needs a Windows runner (or the MinGW cross target, which has its own
skip-list — see `scripts/build-windows.sh`). Not reproducible on Linux: the
same tests pass locally under `cargo test`.

```bash
# On Windows:
cargo test librarian::tools::doctor
cargo test librarian::util::tests::like_escape_idiom_is_not_inlined_outside_helper
cargo test retrieval::index_lock
```

## Environment

`windows-latest` GitHub runner, stable toolchain, both `default` and
`local-embed` configs. Linux and macOS `default` cells pass.

## Root cause

Unknown — under investigation. Three candidate groups, each with a different
likely mechanism:

1. **`doctor` rehome / prune_missing (7 tests).** These compare and rewrite
   absolute root paths. Windows paths carry a drive prefix and `\` separators,
   and the catalog stores a forward-slash form (`doctor`'s own
   `abs_path` checks exist precisely because of this). A dead-root check that
   compares strings rather than normalized paths would fail here while passing
   on POSIX. Note a sibling fix already landed this cohort —
   `fix(tests): make doctor validate_prune_request_gates path-portable` — so the
   pattern is established and these are the cases that pass missed.
2. **`like_escape_idiom_is_not_inlined_outside_helper`.** A source-scanning
   guard: it reads the codebase to assert the escape idiom is not inlined
   outside its helper. Likely a path-glob or line-ending (CRLF) assumption in
   the scan itself, not a defect in `like_escape`.
3. **`lock_path_is_not_sited_in_bare_temp_dir`.** Asserts the per-project index
   lock is not placed in a bare temp dir. Windows temp resolution differs
   (`%TEMP%` under the user profile vs `/tmp`), so the "bare temp dir"
   predicate may not hold. Related: this cohort's
   `fix(retrieval,lsp): stop test lock files leaking into the per-user runtime
   dir` touched the same area.

## Evidence

### CI run 30852803569, 2026-08-03

Job list for the run — 11 of 15 jobs failed; the Windows `default` cell is one
of them:

```
failure  Test (windows-latest / default)
failure  Test (windows-latest / local-embed)
failure  Test (windows-latest / no-features)
success  Test (ubuntu-latest / default)
success  Test (macos-latest / default)
```

The `no-features` Windows cell fails for a different, now-fixed reason
(feature-gate rot — see `7938d68b`), not these 9 tests.

## Hypotheses tried

None yet — filed on discovery during merge preparation.

## Fix

Not implemented. Needs the per-test panic output from a Windows run first;
guessing at path normalization without it risks "fixing" the tests rather than
the code.

## Tests added

N/A — the failing tests already exist; the defect is that they do not pass on
Windows.

## Workarounds

None. If the promotion to `master` cannot wait, the honest options are to fix
these, or to add them to a documented Windows skip-list the way
`scripts/build-windows.sh` already does for nine other tests — the latter being
a deliberate coverage reduction that should be recorded, not a silent one.

## Resume

**2026-08-06 — reproduced on current HEAD; the run to read is `31091169757`** (commit
`99695a10`, branch `experiments`). This is the first CI run against a non-stale tree since
2026-08-03, so it is the first trustworthy evidence for this bug — earlier runs described
code 21 commits behind.

All three Windows cells fail: `Test (windows-latest / default)`,
`Test (windows-latest / no-features)`, `Test (windows-latest / local-embed)`. That the
failure is feature-independent is itself information: it points at the platform-behaviour
failures catalogued here rather than at feature-gate rot, which is what the other six
previously-red cells turned out to be (all now green).

Fetch the panic output with:

```bash
gh run view 31091169757 --log-failed \
  --job $(gh run view 31091169757 --json jobs \
            -q '.jobs[] | select(.name=="Test (windows-latest / default)") | .databaseId')
```

Still blocked on a Windows runner for interactive work, but the logs alone should give the
nine panic messages, which is enough to start.

Get the panic output: re-run CI on the current `experiments` HEAD (11 commits
newer than run 30852803569 — some of these may already be fixed), then open the
`Test (windows-latest / default)` job log and capture the assertion message for
each of the 9. Group by mechanism using the three candidates under *Root cause*,
then fix per group. Start with `validate_rehome_gates`, which is the narrowest
and most likely to reveal the shared path-comparison assumption.

## References

- `src/librarian/tools/doctor.rs` — the rehome / prune_missing modes and tests
- `src/librarian/util.rs` — `like_escape` and its idiom guard
- `src/retrieval/index_lock.rs` — the per-project lock
- `scripts/build-windows.sh` — the existing MinGW cross skip-list
- CI run: https://github.com/mareurs/codescout/actions/runs/30852803569
- `docs/trackers/windows-platform-support.md` — WIN-N issue index
