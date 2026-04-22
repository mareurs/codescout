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
| `2026-04-23-codescout-refactoring-plan-phase-1b.md` | `refactoring` branch Phase 1b commits `29c0568`…`bf8e211` |

When moving a plan here, add its row above with the commit(s) that shipped it — that is the whole point of keeping these around. A plan without a "shipped as" pointer is a breadcrumb to nowhere.
