---
kind: bug
status: fixed
tags:
- windows
- librarian
- catalog
- doctor
- retrieval
- ci
closed: 2026-08-06
opened: 2026-08-06
owner: marius
related: []
severity: high
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

**2026-08-06 — UNBLOCKED. All nine panic messages obtained from CI logs (run `31092134665`,
job 92585395727), so no Windows runner was needed after all.** `gh run view --log-failed`
carries the full test stdout. The nine fall into **three** root causes, not nine:

### Cluster A — POSIX-shaped absolute paths in fixtures are not absolute on Windows (4 tests)

```
doctor.rs:1235  validate_rehome_gates
  assertion failed: validate_rehome_request(Some("/gone/old"), Some(live.to_str().unwrap()), &cat.conn).is_ok()
doctor.rs:1262  run_fix_rehome_dry_run_then_confirm_migrates_rows
  called `Result::unwrap()` on an `Err` value: old_root and new_root must both be absolute paths
doctor.rs:1347  run_fix_rehome_via_surfaced_old_root_arg_dry_runs
  called `Result::unwrap()` on an `Err` value: old_root and new_root must both be absolute paths
doctor.rs:1368  run_fix_rehome_errors_when_no_rows_under_old_root
  assertion failed: err.to_string().contains("no catalog rows found")
```

`Path::is_absolute()` on Windows requires a drive or UNC prefix, so the hardcoded
`"/gone/old"` fixture is **relative** there and `validate_rehome_request`'s absolute-path
gate rejects it before the test's actual subject runs. The fourth test is the same cause
wearing a different mask: it asserts on the *wrong* error string because the call failed at
the path gate rather than at the row lookup it meant to exercise.

**Test-side defect, not a product defect.** The gate is behaving correctly; the fixture is
POSIX-only.

### Cluster B — the same POSIX fixture shape, one layer on (3 tests)

```
doctor.rs:1516  prune_missing_batch_dry_run_lists_dead_roots_without_deleting        left 0, right 1
doctor.rs:1554  prune_missing_batch_dry_run_excludes_worktree_covered_root_from_totals
  "per-root counts are still shown even for a covered root"                          left 0, right 1
doctor.rs:1581  prune_missing_batch_confirm_prunes_dead_roots_only                   left 0, right 1
```

All three expect to find exactly one dead root and find zero. Consistent with the same
cause — a POSIX-shaped root that matches no catalog row on Windows, where stored paths look
like `D:\a\codescout\...`. **Not yet proven**: the panic gives only the count, not the
prefix that was matched. Confirm by logging the root under test before asserting, or by
running the fixture with a `\\`-prefixed root.

### Cluster C — two separator-normalisation guards (2 tests)

```
librarian/util.rs:109  like_escape_idiom_is_not_inlined_outside_helper
   left: ["D:\a\codescout\codescout/src\librarian\util.rs (1)"]
  right: ["D:\a\codescout\codescout/src/librarian/util.rs (1)"]
```

A **mixed-separator** string: the repo root is joined with `/` and the relative part with
OS separators. The expected value normalises the relative part to forward slashes; the
actual does not. Test-side — the guard's reporting path needs `RepoPath`/forward-slash
normalisation before comparison.

```
retrieval/index_lock.rs:288  lock_path_is_not_sited_in_bare_temp_dir
  assertion `left != right` failed: lock file must not sit directly in the bare temp dir,
  got parent Some("C:\Users\RUNNER~1\AppData\Local\Temp")
   left: Some("C:\Users\RUNNER~1\AppData\Local\Temp")
  right: Some("C:\Users\RUNNER~1\AppData\Local\Temp\")
```

Read this one carefully before acting. `assert_ne!` **failed**, meaning the two compared
equal — and they do, as `Path` values, because component iteration ignores a trailing
separator even though the `Debug` strings differ by one. So on Windows the lock file's
parent really is the bare temp dir, which is exactly what this assertion exists to forbid.

**This is the one cluster that may be a genuine product finding rather than a fixture
shape**, and the panic alone does not settle which: either `index_lock` sites the lock
directly in `%TEMP%` on Windows (product bug — the guard is right), or the fixture hands it
a temp path whose trailing separator collapses the intended subdirectory (test bug). Read
`src/retrieval/index_lock.rs` around the path construction before choosing; do **not**
relax the assertion to make it pass.

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

**FIXED AND VERIFIED IN CI 2026-08-06 (experiments, `cd643d58`).** Run `31098286970`:
`Test (windows-latest / default)` **3283 passed, 0 failed**, plus `windows-latest/no-features`,
`windows-latest/local-embed` and `Windows-gnu cross (MinGW + wine)` all green. 14 of 15 jobs
green; the only red left is `Audit Doc Refs`, tracked separately in
`docs/issues/2026-08-06-docs-ref-drift-backlog-across-eleven-subdirs.md`.

**Not one of the nine was a product defect.** Two were assertions failing against *correct*
implementations; the rest were fixtures encoding POSIX-only path shapes. Fixing the product to
satisfy them would have been actively harmful — see cluster A.

### Cluster A + B — POSIX-shaped absolute literals (7 tests)

Added a `dead_root(tag)` helper in the doctor test module: `std::env::temp_dir()` supplies a
real absolute prefix on every OS, a unique suffix keeps the path missing, and its parent
exists so the derived dead root lands on the suffix rather than climbing further.
`RepoPath`-normalised so it concatenates with the forward-slash `abs_path` shape the catalog
stores.

`derive_dead_roots`' `if !path.is_absolute() { continue; }` guard was left alone deliberately.
It is protected by its own test (`derive_dead_roots_skips_non_absolute_paths`) because without
it the ancestor climb bottoms out at an empty `PathBuf` whose prune `WHERE` matches **every**
absolute row. Relaxing it to accept `/gone/old` on Windows would have converted a red test into
a data-loss bug.

`count_dead_root_counts_rows_under_root` keeps its POSIX literals on purpose — it calls
`count_dead_root` directly with an explicit root, so it is pure LIKE string matching that never
reaches the `is_absolute()` guard, and its assertions are about prefix-sibling semantics rather
than paths on disk.

### Cluster C — separators (2 tests)

`like_escape_idiom_is_not_inlined_outside_helper`: both sides of the comparison now go through
`normalize_rel_path`, which already lived in that file. `CARGO_MANIFEST_DIR` is OS-shaped and
walkdir appends OS separators to it, so the actual hit was mixed
(`...codescout/src\librarian\util.rs`) against a forward-slash expectation.

`lock_path_is_not_sited_in_bare_temp_dir`: **the product was right.** The threat is `/tmp` being
world-writable; on Windows `temp_dir()` is `%LOCALAPPDATA%\Temp`, already inside the user's
profile, so `per_user_runtime_dir()`'s `#[cfg(not(unix))]` arm returns it unchanged by design and
`lock_path`'s doc comment says so. Scoped `#[cfg(unix)]` rather than relaxed, and paired with a
new platform-independent `lock_path_is_sited_in_the_per_user_runtime_dir` so Windows keeps
coverage of the siting invariant.

### Two more that only CI could surface

- `prune_missing_batch_dry_run_excludes_worktree_covered_root_from_totals` needed **two** passes.
  First the seed: `covered_root.join("x.md").to_string_lossy()` yields backslashes on Windows
  while `count_dead_root` builds its LIKE prefix through `RepoPath`, so the fixture seeded a
  shape the catalog never contains. Normalising the seed then moved the mismatch downstream into
  the test's own `find`, which compared the (now forward-slash) row `root` against a
  backslash-rendered `covered_root`. Both sides now go through `RepoPath`.
- `lsp::client::tests::workspace_symbols_returns_project_symbols` was **timing, not logic** — it
  passed on run `31092134665` and failed on `31094160241`. Its own comment already recorded GHA
  Windows as I/O-slow with a 15 s budget; that is still not enough there (cold `cargo metadata`,
  no warm target cache) while ubuntu and macOS pass. Windows now gets 4× (60 s); the loop exits
  early on success so the ceiling is free when indexing is fast. Widened rather than skipped, so
  Windows keeps `workspace/symbol` coverage.

**On Linux every one of these spellings coincides, so no local run could have caught them** —
3488 tests passed locally before each push. They were only ever observable in CI.

Experiments-branch SHAs: `0326a016` (clusters A/B/C), `2784131a` (seed normalisation + LSP
budget), `cd643d58` (dry-run row comparison). Master-side SHAs still need recording after
cherry-pick per CLAUDE.md § "After cherry-pick".

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

N/A — fixed and verified in CI on `experiments`.

One technique worth keeping, because it was needed to diagnose this and will be needed again:
`gh run view --log-failed` refuses per-job logs until the **whole** run finishes, and a stalled
sibling job (`Windows-gnu cross` sat 41+ minutes on "Install MinGW + wine" and was ultimately
cancelled by the next push) blocks that indefinitely. The REST endpoint serves one job's log
regardless of run state:

```bash
gh api --allow-escape-sequences \
  "/repos/mareurs/codescout/actions/jobs/<job_id>/logs"
```

`--allow-escape-sequences` is required or `gh` refuses the ANSI-bearing body.
## References

- `src/librarian/tools/doctor.rs` — the rehome / prune_missing modes and tests
- `src/librarian/util.rs` — `like_escape` and its idiom guard
- `src/retrieval/index_lock.rs` — the per-project lock
- `scripts/build-windows.sh` — the existing MinGW cross skip-list
- CI run: https://github.com/mareurs/codescout/actions/runs/30852803569
- `docs/trackers/windows-platform-support.md` — WIN-N issue index
