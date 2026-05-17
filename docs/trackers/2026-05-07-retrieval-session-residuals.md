---
kind: tracker
status: draft
title: Retrieval Stack — Session Residuals 2026-05-07
owners: []
tags:
  - retrieval
  - session-log
---

# Retrieval Stack — Session Residuals (2026-05-07)

**Created:** 2026-05-07 · **Status:** open

Issues that surfaced during the Phase 5.5 / 6 / 7 (narrow) work and are **not**
captured by:
- `docs/trackers/2026-05-07-legacy-retrieval-removal.md` (legacy code deletion)
- `docs/archive/old-trackers/TODO-tool-misbehaviors.md` (BUG-NNN log; BUG-053 lives there)
- `docs/research/2026-05-06-retrieval-stack-benchmark.md` (open follow-ups in
  the research doc — listed here only when they need a separate owner)

Each item is a single-paragraph note, not a design doc.

## Stack tuning follow-ups (matrix-adjacent)

### S-01 — CPU bm25_boost sweep on bs_c1200 not run

GPU sweep landed `cr_c1200 @ boost=3.0 = 30/60`. CPU default `bs_c1200`
inherited the same `bm25_boost=3.0` by analogy without an empirical sweep.
Replicate the sweep against the CPU profile to confirm the +2..+3 projected
gain. ~1.5 h of wall time per sweep on this corpus.

### S-02 — Reranker on/off ablation at the new tuned defaults

The stack ships with a reranker container (`bge-reranker-v2-m3`) and
`SearchOpts::rerank: true`. Phase 5.5 matrix turned it OFF to isolate dense
+ sparse fusion quality. Has not been re-tested at `chunk=1200,
bm25_boost=3.0` to know whether the reranker still pulls its weight at the
new operating point.

### S-03 — Kotlin 25-TC suite re-run at chosen defaults

`scripts/extract-kotlin-tcs.py` mined a 25-TC suite from `usage.db`
session causality (semantic_search → file opens). Phase 5.5 matrix used
the codescout 20-TC v2 suite. Re-run the kotlin suite at
`cr_c1200 @ boost=3.0` and `bs_c1200 @ boost=3.0` to validate the defaults
generalise across corpora.

### S-04 — Try `nomic-embed-code` as a 4th GPU candidate

Listed in the research doc's open follow-ups. nomic-ai released a
code-tuned dense model trained on a different mix than CodeRankEmbed; worth
one matrix cell to see if it beats `cr_c1200 = 28/60` base.

## Stack runtime gaps (Phase 7 narrow side-effects)

### S-05 — Stack search ignores user-provided `project_id`

`SemanticSearch::call` reads `project_id` from input but the stack path uses
the active project's name from `Agent::active_project`, never the input
field. Pre-existing in Phase 6 stack code; surfaced during Phase 7 review.
Either honor the input (multi-project queries) or drop the schema field.
The Iron Law "scope a multi-project workspace with `project_id`" in
server_instructions.md still advertises it.

### S-06 — `SemanticSearch::long_docs` references the legacy index

`long_docs` claims `index(action='build')` is a prerequisite. Post-Phase 7
that's wrong — the prerequisite is the docker stack + `sync_project`. Same
file's `input_schema` advertises `include_memories: bool` without flagging
that the stack returns RecoverableError when set. Update both.

### S-07 — `chunk-model-matrix.py disable_sparse` flag never validated

The orchestrator added a `disable_sparse` cell flag in late Phase 5.5 but no
control cells were run with it. The flag plumbs through `cell_env` →
`CODESCOUT_DISABLE_SPARSE`, and the runtime path is exercised by hand-runs
but the matrix never recorded a `*_b0_disabled` row. Run a 4-cell control
when convenient (just to confirm sparse leg materially contributes at the
chosen defaults).

## Repo / docs hygiene

### S-08 — Plan doc Phase 7 still describes the unscoped removal

`docs/superpowers/plans/2026-05-06-retrieval-stack-plan.md` § Phase 7 still
walks the reader through "delete src/embed/index.rs / bm25.rs / fusion.rs"
without acknowledging the narrowing decided in this session. Add a status
note at the top of Phase 7 pointing to
`docs/trackers/2026-05-07-legacy-retrieval-removal.md`. Do not rewrite —
the plan is historical record.

### S-09 — `CLAUDE.md` references nonexistent `docs/ARCHITECTURE.md`

The Docs section in `CLAUDE.md` lists "`docs/ARCHITECTURE.md` — Component
details, tech stack, design principles" but no such file exists in the repo.
Either delete the line or write the file. Found during the Frog audit;
unrelated to retrieval stack so left alone for that commit.

### S-10 — `system_prompt_points_to_tool_guide_resource` test asserts the literal version

`src/tools/run_command/tests.rs` has `assert_eq!(ONBOARDING_VERSION, N)` that
must be updated by hand on every bump. This session bumped it twice
(24→25 sync from master, then 25→26→27 for our prompt edits) and it
flagged each time. Either drop the literal-version assertion (the bump
itself is the version) or compute it programmatically.

### S-11 — `scripts/sweep-bm25-boost.sh` and `scripts/sweep-bm25-cr1200.sh` overlap

Two near-identical bash sweeps with different hardcoded paths. Consolidate
into one script with a `--profile cpu|gpu` arg, or delete the older one
(`sweep-bm25-boost.sh` predates `sweep-bm25-cr1200.sh`).

### S-12 — `docker-compose.matrix.yml` is matrix-only scaffolding

The 4 parallel TEI containers (8090–8093) were used to run all model cells
in parallel during the Phase 5.5 matrix. After Phase 6 the steady-state
stack only needs the regular `docker-compose.yml`. Either document
`docker-compose.matrix.yml` as a benchmark-only file or move it under
`scripts/bench/` so it's not picked up by `docker compose up` accidentally.

### S-15 — `cargo clippy --tests -- -D warnings` fails on master (7 errors) ✅ DONE 2026-05-07 (`b483d48`)

Cleared in three groups. Worth flagging: the test-attribute group (3 + 2)
was a silent test-coverage bug — `config_from_env_*`, `client_from_env_*`,
`embedder_returns_dense_and_sparse`, `embedder_dim_mismatch_errors` had
been compiling but never running since the file was first added. One
assertion (`model_dim == 1024`) was stale by ~3 model swaps; flipped to
768 (CodeRankEmbed, current default).

Original analysis kept for retrospective:

`master` HEAD violates the CLAUDE.md "clippy clean before completion" gate.
Errors are pre-existing — present before this session's L-02 commit and
unrelated to the retrieval stack work. Breakdown:

- **`src/tools/symbol/call_edges/cache.rs`** — 6× `cloned_ref_to_slice_refs`
  (e.g. `cache.upsert(&[edge.clone()])` → suggested `std::slice::from_ref(&edge)`).
  Likely surfaced by a clippy version bump on rust-1.94.0; the lint is recent.
- **`src/lsp/ops.rs:75`** — `items after a test module` on
  `mod call_hierarchy_trait_tests`. Structural — module ordering wrong.
- **`tests/retrieval_unit.rs` and `tests/retrieval_integration.rs`** — 5×
  `dead_code` on `#[test]`/`async fn` test functions. The functions miss
  `#[test]` / `#[tokio::test]` attributes (or their gating cfg is off in default
  build). Either restore the missing attributes or move the file behind a
  `#[cfg(feature = "...")]` so the lint stops firing.

All seven are mechanical fixes, single commit. Do this **before** any
release tag — `cargo publish` runs clippy implicitly on some CI configs.

### S-16 — Four uncommitted docs sit in working tree on master

`git status` after L-02 reports four untracked artifacts that have lived
outside version control across multiple sessions:

- `docs/superpowers/plans/2026-05-07-onboarding-refactor.md`
- `docs/superpowers/plans/2026-05-07-server-instructions-consolidation.md`
- `docs/superpowers/specs/2026-05-07-onboarding-refactor-design.md`
- `docs/trackers/retrieval-benchmark.md`

Either commit them (if they are still load-bearing) or delete them (if they
are scratch). They show up in `git status` on every session, increasing the
chance one is accidentally `git add .`-ed without review. Audit + decide
in a single triage pass.

### S-17 — L-02 leaves a `pub use` re-export shim in `embed::index`

To avoid touching the 4 in-file callers + 2 `run_command/tests.rs` callers
that already deletes when L-01 lands, L-02 left
`pub use crate::memory::hash::hash_file;` at the top of `src/embed/index.rs`.
When L-01 actually deletes `src/embed/index.rs`, the two test sites in
`src/tools/run_command/tests.rs` (lines 2948, 3002) that import via
`crate::embed::index::hash_file` must be flipped to
`crate::memory::hash::hash_file` in the same commit. Add a checklist note
to L-01 in the legacy-removal tracker if not already there.

## Operational

### S-13 — Local `master` is 64 commits ahead of `origin/master`

24 prior prompt commits + 35 retrieval commits + 3 Phase 7 commits +
2 tracker/L-02 commits = 64. Worth eyeballing `git log --oneline origin/master..master`
before push, especially the prompt-surface commits from earlier sessions
that have not been individually reviewed in this session.

### S-14 — Phase 7 commits landed on `master` directly, not via cherry-pick

`CLAUDE.md` Standard Ship Sequence says cherry-pick from a feature branch.
Phase 7 was committed directly on `master` because `master` and
`retrieval-stack` were already at the same SHA after the Phase 6 fast-forward.
Operationally the same outcome but the Frog ritual / cherry-pick discipline
was bypassed. No action needed; flagged for retrospective.
