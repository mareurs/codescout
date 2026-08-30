---
status: fixed
opened: 2026-08-30
closed: 2026-08-30
severity: high
owner: marius
related: []
tags: [codescout-embed, retrieval, panic, robustness]
kind: bug
---

# `RemoteEmbedder::embed` panics when the server returns fewer vectors than inputs

`RemoteEmbedder::embed` (`crates/codescout-embed/src/remote.rs`) reconstructs its
output by indexing the server's response once per non-empty input:

```rust
let mut all = vec![vec![0.0; dim]; texts.len()];
for (slot, (orig_idx, _)) in non_empty.iter().enumerate() {
    all[*orig_idx] = std::mem::take(&mut embedded[slot]);   // <-- indexes `embedded`
}
```

Nothing checked that `embedded.len() == non_empty.len()`. A server returning fewer
vectors than requested therefore produced

```
panicked at crates/codescout-embed/src/remote.rs:565:
index out of bounds: the len is 1 but the index is 1
```

— a **library aborting the process on remote input it does not control.** The
trigger is an endpoint that silently truncates an oversize request instead of
refusing it, which is exactly the failure mode root's own consumer-side arity check
was written for ("the dense embedder may be silently truncating an oversize request
instead of erroring").

Reachable before this fix from any deployment on the `ollama:` / `openai:` resolver
path, which `build_embedder` routes through `RemoteEmbedder` whenever no
`embedder_url` is configured.

## Why it survived

The block immediately above it anticipates the *neighbouring* case and stops one
step short:

> *"If `embedded` is empty here, the server returned 200 with no data — refuse
> rather than fall back to a 1-element dim sentinel that would corrupt the vec0
> INSERT downstream."*

That guard is about determining `dim`, and it has a **cached-dims fallback** — so
for a no-data response with dimensions already cached it did not refuse at all. It
computed a dim, built an all-zero `all`, and walked straight into the same panic. The
guard that looked like it covered the empty case made the empty case worse.

The short case (1 vector for 3 inputs) was never considered.

## How it was found

Not by review, and not by a test written for it. Root's `EmbedderHttp::dense_batch`
began delegating to this function (T6 step D), and root's
`embed_one_batch_errors_on_dense_arity_mismatch` — which had been catching this
cleanly for as long as it existed — stopped being the first thing to run. The test
went from passing to panicking inside the crate.

That is the interesting part: **the defect was always there, and was invisible
because a consumer happened to check first.** Moving the boundary did not create it;
it removed the accidental cover. Any other consumer of `codescout-embed` — the
resolver path included — never had that cover at all.

## Fix

Fixed on `experiments` in `797dd023`, patch-id
`095ae63248a236e74a2135f101fa416cffb643dc` — the same commit as T6 step D, since
the delegation is what made the panic reachable through root.

An explicit arity check before the reconstruction, placed after the `dim` block so
the existing no-data guard keeps its message where it is the more specific one:

```rust
if embedded.len() != filtered.len() {
    bail!("embedding server returned {} vectors for {} non-empty inputs — the \
           server may be silently truncating an oversize request instead of \
           refusing it. Send a smaller batch.", embedded.len(), filtered.len());
}
```

**Regression test:** `remote::tests::a_short_response_errors_instead_of_panicking`
— a loopback server answering 200 with one vector for three inputs; asserts an error
naming both counts, so an operator can tell truncation from an outage.

Root's `embed_one_batch_errors_on_dense_arity_mismatch` is kept as well. The two now
fail for different reasons and at different layers, and the root one asserts only
that both counts are named rather than the exact wording, so it survived the producer
changing underneath it.

## Provenance

Found 2026-08-30 during T6 step D
(`resume-embedding-transport-stages-1-3:ET-10`). Sibling in kind to
`docs/issues/archive/2026-08-30-crate-status-errors-hijack-the-qdrant-collection-bucket.md`:
both are cases where a hazard was handled on root's side of a duplicated pair and
never on the crate's, and both surfaced when the duplication was removed.
