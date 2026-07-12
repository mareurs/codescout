---
status: open
opened: 2026-07-10
closed:
severity: low
owner: marius
related: ['2026-07-10-librarian-context-silent-empty-no-embedder']
tags: [librarian, docs, doc-vs-code-drift, embedding]
kind: bug
---

# BUG: docs promise librarian find/context "fall back to SQL LIKE when no embedder" — neither actually does (doc-vs-code drift)

## Summary
`docs/manual/src/concepts/librarian-mcp.md` § Semantic search states that
`artifact_find` and `librarian_context` "accept `semantic: "<natural language>"`
and **fall back to SQL `LIKE` when no embedder is configured**." Observed behavior
contradicts this: with no embedder, `find(semantic=…)` errors and `context(topic=…)`
returns empty — neither performs a LIKE fallback.

## Symptom (Effect)
Documented contract (`librarian-mcp.md`, § Semantic search):
> `artifact_find` and `librarian_context` accept `semantic: "<natural language>"`
> and fall back to SQL `LIKE` when no embedder is configured.

Actual, no embedder configured:
- `artifact(action="find", semantic="x")` → `semantic search requires an embedding service` (hard error).
- `librarian(action="context", topic="x")` → `{markdown:"", included_ids:[]}` (silent empty).

Neither runs a `LIKE` query on the natural-language string.

## Reproduction
Same as the related bug: run codescout MCP without `LIBRARIAN_EMBED_MODEL`, then
call `find(semantic=…)` and `context(topic=…)`. Compare against the doc claim.

## Environment
Linux, codescout MCP over stdio, integrated librarian (`librarian` feature). Note
the doc uses the pre-embedding tool names (`artifact_find`/`librarian_context`);
per `docs/manual/src/concepts/librarian-embedded.md` those are the same code now
surfaced as `artifact`/`librarian`, so the contract still applies.

## Root cause
Either the code never implemented the documented LIKE fallback, or it was removed
and the doc wasn't updated. `find.rs:325-327` unconditionally `bail!`s on missing
embedder; `context.rs` fn `call` drops to `topic_vec = None` and emits nothing.
No path derives a `LIKE` filter from the semantic/topic string.

## Evidence
- Doc claim: `docs/manual/src/concepts/librarian-mcp.md` § Semantic search.
- Code: `src/librarian/tools/find.rs:325-327` (bail), `src/librarian/tools/context.rs` fn `call` (silent empty).
- Live calls 2026-07-10 (see Symptom).

## Hypotheses tried
N/A — divergence read directly from doc + source.

## Fix
Not started. Decide the intended contract:
- **If LIKE fallback is desired:** implement it in both `find` (on `semantic`) and
  `context` (on `topic`) — derive a `title/body LIKE '%…%'` filter when
  `ctx.embedding` is `None`. This also resolves the related silent-empty bug.
- **If not:** correct `librarian-mcp.md` to say the semantic param requires an
  embedder and errors without one, and fix `context` to error rather than empty.

## Tests added
N/A — not yet fixed.

## Workarounds
Use explicit `artifact(action="find", filter={…contains…})` for keyword discovery
without an embedder (works today; measured high-precision on real trackers).

## Resume
Pick the contract (LIKE fallback vs error) with the related bug, implement in
`find.rs`/`context.rs`, update `librarian-mcp.md` to match, and add a no-embedder
regression test that pins whichever behavior is chosen.

## References
- `docs/manual/src/concepts/librarian-mcp.md` (§ Semantic search)
- `docs/manual/src/concepts/librarian-embedded.md` (tool-name evolution)
- `src/librarian/tools/find.rs:325-327`, `src/librarian/tools/context.rs` (fn `call`)
- Related: `docs/issues/2026-07-10-librarian-context-silent-empty-no-embedder.md`
