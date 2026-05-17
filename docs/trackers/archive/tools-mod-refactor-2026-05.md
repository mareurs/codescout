---
id: ccd1cda1b4135fff
kind: tracker
status: archived
title: src/tools — Phase 7 File Splits (B→C→A) — COMPLETE
owners: []
tags: []
topic: null
time_scope: null
---

Continuation of the codescout refactoring plan. Three splits, done in order of risk:
B (test extraction) → C (run_command concerns) → A (mod.rs foundation split).

Reference plan: `docs/plans/2026-04-22-codescout-refactoring-plan.md`

---

## Task B — `file_summary.rs`: extract tests

**File:** `src/tools/file_summary.rs` (1350 lines total, ~575 lines tests)

**Goal:** Move inline tests to `src/tools/file_summary/tests.rs` using directory module pattern.

**Steps:**
1. Create `src/tools/file_summary/` directory
2. Move `src/tools/file_summary.rs` → `src/tools/file_summary/file_summary.rs`
3. Create `src/tools/file_summary/mod.rs`:
   ```rust
   mod file_summary;
   pub use file_summary::*;
   #[cfg(test)]
   mod tests;
   ```
4. Extract `#[cfg(test)] mod tests { ... }` block → `src/tools/file_summary/tests.rs`
   - `tests.rs` needs: `use super::*;` at top (inherits everything via mod.rs re-export)
   - Check for any helpers marked `#[cfg(test)]` in file_summary.rs that tests call — make them `pub(crate)` if needed
5. Delete original `src/tools/file_summary.rs`
6. `cargo test -p codescout -- file_summary` → must be green

**Risk:** Low. Pure mechanical extraction. No visibility changes needed if all helpers are already `pub(crate)`.

**Status:** [x] complete — commit `1fc60c4`

---

## Task C — `run_command/mod.rs`: split by concern

**File:** `src/tools/run_command/mod.rs` (1196 lines)

**Three large functions to extract:**
- `run_command_inner` (~220 lines) — main orchestration loop
- `run_command_interactive` (~206 lines) — PTY/interactive mode
- `handle_successful_output` (~200 lines) — output formatting + buffer routing

**Goal split shape:**
```
src/tools/run_command/
  mod.rs          (RunCommand struct + Tool impl only, ~100 lines)
  inner.rs        (run_command_inner + its private helpers)
  interactive.rs  (run_command_interactive + PTY helpers)
  output.rs       (handle_successful_output + output formatting helpers)
  tests.rs        (already exists — no change needed)
```

**Steps:**
1. Survey cross-calls: `call_graph` on each of the 3 big functions to see what they call and who calls them
2. Extract `run_command_interactive` → `interactive.rs` first (most self-contained)
   - Promote any private helpers it uses to `pub(crate)` in their current file
   - Add `mod interactive;` + `use interactive::run_command_interactive;` in `mod.rs`
3. Extract `handle_successful_output` + output helpers → `output.rs`
   - Same visibility promotion pattern
4. Extract `run_command_inner` → `inner.rs` (calls the two above — do last)
5. `cargo test -p codescout -- run_command` → must be green

**Risk:** Medium. Cross-calls between the 3 functions mean order matters. Start with the leaf (most self-contained) first.

**Status:** [x] complete — commit `ba9fe16`

---

## Task A — `tools/mod.rs`: split into `core/`

**File:** `src/tools/mod.rs` (1487 lines)

**Three responsibilities to separate:**
1. **Types/trait:** `Tool` trait, `ToolContext`, `RecoverableError`, `Guidance` enum (~200 lines)
2. **Param helpers:** `require_str_param`, `optional_array_param`, `parse_bool_param`, `optional_u64_param`, etc. (~15 functions, ~300 lines)
3. **Guards:** `guard_worktree_write`, `strip_project_root`, `guard_read_only` etc. (~150 lines)
4. **Tests:** inline ~700 lines

**Goal split shape:**
```
src/tools/
  mod.rs          (thin re-export facade: pub use core::*; pub mod <all tool dirs>;)
  core/
    mod.rs        (re-exports: pub use types::*; pub use params::*; pub use guards::*;)
    types.rs      (Tool trait, ToolContext, RecoverableError, Guidance, OutputGuard)
    params.rs     (all ~15 param-parsing helpers)
    guards.rs     (guard_worktree_write, strip_project_root, guard_read_only, etc.)
    tests.rs      (all inline tests)
```

**Key constraint:** Every sibling tool file (symbol.rs, grep.rs, etc.) uses `crate::tools::X` — they already got fixed in Phase 6. No `super::` references to hunt down in siblings. Only `mod.rs` itself uses `self::` or bare unqualified paths.

**Steps:**
1. Survey `mod.rs` symbols: `symbols("src/tools/mod.rs")` to get full inventory before splitting
2. Categorize every symbol into types / params / guards / other
3. Extract `types.rs` first (most depended-upon — others import from it)
4. Extract `params.rs` (no deps on guards)
5. Extract `guards.rs` (may depend on types)
6. Extract tests → `tests.rs`; add `pub(crate)` to any private helpers tests use
7. Write thin `core/mod.rs` and thin outer `mod.rs` re-exports
8. `cargo test` full suite → must be green
9. `cargo clippy -- -D warnings` → clean

**Risk:** High foundation impact — but all callers already use `crate::tools::X` so the public API surface doesn't change. Risk is internal cross-deps between the three new files.

**Status:** [x] complete

---

## Completion Criteria

All three tasks done when:
- [x] `cargo fmt && cargo clippy -- -D warnings && cargo test` passes clean
- [x] No file in `src/tools/` exceeds ~600 lines
- [ ] Plan doc `docs/plans/2026-04-22-codescout-refactoring-plan.md` updated to reflect completion
