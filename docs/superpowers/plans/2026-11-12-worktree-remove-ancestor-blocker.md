# worktree-remove.sh: an ancestor of the script is a blocker — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** With `--stop`, `scripts/worktree-remove.sh` exits 4 and signals nothing when a holder of the worktree is an ancestor of the script (the parent, the grandparent, and so on), whatever that holder's kind. Bug `786b1826bab82b18`.

**Architecture:** One function builds the set of the script's ancestors. `list_holders` already builds that set inline for the `child` pass; the new function replaces that inline walk and also serves `blockers_of`, `send_term` and the collection loop of `stop_holders`, which today protect only `$PPID`.

**Tech Stack:** bash (`scripts/worktree-remove.sh`, Linux `/proc`), the bash suite `tests/worktree-remove.sh`, wrapped by `tests/worktree_remove_script.rs`.

**Spec:** `docs/superpowers/specs/2026-11-12-worktree-removal-helper-design.md`, Design step 3 (the controller amends its text after the merge: it says only the parent is a blocker; the rule becomes "any listed ancestor of the script"). Bug: `docs/issues/2026-11-12-worktree-remove-stop-signals-a-stoppable-holder-that-is-an-ancestor-of-the-script.md`.

## Global Constraints

- SIGTERM only. Never SIGKILL a holder. The script never signals its own pid, its subshells, its parent or, after this plan, any ancestor of itself.
- An `other` holder stays a blocker: with `--stop` the script prints it and exits 4 before any signal. A holder that is an ancestor of the script is a blocker too, whatever its kind.
- stdout rows keep the format `PID<TAB>KIND<TAB>CMDLINE`; the kinds and their order do not change. The new text goes to stderr only, in the blocker list.
- Linux `/proc` only. Tests point the scan at a fixture tree with `WORKTREE_REMOVE_PROC`: `<dir>/<pid>/cwd` (symlink), `<dir>/<pid>/cmdline` (NUL-separated), `<dir>/<pid>/status` (a `PPid:` line).
- The ancestor walk keeps the bounds of the `child` walk: at most 64 steps, a seen set, and it ends at pid 1 or 0, a repeated pid, a missing or unreadable `status`, a `PPid:` that is not a number or has 11 or more digits. Use `sed -n 's/^PPid:...'`, never `awk` (a `$` followed by a digit is rewritten by some harnesses): the existing `ppid_of` does that; use it.
- Agent-facing text (script header, `docs/conventions/removing-a-worktree.md`) states current behavior only: no "was", "formerly", "now", "no longer".
- Verification command: `bash tests/worktree-remove.sh` (exit 0, every case `ok`). The controller runs `./scripts/gate.sh` once at the end.

## Review Focus

1. A stoppable holder `cargo` G that is the grandparent of the script (the script's parent has `PPid: G`), with no other blocker: `--stop` exits 4 and prints no signal line. At distance 3 as well.
2. G plus an unrelated stoppable holder (`rustc`): exit 4, nothing signalled (G blocks the whole run). Without G, the unrelated `rustc` is still stopped (the existing stop cases stay green).
3. Without `--stop`: exit 3, the stdout rows are unchanged, G listed with its own kind.
4. The ancestry is unknown past a `status` that cannot be read: only the ancestors found so far count. This is a documented limit of the script, not a test.
5. The blocker line names the reason: `(an ancestor of this script)`; the parent keeps `(the parent of this script)`.

---

### Task 1: Ancestors of the script are blockers

**Files:**
- Modify: `scripts/worktree-remove.sh` (`list_holders`, `blockers_of`, `send_term`, `stop_holders`, a new function `ancestry_of`; the header sections `Output`, `WHY IT CLASSIFIES, AND WHY other IS A BLOCKER`, `HOW IT STOPS`)
- Modify: `docs/conventions/removing-a-worktree.md` (section "A blocker is not stoppable", and step 4 of "The order" if it needs the word)
- Test: `tests/worktree-remove.sh` (use `run_with_fake_parent` and its `PARENT_PPID` seam)

**Interfaces:**
- Produces: `ancestry_of START` sets the global `ANCESTRY` to a space-padded list of pids (for example `" 4100 4090 "`): START itself, then each ancestor found through `ppid_of`, under the bounds in Global Constraints. It returns 0. For an empty or non-numeric START it sets `ANCESTRY=" "`.
- Consumes: the existing `ppid_of PID` (sets `UP`, returns 1 when there is no usable parent).
- `list_holders` calls `ancestry_of "$parent"` where it builds the set today, and keeps building it lazily (only when an `other` row exists that needs it). The inline walk is removed: one walk, one function.
- `blockers_of ROWS` (runs in the main process): builds `ANCESTRY` lazily from `$PPID` on first use. For each row: kind `other` prints `  <pid>  <cmd>`; else pid equal to `$PPID` prints `  <pid>  <cmd>  (the parent of this script)`; else a pid in `ANCESTRY` prints `  <pid>  <cmd>  (an ancestor of this script)`.
- `send_term PID KIND` and the pid-collection loop of `stop_holders` skip a pid in `ANCESTRY` (build it lazily from `$PPID` the same way), in addition to `$$`, `$BASHPID` and `$PPID`.

- [ ] **Step 1: Write the failing tests** in `tests/worktree-remove.sh`, registered where the other `case_*` functions are called:
  - `case_a_stoppable_grandparent_of_the_script_is_a_blocker`: fixtures G = `cargo build` 5096001 (cwd `$WT`); the script's fake parent has `PPid: 5096001` (`PARENT_PPID=5096001`), cwd `$C`, so the parent is not a holder. `--stop`: assert exit 4; `$ERR` contains `5096001` and `(an ancestor of this script)`; `lacks "no signal line" "worktree-remove: signalled " "$ERR"`; `lacks "no failed-signal line" "worktree-remove: could not signal " "$ERR"`; `kinds` equal to `cargo`.
  - `case_a_stoppable_ancestor_three_levels_up_is_a_blocker`: the same with a non-holder `sh` P2 5096011 (cwd `$C`, `FAKE_PPID=5096012`) between: parent `PPid: 5096011`, P2's parent is G = `cargo build` 5096012 (cwd `$WT`). Assert the same as above.
  - `case_an_ancestor_blocks_the_whole_stop`: G as in the first case plus an unrelated `rustc` holder 5096021 (cwd `$WT`, no parent link). `--stop`: exit 4, no signal line, `kinds` equal to `cargo,rustc`.
  - `case_without_stop_an_ancestor_is_listed_unchanged`: the first case without `--stop`: exit 3, `kinds` equal to `cargo`, and the stdout row for G is exactly `5096001<TAB>cargo<TAB>cargo build`.
  - `case_a_holder_that_is_not_an_ancestor_is_still_stopped`: a control. G = `cargo` 5096031 (cwd `$WT`) that is NOT in the parent's chain (the parent has no `status`). `--stop --wait 1` with fixtures: the run is not refused. Assert exit 5 (a fixture pid cannot be signalled, so it stays listed), `$ERR` contains `worktree-remove: could not signal 5096031 cargo` (the script reached the signal step), and `lacks "(an ancestor of this script)" "$ERR"`. This is the pattern of `case_a_holder_that_exits_before_the_signal_is_not_an_error`.

- [ ] **Step 2: Run `bash tests/worktree-remove.sh`.** Expected: the first three cases fail (exit 5 instead of 4, or no `an ancestor` text); every older case passes.

- [ ] **Step 3: Implement `ancestry_of`, refactor `list_holders` to use it, and extend `blockers_of`, `send_term` and the `stop_holders` collection loop** as specified above. Update the header sections `Output`, `WHY IT CLASSIFIES, AND WHY other IS A BLOCKER` (an ancestor of the script is a blocker whatever its kind) and `HOW IT STOPS`, and the limit list in `WHAT THIS DOES NOT AND CANNOT DO` (the ancestry is known only as far as `status` can be read).

- [ ] **Step 4: Run `bash tests/worktree-remove.sh`.** Expected: exit 0, every case `ok`, no `sleep` or other process of the suite left (`pgrep -af "[s]leep 300"` shows none).

- [ ] **Step 5: Update `docs/conventions/removing-a-worktree.md`** (section "A blocker is not stoppable": any holder that is an ancestor of the script is a blocker, whatever its kind; the script prints it with `(an ancestor of this script)`; the ancestry is known as far as `/proc/<pid>/status` can be read). Then commit: `git add scripts/worktree-remove.sh tests/worktree-remove.sh docs/conventions/removing-a-worktree.md` and `git commit -m "fix(scripts): worktree-remove.sh --stop never signals an ancestor of the script; an ancestor holder is a blocker (786b1826bab82b18)"`.
