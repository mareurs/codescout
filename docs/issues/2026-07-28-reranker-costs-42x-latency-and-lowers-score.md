---
status: open
opened: 2026-07-28
closed:
severity: high
owner: marius
related:
  - docs/issues/2026-07-27-reranker-gpu-tei-cuda-oom.md
tags: [retrieval, reranker, latency, benchmark, measured]
kind: bug
---

# BUG: the GPU reranker costs 42× query latency and *lowers* the benchmark score — strictly worse on both axes

## Summary

`bge-reranker-v2-m3` was brought up on the `gpu` profile on 2026-07-27 and wired in via
`CODESCOUT_RERANKER_URL`, but its effect on retrieval was never measured. Measured now on the
25-TC suite against the bench fixture: it makes queries **42× slower** (p50 51 ms → 2176 ms)
and **lowers the aggregate score** (35/75 → 32/75). It is strictly worse on both axes on this
hardware. The Phase 6 benchmark had explicitly flagged this as untested — *"Reranker: not
re-evaluated post chunk-tuning. Re-test before committing to it for default."* — and nobody
re-tested it before it became the default.

## Symptom (Effect)

Same collection (`bench_base_`, 24,923 chunks), same binary, same queries, back-to-back runs.
The only difference is whether `CODESCOUT_RERANKER_URL` points at a live reranker or a dead
port (which exercises the documented graceful-degrade path at
`src/retrieval/search.rs:106-110`).

```
                     WITH rerank    WITHOUT rerank
  score            :   32/75          35/75
  p50 latency (ms) :   2175.7          51.3      ← 42.4× faster without
  p95 latency (ms) :   3090.7          60.5

  TCs better without rerank: TC-02, TC-11, TC-12, TC-21, TC-25   (5)
  TCs worse  without rerank: TC-17, TC-22                        (2)
  net score delta          : +3 in favour of NO rerank
```

For scale, the Phase 5.5 matrix recorded **p50 169 ms** for this dense/chunk configuration
without a reranker. Today's no-rerank run at 51 ms is consistent with that (faster hardware
path and a smaller fixture); the 2176 ms with-rerank figure is 13× the historical number.

## Reproduction

```bash
cd /home/marius/work/claude/codescout
set -a && . ./.env.gpu && set +a

# with rerank (as shipped)
CODESCOUT_BINARY=./target/release/codescout \
CODESCOUT_PROJECT_PATH=./.worktrees/bench \
  ./scripts/run-tc-benchmark.sh --collection-prefix bench_base_ --label with-rerank

# without rerank — dead port exercises the graceful-degrade path
CODESCOUT_RERANKER_URL=http://127.0.0.1:1 \
CODESCOUT_BINARY=./target/release/codescout \
CODESCOUT_PROJECT_PATH=./.worktrees/bench \
  ./scripts/run-tc-benchmark.sh --collection-prefix bench_base_ --label no-rerank
```

Requires the `bench_base_` collection populated:
`CODESCOUT_QDRANT_COLLECTION_PREFIX=bench_base_ ./target/release/codescout index --project ./.worktrees/bench --force`
(24,923 chunks, 1239 s.)

## Environment

- codescout `0.15.0`, branch `experiments`, commit `5d3142e0`
- GPU: single GTX 1660 Ti, 6 GiB, hosting **three** models simultaneously — dense
  (llama-server, CodeRankEmbed-Q4_K_M), sparse (TEI SPLADE), reranker (llama-server,
  bge-reranker-v2-m3-Q4_K_M)
- Reranker: `codescout-reranker-gpu`, 340 MiB VRAM, `CODESCOUT_RERANKER_PROTOCOL=llama-server`
- Fixture: `.worktrees/bench`, 809 indexable files (486 md / 254 rs)
- Benchmark: `scripts/run-tc-benchmark.sh`, 25 TCs, `--limit 10`, scored 0-3 per TC

## Root cause

Not a defect in the reranker or its wiring — both work correctly. It is a **cost/benefit
failure on this hardware**, with an amplifier nobody accounted for:

`SearchOpts::new` sets `overfetch: limit * 2` (`src/retrieval/search.rs:24`), and
`search_in` reranks **every** overfetched candidate (`:89` collects all candidate contents,
`:91` sends them in one `rerank` call). At the benchmark's `--limit 10` that is **20
query-document pairs per query**, each a full forward pass through a 568M-parameter
XLM-RoBERTa-large cross-encoder — on a card already running two other models. 2176 ms / 20 ≈
**109 ms per pair**, which is a plausible per-pair cost for that model on a saturated Turing
card.

So the latency is not a misconfiguration; it is the designed behaviour meeting hardware that
cannot afford it. The score regression is separate and more interesting: on this corpus the
cross-encoder's reordering is net-negative, demoting correct hits more often than it promotes
them (5 TCs improve without it, 2 degrade).

## Evidence

Raw run artifacts (session scratchpad, not committed):
`bench-baseline.json` / `bench-baseline.log` (with rerank),
`bench-norerank.json` / `bench-norerank.log` (without).

Per-TC latency without rerank was 50-61 ms across all 25 TCs — tight and uniform. With rerank
it ranged 476-3900 ms. The uniformity of the no-rerank numbers rules out the fixture or Qdrant
as the variable.

The `.env.gpu` change that made this the default, and the compose work that brought the
reranker up, are in commit `4036bb9a`; the OOM investigation that preceded it is
`docs/issues/2026-07-27-reranker-gpu-tei-cuda-oom.md` (status `fixed` — the reranker *runs*
correctly, which is what that bug was about).

## Hypotheses tried

1. **Hypothesis:** the 2176 ms is reranker cold-start / model-load, not steady state.
   **Test:** examined per-TC latencies across the run — TC-01 was 1146 ms and later TCs ran
   1279-3900 ms with no downward trend.
   **Verdict:** rejected. There is no warmup curve; the cost is per-query.

2. **Hypothesis:** the score difference is measurement noise.
   **Verdict:** **not resolved, and stated as such.** n=1 per configuration, and ±3/75 is
   within plausible run-to-run variance for a 25-TC suite. The *latency* finding (42×) is far
   outside noise and stands on one run. Before acting on the score claim specifically, repeat
   both runs 3× — see Resume.

## Fix

Not yet applied — the decision is the operator's, and one option is cheap and reversible.

1. **Disable the reranker on this profile** (leave `CODESCOUT_RERANKER_URL` unset in
   `.env.gpu`). Recovers 42× latency and +3 score today. The container can keep running for
   experimentation. This is the option the measurements support.
2. **Shrink what gets reranked.** `overfetch: limit * 2` means rerank cost scales with the
   caller's `limit`. Reranking only the top-5 candidates instead of all 20 would cut the cost
   ~4× while retaining some reordering — but on this evidence the reordering is net-negative,
   so this optimises something that is not earning its keep.
3. **Move the reranker off this GPU** (CPU `bge-reranker-base`, or another card). Only worth
   doing if the score regression turns out to be noise and the reranker actually helps.

Whichever is chosen, `docs/manual/src/concepts/retrieval-stack.md` claims *"~80ms p95"* for
this reranker. Measured p95 is **3091 ms**, 38× that. That figure needs correcting regardless.

## Tests added

None — this is a measurement finding, not a code defect. The reproduction above is the test.
Worth considering: a CI-adjacent smoke check asserting `semantic_search` p50 stays under some
bound on the bench fixture would have caught a 42× regression on the day it landed.

## Workarounds

Unset `CODESCOUT_RERANKER_URL` (or point it at a dead port — `search_in` degrades gracefully
with a `reranker degraded` warning and returns unreranked candidates). No rebuild needed; the
MCP server picks it up on restart.

## Resume

Repeat both configurations **3× each** to settle whether the +3 score delta is real or noise
— that is the only open question, and it decides between fix options 1 and 3. The latency
finding needs no further confirmation. Then correct
`docs/manual/src/concepts/retrieval-stack.md`'s "~80ms p95" claim to the measured figure, and
decide the reranker's fate on the `gpu` profile.

Note the confound to avoid when re-running: both runs must use the **same** populated
collection (`bench_base_`), or chunking differences will contaminate the comparison.

## References

- `src/retrieval/search.rs:24` — `overfetch: limit * 2`, the cost amplifier
- `src/retrieval/search.rs:84-110` — the rerank step and its graceful-degrade path
- `scripts/run-tc-benchmark.sh`, `scripts/run-tc-benchmark.py` — the 25-TC harness
- `docs/research/2026-05-06-retrieval-stack-benchmark.md` — Phase 5.5/6; records p50 169 ms
  without a reranker, and the explicit "re-test before committing to it for default" note
- `docs/issues/2026-07-27-reranker-gpu-tei-cuda-oom.md` — getting the reranker to run at all
- `docs/manual/src/concepts/retrieval-stack.md` — carries the incorrect "~80ms p95" claim
- commit `4036bb9a` — made the reranker the `gpu`-profile default
