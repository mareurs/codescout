---
id: cd886c414f6751b4
kind: tracker
status: draft
title: Legibility Backlog
tags:
- codescout
- legibility
- dzo
expects_augmentation: docs/augmentations/docs-trackers-legibility-backlog.yaml
---

## Backlog (auto-managed)

Ranked by the legibility engine — **Tier 1** = biting-now (structural defect + observed `usage.db` friction); **Tier 2** = latent (structural only). Scanned 2026-08-28 · **47 open**. Re-run `librarian(action="legibility_scan")` to reconcile — refactored targets auto-close with a before→after delta. (`—` in tokens/lines = a non-body defect, e.g. a name collision.) The Dzo's verdicts are below.

| key | tier | defects | score | tok/budget | lines | tr/ed/se |
|---|:--:|---|--:|--:|--:|:--:|
| `src/librarian/tools/doctor.rs::call` | 1 | over_budget_body | 6 | 6284/2500 | 465 | 2/0/2 |
| `src/librarian/tools/doctor.rs::(file)` | 1 | un_mappable_file | 6 | 4855/2500 | 10426 | 2/0/2 |
| `src/prompts/builders.rs::build_system_prompt_draft` | 1 | over_budget_body | 6 | 3148/2500 | 270 | 2/0/1 |
| `src/librarian/tools/doctor.rs::run_fix` | 1 | over_budget_body | 6 | 3022/2500 | 232 | 2/0/2 |
| `src/tools/core/types.rs::Tool/call_content` | 1 | over_budget_body | 3 | 5825/2500 | 438 | 1/0/1 |
| `src/librarian/tools/artifact.rs::Artifact/input_schema` | 1 | over_budget_body | 3 | 3897/2500 | 178 | 1/0/1 |
| `src/librarian/catalog/augmentation.rs::allocate_entry_id` | 1 | over_budget_body | 3 | 3090/2500 | 249 | 1/0/1 |
| `src/retrieval/sync.rs::sync_worktree` | 1 | over_budget_body | 2 | 2874/2500 | 241 | 0/0/1 |
| `src/tools/config/mod.rs::ProjectStatus/call` | 1 | over_budget_body | 1 | 3370/2500 | 278 | 0/0/1 |
| `src/tools/config/mod.rs::build_activation_response` | 1 | over_budget_body | 1 | 2939/2500 | 266 | 0/0/1 |
| `src/librarian/tools/link_scan/mod.rs::call` | 2 | over_budget_body | 0 | 6476/2500 | 499 | 0/0/0 |
| `src/tools/grep.rs::Grep/call` | 2 | over_budget_body | 0 | 6042/2500 | 503 | 0/0/0 |
| `src/librarian/tools/find.rs::call` | 2 | over_budget_body | 0 | 5505/2500 | 478 | 0/0/0 |
| `src/tools/symbol/list_overview.rs::list_overview` | 2 | over_budget_body | 0 | 5480/2500 | 519 | 0/0/0 |
| `src/tools/semantic/index.rs::IndexProject/call` | 2 | over_budget_body | 0 | 5459/2500 | 441 | 0/0/0 |
| `src/librarian/tools/get.rs::call` | 2 | over_budget_body | 0 | 5031/2500 | 503 | 0/0/0 |
| `src/tools/symbol/tests.rs::(file)` | 2 | un_mappable_file | 0 | 4997/2500 | 8920 | 0/0/0 |
| `src/tools/memory/mod.rs::Memory/call` | 2 | over_budget_body | 0 | 4841/2500 | 398 | 0/0/0 |
| `src/util/path_security.rs::(file)` | 2 | un_mappable_file | 0 | 4544/2500 | 4634 | 0/0/0 |
| `src/tools/semantic/index.rs::IndexStatus/call` | 2 | over_budget_body | 0 | 4499/2500 | 301 | 0/0/0 |
| `tests/librarian/timemachine_smoke.rs::timemachine_full_chain` | 2 | over_budget_body | 0 | 4438/2500 | 499 | 0/0/0 |
| `src/librarian/tools/context.rs::call` | 2 | over_budget_body | 0 | 4243/2500 | 435 | 0/0/0 |
| `src/librarian/tools/update.rs::call` | 2 | over_budget_body | 0 | 4080/2500 | 336 | 0/0/0 |
| `src/tools/run_command/inner.rs::run_command_inner` | 2 | over_budget_body | 0 | 4028/2500 | 327 | 0/0/0 |
| `src/tools/symbol/edit_code.rs::EditCode/do_rename` | 2 | over_budget_body | 0 | 3908/2500 | 351 | 0/0/0 |
| `src/server.rs::(file)` | 2 | un_mappable_file | 0 | 3797/2500 | 7496 | 0/0/0 |
| `src/tools/symbol/edit_code.rs::EditCode/do_replace` | 2 | over_budget_body | 0 | 3775/2500 | 291 | 0/0/0 |
| `src/tools/edit_file/tests.rs::(file)` | 2 | un_mappable_file | 0 | 3772/2500 | 5835 | 0/0/0 |
| `src/usage/db.rs::normalize_err_family` | 2 | over_budget_body | 0 | 3623/2500 | 294 | 0/0/0 |
| `tests/e2e/edit_eval/cases.rs::all` | 2 | over_budget_body | 0 | 3506/2500 | 320 | 0/0/0 |
| `src/librarian/tools/append_entry.rs::call` | 2 | over_budget_body | 0 | 3442/2500 | 244 | 0/0/0 |
| `src/librarian/tools/tracker_design.rs::archetype_goal` | 2 | over_budget_body | 0 | 3440/2500 | 92 | 0/0/0 |
| `src/librarian/tools/context.rs::pack_entry_anchor` | 2 | over_budget_body | 0 | 3439/2500 | 292 | 0/0/0 |
| `src/tools/edit_file/mod.rs::perform_edit` | 2 | over_budget_body | 0 | 3246/2500 | 263 | 0/0/0 |
| `src/tools/run_command/output.rs::handle_successful_output` | 2 | over_budget_body | 0 | 3215/2500 | 279 | 0/0/0 |
| `src/librarian/tools/reindex.rs::call` | 2 | over_budget_body | 0 | 3159/2500 | 274 | 0/0/0 |
| `src/main.rs::main` | 2 | over_budget_body | 0 | 3091/2500 | 294 | 0/0/0 |
| `src/tools/run_command/tests.rs::(file)` | 2 | un_mappable_file | 0 | 3027/2500 | 4456 | 0/0/0 |
| `src/tools/edit_file/mod.rs::EditFile/call` | 2 | over_budget_body | 0 | 2943/2500 | 241 | 0/0/0 |
| `src/tools/semantic/semantic_search.rs::SemanticSearch/call` | 2 | over_budget_body | 0 | 2864/2500 | 235 | 0/0/0 |
| `src/lsp/manager.rs::LspManager/get_or_start` | 2 | over_budget_body | 0 | 2840/2500 | 222 | 0/0/0 |
| `src/tools/symbol/edit_code.rs::EditCode/do_insert` | 2 | over_budget_body | 0 | 2798/2500 | 208 | 0/0/0 |
| `src/librarian/indexer.rs::index_repo_sync` | 2 | over_budget_body | 0 | 2794/2500 | 278 | 0/0/0 |
| `src/librarian/tools/find.rs::build_hints` | 2 | over_budget_body | 0 | 2776/2500 | 265 | 0/0/0 |
| `src/server.rs::CodeScoutServer/call_tool_inner` | 2 | over_budget_body | 0 | 2673/2500 | 217 | 0/0/0 |
| `src/tools/markdown/tests.rs::(file)` | 2 | un_mappable_file | 0 | 2644/2500 | 3348 | 0/0/0 |
| `src/server.rs::run` | 2 | over_budget_body | 0 | 2637/2500 | 248 | 0/0/0 |


### Closed (refactored — before → after)

| key | defects cleared | before → after | closed |
|---|---|---|:--:|

---

## Verdicts (Dzo-owned)

**2026-06-13 — `name_collision` retired as a defect class.** (ADR `docs/adrs/2026-06-13-drop-name-collision-defect.md`, commit `919dbe5c`.) The 7 open `name_collision` rows that closed on this scan closed because the **detector was removed, not because the code was refactored** — their before→after deltas are not meaningful (they render as "structural"). The earlier `name_collision` closes (the `LspClient` cluster + the two `LspManager` forwarders) *were* genuine trait-impl relocations, but those moves are now known to have been unnecessary: `edit_code` resolves the qualified `impl Trait for Type/method` form (hint fixed in `c21ad73b`), so the collision never blocked it. The engine now emits only language-agnostic, AST-measurable defects (`over_budget_body`, `un_mappable_file`).

_Per-key triage goes here — classify code-class vs tool-class, name the move, note human-cost. One `### <key>` section per target the Dzo picks up._

### src/lsp/manager.rs — LspManager/get_or_start ✅ CLOSED 2026-06-13
**Was:** Tier 1, both defects — a 242-line / 3036-token body (1 observed truncation) AND a name_collision. The inherent `get_or_start` shared the `LspManager/get_or_start` name_path with an `LspProvider` trait forwarder in the same file, so `edit_code` hard-failed "matches 2 symbols" — the collision blocked the very refactor needed to shrink the over-budget body.
**Move (2 transformations, behavior-preserving, 39 tests green throughout):**
1. Relocate `impl LspProvider for LspManager` → new `src/lsp/manager_provider.rs` (`b946171d`). Clears the collision per-file (the detector is per-file because `edit_code`'s LSP `document_symbols` is per-file) and unblocks `edit_code`, while preserving the public API name `LspManager::get_or_start`. Renaming the inherent method was impossible — `edit_code(action=rename)` must first *resolve* the symbol, which is exactly what the collision blocks; the trait-impl block's distinct name_path is the only collision-free handle.
2. Extract the LRU-eviction phase → `evict_lru_if_at_capacity()` (`95ea8e0e`). Sheds 573 tok / 46 ln, crossing under the 2500 budget (3036 → 2463). The circuit-breaker and fast-path phases were left inline — YAGNI, the body is under budget and no truncation recurs.
**Outcome:** re-scan auto-closed the row; the move also swept up the `notify_file_changed` + `shutdown_all` collisions (same forwarder block) → 3 rows closed.
**Reusable template:** the identical fix clears the `LspClientOps` cluster (next verdict). One trait-impl relocation → N collisions cleared.

### src/lsp/client.rs — the LspClientOps collision cluster ✅ CLOSED 2026-06-13
**Was:** code-class (real `edit_code` ambiguity). Ten `LspClient` methods resolved to TWO symbols each — an inherent `impl LspClient` plus a trait `impl crate::lsp::ops::LspClientOps for LspClient` exposing the same names (verified: `LspClient/hover` at `client.rs:1155` and `:1498`). Any `edit_code(symbol="LspClient/<m>")` hard-failed "matches 2 symbols".
**Move (`2b35f2a1`, behavior-preserving, 22 lsp::client tests green):** applied the `get_or_start` template verbatim — confirmed pure-forwarder + all 10 inherent methods `pub`, then relocated `impl LspClientOps for LspClient` → new `src/lsp/client_ops.rs`. One move cleared all ten collisions and unblocked `edit_code` on every `LspClient` method; public API unchanged.
**Human-cost:** low — the template amortized the `get_or_start` reconnaissance to near-zero. The legibility win is navigational: every `LspClient` method is now uniquely `edit_code`-addressable by name.

### src/ast/parser.rs — extract_rust_symbols ✅ CLOSED 2026-06-14
**Was:** Tier 1 — over_budget_body, ~2948 tok / 252 ln (1 observed search friction). 13 `match child.kind()` arms each repeated the same ~10-line `SymbolInfo` position-field literal; `symbols(include_body)` truncated/buffered on every fetch.
**Fresh read (2026-06-14):** confirmed live — body buffered (~3100 tok), not stale (Self-Trap 4 cleared).
**Move (1 transformation, behavior-preserving, full lib suite 2742 identical to baseline):** extracted a shared `rust_symbol(child, file, name_path, name, kind, children)` constructor (`4f1f88cb`); each arm collapses to one `symbols.push(rust_symbol(...))`. Match dispatch + per-kind name/children logic unchanged; `impl_item` (method-merge) left as-is.
**Instrument delta:** `symbols(name=extract_rust_symbols, include_body=true)` → **truncated/buffered → returns WHOLE**. Token mass fell below the inline budget; formatted line count barely moved (252→211) — the budget was the trigger, not LoC (Heuristic 1).
**Human-cost:** negligible — the constructor reads naturally and the match is now pure dispatch.
**Ledger:** `legibility_scan` will auto-close the row on next reconcile; verdict recorded now.
**Confidence:** high.



### src/tools/onboarding.rs — perform_full_onboarding ✅ CLOSED 2026-06-14
**Was:** Tier 1 — over_budget_body, 393 ln / ~3839 tok (1 observed truncation). `symbols(include_body)` buffered (~16 KB) on every fetch — no clean retrieval path.
**Fresh read (2026-06-14):** confirmed live post-rebuild — body buffered, not stale (Self-Trap 4 cleared).
**Move (behavior-preserving; `cargo test` 2864 passed / 0 failed = baseline; clippy `--all-targets -D warnings` + fmt clean; commit `333d6281`):** extracted 7 cohesive phases into private module-level helpers — `detect_languages`, `list_top_level_entries`, `build_key_files`, `write_workspace_config_if_needed`, `probe_index_status`, `write_onboarding_memories`, `gather_per_project_protected`. Pure phase extraction; the parent is now a flat orchestration sequence. Existing free-fn idiom (`gather_project_context`, `build_system_prompt_draft`) matched.
**Instrument delta:** `symbols(include_body)` **buffered (10271 B / ~2568 tok after the first 6 cuts — still over) → returns WHOLE** after the 7th extraction. The *instrument* set the stopping point, not a line target: the 6-helper cut measured 2568 tok, so `gather_per_project_protected` was added to cross 2500 (Heuristic 1 — budget is the trigger). Re-scan auto-closed the row (open 22→20).
**Human-cost:** negligible/positive — named phases read as clean orchestration, no duplication. Note: `onboarding.rs` was a documented "won't-do-at-this-scale" outlier, but that blocker was *test-module* extraction (needs ToolContext), orthogonal to this body-helper extraction.
**Confidence:** high.



### src/librarian/tools/augment.rs — ArtifactAugment/call ✅ CLOSED 2026-06-15
**Was:** Tier 1 — over_budget_body, 284 ln / ~3484 tok. `symbols(include_body)` buffered (~14.5 KB) on every fetch.
**Scout (W-7):** the body is a lock-held `!Send` region — `ctx.catalog.lock()` is scoped in a bare block so the `parking_lot` guard drops before the async `event_create`. The onboarding async-phase template does NOT transfer; the seam is *sync* value-logic.
**Move (`ede1c07d`, behavior-preserving; `cargo test` 2864 passed / 0 failed = baseline incl. the 22 inline augment.rs tests; clippy `--all-targets -D warnings` + fmt clean):** extracted 3 sync helpers — `validate_merged_against_schema`; `process_goal_tracker_merge` (scope-growth guard + auto-close gate evidence, ~70 ln — the W-7 seam); `create_or_replace_augmentation` (the merge=false branch, locks internally). The lock-scope skeleton and the post-lock async `event_create` stay verbatim; no guard crosses an await.
**Instrument delta:** `symbols(include_body)` **buffered → returns WHOLE** (284→144 ln); re-scan auto-closed the row (open 20→19). The gate logic is now independently unit-testable.
**Human-cost:** positive — the merge branch reads as validate → gate → upsert; concurrency invariants preserved exactly. No duplication.
**Confidence:** high.


### src/tools/symbol/symbols.rs — Symbols/call ✅ CLOSED 2026-06-15
**Was:** Tier 2 (latent — `cost: {truncations:0, edit_fails:0, sessions:0}`; the three prior loops drained tier 1) — over_budget_body, 469 ln / ~5789 tok, the single heaviest body in the index and the most-called navigation tool. Every `symbols(include_body)` on it buffered (~24 KB).
**Scout (W-9):** the body holds NO lock across its awaits — the **complement** of ArtifactAugment (W-7/W-8), so helpers stay `async` (the W-8 sync-only constraint does NOT apply). The one real trap: the `name_ok` predicate closure is borrowed across the helpers' `.await` points, so `Box<dyn Fn + Send>` was widened to `+ Send + Sync` to keep the `&`-borrow `Send` (`Tool: Send + Sync` requires `call`'s future `Send`). Scout decided the async-vs-sync axis correctly.
**Move (`247be16f`, behavior-preserving; `cargo test` 2864 passed / 0 failed = baseline; clippy `--all-targets -D warnings` + fmt clean):** extracted the three search strategies + result assembly into four module-level helpers matching the file's free-fn idiom — `search_files_restricted` (A: path/glob documentSymbol), `search_project_symbols` (B: workspace/symbol + tree-sitter fallback), `search_library_symbols` (C: library-root walk), and sync `finalize_search_results` (by_file / cap / body-strip / focus / hoist). `call` collapses to prelude → dispatch → finalize.
**Instrument delta:** `symbols(include_body)` **buffered (~24 KB) → returns WHOLE** (469→164 ln); re-scan auto-closed the row (open 19→18). Each helper is independently under budget and uniquely `edit_code`-addressable by name.
**Human-cost:** positive — `call` reads as parse → pick-strategy → finalize; the three search lanes are separable and individually testable. No duplication; comments preserved verbatim.
**Note (Principle 2):** Tier-2 (latent, not biting) — picked on token weight + call-frequency, not observed friction, since loops 1–3 drained tier 1. Flagged honestly rather than dressed up as friction-driven.
**Confidence:** high.


### src/tools/markdown/read_markdown.rs — ReadMarkdown/call ✅ CLOSED 2026-06-15
**Was:** Tier 2 (latent — `cost: {truncations:0, edit_fails:0, sessions:0}`) — over_budget_body, 446 ln / ~4798 tok. The primary markdown-reading tool; every `symbols(include_body)` on it buffered (~20 KB).
**Scout (W-10):** the **third distinct seam shape** of the campaign. Unlike ArtifactAugment (lock-held `!Send` → sync helpers) and Symbols (lock-free but genuinely *async* → async helpers), here only the path-resolution prelude awaits (`project_root_for`/`security_config_for`); the four read branches (multi-heading, single-heading, line-range, default-tiers) hold no lock and contain **zero `.await`** — the `section_coverage.lock()` blocks never cross an await. So 4 of 5 helpers are plain **sync `fn`**; only `resolve_markdown_source` is async. No Send-future concern.
**Move (`4d601b5d`, behavior-preserving; `cargo test` 2864 passed / 0 failed = baseline; clippy `--all-targets -D warnings` + fmt clean):** extracted `resolve_markdown_source` (async), `read_markdown_multi_heading`, `read_markdown_single_heading`, `read_markdown_line_range`, `read_markdown_default_tiers` (sync). `call` collapses to resolve → guard → params → validate → dispatch.
**Instrument delta:** `symbols(include_body)` **buffered (~20 KB) → returns WHOLE** (446→55 ln); re-scan auto-closed the row (open 18→17).
**Recon sub-miss (low):** first typed the threaded `resolved` param as `&Path`; the collaborators (`section_coverage::mark_seen`/`status`, `markdown_coverage`) take `&PathBuf`, so the first `cargo check` failed 5× E0308. Fixed in one cycle by threading `&PathBuf` (forwarded straight to those consumers, so `clippy::ptr_arg` stays quiet). Lesson: scout the *consumer* param types before choosing an extracted helper's signature.
**Human-cost:** positive — `call` reads as a clean orchestrator; the four read strategies are separable and individually testable. Comments preserved verbatim.
**Note (Principle 2):** Tier-2 latent — picked on token weight, not observed friction (tier 1 long drained).
**Confidence:** high.

