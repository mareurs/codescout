---
status: open
opened: 2026-07-10
closed:
severity: medium
owner: marius
related: ['2026-07-10-librarian-semantic-no-like-fallback-doc-drift']
tags: [librarian, embedding, silent-failure, ux]
kind: bug
---

# BUG: librarian(context, topic=…) returns empty SILENTLY when no embedder is configured, while find(semantic=…) errors — inconsistent no-embedder handling

## Summary
When the librarian embedder is not configured (`ctx.embedding == None`),
`librarian(action="context", topic=…)` returns `{markdown:"", included_ids:[]}`
with no error, whereas `artifact(action="find", semantic=…)` correctly bails with
a clear message. Same missing-embedder condition, two different behaviors — the
silent one can make an agent conclude "no relevant artifacts exist" when the real
cause is a config gap.

## Symptom (Effect)
With the MCP server launched without `LIBRARIAN_EMBED_MODEL` (embedder absent):

`artifact(action="find", semantic="…")` →
```
semantic search requires an embedding service
```

`librarian(action="context", topic="…")` →
```json
{"markdown": "", "included_ids": [], "scope": {"applied": "repo", …}}
```
No error, no hint that the embedder is missing.

## Reproduction
1. Launch codescout MCP with no `LIBRARIAN_EMBED_MODEL` in env (default registration).
2. `librarian(action="context", topic="anything")` → empty markdown, no error.
3. `artifact(action="find", semantic="anything")` → errors (the correct behavior).

Observed live 2026-07-10 on branch `experiments` (integrated librarian path).

## Environment
Linux, codescout MCP over stdio (Claude Code), integrated librarian (`librarian`
cargo feature on). Embedder services were actually UP (dense :48081, qdrant :6333)
— the server process simply had no `LIBRARIAN_EMBED_*` env, so `ctx.embedding` was
`None`.

## Root cause
`src/librarian/tools/context.rs` fn `call`: `topic_vec` is bound only when BOTH
`a.topic` and `ctx.embedding` are `Some` —
`if let (Some(ref topic), Some(ref svc)) = (&a.topic, &ctx.embedding)`. When the
embedder is `None`, the pattern falls to the `else` arm → `topic_vec = None` →
`semantic_candidate_ids = None` → the function proceeds with no candidates and
emits an empty bundle. There is no "topic requested but embedder absent" guard.

Contrast `src/librarian/tools/find.rs:325-327`, which handles the identical
condition loudly:
```rust
match ctx.embedding.as_ref() {
    Some(svc) => Some(svc.embedder.embed_query(query).await?),
    None => anyhow::bail!("semantic search requires an embedding service"),
}
```

## Evidence
Live tool calls (2026-07-10), see Symptom. `printenv` in the MCP process confirmed
`LIBRARIAN_EMBED_MODEL=<unset>`; `src/librarian/mod.rs:56` only builds the embedder
when that var is set ("when absent … we skip embedding silently").

## Hypotheses tried
N/A — root cause read directly from source.

## Fix
Not started. Options (decide alongside the related doc-drift bug):
1. Make `context` bail with the same message as `find` when `a.topic.is_some()` and
   `ctx.embedding.is_none()`.
2. OR implement the SQL `LIKE` fallback the manual promises (see related bug), so
   `context` degrades to keyword search on the topic instead of erroring/emptying.
Option 2 is the more useful behavior and would also satisfy the documented contract.

## Tests added
N/A — not yet fixed.

## Workarounds
Configure the embedder (`LIBRARIAN_EMBED_MODEL` + `LIBRARIAN_EMBED_URL` in the MCP
launch env), or use `artifact(action="find", filter={…contains…})` for keyword
discovery, which works without an embedder.

## Resume
Decide the intended no-embedder contract for the semantic path (error vs LIKE
fallback) with the related bug, then patch `src/librarian/tools/context.rs` fn
`call` accordingly and add a regression test asserting `context(topic=…)` with a
`None` embedder does NOT silently return empty.

## References
- `src/librarian/tools/context.rs` (fn `call`, `topic_vec` binding)
- `src/librarian/tools/find.rs:325-327`
- `src/librarian/mod.rs:56`
- Related: `docs/issues/2026-07-10-librarian-semantic-no-like-fallback-doc-drift.md`
