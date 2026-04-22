# Codescout Refactoring Plan — Phase 1b

> Continuation of `2026-04-22-codescout-refactoring-plan.md`.
> Picks up where Phase 1 left off after commit `a81ece2`.

**Status:** In progress on branch `refactoring`.

- Phase 1b.1 ✅ done — commit `29c0568` (path_helpers.rs extracted; mod.rs 6793 → 6455)
- Phase 1b.2 ✅ done — commit `8eb4d2e` (symbol_query.rs extracted; mod.rs 6455 → 5995)
- Phase 1b.3 ✅ done — commit `75149cd` (edit_helpers.rs extracted; mod.rs 5995 → 5528)
- Phase 1b.4 🟡 partial — commit `eb572d2` (3 floating format_list_symbols tests moved to display.rs)
  - Remaining: migrate 226 tests from two `mod tests {}` blocks (L143 & L2894) into per-helper files
- Phase 1b.5 ⏳ pending — blocked on 1b.4 (mod.rs ≤ 100 line exit condition requires test migration)

---

## Why this phase exists

Phase 1 met its architectural goal: every symbol tool now lives in its own
file under `src/tools/symbol/`. What it did **not** do is finish the cleanup
of `mod.rs`, which remains ~6800 lines:

- ~45 shared helper functions (lines 49–1345)
- ~5600 lines of tests that reach helpers via `use super::*;` inside
  `mod tests {}` blocks, plus three free-floating top-level `#[test]` fns
- A handful of `pub use` re-exports and submodule declarations

The original plan sized Phase 1.10 as a single atomic "move surviving shared
helpers → `helpers.rs`" step. In practice the helpers split cleanly along
three cohesion lines, and the test block is large enough that it must migrate
in its own sub-phases. Hence Phase 1b, budgeted at ~5 atomic commits instead
of one.

---

## Goals

End state:

- `src/tools/symbol/mod.rs` is < 100 lines: module declarations, re-exports,
  and nothing else.
- Shared helpers live in three sibling files grouped by cohesion:
  `path_helpers.rs`, `symbol_query.rs`, `edit_helpers.rs`.
- Each helper file owns the tests that exercise it.
- All 1751 tests still pass, clippy clean, `cargo fmt` applied.

Non-goals (still out of scope per the parent plan):

- Changing helper signatures or behavior.
- Introducing a shared helper trait or facade.
- Reorganizing prompt surfaces (that's parent-plan Phase 7).

---

## Entry conditions

1. Working tree on `refactoring` clean.
2. Phase 1 commits 1–9 merged and pushed (`a81ece2` or later HEAD).
3. `cargo fmt && cargo clippy -- -D warnings && cargo test --lib` green.

---

## Helper inventory (from mod.rs as of `a81ece2`)

Evidence for the split, with current line ranges.

### Group A — `path_helpers.rs` (path, LSP client, library resolution)

Cohesion: "given user input, resolve it to a filesystem path and an LSP
client, tagging whether it lives in the project or a library." Shared by
every tool.

| Symbol | Lines | Currently pub(super)? |
|---|---|---|
| `LspTimer` (struct + impl) | 49–65 | yes |
| `is_glob` | 68–70 | yes |
| `resolve_read_path` | 76–99 | yes |
| `resolve_write_path` | 102–109 | yes |
| `resolve_library_roots` | 115–160 | yes |
| `format_library_path` | 164–169 | yes |
| `classify_reference_path` | 173–189 | yes |
| `resolve_glob` | 194–242 | yes |
| `get_path_param` | 245–259 | yes |
| `require_path_param` | 263–275 | yes |
| `guard_not_markdown` | 279–291 | yes |
| `get_lsp_client` | 297–314 | yes |
| `uri_to_path` | 1206–1210 | yes |
| `path_in_excluded_dir` (+ nested `EXCLUDED`) | 1214–1237 | yes |
| `tag_external_path` | 1309–1345 | yes |

Estimated size: ~260 lines + ~300 lines of tests.

### Group B — `symbol_query.rs` (AST/LSP symbol lookup + JSON shaping)

Cohesion: "find, classify, validate, and serialize `SymbolInfo`." Read-path
only — no file mutation. Shared by `find_symbol`, `find_references`,
`list_symbols`, and the edit tools when they resolve a target.

| Symbol | Lines | Currently pub(super)? |
|---|---|---|
| `matches_kind_filter` | 319–332 | yes |
| `filter_variable_symbols` | 338–354 | yes |
| `collect_matching` | 357–390 | yes |
| `symbol_to_json` | 392–451 | yes |
| `validate_symbol_range` | 460–480 | yes |
| `validate_symbol_position` | 498–551 | private (only used from here) |
| `is_lead_in_line` | 558–580 | private |
| `find_ast_end_line_in` | 584–594 | private |
| `fetch_validated_symbol` (+ `MAX_RETRIES`) | 609–647 | yes |
| `count_symbols_by_name_path` | 652–660 | yes |
| `resolve_range_via_document_symbols` | 665–676 | yes |
| `find_matching_symbol` | 680–690 | private |
| `symbol_name_matches` | 1080–1095 | yes |
| `find_symbol_by_name_path` (test-only) | 1098–1111 | `#[cfg(test)]` |
| `find_unique_symbol_by_name_path` | 1152–1188 | yes |
| `collect_matching_symbols` | 1191–1200 | private (dead? verify) |

Estimated size: ~420 lines + ~2200 lines of tests.

**Yak note:** `collect_matching_symbols` at 1191 looks dead — no grep hits
outside the function definition and its own test. Confirm before moving; if
dead, delete instead of relocating.

### Group C — `edit_helpers.rs` (edit-range computation, writes, sweeps)

Cohesion: "turn a resolved `SymbolInfo` into a byte range, write the file,
and detect integrity violations." Shared by `insert_code`, `remove_symbol`,
`replace_symbol`, `rename_symbol`.

| Symbol | Lines | Currently pub(super)? |
|---|---|---|
| `editing_start_line` | 725–786 | yes |
| `editing_end_line` | 796–804 | yes |
| `clamp_range_to_parent` | 819–830 | yes |
| `collect_all_name_paths` (+ nested `walk`) | 836–848 | yes |
| `find_ast_name_path` | 855–869 | yes |
| `find_parent_symbol` | 1121–1139 | yes |
| `write_lines` | 1062–1072 | yes |
| `utf16_to_byte_offset` | 1245–1256 | private (used only by `apply_text_edits`) |
| `apply_text_edits` | 1258–1305 | yes |
| `TextualMatch` (struct) | ~940 | yes |
| `text_sweep` | 983–1057 | yes |

Estimated size: ~350 lines + ~1800 lines of tests.

### Floating items outside the three groups

- Three top-level `#[test]` fns at lines 1348–1405
  (`format_list_symbols_*_mode`). They reference `format_list_symbols` from
  `display.rs`. **Move to `display.rs`** — they're testing display output,
  not helpers.
- `#[cfg(test)] mod tests { ... }` at line 1408: split across the three new
  files as tests migrate with their helpers.
- `#[cfg(test)] mod tests { ... }` at line 4160: a second tests block (LSP
  integration tests). Inspect during Phase 1b.4 — may stay in mod.rs or
  move wholesale to a new `tests_integration.rs` if it's self-contained.

---

## Phases

Each sub-phase is one atomic commit. Run
`cargo fmt && cargo clippy -- -D warnings && cargo test --lib` before every
commit.

### Phase 1b.1 — Extract `path_helpers.rs` (helpers only, tests stay)

1. Create `src/tools/symbol/path_helpers.rs`.
2. Move Group A symbols + their imports. Keep `pub(super)` visibilities
   (same `super` scope works from a sibling module).
3. In `mod.rs`: add `mod path_helpers;` and a *transitional*
   `pub(super) use path_helpers::*;` so the existing `use super::*;` inside
   `mod tests {}` still resolves.
4. Sibling tool files (`hover.rs`, etc.) currently `use super::{…}` — those
   stay working because of the re-export. Do **not** change them in this
   sub-phase.
5. Verify green; commit.

Success: mod.rs shrinks by ~260 lines; tests unchanged in location.

**Yak watch:** the `tag_external_path` body lives at line 1309, far from
the other path helpers. Move it with Group A despite its position.

### Phase 1b.2 — Extract `symbol_query.rs`

Same shape as 1b.1 but for Group B. Verify `collect_matching_symbols` is
actually dead before moving; if so, delete and note in the commit.

**Lion watch:** resist the urge to split Group B into "matching" vs
"validation". Fifteen cohesive helpers is one module; ten-plus-five becomes
two modules that import each other.

### Phase 1b.3 — Extract `edit_helpers.rs`

Same shape as 1b.1 but for Group C.

**Yak watch:** `utf16_to_byte_offset` is an implementation detail of
`apply_text_edits`. Keep it private (module-scoped) inside `edit_helpers.rs`;
do not re-export from `mod.rs`.

### Phase 1b.4 — Migrate tests to their helper files

Tests currently reach helpers via `use super::*;` inside `mod tests {}`.
With 1b.1–1b.3 done, `super::*` still works thanks to the transitional
re-export in `mod.rs`, but the tests sit in the wrong file.

Move in chunks, one test-mod at a time. For each chunk:

1. Identify the tests that exclusively exercise one helper module.
2. Copy them to a `#[cfg(test)] mod tests { … }` block at the bottom of
   that helper file.
3. Remove them from `mod.rs`.
4. Run tests. Commit.

Estimated sub-commits: 3–5, one per helper file, plus any leftover.

**Yak watch:** the second tests block at line 4160 must be inspected on its
own — it may be integration-style and not cleanly attach to any single
helper file.

### Phase 1b.5 — Drop transitional re-exports; shrink `mod.rs`

Once helper tests live with their helpers:

1. Remove the `pub(super) use path_helpers::*;` etc. from `mod.rs`.
2. Update sibling tool files (`hover.rs`, …) to import from the new helper
   modules directly:
   `use super::path_helpers::{require_path_param, resolve_read_path, …};`
3. Any test that still lives in `mod.rs` must use fully-qualified paths.
4. Verify `mod.rs` < 100 lines. Commit.

Success: `mod.rs` is a module-declaration file and nothing more.

---

## Watch points (cross-phase)

**Yak:**
- Prefer `remove_symbol` over `edit_file` for moving helper bodies — it uses
  the LSP range so indentation and attributes move together.
- When `remove_symbol` leaves orphan blank lines, always read ±5 lines and
  collapse them. Rustfmt won't catch triple-blank runs.
- If a test fails after extraction, the cause is 99% a missing `use` in the
  destination file, not a behavior change. Check imports first.
- One sub-phase = one commit. Don't bundle 1b.1 + 1b.2 to "save time." A
  bisect window of 200 lines is worth keeping.

**Lion:**
- The three-file split is the structural commitment. Resist later impulses
  to introduce `SymbolUtil` traits or facade structs unifying them — the
  files are meant to be flat collections of free functions.
- Every `pub(super)` in the new files must stay `pub(super)`. Promoting
  any of them to `pub` would leak internal symbol-tool plumbing to the
  rest of the crate.
- If Phase 1b.5 reveals that a helper is used by **only one** tool,
  consider moving it into that tool's file instead of keeping it shared.
  Shared files should contain genuinely shared code.

---

## Exit conditions

- `src/tools/symbol/mod.rs` ≤ 100 lines.
- Three new helper files: `path_helpers.rs`, `symbol_query.rs`,
  `edit_helpers.rs`.
- All 1751 tests passing (or a documented new count if a dead helper is
  deleted).
- `cargo clippy --lib -- -D warnings` clean.
- `cargo fmt` applied.
- No new `pub` items in the `symbol` subtree (verify with
  `grep -rn '^pub fn' src/tools/symbol/`).

---

## What this plan does NOT cover

- **Tool-file internals.** `list_symbols.rs` has private helpers (e.g.
  `find_split_point`); those stay inside it.
- **Cross-tree refactors.** Parent plan Phases 2–8 cover `workflow.rs`,
  `file.rs`, `agent/mod.rs`, `server.rs`, providers, prompts, and docs.
- **Test restructuring beyond relocation.** If a test is flaky or slow,
  note it in `docs/TODO-tool-misbehaviors.md` — do not rewrite it during
  relocation.
