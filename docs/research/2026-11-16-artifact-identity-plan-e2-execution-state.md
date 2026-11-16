---
id: '82be9a55b2fe0746'
kind: research
status: active
title: Plan E2 — execution state (in progress) and how to resume
owners:
- marius
tags:
- identity
- plan-e2
- state
- resume
---

# Plan E2 — execution state (in progress) and how to resume

Written 2026-11-16 for a session compaction. Plan `docs/superpowers/plans/2026-11-16-artifact-identity-plan-e2-derived-schema-v2.md` (`dc0d65557a9577ee`, 10 tasks, rulings E2-R1..R16, committed `def9c8c75`), plan review `9a89ced57da874b7` (22 findings, all accepted), scout `02d716631c64e393` (rulings Q1-Q8 in section (e)), E1 state `6ba1be1f51416c0e`.

## Where the work is

- Worktree `/home/marius/work/claude/codescout/.worktrees/e2-derived-schema-v2-20261116`, branch `feat/e2-derived-schema-v2-20261116`, BASE `725eea39a` (= integration tip = Plan E1 landed). HEAD `01cad96e9`. Nothing is pushed, nothing is landed. Integration is still `725eea39a`; `experiments` is `def9c8c75` (docs only on top of the fork's `0529d8274`).
- SDD workspace (git-ignored, in the worktree): `.superpowers/sdd/2026-11-16-artifact-identity-plan-e2-derived-schema-v2/` holds `progress.md` (the ledger: pre-flight table, rulings F1..F14, every task ruling and review summary), `task-N-brief.md`, `task-N-report.md`, `task-N-review-*.md`, `review-*.diff`. Read the ledger first. Run the SDD scripts with the plan's ABSOLUTE path (the plan is on `experiments`, not in the worktree). Script dir: `/home/marius/.claude-stef/plugins/cache/superpowers-marketplace/superpowers/7.0.2/skills/subagent-driven-development/scripts` (`task-brief PLAN N`, `review-package PLAN BASE HEAD`).
- **Task 1 complete** (schema v2; commits `04e2ef5c7`, fix `71edcda3e`; review clean). **Task 2 fix round 1 committed** (`d2d948965` event sections to `event` rows, fix `01cad96e9`); its scoped re-review is RUNNING (Sonnet agent `aef58f96b3ba79745`, writes `.../task-2-review-2.md`, runs temporary mutations, so `src/librarian/derived/incremental.rs` may show as modified in the worktree until it finishes and restores). On resume: confirm that agent has finished (its report arrives as a message; read `task-2-review-2.md`); only then check `git status --short` in the worktree; if it is still dirty and no agent runs, `git diff` it and restore with `git checkout -- <file>` (it is a leftover mutation). If the re-review is clean, record `Task 2: complete (commits 71edcda..01cad96, 1 fix round, review clean)` in the ledger and go on to Task 3. If it found something, resume the Task 2 implementer (`a1d8aed452d846e7e`, SendMessage) with the findings.
- Tasks 3-9 not started; Task 10 is the controller's (gate, three ignored measurements, Q4 check, bugs, this state doc).

## Method (continue exactly this)

Per task: `task-brief` -> Sonnet implementer (dispatch names the brief, the plan sections to read by absolute path, the ledger rulings that apply, pins `workspace=` to the worktree, TDD, targeted tests only through `scripts/with-slot.sh cargo test ...` with the command forms in the plan's Global Constraints, no `gate.sh`, commit by pathspec, report file) -> `review-package` -> Opus reviewer (reconnaissance and snow-lion skills; adversarial probes; mutations; must leave `git status` clean) -> fix round by resuming the implementer (SendMessage to its agent id) -> scoped re-review (Sonnet is fine for small fixes) -> ledger line `Task N: complete (...)`. After Task 9: whole-branch Opus review (`review-package PLAN 725eea39a <HEAD>`), ONE fix wave (one Opus fixer), exactly one scoped re-review, then `./scripts/gate.sh` in the background (about 7.5 minutes; the wake hook reports it; do not poll), then: check integration tip is still `725eea39a` (if it moved, merge integration into the branch and re-gate), announce to the lifecycle fork, `git merge --ff-only feat/e2-derived-schema-v2-20261116` in `.worktrees/integration-identity-lifecycle`, announce again. Peer address: `Organizare și arhivare docs/trackers ⑂ "I want to keep this session saved since it has continuous w [cafcab]` (the bare name is refused; use this exact string; `ListAgents` shows it). Never push. Do not delete the worktree or the SDD workspace without the user's answer.

## Rulings carried into the remaining dispatches (full text in the ledger)

- F1 (Task 7): make `a_rename_onto_a_freed_path_keeps_both_histories` a two-commit freed-path test; drop the order mutation. F2 (Task 7): stop on `parent_count()==0` (libgit2 applies shallow grafts); drop the NotFound clause. F3 (Tasks 6, 8): Task 6 also adds `testkit::git_exit(dir, args) -> (i32, String)`. F5 (Task 5): `sidecar_*` and `augmentation_unparseable` problem rows join Task 4's sorted per-document list and increment `report.problems`. F6 (Task 5): store the Normal-joined path. F7 (Tasks 6-9): Step 4 also runs `--test result_caps --test inline_gates`; new `.next()`/`.take(` get `// cap-class:`. F8 (Task 9): expected red is 3 of the 4 tests. F11 (Task 6): pathspec test uses subdirectories; name Typechange in E2-R10's cost. F12 (Task 8): symlink path gets a layer row with NULL sha256; record the layer-deleted/`doc_history` fact for E4. F13 (Task 8): symlink test `#[cfg(unix)]`. F14 (Tasks 4, 7, 8): each ignored measurement gets one structural assertion; reuse a `statements::Validity` helper if one exists.
- Instruction from the Task 1 and 2 reviews, for Tasks 3-5: any field added to `FactsJson`/`EventJson` must be REQUIRED (a cache row without it is a miss) or `PARSER_VERSION` is bumped; E2-R3 relies on it. The task that fills a table pins its `DERIVED_TABLES` entry (Task 2 did for `event`).
- Parked for the final fix wave: Task 1 nit N-1 (`schema.rs:309` `NOT LIKE 'sqlite_%'` should be `NOT LIKE 'sqlite\_%' ESCAPE '\'`). Deferred by ruling: Task 2 m-2 (a malformed event section is silently dropped, same as the live reader; E4 decides), m-5 (no events count in `BuildReport`).
- Open items for the E4 plan to decide: entry fields for section ledgers (16 of 25 committed sidecars declare an `entry_collection` whose rows live only in catalog params; no committed document has `entry_kind` frontmatter); E6 extends the working-tree layer to every declared sidecar path when `augment` becomes file-only.

## Other open items (not E2)

- Operator: when to merge integration into `experiments` (the lifecycle fork does it on request; it did once, 2026-11-15, `0529d8274`); when to run the cutover window; the E1 worktree `.worktrees/e1-storage-location-20261115` (branch `feat/e1-storage-location-20261115`), the Plan D worktree `.worktrees/cutover-tool-20261114` and their SDD workspaces are kept until the user answers (E1 landed at `725eea39a`, Plan D at `19e87e370`).
- Plan D follow-ups: bug `28c8a999b2f3132b` (C1-c, I1-a, two nits), cross-document observation id collision (M54), the entry-document tool timing owed by codescout-37, `status_since` missing from exported ledger entries.
- Plan E order after E2: E3 (key addressing, additive), E4 (first reader; duties listed in the E1 state), E5, E6, E7, E8. Ruling Q4: E4-E8 must not reach `experiments` before the operator runs the cutover window.
