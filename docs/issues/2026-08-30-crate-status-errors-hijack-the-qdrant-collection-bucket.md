---
status: investigating
opened: 2026-08-30
closed:
severity: medium
owner: marius
related: []
tags: [retrieval, error-handling, classifier, codescout-embed]
kind: bug
---

# A crate status error whose body says "not found" is reported as a missing Qdrant collection

`RemoteEmbedder::embed` (`crates/codescout-embed/src/remote.rs`) reports a non-2xx
response as:

```
HTTP {status} from embedding server: {body}
```

where `{body}` is the server's own response text — arbitrary remote content.

`classify_search_error` (`src/tools/semantic/semantic_search.rs:59`) checks its arms
in order. The relevant two are, in this order:

1. `contains("doesn't exist") || contains("not found") || contains("Collection")`
   → *"Qdrant collection is missing for project `X`. Populate it: `cargo run …
   sync_project …`"*
2. `contains("embedding server")` → the resolver-path embedder hint

Because (1) precedes (2), a perfectly ordinary embedder 404 —

```
HTTP 404 from embedding server: model 'coderank' not found
```

— is classified as a **missing Qdrant collection** and the operator is told to
re-index a collection that is fine. Verified:

```
$ echo 'HTTP 404 from embedding server: model not found' | grep -c 'not found'
1
```

This is live today on the `ollama:` / `openai:` resolver path (no `embedder_url`
configured), which is the path `build_embedder` routes through `RemoteEmbedder`.

## Why root's own dense leg is immune, and why that matters

Root's `EmbedderHttp::dense_batch` emits `dense openai status {code}: {body}`, and
that arm was deliberately **hoisted above** the collection bucket. Its comment states
the reason exactly:

> *That message now carries the embedder's RESPONSE BODY, which is arbitrary remote
> text. A body containing "not found" or "Collection" would hijack the collection
> bucket. Specificity first, per this function's own contract.*

So the hazard was identified and fixed for root's producer, and the crate's producer
— which has the same shape — was never given the same protection. The fix landed on
one of two producers.

## Fix

Publish the contract as a **type**, the remedy `ET-5`/T4 already applied to the
connect case: `EmbedError::Status { url, status, body }` rendering a
`STATUS_FAILED_MARKER`, re-exported from `codescout_embed`, matched by
`classify_search_error` in the hoisted arm alongside `dense openai status`. A literal
on each side of a crate boundary is what `ET-5` records as unfixable-by-testing —
nothing makes the two fail together.

This is step A/B of the T6 plan
(`resume-embedding-transport-stages-1-3:ET-10`), so it is being fixed there rather
than separately; the swap requires it regardless, since after T6 the crate becomes
the *only* dense producer and root's hoisted arm would otherwise stop firing
altogether.

## Provenance

Noticed 2026-08-30 while auditing which error-string contracts the T6 dense-leg swap
would break. Sibling of
`docs/issues/2026-08-30-sparse-status-errors-never-match-their-classifier-arm.md`,
found in the same pass — same function, same class, different producer.
