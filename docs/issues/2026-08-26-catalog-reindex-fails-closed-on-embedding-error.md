---
status: open
opened: 2026-08-26
closed:
severity: high
owner: marius
related: [docs/issues/archive/2026-08-26-dense-embedder-slot-context-drops-large-embeds.md]
tags: [librarian, catalog, embeddings, silent-failure]
kind: bug
---

# BUG: a reindex embedding failure aborts the whole loop — later targets are never scanned, and the caller loses the catalog report

## Summary

`librarian(action="reindex")` embeds inside the same loop that walks its
targets, with a bare `?` on the embed call. One embedding transport failure
aborts `call` outright: the caller gets an error instead of the reindex
envelope, git-commit backfill is skipped, and — the real damage — **every
remaining target in a multi-target scope is never walked at all**. Nothing
durable records that the catalog refresh was left half-done.

Imported from GitHub issue #19 (reporter: mic-urs, 2026-08-26). **The report's
diagnosis is inverted and the corrected mechanism is worse in one dimension and
better in another** — see Root cause.

## Symptom (Effect)

`librarian(action="reindex")` returns the embedding transport error. No
`{added, updated, removed, unchanged, embedded, embed_note}` envelope is
produced. New or modified tracker/bug markdown may then be absent from
`artifact(action="find", …)`, and there is no durable, prominent indication that
catalog results are stale because the last refresh failed. A caller may receive
an `unindexed_files` hint, which does not say *why*.

## Reproduction

1. Configure an embedding backend that cannot accept requests (wrong port, or
   a payload over the model ceiling — see the related bug).
2. `librarian(action="reindex", scope="umbrella")` — a scope with ≥2 targets.
3. Create or modify a tracker/bug markdown file under the *second* target.
4. `artifact(action="find", kind="bug", filter={"status": {"in": ["open", "investigating"]}})`.

Expected under the corrected mechanism: the first target's rows are refreshed,
the second target's are not, and the caller cannot tell which from the error.

## Environment

- codescout `experiments` at `d5ed4d6f`; catalog is machine-local and
  git-ignored, so a half-done refresh is not recoverable from the repo.

## Root cause

Read from `src/librarian/tools/reindex.rs:208-253` on 2026-08-26.

The order is **catalog first, embeddings second**, inside a per-target loop:

```rust
for abs_root in &targets {
    let (report, embed_queue) = {
        let cat = ctx.catalog.lock();
        indexer::index_repo_sync(&cat, &ctx.rules, abs_root, &ignore,
                                 want_embeddings, force, reembed)?      // :208-218  catalog work
    };
    total_added += report.added;  /* … */                                // :221-225

    if let (Some(svc), Some(store)) = (ctx.embedding.as_ref(), ctx.artifact_store.as_ref()) {
        for (id, title, chunk_text) in &embed_queue {
            let vec = svc.embed_artifact(title.as_deref(), chunk_text).await?;  // :236  ← aborts `call`
            store.upsert(&project_id, id, &vec).await?;                          // :237  ← also aborts
            total_embedded += 1;
        }
    }

    { /* backfill_commits — never reached for this or any later target */ }       // :243-254
}
```

### Correcting the report

The issue states the reindex *"returns the embedding transport error before
refreshing the catalog"*. That is **false**: `index_repo_sync` — the scan,
classification and frontmatter refresh — completes and commits to the catalog
*before* any embedding is attempted. The reporter's requested acceptance
criterion "with an unavailable embedder, a reindex still discovers and
classifies new artifacts" is therefore **already satisfied for the first
target**.

What is actually broken, and is not in the report:

1. **Multi-target abort.** The `?` at `:236` escapes the `for abs_root in
   &targets` loop. Under `scope="umbrella"` or `scope="all"`, targets after the
   failing one are **never walked** — their catalog rows are genuinely stale,
   exactly as the report claims, just for a different reason and only past the
   failure point. This is the severe half.
2. **The report is destroyed, not just the vectors.** `total_added`,
   `total_updated`, `total_removed` and `total_unchanged` are accumulated into
   locals and only serialized at `:263-296`. An abort at `:236` throws away work
   that *did* succeed, so the caller cannot distinguish "catalog refreshed,
   vectors missing" from "nothing happened".
3. **`backfill_commits` is skipped** (`:243-254`), leaving the `commits` table
   short. Its own comment records that a silent failure here produces the
   "commit not indexed" error that misleads callers into re-running reindex —
   so this abort re-creates a bug that was already fixed once, by a different
   route.
4. **No durable degraded state.** `embeddings_enabled` and `embed_note` exist
   (`:266`, `:274-282`) and are good, but they are *envelope* fields — and the
   envelope is precisely what an abort prevents. Nothing is persisted, so the
   next `artifact(action="find")` has no way to know the last refresh failed.

`measured 2026-08-26: read_file src/librarian/tools/reindex.rs:196-296 — ordering
and the two bare `?` confirmed by inspection; not executed against a downed
embedder this session.`

## Evidence

### The abort points (`read_file`, 2026-08-26)

```
236: let vec = svc.embed_artifact(title.as_deref(), chunk_text).await?;
237: store.upsert(&project_id, id, &vec).await?;
```

Both are inside `for (id, title, chunk_text) in &embed_queue`, which is inside
`for abs_root in &targets`. Neither is wrapped.

### Partial credit where due — the envelope already models this

`:265-282` already reports `embedded`, `embeddings_enabled` and a written-out
`embed_note`, added for
`docs/issues/archive/2026-07-25-reindex-reembed-noop-without-force.md`. The
design intent the issue asks for is present; it is unreachable on the failure
path. Test `envelope_reports_embedding_state` (`:333-364`) covers the
no-embedder branch and explicitly notes the populated path is untested because
`TestToolContextBuilder` has no `with_embedding` setter — which is also why this
bug has no regression test today.

## Hypotheses tried

1. **Hypothesis** (from the report): catalog refresh is gated behind embedding
   and never runs when the embedder is down.
   **Test:** read `src/librarian/tools/reindex.rs:208-253`.
   **Verdict:** rejected. `index_repo_sync` runs first and commits. The real
   defect is loop abort across targets plus a lost envelope.
2. **Hypothesis:** the missing piece is a new degraded-state field.
   **Verdict:** deferred — partly already built (`embeddings_enabled`,
   `embed_note`). The gap is reachability on the error path and *durability*
   across calls, not the field's existence.

## Fix
### Progress 2026-08-26 — step 1 shipped, steps 2-3 still open

- **SHA:** `69e78a2f` (`experiments`)
- **patch-id:** `139205fb275ae4ec0329d5816a0ba4be243e9a6e`

`fix(librarian): collect reindex embed failures instead of aborting the target
loop`. The patch-id is the durable anchor — `experiments` is rebased after every
ship, so the SHA will orphan.

**Step 1 is done.** The embed loop no longer propagates. Failures accumulate into
`embed_errors` and surface as `embed_error_count` plus a 20-entry capped sample,
mirroring the `backfill_errors` field immediately below it in the same function.
`embed_note` now leads with `DEGRADED` when anything failed and states explicitly
that `artifact(action="find")` is accurate while semantic search will not surface
the un-vectored artifacts. That fixes all four numbered consequences in § *Root
cause*: the multi-target abort, the destroyed report, the skipped
`backfill_commits`, and — partially — the missing degraded signal.

**Also shipped:** `TestToolContextBuilder::with_embedding` and
`::with_artifact_store` (`src/librarian/tools/mod.rs`). This was step 4, and it
was a genuine prerequisite: the fields already existed, only the setters were
missing, which is precisely why this code path had no coverage. Both are needed
together — the embed block is gated on `if let (Some(svc), Some(store))`, so a
test setting only the embedder would have passed while exercising nothing.

### Progress 2026-08-27 — step 2 shipped, step 3 still open

- **SHA:** `050ec61a` (`experiments`)
- **patch-id:** `3676e85cd8937241edf5079cefff20eceb1cd3d1`

`fix(librarian): persist a durable degraded marker for reindex embed failures`.

**Step 2 is done**, using the precedent named in § Resume below
(`IndexState.last_sync_skipped_count`) applied to a simpler store than that fix
needed: the catalog already carries a generic key-value table, `catalog_meta`
(`key TEXT PRIMARY KEY, value TEXT`), with `get_meta`/`set_meta` helpers in
`src/librarian/catalog/gc.rs` previously used only for `gc_grace_days`. No new
sidecar, no schema migration.

`reindex.rs`'s `call` now writes `last_reindex_embed_error_count` and
`last_reindex_embed_errors_sample` (JSON, 20-capped) to `catalog_meta` on every
run where embeddings were attempted (`want_embeddings`), including a clean 0/[]
run — the same "clean run clears the marker" invariant used for the sibling fix,
so a repaired embedder un-degrades the catalog instead of leaving it stuck.
Gated on `want_embeddings` specifically: a run with no embedder configured has no
evidence about embed health either way, so it must not overwrite a real marker
from an earlier run.

`find.rs`'s `build_hints` reads the marker back and, when the count is nonzero,
adds `catalog_degraded` / `catalog_degraded_hint` to the response — the read
side of the exact `unindexed_files` / `unindexed_hint` pattern already in that
function. Unlike `unindexed_files`, it is not scope-gated: the marker is one
reindex call's aggregate across every target it walked, not a per-artifact
fact, so it surfaces regardless of the query's `scope`.

**Step 3 remains open** and is unchanged by this: reconciling catalog
freshness, embedding freshness, and queryability into one state model across
`index(action="status")` (the code-chunk index) and `artifact(action="find")`
(the librarian catalog) is still a cross-subsystem design task, not an edit.

**Verified:** `cargo fmt`, `cargo clippy --all-targets -- -D warnings`,
`cargo test --lib` → 4414 passed, 0 failed, 8 ignored.
## Tests added

`an_embed_failure_still_walks_every_target_and_reports_it`
(`src/librarian/tools/reindex.rs`), shipped in `69e78a2f`.

Uses **two roots**, so the default `scope="all"` yields two targets — without
that the test would not exercise the loop the `?` used to escape, which is the
severe half of this bug. A `FailingEmbedder` bails on every call. Asserts:

- `call` returns `Ok`, not `Err` — the core behavioural change;
- `added == 2`, so **both** targets' artifacts reached the catalog;
- `embed_error_count == 2` — every failure counted, not just the first;
- `embed_note` contains `DEGRADED`;
- `backfill_error_count == 0`, proving `backfill_commits` still ran. It sits
  *after* the embed block, so the old `?` skipped it entirely.

**Verified red before green.** With the pre-fix `?` restored, the test fails with
`connection refused` propagating out of `call`. This matters more than usual here:
the test's central assertion is that a function returns `Ok`, and a test asserting
`Ok` is exactly the shape that can pass without the fix if the failure path is
never actually reached. Confirming red is what rules that out.

**Step 2, shipped in `050ec61a`:**

- `an_embed_failure_persists_a_durable_catalog_meta_marker`
  (`src/librarian/tools/reindex.rs`) — after a run with a failing embedder,
  `catalog_meta` holds `last_reindex_embed_error_count == "2"` and a
  2-element JSON error sample, read directly off `ctx.catalog.lock().conn`
  via `gc::get_meta`.
- `a_clean_reindex_after_a_failure_clears_the_persisted_marker`
  (`src/librarian/tools/reindex.rs`) — a `FlakyEmbedder` toggled from
  failing to succeeding between two `call`s; asserts the marker reads `"1"`
  after the failing run and `"0"` after the clean one. Same invariant as
  `sync_project_clears_a_previously_recorded_skip_count_on_a_clean_run` for
  the sibling bug — a marker that only ever increments would misreport a
  repaired embedder as permanently degraded.
- `catalog_degraded_hint_appears_after_a_persisted_embed_failure_then_clears`
  (`src/librarian/tools/find.rs`) — seeds `catalog_meta` directly, asserts
  `find`'s `hints.catalog_degraded` / `catalog_degraded_hint` appear on the
  very next call (not just the call that wrote the marker), then clears the
  marker and asserts the hint disappears too.

All three verified RED first: the get/set helpers already existed (from
`gc.rs`), so RED came from the assertions failing against an unwritten key
(`None` where a value was expected), not a compile error — confirmed by
reading the actual panic message before writing the implementation.

**Not yet covered:** step 3. There is no test for status-surface agreement,
because that state model does not exist yet.

## Workarounds

Run `librarian(action="reindex", scope="repo")` per repo rather than `umbrella`
or `all`, so an embedder failure can only cost the one target you named — and
re-run each explicitly after the embedder is healthy. Treat any reindex that
returns an error, rather than an envelope, as "catalog state unknown".

## Resume

**Update 2026-08-27:** steps 1 and 2 are both shipped (`69e78a2f`, `050ec61a`).\nWhat remains is step 3 only — a **design decision, not an edit** — so do not\nstart by opening `reindex.rs` or `find.rs` again for this bug; both are done.\n\nReconcile catalog freshness, embedding freshness, and queryability into one\nstate model. Right now there are **two** independently-written, independently-\nread degraded markers, not one:\n\n- Code index: `IndexState.last_sync_skipped_count` / `_sample`\n  (`src/retrieval/index_state.rs`, written by `sync_project`, read by\n  `index(action=\"status\")`) — shipped fixing\n  `docs/issues/archive/2026-08-26-index-status-claims-complete-without-checking-coverage.md`.\n- Librarian catalog: `catalog_meta.last_reindex_embed_error_count` / `_sample`\n  (`src/librarian/catalog/gc.rs` get/set, written by `librarian(action=\"reindex\")`,\n  read by `artifact(action=\"find\")`'s `build_hints`) — shipped by step 2 above.\n\nBoth now exist, both work, and both were deliberately built as parallel\nimplementations of the same shape rather than one shared abstraction — that\nwas the right call for shipping each independently (different store, different\nwrite path, no shared code to extract without coupling two subsystems that\ndon't otherwise depend on each other). **Step 3 is deciding whether that\nremains two facts a caller must know to check separately, or becomes one\nqueryable health surface** — and if the latter, where it lives (a shared\ntrait/module both `sync_project` and `reindex.rs`'s `call` write through, vs.\na single aggregating tool that reads both `catalog_meta` and the `IndexState`\nsidecar). Read both shipped implementations before designing this — they are\nnow the two data points step 3 has to unify, not precedents to repeat a third\ntime.\n
## References

- GitHub issue #19 — <https://github.com/mareurs/codescout/issues/19>
- `src/librarian/tools/reindex.rs:196-296` (`call`), `:236-237` (the bare `?`s),
  `:243-254` (`backfill_errors`, the pattern to copy)
- `docs/issues/archive/2026-07-25-reindex-reembed-noop-without-force.md` — why
  `embedded` / `embed_note` exist at all
- `docs/issues/archive/2026-08-26-dense-embedder-slot-context-drops-large-embeds.md` —
  a live way to *cause* the embed failure that triggers this abort
