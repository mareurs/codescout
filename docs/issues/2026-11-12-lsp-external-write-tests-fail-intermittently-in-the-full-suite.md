---
id: '8117d5eac05e5723'
kind: bug
status: open
title: The LSP external-write tests fail intermittently in the full suite
owners:
- marius
tags:
- cluster/unclassified
- testing
opened: 2026-11-12
severity: medium
unverified: Mechanism not measured for the rename test; it did not fail in 26 runs (18 loaded, 8 full-suite lean). The goto_definition and hover result is the other session's measurement in b398b53d2, which I read but did not re-run on the old code. The original rate was 2 of 6 lane runs on the branch and 1 of 6 on experiments.
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

## Reproduction

Not reproducible on demand. The full lane fails in a minority of runs.

- Full lane: `cargo nextest run --no-default-features --no-fail-fast`, or the same command with `--workspace`.
- Alone, `-E 'test(/follows_a_write_made_outside_codescout/)'` passed 3 of 3 runs on the branch.

## Environment

Linux, rust-analyzer started by the tests, a repo with 28 linked worktrees under `.worktrees`. Measured on the branch `feat/artifact-identity-plan-b1-20261111` and on `experiments` at `56b3eb525`.

## Root cause

Unknown — see Hypotheses tried. Measured 2026-11-12: the failures are in the tests named above, the same step passes alone, and the tests belong to the peer fix for `docs/issues/2026-11-11-symbol-at-and-hover-answer-from-stale-lsp-content-after-an-external-edit.md`. Inferred, not measured: a race between the external write and the language server's view of the file.

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

The branch failed in 2 of 6 lane runs and `experiments` in 1 of 6. This sample size cannot tell the rates apart. The branch does not touch `src/lsp/client.rs`.

## Hypotheses tried

1. **Hypothesis:** The branch causes the failures. **Test:** Run the full lanes three times each on a clean checkout of `experiments`. **Verdict:** rejected. The clean tree failed once in six lane runs.
2. **Hypothesis:** The failures come from load, because the tests pass alone. **Test:** commit `b398b53d2` (session `5f715d8f`, 2026-11-12) ran hover, goto_definition and prepare_call_hierarchy under one busy process per core: 4 of 20 iterations failed with an EMPTY answer within 1.5 s of the write, 0 of 20 with the CPU idle. **Verdict:** supported for those three tests. Their asks now retry errors and empty answers for up to 20 s (`answered_after_a_write`), and 20 of 20 loaded iterations pass. The retry does not hide the defect: with the resync in `request` disabled they still fail after about 24 s.
3. **Hypothesis:** the rename test fails the same way under load. **Test (2026-11-12, session `5566e8a8`, tree with `b398b53d2`):** the four `rename_edit_(for|does_not_land)_…` tests, 20 iterations under 64 busy processes (one per core). **Verdict:** not reproduced. 18 iterations compiled and passed; 2 iterations (2 and 3) failed to COMPILE because of another session's uncommitted edit in the shared checkout (`MakeWriter` bound on `Arc<Mutex<Vec<u8>>>`), so they say nothing about the tests.
4. **Hypothesis:** the rename test fails only when the whole suite runs. **Test:** `cargo nextest run --no-default-features --no-fail-fast`, 8 iterations, same tree, no extra load. **Verdict:** not reproduced. 8 of 8 green.

## Fix

Not fixed for the rename test. For `goto_definition_follows_…`, `hover_follows_…` and `prepare_call_hierarchy_follows_…`, `b398b53d2` (another session) made the post-write asks retry empty answers; that commit is that session's, and its patch-id is not recorded here.

## Tests added

None. This bug is about existing tests.

## Workarounds

Rerun the gate. A failure in only these tests, with a pass on the rerun, is this bug.

## Resume

The failing rename test is `rename_edit_does_not_land_on_another_callee_after_a_same_size_write_outside_codescout` (`resync_other = false`). Its panic text was never captured and 26 runs did not produce it. Do not guess a fix. Next: when it fails in a gate run, keep the log of that run and read the panic text first. `rename_after_an_external_write` asserts three different things, and each points at a different cause: the rename must edit `lib.rs`; `other.rs` must be edited for a prepend; every `other.rs` range must read `target_fn` on disk. A failure on the first is an empty or partial answer, like the three fixed tests; a failure on the third is a stale-copy defect in `resync_all_drifted`. `rename_retrying_when_cancelled` retries only the cancelled error, not an empty edit.

## References

- `docs/trackers/artifact-identity-session-log.md` F-13
- `docs/issues/2026-11-11-symbol-at-and-hover-answer-from-stale-lsp-content-after-an-external-edit.md`
- `src/lsp/client.rs`
