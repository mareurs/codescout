---
status: open
opened: 2026-08-26
closed:
severity: high
owner: marius
related: [docs/issues/2026-08-26-onnx-local-path-lacks-coderankembed.md]
tags: [embeddings, memory, silent-failure, retrieval]
kind: bug
---

# BUG: oversized dense-embed payloads are dropped silently — memory writes report `ok` with no vector

## Summary

A memory or chunk whose text exceeds the dense embedder's per-request token
ceiling gets an HTTP 400 from llama-server. `cross_embed_memory` propagates the
error, but its **caller downgrades it to a warning and still returns
`status: "ok"`** — so the markdown lands on disk while the vector never reaches
the semantic store. The memory is then invisible to `recall` forever, and
nothing marks it as needing repair.

Imported from GitHub issue #15 (reporter: mic-urs, 2026-08-20), including the
reporter's own follow-up correction, which inverts the prescribed fix.

## Symptom (Effect)

The write succeeds. The only trace is a non-fatal warning in the response:

```
"cross-embed failed: dense openai status"
"semantic anchor creation failed: dense openai status"
```

The warning names neither the payload size nor the HTTP body, so it is not
self-diagnosing. Reporter's direct probe against the running server:

```json
POST /v1/embeddings  (~4.5KB input)
HTTP 400 {"error":{"code":400,"message":"input (1682 tokens) is larger than the max context size (1024 tokens). skipping","type":"exceed_context_size_error"}}
```

## Reproduction

1. Point `CODESCOUT_EMBED_URL` at a llama-server started with
   `--ctx-size 8192 --parallel 8` (the CPU-stack default → 1024 tokens/slot).
2. `memory(action="write", topic="big", content=<~4KB of prose>)`.
3. Observe `status: "ok"` with a `cross-embed failed` warning.
4. `memory(action="recall", query=<a phrase unique to that content>)` — no hit.

## Environment

- codescout `experiments`, build 2026-08-19 (reporter) — mechanism re-verified
  at `d5ed4d6f` on 2026-08-26.
- `CODESCOUT_VECTOR_BACKEND=sqlite-vec`; dense = llama-server CPU
  (`CodeRankEmbed-Q4_K_M.gguf`), ports 48081 / 48083 / 48084.

## Root cause

Two independent mechanisms, both read from the code on 2026-08-26 (the HTTP 400
itself was measured by the reporter, not by this session):

1. **No segmentation.** `src/tools/memory/mod.rs:340-371` — `cross_embed_memory`
   calls `embed_document(content)` on the *whole* content in one request. There
   is no chunk/segment/pool step, so payload size is bounded only by what the
   caller happens to write. `inferred from src/tools/memory/mod.rs:358 — not
   measured at runtime this session.`
2. **The failure is swallowed.** `src/tools/memory/mod.rs:735-737`:
   ```rust
   if let Err(e) = cross_embed_memory(ctx, topic, content).await {
       warnings.push(format!("cross-embed failed: {e}"));
   ```
   The `Result` becomes a warning string; the tool still returns `ok`. The
   `{e}` rendering drops the HTTP response body, which is where
   `exceed_context_size_error` and the real token counts live.

### The ceiling is the model's, not the config's — this inverts the fix

The reporter's follow-up comment (2026-08-20) is the load-bearing correction and
the reason a config-only fix must not be shipped:

> After recreating the dense container with `--ctx-size 65536 --parallel 8`
> (8192/slot), a 3000-token payload still fails with `max context size (2048
> tokens)` — the real ceiling is the model's own training context
> (CodeRankEmbed `n_ctx_train` = 2048), which llama.cpp enforces regardless of
> `--ctx-size`.

So `ctx_size / parallel` is only the *first* ceiling. Raising it lifts the
practical limit from 1024 to 2048 tokens and no further. **No server
configuration can push past `n_ctx_train`** — segmentation in codescout is the
only fix that actually closes the hole. This is exactly the CLAUDE.md
"run the reproduction before reading the fix plan" case: the issue's own first
suggested fix (raise `--ctx-size`) would have shipped a partial fix that reads
as a complete one.

## Evidence

### Live call site (`src/tools/memory/mod.rs`, grep 2026-08-26)

```
340: async fn cross_embed_memory(ctx: &ToolContext, topic: &str, content: &str) -> anyhow::Result<()>
735: if let Err(e) = cross_embed_memory(ctx, topic, content).await {
737: warnings.push(format!("cross-embed failed: {e}"));
```

### Body of the embed call (`symbols include_body`, 2026-08-26)

```rust
let dense = ctx.agent.memory_embedder().await?
    .embed_document(content)   // whole content, one request, no segmentation
    .await?;
```

## Hypotheses tried

1. **Hypothesis:** raising `--ctx-size` fixes it.
   **Test:** reporter recreated the container at `--ctx-size 65536 --parallel 8`
   (8192/slot) and re-sent a 3000-token payload.
   **Verdict:** rejected — still HTTP 400, now naming a 2048-token ceiling.
   `n_ctx_train` is enforced independently of `--ctx-size`.
   **Evidence:** GitHub #15 comment 5354817007.
2. **Hypothesis:** the caller surfaces the failure loudly enough to notice.
   **Verdict:** rejected — `src/tools/memory/mod.rs:735-737` downgrades it to a
   warning and the envelope still says `ok`.

## Fix
### Progress 2026-08-26 — code-index surface fixed, memory surface still open

**Fixed: the code-index surface**, which re-triage showed was the severe one.
`flush_pending` (`src/retrieval/sync.rs`) no longer aborts the index walk when a
batch fails. It retries chunk-by-chunk to isolate the unembeddable payload,
stores everything that does embed, and reports the rest via a new `skipped`
channel threaded through `stream_index` → `SyncReport.skipped` → the `index`
tool's status `detail`, which now reads `INDEX INCOMPLETE` with a sample. A
minimal `"ok"` probe separates "one oversized chunk" (skip and continue) from
"embedder down" (still abort loudly) — the two are indistinguishable from
"every chunk in this batch failed" alone, and they need opposite handling.

Regression tests in `src/retrieval/sync.rs`:

- `one_oversized_chunk_is_skipped_and_the_rest_of_the_walk_still_indexes` —
  uses a `CeilingEmbedder` that fails the *whole batch* when any member is
  oversized (the granularity `POST /v1/embeddings` actually has). Asserts
  **conservation** against a healthy-embedder control run: `added + skipped`
  must equal the tree's true chunk count, so nothing can vanish silently.
- `a_dead_embedder_aborts_instead_of_skipping_every_chunk` — pins the probe, so
  a future "just swallow the error" simplification cannot turn an outage into a
  successful empty sync.

**Still open: the memory surface.** `cross_embed_memory` still embeds whole
content in one request with no segmentation, and its caller at
`src/tools/memory/mod.rs:735-737` still downgrades the failure to a warning while
returning `status: "ok"`. Fix steps 1 (attach the HTTP body), 2 (segment and
mean-pool), 3 (startup probe) and 4 (repair path for already-vectorless
memories) are all outstanding. **The workaround below still applies.**

Gate: `cargo fmt`, `cargo clippy --all-targets -- -D warnings` (also with
`--features dashboard`), `cargo test` → 4470 passed, 0 failed, 46 ignored.

**Fix provenance (partial fix — this bug stays open for the memory surface):**

- **SHA:** `a5f8e5ad` (`experiments`)
- **patch-id:** `f248f9e159cf60f848ddf487182f3dc3125ba21b`

`fix(retrieval): stop one oversized chunk from truncating the whole index build`.
The patch-id is the durable anchor: `experiments` is rebased after every ship, so
the SHA above will eventually orphan.

Plan, in ascending cost. (1) and (2) are independent and both worth doing; (1)
is the cheap one that makes every future instance self-diagnosing.

1. **Lift the HTTP body into the error.** Wherever the dense-openai transport
   renders `dense openai status`, include the response body (or at minimum the
   `type` and `message` fields). A payload-size failure should say so.
2. **Segment and mean-pool in `cross_embed_memory`.** Split content into
   sub-ceiling segments, `embed_document` each, mean-pool into one vector. Pick
   the segment budget from the *model's* ceiling, not `ctx_size / parallel`.
3. **Startup probe.** Report effective per-slot context and the model's
   `n_ctx_train` at connect time, so a misconfiguration is visible before the
   first dropped write rather than after.
4. **Repair path.** Memories already written without vectors need re-embedding.
   Confirm whether the in-place re-embed migration picks up entries that *lack*
   a vector, or only those on the old naming convention — if the latter, the
   already-lost memories stay lost after the fix.
5. **Same treatment for oversized code chunks** in the sync path — the reporter
   flagged this as unverified, and it is (see Resume).

Not fixed yet. No SHA, no patch-id.

## Tests added

None yet. Required before this can be called fixed:

- a unit test that `cross_embed_memory` on content above the segment budget
  produces one pooled vector rather than an error;
- a test that a transport error's payload-size cause survives into the warning
  string (guards fix 1 against regressing to `dense openai status`);
- a test pinning that a failed cross-embed does **not** report `status: "ok"`
  without a machine-readable `embedded: false` marker.

## Workarounds

Keep individual structured memories under ~2000 tokens (~6-8KB) so the
single-request path stays inside `n_ctx_train`. Splitting one large memory into
several topics is retrieval-equivalent and embeds reliably. There is no way to
detect already-affected memories from the tool surface — query the memories DB
for rows lacking a vector.

## Resume

**Resume item 1 is now answered — the code-indexing surface is worse than the
memory surface, and the answer raises this bug's severity.**

Confirmed 2026-08-26 by reading `flush_pending` (`src/retrieval/sync.rs:223-243`)
and its call sites: an oversized chunk does **not** get skipped and does **not**
get silently dropped. It **aborts the entire index build**:

```
sync.rs:238  embedder.embed_batch_dyn(&texts).await?   ← whole BATCH fails, not one chunk
sync.rs:403  added += flush_pending(...).await?        ← early return, mid-walk
sync.rs:409  added += flush_pending(...).await?
sync.rs:422  prune step                                ← skipped
```

Batches already flushed are committed at `:241`, so the result is a durable,
truncated index — and `index(action="status")` reports it as
`indexed: true, queryable: true`, because its only check is a non-zero chunk
count. That is the confirmed root cause of
`docs/issues/2026-08-26-index-status-claims-complete-without-checking-coverage.md`.

So this bug has two distinct surfaces with **opposite** error handling, both wrong:

| Surface | Handling | Result |
|---|---|---|
| memory (`memory/mod.rs:735`) | error swallowed to a warning | write reports `ok`, vector missing |
| code index (`sync.rs:238→403`) | error propagated with `?` | whole build aborts, partial index reported healthy |

Next concrete actions, in order:

1. **Fix `flush_pending`'s granularity.** A batch failure must not condemn the
   batch. On `embed_batch_dyn` error, retry the batch per-chunk to isolate the
   offending payload(s), embed the rest, and report the skipped chunks rather
   than aborting the walk. Segmenting oversized chunks (see Fix step 2) is the
   better end-state, but per-chunk isolation is what stops a truncated index
   being reported as complete.
2. **Attach the HTTP body** to the dense-openai transport error, so both
   surfaces say "payload too large" instead of `dense openai status`. Cheapest
   fix, and it is what makes every future instance self-diagnosing.
3. Then the segmentation work in Fix step 2, which closes the hole properly and
   is model-ceiling-aware rather than `ctx_size`-aware.

Note the shared shape across three bugs filed this session: **a `?` on an embed
call inside a loop that has already committed partial state.** `sync.rs:238`
here, `librarian/tools/reindex.rs:236` in the catalog reindex bug, and the
inverse error (swallowing) at `memory/mod.rs:735`. Worth fixing as one pattern.
## References

- GitHub issue #15 — <https://github.com/mareurs/codescout/issues/15>
- Reporter follow-up (the `n_ctx_train` correction) — comment 5354817007
- `src/tools/memory/mod.rs:340-371` (`cross_embed_memory`), `:735-737` (caller)
- `docs/issues/2026-08-26-onnx-local-path-lacks-coderankembed.md` — the
  suggested escape route from this bug, and why it is currently blocked
- `docs/issues/archive/2026-08-11-memory-documents-stored-query-prefixed.md` —
  prior art on the `embed_document` vs `embed` split in this same function
