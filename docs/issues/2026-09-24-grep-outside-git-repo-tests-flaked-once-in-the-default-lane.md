---
id: '3c5fa35d9d2754a7'
kind: bug
status: open
title: 'BUG: three tools::grep "outside a git repo" tests failed once in the gate''s default lane and passed on re-run'
tags:
- cluster/unclassified
opened: 2026-09-24
owner: marius
severity: low
---

# BUG: three `tools::grep` "outside a git repo" tests failed once in the gate's default lane and passed on re-run

## Summary

In one `./scripts/gate.sh` run, three `tools::grep` tests failed in the default lane. They had passed in the same run's lean lane minutes earlier, and they passed on an immediate re-run. The failing assertion says a `.gitignore` was honoured in a tempdir with no `.git` above it. The cause is unknown. It is recorded now so the next occurrence can be matched against it, instead of being re-read as a regression in whatever diff is under test.


### Re-checked 2026-10-08 — no reproduction in twenty targeted default-feature runs

Ran all three named tests together, twenty times, using `CODESCOUT_SKIP_ONNX_TESTS=1 scripts/with-slot.sh` and `cargo test --lib -- <three full test names>`. Read every repetition's result and all three named verdicts: **20 completed runs, each 3 passed and 0 failed**, final command exit 0. These are 60 successful test invocations in total, not 60 full-suite runs. All three tests also passed in today's full lean and full default-feature runs. This establishes no recurrence in those two full-suite executions, not a root cause or a fix; the gate's overall verdict is reported separately.

Re-read all three fixtures, `rooted_ctx`, `Grep::call`, `WalkAudit::git_ignore_in_effect`, and the two CWD-changing tests named by the record. The grep fixtures supply absolute tempdir paths. The warning predicate walks those absolute paths' ancestors. The current `ignore` dependency's repository discovery also walks the search path's canonical ancestors for `.git` or `.jj`. None of this establishes CWD mutation as the original cause; it remains an unproven candidate. A grouped rerun excludes most of the other tests and cannot settle a suite-wide race.

Keep open for diagnosis rather than claim fixed. On recurrence, capture each tempdir and its ancestor markers at the failure, the warning clause actually emitted, and concurrent CWD-changing tests. No production fix or regression test was added.


Gate fallback completed with `FMT=0 CLIPPY=0 LEAN=0 DEFAULT=101`. The default lane's failures were exclusively `codescout-embed`'s two real-weight ONNX tests, both reporting missing `CODESCOUT_TEST_ONNX_DIR`; the fallback had omitted gate.sh's `CODESCOUT_SKIP_ONNX_TESTS=1` default. A focused `CODESCOUT_SKIP_ONNX_TESTS=1 scripts/with-slot.sh cargo test -p codescout-embed --features local-embed` rerun exited 0. This is fixture setup, not recurrence of the grep flake, and the opt-out does not validate actual ONNX vectors.

### Implementation gate re-check 2026-10-08

The subsequent implementation gate completed with `PYTHON_LIGHT=0 FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0`. The outside-repository grep cases did not recur. No grep production change was made and this remains open; the root-cause hypotheses above are still unproven.

## Symptom (Effect)

Gate run 2026-09-24 22:25:50 local, `FMT=0 CLIPPY=0 LEAN=0 DEFAULT=101`, `5812 passed; 3 failed`:

```
test tools::grep::tests::a_gitignore_outside_a_git_repo_is_not_applied ... FAILED
test tools::grep::tests::include_hidden_suppresses_the_warning_even_on_a_zero ... FAILED
test tools::grep::tests::widening_outside_a_git_repo_stays_bare ... FAILED

panicked at src/tools/grep.rs:2328:9:
assertion `left == right` failed: no .git here, so the .gitignore must not be honoured:
{"file_groups":[],"total":0,"files":0, … "completeness_warning":"this zero describes what was searched, not the pattern. Hidden paths were not searched, including .gitignore at the search root. …"}
  left: 0
 right: 1
```

## Reproduction

Not yet reproducible. The three tests passed on an immediate re-run with the same target dir (`cargo test --lib -- <the three names>`).

## Environment

Linux; per-session gate target `~/.cache/codescout-gate/ebf651ec…`; `experiments`; about 10 sessions live on the checkout, one of them running `tests/gate-slot.sh` tempdir suites in the same period.

## Root cause

Unknown.

**Checked and not it:** measured 2026-09-24 right after the failure, `/tmp` held no `.git`, `.gitignore` or `.ignore`, and `git -C /tmp rev-parse` reported not a repository.

**Candidates, not verified:**
- Process-global state shared by parallel tests in the same binary: two tests call `std::env::set_current_dir` (`src/util/path_security.rs:2360`, `src/agent/mod.rs:3196`).
- A transient `.git`/`.gitignore` in a tempdir ancestor created by another process.

Neither has been shown to reach these tests: they search absolute tempdir paths, and `WalkBuilder::require_git` discovery was not traced.

## Evidence

The gate log above, plus the re-run (3 passed, 0 failed). The lean lane of the same gate run passed all three.

## Hypotheses tried

None beyond the `/tmp` check above.

## Fix

Not started.

## Tests added

N/A: not reproduced.

## Workarounds

Re-run the three tests before reading your own diff when a gate reds on them.

## Resume

On recurrence, capture in the same turn: whether the default lane was running tests that change the CWD at that moment (a `--test-threads=1` re-run that passes would point at parallelism), the full panic text of all three tests, and any `.git` in the tempdir's ancestor chain. The test at `src/tools/grep.rs:2328` is the one to instrument.

## References

- `docs/issues/archive/2026-08-27-grep-zero-is-silent-for-gitignored-paths-and-include-hidden-does-not-reach-them.md`: where two of the tests were written
- `docs/issues/archive/2026-08-07-grep-zero-match-silent-about-hidden-skip.md`: the third
