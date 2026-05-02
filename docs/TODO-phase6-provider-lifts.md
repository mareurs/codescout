# Phase 6 — Deferred Provider Lifts

**Parent plan:** `docs/plans/2026-04-22-codescout-refactoring-plan.md` (Phase 6)
**Status after 6.1 (2026-04-22):** 🟡 Partial — `src/symbol/` provider lifted; remaining lifts tracked here.

## What shipped in 6.1

Commit `f7ca520`:

- `src/tools/symbol/symbol_query.rs` → `src/symbol/query.rs`
- `src/tools/symbol/edit_helpers.rs`  → `src/symbol/edit.rs`
- New `src/symbol/mod.rs` provider.
- 9 call sites updated: `super::symbol_query::` → `crate::symbol::query::`, `super::edit_helpers::` → `crate::symbol::edit::`.

## What's still deferred

### 6.2 — `src/fs/` (path provider)

**Status:** ✅ Complete — shipped 2026-05-02. `src/fs/` created; `path_helpers.rs` deleted.

**Plan target:** `resolve_read_path`, `resolve_write_path`, `resolve_glob`, `resolve_library_roots`, `guard_not_markdown`, `get_lsp_client`, `LspTimer`, `get_path_param`, `require_path_param`, plus the pure helpers below.

**Blocker:** most of these take `&ToolContext`. Lifting to `src/fs/` would cross the tool-layer boundary. Needs an API redesign — take `&Agent` (+ `&dyn LspProvider` where LSP is involved) instead of `&ToolContext`. That's a refactor of its own, not a mechanical move.

**Easy wins (context-free, can be lifted any time):**

| Function | Current location | Notes |
|---|---|---|
| `is_glob` | `src/tools/symbol/path_helpers.rs:37` | pure str predicate |
| `format_library_path` | `src/tools/symbol/path_helpers.rs:133` | pure path fmt |
| `classify_reference_path` | `src/tools/symbol/path_helpers.rs:142` | pure path classify |
| `uri_to_path` | `src/tools/symbol/path_helpers.rs:289` | pure URI parse |
| `path_in_excluded_dir` | `src/tools/symbol/path_helpers.rs:297` | pure path predicate |

**Recommendation:** don't lift these 5 alone — too small to earn `src/fs/` on their own. Wait until a context-free caller from outside `src/tools/symbol/` appears, or until the ctx-coupled neighbours get redesigned — then move the whole cluster together.

### 6.3 — `src/text/` (text-handling provider)

**Plan target:** `utf16_to_byte_offset`, `apply_text_edits`, `is_lead_in_line`, `format_line_range`.

**Current state after 6.1:**

| Function | Current location | Visibility | Callers |
|---|---|---|---|
| `utf16_to_byte_offset` | `src/symbol/edit.rs:425` | private | only `apply_text_edits` (same file) |
| `apply_text_edits` | `src/symbol/edit.rs:441` | `pub` | `src/tools/symbol/rename_symbol.rs` only |
| `is_lead_in_line` | `src/symbol/query.rs:251` | `pub` | only `validate_symbol_position` (same file) |
| `format_line_range` | `src/tools/format.rs:6` | `pub(crate)` | `src/tools/symbol/display.rs` |

**Blocker:** three of the four are single-file / single-caller helpers. Lifting them to a new `src/text/` module would add indirection without consolidating anything (Lion heuristic: *helpers used by exactly one caller stay with that caller*).

**`format_line_range`** already lives at `src/tools/format.rs`, which is itself a tool-layer formatter — adequate for its current use.

**Recommendation:** don't create `src/text/` at this scale. Revisit if/when any of these gain a second caller outside their current module, or if an LSP-adjacent text-edit crate is extracted.

### 6.4 — Tool-file thinning

**Plan exit criterion:** `src/tools/*.rs` files are ≤ 100 lines each (typical case).

**Not measured yet.** Worth a pass after Phase 7 lands. Current larger tool files (spot-check):

- `src/tools/symbol/find_symbol.rs` (~500 lines incl. impl Tool body)
- `src/tools/symbol/rename_symbol.rs` (~265 lines)
- `src/tools/edit_file.rs` (bundled test mod — deferred from Phase 3)

Most of the remaining bulk is `impl Tool for X { async fn call(...) { ... } }` bodies, not helpers — can't be thinned further without lifting the ctx-coupled fs/LSP plumbing described in 6.2.

## When to revisit

- A new tool/consumer lands outside `src/tools/symbol/` that wants one of the pure helpers above.
- Someone takes on the `ToolContext` → `&Agent + &dyn LspProvider` redesign (that's Phase 6's real shape; the current plan underestimated it).
- After Phase 7 (prompt-surface sync) — tool-file line counts may drop naturally.
