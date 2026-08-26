---
status: open
opened: 2026-08-26
closed:
severity: medium
owner: marius
related: []
tags: [retrieval, sqlite-vec, indexing, embeddings]
kind: bug
---

# BUG: `index(action="build", force=true)` cannot migrate a SQLite-vec index across embedding dimensions

## Summary

`force=true` advertises a full reindex, but the dimension guard runs *before*
the force path is reached, so the one situation that genuinely requires a full
rebuild — changing embedding model, and therefore vector width — is the one
`force=true` cannot perform. Changing embedding models requires manual database
surgery.

Imported from GitHub issue #18 (reporter: mic-urs, 2026-08-26). Mechanism
re-verified at `d5ed4d6f` on 2026-08-26 and matches the report exactly.

## Symptom (Effect)

`index(action="build", force=true)` returns `status: started`, then the
background job fails with the original dimension-mismatch error. The immutable
`code_vec` table stays at its original width; no replacement index is created.
The failure is only visible by polling `index(action="status")`.

The error the job dies on:

```
code index was built at {index_dim} dimensions; the configured embedder produces {model_dim}
hint: Delete the code index and reindex — the vector table bakes the dimension
      in at creation and cannot migrate in place. Or set [embeddings].model back
      to the model the index was built with.
```

## Reproduction

1. Build a project index with an embedder returning 768-dimensional vectors.
2. Reconfigure to an embedder returning 384-dimensional vectors.
3. `index(action="build", force=true)`.
4. `index(action="status")` — the job failed; the table is still 768-wide.

## Environment

- codescout `experiments` at `d5ed4d6f`, `CODESCOUT_VECTOR_BACKEND=sqlite-vec`.
- Reproduces on any backend whose vector table bakes width in at creation;
  sqlite-vec is the confirmed case.

## Root cause

Ordering. `src/retrieval/sync.rs` runs the guard at line 755 and only reaches
the force-capable indexing work at line 777:

```
755: self.guard_index_dim(&collection, project_id).await?;   // unconditional reject
777: opts.force_reindex,                                     // → stream_index
```

`guard_index_dim` (`src/retrieval/client.rs:406-428`) takes no `force`
parameter and has no bypass — a mismatch is always `Err`. So `force_reindex`
never gets the chance to influence a dimension migration; it is consumed by
`stream_index` for a different purpose entirely (re-embedding chunks the server
already has, `sync.rs:381`).

The table's immutability is real, not incidental —
`src/retrieval/sqlite_code_store.rs` creates `code_vec` with the dimension
fixed, and sqlite-vec offers no widening. So the fix is not "relax the guard"
but "let `force` drop and recreate the project's vector table".

`inferred from src/retrieval/sync.rs:755,777 and src/retrieval/client.rs:406-428
— read 2026-08-26, not executed against a live dimension change this session.`

## Evidence

### Guard ordering (`grep`, 2026-08-26)

```
src/retrieval/sync.rs
    730: force_reindex = opts.force_reindex,        [sync_project]
    755: self.guard_index_dim(&collection, project_id).await?;   [sync_project]
    777: opts.force_reindex,                        [sync_project → stream_index]
```

### The guard has no force bypass (`symbols include_body`, 2026-08-26)

```rust
pub(crate) async fn guard_index_dim(&self, collection: &str, project_id: &str) -> Result<()> {
    let Some(index_dim) = self.code_store.collection_dim(collection, project_id).await? else {
        return Ok(());
    };
    let model_dim = self.effective_model_dim(index_dim as usize);
    if model_dim == index_dim { return Ok(()); }
    Err(/* unconditional RecoverableError */)
}
```

Signature carries no `force`; every mismatch returns `Err`.

### The guard's existing tests are strong and must keep passing

`src/retrieval/search.rs:576` `guard_index_dim_errors_in_both_mismatch_directions`
deliberately pins `==` rather than `>=`/`<=` so an operator mutation is caught.
`:612` `guard_index_dim_catches_an_unpinned_local_model_switch` covers the
unpinned-local-model case where `model_dim` would otherwise collapse into
`index_dim`. **The fix must not weaken either.** The non-forced path must still
reject; only the forced path changes.

## Hypotheses tried

1. **Hypothesis:** the guard is simply misplaced and moving it after
   `stream_index` fixes it.
   **Verdict:** rejected on reading. The guard must still run for non-forced
   syncs, and it must run *before* embedding work, or a mismatched sync writes
   garbage vectors. The fix is a force-aware branch, not a move.

## Fix

Plan (not yet implemented):

1. Thread `force` into the dimension check — either a `guard_index_dim(…, force:
   bool)` parameter or a sibling `resolve_index_dim` that, on mismatch with
   `force=true`, drops and recreates **this project's** vector table at the
   configured width instead of erroring.
2. ~~Scope the drop to the project. `src/retrieval/sqlite_code_store.rs` holds
   vectors for multiple projects; the recreate must leave unrelated projects
   intact. This is the part most likely to be got wrong.~~
   **Struck 2026-08-26 — refuted at the code (`bug-fix-session-log:F-64`).**
   `SqliteVecCodeStore::conn_for` (`src/retrieval/sqlite_code_store.rs:58-77`)
   calls `open_conn(&self.dir, &self.conns, project_id, ".db", …)`: **one SQLite
   file per project**. `code_vec` lives inside that per-project DB and has no
   `project_id` column at all (`:205`, `:236`; `query` reaches project scope only
   by joining `code_chunk` at `:285-286`). Isolation is a property of the
   filesystem layout, not of any predicate, so a plain `DROP TABLE code_vec`
   under `conn_for(project_id)` **cannot** reach another project. Nothing to
   scope; this step does not exist.
2. **(replaces the struck step)** Confirm `DROP TABLE` on a `vec0` virtual table
   also drops its shadow tables. The live DB holds `code_vec_chunks`,
   `code_vec_rowids`, `code_vec_info` and `code_vec_vector_chunks00` alongside
   `code_vec`; if `DROP TABLE` leaves any behind, `ensure_vec_table`'s
   re-`CREATE VIRTUAL TABLE` may collide with a stale shadow. This is the real
   residual risk and it is a sqlite-vec detail, not an architecture problem.
3. Surface the migration in the final `index(action="status")` — a silent
   successful rebuild at a new width is nearly as confusing as the failure.

No SHA, no patch-id — not yet fixed.

## Tests added

None yet. The reporter's requested coverage, which is the right shape:

- build a temporary SQLite-vec store at one dimension, run the project-level
  **forced** rebuild with a different-dimension embedder, then assert: the old
  table was replaced, the final dimension matches the new embedder, and all
  current files are searchable;
- assert a **non-forced** rebuild still rejects the mismatch (guards against
  fixing this by simply deleting the guard);
- ~~assert a sibling project's vectors survive the migration (guards step 2).~~
  **Struck 2026-08-26 — do not write this test** (`bug-fix-session-log:F-64`,
  `W-54`). Sibling projects live in a *separate `.db` file*, so this assertion
  holds for every possible implementation including a wrong one. It would be
  green, permanent, and prove nothing — and it would teach the next reader that
  `code_vec` is a shared table needing careful scoping, which is false.
- instead: assert the `vec0` shadow tables are gone (or correctly reinitialised)
  after the migration, which is the property that can actually break.

## Workarounds

Delete the vector store and rebuild from scratch: remove
`.codescout/embeddings/<project>.db` and run `index(action="build")`. The
guard's own hint documents this, so the behavior is at least self-describing —
it is `force=true`'s advertised contract that is wrong, not the diagnosis.

## Resume

Reconnaissance ran 2026-08-26 and settled the question this section used to ask;
see `bug-fix-session-log:F-64`. Updated next actions:

1. Run `cargo test guard_index_dim` to anchor current behavior. Both existing
   guard tests (`src/retrieval/search.rs:576`, `:612`) must still pass unchanged
   at the end — they are mutation-resistant and the non-forced path must keep
   rejecting.
2. Implement in `src/retrieval/sync.rs:755`. Prefer a `migrate_or_guard` wrapper
   over adding a `force: bool` to `guard_index_dim`: the guard is also called
   from `search_in` (`src/retrieval/search.rs:109`), where `force` is meaningless
   and a bool parameter would have to be threaded as a permanent `false`.
3. The drop itself is a plain `DROP TABLE code_vec` under
   `conn_for(project_id)` — **no project predicate needed**, per Fix step 2 as
   struck. Then let the existing `ensure_vec_table` recreate it at the new width;
   it already takes the dim as a parameter.
4. Check the `vec0` shadow-table question (new Fix step 2) *before* writing the
   migration, since it decides whether `DROP TABLE` alone suffices.

**Do not** write a "sibling project survives the migration" regression test. It
would pass for every possible implementation — siblings live in a different file
— and enter the suite as a permanently green, uninformative assertion. See the
counterfactual in `bug-fix-session-log:W-54`.
## References

- GitHub issue #18 — <https://github.com/mareurs/codescout/issues/18>
- `src/retrieval/sync.rs:755` (guard call), `:777` (force passed onward)
- `src/retrieval/client.rs:406-428` (`guard_index_dim`)
- `src/retrieval/sqlite_code_store.rs` (immutable vector-table width)
- `src/retrieval/search.rs:576,612` (the guard's mutation-resistant tests)
