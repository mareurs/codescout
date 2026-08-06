---
id: '1dcfdd70de0fcc73'
kind: tracker
status: active
title: Test-Escape Hardening — interventions from the entry-graph Stage 2 review lessons
topic: test escape hardening
---

## Overview

Every real defect in the entry-graph Stage 2 feature was a **discovery** problem in an
untested seam — the implementers wrote correct-looking code, tests passed, and only expensive
Opus reviews (2 per-task + 1 whole-branch) caught them, each because the controller
hand-injected the right lens. The lesson: those lenses must move LEFT into cheaper, standing
mechanisms so they catch by default without a human remembering.

**Defense-in-depth gradient (cheap/early → expensive/late):**
auto-loaded memory → skill prompts → static Rust lints/CI → per-task reviewer → whole-branch review.

## The five defects and what catches each

| # | Defect | Nature | Cheapest mechanical catch | Interventions |
|---|--------|--------|---------------------------|---------------|
| 1 | Table-copy migration dropped a later-added column (`slug`) → dangling FK for one open | static | schema-invariant `#[test]` | I-1, I-4, I-5 |
| 2 | Non-discriminating test: asserted only "an error occurred"; guard-deletion still passed | semantic | mutation testing only | I-3, I-4, I-5 |
| 3 | `resolve_cite_ref` shipped 2 of 3 branches untested | semantic | mutation testing (branch) | I-3, I-4, I-5 |
| 4 | Write/read asymmetry: id-keyed backlinks invisible for slug-less targets; both tests used slugged targets | semantic | mutation testing | I-3, I-4, I-5 |
| 5 | Unescaped user string in a `LIKE` pattern (`%`/`_` acted as wildcards) | static | grep `#[test]` for LIKE-without-ESCAPE | I-2, I-5 |

Key finding from the feasibility exploration: a `call_graph`-based "untested new symbol"
detector (I-6) is **structurally blind** to #3/#4 (a function that IS called by tests but
whose branches aren't exercised reads as covered) and useless on #2. The only mechanical
catch for the three semantic defects is **mutation testing scoped to the diff** (I-3).

## Interventions

### I-1 — Schema-invariant test (defect #1)
Generalize the one-off regression test into a loop: parse `SCHEMA_SQL`'s `CREATE TABLE artifact`
column list and assert, after a single `open_with_workspace` on every legacy seed fixture,
that every declared column + every required index (`ux_artifact_slug`, …) survives and
`pragma_foreign_key_check` is empty. Reuses `column_exists`, `SCHEMA_SQL`, seed fixtures — only
a tiny `parse_create_table_columns` helper is new. Cheap-and-robust; `cargo test`-gated for free.

### I-2 — LIKE-escape helper + gate (defect #5)
The escape idiom is copy-pasted in ~5 inline sites across two idioms with no shared helper
(see bug `docs/issues/archive/2026-07-17-like-escape-idiom-duplicated-no-shared-helper.md`). Extract
`escape_like_pattern` (the Rust-side needle-escaping) and route `filter.rs` + `augmentation.rs`
through it; add a source-scan `#[test]` (mirroring `claude_md_contains_no_deprecated_tool_names`)
asserting every `LIKE` literal has a paired `ESCAPE` clause.

### I-3 — Mutation testing at the ship boundary (defects #2/#3/#4)
`cargo-mutants` is not yet a repo dev-dependency. Add a diff-scoped `cargo mutants --in-diff <range>`
pass to the Standard Ship Sequence in `docs/RELEASE.md` (pre-cherry-pick to master). Surviving
mutants = untested behavior. Diff-scoping keeps cost tractable; ship-boundary cadence keeps it
off the per-edit hot path. This is the only mechanism that would have caught the non-discriminating
guard test (#2).

### I-4 — Standing review-lens bullets (defects #2/#3/#4/#1)
The superpowers `task-reviewer-prompt.md` / `code-reviewer.md` carry only a generic
"edge cases covered?" — none of the four lenses. Add standing bullets: mutation-thinking
("would this test pass if the target code were deleted/inverted?"), branch-coverage (a test per
new branch), round-trip completeness (every writer shape surfaced by a reader test), and a
cross-migration-seam check. **Ownership caveat:** superpowers is a marketplace plugin (cache,
not owned) — editing the cache is ephemeral. Durable home is the owned buddy `testing-snow-leopard`
(carries the lenses as doctrine); the superpowers edit is an upstream suggestion.

### I-5 — Durable recall (all defects)
codescout memories: `catalog-sql-hazards` (#1, #5) and `test-design-discipline` (#2/#3/#4) —
DONE 2026-07-17. Pending: `testing-snow-leopard` SKILL.md gains L3 (assert-on-cause), L4
(branch-pairing), L5 (round-trip completeness); reconnaissance SKILL.md gains two seam classes
(schema-migration ordering; writer-shape↔reader-surfacing). **Eval caveat:** the reconnaissance
edit is a scout-behavior change → requires re-scoring `docs/evals/reconnaissance-output.md`
(baseline n=0). The superpowers/testing-snow-leopard edits are not eval-gated.

### I-6 — Untested-new-symbol detector (deferred, low-yield)
Buildable cheaply from `call_graph(direction=callers)` + a `tests/` name-path heuristic, but only
catches wholly-orphaned new functions — the degenerate case, not the actual #3/#4 shapes. Kept
as deferred: mutation testing (I-3) dominates it on coverage-per-value.

## History

### 2026-07-17 — created from the entry-graph Stage 2 self-reflection
Four parallel deep-exploration subagents grounded the four defense layers (static-lint infra,
review-lens/recon-seam encoding, coverage-diff feasibility, buddy/memory homes). I-5 memories
(`catalog-sql-hazards`, `test-design-discipline`) written this session; LIKE-duplication bug filed.
Remaining interventions scheduled — I-1/I-2 via subagent-driven TDD, I-3 as a RELEASE.md edit,
I-4/I-5 skill edits (reconnaissance staged behind its eval).


### 2026-07-17 — interventions implemented (5 of 6; I-6 deferred)

All actionable interventions shipped this session. **I-1** schema-invariant guard `every_schema_sql_artifact_column_survives_every_migration_path` (commit `3e2459da`, mutation-verified: dropping a column from the migrate_v6 table-copy fails it). **I-2** extracted `escape_like_pattern` to `src/librarian/util.rs`, routed the two Rust-side sites, added the DRY gate `like_escape_idiom_is_not_inlined_outside_helper` (commit `14bd8b55`). **I-3** RELEASE.md now recommends `cargo mutants --in-diff` before cherry-pick (commit `1af2be8e`; advisory until `cargo-mutants` is adopted as a dev-dependency). **I-4/I-5** codescout memories `catalog-sql-hazards` + `test-design-discipline` (commit `fb9fd244`); `claude-plugins@627bbde` added L3/L4/L5 to buddy `testing-snow-leopard` and the two Phase-1 seam classes to `reconnaissance`. Bug filed for the LIKE duplication (`docs/issues/archive/2026-07-17-like-escape-idiom-duplicated-no-shared-helper.md`, now fixed for the 2 Rust-side sites by I-2; SQL-side chains remain out of scope).

**Open follow-up:** the `reconnaissance` SKILL.md edit is a scout-behavior change — establish/re-score the `docs/evals/reconnaissance-output.md` baseline (n=0) via prompt-tdd to bless it. **I-6** stays deferred (mutation testing / I-3 dominates a call_graph reachability detector on coverage-per-value).
