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
- **CRITICAL CONFOUND — all of this is a DENSE-ONLY stack.** `.env.gpu` carries
  `CODESCOUT_DISABLE_SPARSE=1`, set by a concurrent session on 2026-07-28 (with its own
  measured rationale: SPLADE peaked at ~3.0 GiB of this 6 GiB card, costing -2/75). Both the
  `bench_base_` index and every benchmark run above therefore skipped the sparse leg at index
  *and* query time. So this finding is scoped to **dense + rerank vs dense alone** — which is
  the configuration currently live, and therefore the one that matters — but whether the
  reranker's cost/benefit changes under sparse fusion is **untested**.

  Consistency check that supports the numbers: the concurrent session independently measured
  dense-only at **32/75**, exactly matching the with-rerank score here (their run also had
  `CODESCOUT_RERANKER_URL` set, so it was dense+rerank too). Their fusion arm scored 34/75 —
  meaning **dense-only without rerank (35) beats dense+sparse fusion with rerank (34)** on this
  suite, though those are single runs from different sessions and not a controlled comparison.

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
   **Test:** repeated both configurations 4× each against the same `bench_base_` collection.
   **Verdict:** **REJECTED — the delta is real and has zero variance.**

   ```
   run          rerank score / p50      no-rerank score / p50
   1                32 / 2175.7 ms           35 /  51.3 ms
   2                32 / 2000.9 ms           35 /  56.8 ms
   3                32 / 2025.1 ms           35 /  52.9 ms
   4                32 / 1993.8 ms           35 /  60.5 ms
   ```

   The score is *exactly* 32 vs 35 on every run — which is the expected result on reflection:
   retrieval is deterministic given a fixed index and a fixed query, so the only variance is
   latency (rerank 1994-2176 ms, no-rerank 51-61 ms — 33× to 42×). Nothing about this finding
   is noise-limited.

## Fix

**Partially applied 2026-07-28: option 1 in `.env.gpu` only — this does NOT change the running
system.** `CODESCOUT_RERANKER_URL` and `CODESCOUT_RERANKER_PROTOCOL` are now commented out
there, with the measurements recorded inline. The `codescout-reranker-gpu` container is left
running so re-enabling is a one-line revert.

**Why it does not take effect, and why that is a second finding:** commenting out a line in
`.env.gpu` cannot unset a variable that is already exported in a process's environment. The
live MCP servers do not read `.env.gpu` at all — they carry their own env, set when Claude Code
launched them. Read from `/proc/<pid>/environ` on the oldest live server (up >1 day):

```
CODESCOUT_EMBEDDER_URL=http://127.0.0.1:48081
CODESCOUT_SPARSE_EMBEDDER_URL=http://127.0.0.1:48084
CODESCOUT_RERANKER_URL=http://127.0.0.1:48083     <- rerank ON
CODESCOUT_RERANKER_PROTOCOL=infinity              <- not the 'llama-server' .env.gpu sets
(no CODESCOUT_DISABLE_SPARSE)                     <- sparse ON
```

So the **live configuration is dense + sparse + rerank**, while the measurements above compare
**dense + rerank (32/75) against dense alone (35/75)**. The live config is a third arm that was
not measured. `PROTOCOL=infinity` vs `llama-server` is harmless — `Protocol::from_env` maps both
to `Protocol::Infinity` — but it is direct evidence that the two env sources drifted
independently, which is the same class as
`docs/issues/2026-07-25-env-copy-flow-stale-model-dir.md`.

**Nothing about the running system changed as a result of this bug file.** Making it take effect
requires restarting the MCP servers (`/mcp`), which is already an outstanding item for unrelated
reasons — 12 of them still run the pre-lock binary from before today's index-lock work.

The options as they stood:

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

The score and latency questions are settled **for the dense-only arm** (4 runs each, zero score
variance). Three things remain — note that item 0 was discovered while trying to apply the fix
and changes what this bug can claim:

0. **Measure the arm that is actually live: dense + sparse + rerank.** The live MCP servers have
   sparse enabled and rerank enabled; neither of the two arms benchmarked here matches that.
   The concurrent session's single-run figure for fusion+rerank is 34/75. To settle the
   reranker's value for the running system, index a second collection with sparse ENABLED and
   run both rerank arms against it. Until then, the 42× latency claim is established only for
   dense-only — though the mechanism (20 cross-encoder passes per query) is configuration
   independent, so the latency finding is very likely to hold under fusion too. The *score*
   finding may not.

1. **Re-test the reranker under sparse fusion**, if the sparse leg is ever re-enabled. Everything
   here was measured with `CODESCOUT_DISABLE_SPARSE=1`. A cross-encoder reordering hybrid
   candidates is a different problem from reordering pure-dense ones, and the conclusion could
   flip. The re-enable path is documented in `.env.gpu`: drop the flag, start `sparse-gpu`, then
   **reindex with force** — chunks written while the flag was set carry an empty sparse vector
   and will never match.
2. **Correct `docs/manual/src/concepts/retrieval-stack.md`'s "~80ms p95" claim** for this
   reranker. Measured p95 is 1994-3091 ms depending on the run — 25× to 38× the documented
   figure. That number is wrong regardless of which configuration is chosen.

Methodological note for whoever repeats this: both arms must run against the **same populated
collection**, and the sparse-disable state must be recorded in the label. Two separate
measurements in this session were initially mis-attributed because the ambient env had changed
underneath them — once for `DEFAULT_INFLIGHT` (measured while four duplicate indexers competed)
and once for indexing throughput (20.1 chunks/s here vs 4.4 on another corpus, which looked like
a corpus effect and was actually the sparse leg being disabled).
## References

- `src/retrieval/search.rs:24` — `overfetch: limit * 2`, the cost amplifier
- `src/retrieval/search.rs:84-110` — the rerank step and its graceful-degrade path
- `scripts/run-tc-benchmark.sh`, `scripts/run-tc-benchmark.py` — the 25-TC harness
- `docs/research/2026-05-06-retrieval-stack-benchmark.md` — Phase 5.5/6; records p50 169 ms
  without a reranker, and the explicit "re-test before committing to it for default" note
- `docs/issues/2026-07-27-reranker-gpu-tei-cuda-oom.md` — getting the reranker to run at all
- `docs/manual/src/concepts/retrieval-stack.md` — carries the incorrect "~80ms p95" claim
- commit `4036bb9a` — made the reranker the `gpu`-profile default
