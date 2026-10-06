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

Ranked by the legibility engine — **Tier 1** = biting-now (structural defect + observed `usage.db` friction); **Tier 2** = latent (structural only). Scanned 2026-10-06 · **81 open**. Re-run `librarian(action="legibility_scan")` to reconcile — refactored targets auto-close with a before→after delta; a row whose detector no longer exists is **retired**, not closed. (`—` in tokens/lines = a non-body defect, e.g. an un-mappable file.) The Dzo's verdicts are below.

| key | tier | defects | score | tok/budget | lines | tr/ed/se |
|---|:--:|---|--:|--:|--:|:--:|
| `src/lsp/manager.rs::LspManager/get_or_start` | 1 | over_budget_body | 6 | 2840/2500 | 222 | 2/0/1 |
| `tests/librarian/timemachine_smoke.rs::timemachine_full_chain` | 1 | over_budget_body | 4 | 4723/2500 | 521 | 1/0/2 |
| `src/tools/symbol/tests.rs::(file)` | 2 | un_mappable_file | 0 | 5824/2500 | 10582 | 0/0/0 |
| `src/tools/memory/mod.rs::Memory/call` | 1 | over_budget_body | 4 | 5444/2500 | 446 | 1/0/1 |
| `src/tools/symbol/list_overview.rs::list_overview` | 1 | over_budget_body | 9 | 5917/2500 | 554 | 3/0/2 |
| `src/tools/semantic/index.rs::IndexProject/call` | 1 | over_budget_body | 14 | 5979/2500 | 480 | 4/1/1 |
| `tests/e2e/edit_eval/cases.rs::all` | 2 | over_budget_body | 0 | 3506/2500 | 320 | 0/0/0 |
| `src/librarian/tools/tracker_design.rs::archetype_goal` | 1 | over_budget_body | 3 | 3437/2500 | 92 | 1/0/1 |
| `src/tools/edit_file/mod.rs::EditFile/call` | 1 | over_budget_body | 3 | 2928/2500 | 245 | 1/0/1 |
| `src/tools/edit_file/tests.rs::(file)` | 1 | un_mappable_file | 1 | 4069/2500 | 6427 | 0/0/1 |
| `src/prompts/builders.rs::build_system_prompt_draft` | 2 | over_budget_body | 0 | 3159/2500 | 270 | 0/0/0 |
| `src/librarian/tools/get.rs::call` | 1 | over_budget_body | 36 | 6274/2500 | 579 | 12/0/9 |
| `src/tools/grep.rs::Grep/call` | 1 | over_budget_body | 15 | 6327/2500 | 516 | 5/0/4 |
| `src/tools/symbol/edit_code.rs::EditCode/do_rename` | 1 | over_budget_body | 3 | 3908/2500 | 351 | 0/0/2 |
| `src/tools/symbol/edit_code.rs::EditCode/do_replace` | 1 | over_budget_body | 3 | 4382/2500 | 335 | 0/0/2 |
| `src/tools/run_command/inner.rs::run_command_inner` | 1 | over_budget_body | 9 | 4513/2500 | 365 | 3/0/3 |
| `src/tools/run_command/tests.rs::(file)` | 1 | un_mappable_file | 7 | 5374/2500 | 8616 | 2/0/3 |
| `src/librarian/tools/doctor.rs::(file)` | 1 | un_mappable_file | 86 | 8711/2500 | 22255 | 20/0/23 |
| `src/librarian/tools/doctor.rs::run_fix` | 1 | over_budget_body | 86 | 4030/2500 | 303 | 20/0/23 |
| `src/librarian/tools/doctor.rs::tests/row_checks_scoped_by_project_table_driven` | 1 | over_budget_body | 86 | 3625/2500 | 280 | 20/0/23 |
| `src/server.rs::(file)` | 1 | un_mappable_file | 80 | 5751/2500 | 14837 | 17/0/23 |
| `tests/test_measure_join.py::(file)` | 1 | un_mappable_file | 52 | 3018/2500 | 2526 | 14/0/8 |
| `tests/test_measure_lessons.py::(file)` | 1 | un_mappable_file | 41 | 3496/2500 | 1992 | 12/0/2 |
| `src/librarian/tools/append_entry.rs::(file)` | 1 | un_mappable_file | 38 | 3563/2500 | 4756 | 9/0/10 |
| `src/librarian/tools/doctor.rs::call` | 1 | over_budget_body | 36 | 13478/2500 | 909 | 12/0/9 |
| `src/librarian/tools/link_scan/mod.rs::call` | 1 | over_budget_body | 36 | 9718/2500 | 706 | 12/0/9 |
| `src/librarian/tools/find.rs::call` | 1 | over_budget_body | 36 | 8164/2500 | 647 | 12/0/9 |
| `src/librarian/tools/mv.rs::call` | 1 | over_budget_body | 36 | 6625/2500 | 434 | 12/0/9 |
| `src/librarian/tools/reindex.rs::call` | 1 | over_budget_body | 36 | 6592/2500 | 511 | 12/0/9 |
| `src/librarian/tools/append_entry.rs::call` | 1 | over_budget_body | 36 | 5819/2500 | 392 | 12/0/9 |
| `src/librarian/tools/update.rs::call` | 1 | over_budget_body | 36 | 5613/2500 | 439 | 12/0/9 |
| `src/librarian/tools/context.rs::call` | 1 | over_budget_body | 36 | 4373/2500 | 443 | 12/0/9 |
| `src/librarian/tools/audit_log.rs::call` | 1 | over_budget_body | 36 | 3073/2500 | 234 | 12/0/9 |
| `src/tools/core/types.rs::Tool/call_content` | 1 | over_budget_body | 34 | 6040/2500 | 411 | 11/0/6 |
| `src/librarian/catalog/augmentation.rs::(file)` | 1 | un_mappable_file | 33 | 3106/2500 | 5688 | 8/0/9 |
| `src/usage/db.rs::normalize_err_family` | 1 | over_budget_body | 28 | 3848/2500 | 306 | 7/0/9 |
| `scripts/pre-commit-ledger-counts.py::main` | 1 | over_budget_body | 27 | 6415/2500 | 441 | 8/1/6 |
| `src/server.rs::CodeScoutServer/call_tool_inner` | 1 | over_budget_body | 27 | 3868/2500 | 288 | 9/0/5 |
| `scripts/run-artifact-bench.py::main` | 1 | over_budget_body | 27 | 3097/2500 | 210 | 8/1/6 |
| `src/main.rs::main` | 1 | over_budget_body | 27 | 3007/2500 | 286 | 8/1/6 |
| `scripts/file-provenance.py::main` | 1 | over_budget_body | 27 | 2656/2500 | 183 | 8/1/6 |
| `src/tools/read_file.rs::read_from_buffer` | 1 | over_budget_body | 25 | 2766/2500 | 233 | 7/0/6 |
| `src/tools/read_file.rs::(file)` | 1 | un_mappable_file | 25 | 2755/2500 | 5016 | 7/0/6 |
| `scripts/measure/observability.py::coverage` | 1 | over_budget_body | 24 | 3211/2500 | 231 | 8/0/2 |
| `src/util/path_security.rs::(file)` | 1 | un_mappable_file | 18 | 5807/2500 | 6292 | 4/0/4 |
| `src/librarian/catalog/augmentation.rs::append_entry` | 1 | over_budget_body | 15 | 3046/2500 | 260 | 5/0/4 |
| `src/usage/db.rs::open_db` | 1 | over_budget_body | 15 | 2687/2500 | 208 | 5/0/4 |
| `src/server.rs::run` | 1 | over_budget_body | 13 | 2637/2500 | 248 | 4/0/2 |
| `src/tools/config/mod.rs::ProjectStatus/call` | 1 | over_budget_body | 12 | 4300/2500 | 341 | 4/0/3 |
| `src/tools/core/tests.rs::(file)` | 1 | un_mappable_file | 11 | 3253/2500 | 3939 | 1/0/5 |
| `tests/test_measure_packet.py::(file)` | 1 | un_mappable_file | 10 | 3081/2500 | 1491 | 2/0/3 |
| `src/tools/output_buffer.rs::OutputBuffer/resolve_refs` | 1 | over_budget_body | 9 | 3136/2500 | 246 | 3/0/3 |
| `src/librarian/catalog/rekey.rs::rekey_prefix_rows` | 1 | over_budget_body | 7 | 2934/2500 | 256 | 1/0/2 |
| `tests/test_measure_run_sample.py::(file)` | 1 | un_mappable_file | 7 | 2768/2500 | 2378 | 2/0/3 |
| `src/librarian/tools/artifact.rs::Artifact/input_schema` | 1 | over_budget_body | 6 | 5922/2500 | 275 | 0/0/4 |
| `src/librarian/indexer.rs::index_repo_sync` | 1 | over_budget_body | 6 | 5320/2500 | 435 | 2/0/2 |
| `src/tools/semantic/semantic_search.rs::SemanticSearch/call` | 1 | over_budget_body | 6 | 3063/2500 | 244 | 2/0/1 |
| `src/server.rs::CodeScoutServer/from_parts_with_env` | 1 | over_budget_body | 6 | 2833/2500 | 219 | 2/0/1 |
| `src/librarian/tools/find.rs::build_hints` | 1 | over_budget_body | 6 | 2776/2500 | 265 | 2/0/2 |
| `src/server.rs::guide_hint_tests/a_p50_session_stays_under_the_committed_emission_byte_ceiling` | 1 | over_budget_body | 3 | 5475/2500 | 352 | 1/0/1 |
| `src/tools/semantic/index.rs::IndexStatus/call` | 1 | over_budget_body | 3 | 4507/2500 | 300 | 0/0/1 |
| `src/librarian/tools/context.rs::pack_entry_anchor` | 1 | over_budget_body | 3 | 3439/2500 | 292 | 1/0/1 |
| `src/librarian/tools/doctor.rs::scan_open_bug_cited_from_source` | 1 | over_budget_body | 3 | 3417/2500 | 289 | 1/0/1 |
| `src/tools/markdown/edit_markdown.rs::plan_section_edit` | 1 | over_budget_body | 3 | 3177/2500 | 257 | 1/0/1 |
| `src/tools/symbol/edit_code.rs::EditCode/do_insert` | 1 | over_budget_body | 3 | 3115/2500 | 229 | 0/0/2 |
| `src/librarian/tools/doctor.rs::scan_cited_prefix_with_no_definer` | 1 | over_budget_body | 3 | 3073/2500 | 241 | 1/0/1 |
| `src/server.rs::tests/provenance_probes_reference_only_real_tool_names` | 1 | over_budget_body | 3 | 2951/2500 | 229 | 1/0/1 |
| `src/librarian/catalog/mod.rs::apply_migrations_in_txn` | 1 | over_budget_body | 3 | 2771/2500 | 247 | 0/0/3 |
| `scripts/measure/observability.py::render_map` | 1 | over_budget_body | 3 | 2745/2500 | 271 | 1/0/1 |
| `src/librarian/filter.rs::compile_leaf` | 1 | over_budget_body | 3 | 2713/2500 | 242 | 1/0/1 |
| `src/tools/symbol/edit_code.rs::EditCode/do_remove` | 1 | over_budget_body | 3 | 2572/2500 | 209 | 0/0/2 |
| `src/tools/run_command/output.rs::handle_successful_output_with` | 1 | over_budget_body | 2 | 5681/2500 | 450 | 0/0/2 |
| `src/tools/config/mod.rs::build_activation_response` | 1 | over_budget_body | 2 | 3361/2500 | 293 | 0/0/2 |
| `src/tools/symbol/references.rs::References/call` | 1 | over_budget_body | 2 | 2740/2500 | 222 | 0/0/1 |
| `src/tools/edit_file/mod.rs::perform_edit` | 1 | over_budget_body | 1 | 3494/2500 | 274 | 0/0/1 |
| `src/tools/markdown/tests.rs::(file)` | 1 | un_mappable_file | 1 | 3141/2500 | 4239 | 0/0/1 |
| `src/librarian/catalog/audit/shard.rs::export` | 2 | over_budget_body | 0 | 3719/2500 | 272 | 0/0/0 |
| `src/retrieval/sync.rs::sync_worktree` | 2 | over_budget_body | 0 | 3097/2500 | 257 | 0/0/0 |
| `src/tools/file_summary/tests.rs::(file)` | 2 | un_mappable_file | 0 | 2808/2500 | 2602 | 0/0/0 |
| `src/tools/markdown/read_markdown.rs::read_markdown_single_heading` | 2 | over_budget_body | 0 | 2561/2500 | 212 | 0/0/0 |
| `src/librarian/tools/librarian.rs::Librarian/input_schema` | 2 | over_budget_body | 0 | 2512/2500 | 76 | 0/0/0 |


### Closed (refactored — before → after)

| key | defects cleared | before → after | closed |
|---|---|---|:--:|
| `src/ast/parser.rs::extract_rust_symbols` | over_budget_body | 2948 → 2100 tok | 2026-06-14 |
| `src/tools/symbol/symbols.rs::Symbols/call` | over_budget_body | 5789 → 1781 tok | 2026-06-15 |
| `src/tools/markdown/read_markdown.rs::ReadMarkdown/call` | over_budget_body | 4798 → 629 tok | 2026-06-15 |
| `src/tools/onboarding.rs::perform_full_onboarding` | over_budget_body | 3839 → 2147 tok | 2026-06-14 |
| `src/librarian/tools/augment.rs::ArtifactAugment/call` | over_budget_body | 3188 → 1828 tok | 2026-06-14 |
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


### Closed (target gone — renamed or deleted, no re-measure)

| key | defects | closed |
|---|---|:--:|
| `src/tools/markdown/edit_markdown.rs::EditMarkdown/call` | over_budget_body | 2026-10-06 |


### Closed (reason not recorded — pre-dates `closed_reason`; check the Verdicts before reading these as repairs)

| key | defects | closed |
|---|---|:--:|


### Retired (detector removed — nothing was repaired)

| key | defects | retired |
|---|---|:--:|
| `src/config/sensitive.rs::SensitiveString/fmt` | name_collision | 2026-06-13 |
| `src/config/sensitive.rs::SensitiveString/from` | name_collision | 2026-06-13 |
| `src/lsp/mux/process.rs::read_proc_memory` | name_collision | 2026-06-13 |
| `src/util/fs.rs::RepoPath/from` | name_collision | 2026-06-13 |
| `src/util/path_security.rs::DEFAULT_DENIED_EXACT` | name_collision | 2026-06-13 |
| `tests/fixtures/nav-eval-rust/src/trait_dispatch.rs::Counter/next` | name_collision | 2026-06-13 |
| `tests/fixtures/typescript-library/src/extensions/advanced.ts::BookMetadata` | name_collision | 2026-06-13 |

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


### src/tools/markdown/read_file.rs — ReadMarkdown/call ✅ CLOSED 2026-06-15
**Was:** Tier 2 (latent — `cost: {truncations:0, edit_fails:0, sessions:0}`) — over_budget_body, 446 ln / ~4798 tok. The primary markdown-reading tool; every `symbols(include_body)` on it buffered (~20 KB).
**Scout (W-10):** the **third distinct seam shape** of the campaign. Unlike ArtifactAugment (lock-held `!Send` → sync helpers) and Symbols (lock-free but genuinely *async* → async helpers), here only the path-resolution prelude awaits (`project_root_for`/`security_config_for`); the four read branches (multi-heading, single-heading, line-range, default-tiers) hold no lock and contain **zero `.await`** — the `section_coverage.lock()` blocks never cross an await. So 4 of 5 helpers are plain **sync `fn`**; only `resolve_markdown_source` is async. No Send-future concern.
**Move (`4d601b5d`, behavior-preserving; `cargo test` 2864 passed / 0 failed = baseline; clippy `--all-targets -D warnings` + fmt clean):** extracted `resolve_markdown_source` (async), `read_file_multi_heading`, `read_file_single_heading`, `read_file_line_range`, `read_file_default_tiers` (sync). `call` collapses to resolve → guard → params → validate → dispatch.
**Instrument delta:** `symbols(include_body)` **buffered (~20 KB) → returns WHOLE** (446→55 ln); re-scan auto-closed the row (open 18→17).
**Recon sub-miss (low):** first typed the threaded `resolved` param as `&Path`; the collaborators (`section_coverage::mark_seen`/`status`, `markdown_coverage`) take `&PathBuf`, so the first `cargo check` failed 5× E0308. Fixed in one cycle by threading `&PathBuf` (forwarded straight to those consumers, so `clippy::ptr_arg` stays quiet). Lesson: scout the *consumer* param types before choosing an extracted helper's signature.
**Human-cost:** positive — `call` reads as a clean orchestrator; the four read strategies are separable and individually testable. Comments preserved verbatim.
**Note (Principle 2):** Tier-2 latent — picked on token weight, not observed friction (tier 1 long drained).
**Confidence:** high.
