---
kind: bug
status: mitigated
tags:
- cluster/declared-not-wired
closed: null
opened: 2026-09-04
owner: marius
related: []
severity: medium
unverified: 'the ARCHIVE path is fixed and gate-verified (049c6c97); the title''s claim is still literally true for the other two. `doc(action="delete")` and a file removed from disk both still drop a catalog row without touching Qdrant, and `ArtifactVectorStore::delete` still has no production caller. Kept out of the archive folder for that reason. Also not retroactive: the 7 orphan artifacts / 126 points measured here remain.'
---

# BUG: the artifact vector `delete` has no production caller, so every `doc(action="move")` and every catalog-row removal strands that artifact's vectors

## Summary

`ArtifactStore::delete` removes every chunk vector belonging to an artifact. It is
implemented on both backends, covered by two dedicated tests, and **called by nothing in
production** — the Qdrant impl says so in its own comment. Catalog identity is
`id = sha256(abs_path)`, so archiving a bug file re-keys the artifact and its old vectors
stay in the collection forever under an id no catalog row resolves. Measured today:
**7 orphan artifacts holding 126 points** in codescout's collection, one of which this
session created by archiving a bug file the ordinary, documented way.

## Symptom (Effect)

Vectors survive their artifact. A KNN hit on one is returned by Qdrant, fails to resolve to
a catalog row, and is silently dropped at hydration — consuming a result slot and appearing
in `hints` only as an anonymous count:

```
hints: {'cap_suppressed': 160, 'unresolved': 3}
```

Nothing errors. The only visible cost is a search that quietly returns fewer usable results
than it examined, and the population grows by one artifact's worth of vectors per archive.

## Reproduction

```
git rev-parse HEAD          # d71e0e08 at time of filing
```

1. Note a bug file's id and its vector count.
2. Archive it the documented way:
   `doc(action="move", id=<id>, new_rel_path="docs/issues/archive/<same-name>.md")`.
   The response reports `id_changed: true` and `history_grafted` — events, links,
   observations and augmentation all move.
3. Scroll the collection for the **old** id. Its points are all still there.

Observed live this session: archiving `dca8db739f078ef2` left its **29 points** behind
under that id while the catalog row moved to `1a4dd3c6a94e173a`.

## Environment

codescout `experiments` @ `d71e0e08`, Linux, Qdrant at `localhost:6333`,
collection `artifact_chunks_codescout_dc6a871595179329` (29,154 points),
catalog `~/.local/share/librarian/catalog.db`.

## Root cause

**The delete is declared and never wired.** `src/librarian/artifact_store.rs:152` declares
`async fn delete(&self, artifact_id: &str) -> Result<()>` on the trait; the Qdrant impl at
`:307` states the gap itself:

```rust
// Fans out over EVERY artifact collection, because the trait's `delete`
// carries no project and an artifact id does not say which project it
// belongs to. Scanning them all is affordable precisely because this has
// no production caller today — it exists for an explicit vector purge —
// and being wrong in the other direction would silently strand vectors.
```

The last clause names this bug as the thing being avoided, and the mechanism that would
avoid it is the one with no caller.

Three paths therefore drop a catalog row and leave the vectors:

- **`doc(action="move")`** — `id = sha256(abs_path)`, so a move necessarily re-keys. The
  graft moves catalog-side history and touches no vector.
- **`doc(action="delete")`** — cascades over `artifact_augmentation`, links, observations
  and events (FK `ON DELETE CASCADE`), none of which is the vector store.
- **a file removed from disk + reindex** — the per-file walk's `removed` count.

**Nothing sweeps the residue afterwards.** `orphans_removed`
(`src/librarian/tools/reindex.rs:286-304`) reads like the backstop and is not: it counts
**catalog rows** for de-registered *repos*, runs only when
`effective_scope == Scope::All && a.repo.is_none()`, and calls `delete_orphan_repos`, which
never touches Qdrant.

*measured 2026-09-04: full scroll of all 29,154 points, joined against
`SELECT id, abs_path FROM artifact` — 7 artifact_ids present in the collection and absent
from the catalog, holding 126 points.*

## Evidence

### The orphan population, identified positively rather than by elimination

`id = sha256(abs_path)[:16]` was confirmed against a known pair before being used as an
oracle: the `move` response reported `previous_id: dca8db739f078ef2`, and hashing the
pre-move absolute path reproduces that id exactly. Hashing all **3,343** paths that have
ever existed in this repo's history then names five of the remaining six:

```
1067c6c157f0d9ee  docs/issues/2026-09-03-librarian-is-an-embedding-loop-omitted-from-the-embedding-loop-exemption.md
192b99619018ed2c  docs/issues/2026-09-03-two-file-templates-propagate-retired-call-forms-into-new-files.md
3d1349b4e4ed225c  docs/issues/2026-09-03-doc-id-param-routing-omits-the-augment-action.md
af3a7ffe8626562c  docs/issues/2026-09-03-a-bare-heading-query-cannot-reach-the-exact-match-tiers.md
b2ef31cb44c2e9b6  docs/issues/2026-09-03-librarian-guard-refuses-text-grammar-while-promising-it-works.md
411a37951ea7b02e  << unidentified >>
```

All five are bug files archived on 2026-09-03 — the documented flow, performed correctly.
With `dca8db739f078ef2` that is **6 of 7 positively identified as archive-moves**.

`411a37951ea7b02e` is **not identified**, and is recorded as unidentified rather than
assumed: it survives ~15,000 candidates — every historical path in codescout (3,343),
prompt-engineering (705) and claude-plugins (777), plus all three worktree prefixes. A
temp-dir path from an eval arm would fit and is unenumerable by construction, so this stays
open rather than being closed by elimination over a population no instrument spans.

### The tests pass and cannot see it

`delete_is_idempotent` (`src/librarian/artifact_store.rs:689`) and
`deleting_an_artifact_removes_every_one_of_its_chunk_points` (`:709`) both construct
`InMemoryArtifactStore` and call `delete` directly. They establish that the implementation
is correct, which it is. Neither can observe that no shipping code path reaches it — the
same shape as `ListFunctions` / `ListDocs` / `GetUsageStats`, which carried 21 passing tests
while registered nowhere.

The second test's own doc comment states this bug one level down:

```
/// The failure this exists for is silent: an artifact owns N points, so a
/// delete that removes one leaves N-1 orphans that still answer queries and
/// that no sweep collects.
```

That is exactly right, and exactly the defect — at **chunk** grain. The identical failure at
**artifact** grain (an artifact owns N points; nothing calls delete at all, so N orphans
still answer queries and no sweep collects them) is what ships. The author reasoned the
class through carefully and guarded the half a unit test can reach.

`tests/tool_reachability.rs` is the standing guard for that class, and its own header scopes
it to *"a type that `impl Tool` but that no agent can reach"* — **family 1**. A trait method
with no production caller is a different family, and `issue-clusters:IC-3`'s row already
records `2 of 3 families open`.

## Hypotheses tried

1. **Hypothesis:** `reindex`'s `orphans_removed` already sweeps stale vectors.
   **Test:** read `src/librarian/tools/reindex.rs:286-304` and the response builder at `:505`.
   **Verdict:** rejected — it is catalog-row-scoped, `Scope::All`-only, and calls
   `delete_orphan_repos`, which never touches Qdrant. The name reads like a vector sweep,
   which is why it was checked first.
2. **Hypothesis:** the orphans are eval-arm temp-dir artifacts, misfiled into codescout's
   collection by the routing bug fixed at `99558134`.
   **Test:** hash every historical path in three repos and all worktree prefixes.
   **Verdict:** rejected for 6 of 7 — those are archive-moves of real repo files. Not
   rejected for `411a37951ea7b02e`, which remains consistent with it and unproven.
3. **Hypothesis:** a codescout reindex will clear them.
   **Test:** reason from the walk's deletion logic plus the join above.
   **Verdict:** rejected — the walk removes rows for files no longer on disk. An archived
   bug file **is** on disk, at its new path, under a new id. The old id is not a file the
   walk visits, so nothing revisits it.

## Fix

Fixed in `049c6c97` — *fix(librarian): a move RE-FILES its chunk vectors instead of stranding or
deleting them*. patch-id `747f9f72143f21f3d35ce24e5b421b1e9ccb9e33`.

**Re-file, not delete** — the design question this record was filed open, now settled. A move is
a re-key: the bytes are unchanged, so deleting would trade a silent orphan for a silent hole in
the index until the next `reembed`. Points are keyed on `chunk_id` with `artifact_id` as an
INDEXED payload field, so re-filing is a payload write on Qdrant and a join-row `UPDATE` on
sqlite. **Nothing is re-embedded.**

- **`src/retrieval/artifact.rs`** — `artifact_refile`, a filtered `set_payload`. Counts first,
  because `set_payload`'s `UpdateResult` reports operation status rather than how many points
  matched, and "re-filed 0" is exactly the outcome a caller needs to see.
- **`src/librarian/artifact_store.rs`** — `ArtifactVectorStore::refile` on all three backends.
  **It carries no project, deliberately.** `mv` holds a root it could pass, which is the reason
  not to take one: two call sites deriving the same project by their own routes and disagreeing
  is precisely what `99558134` fixed. A signature that never claims to know the project cannot
  be wrong about it.
- **`src/librarian/tools/mv.rs`** — the call, between the upsert and the graft, plus
  `vectors_refiled` in the response.

### The ordering is load-bearing, and so is the block

`refile` must run **after** the new artifact row exists (sqlite's FK rejects the re-point
otherwise) and **before** the graft drops the old row (after it, there is nothing left to
re-point). Both neighbours are stated at the call site.

The catalog guard moved into an explicit block, and **that block is the fix rather than a
tidy-up**. `refile` is async and the guard is a `parking_lot::MutexGuard`, so holding it across
the await does not compile — but the substance is that `SqliteVecArtifactStore::refile`
re-acquires that same mutex, and parking_lot's is not reentrant, so a version that merely
silenced the compiler would **hang that backend on every move**. Caught by rustc, not by a test,
and no test could have caught it: every in-tree test constructs `InMemoryArtifactStore`, whose
`refile` takes no lock and therefore cannot deadlock.

Two dead ends are recorded in the comment because both are things a reader would otherwise
retry:

1. **Reassigning `cat` after the await** — same error. rustc keeps the binding live in the
   generator's state machine for its whole lexical scope even once moved, so `drop` alone does
   not help; only ending the scope does.
2. **Dropping inside the `if`** — compiles, then deadlocks on the `new_id == a.id` path when the
   re-lock runs against a still-held guard. A workaround that compiles and fails only on the
   branch nobody exercises is worse than one that does not compile.
## Tests added

`move_refiles_chunk_vectors_onto_the_new_id` (`src/librarian/tools/mv.rs`) is the one that would
have caught this, and it had to live at the `mv` layer. The store's own tests prove `refile`
works *when called*, which was never in doubt — what was broken is that **nothing called it**,
and every existing `mv` test passes on the pre-fix code because a move that stranded its vectors
returns a byte-identical response.

It asserts **both directions**, because the two backends failed in opposite ones: `under(new_id)
== 3` alone would pass the Qdrant bug if it also copied, and `under(old_id) == 0` alone would
pass a delete. It also carries a second artifact that must not move — without it, a `refile`
that re-pointed the whole store satisfies every other assertion.

Three store-level tests
(`refile_moves_every_chunk_of_one_artifact_and_no_others`,
`refile_preserves_the_vectors_and_the_chunk_ids`,
`refile_of_an_artifact_with_no_vectors_is_zero_not_an_error`) cover the trait. The second
asserts vectors **by value**: a `refile` implemented as delete-then-reinsert keeps every count
identical, and "no embedder ran" is observable only as unchanged bytes.

### Mutation-tested, because green is not evidence

Five mutations of the production path, control green, **all killed**:

| mutation | killed by |
|---|---|
| never call `refile` (the shipped bug) | `move_refiles_chunk_vectors_onto_the_new_id` |
| call `delete` instead (the rejected design) | same |
| re-point only the first chunk | + `refile_moves_every_chunk_of_one_artifact_and_no_others` |
| re-point every chunk, ignore the old id | + `refile_of_an_artifact_with_no_vectors_is_zero_not_an_error` |
| absurd no-op (control on the RUNNER) | + `refile_preserves_the_vectors_and_the_chunk_ids` |

The absurd control earned its place immediately, twice. The first run reported the **control**
RED because the runner string-matched `error[E` in the output instead of reading exit codes; and
the first "never call refile" mutation was killed by a **compile failure**, which is not a kill
— re-typed as `Some(_store) => Some(0u64)` it reds the right test. Both are
`bug-fix-session-log:F-113` recurring in a second language.
## Workarounds

None needed urgently: 126 of 29,154 points is 0.43%, and the cost is a few result slots.
To clear them manually, scroll the collection, join `artifact_id` against
`SELECT id FROM artifact`, and delete points whose id is absent. Do this against the
catalog, never against `project_id`.

## Resume

N/A — fixed and gate-verified.

Two things deliberately **not** done here, recorded so nobody reads them as oversights:

- **The 126 pre-existing orphan points are not swept.** This fix is not retroactive: it stops
  new orphans, and the 7 artifacts already stranded stay until something removes them. 0.43% of
  the collection, and deleting from a live collection is a separate change with its own blast
  radius.
- **`ArtifactVectorStore::delete` still has no production caller.** `refile` covers `move`,
  which is the path with a measured, growing population. `doc(action="delete")` and a
  removed-from-disk file both still drop a catalog row without touching Qdrant. That is the
  remaining half of this class and wants its own record rather than being quietly folded in
  here.
## References

- `src/librarian/artifact_store.rs:152` (trait), `:307-313` (the comment naming the gap),
  `:689`, `:709` (the two tests that pass)
- `src/librarian/tools/reindex.rs:286-304` (`orphans_removed`, not a vector sweep)
- `tests/tool_reachability.rs` (family 1 of this class, already gated)
- `docs/issues/archive/2026-09-03-per-project-vector-collections-follow-the-server-cwd-not-the-workspace-param.md`
  — the routing fix whose § *Residue* section measured this collection
- `docs/trackers/bug-fix-session-log.md` § `W-103` — why an empty `project_id` must not be
  the orphan predicate
