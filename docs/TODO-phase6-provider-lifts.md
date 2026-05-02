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

**Status:** ✅ DONE — `src/fs/mod.rs` exists.

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

**Status after Phase 7b (2026-05-02):** 🟡 Partial.

Test extraction is done — all large test blocks moved to `tests.rs` siblings. Remaining non-test files over 600 lines:

| File | Lines | Why large |
|------|-------|-----------|
| `output_buffer.rs` | 1312 | monolithic buffer subsystem |
| `onboarding.rs` | 1032 | large prompt assembly logic |
| `memory/mod.rs` | 906 | impl Tool body + helpers |
| `command_summary.rs` | 873 | command type detection + summarisers |
| `read_file.rs` | 869 | impl Tool body |
| `file_summary/file_summary.rs` | 772 | impl Tool body |
| `semantic/index.rs` | 728 | index pipeline |

These cannot be thinned further with mechanical splits — bulk is `impl Tool` bodies coupled to `ToolContext`. The structural fix requires the `ToolContext → &Agent + &dyn LspProvider` redesign (Phase 6's real shape).

**Recommendation:** close 6.4 as "won't do at this scale." Reopen if/when the `ToolContext` redesign is taken on.
## When to revisit

- A new tool/consumer lands outside `src/tools/symbol/` that wants one of the pure helpers above.
- Someone takes on the `ToolContext` → `&Agent + &dyn LspProvider` redesign (that's Phase 6's real shape; the current plan underestimated it).
- After Phase 7 (prompt-surface sync) — tool-file line counts may drop naturally.
