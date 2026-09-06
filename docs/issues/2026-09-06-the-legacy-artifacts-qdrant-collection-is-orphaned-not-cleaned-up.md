---
kind: bug
status: open
tags:
- cluster/shared-resource-carries-no-owner
closed: null
opened: 2026-09-06
owner: marius
related:
- docs/issues/archive/2026-09-03-editing-an-artifact-removes-it-from-qdrant-backed-semantic-search.md
severity: low
---

# BUG: the legacy `artifacts` Qdrant collection is orphaned by the per-project migration, and nothing will ever reclaim it

## Summary

`6f032dbd` moved artifact vectors to one Qdrant collection per project
(`artifact_chunks_<name>_<hash>`) and **abandoned** the old monolithic `artifacts`
collection rather than migrating or dropping it. It still holds 2425 points. No code path
enumerates it, no code path deletes it, and nothing reports that it exists.

This is not a search defect — the correctness half is closed. It is a resource whose owner
was removed while the resource stayed, in a store shared by every project on the host.

## Symptom (Effect)

Measured 2026-09-06 against the live daemon at `127.0.0.1:6333`:

```
collection                                         exact point count
artifact_chunks_codescout_dc6a871595179329                    29435
artifact_chunks_claude_plugins_38b0719140f222d8                4818
artifact_chunks_backend_kotlin_dcb6382ad43d85ac                 164
artifacts                     (legacy, mixed-grain)            2425
```

Nothing distinguishes the last row from the others at the Qdrant UI or API. An operator
inspecting the daemon sees four artifact collections and has no way to tell that one of
them is dead — that fact lives only in a Rust string constant.

## Reproduction

```
curl -s http://127.0.0.1:6333/collections
# -> `artifacts` is listed alongside the three live artifact_chunks_* collections
```

## Environment

`experiments` @ `1aab76c5`, release build (`server-stack`), Qdrant 127.0.0.1:6333.
Host-local: a machine that never ran a pre-`6f032dbd` build will not have this collection,
which is why it cannot be found by reading the repo.

## Root cause

`QdrantArtifactStore` addresses collections **by name prefix**. `artifact_collections`
filters `n.starts_with(prefix)` (`src/retrieval/artifact.rs:230-244`) with
`prefix = "artifact_chunks_"` (`src/librarian/artifact_store.rs:219`), and
`"artifacts".starts_with("artifact_chunks_")` is `false`.

That predicate is what makes the collection unreachable, and it is doing its job: the
comment on `artifact_collections` says enumerating by prefix rather than from the workspace
registry is deliberate, so a de-registered project's vectors stay visible. The prefix scheme
gives a **clean cutover** and, by the same property, **no migration and no reclamation** —
the old name simply falls outside the namespace.

`delete` and `refile` both iterate `artifact_collections`, so neither can reach it either.
There is no drop path for a collection the prefix no longer matches.

Verified 2026-09-06 by reading both call sites plus the live `list_collections` output —
not inferred from the rename alone.

## Evidence

### The prefix predicate

`src/retrieval/artifact.rs:230-244`:

```rust
.filter(|n| n.starts_with(prefix))
```

with `prefix` documented at `src/librarian/artifact_store.rs:207` as
`{config_prefix}artifact_chunks_`.

### Every consumer goes through it

`QdrantArtifactStore::delete` (`artifact_store.rs:336`), `::refile` (`:360`) and `::knn`
(`:392`) all iterate `self.qdrant.artifact_collections(&self.prefix)`. There is no other
enumeration and no hard-coded `"artifacts"` remaining in the write or read paths.

## Hypotheses tried

1. **Hypothesis:** the legacy collection is still read, so its points inflate `unresolved`
   on every query.
   **Test:** checked the prefix predicate against the literal name.
   **Verdict:** rejected — `"artifacts".starts_with("artifact_chunks_")` is false, so `knn`
   never opens it. Query cost is zero, not merely small.

## Fix

Not implemented. It is one command, and the reason to file rather than just run it is that
the command is **irreversible and host-local**, so it wants a decision rather than a
drive-by:

```
curl -X DELETE http://127.0.0.1:6333/collections/artifacts
```

Before running it, confirm no other tool on this host reads that name — the collection
predates the per-project scheme and was the shared artifact store for every project, so a
sibling checkout on an older binary would still be writing to it.

The durable half is separate: **a prefix migration leaves no reclamation path by
construction.** If the prefix changes again, the same thing happens again and nothing will
report it. A `doctor` check that lists Qdrant collections not matching the current prefix
would turn this from a thing someone happens to notice into a thing the tool says.

## Tests added

None — filed at notice. A regression test is not obviously available: the defect is a
leftover in a stateful external daemon, not a branch in the code, and no gate lane compiles
the Qdrant path at all (`server-stack` is not a default feature). The `doctor` check
proposed above **would** be testable, against `InMemoryArtifactStore` or a stubbed
collection list.

## Workarounds

None needed. The collection costs disk and nothing else; no query reads it.

## Resume

Decide whether to drop it. If yes: run the `DELETE` above, having first checked for other
readers on this host, and record the point count you removed. If the `doctor` check is
wanted, it belongs beside the existing catalog-drift checks in
`src/librarian/tools/doctor.rs` and needs a live Qdrant handle, which no current check has —
that is the real cost of the second half, not the predicate itself.

## References

- `docs/issues/archive/2026-09-03-editing-an-artifact-removes-it-from-qdrant-backed-semantic-search.md`
  — the bug whose fix (`6f032dbd`) orphaned this collection. Its § *Fix* carries the same
  census.
- `src/librarian/artifact_store.rs:85-113` — `artifact_collection_name`, the naming scheme.
