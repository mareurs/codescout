# worktree-remove.sh: signal lines and the `child` kind — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `scripts/worktree-remove.sh --stop` prints each process it signals, and it stops the descendants of a stoppable holder (the C compiler chain of a build script) instead of refusing on them.

**Architecture:** Task 1 adds one stderr line per SIGTERM in `send_term`'s callers. Task 2 adds a seventh stoppable kind, `child`: after `list_holders` has classified every holder by `argv[0]`, an `other` holder whose ancestor chain (`PPid:` in `<PROC>/<pid>/status`) reaches a stoppable holder becomes `child`. `child` rides the existing stop loop unchanged, because the loop already signals every non-`other`, non-`mux` row.

**Tech Stack:** bash (`scripts/worktree-remove.sh`, Linux `/proc`), the bash suite `tests/worktree-remove.sh`, wrapped by `tests/worktree_remove_script.rs`.

**Spec:** `docs/superpowers/specs/2026-11-12-worktree-removal-helper-design.md`. This plan amends two of its decisions: the line on what `--stop` prints, and the rule on which holders are stoppable. The controller amends the spec text after the merge. Evidence for both changes: bug `3dde1548c3e5c310`, section Fix, "Second use".

## Global Constraints

- SIGTERM only. Never SIGKILL a holder. The script never signals its own pid, its subshells or its parent (`$$`, `$BASHPID`, `$PPID`).
- An `other` holder stays a blocker: with `--stop` the script prints it and exits 4 before any signal.
- The script's own parent is a blocker whatever its kind, and it is never the ancestor that turns another process into `child`.
- stdout rows keep the format `PID<TAB>KIND<TAB>CMDLINE`, ordered by kind rank, then pid. The new signal lines go to stderr only.
- KIND is one of `mux | rust-analyzer | cargo | rustc | sccache | build-output | child | other`. Rank of `child` is 7, rank of `other` becomes 8.
- Linux `/proc` only. Tests point the scan at a fixture tree with `WORKTREE_REMOVE_PROC`: `<dir>/<pid>/cwd` (symlink), `<dir>/<pid>/cmdline` (NUL-separated), and, new in this plan, `<dir>/<pid>/status` (a `PPid:` line).
- Verification command for each task: `bash tests/worktree-remove.sh` (exit 0, and every case prints `ok`). The controller runs `./scripts/gate.sh` once at the end.

## Review Focus

1. A long-lived process that is a descendant of a stoppable holder but unrelated to the build (for example the binary that `cargo run` started, with its cwd in the worktree) becomes `child` and `--stop` signals it. Expected: it is listed as `child` in the listing without `--stop` (exit 3, no signal), so the operator sees it before asking for `--stop`.
2. The ancestor walk meets a loop (`a` → `b` → `a`), a missing `status` file, a `PPid:` that is not a number, or a chain longer than 64 steps. Expected: the process stays `other`. The walk must end, never hang.
3. A sibling of the script that has the script's parent as its parent, where that parent is stoppable by kind (a `cargo` parent). Expected: the sibling stays `other`, because the parent is never an ancestor that makes a `child`.
4. A descendant whose ancestor exited between two scans is re-parented to pid 1 and is `other` in the rescan. Expected: it is never signalled for that reason; if it is still alive at the end the run exits 5 and lists it. Document this in the script header; no test is required.
5. A `--stop` run that refuses with exit 4, and a run without `--stop`, print no signal line at all.

---

### Task 1: One stderr line per signal

**Files:**
- Modify: `scripts/worktree-remove.sh` (`send_term` at L383, its two call sites in `stop_holders` at L404, the header comment sections `Output` and `HOW IT STOPS`)
- Modify: `docs/conventions/removing-a-worktree.md` (the `--stop` section: say that the script prints each signal)
- Test: `tests/worktree-remove.sh`

**Interfaces:**
- Produces: `send_term PID KIND` (two arguments). Prints to stderr, after it tries the signal, exactly one of:
  - `worktree-remove: signalled <pid> <kind> with SIGTERM` when `kill -TERM` succeeded;
  - `worktree-remove: could not signal <pid> <kind> (already gone, or not permitted)` when it failed.
  It prints nothing for `$$`, `$BASHPID` and `$PPID` (the existing guard, unchanged). It always returns 0.
- `stop_holders` must know the kind of each pid it signals: keep an associative array `kind_of[pid]` filled in the loop that reads `rows`.

- [ ] **Step 1: Write the failing tests** in `tests/worktree-remove.sh`, registered where the other `case_*` functions are called:
  - `case_stop_prints_each_signal_it_sends`: build on the fixture of `case_stop_signals_the_mux_before_the_other_holders` (a mux stub and a cargo stub, real processes, `run_real --stop --wait 5 "$WT"`). Assert exit 0, and that `$ERR` contains `signalled <pm> mux with SIGTERM` and `signalled <pc> cargo with SIGTERM`, and that the mux line comes before the cargo line (compare the byte offsets of the two lines in `$ERR`).
  - Extend `case_a_holder_that_exits_before_the_signal_is_not_an_error`: assert that `$ERR` contains `could not signal <pid>` for the pid that was gone.
  - Extend `case_stop_with_a_blocker_refuses_before_any_signal` and `case_holders_without_stop_signal_nothing`: assert `lacks "no signal line" "worktree-remove: signalled " "$ERR"` and `lacks "no failed-signal line" "worktree-remove: could not signal " "$ERR"`. The pattern keeps the `worktree-remove: ` prefix and the trailing space on purpose: the exit-4 refusal already says "nothing was signalled", so the bare word `signalled` would match it.

- [ ] **Step 2: Run `bash tests/worktree-remove.sh`.** Expected: the three assertions above fail, every other case passes.

- [ ] **Step 3: Implement `send_term PID KIND`** in `scripts/worktree-remove.sh` and pass the kind at both call sites. Update the header comment: the `stderr` line of `Output` names the signal lines, and `HOW IT STOPS` says the script prints one line per signal it sends.

- [ ] **Step 4: Run `bash tests/worktree-remove.sh`.** Expected: exit 0, every case `ok`.

- [ ] **Step 5: Update `docs/conventions/removing-a-worktree.md`**, then commit: `git add scripts/worktree-remove.sh tests/worktree-remove.sh docs/conventions/removing-a-worktree.md` and `git commit -m "feat(scripts): worktree-remove.sh --stop prints each signal it sends (3dde1548c3e5c310)"`.

---

### Task 2: The `child` kind

**Files:**
- Modify: `scripts/worktree-remove.sh` (`kind_rank` L230, `list_holders` L248, the header comment sections `Output`, `WHY IT CLASSIFIES`, `HOW A PROCESS IS MATCHED`, `WHAT THIS DOES NOT AND CANNOT DO`)
- Modify: `docs/conventions/removing-a-worktree.md` (the kind table)
- Test: `tests/worktree-remove.sh` (the `fake` helper gets an optional `FAKE_PPID`)

**Interfaces:**
- Consumes: Task 1's `send_term PID KIND` (the signal line then reads `signalled <pid> child with SIGTERM`).
- Produces:
  - `kind_rank child` prints `7`; every other unknown kind prints `8`.
  - `fake PID CWD ARGV...` with `FAKE_PPID=<n>` in the environment also writes `$PROCD/<pid>/status` containing `Name:\t<comm>` and `PPid:\t<n>`. Without `FAKE_PPID` it writes no `status` file.
  - In `list_holders`, after the loop that builds the rows: every row of kind `other` whose pid is not the script's parent becomes `child` when an ancestor of it is a holder of a stoppable kind (`mux`, `rust-analyzer`, `cargo`, `rustc`, `sccache`, `build-output`) other than the script's parent. The walk follows `PPid:` in `<PROC>/<pid>/status` upward; it ends at pid 1 or 0, at a repeated pid, at a missing or unreadable `status`, at a `PPid:` that is not a number, and after 64 steps. A walk that ends without finding such an ancestor leaves the row `other`. The walk visits non-holder ancestors too. The kind used for the test is the first-pass kind (by `argv[0]`), never a `child` result.

- [ ] **Step 1: Write the failing tests** in `tests/worktree-remove.sh`:
  - `case_a_descendant_of_a_stoppable_holder_is_a_child`: fixtures `cargo` 5092001 (cwd `$WT`), `sh -c build` 5092002 (cwd `$C`, so not a holder; `FAKE_PPID=5092001`), `cc -c x.c` 5092003 (cwd `$WT`, `FAKE_PPID=5092002`). Assert exit 3 and `kinds` equal to `cargo,child`.
  - `case_without_a_status_file_a_descendant_stays_other`: the same fixtures without `FAKE_PPID`. Assert `kinds` equal to `cargo,other`.
  - `case_an_ancestor_walk_that_cannot_finish_leaves_other`: four sub-cases, each in its own `new_case`: a loop (`a` → `b` → `a`), a `PPid:` that is `abc`, a chain of 70 `sh` ancestors above a `cargo`, a missing ancestor `status`. Each sub-case asserts exit 3 and that the process in question is listed with kind `other` (in the 70-step case `kinds` equals `cargo,other`, because the `cargo` is too far above to count). The run must return within the suite's time limit.
  - `case_the_script_parent_never_makes_a_child`: use `run_with_fake_parent` with `PARGV0=cargo` and a parent cwd inside `$WT` (the existing blocker case). Add a sibling fixture `vim` whose `PPid:` is the parent's pid (extend the helper so the sibling is written after `$$` is known). Assert exit 4 and `kinds` equal to `cargo,other`: the parent row keeps kind `cargo`, and the sibling stays `other`, not `child`.
  - `case_stop_ends_a_c_compile_child_and_removes`: real processes. A cargo stub (`spawn "$WT" cargo bash "$C/cargo-stub"`) whose script is `trap 'exit 0' TERM; ( cd "$WT"; exec -a cc sleep 300 ) & wait`. Wait until both exist. `run_real --stop --wait 5 "$WT"`. Assert exit 0, both processes gone, the directory gone, and `$ERR` contains `signalled <child pid> child with SIGTERM`.
  - `case_a_descendant_that_is_not_a_holder_is_not_listed`: the `sh` of the first case never appears in the rows (assert `pids` equals `5092001,5092003`).
  - Update `case_lists_every_holder_kind` only if its fixtures change; its expected kinds must still pass (no fixture there has a `status` file).

- [ ] **Step 2: Run `bash tests/worktree-remove.sh`.** Expected: the new cases fail (unknown kind order, `other` where `child` is expected, no signal line for the child), every older case passes.

- [ ] **Step 3: Implement `kind_rank` and the `child` pass in `list_holders`** as specified above. Read `status` with `sed -n 's/^PPid:[[:space:]]*//p'`, not `awk` (a `$` followed by a digit is rewritten by some harnesses). The walk is a bounded loop in bash; it must not fork per step on the real `/proc` more than the one `sed`. The stop loop needs no change; check by reading `stop_holders` that a `child` row is put in `other_pids` and is in `fresh`.

- [ ] **Step 4: Run `bash tests/worktree-remove.sh`.** Expected: exit 0, every case `ok`.

- [ ] **Step 5: Update the header comment and `docs/conventions/removing-a-worktree.md`.** The `Output` section lists `child`. `WHY IT CLASSIFIES` says what `child` is and that an `other` row still blocks `--stop`. `WHAT THIS DOES NOT AND CANNOT DO` gets two items: a long-lived process started by a stoppable holder (for example `cargo run`) is a `child` and is signalled, and a descendant re-parented to pid 1 after its ancestor exited is `other` in the rescan (never signalled for that reason; exit 5 if it survives). Then commit: `git add scripts/worktree-remove.sh tests/worktree-remove.sh docs/conventions/removing-a-worktree.md` and `git commit -m "feat(scripts): worktree-remove.sh treats the descendants of a stoppable holder as child, so a cold build's C compiler does not block --stop (3dde1548c3e5c310)"`.
