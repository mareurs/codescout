---
status: archived
---
# Archived plans

Plans in this directory have **shipped** or are otherwise settled. They are preserved for historical context — what we were trying to build, why, and what tradeoffs we considered — not as living specifications. For current behavior, read the code; for change history, read `git log`.

Active plans live one level up in `docs/plans/`.

## Index

| Plan | Shipped as |
|---|---|
| `2026-03-20-code-review-and-platform-abstraction-design.md` | `feat(platform)` commit `341fb46` + follow-ups |
| `2026-03-20-phase1-security-profiles.md` | `SecurityProfile` in `src/config/` |
| `2026-03-20-phases-1b-2-3-implementation.md` | code-review Phases 1b / 2 / 3 (pre-refactor branch) |
| `2026-03-23-document-section-editing-design.md` | `feat: document section editing` commit `4991cc2` |
| `2026-03-23-document-section-editing-plan.md` | same |
| `2026-04-02-usage-traceability-design.md` | `codescout_sha` + `session_id` columns, `--debug` flag |
| `2026-04-02-usage-traceability-plan.md` | same |
| `2026-04-02-corrections-store-design.md` | **Not built.** Need met instead by semantic memory (`memory` remember/recall, `src/memory/`) + `src/operator_rules/`. Kept for its unimplemented `times_applied` / decay design |
| `2026-04-02-onnx-intent-router-design.md` | **Router not built** (no `ort`, no unified `navigate`/`explore`/`edit`). Its Phase 0 shipped by hand: `symbol_at`, `edit_code(action=…)`, `references`, `symbols`, and the `action=`-dispatch shape across `workspace`/`index`/`library`/`artifact`/`librarian` |
| `2026-04-13-mcp-token-budget-design.md` | *(pointer never recorded — predates this rule)* |
| `2026-04-13-mcp-token-budget-plan.md` | *(pointer never recorded — predates this rule)* |
| `2026-04-22-codescout-refactoring-plan.md` | Phases 0–7b: `15cabec`, `775eb22`, `3947cd8`…`6f1bf8f`, `ad9f70e`…`b79d09b`, `e98c8ec`, `6cc878d`…`28a2932`, `555b1ac`. Phase 8 partial by decision (8.3 deferred to hand-editing, 8.4/8.6 dropped) |
| `2026-04-23-codescout-refactoring-plan-phase-1b.md` | `refactoring` branch Phase 1b commits `29c0568`…`bf8e211` |
| `2026-06-16-two-stack-retrieval-lite.md` | Phases 0–4: `825c0c52`, `0ff972f7`, `b96c8ae4`, `93ef0d43`, `9d40d36b`, `5c1ecfa8` |
| `2026-07-17-tracker-lifecycle-stage1-plan.md` | D10 detector `claude-plugins:a000916`; ledger bootstrap `e32d42cf`; plan↔tracker link `bfab09ac`; first sweep `36588c95` |

When moving a plan here, add its row above with the commit(s) that shipped it — that is the whole point of keeping these around. A plan without a "shipped as" pointer is a breadcrumb to nowhere.
