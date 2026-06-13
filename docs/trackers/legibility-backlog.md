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

Ranked by the legibility engine — **Tier 1** = biting-now (structural defect + observed `usage.db` friction); **Tier 2** = latent (structural only). Scanned 2026-06-13 @ `9bbfb922` · **42 open, 0 closed**. Re-run `librarian(action="legibility_scan")` to reconcile — refactored targets auto-close with a before→after delta. (`—` in tokens/lines = a non-body defect, e.g. a name collision.)

| # | key | tier | defects | score | tok/budget | lines | tr/ed/se |
|--:|---|:--:|---|--:|--:|--:|:--:|
| 1 | `src/lsp/manager.rs::LspManager/get_or_start` | 1 | over_budget_body, name_collision | 3 | 3036/2500 | 242 | 1/0/1 |
| 2 | `src/ast/parser.rs::extract_rust_symbols` | 1 | over_budget_body | 1 | 2948/2500 | 252 | 0/0/1 |
| 3 | `src/tools/symbol/symbols.rs::Symbols/call` | 2 | over_budget_body | 0 | 5789/2500 | 469 | 0/0/0 |
| 4 | `src/tools/markdown/read_markdown.rs::ReadMarkdown/call` | 2 | over_budget_body | 0 | 4798/2500 | 446 | 0/0/0 |
| 5 | `tests/librarian/timemachine_smoke.rs::timemachine_full_chain` | 2 | over_budget_body | 0 | 4438/2500 | 499 | 0/0/0 |
| 6 | `src/tools/symbol/tests.rs::(file)` | 2 | un_mappable_file | 0 | 4310/2500 | 7143 | 0/0/0 |
| 7 | `src/tools/memory/mod.rs::Memory/call` | 2 | over_budget_body | 0 | 4301/2500 | 355 | 0/0/0 |
| 8 | `src/tools/symbol/list_overview.rs::list_overview` | 2 | over_budget_body | 0 | 4207/2500 | 416 | 0/0/0 |
| 9 | `src/tools/onboarding.rs::perform_full_onboarding` | 2 | over_budget_body | 0 | 3839/2500 | 393 | 0/0/0 |
| 10 | `src/tools/semantic/index.rs::IndexProject/call` | 2 | over_budget_body | 0 | 3659/2500 | 312 | 0/0/0 |
| 11 | `tests/e2e/edit_eval/cases.rs::all` | 2 | over_budget_body | 0 | 3506/2500 | 320 | 0/0/0 |
| 12 | `src/librarian/tools/tracker_design.rs::archetype_goal` | 2 | over_budget_body | 0 | 3440/2500 | 92 | 0/0/0 |
| 13 | `src/tools/edit_file/mod.rs::EditFile/call` | 2 | over_budget_body | 0 | 3275/2500 | 280 | 0/0/0 |
| 14 | `src/tools/edit_file/tests.rs::(file)` | 2 | un_mappable_file | 0 | 3267/2500 | 5110 | 0/0/0 |
| 15 | `src/librarian/tools/augment.rs::ArtifactAugment/call` | 2 | over_budget_body | 0 | 3188/2500 | 263 | 0/0/0 |
| 16 | `src/prompts/builders.rs::build_system_prompt_draft` | 2 | over_budget_body | 0 | 3135/2500 | 270 | 0/0/0 |
| 17 | `src/librarian/tools/get.rs::call` | 2 | over_budget_body | 0 | 3054/2500 | 329 | 0/0/0 |
| 18 | `src/tools/grep.rs::Grep/call` | 2 | over_budget_body | 0 | 2918/2500 | 266 | 0/0/0 |
| 19 | `src/tools/symbol/edit_code.rs::EditCode/do_rename` | 2 | over_budget_body | 0 | 2778/2500 | 279 | 0/0/0 |
| 20 | `src/tools/symbol/edit_code.rs::EditCode/do_replace` | 2 | over_budget_body | 0 | 2762/2500 | 239 | 0/0/0 |
| 21 | `src/tools/run_command/inner.rs::run_command_inner` | 2 | over_budget_body | 0 | 2714/2500 | 238 | 0/0/0 |
| 22 | `src/tools/run_command/tests.rs::(file)` | 2 | un_mappable_file | 0 | 2612/2500 | 3753 | 0/0/0 |
| 23 | `src/tools/markdown/edit_markdown.rs::EditMarkdown/call` | 2 | over_budget_body | 0 | 2593/2500 | 230 | 0/0/0 |
| 24 | `src/config/sensitive.rs::SensitiveString/fmt` | 2 | name_collision | 0 | — | — | 0/0/0 |
| 25 | `src/config/sensitive.rs::SensitiveString/from` | 2 | name_collision | 0 | — | — | 0/0/0 |
| 26 | `src/lsp/client.rs::LspClient/did_change` | 2 | name_collision | 0 | — | — | 0/0/0 |
| 27 | `src/lsp/client.rs::LspClient/document_symbols` | 2 | name_collision | 0 | — | — | 0/0/0 |
| 28 | `src/lsp/client.rs::LspClient/goto_definition` | 2 | name_collision | 0 | — | — | 0/0/0 |
| 29 | `src/lsp/client.rs::LspClient/hover` | 2 | name_collision | 0 | — | — | 0/0/0 |
| 30 | `src/lsp/client.rs::LspClient/incoming_calls` | 2 | name_collision | 0 | — | — | 0/0/0 |
| 31 | `src/lsp/client.rs::LspClient/outgoing_calls` | 2 | name_collision | 0 | — | — | 0/0/0 |
| 32 | `src/lsp/client.rs::LspClient/prepare_call_hierarchy` | 2 | name_collision | 0 | — | — | 0/0/0 |
| 33 | `src/lsp/client.rs::LspClient/references` | 2 | name_collision | 0 | — | — | 0/0/0 |
| 34 | `src/lsp/client.rs::LspClient/rename` | 2 | name_collision | 0 | — | — | 0/0/0 |
| 35 | `src/lsp/client.rs::LspClient/workspace_symbols` | 2 | name_collision | 0 | — | — | 0/0/0 |
| 36 | `src/lsp/manager.rs::LspManager/notify_file_changed` | 2 | name_collision | 0 | — | — | 0/0/0 |
| 37 | `src/lsp/manager.rs::LspManager/shutdown_all` | 2 | name_collision | 0 | — | — | 0/0/0 |
| 38 | `src/lsp/mux/process.rs::read_proc_memory` | 2 | name_collision | 0 | — | — | 0/0/0 |
| 39 | `src/util/fs.rs::RepoPath/from` | 2 | name_collision | 0 | — | — | 0/0/0 |
| 40 | `src/util/path_security.rs::DEFAULT_DENIED_EXACT` | 2 | name_collision | 0 | — | — | 0/0/0 |
| 41 | `tests/fixtures/nav-eval-rust/src/trait_dispatch.rs::Counter/next` | 2 | name_collision | 0 | — | — | 0/0/0 |
| 42 | `tests/fixtures/typescript-library/src/extensions/advanced.ts::BookMetadata` | 2 | name_collision | 0 | — | — | 0/0/0 |

> **Note:** this table was hand-rendered from `params` for the 2026-06-13 dogfood. The `render_template` is attached but the write path does not yet auto-project it onto the body (tracked as a 2b follow-up). `params` is the source of truth; query it with `artifact(action="get", id="cd886c414f6751b4", entry_filter=...)`.

---

## Verdicts (Dzo-owned)

_Per-key triage goes here — classify code-class vs tool-class, name the move, note human-cost. One `### <key>` section per target the Dzo picks up._

### src/lsp/client.rs — the LspClientOps collision cluster (rows 26–35)
**Verdict:** code-class (real `edit_code` ambiguity). Eleven `LspClient` methods resolve to TWO symbols each — an inherent `impl LspClient` plus a trait `impl crate::lsp::ops::LspClientOps for LspClient` exposing the same names (verified: `LspClient/hover` at `client.rs:1155` and `:1498`). Any `edit_code(symbol="LspClient/<m>")` hard-fails "matches 2 symbols".
**Move:** none required for behavior — the trait mirror is intentional. The legibility cost is navigational: address these via `symbol_at(path, line)` or a disambiguating path, not by name. If the duplication is purely a forwarding shim, consider collapsing the inherent impl into the trait (one definition site).
**Human-cost:** low — a known pattern, not a latent bug.
