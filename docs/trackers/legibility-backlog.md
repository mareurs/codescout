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

Ranked by the legibility engine — **Tier 1** = biting-now (structural defect + observed `usage.db` friction); **Tier 2** = latent (structural only). Scanned 2026-06-13 @ `95ea8e0e` · **39 open, 3 closed**. Re-run `librarian(action="legibility_scan")` to reconcile — refactored targets auto-close with a before→after delta. (`—` in tokens/lines = a non-body defect, e.g. a name collision.)

| # | key | tier | defects | score | tok/budget | lines | tr/ed/se |
|--:|---|:--:|---|--:|--:|--:|:--:|
| 1 | `src/ast/parser.rs::extract_rust_symbols` | 1 | over_budget_body | 1 | 2948/2500 | 252 | 0/0/1 |
| 2 | `src/tools/symbol/symbols.rs::Symbols/call` | 2 | over_budget_body | 0 | 5789/2500 | 469 | 0/0/0 |
| 3 | `src/tools/markdown/read_markdown.rs::ReadMarkdown/call` | 2 | over_budget_body | 0 | 4798/2500 | 446 | 0/0/0 |
| 4 | `tests/librarian/timemachine_smoke.rs::timemachine_full_chain` | 2 | over_budget_body | 0 | 4438/2500 | 499 | 0/0/0 |
| 5 | `src/tools/symbol/tests.rs::(file)` | 2 | un_mappable_file | 0 | 4310/2500 | 7143 | 0/0/0 |
| 6 | `src/tools/memory/mod.rs::Memory/call` | 2 | over_budget_body | 0 | 4301/2500 | 355 | 0/0/0 |
| 7 | `src/tools/symbol/list_overview.rs::list_overview` | 2 | over_budget_body | 0 | 4207/2500 | 416 | 0/0/0 |
| 8 | `src/tools/onboarding.rs::perform_full_onboarding` | 2 | over_budget_body | 0 | 3839/2500 | 393 | 0/0/0 |
| 9 | `src/tools/semantic/index.rs::IndexProject/call` | 2 | over_budget_body | 0 | 3659/2500 | 312 | 0/0/0 |
| 10 | `tests/e2e/edit_eval/cases.rs::all` | 2 | over_budget_body | 0 | 3506/2500 | 320 | 0/0/0 |
| 11 | `src/librarian/tools/tracker_design.rs::archetype_goal` | 2 | over_budget_body | 0 | 3440/2500 | 92 | 0/0/0 |
| 12 | `src/tools/edit_file/mod.rs::EditFile/call` | 2 | over_budget_body | 0 | 3275/2500 | 280 | 0/0/0 |
| 13 | `src/tools/edit_file/tests.rs::(file)` | 2 | un_mappable_file | 0 | 3267/2500 | 5110 | 0/0/0 |
| 14 | `src/librarian/tools/augment.rs::ArtifactAugment/call` | 2 | over_budget_body | 0 | 3188/2500 | 263 | 0/0/0 |
| 15 | `src/prompts/builders.rs::build_system_prompt_draft` | 2 | over_budget_body | 0 | 3135/2500 | 270 | 0/0/0 |
| 16 | `src/librarian/tools/get.rs::call` | 2 | over_budget_body | 0 | 3054/2500 | 329 | 0/0/0 |
| 17 | `src/tools/grep.rs::Grep/call` | 2 | over_budget_body | 0 | 2918/2500 | 266 | 0/0/0 |
| 18 | `src/tools/symbol/edit_code.rs::EditCode/do_rename` | 2 | over_budget_body | 0 | 2778/2500 | 279 | 0/0/0 |
| 19 | `src/tools/symbol/edit_code.rs::EditCode/do_replace` | 2 | over_budget_body | 0 | 2762/2500 | 239 | 0/0/0 |
| 20 | `src/tools/run_command/inner.rs::run_command_inner` | 2 | over_budget_body | 0 | 2714/2500 | 238 | 0/0/0 |
| 21 | `src/tools/run_command/tests.rs::(file)` | 2 | un_mappable_file | 0 | 2612/2500 | 3753 | 0/0/0 |
| 22 | `src/tools/markdown/edit_markdown.rs::EditMarkdown/call` | 2 | over_budget_body | 0 | 2593/2500 | 230 | 0/0/0 |
| 23 | `src/config/sensitive.rs::SensitiveString/fmt` | 2 | name_collision | 0 | — | — | 0/0/0 |
| 24 | `src/config/sensitive.rs::SensitiveString/from` | 2 | name_collision | 0 | — | — | 0/0/0 |
| 25 | `src/lsp/client.rs::LspClient/did_change` | 2 | name_collision | 0 | — | — | 0/0/0 |
| 26 | `src/lsp/client.rs::LspClient/document_symbols` | 2 | name_collision | 0 | — | — | 0/0/0 |
| 27 | `src/lsp/client.rs::LspClient/goto_definition` | 2 | name_collision | 0 | — | — | 0/0/0 |
| 28 | `src/lsp/client.rs::LspClient/hover` | 2 | name_collision | 0 | — | — | 0/0/0 |
| 29 | `src/lsp/client.rs::LspClient/incoming_calls` | 2 | name_collision | 0 | — | — | 0/0/0 |
| 30 | `src/lsp/client.rs::LspClient/outgoing_calls` | 2 | name_collision | 0 | — | — | 0/0/0 |
| 31 | `src/lsp/client.rs::LspClient/prepare_call_hierarchy` | 2 | name_collision | 0 | — | — | 0/0/0 |
| 32 | `src/lsp/client.rs::LspClient/references` | 2 | name_collision | 0 | — | — | 0/0/0 |
| 33 | `src/lsp/client.rs::LspClient/rename` | 2 | name_collision | 0 | — | — | 0/0/0 |
| 34 | `src/lsp/client.rs::LspClient/workspace_symbols` | 2 | name_collision | 0 | — | — | 0/0/0 |
| 35 | `src/lsp/mux/process.rs::read_proc_memory` | 2 | name_collision | 0 | — | — | 0/0/0 |
| 36 | `src/util/fs.rs::RepoPath/from` | 2 | name_collision | 0 | — | — | 0/0/0 |
| 37 | `src/util/path_security.rs::DEFAULT_DENIED_EXACT` | 2 | name_collision | 0 | — | — | 0/0/0 |
| 38 | `tests/fixtures/nav-eval-rust/src/trait_dispatch.rs::Counter/next` | 2 | name_collision | 0 | — | — | 0/0/0 |
| 39 | `tests/fixtures/typescript-library/src/extensions/advanced.ts::BookMetadata` | 2 | name_collision | 0 | — | — | 0/0/0 |

> **Note:** table hand-rendered from `params` (re-render @ `95ea8e0e`). The `render_template` is attached but the write path does not yet auto-project it onto the body (2b follow-up). `params` is the source of truth; query it with `artifact(action="get", id="cd886c414f6751b4", entry_filter=...)`.

## Closed (auto-closed with before→after delta)

First full loop run 2026-06-13: logs picked the target → engine ranked it → refactor under green baseline → re-scan auto-closed it.

| key | defects cleared | before → after | closed |
|---|---|---|:--:|
| `src/lsp/manager.rs::LspManager/get_or_start` | over_budget_body + name_collision | **3036 tok / 242 ln → 2463 tok / 196 ln** | 2026-06-13 |
| `src/lsp/manager.rs::LspManager/notify_file_changed` | name_collision | structural (— → —) | 2026-06-13 |
| `src/lsp/manager.rs::LspManager/shutdown_all` | name_collision | structural (— → —) | 2026-06-13 |

---

## Verdicts (Dzo-owned)

_Per-key triage goes here — classify code-class vs tool-class, name the move, note human-cost. One `### <key>` section per target the Dzo picks up._

### src/lsp/manager.rs — LspManager/get_or_start ✅ CLOSED 2026-06-13
**Was:** Tier 1, both defects — a 242-line / 3036-token body (1 observed truncation) AND a name_collision. The inherent `get_or_start` shared the `LspManager/get_or_start` name_path with an `LspProvider` trait forwarder in the same file, so `edit_code` hard-failed "matches 2 symbols" — the collision blocked the very refactor needed to shrink the over-budget body.
**Move (2 transformations, behavior-preserving, 39 tests green throughout):**
1. Relocate `impl LspProvider for LspManager` → new `src/lsp/manager_provider.rs` (`b946171d`). Clears the collision per-file (the detector is per-file because `edit_code`'s LSP `document_symbols` is per-file) and unblocks `edit_code`, while preserving the public API name `LspManager::get_or_start`. Renaming the inherent method was impossible — `edit_code(action=rename)` must first *resolve* the symbol, which is exactly what the collision blocks; the trait-impl block's distinct name_path is the only collision-free handle.
2. Extract the LRU-eviction phase → `evict_lru_if_at_capacity()` (`95ea8e0e`). Sheds 573 tok / 46 ln, crossing under the 2500 budget (3036 → 2463). The circuit-breaker and fast-path phases were left inline — YAGNI, the body is under budget and no truncation recurs.
**Outcome:** re-scan auto-closed the row; the move also swept up the `notify_file_changed` + `shutdown_all` collisions (same forwarder block) → 3 rows closed.
**Reusable template:** the identical fix clears the `LspClientOps` cluster below — an inherent `impl LspClient` + `impl LspClientOps for LspClient` in one file. One trait-impl relocation → 10 collisions cleared.

### src/lsp/client.rs — the LspClientOps collision cluster (rows 25–34)
**Verdict:** code-class (real `edit_code` ambiguity). Ten `LspClient` methods resolve to TWO symbols each — an inherent `impl LspClient` plus a trait `impl crate::lsp::ops::LspClientOps for LspClient` exposing the same names (verified: `LspClient/hover` at `client.rs:1155` and `:1498`). Any `edit_code(symbol="LspClient/<m>")` hard-fails "matches 2 symbols".
**Move:** apply the `get_or_start` template — relocate `impl LspClientOps for LspClient` to `src/lsp/client_ops.rs`. Clears all ten collisions in one move and unblocks `edit_code` on every `LspClient` method, public API unchanged. (Prior verdict suggested collapsing the inherent impl into the trait; the relocation is lower-risk and proven.)
**Human-cost:** low — a known pattern, not a latent bug; now has a tested fix recipe.

