---
kind: bug
status: wontfix
tags:
- windows
- ci
- cross-compile
- mingw
- undiagnosed
closed: 2026-08-06
opened: 2026-08-06
owner: marius
related: []
severity: medium
---

# BUG: `Windows-gnu cross (MinGW + wine)` CI job is red and has not been diagnosed

## Summary

> **CLOSED 2026-08-06 as a DUPLICATE of
> `docs/issues/2026-08-06-windows-doctor-rehome-and-index-lock-tests-fail.md` (WIN-28).**
> Status `wontfix` in the duplicate sense — there is nothing separate to fix here.
>
> The hypothesis this file recorded ("probably WIN-28 wearing a second hat, i.e. failures
> outside WIN-27's 12-test wine skip-list") is **confirmed**. Run `31092134665` on
> `99695a10`: the failing-test set of `Windows-gnu cross (MinGW + wine)` (job 92585395679)
> and of `Test (windows-latest / default)` (job 92585395727) are **identical — the same
> nine tests, byte for byte**:
>
> ```
> librarian::tools::doctor::tests::prune_missing_batch_confirm_prunes_dead_roots_only
> librarian::tools::doctor::tests::prune_missing_batch_dry_run_excludes_worktree_covered_root_from_totals
> librarian::tools::doctor::tests::prune_missing_batch_dry_run_lists_dead_roots_without_deleting
> librarian::tools::doctor::tests::run_fix_rehome_dry_run_then_confirm_migrates_rows
> librarian::tools::doctor::tests::run_fix_rehome_errors_when_no_rows_under_old_root
> librarian::tools::doctor::tests::run_fix_rehome_via_surfaced_old_root_arg_dry_runs
> librarian::tools::doctor::tests::validate_rehome_gates
> librarian::util::tests::like_escape_idiom_is_not_inlined_outside_helper
> retrieval::index_lock::tests::lock_path_is_not_sited_in_bare_temp_dir
> ```
>
> Method (the recipe this file itself specified, now executed): dump both jobs with
> `gh run view 31092134665 --log-failed --job <id>`, strip ANSI, extract `... FAILED`
> names, `sort -u`, diff. Zero difference.
>
> So there is no MinGW/wine-specific defect: the cross job simply runs the same suite and
> trips the same nine platform assumptions. It will go green when WIN-28 does, and needs no
> independent diagnosis — which is what this file was opened to obtain.
>
> **What it did contribute:** it forced the comparison that turned four red CI cells into
> one bug. Before this, `Windows-gnu cross` was the only red cell with no known cause, and
> its presence made the Windows failures look like two independent problems.
>
> Reopen only if the two sets ever diverge — i.e. the cross job fails a test that
> `windows-latest` passes. The diff recipe above is the check.

The `windows-gnu` CI job fails on `experiments`. Unlike the other red jobs from
the same run it was **not investigated** — this file exists so a known-failing
gate is recorded in the ledger rather than carried silently into a promotion.

## Symptom (Effect)

From `gh run view 30852803569 --json jobs` (2026-08-03):

```
failure  Windows-gnu cross (MinGW + wine)
```

No per-step log was pulled, so the failing step is unknown. It could be either
of the job's two phases:

```yaml
- name: Cross-build (default features)
  run: scripts/build-windows.sh build
- run: scripts/build-windows.sh test --lib -- --skip symbols_path_type --skip … (9 skips)
```

## Reproduction

Not attempted. The job installs `gcc-mingw-w64-x86-64` + `wine` + `wine64` with
`dpkg --add-architecture i386`, then cross-builds for
`x86_64-pc-windows-gnu` and runs the lib tests under wine. Reproducing locally
needs that toolchain installed; `scripts/build-windows.sh` is the entry point.

## Environment

`ubuntu-latest` runner cross-compiling to `x86_64-pc-windows-gnu`, stable
toolchain, 45-minute timeout. Branch `experiments` at the run's commit — which
is **11 commits behind** the current local HEAD, so some or all of this may
already be fixed.

> **Prior art found after filing (2026-08-06).** A ledger query I should have run
> first turns candidate 1 below from a guess into the leading hypothesis:
>
> - `docs/issues/archive/2026-07-02-windows-gnu-wine-20-test-failures.md`
>   (`status: mitigated`) is the pre-existing wine-suite bug, and WIN-27 in
>   `docs/trackers/windows-platform-support.md` records its outcome: the 8-test
>   `guide_hint` cluster was fixed and un-skipped, **12 tests remain skipped** in
>   `scripts/build-windows.sh`, and `validate_prune_request_gates` was "the one
>   real-Windows MSVC failure" (since fixed this cohort).
> - A red job therefore means failures **outside** that skip list. WIN-28's nine
>   new failures — catalog `rehome`/`prune_missing`, the `like_escape` guard, the
>   index lock — are all new surface from this cohort and are **not** in the skip
>   list. Under wine they would run, and fail.
>
> So this is most likely
> `docs/issues/2026-08-06-windows-doctor-rehome-and-index-lock-tests-fail.md`
> wearing a second hat, not an independent defect. Confirm before fixing twice.
## Root cause

**Unknown — not investigated.** Three candidates, none tested:

1. **The same defects as the native Windows cells.** The job's test step already
   carries a 9-test skip list, and `docs/issues/2026-08-06-windows-doctor-rehome-and-index-lock-tests-fail.md`
   records 9 *different* Windows failures (catalog rehome/prune, `like_escape`
   guard, index lock) that are NOT in that skip list. If those run under wine
   too, this job fails for exactly that reason and the two bugs are one.
2. **Feature-gate rot, already fixed.** The job cross-builds *default* features,
   so the `--no-default-features` breakage fixed in `7938d68b` should not affect
   it — but the run predates that fix, and its test step is `--lib`, which was
   also where the three librarian-dependent tests fixed in `be75e705` lived.
3. **Toolchain / environment.** `apt-get install wine wine64` after
   `dpkg --add-architecture i386` is historically brittle on ubuntu-latest
   image bumps, and this is an infrastructure failure rather than a code one.

Candidate 2 makes a re-run worthwhile before any real investigation.

## Evidence

### CI run 30852803569, 2026-08-03

11 of 15 jobs failed. The four that passed: `MSRV (1.88)`, `Format`,
`Test (ubuntu-latest / default)`, `Test (macos-latest / default)`.

`gh run view <id> --log-failed` was pulled for this run and grepped, but the
extraction targeted the Clippy / Tool Docs Sync / Audit Doc Refs / test-matrix
sections; the windows-gnu section was never read.

## Hypotheses tried

None. Filed as a known-unknown during merge preparation.

## Fix

Not implemented, and not diagnosable from what has been gathered.

## Tests added

N/A.

## Workarounds

None needed for local development — the job is a cross-compilation guard, not a
runtime path. It does gate CI green, so a promotion to `master` carries the red
job over.

## Resume

**2026-08-06 — still red on current HEAD; the run to read is `31091169757`** (commit
`99695a10`). First trustworthy evidence for this job: every prior verdict described code 21
commits stale.

One fact now available that was not before: the six previously-red **non-Windows** cells
(Clippy, Tool Docs Sync, and the four ubuntu/macos feature-gate cells) are all green, so
this job's failure is not shared with them. Combined with all three `windows-latest` cells
still failing, the leading hypothesis stays the one recorded here — that this is WIN-28
wearing a second hat, i.e. failures outside the 12-test wine skip-list recorded in WIN-27,
not a distinct MinGW/wine defect. Confirm or kill it by diffing the two failure sets:

```bash
for J in "Windows-gnu cross (MinGW + wine)" "Test (windows-latest / default)"; do
  gh run view 31091169757 --log-failed \
    --job $(gh run view 31091169757 --json jobs -q \
              ".jobs[] | select(.name==\"$J\") | .databaseId")
done
```

If the failing test names are a subset of WIN-28's nine, close this as a duplicate and
track the fix there.

Cheapest first step, because it may close the file for free: push the current
`experiments` HEAD (14+ commits newer, carrying `7938d68b` and `be75e705`) and
see whether the job goes green on its own. If it is still red, pull the job's
own log rather than the whole run:

```bash
gh run view <new-run-id> --log --job <windows-gnu-job-id> > /tmp/wgnu.log
grep -nE 'error|FAILED|panicked|wine:' /tmp/wgnu.log
```

Then classify against the three candidates under *Root cause*. If the failures
match the 9 tests in
`docs/issues/2026-08-06-windows-doctor-rehome-and-index-lock-tests-fail.md`,
fold this file into that one via
`artifact(action="graft", from_id=<this>, into_id=<that>)` rather than fixing
twice.

## References

- `.github/workflows/ci.yml` — job `windows-gnu`, lines 79-108
- `scripts/build-windows.sh` — the cross-build/test entry point and its 9-test skip list
- `docs/issues/2026-08-06-windows-doctor-rehome-and-index-lock-tests-fail.md` — the native-Windows failures, possibly the same root cause
- `docs/trackers/windows-platform-support.md` — WIN-N issue index
- CI run: https://github.com/mareurs/codescout/actions/runs/30852803569

## Fix provenance

- **SHA:** `7938d68b` (experiments-only) — positional; does not survive a rebase of `experiments`.
- **patch-id:** `29abeee1494bbeec1c5ece4421adf94078ce903f` — content hash of the diff; survives rebase and cherry-pick.

If the SHA stops resolving, recover the commit by patch-id. Use redirects, not pipes —
codescout's Iron Law 3 blocks an unbounded `git log -p` piped to a trimmer:

```
git log --all -p > /tmp/all.patch
git patch-id --stable < /tmp/all.patch > /tmp/patch-ids.txt
grep 29abeee1494b /tmp/patch-ids.txt
```

Each hit is `<patch-id> <commit>`. Several hits mean the change exists on several
branches (cherry-pick) and any of them is the fix. Recorded 2026-08-19.
