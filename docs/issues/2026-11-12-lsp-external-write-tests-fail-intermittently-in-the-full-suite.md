---
id: '8117d5eac05e5723'
kind: bug
status: mitigated
title: The LSP external-write tests fail intermittently in the full suite
owners:
- marius
tags:
- cluster/unclassified
- testing
closed: 2026-11-12
opened: 2026-11-12
severity: medium
unverified: 'Mitigated, not shown fixed, and the mitigation covers the rename tests only: 20 clean full-suite iterations against an earlier rate of 2 in 15, so about a 6% chance of that result if nothing had changed. A peer session (aadfcca23) recorded one failure of `goto_definition_follows_a_write_made_outside_codescout` (empty result, `src/lsp/client.rs:4262`) in the lean lane of a Plan B2 branch tree that does not contain 4603d77cf (checked with `git merge-base --is-ancestor`, rc 1). 4603d77cf changes only the rename helper, so no run measured whether goto-definition still fails on the fixed code; one failure in four gates is a sample, not a rate. Why the server answers `null` was not established. The null answers still happen on the fixed code for all I know: the retry hides them, and they were not counted. The mutation check on the retry was one run of one mutant (both resync sites removed). Two earlier attempts to reproduce the failure (18 loaded iterations of the rename tests alone, 12 loaded module-parallel iterations) did not reproduce it; only the whole suite did, so the condition may be the suite''''s own parallelism and not load. The original failure rate was 2 of 6 lane runs on the branch and 1 of 6 on experiments.'
---

# BUG: The LSP external-write tests fail intermittently in the full suite

## Summary

Several tests in `src/lsp/client.rs` that write a file outside codescout and then query the language server fail intermittently when the whole suite runs. They pass when run alone. They failed on a branch and on a clean `experiments`, so the cause is not in the branch that exposed it.

## Class fit

The failing tests are measured but their mechanism is not established, so cluster/unclassified keeps the uncertainty. The same step passes alone and fails under load, which looks like a timing race, but no run has shown which side lost the race.

## Symptom (Effect)

Failures seen on 2026-11-12, in the full suite only. Each failing test ran for about 0.6 to 0.7 s.

```
thread 'lsp::client::tests::goto_definition_follows_a_write_made_outside_codescout' panicked at src/lsp/client.rs:4262:9:
assertion `left == right` failed: the call is on line 4 now and must still resolve: []
  left: 0
 right: 1
```

```
thread 'lsp::client::tests::hover_follows_a_write_made_outside_codescout' panicked at src/lsp/client.rs:4311:9:
hover at the call's new position must describe target_fn, got None
```

The third failing test was `lsp::client::tests::rename_edit_does_not_land_on_another_callee_after_a_same_size_write_outside_codescout`. Its panic text was not captured.

Captured 2026-11-12 (session `5f715d8f`, full-suite stress run, iterations 8 and 14 of 15, after 0.55 s and 0.74 s), the missing panic text of the rename test:

```
thread 'lsp::client::tests::rename_edit_does_not_land_on_another_callee_after_a_same_size_write_outside_codescout' panicked at src/lsp/client.rs:4106:27:
rename failed: invalid type: null, expected struct WorkspaceEdit
```

So the server answered the rename with JSON `null`, and `LspClient::rename` could not decode it. See `9391e97eeb937e90`.

## Reproduction

Not reproducible on demand. The full lane fails in a minority of runs.

- Full lane: `cargo nextest run --no-default-features --no-fail-fast`, or the same command with `--workspace`.
- Alone, `-E 'test(/follows_a_write_made_outside_codescout/)'` passed 3 of 3 runs on the branch.

## Environment

Linux, rust-analyzer started by the tests, a repo with 28 linked worktrees under `.worktrees`. Measured on the branch `feat/artifact-identity-plan-b1-20261111` and on `experiments` at `56b3eb525`.

## Root cause

Unknown — see Hypotheses tried. Measured 2026-11-12: the failures are in the tests named above, the same step passes alone, and the tests belong to the peer fix for `docs/issues/2026-11-11-symbol-at-and-hover-answer-from-stale-lsp-content-after-an-external-edit.md`. Inferred, not measured: a race between the external write and the language server's view of the file.

**Update 2026-11-12 (session `5f715d8f`), for the rename test:** the failing step is the rename request itself, not an assertion about the edit. The server answered `null`, which the protocol allows (`lsp_types::request::Rename::Result` is `Option<WorkspaceEdit>`); `LspClient::rename` decodes into a plain `WorkspaceEdit` and so turns it into an error, and the test helper retried only a cancelled request. So it is the same family as the three tests fixed in `b398b53d2`: an empty answer right after an outside write, on a loaded host. Not established: why the server answered `null`. It is the failing variant only (`resync_other = false`, a same-size swap); the other three rename tests passed in those iterations.

## Evidence

Full-suite results, each lane run once per row unless stated:

| Tree | Lane | Result |
|---|---|---|
| branch `ec51112f0` | lean and default | all green |
| branch after the second merge | lean | `goto_definition_…` and `hover_…` failed |
| branch after the second merge | default | 9735 passed |
| branch after the third merge | lean | green |
| branch after the third merge | default | `goto_definition_…` failed; 9742 passed |
| `experiments` `56b3eb525`, 3 runs | lean | 3 of 3 green |
| `experiments` `56b3eb525`, 3 runs | default | 2 of 3 green; `rename_edit_does_not_land_on_another_callee_…` failed once |
| Plan B2 branch `0bbc6c874` (2026-11-12), full gate | lean | `lsp::client::tests::goto_definition_follows_a_write_made_outside_codescout` failed (`src/lsp/client.rs:4262`, empty result) |
| Plan B2 branch, same test run alone on that tree | — | passed (3.9 s) |
| Plan B2 branch `723f5a4d7`, `2c66232d4`, `74c13818b`, full gate each | lean and default | all green (3 of 3 runs) |

The branch failed in 2 of 6 lane runs and `experiments` in 1 of 6. This sample size cannot tell the rates apart. The branch does not touch `src/lsp/client.rs`.

Plan B2 adds a third sample: 1 failing full gate in 4 (the lean lane of `0bbc6c874`), and that test passed when run alone on the same tree. The B2 branch changes no file under `src/lsp/`. This does not prove the test is flaky, and it does not find the cause. It is the same test family as the earlier rows.

## Hypotheses tried

1. **Hypothesis:** The branch causes the failures. **Test:** Run the full lanes three times each on a clean checkout of `experiments`. **Verdict:** rejected. The clean tree failed once in six lane runs.
2. **Hypothesis:** The failures come from load, because the tests pass alone. **Test:** commit `b398b53d2` (session `5f715d8f`, 2026-11-12) ran hover, goto_definition and prepare_call_hierarchy under one busy process per core: 4 of 20 iterations failed with an EMPTY answer within 1.5 s of the write, 0 of 20 with the CPU idle. **Verdict:** supported for those three tests. Their asks now retry errors and empty answers for up to 20 s (`answered_after_a_write`), and 20 of 20 loaded iterations pass. The retry does not hide the defect: with the resync in `request` disabled they still fail after about 24 s.
3. **Hypothesis:** the rename test fails the same way under load. **Test (2026-11-12, session `5566e8a8`, tree with `b398b53d2`):** the four `rename_edit_(for|does_not_land)_…` tests, 20 iterations under 64 busy processes (one per core). **Verdict:** not reproduced. 18 iterations compiled and passed; 2 iterations (2 and 3) failed to COMPILE because of another session's uncommitted edit in the shared checkout (`MakeWriter` bound on `Arc<Mutex<Vec<u8>>>`), so they say nothing about the tests.
4. **Hypothesis:** the rename test fails only when the whole suite runs. **Test:** `cargo nextest run --no-default-features --no-fail-fast`, 8 iterations, same tree, no extra load. **Verdict:** not reproduced. 8 of 8 green.
5. **Hypothesis:** the rename test fails when many rust-analyzer instances start at once AND the CPU is oversubscribed, a combination attempts 3 and 4 did not have. **Test (2026-11-12, session `5f715d8f`, tree at `e610be3a2`):** `cargo nextest run --no-fail-fast --stress-count 12 -E 'test(/lsp::client::tests/)'`, so 71 tests per iteration in parallel, with `burn.py` (one busy process per core, 64 cores) running beside it; `/proc/loadavg` read after the run: 82.97 / 61.01 / 34.61, 66 busy processes. **Verdict:** not reproduced. 12 of 12 iterations passed (852 test runs, the rename test among them 12 times; in the last iteration it took 6.4 s, the only per-test times I read). The whole workspace suite (9,700 tests) was not run under load in this attempt; item 6 ran it.

6. **Hypothesis:** the rename test fails in the whole suite on an ordinary busy host, without an extra load job. **Test (2026-11-12, session `5f715d8f`, tree at `d516da4cb`):** `scripts/with-slot.sh cargo nextest run --workspace --no-fail-fast --stress-count 15`, 9,776 tests per iteration. **Verdict:** reproduced. The rename test failed in 2 of 15 iterations (8 and 14) with the panic above. Run 1 also had four `server::exit_rows_tests` failures in iteration 13 (a different bug, `6767d5eaafa320c7`).
7. **Hypothesis:** the rename helper retrying the `null` answer removes the flake without hiding a stale view. **Test:** (a) with the resync removed in `request` and in `document_symbols` in a scratch copy, run from the mutant binary, all 4 rename tests fail at the check that compares edit ranges with the file on disk (4 of 4, one run); (b) on the fixed code, after `cargo clean -p codescout` inside the lease, 20 iterations of the whole default lane, 9,776 tests each. **Verdict:** supported, not proven. 20 of 20 iterations had no rename failure; one iteration had a failure in `server::guide_hint_tests::native_codex_root_is_recorded_for_a_refused_pin`, a different test (`6767d5eaafa320c7`). At the earlier rate of 2 in 15 the chance of 20 clean iterations is about 6%. One earlier 20-iteration run was discarded: it executed my mutant binary because a scratch build shared the slot with the real tree (bug-fix-session-log F-238), so it says nothing about the fix.

## Fix

The rename test is fixed in `4603d77cf` (session `5f715d8f`): the helper, renamed `rename_retrying_while_the_server_is_busy`, retries, for up to 20 s, on a cancelled request (as before) and on the `null` answer (`invalid type: null`), and on no other error. For `goto_definition_follows_…`, `hover_follows_…` and `prepare_call_hierarchy_follows_…`, `b398b53d2` made the post-write asks retry empty answers.

The product is not changed: `LspClient::rename` still turns a legal `null` answer into an opaque error, and `edit_code rename` passes it on. That is `9391e97eeb937e90`, the owner's decision.

## Fix provenance

- **SHA:** `4603d77cf` (`experiments`): the rename helper retries the null answer
- **patch-id:** `cfc846f7d876`
- **SHA:** `b398b53d2` (`experiments`): the post-write asks retry empty answers
- **patch-id:** `20afb87a74c8`

## Tests added

None new. The helper `rename_retrying_while_the_server_is_busy` in `src/lsp/client.rs` changed, and the four `rename_edit_*` tests use it; the mutation and stress results are under Hypotheses tried, items 7 and 6.

## Workarounds

Rerun the gate. A failure in only these tests, with a pass on the rerun, is this bug.

## Resume

Mitigated on 2026-11-12 by `4603d77cf`: 20 of 20 full-suite iterations without a rename failure, after 2 in 15 before. That is evidence the fix works, not a proof: the chance of 20 clean iterations at the old rate is about 6%.

**Reopen trigger:** any failure of an `rename_edit_*`, `*_follows_a_write_made_outside_codescout`, `references_into_another_file_*` or `document_symbols_reflects_*` test in a gate run after `4603d77cf`. Keep the log of that run and read the panic text first: `rename_after_an_external_write` asserts three different things, and each points at a different cause (the rename must edit `lib.rs`; `other.rs` must be edited for a prepend; every `other.rs` range must read `target_fn` on disk). A failure on the third is a stale-copy defect in `resync_all_drifted`, not a flake.

Open, none blocking: why rust-analyzer answers `null` (`9391e97eeb937e90` holds the product side); the `usage.db` row tests that failed in the same stress runs (`6767d5eaafa320c7`); the process friction of the discarded run (bug-fix-session-log F-238).


Scope of the mitigation (added 2026-11-14): `4603d77cf` fixes the rename helper only. The peer failure recorded in `aadfcca23` is a goto-definition test with an empty result, on a tree without `4603d77cf`. No run has stressed the goto-definition, references and `document_symbols` tests on the fixed code, so the 20 of 20 result says nothing about them. To measure it, run the full default lane with `--stress-count 20` inside one slot lease, with `cargo clean -p codescout` first (bug-fix-session-log F-238), and count failures per test name, not per iteration.

## References

- `docs/trackers/artifact-identity-session-log.md` F-13
- `docs/issues/2026-11-11-symbol-at-and-hover-answer-from-stale-lsp-content-after-an-external-edit.md`
- `src/lsp/client.rs`

- `9391e97eeb937e90` (`LspClient::rename` and a legal `null` answer)
- `6767d5eaafa320c7` (the `usage.db` row tests)
- `bug-fix-session-log:F-238`
