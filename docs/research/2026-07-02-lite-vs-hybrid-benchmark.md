---
title: Lite vs. Hybrid Retrieval Benchmark — 2026-07-02
date: 2026-07-02
topic: retrieval-quality
summary: 25-TC pinned-suite comparison of hybrid, SPLADE-disabled and lite sqlite-vec retrieval. The headline lite-beats-hybrid result is confounded by a confirmed harness bug, documented inline.
status: complete
---

# Lite vs. Hybrid Retrieval Benchmark — 2026-07-02

Task 10 (WIN-26 quality gate): a 25-TC pinned-suite comparison of three retrieval
configurations reachable via `scripts/run-tc-benchmark.sh` — the full hybrid stack
(dense + sparse + rerank), the same stack with SPLADE disabled, and the "lite"
sqlite-vec dense-only backend intended for constrained/VDI deployments. The headline
result — lite scoring higher than hybrid — is real in the numbers the harness emitted,
but a confirmed harness bug (documented below and in
`docs/issues/archive/2026-07-02-tc-benchmark-harness-swallows-buffered-results.md`) silently
zeroes out any test case whose `semantic_search` response is large enough to be
buffered. That bug hits the hybrid and no-sparse arms far harder than the lite arm, so
the raw A-vs-C delta below is a **lower bound**, not a clean quality comparison. Read
the Known Issues section before drawing conclusions from the numbers alone.

## Setup

| | Arm A: Hybrid | Arm B: No-sparse | Arm C: Lite |
|---|---|---|---|
| Invocation | `./scripts/run-tc-benchmark.sh` | `CODESCOUT_DISABLE_SPARSE=1 ./scripts/run-tc-benchmark.sh` | `CODESCOUT_VECTOR_BACKEND=sqlite-vec ./scripts/run-tc-benchmark.sh` |
| Backend | Qdrant v1.x (shared/live `code_chunks` collection) | Qdrant v1.x (same collection) | sqlite-vec (`vec0`, in-process, one DB per project) |
| Dense model | CodeRankEmbed, 768d, `http://127.0.0.1:48081` | same | same (identical embedder URL/model) |
| Sparse model | SPLADE, `http://127.0.0.1:48084`, bm25_boost=3.0 | **disabled** (`disable_sparse=1`) | n/a (lite stack has no sparse leg by design) |
| Reranker | bge-reranker-v2-m3, `http://127.0.0.1:48083` | same (still enabled) | **none** (lite stack skips rerank unconditionally — `src/retrieval/search.rs`) |
| `collection_prefix` | empty (shared/live collections) | empty | n/a (sqlite-vec is per-project by construction) |
| Project | `.worktrees/bench` (project id `code-explorer`) | same | same |
| Index freshness | force-reindexed this session (`added=24923 deleted=16180`) before Arm A/B runs | reused Arm A's fresh index | one-time `codescout index --project .worktrees/bench --force` with `CODESCOUT_VECTOR_BACKEND=sqlite-vec` before the run |
| `codescout_repo_head_sha` | `cb8e77c6e38e20bd9303bc329f5bd696f03edd85` (all three arms) | same | same |
| `codescout_build_sha` | `86292aec5665313d9d975d05bfb3eb02b5d1fe37` (dirty) | same | same |
| `codescout_version` | 0.15.0 | same | same |

All three arms ran against the **same** binary build (see Environment) and the same
frozen `.worktrees/bench` content, sequentially, in a single session.

## Aggregate Results (as emitted by the harness)

| Metric | Arm A: Hybrid | Arm B: No-sparse | Arm C: Lite |
|---|---|---|---|
| **Total score** | **9 / 75** | **26 / 75** | **31 / 75** |
| Tier 1 (5 TCs, max 15) | 5 | 6 | 8 |
| Tier 2 (7 TCs, max 21) | 1 | 5 | 10 |
| Tier 3 (5 TCs, max 15) | 1 | 5 | 6 |
| Tier 4 (3 TCs, max 9) | 1 | 3 | 2 |
| Tier 5 (5 TCs, max 15) | 1 | 7 | 5 |
| p50 latency | 1863.6 ms | 1021.7 ms | **72.3 ms** |
| p95 latency | 2629.7 ms | 1455.1 ms | **116.6 ms** |
| TCs with `top10_files: []` | **17 / 25** | **8 / 25** | **1 / 25** |

Per-TC, Arm C (lite) ties or beats Arm A (hybrid) on 24 of 25 TCs; Arm A wins only on
TC-05 (3/3 vs. 2/3). Given the empty-result caveat below, this comparison is not a
trustworthy read of true dense-only-vs-hybrid quality — see Analysis.

## Analysis

### The empty-result confound dominates the comparison

Of hybrid's 17 empty TCs, 8 overlap exactly with no-sparse's 8 empty TCs (SPLADE
on/off didn't change the outcome for those); the remaining 9 are unique to the hybrid
arm. Lite has a single empty TC (TC-06), which is also empty in both other arms —
the only TC plausibly a genuine cross-backend miss rather than a harness artifact.

Manually replaying one "empty" hybrid query outside the harness — TC-07
(`"parse_all_headings compute_section_end heading boundary markdown"`) — returned a
real result set containing **both** of TC-07's expected files
(`src/tools/markdown/edit_markdown.rs`, `src/tools/file_summary/file_summary.rs`) in
the top handful of hits. That query would have scored non-zero had the harness parsed
its own tool's response correctly. Root cause, fully reproduced: `scripts/run-tc-benchmark.py`'s
`semantic_search()` client assumes `read_file`'s paginated-buffer response is
JSON-encoded (`{"content": ..., "shown_lines": [...], "complete": ...}`); in reality
`read_file` renders that response as human-readable text
(`"{N} lines\n\n{content}"` — `src/tools/read_file.rs:789`). The resulting
`json.loads()` failure is silently swallowed (`scripts/run-tc-benchmark.py:320-323,
332-335`) and coerced to `top10_files: []` with **zero** warning output — the same
shape a genuine zero-hit query would produce. Full writeup, evidence, and fix plan:
`docs/issues/archive/2026-07-02-tc-benchmark-harness-swallows-buffered-results.md`.

This bug fires whenever a `semantic_search` response crosses codescout's ~10 KB
inline-buffering threshold (`MAX_INLINE_TOKENS` = 2,500 tokens,
`src/tools/core/types.rs:18`). The three arms cross that threshold at very different
rates — 17/25, 8/25, and 1/25 respectively — which tracks how much content each
config's top-10 tends to carry (hybrid's reranked/merged candidate set is the
heaviest; lite's dense-only top-10 is the lightest), **not** how often each config
actually found relevant code. The raw totals above therefore understate hybrid and
no-sparse quality by an unknown but likely substantial margin, while the lite total
(only 1 TC affected) is close to a true reading of that arm's performance.

### Where the sparse leg and reranker each cost

Comparing Arm A to Arm B in isolation (SPLADE on vs. off, rerank on in both) shows
adding the sparse leg coincides with a large score drop (26→9) and roughly a
doubling of latency (1022ms→1864ms p50) — but given the empty-result confound above,
most of that apparent drop is attributable to sparse-enabled responses crossing the
buffering threshold more often (larger merged candidate sets, BM25-boosted content),
not necessarily to SPLADE hurting retrieval per se. Comparing Arm B (rerank ON, 26/75) with Arm C (rerank OFF, 31/75) shows NO measurable rerank benefit on this confounded run — if anything an apparent cost of keeping it — so the reranker's true value remains unmeasured until the harness bug is fixed. No-sparse's empty-rate (8/25) is still elevated relative to lite's (1/25), so even
this narrower comparison isn't fully clean, but it's the least confounded pairing
available from this run.

### Latency

Latency numbers are trustworthy (latency isn't subject to the buffering-parse bug —
it's measured client-side around the whole call regardless of how the response body
parses). Lite is dramatically faster: p50 72.3ms vs. hybrid's 1863.6ms (~26×) and
no-sparse's 1021.7ms (~14×). This is expected — lite is a single in-process
dense-vector lookup with no network round-trips to a sparse embedder or reranker
container.

## Verdict

The measured 9/75 (hybrid) vs. 31/75 (lite) gap cannot be taken at face value: a
confirmed benchmark-harness bug (not a retrieval-quality defect) suppresses scores
for any TC whose top-10 result content crosses ~10 KB, and it suppresses hybrid's
score far more often (17/25 TCs) than lite's (1/25 TCs). The true quality gap between
hybrid and lite is unknown until the harness is fixed
(`docs/issues/archive/2026-07-02-tc-benchmark-harness-swallows-buffered-results.md` — Fix
section) and all three arms are re-run. What the data does support without caveat:
lite's per-query latency (72ms p50) is roughly an order of magnitude or more faster
than both Qdrant-backed arms, and lite's own score (31/75, only 1 TC affected by the
harness bug) represents dense-CodeRankEmbed-only retrieval quality reasonably
faithfully on this suite.

**Recommendation:** Do not use this run's raw totals to green- or red-light the lite
stack for VDI deployment — the comparison is confounded, not merely noisy. Before
making that call: (1) fix the harness's buffered-result reconstruction (documented
fix plan in the linked bug file) and re-run all three arms for a clean comparison; (2)
only then evaluate whether dense-only quality (currently ~41% of max score, 31/75) is acceptable — lite's single empty TC (TC-06) was not independently replayed the way TC-07/TC-08 were for hybrid, so the 31/75 reading is close to faithful but not verified clean — or whether the VDI's lite
deployment needs a stronger code-specific embedding model than CodeRankEmbed for the
dense leg. Separately — and independent of this confound — the harness bug itself
means every prior benchmark report in this repo that used `mode=full` and reported
scores for a Qdrant-hybrid or reranked arm should be treated with the same suspicion
until re-verified; see the cross-report timeline discrepancy noted in the bug file
(the reconstruction path may never have worked correctly, including in the
2026-05-06 `docs/research/2026-05-06-retrieval-stack-benchmark.md` report that
claimed to have fixed exactly this class of issue).

## Environment

- Repo HEAD: `cb8e77c6e38e20bd9303bc329f5bd696f03edd85` (branch `experiments`).
- Binary under test: `target/release/codescout`, build SHA
  `86292aec5665313d9d975d05bfb3eb02b5d1fe37` (**one commit behind HEAD, dirty tree** —
  `codescout_build_dirty: true` in all three arms' emitted config). The binary was not
  rebuilt against HEAD before this run; scores reflect the build as installed at
  benchmark time, not necessarily HEAD exactly.
- Endpoints (all local, AMD ROCm llama.cpp containers): dense embedder
  `http://127.0.0.1:48081` (CodeRankEmbed, 768d), sparse embedder
  `http://127.0.0.1:48084` (SPLADE), reranker `http://127.0.0.1:48083`
  (bge-reranker-v2-m3), Qdrant `http://127.0.0.1:6334`. `bm25_boost=3.0`,
  `retrieval_profile=amd`. No arm used a remote/VDI-representative embedding
  endpoint — `.env.lite`'s `CODESCOUT_EMBEDDER_URL` is an explicit unreachable
  placeholder (`https://embed.corp.example/v1`), so Arm C used the same local
  CodeRankEmbed dense endpoint as Arms A/B. This isolates architecture (hybrid vs.
  dense-only) from embedding-model choice, but means this run says nothing about a
  weaker or stronger remote code-embedding model's effect on the "lite" path — that
  is a separate open question the recommendation above defers explicitly.
- Project under test: `.worktrees/bench` (project id `code-explorer`, per its stale
  `.codescout/project.toml`, predating this repo's rename from `code-explorer` to
  `codescout`). `_git_sha()` returned empty string (`project_sha: ""` in all three
  configs) because `.worktrees/bench/.git` is a dangling gitlink pointing at
  `/home/marius/work/claude/code-explorer/.git/worktrees/bench` — a path that no
  longer exists post-rename. The worktree's content is otherwise intact and pinned
  (consistent with the historical 25-TC suite's 2026-05-12 baseline in
  `docs/research/2026-04-03-embedding-model-benchmark.md`); this is a known,
  long-standing environment quirk, not something fixed or worked around for this
  run.
- Qdrant `code_chunks` is a **shared/live** collection (no `collection_prefix` set in
  any arm, matching the brief's literal reproducer commands) filtered by
  `project_id` payload, not an isolated per-run collection. It was force-reindexed
  once before Arms A/B (`codescout index --project .worktrees/bench --force`,
  `added=24923 deleted=16180`) to clear staleness from the pre-rename content; this
  cleanup did not resolve the low hybrid score (a re-run after reindexing scored
  *lower*, 9/75 vs. an initial pre-reindex 11/75), which is what led to discovering
  the harness bug documented above.
- No arm was skipped — all three ran to completion with a full 25/25 TCs attempted.

## Known Issues Encountered During Benchmarking

1. **Benchmark-harness buffered-result bug (primary finding).**
   `scripts/run-tc-benchmark.py`'s `read_file`-pagination reconstruction assumes a
   JSON envelope that `read_file` does not actually emit (it emits human-readable
   text). Any `semantic_search` response over ~10 KB silently scores `top10_files: []`
   with no warning. Confirmed to have affected 17/25 hybrid TCs and 8/25 no-sparse
   TCs; confirmed via direct reproduction (TC-07 and TC-08) and via a line-for-line
   replay of the harness's own pagination loop against a live buffer. Full
   writeup: `docs/issues/archive/2026-07-02-tc-benchmark-harness-swallows-buffered-results.md`.
   Not fixed in this session (out of scope for Task 10 — docs-only deliverable).

2. **Dangling `.worktrees/bench` gitlink.** `.worktrees/bench/.git` points at
   `/home/marius/work/claude/code-explorer/.git/worktrees/bench`, a path that no
   longer exists after this repo's rename from `code-explorer`. `git -C
   .worktrees/bench status` fails with `fatal: not a git repository: (null)`. Worked
   around by using plain filesystem inspection (`ls`, `cat`, mtimes) instead of git
   commands against the worktree. This also means `_git_sha()` in the harness
   returns `""` for `project_sha` in every arm's emitted config — not a benchmark
   defect, but worth knowing when reading the raw JSON.

3. **Shared Qdrant collection staleness.** The live `code_chunks` collection (used
   because no arm set `--collection-prefix`) had accumulated 16,180 orphaned points
   from the pre-rename project content. A forced reindex before Arms A/B cleared
   this, but did not improve the hybrid score — ruling out staleness as the
   explanation for hybrid's low score and pointing investigation toward the harness
   bug above instead.

4. **`CODESCOUT_RERANKER_PROTOCOL` ambient/`.env.amd` mismatch — investigated and
   ruled out.** Ambient environment had `CODESCOUT_RERANKER_PROTOCOL=infinity`
   while `.env.amd` specifies `llama-server`; both map to the identical
   `Protocol::Infinity` enum variant in `src/retrieval/reranker.rs`, so this is not
   a functional difference and not a contributing cause.

## Raw Data

- Arm A (hybrid) JSON:
  `/tmp/claude-1000/-home-marius-work-claude-codescout/ef2bd921-8834-4a63-add4-0757afdc9883/scratchpad/hybrid.json`
  (timestamp `2026-07-02T12:33:59Z`, label `hybrid-baseline-reindexed`).
- Arm B (no-sparse) JSON:
  `/tmp/claude-1000/-home-marius-work-claude-codescout/ef2bd921-8834-4a63-add4-0757afdc9883/scratchpad/no-sparse.json`
  (label `no-sparse`).
- Arm C (lite) JSON:
  `/tmp/claude-1000/-home-marius-work-claude-codescout/ef2bd921-8834-4a63-add4-0757afdc9883/scratchpad/lite.json`
  (label `lite-sqlite-vec`).
- Scratch files live under this session's scratchpad and are not committed to the
  repo; the tables above are the durable record.
