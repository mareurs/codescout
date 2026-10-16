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


### Measured 2026-10-16 — a `.git` above the tempdir reproduces the exact failure

I ran the three tests with the pre-change binary and `TMPDIR` pointed at a directory under a throwaway `git init` repo. All three failed with `left: 0 right: 1` and the same assertion text as the 2026-09-24 log. The control run, with the normal `TMPDIR`, passed. So a `.git` in any ancestor of the tempdir is **sufficient** to cause this failure. That does not show it caused the 2026-09-24 failure.

**Ruled out:** no test sets `TMPDIR`. Only two comments mention it: `src/server.rs:14897` and `src/tools/run_command/tests.rs:1243`.

**Checked clean:** the `git init` sites in `src/agent/mod.rs` (`activate_populates_head_sha`, which uses `.current_dir(dir.path())`) and in `src/librarian/tools/mv.rs` (which uses `-C <tmp>`).

**Audited in a second pass, same day:** 24 Rust init sites in 14 files. The search pattern was `Repository::init`, `Repository::init_opts`, `.arg("init")`, `.args(["init"` and `["init"`. An earlier note said "about 34" sites. That figure came from a looser search, and I could not reproduce it. Every one of the 24 targets its own `tempfile::tempdir()`, or a child of one, through `-C <tmp>`, `.current_dir(<tmp>)` or a path joined onto a tempdir.
- `doctor.rs` joins `ROUTE_ROOT_NAMES` onto a tempdir. All six names are relative, so `Path::join` cannot replace the base.
- `retrieval/index_state.rs:393` calls `Repository::open(root)` first, which does not search parent directories. It falls back to `init(root)`, and `root` is a tempdir.
- `rekey.rs:1869` writes a `.git` file inside its own tempdir. That cannot reach a sibling tempdir.

The shell suites under `tests/` build their repos under `mktemp -d`. I read the two unscoped `git init -q .` calls (`tests/hooks-discrimination.sh:102` and `tests/count-with-members.sh:56`). Each follows `cd "$T" || exit 1` or `cd "$REPO" || exit 1`. `tests/gate-slot.sh:311` inits `"$WORK/repo"`. The search over shell and Python files returned 110 matches. I read 80 of them, so about 30 are not read.

**Result:** no Rust test in this repository can plant a `.git` in an ancestor of a grep tempdir.

**The ancestor chain depends on the launcher.** `TMPDIR` in the session that ran this audit was `/home/marius/.cache/cc-tmp`, not `/tmp`. Tempdirs made from that shell have the ancestors `~/.cache`, `~`, `/home` and `/`. None held a `.git` on 2026-10-16. A transient `.git` at any of them, or in `/tmp` when `TMPDIR` is unset, would reproduce the failure. That is a candidate, not a finding: nothing shows that one existed on 2026-09-24, and the 2026-09-24 gate's `TMPDIR` is unknown.

**Stale references corrected:** the `set_current_dir` lines cited under "Root cause" had moved. The sites are now `src/util/path_security.rs:2861` (test `write_within_cwd_allowed_even_outside_project_root`) and `src/agent/mod.rs:3317` (test `new_with_relative_path_canonicalizes_home_root`). A third site, `tests/e2e/eval_common/report.rs:144`, is in a separate test binary.

**Diagnostic change, not a fix.** Commit `1e67e8a3` (patch-id `07ed8ad7153c101ba089dd9a97e98a159895c191`, from `git show 1e67e8a3 | git patch-id --stable`) adds `tempdir_with_no_git_above()` to `src/tools/grep.rs`. The three tests use it. When an ancestor holds a `.git`, it panics and names that ancestor and the tempdir. Evidence: `tools::grep::tests` 52 passed, 0 failed. With `TMPDIR` under a throwaway repo, each guarded test now fails with the named-ancestor message. A mutation of the helper from `.exists()` to `.is_dir()` failed only the new helper test. The gate gave `FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0 FMT_STATE=passed`. This does not find the creator of the 2026-09-24 `.git`, and `WalkAudit::git_ignore_in_effect` is unchanged.

### Stress, ancestor watch and a `.jj` gap, 2026-10-16 (second pass) — still no recurrence

**Full-suite run with a private `TMPDIR`.** `TMPDIR=<fresh empty dir> CODESCOUT_SKIP_ONNX_TESTS=1 scripts/with-slot.sh cargo test --lib` (pre-change binary plus the guard commit `1e67e8a3`): `6848 passed; 17 failed`, 701 s. All 54 `tools::grep` tests passed. After the run the private `TMPDIR` held **no `.git` and no `.jj`**: this single execution of the whole lib suite, including the two `set_current_dir` tests (one of which sets the CWD to exactly that directory, the tempdir's parent), planted no marker in the tempdir's parent. The 17 failures are not grep tests (`peer::*`, `lsp::client::tests::*`, `librarian::tools::context::tests::*`); their cause was not investigated, and they ran with a non-default `TMPDIR`.

**Ancestor watch.** A 50 ms poll loop for 25 minutes, during the run above and with about ten peer sessions live, checked `.git` and `.jj` in `/tmp`, `/home`, `~`, `~/.cache`, `~/.cache/cc-tmp`, `~/.cache/cc-tmp/claude-1000` and the private `TMPDIR`. It logged nothing. A 50 ms poll would miss a marker that lives for less than 50 ms; three separate tests failed on 2026-09-24, which suggests a marker present for longer than that, though the timing of those three tests was not recorded.

**Loop with the CWD-changing tests.** 40 consecutive runs of `tools::grep` together with `new_with_relative_path_canonicalizes_home_root`, `write_within_cwd_allowed_even_outside_project_root` and `activate_populates_head_sha`: 40 times `59 passed; 0 failed`.

**Read at the bytes, `ignore` 0.4.25 `src/dir.rs`.** `add_parents` (lines 225-229) and `add_child_path` (266-267) set `has_git` when the directory holds `.git` (file or directory) **or `.jj`**. `matched_ignore` gates `.gitignore`, `.git/info/exclude` and the global ignore file on `has_git`, but not `.ignore` files: an `.ignore` in a tempdir ancestor would hide a file without any `.git` at all. Nobody has found one; `/tmp` held none on 2026-09-24 (measured then) and none held one today.

**One real gap found, fixed.** `WalkAudit::git_ignore_in_effect` and the guard helper looked for `.git` only. A `.jj` directory turns gitignore on in the walk (the new test `a_jj_workspace_applies_gitignore_so_the_widened_zero_names_it` measured `total: 0` with the file ignored), yet the completeness warning stayed silent and the guard would not have named a `.jj` ancestor. Both now share `WalkAudit::nearest_vcs_marker_dir`. Red first: both new tests failed, the second with the bare zero and no warning. This is not shown to be the cause of the 2026-09-24 flake (no `.jj` exists on this machine and `jj` is not installed); it only closes a place where the same symptom could hide.

**State of the root cause.** Still unknown. Narrowed: every `git init` / `.git` writer in the Rust tests targets its own tempdir (audited twice); a full run plants nothing in the tempdir's parent; the CWD-changing tests do not reproduce it in a loop. What remains is a `.git` (or `.jj` / `.ignore`) created by a process outside this test binary, present for the minutes the default lane ran. The 2026-09-24 `/tmp` check may also have looked at the wrong directory: sessions under the Claude harness have `TMPDIR=~/.cache/cc-tmp`, whose ancestors are `~/.cache`, `~`, `/home`, `/`, not `/tmp`.

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
- Process-global state shared by parallel tests in the same binary: two tests call `std::env::set_current_dir` (`src/util/path_security.rs:2861`, `src/agent/mod.rs:3317`; lines re-checked 2026-10-16).
- A transient `.git`/`.gitignore` in a tempdir ancestor created by another process.

Neither has been shown to reach these tests: they search absolute tempdir paths, and `WalkBuilder::require_git` discovery was not traced.

## Evidence

The gate log above, plus the re-run (3 passed, 0 failed). The lean lane of the same gate run passed all three.

## Hypotheses tried

- `/tmp` held a `.git` after the failure: no (measured 2026-09-24).
- A test sets `TMPDIR`: no (searched 2026-10-16).
- Two `git init` sites, `activate_populates_head_sha` and `mv.rs`: both scoped to their own tempdir, so not it.
- Recurrence in twenty targeted runs and two full gates (2026-10-08): none.

## Fix

Not started for the flake: the root cause is unknown, so the status stays `open`. Commit `1e67e8a3` only makes the next occurrence self-diagnosing; the second pass (2026-10-16) widened that diagnosis and the completeness warning's gate to `.jj` (`WalkAudit::nearest_vcs_marker_dir` in `src/tools/grep.rs`).

## Tests added

`an_ancestor_git_is_named_whether_it_is_a_directory_or_a_worktree_link_file` in `src/tools/grep.rs` (commit `1e67e8a3`). It pins the helper that names the ancestor `.git`, for both a directory and a worktree link file. It is not a regression test for the flake itself.

Second pass 2026-10-16: `an_ancestor_jj_marker_is_named_like_a_git_one` and `a_jj_workspace_applies_gitignore_so_the_widened_zero_names_it`, both red before `WalkAudit::nearest_vcs_marker_dir` and green after. They cover the `.jj` gap, not the flake.

## Workarounds

Re-run the three tests before reading your own diff when a gate reds on them.

## Resume

On recurrence, read the panic first. If it names an ancestor, the cause is an environment fault: find out what created that `.git`, and when. Also record `TMPDIR` for the gate run. If the old `left: 0 right: 1` appears instead, the premise held and the cause is something else, so also check whether a CWD-changing test ran at that moment (a `--test-threads=1` re-run that passes would point at parallelism).

The audit of this repository's own `git init` sites is done (see above). The cause, if it is an ancestor `.git`, is therefore not in the Rust tests: a full lib run with a private `TMPDIR` left it clean (2026-10-16). It may still be in the unread shell files or outside the repository. What is left without a recurrence:
- read the 30 unread shell and Python matches (the unscoped `git init` calls in `tests/*.sh` were read again 2026-10-16: each follows a `cd` into its own `mktemp -d`, or is `git -C <dir>`);
- look for a tool or a peer session that runs `git init` in `~`, `~/.cache`, `~/.cache/cc-tmp` or `/tmp`: a 25-minute poll found none, but the failure was one-off;
- on recurrence, record `TMPDIR` and `ls -a` of every ancestor of it, `.ignore` included.

## References

- `docs/issues/archive/2026-08-27-grep-zero-is-silent-for-gitignored-paths-and-include-hidden-does-not-reach-them.md`: where two of the tests were written
- `docs/issues/archive/2026-08-07-grep-zero-match-silent-about-hidden-skip.md`: the third
