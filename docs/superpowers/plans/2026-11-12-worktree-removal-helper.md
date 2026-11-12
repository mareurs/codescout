---
id: '7703d4e6d4000f23'
kind: plan
status: draft
title: Worktree Removal Helper and Activation Fix Implementation Plan
owners:
- marius
tags:
- worktree
- tooling
---

# Worktree Removal Helper and Activation Fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `scripts/worktree-remove.sh`, which removes a linked worktree and ends with no registered worktree, no directory and no process using it; and make `workspace(action="activate")` work after the active project's directory is gone.

**Architecture:** The script scans `/proc` for holders of the worktree path, classifies each as stoppable or blocker, stops stoppable ones only with `--stop`, then runs a plain `git worktree remove` and verifies the result. The activation fix is a pass-through in `scope_root` (`src/tools/provenance_fence_context.rs`): a `workspace` write call on a root that no longer exists skips the fence.

**Tech Stack:** Bash (Linux `/proc`), Rust (tokio tests in `src/tools/provenance_fence_context_tests.rs` and `src/server/workspace_pin_tests.rs`).

**Spec:** `docs/superpowers/specs/2026-11-12-worktree-removal-helper-design.md` (id `9189852e75f80f08`). Bugs: `3dde1548c3e5c310`, `d291b97d9e69edef`.

## Global Constraints

- The script never uses `git worktree remove --force`, never uses `rm -rf`, never sends SIGKILL, and never deletes the branch.
- Without `--stop` the script signals nothing. With `--stop`, a blocker means exit 4 with nothing signalled.
- Default wait is 30 seconds (`--wait SECS`); at most three scan rounds; the mux is signalled first, with SIGTERM.
- Stoppable holders: `codescout mux`, `rust-analyzer`, `cargo`, `rustc`, `sccache`, and any executable under `<path>/target/`. Anything else is a blocker.
- Linux `/proc` only. Without `/proc` the script refuses and says why.
- Exit codes: 0 removed; 2 refused precondition (usage, unregistered path, main checkout, caller's cwd inside the path, no `/proc`); 3 holders listed (no `--stop`); 4 blocker present with `--stop`; 5 a stoppable holder survived; 6 git refused the removal; 7 the directory remains.
- No new `CLAUDE.md` text beyond its byte budget (`claude_md_stays_within_its_byte_budget`, `src/prompts/mod.rs`).
- Rust tests that build a deleted root are `#[cfg(unix)]`, as `a_refused_pin_does_not_recreate_a_deleted_default_root` is.

## Plan decisions (rulings against the spec)

- Stoppable matching is by the basename of `argv[0]` from `/proc/<pid>/cmdline`, never by `comm` (a pinned binary reports `comm=2.1.258`; `scripts/stale-servers.sh` records the same trap). `codescout` counts as a mux only when `argv[1]` is `mux`.
- `rust-analyzer*` (prefix) is stoppable, so `rust-analyzer-proc-macro-srv` does not turn into a blocker. This widens the spec's list by one helper and keeps its intent.
- A fixture `/proc` is reachable through the env var `WORKTREE_REMOVE_PROC` (default `/proc`), as `CS_PEERS_PROC` does in the peer skill, so classification and listing are tested with no live process population.
- A Rust wrapper test runs the shell suite so the gate carries it (`tests/hook_config.rs` already spawns `bash`).
- The pointer for discoverability goes in the `Git Workflow` paragraph of `CLAUDE.md` if the byte budget allows; otherwise in `docs/RELEASE.md` § Branch Strategy.

## Review Focus

1. A sibling directory with the same prefix (`wt` and `wt-other`): a process in `wt-other` must not count as a holder of `wt` (Task 2).
2. A path with a space, or given relative, with a trailing slash, or through a symlink: it must resolve to the registered worktree, quoted correctly (Task 2).
3. A holder that exits between the scan and the signal: this is not an error (Task 3).
4. The caller's own cwd inside the worktree: refuse with exit 2, never remove it (Task 2).
5. A write tool on a missing root other than `workspace`: its behavior does not change (Task 1 control test).

---

### Task 1: Activation passes the fence when the active root is gone

**Files:**
- Modify: `src/tools/provenance_fence_context.rs` (`scope_root`, L121-198)
- Test: `src/tools/provenance_fence_context_tests.rs` (unit), `src/server/workspace_pin_tests.rs` (dispatch)

**Interfaces:**
- Consumes: `scope_root(root: &Path, writing: bool, name: &str, input: &Value, meta: Option<&Map<String, Value>>, client: Option<&ClientIdentity>, future: F)`; `make_server() -> (TempDir, CodeScoutServer)` and `dispatch_pinned` (request-building model) in `workspace_pin_tests.rs`.
- Produces: nothing new. Behavior: `scope_root` runs `future` unfenced when `writing && name == "workspace"` and `root.try_exists()` is `Ok(false)`.

- [ ] **Step 1: Write the failing tests.**
  - Unit, in `provenance_fence_context_tests.rs`, `#[cfg(unix)] #[tokio::test]`: `a_workspace_write_on_a_missing_root_runs_unfenced`. Build `checkout()`, remember its path, drop the `TempDir` (the directory is gone), call `scope_root(&path, true, "workspace", &json!({"action":"activate"}), None, None, async { Ok(Vec::new()) })`, and `assert!(result.is_ok())`.
  - Control, same file: `a_non_workspace_write_on_a_missing_root_is_still_refused`. Same setup, `name = "create_file"`, `assert!(result.is_err())`.
  - Dispatch, in `workspace_pin_tests.rs`, `#[cfg(unix)] #[tokio::test]`: `activation_proceeds_after_the_default_root_was_deleted`. `let (home, server) = make_server().await;` `let target = tempdir().unwrap();` remove `home`'s canonical root with `remove_dir_all`; dispatch `workspace` with `{"action":"activate","path": <target path>}` through `server.call_tool_inner(...)` (build the request like `dispatch_pinned` L42-54, without a pin). Assert the result is not an error and `all_text` does not contain `No such file or directory`.
- [ ] **Step 2: Run them to see the failure.** Run: `scripts/with-slot.sh cargo test --lib -- a_workspace_write_on_a_missing_root_runs_unfenced a_non_workspace_write_on_a_missing_root_is_still_refused activation_proceeds_after_the_default_root_was_deleted`. Expected: the first and third fail with `No such file or directory (os error 2)`; the control passes. If the failure text or the failing line differs, stop and report to the controller: the seam is wrong.
- [ ] **Step 3: Implement the pass-through** at the top of the `#[cfg(unix)]` block in `scope_root`: when `writing && name == "workspace" && matches!(root.try_exists(), Ok(false))`, `return future.await;`. Add a comment that names bug `d291b97d9e69edef` and says why: nothing under a missing root can be fenced, and `activate` is the call every write refusal prescribes.
- [ ] **Step 4: Run the three tests again.** Expected: all pass.
- [ ] **Step 5: Mutation check.** Replace `name == "workspace"` by `true` in the new condition; `a_non_workspace_write_on_a_missing_root_is_still_refused` must fail. Restore it. Replace `Ok(false)` by `Ok(true)`; the unit and dispatch tests must fail. Restore it.
- [ ] **Step 6: Commit.** `git add src/tools/provenance_fence_context.rs src/tools/provenance_fence_context_tests.rs src/server/workspace_pin_tests.rs` then `git commit -m "fix(workspace): activation no longer fails when the active project's directory is gone (d291b97d9e69edef)"`.

### Task 2: The script finds and classifies holders

**Files:**
- Create: `scripts/worktree-remove.sh` (executable)
- Create: `tests/worktree-remove.sh` (executable; follows `tests/stale-servers.sh`: `PASS`/`FAIL` counters, non-zero exit on any failure, writes only to its own temp directory)

**Interfaces:**
- Produces, CLI: `scripts/worktree-remove.sh [--stop] [--wait SECS] <worktree path>`. Env: `WORKTREE_REMOVE_PROC` (default `/proc`).
- Produces, output: holder rows on stdout, one per line, `PID<TAB>KIND<TAB>CMDLINE`, `KIND` in `mux | rust-analyzer | cargo | rustc | sccache | build-output | other`, ordered mux first, then the other stoppable kinds, then `other`. The occupancy warning goes to stderr and contains the word `occupant`.
- Produces, functions: `resolve_worktree PATH` (prints the canonical registered path or returns 2), `kind_of_argv ARGV0 ARGV1 PATH` (prints a kind), `list_holders PATH` (prints rows).

- [ ] **Step 1: Write the failing suite cases** in `tests/worktree-remove.sh`. Each case builds a temp git repo with a linked worktree and a fixture `/proc` (`<pid>/cwd` symlink, `<pid>/cmdline` with NUL-separated arguments) and sets `WORKTREE_REMOVE_PROC`. Cases and asserted results:
  - `lists_every_holder_kind`: fixture processes `codescout mux --cwd <wt>`, `rust-analyzer`, `cargo check --manifest-path <wt>/Cargo.toml`, `<wt>/target/debug/build/x/build-script-build`, and `vim` with cwd in `<wt>`. Exit 3. Rows have kinds `mux`, `rust-analyzer`, `cargo`, `build-output`, `other`, in that order.
  - `a_codescout_server_is_not_a_mux`: `codescout start --cwd <wt>` is `other`, not `mux`.
  - `a_sibling_prefix_is_not_a_holder`: a process with cwd in `<wt>-other` produces no row for `<wt>`.
  - `a_path_with_a_space_resolves`: a worktree directory named `my wt` is listed and refused/accepted the same as a plain one.
  - `relative_trailing_slash_and_symlink_spellings_resolve`: the same worktree given as `./wt/`, and through a symlink, reaches the same holders.
  - `refuses_an_unregistered_path`, `refuses_the_main_checkout`, `refuses_when_the_callers_cwd_is_inside`: exit 2 each; the message names the reason.
  - `refuses_without_proc`: `WORKTREE_REMOVE_PROC` pointing at a missing directory gives exit 2 and a message containing `/proc`.
  - `stop_with_a_blocker_refuses_before_any_signal`: with `--stop`, fixture holders of kind `mux` and `other`: exit 4, and the blocker row is printed. (The real-process proof that nothing was signalled is in Task 3.)
- [ ] **Step 2: Run the suite.** Run: `bash tests/worktree-remove.sh`. Expected: every case fails because the script does not exist.
- [ ] **Step 3: Implement the script** up to listing: argument parsing, `resolve_worktree` against `git worktree list --porcelain` (canonicalize with `realpath`), the refusals, `list_holders` (cwd under the path, or an `argv` element that holds the path as a whole component: preceded by start or a non-path character, followed by `/` or end), `kind_of_argv` (basename of `argv[0]`; `codescout` is `mux` only with `argv[1]` equal to `mux`; `rust-analyzer*`, `cargo`, `rustc`, `sccache`; `argv[0]` under `<path>/target/` is `build-output`), the stderr warning, exit 3 when holders exist. Skip the script's own pid and `$PPID`. With no holders it prints `no holders under <path>` and exits 0; Task 3 replaces that line with the removal sequence. With `--stop` and a blocker it prints the blocker and exits 4.
- [ ] **Step 4: Run the suite.** Expected: all Task 2 cases pass.
- [ ] **Step 5: Mutation check.** (a) Make `kind_of_argv` return a stoppable kind for everything: `lists_every_holder_kind` and the blocker case must fail. (b) Replace the whole-component test by a substring test: `a_sibling_prefix_is_not_a_holder` must fail. (c) Match `comm` instead of `argv[0]`: the kind cases must fail. Restore each.
- [ ] **Step 6: Commit.** `git add scripts/worktree-remove.sh tests/worktree-remove.sh` then `git commit -m "feat(scripts): worktree-remove.sh lists and classifies the holders of a worktree (3dde1548c3e5c310)"`.

### Task 3: Stop, remove and verify; the gate runs the suite

**Files:**
- Modify: `scripts/worktree-remove.sh`, `tests/worktree-remove.sh`
- Create: `tests/worktree_remove_script.rs`

**Interfaces:**
- Consumes: Task 2's `list_holders`, `kind_of_argv`, exit codes.
- Produces: `stop_holders PATH WAIT` (SIGTERM the mux first, then the other stoppable holders; poll every 0.5 s up to `WAIT`; at most three scan rounds; returns 0 when none remain, 5 otherwise), and the removal sequence (steps 6 and 7 of the spec).

- [ ] **Step 1: Write the failing cases** in `tests/worktree-remove.sh`, using real processes started by the suite (`(cd "$wt" && exec -a rust-analyzer sleep 300) &`; the suite's `trap` kills any survivor):
  - `no_holders_removes_cleanly`: exit 0; `git worktree list` no longer names it; the directory is gone; the branch still exists.
  - `holders_without_stop_signal_nothing`: a live `rust-analyzer`-named process: exit 3 and the process is still alive (`kill -0`).
  - `stop_ends_a_stoppable_holder_and_removes`: exit 0; the process is gone; the directory is gone.
  - `stop_with_a_blocker_leaves_the_stoppable_alive`: a live `rust-analyzer` and a live plain `sleep` in the worktree: exit 4 and the `rust-analyzer`-named process is still alive.
  - `a_holder_that_ignores_sigterm_is_a_survivor`: a holder that ignores SIGTERM (`exec -a cargo bash -c 'trap "" TERM; while :; do sleep 1; done'`) with `--wait 2`: exit 5, the process is still alive afterwards (no SIGKILL), the worktree is not removed.
  - `a_holder_that_exits_before_the_signal_is_not_an_error`: the fixture lists a pid that no longer exists at signal time (start a short `sleep 1`, scan, wait): exit 0.
  - `untracked_files_make_git_refuse`: an untracked file in the worktree: exit 6, `git status --porcelain --ignored -uall` output printed, the directory still present, no force.
  - `a_leftover_directory_is_reported_not_deleted`: a stub that recreates a file after git deregisters (a `git` shim earlier in `PATH` that runs the real `git worktree remove` and then `mkdir -p "$wt/target"`): exit 7, the contents are printed, the directory is still present.
  - `a_detached_head_worktree_is_removed`: same as `no_holders_removes_cleanly` on a detached worktree.
- [ ] **Step 2: Run the suite.** Expected: the new cases fail.
- [ ] **Step 3: Implement** `stop_holders` and the removal sequence. Ignore `ESRCH` on a signal. After the last round, a stoppable holder still alive is a survivor (exit 5, list it, remove nothing). Run `git worktree remove "$path"` without `--force`; on refusal print `git -C "$path" status --porcelain --ignored -uall` and exit 6. Then `git worktree prune`, then check that `"$path"` is absent; if it exists, list its contents and exit 7. Never delete it.
- [ ] **Step 4: Run the suite.** Expected: all cases pass.
- [ ] **Step 5: Add the Rust wrapper** `tests/worktree_remove_script.rs`: `#[test] fn the_worktree_remove_suite_passes()` spawns `bash tests/worktree-remove.sh` from the repo root (derive the root as other integration tests do) and asserts a zero exit, printing the suite's stdout and stderr on failure. Run: `scripts/with-slot.sh cargo test --test worktree_remove_script`. Expected: pass.
- [ ] **Step 6: Mutation check.** (a) Skip the re-scan loop: `a_holder_that_exits_before_the_signal_is_not_an_error` or `stop_ends_a_stoppable_holder_and_removes` must fail. (b) Add `--force` to the git call: `untracked_files_make_git_refuse` must fail. (c) Replace the exit-7 branch by `rm -rf`: `a_leftover_directory_is_reported_not_deleted` must fail. Restore each.
- [ ] **Step 7: Commit.** `git add scripts/worktree-remove.sh tests/worktree-remove.sh tests/worktree_remove_script.rs` then `git commit -m "feat(scripts): worktree-remove.sh stops holders, removes and verifies (3dde1548c3e5c310)"`.

### Task 4: Pointer, convention page and the bug files

**Files:**
- Create: `docs/conventions/removing-a-worktree.md`
- Modify: `CLAUDE.md` (the `Git Workflow` paragraph) or `docs/RELEASE.md` § Branch Strategy
- Modify (through `doc(update)`): bugs `3dde1548c3e5c310` and `d291b97d9e69edef`

- [ ] **Step 1: Write `docs/conventions/removing-a-worktree.md`.** Content: the command, the exit codes, the occupancy rule (ask the occupant before `--stop`; `docs/conventions/reaching-a-peer-session.md` Step 4), and the cause (rust-analyzer's `cargo check` rewrites `target/` after git deregisters the worktree). Cite both bug ids and the script.
- [ ] **Step 2: Add the one-line pointer.** First try `CLAUDE.md` `Git Workflow`: add `removing a worktree → scripts/worktree-remove.sh` to the existing sentence. Run `scripts/with-slot.sh cargo test --lib claude_md_stays_within_its_byte_budget`. If it fails, revert that edit and add the pointer under `docs/RELEASE.md` § Branch Strategy instead.
- [ ] **Step 3: Update the bugs.** In `3dde1548c3e5c310`: `Fix` item 3 becomes applied (commit shas), `Workarounds` names the script, `Resume` lists what is left (fix 2 in the superpowers plugin; the unexplained `.codescout/embeddings` and plan-b `.codescout`). Set status `fixed` only if the operator agrees that fix 2 belongs to a separate plugin bug; otherwise leave it open. In `d291b97d9e69edef`: `Root cause` verified, `Fix` applied, `Tests added` names the three tests, status per the bug template's fixed vocabulary.
- [ ] **Step 4: Run the full gate once.** Run: `./scripts/gate.sh`. Expected: green, except the known load-sensitive test named in the known-issue note if it flakes; re-run the gate once and report both results.
- [ ] **Step 5: Commit.** `git add docs/conventions/removing-a-worktree.md CLAUDE.md` (or `docs/RELEASE.md`) and the two bug files, then `git commit -m "docs: removing a worktree convention; bugs 3dde1548c3e5c310 and d291b97d9e69edef updated"`.

## After the plan

Final whole-branch review on the most capable model; one fix wave; one full gate; merge into `experiments` with the shared-checkout rules (`docs/conventions/shared-checkout-commit-sequence.md`); then run `scripts/worktree-remove.sh` on the plan's own worktree as the first real use, and record the result in bug `3dde1548c3e5c310`.
