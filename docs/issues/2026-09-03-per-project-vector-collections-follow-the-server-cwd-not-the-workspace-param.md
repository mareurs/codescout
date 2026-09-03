---
status: open
opened: 2026-09-03
closed:
severity: high
owner: marius
related: []
tags:
  - cluster/accepted-parameter-silently-dropped
kind: bug
unverified: 'the misfiled points were not read back out of codescout''s collection — the payload carries no path, so ownership was inferred from the absent collection plus the code path, not observed directly'
---

# BUG: per-project vector collections follow the server's cwd, so a `workspace=`-scoped reindex files another project's vectors under the active project and reports success

## Summary
`6f032dbd` gave each project its own Qdrant collection, named from `current_project`. That
value is resolved **once, when the tool context is built**, from the MCP server's cwd — so
the per-call `workspace=` parameter cannot reach it. The catalog honours `workspace=`; the
vector store does not. A `reindex(workspace=<other project>, reembed=true)` therefore embeds
that project's artifacts into the **active** project's collection, reports
`vectorless: 0, embed_error_count: 0`, and leaves the other project's semantic search
returning nothing relevant — with no error at any point.

## Symptom (Effect)

Three `reindex(workspace="/home/marius/work/claude/prompt-engineering", reembed=true)` calls,
each reporting success server-side:

```
"embedded": 1, "embed_error_count": 0, "vectorless": 0,
"vectorless_note": "every indexed artifact has a vector"
```

And yet no collection for that project exists:

```
$ curl -s localhost:6333/collections
  artifacts                                     5394 points   (the superseded artifact-grain one)
  artifact_chunks_codescout_dc6a871595179329   29046 points
  code_chunks / memories / bench_coderank_code_chunks

$ python3 -c "import hashlib; print(hashlib.sha256(b'/home/marius/work/claude/prompt-engineering').hexdigest()[:16])"
f6504bfff91be097          # artifact_chunks_prompt-engineering_f6504bfff91be097 — ABSENT
```

Reading back, a query quoting a document's own title near-verbatim does not return it:

```
doc(action="find", semantic="the arm's prompt asserts a bug that its own fixture does not
    contain, so verifying the premise scores as a failure",
    workspace="…/prompt-engineering", include_archived=true)
→ 17 hits, none of them that document
  hints.unresolved = 846
  "846 vector(s) the store returned resolved to no chunk row and were discarded before ranking"
```

## Reproduction

```bash
# with the MCP server's cwd in project A, and B a different registered project
librarian(action="reindex", workspace="<B>", reembed=true)     # reports success
curl -s localhost:6333/collections                              # no collection for B
doc(action="find", semantic="<text verbatim from a B document>", workspace="<B>")
                                                                # does not return it
```

Observed 2026-09-03 at `6f032dbd`, server cwd `/home/marius/work/claude/codescout`,
target `/home/marius/work/claude/prompt-engineering`, Qdrant backend.

## Environment

- Backend: Qdrant (the default). The sqlite-vec escape hatch is not affected the same way —
  it keys on `chunk_id` in the catalog the call is already scoped to.
- Only reachable when the server's project and the call's `workspace=` differ, which is
  exactly the cross-project pattern `CLAUDE.md` documents and the `workspace=` param exists for.

## Root cause

`src/librarian/mod.rs::build_tool_context_with` resolves the project **once**, from the
process's cwd, and hands its path to the store constructor:

```rust
let current_project = env.cwd.clone()
    .or_else(|| std::env::current_dir().ok())
    .and_then(|cwd| current_project::resolve(&cwd, &ws_arc))
    .map(std::sync::Arc::new);
…
let project_path = current_project.as_deref().map(|cp| cp.abs_path.to_string_lossy().into_owned());
…
QdrantArtifactStore::new(qdrant, prefix, project_path.as_deref().unwrap_or_default())
```

The store is built at process start and holds that path for its lifetime. Nothing in a
per-call path re-derives it, so `workspace=` — which the catalog *does* honour, and visibly:
the same call's `find` returns the other project's artifacts — is invisible to the vector
layer. The two halves of one call disagree about which project they are serving, and only
one of them says so.

**The commit that introduced this diagnosed the same shape one layer down.** Its message
rejects payload-based scoping because *"`reindex` derives the id from
`containing_root(...).unwrap_or_default()` … so 4395 of 5388 live points carried an EMPTY
project_id"*. The replacement reaches for `.unwrap_or_default()` on the project path itself,
so an unresolved project yields the collection `artifact_chunks_` + `sha256("")` rather than
an error — the same silent-default failure it was written to remove, moved up a level.

Measured 2026-09-03: collection census above; `sha256` of both project paths computed
directly; `build_tool_context_with` read at `src/librarian/mod.rs:78-233`.

**Not established:** that prompt-engineering's points are physically inside
`artifact_chunks_codescout_…`. Its payload carries only `chunk_id`, `artifact_id` and an
empty `project_id`, so a scroll cannot attribute a point to a project. Ownership is inferred
from the absent collection plus the code path. The reachable-consequence half — success
reported, document unretrievable — is observed.

## Evidence

### `reembed` converges rather than repairing, which is what sent me looking

`unresolved` across three consecutive `reembed=true` runs: **429 → 846 → 844**.

Two hypotheses fit the first pair and predict differently — additive (+417 → 1263) and
multiplicative (×1.97 → 1692). **Both were falsified by the third run.** The population is
stable, consistent with deterministic (UUID-v5-shaped) point ids making re-embedding
idempotent: the same chunks are rewritten to the same point ids, in the same wrong place.

Recorded because the n=2 reading — *"the prescribed remedy amplifies the condition"* — was
one message from being published and is wrong.

### The remedy the hint names cannot fix this

`unresolved_hint` prescribes `librarian(action="reindex", reembed=true)`. That is the call
whose collection is misderived, so following it re-files the vectors in the same wrong
collection and reports success again.

## Hypotheses tried

1. **Hypothesis:** this is the already-filed grain mixture,
   `docs/issues/2026-09-03-editing-an-artifact-removes-it-from-qdrant-backed-semantic-search.md`.
   **Test:** read that file; check whether the symptom survives its fix at `6f032dbd`.
   **Verdict:** rejected as the same defect — that one is the pre-fix `artifacts` collection
   holding two id grains, and `6f032dbd` fixes it (embed errors went 45 → 0 on the same call
   in the same session). This is a new defect **introduced by** that fix. Related, not
   duplicate.
2. **Hypothesis:** `reembed=true` never completes, because every call returned
   `Tool 'librarian' timed out after 60s`.
   **Verdict:** rejected as the explanation — the work completes server-side regardless of
   the timeout: `vectorless` went 114 → 0 across a call that reported failure. That mismatch
   is a **separate, already-filed defect**, not part of this one:
   `docs/issues/2026-09-03-librarian-is-an-embedding-loop-omitted-from-the-embedding-loop-exemption.md`
   (`1067c6c157f0d9ee`), whose Summary states it exactly — *"the work continues server-side;
   only the caller's view of it is destroyed"*. Rediscovered here independently; not re-filed.

## Fix

Not attempted — this is a peer's commit from the same day and the design call is theirs.

The shape of it: the collection name has to be derived per call from the same project the
catalog resolved for that call, rather than captured at context-build time. `knn`'s
cross-project fan-out already enumerates by name prefix, so the read side may need nothing;
it is the write side that is pinned.

And `.unwrap_or_default()` on the project path deserves to become a hard error. A collection
named from `sha256("")` is indistinguishable from a working one at every observation the
caller can make, which is the property this whole class turns on.

## Tests added
None yet. The regression guard should assert the collection NAME a `workspace=`-scoped
reindex writes to, not that the reindex succeeded — success is what it already reports while
misfiling. A test asserting only `embed_error_count == 0` passes today.

## Workarounds
Run the reindex from a server whose cwd is the project being indexed. On the CLI path
(`reindex_cli`, `src/librarian/mod.rs:472`) the prefix is derived separately and may not
share the defect — untested.

## Resume
Read `QdrantArtifactStore::new` and `knn` (`src/librarian/artifact_store.rs:191-207`) to
confirm the write path uses the captured `project_root` and the read path enumerates by
prefix. Then decide whether the store should take the project per call or be rebuilt per
call. Settle the open question this record could not: scroll
`artifact_chunks_codescout_dc6a871595179329` and join `artifact_id` against the two projects'
catalogs to see whether prompt-engineering's artifacts are physically in there.

## References
- `src/librarian/mod.rs:78-233` — `build_tool_context_with`
- `src/librarian/artifact_store.rs:191-207` — `QdrantArtifactStore`
- `6f032dbd` — the commit that introduced per-project collections, and the fix for the grain mixture
- `docs/issues/2026-09-03-editing-an-artifact-removes-it-from-qdrant-backed-semantic-search.md` — the defect `6f032dbd` fixes; related, not duplicate
