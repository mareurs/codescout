---
kind: bug
status: fixed
tags:
- cluster/shared-resource-carries-no-owner
closed: 2026-10-27
no_fix_commit: The fix was a one-off operator deletion of the host-local Qdrant collection `artifacts` (curl -X DELETE), approved by the operator on 2026-10-27; no code changed. The detector shipped earlier on 2026-10-16 is unchanged.
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
### ⚠ DO NOT DELETE — re-verified 2026-09-24: the collection is growing, so something still writes it

Open-bug sweep (`deep-agent-workflow-observations:DWF-7`). Verifier evidence from read-only Qdrant GETs and counts; nothing was deleted. **This file's premise has changed.** The `artifacts` collection's exact count is now **5394** points, against 2425 when this file was written on 2026-09-06. It is not orphaned: some writer is still adding to it. 993 points carry a non-empty `project_id` (e.g. `/home/marius/work/mirela`, `/home/marius/work/stefanini/southpole`), and 4401 carry an empty one, some with UUID-shaped `artifact_id`s.

The current tree has **no** Qdrant write to `"artifacts"`. The only source hit is a stale July worktree, `codescout.worktrees/check-codescout-integration/src/librarian/mod.rs:123,398`, with no built binary. Every running `codescout` process started on 09-18 or later. There is no `doctor` Qdrant check.

**The writer is unidentified, and this file's own pre-DELETE caveat ("confirm no other tool writes that name") now fires.** Identifying the writer comes before any cleanup. A DELETE today would destroy data that something is actively producing.

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

## Detector shipped 2026-10-16 (report only) — the bug stays OPEN

The second half of the proposal is done; the first half is deliberately not. `librarian(action="doctor")` now emits `retired_vector_collection_present` (`Check::RetiredVectorCollectionPresent`, `scan_retired_vector_collections` in `src/librarian/tools/doctor.rs`): **informational** (does not count in `summary.defects` or the CLI exit code), `id: None`, path `qdrant:<name>`, with the **exact** point count and a row that says *DO NOT delete* and why. A named `EXEMPT` entry in `every_declared_check_is_scope_gated_or_a_named_exemption`: its subject is the shared backend, not a catalogued artifact.

- Seam: new default-empty method `ArtifactVectorStore::retired_collections()` (`src/librarian/artifact_store.rs`), overridden only by `QdrantArtifactStore`, which derives the retired name from its own prefix (`{config_prefix}artifacts`, via `retired_collection_names`) and counts through new `QdrantWrap::collection_point_count` (`src/retrieval/artifact.rs`). sqlite-vec and the in-memory store keep the empty default and are silent.
- **No deletion code exists and none was added** — no `fix=`, no `drop_retired`. The trait method's doc comment states it is read-only by contract. The evidence above (growing, writer unidentified) is why.
- Runs before the catalog lock is taken (it awaits). An unreachable daemon yields no row and a `tracing::warn!`, not a failed doctor.

### What would close this bug

One of, in order:

1. **Identify the writer of `artifacts`.** Re-run `doctor` (or `curl .../collections/artifacts` count) twice some minutes apart to confirm it still grows, then find which binary or checkout writes that name: for each `codescout` process, `readlink /proc/<pid>/exe` and its cwd; look for other checkouts/worktrees still carrying a `"artifacts"` Qdrant write (the stale July worktree `check-codescout-integration` was the only source hit on 2026-09-24, with no built binary — so look for a *built* binary elsewhere, e.g. a `cargo rb` install or another machine pointed at this daemon).
2. Once the count has been **flat across a period in which every process on the host was restarted on a post-`6f032dbd` build**, the collection is genuinely orphaned: record the point count, then drop it by hand (`DELETE /collections/artifacts`). Close the bug with that count in the Resolution.
3. Or decide the data is wanted: migrate its 993 `project_id`-bearing points into the per-project collections instead of deleting.

The doctor row disappears by itself once the collection is dropped.

## Tests added

- `a_retired_vector_collection_is_reported_with_its_count_and_a_do_not_delete_warning` (`doctor.rs`) — controls first (no backend; default-empty store), then the finding: informational, `id: None`, exact count, "DO NOT delete".
- `a_store_that_cannot_list_collections_yields_no_row_not_a_failed_doctor` (`doctor.rs`).
- `the_retired_collection_name_follows_the_config_prefix_and_never_guesses` and `a_store_that_keeps_the_default_has_nothing_retired` (`artifact_store.rs`).
- Compiled and run with `--features server-stack` as well as the default build, so the `QdrantArtifactStore` override is type-checked; **the override's daemon calls (`collection_exists`, `collection_point_count`) are NOT exercised by any test** — there is no Qdrant in the gate. Confirm once against a live daemon: `librarian(action="doctor")` should list `qdrant:artifacts` with a count matching `curl`.

## Workarounds

None needed. The collection costs disk and nothing else; no query reads it.

## Resume

N/A — resolved. See Resolution.

## Resolution 2026-10-27

The operator approved the deletion ("approve it"). The approval was given on a summary that called the collection orphaned and did not mention the `DO NOT DELETE` section above, so the state was re-checked before the command ran.

- **Point count: 5394**, read twice about a minute apart on 2026-10-27 (12:02:58 and 12:04:01) with `POST /collections/artifacts/points/count {"exact":true}`. It is exactly the figure recorded on 2026-09-24, so the collection did not grow in about 33 days. `6f032dbd` is dated 2026-09-03, and the oldest `codescout start` process running on 2026-10-27 (`ps -eo lstart`, pid 412806) started on 2026-10-03, a month later; each binary's own build date was not checked. That is close to the issue's criterion 2 (flat across a period in which every process restarted on a post-`6f032dbd` build), not a proof of it.
- **The writer was never identified.** The count stopped growing; nothing explains why it ever grew. If it starts growing again, `doctor` will not show it (the collection no longer exists) and a new collection named `artifacts` would reappear in `GET /collections`.
- **Safety net:** a snapshot was taken before the delete, `artifacts-968029894703799-2026-10-27-10-03-12.snapshot` (45,170,176 bytes, sha256 `720da13d7d2e52ad4710d38c8fdc6eba150aed0151a5b4e4e92906706094d244`). After the delete, the snapshots API no longer lists it. Whether the file survived on disk is NOT verified: the Qdrant process (pid 3909) runs under another user and its directory is not readable from this session.
- **Command:** `curl -X DELETE http://127.0.0.1:6333/collections/artifacts`, answer `{"result":true}`. Afterwards `GET /collections/artifacts` answers `Not found`, and the live collection `artifact_chunks_codescout_dc6a871595179329` still counts 52008 points.
- **Lost verification:** the issue asked for the new `doctor` row to be confirmed once against a live daemon while the collection existed (its row must show `qdrant:artifacts` with a count matching `curl`). That was not done before the delete and cannot be done now. After the delete, `doctor` lists `retired_vector_collection_present: 0`, which shows the check runs but not that it would have counted 5394. The daemon calls of `scan_retired_vector_collections` stay unexercised by any test and by any live run.
- The 993 points that carried a `project_id` were not migrated (option 3 of the issue). The per-project collections are rebuilt from each project's catalog by a reindex, so those vectors are derived data.

## References

- `docs/issues/archive/2026-09-03-editing-an-artifact-removes-it-from-qdrant-backed-semantic-search.md`
  — the bug whose fix (`6f032dbd`) orphaned this collection. Its § *Fix* carries the same
  census.
- `src/librarian/artifact_store.rs:85-113` — `artifact_collection_name`, the naming scheme.
