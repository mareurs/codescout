---
id: cd886c414f6751b4
kind: tracker
status: draft
title: Legibility Backlog
owners: []
tags:
- legibility
- dzo
topic: null
time_scope: null
---

## Backlog (auto-managed)

Ranked by the legibility engine — **Tier 1** = biting-now (structural defect + observed `usage.db` friction); **Tier 2** = latent (structural only). Scanned 2026-06-15 · **17 open**. Re-run `librarian(action="legibility_scan")` to reconcile — refactored targets auto-close with a before→after delta. (`—` in tokens/lines = a non-body defect, e.g. a name collision.) The Dzo's verdicts are below.

| key | tier | defects | score | tok/budget | lines | tr/ed/se |
|---|:--:|---|--:|--:|--:|:--:|
| `tests/librarian/timemachine_smoke.rs::timemachine_full_chain` | 2 | over_budget_body | 0 | 4438/2500 | 499 | 0/0/0 |
| `src/tools/symbol/tests.rs::(file)` | 2 | un_mappable_file | 0 | 4332/2500 | 7198 | 0/0/0 |
| `src/tools/memory/mod.rs::Memory/call` | 2 | over_budget_body | 0 | 4301/2500 | 355 | 0/0/0 |
| `src/tools/symbol/list_overview.rs::list_overview` | 2 | over_budget_body | 0 | 4207/2500 | 416 | 0/0/0 |
| `src/tools/semantic/index.rs::IndexProject/call` | 2 | over_budget_body | 0 | 3662/2500 | 311 | 0/0/0 |
| `tests/e2e/edit_eval/cases.rs::all` | 2 | over_budget_body | 0 | 3506/2500 | 320 | 0/0/0 |
| `src/librarian/tools/tracker_design.rs::archetype_goal` | 2 | over_budget_body | 0 | 3440/2500 | 92 | 0/0/0 |
| `src/tools/edit_file/mod.rs::EditFile/call` | 2 | over_budget_body | 0 | 3275/2500 | 280 | 0/0/0 |
| `src/tools/edit_file/tests.rs::(file)` | 2 | un_mappable_file | 0 | 3267/2500 | 5110 | 0/0/0 |
| `src/prompts/builders.rs::build_system_prompt_draft` | 2 | over_budget_body | 0 | 3135/2500 | 270 | 0/0/0 |
| `src/librarian/tools/get.rs::call` | 2 | over_budget_body | 0 | 3054/2500 | 329 | 0/0/0 |
| `src/tools/grep.rs::Grep/call` | 2 | over_budget_body | 0 | 2918/2500 | 266 | 0/0/0 |
| `src/tools/symbol/edit_code.rs::EditCode/do_rename` | 2 | over_budget_body | 0 | 2778/2500 | 279 | 0/0/0 |
| `src/tools/symbol/edit_code.rs::EditCode/do_replace` | 2 | over_budget_body | 0 | 2762/2500 | 239 | 0/0/0 |
| `src/tools/run_command/inner.rs::run_command_inner` | 2 | over_budget_body | 0 | 2714/2500 | 238 | 0/0/0 |
| `src/tools/run_command/tests.rs::(file)` | 2 | un_mappable_file | 0 | 2612/2500 | 3753 | 0/0/0 |
| `src/tools/markdown/edit_markdown.rs::EditMarkdown/call` | 2 | over_budget_body | 0 | 2593/2500 | 230 | 0/0/0 |


### Closed (refactored — before → after)

| key | defects cleared | before → after | closed |
|---|---|---|:--:|
| `src/lsp/manager.rs::LspManager/get_or_start` | over_budget_body, name_collision | 3036 → 2463 tok | 2026-06-13 |
| `src/ast/parser.rs::extract_rust_symbols` | over_budget_body | 2948 → 2100 tok | 2026-06-14 |
| `src/tools/symbol/symbols.rs::Symbols/call` | over_budget_body | 5789 → 1781 tok | 2026-06-15 |
| `src/tools/markdown/read_markdown.rs::ReadMarkdown/call` | over_budget_body | 4798 → 629 tok | 2026-06-15 |
| `src/tools/onboarding.rs::perform_full_onboarding` | over_budget_body | 3839 → 2147 tok | 2026-06-14 |
| `src/librarian/tools/augment.rs::ArtifactAugment/call` | over_budget_body | 3188 → 1828 tok | 2026-06-14 |
| `src/config/sensitive.rs::SensitiveString/fmt` | name_collision | structural | 2026-06-13 |
| `src/config/sensitive.rs::SensitiveString/from` | name_collision | structural | 2026-06-13 |
| `src/lsp/client.rs::LspClient/did_change` | name_collision | structural | 2026-06-13 |
| `src/lsp/client.rs::LspClient/document_symbols` | name_collision | structural | 2026-06-13 |
| `src/lsp/client.rs::LspClient/goto_definition` | name_collision | structural | 2026-06-13 |
| `src/lsp/client.rs::LspClient/hover` | name_collision | structural | 2026-06-13 |
| `src/lsp/client.rs::LspClient/incoming_calls` | name_collision | structural | 2026-06-13 |
| `src/lsp/client.rs::LspClient/outgoing_calls` | name_collision | structural | 2026-06-13 |
| `src/lsp/client.rs::LspClient/prepare_call_hierarchy` | name_collision | structural | 2026-06-13 |
| `src/lsp/client.rs::LspClient/references` | name_collision | structural | 2026-06-13 |
| `src/lsp/client.rs::LspClient/rename` | name_collision | structural | 2026-06-13 |
| `src/lsp/client.rs::LspClient/workspace_symbols` | name_collision | structural | 2026-06-13 |
| `src/lsp/manager.rs::LspManager/notify_file_changed` | name_collision | structural | 2026-06-13 |
| `src/lsp/manager.rs::LspManager/shutdown_all` | name_collision | structural | 2026-06-13 |
| `src/lsp/mux/process.rs::read_proc_memory` | name_collision | structural | 2026-06-13 |
| `src/util/fs.rs::RepoPath/from` | name_collision | structural | 2026-06-13 |
| `src/util/path_security.rs::DEFAULT_DENIED_EXACT` | name_collision | structural | 2026-06-13 |
| `tests/fixtures/nav-eval-rust/src/trait_dispatch.rs::Counter/next` | name_collision | structural | 2026-06-13 |
| `tests/fixtures/typescript-library/src/extensions/advanced.ts::BookMetadata` | name_collision | structural | 2026-06-13 |

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
