---
id: e4087db51d3d3bd6
kind: bug
status: open
title: 'BUG: semantic_search end-to-end is ~990 ms while the measured retrieval stages sum to ~32 ms — roughly 950 ms is unaccounted for'
tags:
- retrieval
- performance
- latency
- mcp
---

# BUG: `semantic_search` end-to-end is ~990 ms while the measured retrieval stages sum to ~32 ms

## Summary

Split out of `docs/issues/archive/2026-07-28-reranker-costs-42x-latency-and-lowers-score.md`, which
surfaced it while measuring something else and would have buried it on archival. With the reranker
**off**, a `semantic_search` round-trip measures a **990 ms warm median**, while the retrieval stages
that can be measured individually add up to roughly **32 ms**. Nothing established where the other
~950 ms goes, and until that is known, optimising the retrieval legs is optimising the wrong thing.

## Symptom (Effect)

Measured 2026-08-07 on the AMD profile, against the index rebuilt the same day (100% sparse
coverage), reranker off, 25-TC suite via `scripts/run-tc-benchmark.py`:

```
warm median end-to-end : 990 ms   (first query excluded — it pays cold-start, see W-14)
warm mean              : 972 ms
warm min / max         : 644 / 1065 ms
```

Against the stages that were measured directly:

| stage | measured | source |
|---|---|---|
| sparse embed (single query) | **16.8 ms** | `codescout-sparse-amd`'s own `embed_sparse` span, warm |
| dense embed (single query) | ~5 ms | `docs/manual/src/concepts/retrieval-stack.md` stage table (GPU) |
| Qdrant hybrid search (RRF) | ~10 ms | same table |
| **sum** | **~32 ms** | |

So ~**950 ms**, about 97% of the wall time, is outside the retrieval stages.

## Reproduction

```bash
cargo rb            # server-stack build — a plain `cargo build --release` is lean (F-17)
python3 scripts/run-tc-benchmark.py --project-path "$PWD" --binary ./target/release/codescout \
  --label latency-probe
# read aggregate.p50_latency_ms, and the per-TC latency_ms array; DISCARD the first entry
```

`git rev-parse HEAD` at time of measurement: `6ce49487` (experiments).

## Environment

AMD ROCm profile (`.env.amd`), sparse fusion on, reranker off (the new default). Qdrant 6334, dense
48081, sparse 48084. Host: 125 GiB RAM, AMD 16 GiB card + idle RTX A5000.

## Root cause

**Unknown — see Hypotheses tried.** No stage-level attribution has been done; the numbers above are
the only measurements, and three of the four are inherited from a doc table rather than measured in
this run.

*Measured 2026-08-07:* `embed_sparse` 16.8 ms warm, from the container's own span. *Inferred, not
measured:* the dense and Qdrant figures, which come from the manual's stage table and have not been
re-measured on this stack. That distinction matters — if either is stale by an order of magnitude the
gap shrinks accordingly, and re-measuring them is cheaper than instrumenting the whole path.

## Evidence

The arms that produced these numbers are recorded in
`docs/issues/archive/2026-07-28-reranker-costs-42x-latency-and-lowers-score.md` §
*Measurement 2026-08-07 (later)*, together with the check that the arms genuinely differed (reranker
container request deltas of +3553 vs +2).

## Hypotheses tried

None yet. Candidates, in rough order of how cheap they are to test:

1. **The harness measures the MCP round-trip, not retrieval.** `run-tc-benchmark.py` times a full
   JSON-RPC call over stdio to a child `codescout start`, including tool dispatch, `OutputGuard`
   formatting, and serialisation of 10 hits with their `content`. This is the leading candidate
   purely because it is the largest thing in the path that is not retrieval.
2. **Overfetch payload transfer.** `SearchOpts::new` sets `overfetch = limit * 2`, so 20 candidates
   are fetched from Qdrant *with their content payload* and 10 are discarded. Chunk content at
   `chunk_target=1200` makes that a non-trivial transfer.
3. **The inherited stage figures are stale.** See § Root cause — two of the three are unverified on
   this stack.
4. **Project activation / per-call state.** Whether anything per-call re-resolves project state.

## Fix

None yet — this is a measurement task before it is a fix task. The code already has the seams:
`search_in` calls `timer.lap("vector_query")` and `timer.lap("rerank")`, so adding laps around the
embed call and around response formatting would attribute the gap without new machinery. Do that
first; do not optimise a stage before it is implicated.

## Tests added

N/A — nothing is fixed yet. A latency regression test is deliberately not proposed: wall-clock
assertions are what W-30/WIN-30 and the `std::time::Instant` vs `tokio::time::Instant` trap
(`docs/issues/2026-08-07-windows-ci-timing-flakes-block-the-gate.md`) are about.

## Workarounds

None needed — 990 ms is usable, and turning the reranker off (now the default) already removed
~569 ms from the previous figure. This bug is about knowing where the remaining time goes, not about
an outage.

## Resume

Add `timer.lap(...)` calls around (a) the embed call and (b) response formatting in
`src/retrieval/search.rs::search_in`, then run one query and read the lap breakdown. That single run
either implicates hypothesis 1/2 or eliminates both, and costs minutes. Re-measure the dense and
Qdrant stages on this stack in the same pass so § Root cause stops relying on the manual's table.

## References

- `docs/issues/archive/2026-07-28-reranker-costs-42x-latency-and-lowers-score.md` — where this was
  found, and the full A/B data
- `docs/manual/src/concepts/retrieval-stack.md` § *Stack-wide latency (champion config)* — the stage
  table whose figures are inherited here
- W-14 in `docs/trackers/release-promotion-session-log.md` — why the first query is excluded

