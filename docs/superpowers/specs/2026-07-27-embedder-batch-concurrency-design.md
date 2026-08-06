# Design — Embedder batch sizing + bounded request concurrency

**Status:** design approved 2026-07-27 · branch `experiments` · **two sequenced stages**
(Stage 1 index lock, Stage 2 batch+concurrency) after the 2026-07-27 four-indexer discovery
**Bugs:** [[2026-07-27-ast-chunker-no-minimum-chunk-size]] (same symptom — slow index —
different cause; this spec addresses the throughput half, the chunk floor is deferred)
**Related:** [[2026-07-27-indexer-walks-git-and-tool-state-dirs]],
`docs/research/2026-05-06-retrieval-stack-benchmark.md`

## Problem

A full re-index of `backend-kotlin` (43,582 chunks) has been running for over 2.5 hours at
roughly **5.8 chunks/sec**. The Phase 5.5 benchmark synced 21,115 points in 185 s — about
**114 chunks/sec**. Some of that gap is hardware (the benchmark ran on a faster AMD card,
and its `Sync (s)` column may exclude the sparse pass), but not a 20× factor.

The cause is request framing, not model speed. `EmbedderHttp::embed_batch`
(`src/retrieval/embedder.rs:328-440`) issues **8 inputs per HTTP request, one request at a
time**:

```rust
// The sparse (SPLADE/TEI) server caps client batches at 8
// (HTTP 422 "batch size N > maximum allowed batch size 8"), so keep
// both the dense and sparse legs at or below that limit.
const BATCH: usize = 8;
for chunk in texts.chunks(BATCH) { ... }   // sequential await
```

**That comment is stale.** The running sparse server reports:

```
max_client_batch_size : 32
max_input_length      : 512
max_batch_tokens      : 16384      (32 × 512 — the cap is the token budget)
max_concurrent_requests: 512
```

The only service that ever imposed 8 was `sparse-amd`, which set
`--max-client-batch-size 8` explicitly. That service was deleted from
`docker-compose.yml` on 2026-07-27. The GPU profile never had the cap, so the client has
been running at a quarter of the server's per-request capacity, against a dense server
provisioned `--parallel 16 --batch-size 4096 --ubatch-size 4096` that logs
`all slots are idle` between requests.

### Measurements

Batch sweep against the live servers, 2026-07-27. **Taken while the index was running**, so
absolute values are a pessimistic floor; the ordering and saturation points reproduced
across repetitions.

```
sequential, one request at a time
 batch    dense s  d chunks/s    sparse s  s chunks/s
     8      0.222        36.0       7.913         1.0
    16      1.515        10.6       8.178         2.0
    32      2.030        15.8      11.300         2.8
    64      3.124        20.5         HTTP 413 Payload Too Large
   128      6.373        20.1         HTTP 413 Payload Too Large

concurrency at batch 32
  dense    conc=1 16.3/s   conc=2 32.0/s   conc=4 32.2/s   conc=8 28.2/s
  sparse   conc=1  2.7/s   conc=2  8.7/s   conc=4  9.2/s   conc=8  9.2/s
```

Three conclusions:

1. **SPLADE is the bottleneck**, 4–10× slower than dense at every batch size. Because
   `embed_batch` already runs the two legs concurrently under `tokio::try_join!`, the
   pipeline moves at sparse's pace — dense-side tuning buys nothing.
2. **Sparse hard-caps at 32 per request.** TEI signals a `max_client_batch_size` violation
   with `413`, not `422` as the stale comment says. The 64-input probe carried roughly 16 KB
   against the server's `payload_limit: 2000000`, so the rejection is the batch cap, not a
   byte-size limit.
3. **Concurrency saturates at 4** on both legs; at 8 both regress.

Non-monotonic dense readings (36/s at batch 8 vs 10.6/s at 16) are contention noise.

## Scope

**In scope — Stage 1 (lands first, see the addendum at the end of this doc)**

0. A per-project index lock so concurrent `codescout index` runs on one project refuse
   instead of duplicating the entire embedding workload.

**In scope — Stage 2**

1. Lazy discovery of the sparse server's `max_client_batch_size` via `GET {sparse_base}/info`.
2. Bounded, **order-preserving** concurrent issue of sub-batches inside
   `EmbedderHttp::embed_batch`.
3. Extraction of the existing loop body into a testable `embed_one_batch`.
4. Env overrides `CODESCOUT_EMBED_BATCH` / `CODESCOUT_EMBED_INFLIGHT`.
5. A clearer `413` error naming both the attempted size and the discovered cap.

**Out of scope**

- **The chunk floor / sibling merge.** Deferred deliberately: it changes embeddings and so
  needs score re-validation against the retrieval benchmark, whereas this change does not.
  Measure this first, then decide what the floor is still worth. Tracked in
  [[2026-07-27-ast-chunker-no-minimum-chunk-size]].
- **Raising TEI's `--max-client-batch-size`** (and `--max-batch-tokens` alongside it, since
  the cap is `max_batch_tokens / max_input_length`) to lift the 32 ceiling. Possible later;
  the sweep shows concurrency saturating before the per-request cap becomes binding.
- **Moving SPLADE off this GPU**, or dropping sparse from the index path.
- Any change to `stream_index`, `flush_pending`, `flush_batch`, the chunker, or the payload.

**Decisions locked (2026-07-27)**

- Concurrency lives **in `EmbedderHttp::embed_batch`**, not in `stream_index` — keeps the
  `flush_batch=256` memory bound (a 68 GB OOM scar,
  `docs/issues/archive/2026-06-19-mcp-server-oom-68gb.md`) untouched, and benefits every
  caller.
- Batch size is **discovered from `/info`**, not hardcoded — hardcoding is what produced
  this bug.
- `CODESCOUT_EMBED_BATCH` **wins outright** over the discovered cap. It is an escape hatch
  for a server whose `/info` lies or is absent; a wrong value surfaces as a clear `413`.
- `inflight` default **4**, from the sweep's saturation point. Measured under contention —
  re-validate on an idle card (see Verification).

## Retrieval quality is unchanged by construction

Same chunks, same texts, same models — therefore byte-identical vectors. llama-server
mean-pooling and TEI SPLADE both embed each input independently of batch composition, and
`--auto-truncate` applies per input at `max_input_length: 512`. Only request framing
changes.

This is the property that makes the change cheap to land: **no benchmark re-run is required
to accept it**, unlike the deferred chunk floor. Verification is a timing exercise, not a
quality exercise.

## Mechanism

### a. Lazy batch-cap discovery

`EmbedderHttp::new` is **synchronous** (`src/retrieval/embedder.rs:120`), so an async probe
cannot run in the constructor. Discovery is lazy via a new field:

```rust
sparse_batch_cap: tokio::sync::OnceCell<usize>,
```

Resolved on the first non-`dense_only` `embed_batch` call:

```
CODESCOUT_EMBED_BATCH  →  /info max_client_batch_size  →  8
```

The `8` fallback covers a non-TEI sparse server, a `/info` 404, a timeout, or a parse
failure — preserving today's behaviour exactly in every case where discovery cannot answer.
The effective value is logged once at `info`.

For the `dense_only` path there is no sparse server to probe: use the env override if set,
else a default of 32 (the sweep showed dense healthy through 128; 32 is deliberately
conservative).

### b. Bounded concurrent issue

Extract the current loop body verbatim — the `try_join!` of dense and sparse, the
empty-string omit-and-re-expand, the dim check, the retry loop — into:

```rust
async fn embed_one_batch(&self, inputs: &[&str]) -> Result<Vec<EmbedOutput>>
```

`embed_batch` becomes a driver:

```rust
futures::stream::iter(texts.chunks(batch).map(|c| self.embed_one_batch(c)))
    .buffered(inflight)          // buffered, NOT buffer_unordered
    .try_collect::<Vec<Vec<EmbedOutput>>>()
    .await
    .map(|v| v.concat())
```

`futures = "0.3"` is already a dependency (`Cargo.toml:148`).

### c. The invariant that must not break

`flush_pending` zips embeddings back onto payloads **positionally** — the test module notes
*"length matches `texts` so the zip in `flush_pending` stays aligned."* Misordering attaches
every vector to the wrong chunk: no error, no crash, a silently corrupt index that only
shows up as degraded search.

`buffered` preserves input order. `buffer_unordered` does not. This is the single
highest-risk line in the change, and a length-only assertion passes under both.

## Error handling

- The retry loop (424/429/5xx, 8 attempts, exponential backoff capped at ~6.4 s) moves
  inside `embed_one_batch` unchanged. Concurrency raises contention on the shared sparse
  server, so it becomes more load-bearing, not less.
- `413` remains non-retryable — correct, since it is permanent for a given batch size. Its
  message gains the attempted size and the discovered cap, because with discovery in place a
  `413` specifically means the cap is wrong.
- `try_collect` short-circuits on the first error and drops in-flight requests. Failure
  granularity is unchanged: the flush fails, the index errors.
- Peak in-flight memory rises from 8 to `inflight × batch` = 128 chunks, roughly 400 KB of
  f32 plus sparse pairs. The caller's `flush_batch=256` bound is untouched.

## Testing

1. **Order preservation** — the critical test. A mock embedder returns a vector encoding its
   input's index; assert output order matches input order across several sub-batches with
   `inflight > 1`. Must be written so it **fails under `buffer_unordered`** — otherwise it
   is not testing the thing that can break.
2. **Discovery** — `/info` returns 32 → batch 32; `/info` 404 → batch 8; `/info` times out →
   batch 8; `CODESCOUT_EMBED_BATCH` set → wins over both. Uses the existing `EnvGuard`
   pattern (`docs/conventions/test-env-isolation.md`).
3. **Behaviour preservation** — empty-string omit-and-re-expand alignment, dim-mismatch
   error, `dense_only` path, and the existing `src/retrieval/sync.rs` tests
   (`stream_index_force_reembeds_all_present_chunks` and siblings) all still pass.
4. **413 message** — asserts the error text contains both the attempted size and the cap.

## Verification

**MEASURED 2026-07-27 on an idle card. The result contradicts this spec's premise: Stage 2
delivered no throughput gain, and its concurrency setting is actively harmful on this hardware.
Stage 1 (the index lock) was the entire win.**

### What the sparse server actually does with real chunks

`--force` re-index of `backend-kotlin` (55,815 chunks), single process, new binary. The log
confirms discovery works — `embed batch size batch=32 source="info"` — and 180 s of steady state:

```
dense   768 chunk-embeddings / 24 requests  = 32.0 chunks/request, 4.27 chunks/s
sparse   25 requests × 32                   = 800 chunks,          4.44 chunks/s
```

Per-request sparse timings, the decisive evidence:

```
total_time 32.9s   queue_time 23.5s   inference_time  7.1s
total_time 26.6s   queue_time 14.5s   inference_time 12.1s
total_time 23.5s   queue_time 16.9s   inference_time  6.6s
```

`inference_time` of 6.6–12.1 s for 32 inputs **is** the SPLADE ceiling: ~2.7–4.8 chunks/s,
GPU-bound. Batch size and client concurrency cannot move it — it is the same tokens through the
same card.

### Three corrections to this spec

**1. The batch sweep in "Measurements" above was unrepresentative and its absolute numbers do not
transfer.** It used a ~260-char / ~65-token sample and measured sparse at 20.6 chunks/s. Real
corpus chunks cost 4–5× more, giving 4.4 chunks/s. Only the *ordering* conclusion survived —
sparse is the bottleneck, dense is not. Repeating the sweep on an idle card confirmed the flatness
that the contended run had hidden: sparse measured 20.5 / 20.8 / 20.6 chunks/s at batch 8 / 16 / 32,
and 20.5 / 20.9 / 20.7 / 20.7 at concurrency 1 / 2 / 4 / 8. **Flat in both dimensions.**

**2. Raising the batch 8 → 32 bought nothing measurable.** It cut HTTP requests 4× (24 rather than
~96 per 180 s), which is negligible overhead against a 7–12 s inference. Not harmful; not a win.

**3. `DEFAULT_INFLIGHT = 4` should be 1 (or 2). It has two costs and no benefit.**

- **Latency:** `queue_time` is 14.5–23.5 s of each request's 23–33 s total ≈ `(inflight − 1) ×
  inference_time`. TEI serialises on the GPU, so 4 in flight means 3 sit in a queue.
- **VRAM:** SPLADE went from **374 MiB idle to 2710 MiB** — 7×. It projects every token to a
  vocab-sized `[tokens × 30522]` f16 tensor, so peak memory scales with concurrent tokens; four
  in-flight 32-input batches multiply it. Card total reached 3783 / 6144 MiB. Exactly the hazard
  the deleted `sparse-amd` service documented ("TEI's default 16384 drives ~13.7 GiB on this
  16 GiB card") and that this plan recorded as unexplained.

The `4` came from the *contended* sweep, where it appeared to lift sparse from 2.7 to 9.2
chunks/s. That was concurrency relieving contention between four competing indexers — not
concurrency helping a single client. A measurement taken under the bug being fixed.

### Where the real gain came from

Before: ~5.8 chunks/s aggregate, split across **four** indexers each doing the same full pass ⇒
roughly **1.45 chunks/s of useful progress**. After: **4.45 chunks/s**, single pass.

**~3× improvement, entirely attributable to Stage 1's per-project lock.** The GPU was saturated
in both cases; duplication was wasting three quarters of it.

### What is still on the table

At 4.45 chunks/s a full `--force` re-embed of 55,815 chunks projects to ~3.5 h. The deferred chunk
floor is now the only remaining lever with real headroom: 34% of chunks span ≤5 lines and 12% are
single lines (`docs/issues/2026-07-27-ast-chunker-no-minimum-chunk-size.md`). Removing that
population would cut roughly an hour from a full pass, and unlike batching it is a genuine
reduction in work rather than a re-framing of the same work.

Retrieval quality was not re-checked, and does not need to be: vectors are byte-identical by
construction (per-input embedding, per-input truncation), which is why this section is a timing
exercise. That property held — the only change is request framing.
## Addendum — Stage 1: per-project index lock

Added 2026-07-27 after discovering **four** concurrent `codescout index` processes on
`backend-kotlin` (3h24m / 2h02m / 1h08m / 1h05m), all orphaned to `systemd --user`. See
[[../../issues/2026-07-25-concurrent-index-no-project-lock]].

This lands **before** the batching work for two reasons: duplication costs more than a 4x
batch improvement recovers, and every throughput number in this spec was measured while
four clients shared the servers, so a clean Stage 2 measurement is impossible until
duplication is impossible.

### Mechanism

Follow the mux precedent (`src/lsp/mux/process.rs:75-79`) rather than the write-guard
precedent:

```rust
let lock_file = std::fs::File::create(&index_lock_path)?;
lock_file.try_lock_exclusive()
    .context("another codescout index is already running for this project")?;
writeln!(&lock_file, "{}", std::process::id())?;   // PID for diagnostics
```

Acquired in `RetrievalClient::sync_project` (`src/retrieval/sync.rs:196`) **before**
`chunk_refs`, so the drift baseline is read under the lock. Released on drop / process exit.

### Two decisions that matter

**Use a dedicated `.codescout/index.lock`, NOT the existing `.codescout/write.lock`.**
`write.lock` is taken per write-tool call by `WriteGuard` (`src/agent/write_guard.rs`). An
index holding it for hours would block every edit tool for the whole run — strictly worse
than the bug being fixed. The two locks protect different things and must stay separate.

**Fail fast, do not queue.** `try_lock_exclusive` and exit with a clear message, rather than
blocking until the other run finishes. A queued second run is nearly free (it would find
every `chunk_id` already present and skip re-embedding), but it hides the duplication that
caused this bug instead of surfacing it. On the MCP path, surface as `RecoverableError` so
it renders as `isError: false`, per `get_guide("error-handling")`.

### Testing

1. Two `sync_project` calls against the same project root — second returns the
   lock-held error, does not embed. Assert via a recording embedder that it saw zero calls.
2. Different project roots do not contend.
3. Lock is released after a successful run, and after a panic (RAII, not explicit unlock).
4. A stale lock file left by a killed process does not block a new run — `flock` is released
   by the kernel on process death, so this should pass without special handling; the test
   pins that property so nobody "fixes" it by adding PID-liveness checks.

### Verification

`pgrep -af 'codescout index'` returns at most one process per project while a second
invocation is attempted. This command is also the first diagnostic step for any future
"indexing is slow" report — a per-process `ps -o etime -p <pid>` view hid the duplication
for the entire investigation that produced this spec.

## Follow-ups

- Decide the chunk floor's fate once this is measured
  ([[2026-07-27-ast-chunker-no-minimum-chunk-size]]).
- `RawChunk.metadata` is documented as a *"searchable header prepended before embedding"*,
  but `stream_index` discards it — every payload carries `ast_header: ""`
  (`src/retrieval/sync.rs:157-158`, confirmed against live qdrant payloads). Either the
  header is dead weight in the chunker or it is a missing retrieval signal. Out of scope
  here; worth its own bug.
- `src/retrieval/sync.rs:167` cites `docs/issues/archive/2026-06-19-mcp-server-oom-68gb.md`, but that
  bug was archived to `docs/issues/archive/`. A one-line doc-ref fix; noted here because the
  same stale path would otherwise have been copied into this spec.
- `LanguageSpec::inner_node_types` documents size-gated recursion (*"when a container node
  is too large"*) while `nodes_to_chunks` recurses unconditionally. The doc is stale
  relative to `aa6bff1d` (2026-04-21). Fix alongside the chunk floor.
