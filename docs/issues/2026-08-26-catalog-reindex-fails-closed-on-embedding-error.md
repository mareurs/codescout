---
status: open
opened: 2026-08-26
closed:
severity: high
owner: marius
related: [docs/issues/2026-08-26-dense-embedder-slot-context-drops-large-embeds.md]
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

**Steps 2 and 3 remain open, and they are the design half:**

- **2 — a durable degraded marker.** `embed_note` is an *envelope* field. It is
  now reachable on the failure path (it wasn't before), but it still does not
  outlive the call: a later `artifact(action="find")` has no way to know the last
  refresh was partial. That needs persisted state.
- **3 — reconcile the status surfaces.** Catalog freshness, embedding freshness
  and queryability are three distinct facts reported independently by surfaces
  that can disagree. They should derive from one state model. This is the
  report's "related status problem", and it overlaps
  `docs/issues/2026-08-26-index-status-claims-complete-without-checking-coverage.md`
  — the two should probably be designed together rather than patched separately.

**Verified:** `cargo fmt`, `cargo clippy --all-targets -- -D warnings`,
`cargo test` → 4474 passed, 0 failed, 46 ignored.

Plan (not yet implemented). Order matters — (1) is small and removes most of the
harm:

1. **Collect embed errors instead of propagating them.** Mirror the existing
   `backfill_errors` pattern already in this function (`:198`, `:243-254`): push
   failures into an `embed_errors: Vec<String>`, keep the loop going, and report
   `embed_error_count` / `embed_errors` in the envelope. This alone fixes the
   multi-target abort, the lost report, and the skipped commit backfill.
2. **Persist a degraded marker** so staleness outlives the call — last failed
   refresh time and stage, readable by `artifact(action="find")`, which should
   then expose a machine-readable *and* human-readable stale-catalog warning.
3. **Reconcile the status surfaces** (the report's "related status problem").
   Catalog freshness, embedding freshness and queryability are three distinct
   facts and should derive from one state model rather than being reported
   independently by surfaces that can disagree.
4. Give `TestToolContextBuilder` a `with_embedding` setter — without it, none of
   the above is testable at this layer, and the existing test says so.

Steps 2 and 3 are a design change and should be scoped separately from 1.

No SHA, no patch-id — not yet fixed.

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

**Not yet covered:** steps 2 and 3. There is no test for a durable degraded
marker or for status-surface agreement, because neither exists yet.

## Workarounds

Run `librarian(action="reindex", scope="repo")` per repo rather than `umbrella`
or `all`, so an embedder failure can only cost the one target you named — and
re-run each explicitly after the embedder is healthy. Treat any reindex that
returns an error, rather than an envelope, as "catalog state unknown".

## Resume

Step 1 shipped in `69e78a2f`; the `src/librarian/tools/reindex.rs` work is done.
What remains is a **design decision, not an edit**, so do not start by opening
that file again.

Decide where durable index-health state lives, for catalog freshness and embedding
freshness jointly. Then read
`docs/issues/2026-08-26-index-status-claims-complete-without-checking-coverage.md`
§ *Fix* before writing anything: that bug's `(0,0)`-only status discriminator and
this bug's missing degraded marker are the same missing abstraction seen from two
tools (`index(action="status")` and `librarian(action="reindex")` /
`artifact(action="find")`). Designing them separately will produce two
disagreeing health surfaces, which is the third acceptance criterion this bug
already asks for.

Concretely: name the state model first (what facts, stored where, written by
whom), and only then pick the two call sites. `src/retrieval/index_state.rs`
already persists a sidecar for git-freshness detection and is the closest
existing precedent — read it before inventing a new store.
## References

- GitHub issue #19 — <https://github.com/mareurs/codescout/issues/19>
- `src/librarian/tools/reindex.rs:196-296` (`call`), `:236-237` (the bare `?`s),
  `:243-254` (`backfill_errors`, the pattern to copy)
- `docs/issues/archive/2026-07-25-reindex-reembed-noop-without-force.md` — why
  `embedded` / `embed_note` exist at all
- `docs/issues/2026-08-26-dense-embedder-slot-context-drops-large-embeds.md` —
  a live way to *cause* the embed failure that triggers this abort
