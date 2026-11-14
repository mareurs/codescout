---
id: '512e01311fe5d7bb'
kind: research
status: active
title: Plan C3 — execution state, rulings, deferred findings and what to do next
owners:
- marius
tags:
- identity
- plan-c3
- rulings
- state
---

# Plan C3 — execution state, rulings, deferred findings and what to do next

Written 2026-11-14. The SDD ledger lives in the git-ignored directory `.superpowers/sdd/2026-11-14-artifact-identity-plan-c3-vector-cache/` of the C3 worktree and dies with it. This document holds what the ledger holds. Related: scout `561841b0ae4a01d8`, session log `abff63149e3c24e8` (F-19..F-24, W-12), plan `docs/superpowers/plans/2026-11-14-artifact-identity-plan-c3-vector-cache.md`.

## Where the work is

- **LANDED 2026-11-14.** The operator chose "merge locally". `experiments` was merged into the branch twice (the second merge brought only my two docs commits), the gate ran green on the first merged tree, and `experiments` fast-forwarded `8f4696f7f` to `a54988e05` (10 files, +3229/-29). The worktree and the branch are removed; the SDD workspace went with the worktree.
- Gate on the gated tree (HEAD `b3535ceb7`, untouched after the merge): FMT 0, CLIPPY 0, LEAN 0 (10129 passed, 61 skipped), DEFAULT 0 (10224 passed, 66 skipped), python 1836 passed, 7 skipped. The second merge changed only two docs files, so the gate was not repeated.
- Removal detail: the only client of the rust-analyzer mux held by the worktree was this session's own MCP server (socket peer pid check), so `--stop` was used; git refused once for an empty untracked `.cs-worktree-pending` marker, which was removed (0 bytes) without `--force`.

## What was built

A per-user file `vector-cache.db` in the directory of the catalog file. Key: (`model|budget|COMPOSITION_VERSION`, `embed_input_hash(title, text)`). A seam on `EmbeddingService` (`embed_artifact_cached`) reads the cache before the embedder call and stages each fresh vector. Each site flushes the cache beside its store write. Sites: the reindex drain (`tools/reindex.rs`), backfill and `index_repo` (`indexer.rs`). Reindex responds with `embed_cache {status, hits, stored, errors}`. Nothing evicts a row.

**Honest limit (F-20):** queueing is per artifact. An artifact embedded before C3 and unchanged is never queued. On the day C3 ships the cache is EMPTY for the whole corpus. It fills only from text embedded afterwards (new or changed file, or `reembed=true`). The cutover rebuild (spec L230) reuses vectors only after one `reembed=true` pass per project or after the follow-up "seeding" ships.

## Commits (on the branch)

| Task | Commits |
|---|---|
| 1 store `vector_cache.rs` | 013113b7f, fix 69bb3af22 |
| 2 model key and seam | 18dc50fcc, fix e7a470f0e |
| 3 reindex drain | 8c80a17ca |
| 4 backfill and `index_repo` | b2e3469bb |
| 5 manual page and guide | 9714ec4c2, fix 18b9e3c2e |
| final-review fix wave | c7ede5956, c0f8cb112 |
| doc micro-fix after the re-review | 26138ebae |
| merge of `experiments` | b3535ceb7 |

Method: subagent-driven. Implementers sonnet; reviewers Opus with the recon skill (snow-lion on Tasks 1, 2 and the whole branch); one whole-branch Opus review ("Ready after fixes", no Critical or Important); one fix wave; one scoped re-review (all 15 items addressed).

## Rulings (controller; the operator may overrule any)

- R1 Task implementers on sonnet, reviewers on Opus. Cost if wrong: one extra fix round per task.
- R2 The branch merges `experiments` before its gate (W-5 route). Cost: a conflict, none occurred.
- R3 to R5 Minors of Tasks 1, 2 and 5 rode in the fix round of the Important finding. Cost: a slightly larger re-review.
- R6 The per-run cache handle redesign and the embed URL in `ModelKey` are follow-ups, not part of this branch. Cost: a cancelled or concurrent reindex can flush another call's rows; a model name that lies stays accepted (plan C3-R3).
- R7 N-4 (a flush-on-drop guard on the reindex path) went into the fix wave in its small form. The guard type now lives in `embedding.rs` (`flush_on_drop`), used by reindex, backfill and `index_repo`.
- R8 A micro-fix subagent corrected three doc statements after the re-review, without a second re-review, because the fix wave allows none and a gate on a changed tree is void. The controller read the diff: three comment and doc edits, no code.
- Plan C3-R1..R14 are in the plan header. R4 (no eviction) and R5 (cold start) were rewritten after review F-19 and F-20.

## Deferred findings (shipped as is)

- T1: WAL pragma runs before the write lock (a newer binary committing in that window would see its journal mode changed; the format does not change).
- `transactions()` and `vector_cache()` keep `#[cfg_attr(not(test), allow(dead_code))]`: no production caller.
- T3: no test for a hit whose store upsert fails; the DEGRADED `embed_note` omits hits.
- T4: backfill test fixtures return the same vector for every text; a test comment says "a drain" in a backfill test.
- T5: the guide's weights-change sentence is long; the guide test does not guard the remedy order, the remote-embedder sentence or the size qualifier.
- Re-review Minor 4: the manual says "batch" without defining it. `indexer.rs` near L1655 says "how many cache operations failed" while a failed flush counts rows.
- Unrelated, found on the way: `truncation_sites_reach_the_real_corpus` pins `indexer.rs:918` and `1051`, which were already not stream drains at the base commit.

## Follow-ups (not planned yet)

1. **Seeding:** export vectors of marked chunks into the cache. sqlite: one SELECT over `artifact_vec_v2`; Qdrant: no read-back code exists. Needs a second sanctioned writer (the `vector_cache()` doc says the seam is the one writer).
2. **Eviction:** F-19 trap: an unchanged file is never looked up, so `last_used` cannot drive eviction. A refresh of marked chunks or a live-set rule is needed. The file only grows (about 3 KB per distinct input for a 768-dimension model).
3. **Per-run cache handle** owned by the seam (`cache_run()`: retry the open, tally, flush on drop). Removes the duty of each caller to pair embeds with a flush and the cross-call tallies.
4. **Embed URL in `ModelKey`** so two servers behind one model name do not share vectors.
5. **Cache location vs the cutover:** `VectorCache::beside` follows the catalog file. If the cutover puts the new catalog in another directory, the cache starts empty exactly for the rebuild the spec names. Decide the location in Plan D.

## Preconditions for Plan D and E from this plan

- Before the cutover rebuild, run `reindex` with `reembed=true` per project (or ship seeding), or the rebuild re-embeds everything (about 29 to 48 minutes for this corpus, scout arithmetic).
- Stop every codescout process before deleting `vector-cache.db` and its `-wal` and `-shm` files (F-22).

## Next steps, in order

1. Write the Plan D plan (scout `489339eef5f9e93b`; open questions: who commits; file-only dual-write variant vs scratch copy of the old catalog; the C3 cache location, follow-up 5 above).
2. Plan E.
3. Optional follow-ups 1 to 4 above, in the order the operator chooses.
