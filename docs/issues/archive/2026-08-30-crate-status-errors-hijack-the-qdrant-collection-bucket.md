---
status: fixed
opened: 2026-08-30
closed: 2026-08-30
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

**Fixed on `experiments` 2026-08-30, in two commits.**

| | SHA (`experiments`) | patch-id (survives rebase) |
|---|---|---|
| A — crate publishes the type | `8097c2d6` | `18922aa3cc9f4be601e26f53ee68c9c483fec01b` |
| B — root's classifier matches it | `4fd4e5f4` | `d377f8ab6086f9d7137b4f4fc10d4628a26aa01c` |

The remedy is the one `ET-5`/T4 already applied to the connect case: publish the
contract as a **type**, not a literal. `EmbedError::Status { url, status, body }`
renders `STATUS_FAILED_MARKER`, re-exported from `codescout_embed`, and
`classify_search_error` matches the imported constant in the arm already hoisted
above the collection bucket. A literal on each side of a crate boundary is what
`ET-5` records as unfixable by testing — nothing makes the two fail together.

**Regression tests** (`src/tools/semantic/semantic_search.rs`):

- `a_status_body_saying_not_found_is_not_reported_as_a_missing_collection` — the
  negative. Asserts the hint does **not** mention a missing collection or
  `sync_project`. Asserting only that the right hint appears would have passed
  with the arm ordered *after* the collection bucket, since both hints mention the
  embedder.
- `the_crates_own_status_error_routes_where_roots_does` — the differential, in the
  shape of its connect-case sibling.

**Mutation-confirmed, not merely green.** Removing the marker from the classifier
arm kills both, and the failure output reproduces the bug verbatim:

```
hint: Qdrant collection is missing for project `codescout`.
      Populate it: `cargo run --release --bin sync_project -- . codescout`
```

for an embedder 404. Meanwhile `an_embedder_body_mentioning_not_found_does_not_hijack_the_collection_bucket`
— the pre-existing guard for *root's* producer — passes unchanged under that same
mutation. That is the evidence for this file's central claim: the two producers
were covered independently, and the fix had landed on only one of them.

Gate at fix time: fmt clean, clippy exit 0, 4836/0 default, 3362/0 lean.
## Provenance

Noticed 2026-08-30 while auditing which error-string contracts the T6 dense-leg swap
would break. Sibling of
`docs/issues/2026-08-30-sparse-status-errors-never-match-their-classifier-arm.md`,
found in the same pass — same function, same class, different producer.
