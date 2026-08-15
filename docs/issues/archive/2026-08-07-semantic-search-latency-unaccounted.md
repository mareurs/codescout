---
id: e4087db51d3d3bd6
kind: bug
status: fixed
title: 'BUG: semantic_search end-to-end is ~990 ms while the measured retrieval stages sum to ~32 ms — roughly 950 ms is unaccounted for'
tags:
- retrieval
- performance
- latency
- mcp
closed: 2026-08-15
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

**Measured 2026-08-15 on `HEAD` (`89f5d591`), live stack, warm, median of 5** — one
MCP session, four cases over the same connection, so per-call cost is isolated from
process startup:

| case | median ms |
|---|---|
| `semantic_search` limit=10 | **126.9** |
| `semantic_search` limit=1 | **122.2** |
| `tree` depth=1 | **39.5** |
| `tools/list` (no tool dispatch) | **1.0** |
| query embedding, direct HTTP to the embedder | **3.6** |

**The premise no longer reproduces.** `semantic_search` is ~127 ms, not ~990 ms —
roughly 8× faster than when this was filed, with the reranker off in both cases.

Against the four candidates:

1. **Harness overhead — partly true, and now the dominant identified slice.** The
   JSON-RPC floor is ~1 ms, but tool dispatch + context is ~39 ms (`tree depth=1`,
   which does trivial work). That ~39 ms is common to *every* tool and has nothing
   to do with retrieval. It is now the largest single named component of a
   `semantic_search` call.
2. **Overfetch payload transfer — REFUTED.** limit=10 costs 4.7 ms more than
   limit=1. If discarded payload dominated, the two would differ by far more. Cost
   is essentially fixed per call, not per hit.
3. **Stale stage figures — moot.** Whatever the stages were, the total is now 127 ms.
4. **Per-call project re-resolution** — not separately measured; it is inside the
   ~39 ms dispatch figure, which bounds it.

**Query embedding is not the gap either** — 3.6 ms direct to the endpoint
(`/v1/embeddings`; note `/embed` 404s, the stack is OpenAI-protocol).

Accounting the current 127 ms: ~1 transport + ~39 dispatch + ~4 embedding + ~32
historical retrieval stages leaves **~50 ms genuinely unattributed**. That is a
much smaller and much less alarming residual than 950 ms, and it is a different
question from the one this file asked.
## Fix

No code change here. The condition this file reports — ~950 ms unaccounted — does
not exist on `HEAD`; something in the intervening cohort removed it.

**Leading candidate, stated as a candidate and not a conclusion:** `feac9539`
*"ask whether the project has chunks, instead of counting them all to find out"*.
`check_has_index` was answering a yes/no by enumerating **every chunk in the
corpus**, and its two siblings in the same commit passed `with_payload(true)` on
scrolls that read one or two keys. The *shape* matches this bug's evidence exactly:
a large cost, independent of `limit`, scaling with corpus rather than result count.

**What stops that being a conclusion:** `check_has_index` runs on *activation*, not
per query, so on its own it does not explain a per-query figure. Attributing the
drop properly would mean bisecting the cohort with the benchmark, which is not
worth doing to explain a number that is no longer wrong. Recorded so a future
session does not mistake the candidate for a measurement — the mistake this file's
own § Root cause was already careful to flag about its inherited stage figures.
## Tests added

None — a latency figure is not a regression test worth pinning. A threshold assert
would be an environment-specific constant, which this project's conventions
explicitly forbid (`feac9539` makes the same argument about `FIRST_PROBE_TIMEOUT`:
*"the fix is to make the question cheap, not the deadline generous"*).

The measurement method is reproducible instead: one MCP session over
newline-delimited JSON-RPC, four cases timed on the same connection, warmed by one
discarded call, median of five. The discriminating design is the **trivial-tool
control** (`tree depth=1`) and the **`tools/list` floor** — without them a 127 ms
number says nothing about whether the cost is retrieval or dispatch.
## Workarounds

None needed — 990 ms is usable, and turning the reranker off (now the default) already removed
~569 ms from the previous figure. This bug is about knowing where the remaining time goes, not about
an outage.

## Resume

Closed — the reported condition is gone, measured rather than assumed.

One observation worth its own look, and deliberately not folded in here: **a
trivial tool call costs ~39 ms of dispatch + context**, and that is paid by every
tool on every call, not by `semantic_search` specifically. At this point it is the
largest identified component of a search. That is a different question with a
different blast radius, and filing it as "semantic_search is slow" would repeat
exactly the mis-attribution this file was opened to correct.

The method generalises better than the result: **time the subject against a
trivial control on the same connection.** This file's leading hypothesis was
"the harness measures the round-trip" — correct in spirit, and the control is what
turns that from a suspicion into a 1 ms / 39 ms / 127 ms breakdown.
## References

- `docs/issues/archive/2026-07-28-reranker-costs-42x-latency-and-lowers-score.md` — where this was
  found, and the full A/B data
- `docs/manual/src/concepts/retrieval-stack.md` § *Stack-wide latency (champion config)* — the stage
  table whose figures are inherited here
- W-14 in `docs/trackers/release-promotion-session-log.md` — why the first query is excluded
