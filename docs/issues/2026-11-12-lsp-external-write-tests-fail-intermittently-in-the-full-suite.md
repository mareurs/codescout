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
unverified: Mechanism not measured. The failure rate is 2 of 6 lane runs on the branch and 1 of 6 on experiments. The panic text of the rename_edit test was not captured.
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
2. **Hypothesis:** The failures come from load, because the tests pass alone. **Test:** None yet. **Verdict:** deferred.

## Fix

None yet.

## Tests added

None. This bug is about existing tests.

## Workarounds

Rerun the gate. A failure in only these tests, with a pass on the rerun, is this bug.

## Resume

Run the lean lane ten times on a clean `experiments` checkout and count failures per test. Then check whether `src/lsp/client.rs` writes the file and queries the language server without waiting for the server to process the change. Look at the helper that the external-write tests share.

## References

- `docs/trackers/artifact-identity-session-log.md` F-13
- `docs/issues/2026-11-11-symbol-at-and-hover-answer-from-stale-lsp-content-after-an-external-edit.md`
- `src/lsp/client.rs`
