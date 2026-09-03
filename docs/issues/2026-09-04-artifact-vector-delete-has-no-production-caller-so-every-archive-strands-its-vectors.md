---
kind: bug
status: open
tags:
- cluster/declared-not-wired
closed: null
opened: 2026-09-04
owner: marius
related: []
severity: medium
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

Not attempted. The shape: wire the existing `delete` into the three paths that drop a
catalog row, `move` first since it is the one with a measured, growing population.

`move` is the awkward one and is worth stating before implementing. It is a *re-key*, not a
removal — the right behaviour is arguably to **re-file** the vectors under the new id rather
than delete them, since the content is unchanged and re-embedding costs an embedder pass.
Deleting is correct-but-lossy: the artifact silently leaves the semantic index until the
next `reembed`, which is a *second* silent degradation and not obviously better than the
first. Whichever is chosen, `move`'s response should say which happened.

Note the trait's `delete` carries no project and so fans out over every collection
(`:308-313`). That was affordable while it had no caller; on the archive path it becomes a
per-archive full scan of every artifact collection, so the signature likely wants the
project the caller already holds.

**Do not add a sweep keyed on `project_id` being unresolvable.** Every pre-`99558134` vector
carries `project_id: ''` — 200 of 200 sampled — so that predicate matches the entire legacy
index. See `bug-fix-session-log:W-103`.

## Tests added

None — not fixed. The regression guard must assert the **effect**: after a `move`, no point
remains under the previous id. A test asserting `delete` works passes today, which is the
whole problem.

The durable guard is a reachability check for this family — a production call site for
`ArtifactStore::delete` — extending `tests/tool_reachability.rs`'s approach past `impl Tool`
types. Without it the fix is one commit away from regressing invisibly again.

## Workarounds

None needed urgently: 126 of 29,154 points is 0.43%, and the cost is a few result slots.
To clear them manually, scroll the collection, join `artifact_id` against
`SELECT id FROM artifact`, and delete points whose id is absent. Do this against the
catalog, never against `project_id`.

## Resume

Decide re-file vs delete for `doc(action="move")` — that is the design call gating
everything else. Then read `ArtifactStore::delete` (`src/librarian/artifact_store.rs:307`)
and settle whether it takes a project, since the current fan-out becomes a per-archive full
scan once it has a caller. `move`'s implementation lives under
`src/librarian/tools/` (not `move.rs` — that path does not exist; locate it with
`grep "new_rel_path"`).

## References

- `src/librarian/artifact_store.rs:152` (trait), `:307-313` (the comment naming the gap),
  `:689`, `:709` (the two tests that pass)
- `src/librarian/tools/reindex.rs:286-304` (`orphans_removed`, not a vector sweep)
- `tests/tool_reachability.rs` (family 1 of this class, already gated)
- `docs/issues/archive/2026-09-03-per-project-vector-collections-follow-the-server-cwd-not-the-workspace-param.md`
  — the routing fix whose § *Residue* section measured this collection
- `docs/trackers/bug-fix-session-log.md` § `W-103` — why an empty `project_id` must not be
  the orphan predicate
