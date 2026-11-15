---
id: bb10d700b9552d01
kind: research
status: active
title: Plan D — execution state, rulings, deferred findings and what to do next
owners:
- marius
tags:
- identity
- plan-d
- rulings
- state
---

# Plan D — execution state, rulings, deferred findings and what to do next

Written 2026-11-15. The SDD ledger lives in the git-ignored directory `.superpowers/sdd/2026-11-14-artifact-identity-plan-d-cutover-tool/` of the worktree `.worktrees/cutover-tool-20261114` and dies with it. It holds 50 `Ruling:` lines. This document holds the ones that matter. Related: plan `docs/superpowers/plans/2026-11-14-artifact-identity-plan-d-cutover-tool.md` (D-R1..D-R30), scout `489339eef5f9e93b`, bug `28c8a999b2f3132b` (C1-a), bug file for `rel_path_for` (committed `95fb4042a`).

## Where the work is

- **Landed on integration, not on `experiments`.** `integration/identity-lifecycle-20261114` was fast-forwarded twice on 2026-11-15: `5d655bfef` to `ead2f7b31` (Plan D), then `ead2f7b31` to `308f9f5df` (C1 fix wave 2). The operator chose the integration branch as the landing route. The peer `codescout-e4` (lifecycle session) was told before and after each fast-forward. Nothing is pushed. `experiments` gets one merge from integration only when the operator says all work is done.
- The branch `feat/cutover-tool-20261114`, its worktree and the SDD workspace are kept. Delete the workspace after the operator agrees that the F1/F2 residue (below) may stay; remove the worktree only on the operator's answer.
- Gate on the first merged tree (HEAD `ead2f7b31`): python 1836 passed, fmt 0, clippy 0, lean 0, default 0 (470 s). Gate on the second tree (HEAD `308f9f5df`): all lanes 0 (445 s).

## What was built

`codescout cutover plan|export|build`. `plan` is a pure planner: it composes an `ExportPlan` from the committed blobs of `HEAD` plus read-only old catalog rows. `export` applies the plan through the atomic write door under the project write lock, then writes a markdown and a JSON report beside the old catalog (never overwritten). `build` makes `derived.db` beside the old catalog from `HEAD`, only when the plan writes nothing. Manual page: `docs/manual/src/concepts/artifact-cutover.md`. End-to-end test: `tests/cli_cutover.rs`. Code: `src/librarian/cutover/*`, `src/cli/cutover.rs`.

## Commits (on the branch)

- Tasks 1-7: `5ed828ae8`..`ef33f2315` (21 commits from merge base `755630fa0`).
- Task 8: `6ba948ecd`, fix `4bbc9e8e6`. Task 9: `df6a1f5b1`, fixes `4f392ab3f`, `9116ab018`.
- Final fix wave: `16938991e` (C1, I2, m7), `db0576417` (I1 fix a, m6), `19ff88427` (m1-m4).
- Merge of integration: `ead2f7b31`.

Method: subagent-driven. Implementers on sonnet. Reviewers on Opus with the recon skill. One whole-branch Opus review, one fix wave, one scoped re-review (verdict: not merge-ready because of C1-a; the controller landed on integration anyway under the operator's integration route and recorded C1-a as parked).

## Rulings (controller; the operator may overrule any)

- Bare token: rewritten only when exactly one document defines it, counting every definer; else reported (spec binds over the plan). Cost: more reports, fewer rewrites.
- Export waits `LOCK_ATTEMPT` = 250 ms for the write lock, not zero (`write_guard::acquire` refuses a zero budget). Cost: a held lock is refused after about six tries.
- `apply` refusal reasons are fixed texts and name no path. A report-write failure after the writes prints a note and exits 1.
- `build` refuses when HEAD moved after the plan, when a side file (`-wal`, `-shm`, non-empty `-journal`) lies beside `derived.db`, and when `derived.db` was built for another repository (`repo_name` differs or the `commit_sha` is unknown to this git). Cost: a crash leftover blocks `build`; a clone with unrelated history counts as another repository; `build` takes no write lock.
- Multi-repository `derived.db` layout is NOT decided here. Plan E decides. The manual page warns the operator not to cut over a second repository first.
- C1 fix: added text (notes, observations, entry titles) is rewritten like committed text; `cited_ids` also reads old-row note, observation and title texts. Cost: a history-only id in a note counts as cited.
- I1 fix (a) only: data files (`.jsonl`, `.txt`, `.json`, `.csv`, `.log`), key-space-excluded subtrees and `.codescout/audit` no longer count as code references. After it 914 of 2639 docs stay frozen (738 bug files). Fix (b) (a full-path mention blocks only the rename) waits for the operator because it relaxes D-R26.
- `main.rs --help` still says "was made by a newer codescout". Cost: help and page wording differ.

## Open and parked findings

- C1-a (the citation index built before entry recovery), C1-b and C1-d are FIXED and re-reviewed (`51f0d971a`, `052140615`, `308f9f5df`); bug `28c8a999b2f3132b` is `mitigated`.
- Still open, rare, lose no data, settle on a second export and commit: F1 (a seven-hash heading in a ledger), F2 (a frontmatter comment shaped like an entry heading), CRLF mixed line endings. The reviewer's fix: make `recover_params_entries` decide "already defined" with `defined_tokens`, the index's own reader, and add F1 and F2 as fixed-point tests.
- Parked minors: C1-c, I1-a, a manual-page nit (`book.toml` in an excluded subtree still freezes its book).
- Task 8 deferred: a rename can replace a target that appears in the check window (M56); a report name is claimed by check-then-atomic-replace, so two worktrees can collide (M57); post-apply failures exit 2 (M58); the lock test cannot tell `lock_root_for` from the git root (M59); case-insensitive filesystems untested (M60).
- Other parked items: cross-document observation id collision (M54); the tool timing owed by codescout-37 for entry documents; exported ledger entries lack `status_since` (Plan L-E input).

## Preconditions for Plan E

- Decide the multi-repository `derived.db` layout (the spec says storage is scoped by checkout).
- Remove the cutover entry points in the deletion tail: only `main.rs`, `cli/mod.rs` and `librarian/mod.rs` reference the cutover outside `src/librarian/cutover/` and `src/cli/cutover.rs`.
- Before the cutover rebuild, run `reindex` with `reembed=true` per project or ship vector seeding (C3 follow-up), or the rebuild re-embeds everything.

## Next steps, in order

1. Plan E (read/write flip, deletion tail). The operator said "you choose" on 2026-11-15 and I made three choices: C1-a fixed now (done); I1 fix (b) NOT taken (D-R26 stays; 738 bug files stay frozen, reported and edited by hand; reversible later); the multi-repository `derived.db` layout goes to Plan E, direction: storage scoped by checkout.
2. Optional: fix F1 and F2 with the `defined_tokens` change above.
3. C3 follow-ups: seeding, eviction, per-run cache handle, embed URL in the key.
