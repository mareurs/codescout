---
id: '64f10cc45d802a11'
kind: tracker
status: draft
title: I1 Refactor — Task List
owners: []
tags:
- task_list
- goal-tracker
- cross-pollination
- i1-refactor
topic: null
time_scope: null
---

## Tasks

Mirror of the I1 refactor plan tasks. Each task here corresponds 1:1 to a
task in [the plan doc](../superpowers/plans/2026-05-17-i1-refactor.md).
Use this tracker to see plan status at a glance; the plan doc itself
remains the source of truth for *how* to execute each task.

## Per-task notes

### Phase 1 — I1 Core Landing (the minimum viable refactor)

- **T-1** — Extract rule-1 clauses into Rust constants in `tracker_design.rs`.
  Prep for T-2. Single file, ~30 LOC. Snapshot test may regenerate.
- **T-2** — `src/librarian/tools/goal_aggregation.rs` with `child_status_pure`
  covering 5 clean archetypes (failure_table, task_list, audit_issues,
  reflective, nested goal) plus deployment_state (per D3). `metric_baseline`
  returns `Unknown` here — needs parent context, lands in Phase 2.
- **T-3** — `gather_goal_children` helper in `gather.rs` + refresh dispatch.
  Yak's variant (b) gather-time injection.
- **T-4** — Prompt rule 1 collapses to "copy verbatim from
  `deterministic_child_statuses`". Rule 2 phrasing tightened. Rule 4a
  strengthened to `len(children) >= 2`. Discovery paragraph in `source.md`
  updated.
- **T-5** — Drift tripwire test: prompt's "Rust-handled" list matches
  `child_status_pure`'s non-`Unknown` archetypes.

### Phase 2 — Structured Signals

- **T-6** — `AcceptanceSignal` enum with `kind` discriminant (D4). 7 kinds
  including freeform default.
- **T-7** — `metric_baseline` aggregation via `child_status_in_context`
  (D8). Reads parent's `acceptance_signals[kind=metric_threshold]` to
  derive child status.
- **T-8** — Update prompt rule 1b to drop `metric_baseline` from the
  LLM-handled list once T-7 lands.

### Phase 3 — Refresh Discipline

- **T-9** — Split `progress_log` into `refresh_meta` (Rust) + `progress_log[].note`
  (LLM). Solves F-A. Surfaces `last_refresh_at` for the Stop hook's reason
  text (S-1 already shipped via T-14; T-9 makes it canonical).
- **T-10** — `evaluate_gate` in Rust. Rule 6 NEVER list collapses. Done-status
  transitions go through Rust gate.
- **T-11** — Scope-growth cap validator. Refuses >1 new child per refresh.
- **T-12** — Gate-check audit emitted as `note` event with `tag: gate_check`.
  Resolves A-3 (verdict event kind misuse).

### Phase 4 — Schema + Hook Cleanup

- **T-13** — Widen `audit_issues` archetype schema to fold optional fields
  audit-doc-refs writes (severity_reason, ref_kind, md_file, md_line,
  raw_ref, first_seen_commit, first_seen_at, last_verified_at).
  Resolves F-D.
- **T-14** — *(DONE — commit `0b75991` in `codescout-companion`,
  2026-05-17)* Stop hook reason text surfaces `last_refreshed_at` on every
  branch; `unknown` status splits from `*` glob with distinct fail-open
  signal. Hamsa S-1 + S-2.

## History

### 2026-05-17 — Plan written, T-14 shipped

Plan doc created at `docs/superpowers/plans/2026-05-17-i1-refactor.md`.
14 tasks across 4 phases. T-14 shipped same day as the quick-win commit
in the companion plugin repo. T-1 through T-13 pending — they live in
the codescout repo and Phase 1 (T-1 to T-5) is the minimum-viable I1
landing.

