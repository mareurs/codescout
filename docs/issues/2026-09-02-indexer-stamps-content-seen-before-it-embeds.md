---
kind: bug
status: open
tags:
- cluster/gate-keyed-on-unobservable-event
closed: null
opened: 2026-09-02
owner: marius
related: []
severity: high
unverified: The ENTRY path is not established — measurement proves 729 artifacts are trapped and cannot leave, but not which run stamped each one. Candidate entries (a run with no embedder, a per-artifact embed error, an artifact predating embeddings) are not distinguished; the catalog keeps no per-artifact embed-attempt record, so they may not be distinguishable after the fact.
---

# BUG: the indexer stamps content as seen before it embeds it, trapping 729 artifacts permanently unembeddable

## Summary

`index_repo_sync` writes an artifact's `file_sha256` to the catalog at
`src/librarian/indexer.rs:302`, then decides whether to embed it at `:309` — and that
decision reads the very stamp just written. Any run that reaches `:302` without
reaching `:309`'s true branch commits the content as "seen" while leaving it
unembedded, and every later run then computes `content_unchanged == true` and skips
it. The state is **absorbing**: no automatic path can leave it.

Measured today, **729 of codescout's 1373 catalog artifacts have no searchable
representation of any kind** — no legacy per-artifact vector, no chunk rows — and no
ordinary reindex can give them one. That is 53% of the corpus invisible to
`semantic_search`, with no error, no warning, and no count anywhere in the system that
reports it.
## Symptom (Effect)

No error is emitted. `librarian(action="reindex")` reports success and a plausible
`unchanged: N`. The observable is a silent absence: artifacts that exist, are
classified, carry correct metadata, and return from `artifact(find)` — but never
appear in `semantic_search` results, because they have no vector.

The nearest thing to a visible symptom is that two artifacts written **41 seconds
apart** land on opposite sides of the split:

```
abs_path: .../docs/trackers/codescout-usage-hookify.md   updated: 2026-09-02 11:53:26   HAS a vector
abs_path: .../docs/trackers/observer-blindness.md        updated: 2026-09-02 11:53:46   has NO vector
```

## Reproduction

At `de434ca5959682a1625e8794cd6bffbbc3e15142` on `experiments`. Two runs over one
file, no `force_*` flags — i.e. exactly what `index_repo` does:

1. **Run A — index with no embedder.** `index_repo_sync(..., want_embeddings=false, false, false)`.
   Content differs from the catalog row (or no row exists), so `content_unchanged` is
   `false`, `:262`'s early return is not taken, and `:302` commits the new
   `file_sha256`. `:309` requires `want_embeddings`, which is `false` → nothing queued.
2. **Run B — index with an embedder.** `index_repo_sync(..., want_embeddings=true, false, false)`.
   `content_unchanged` is now `true` *because run A committed the sha*. If metadata
   also matches, `:262` returns early and `:276` requires `force_embed` → skipped. If
   metadata differs, the early return is skipped, the row is rewritten, and control
   reaches `:309`, where `!content_unchanged` is `false` and `force_embed` is `false`
   → skipped.

The artifact now has a current `file_sha256` and no vector. Repeat run B any number
of times; the result does not change.

**A metadata-only edit does not repair it** — that is the second branch of step 2, and
it is the branch that makes the state absorbing rather than merely sticky: touching
the file's frontmatter refreshes `updated_at`, making the row *look* freshly processed
while `:309` still declines to embed.

To observe the live population instead (read-only, no writes):

```
sqlite3 -noheader "file:$HOME/.local/share/librarian/catalog.db?mode=ro" \
  "SELECT a.file_sha256||'  '||a.abs_path FROM artifact a
     LEFT JOIN artifact_vec_rowids r ON r.id=a.id
    WHERE r.id IS NULL AND a.abs_path LIKE '<repo>/%';" > novec.sums
sha256sum -c novec.sums 2>/dev/null | grep -c ': OK$'
```

Every `OK` line is an artifact whose catalog hash already matches disk and which has
no vector — i.e. one that is trapped.

## Environment

- Branch `experiments`, HEAD `de434ca5959682a1625e8794cd6bffbbc3e15142`, 2026-09-02.
- Catalog `~/.local/share/librarian/catalog.db` (machine-local, gitignored).
- Linux 7.1.9-zen1-2-zen. Queries run through `run_command` with `mode=ro`.
- **The system `sqlite3` CLI cannot load `vec0`**, so `artifact_vec` / `artifact_vec_v2`
  are not directly queryable from the shell (`Parse error: no such module: vec0`). All
  counts below go through the plain shadow tables `artifact_vec_rowids` /
  `artifact_vec_v2_rowids`, which mirror the virtual tables' id sets.

## Root cause

**The gate's condition is an event outside the gate's observation boundary, so it
reads a proxy that the gate itself has already falsified.**

`file_sha256` is written by `artifact::upsert_and_mint_slug` (`src/librarian/indexer.rs:302`),
a function that knows nothing about embeddings. It is then read seven lines later, at
`:309`, as the condition for *"does this content still need a vector?"*:

```rust
// src/librarian/indexer.rs:244-247
let content_unchanged = existing
    .as_ref()
    .map(|ex| ex.file_sha256 == sha)
    .unwrap_or(false);

// :302
artifact::upsert_and_mint_slug(cat, &row)?;

// :309
if want_embeddings && (!content_unchanged || force_embed) {
    embed_queue.extend(embed_queue_items(cat, &id, title, body)?);
}
```

The stamp means *"this content has been written to the catalog."* It is read as
*"this content has been embedded."* Those coincide only when every write is
accompanied by an embed, which `:309`'s own condition is what decides — so the write
at `:302` makes a promise that the code at `:309` is free to break, and nothing
reconciles the two. `:302` is unconditional; `:309` is not.

Two further sites make the state absorbing rather than recoverable:

- `src/librarian/indexer.rs:689` — `index_repo` calls
  `index_repo_sync(cat, rules, abs_root, ignore, want, false, false)`, hardcoding both
  `force_rewalk` and `force_embed` to `false`. This is the path every ordinary reindex
  takes, so **no automatic caller can supply the one lever that escapes the trap.**
- `src/librarian/indexer.rs:276` — the early-return branch's embed is likewise gated on
  `force_embed`, so the `:262` path is not an escape either.

`embed_queue_items` (`src/librarian/indexer.rs:70-102`) is the sole production writer of
embed-queue entries, and both call sites are the two gates above. There is no third path.

**The consequence is already documented in the code, as a known property rather than a
defect.** `index_repo_sync`'s doc comment (`src/librarian/indexer.rs:105-114`) ends:

> *"Without it, already-indexed unchanged content never gets embedded, silently,
> forever."*

That sentence is correct and is about `force_embed` being the intended remedy. What it
does not say is that `:302` can *manufacture* the "unchanged" condition on content that
was never embedded — so the escape hatch is described while the hole it must be used on
is not.

**measured 2026-09-02:** `sha256sum -c` over all 733 vectorless codescout artifacts →
`729 : OK` (catalog hash equals disk hash), `4 : FAILED`, `0` absent. The 729 are
trapped; the 4 have since-changed content and will embed on the next ordinary run.
Mechanism read at the bytes the same day at `de434ca5` via `read_file(force=true)` over
`src/librarian/indexer.rs:240-320` and `:676-727`.

## Evidence

### E1 — Vector coverage, codescout project

Two generations of vector storage coexist. The legacy per-artifact `artifact_vec` is
frozen — nothing writes it any more; new work lands in the chunk-grain
`artifact_chunk` / `artifact_vec_v2` pair. So the population that matters is artifacts
carrying **neither**.

```
codescout artifacts in catalog                       1373
  ... with a legacy per-artifact vector                639   (46.5%)
  ... with chunk rows                                    6
  ... with NEITHER — no searchable representation      730   (53.2%)
```

Queries: `LEFT JOIN artifact_vec_rowids r ON r.id = a.id WHERE r.id IS NULL`, and
`NOT EXISTS (SELECT 1 FROM artifact_chunk c WHERE c.artifact_id = a.id)`, both scoped
by `abs_path LIKE '<repo>/%'`.

The system `sqlite3` CLI cannot load `vec0`, so the virtual tables are counted through
their plain shadow tables `artifact_vec_rowids` / `artifact_vec_v2_rowids`.
### E2 — Per-member trap check across the whole vectorless population

```
no_vec_AND_no_chunks: 730
  of which sha MATCHES disk (trapped):      729
  of which sha DIFFERS (will index):          1
  of which file absent from disk:             0
```

This is a **per-member** result, not an aggregate: each row was hashed individually
against its own catalog value. The single exception is the control — it shows the check
can come out the other way, so `729` is a measurement rather than a tautology of the
query's construction.

An earlier run of the same check, before this session's reindex, over the then-733
vectorless rows gave `729 : OK`, `4 : FAILED`, `0` absent. The trapped figure is stable
across both; the four that were going to index did.
### E3 — Three named artifacts in the trap state

```
753e5284...ed623  .../docs/trackers/observer-blindness.md    (catalog)
753e5284...ed623  docs/trackers/observer-blindness.md        (disk)
72b75d52...28b3d  .../docs/trackers/issue-clusters.md        (catalog)
72b75d52...28b3d  docs/trackers/issue-clusters.md            (disk)
b9569e4b...d470b  .../docs/PROBES.md                         (catalog)
b9569e4b...d470b  docs/PROBES.md                             (disk)
```

All three are large, actively-edited trackers with substantial bodies, all committed
today, none with a vector. Their size rules out the deliberate empty-body skip
(`index_repo_sync_skips_empty_body_from_embed_queue`, `src/librarian/indexer.rs:1383`)
as the explanation for them.

### E4 — Missing-vector breakdown by kind

```
bug 519 | tracker 72 | plan 54 | spec 42 | doc 19 | unknown 8 | memory 7 | adr 7 | convention 4 | task 1
```

The hole is not concentrated in one classifier bucket, which is what a
classification-rule bug would look like.


### E5 — `artifact_chunk`'s emptiness was NOT evidence of this bug, and a follow-up proved it

At first measurement `artifact_chunk` held **0 rows across all 4540 artifacts on this
machine**, including projects untouched by any of this. It was tempting to cite as a
symptom. It was instead recorded as most likely a newly-migrated table awaiting its
first embedder-backed reindex.

One ordinary `librarian(action="reindex", scope="project")` later:

```
artifact_chunk_rows:          683   (was 0)
artifact_vec_v2_rows:           0
cs_artifacts_with_chunks:       6   (== the 1 added + 5 updated)
```

The caveat was correct: the table was empty because it was new, not because of this
defect. **Recorded here so the emptiness is never retro-cited as a symptom** — a bug
file that had claimed it would now contain an assertion its own follow-up refutes.

Two residual observations, neither investigated, neither part of this bug's claim:
`artifact_vec_v2` stays at 0 while `artifact_chunk` fills (plausibly because a
configured `ArtifactVectorStore` diverts writes away from the sqlite-vec path — see the
`store: Option<&dyn ArtifactVectorStore>` param on `index_repo`), and the reindex
reported `embedded: 678` against 683 chunk rows written, a difference of 5 that may or
may not be the empty-chunk filter from `de434ca5`.

### E6 — Prediction test: an ordinary reindex released none of the trapped set

The static read of the gate predicts that no automatic path can embed a trapped
artifact. That prediction was then tested rather than assumed. Running a full project
reindex with no force levers:

```
added: 1, updated: 5, unchanged: 1364, embedded: 678, embeddings_enabled: true,
embed_error_count: 0, backfill_error_count: 0
```

Before and after, `cs_with_legacy_vec` reads **639 — identical**. The 729 did not move.

**The control is what makes this evidence rather than a coincidence.** A reindex that
silently no-op'd, errored, or found no embedder would produce the same unchanged 639.
This one demonstrably ran and demonstrably worked: it added a row, updated five, wrote
683 chunk rows and embedded 678, with `embeddings_enabled: true` and zero embed errors.
It worked, and it skipped the 729 — which is exactly and only what the mechanism
predicts.
## Hypotheses tried

1. **Hypothesis:** the hole is empty-body artifacts, which `index_repo_sync`
   deliberately skips.
   **Test:** inspect the largest, most recently edited vectorless files (E3).
   **Verdict:** rejected — `observer-blindness.md`, `issue-clusters.md` and `PROBES.md`
   are among the repo's largest markdown files.
   **Evidence:** E3.

2. **Hypothesis:** no embedder is configured, so nothing embeds and the split is an
   artifact of the environment.
   **Test:** find the most recently updated codescout artifacts that **do** have a
   vector.
   **Verdict:** rejected — `codescout-usage-hookify.md` was vectored at 11:53:26 today,
   20 seconds before `observer-blindness.md` was updated without one. Embedding runs;
   the population divides inside a single reindex.
   **Evidence:** Symptom section.

3. **Hypothesis:** the vectorless artifacts are simply stale and will be picked up on
   the next ordinary reindex.
   **Test:** compare each vectorless artifact's catalog `file_sha256` against its file
   on disk.
   **Verdict:** rejected for 729 of 733 — their hashes already match, so
   `content_unchanged` is `true` and `:309` will decline on every future run. Confirmed
   for exactly 4.
   **Evidence:** E2.

4. **Hypothesis:** `artifact_chunk` being empty is a further symptom of this defect.
   **Test:** count `artifact_chunk` machine-wide rather than project-scoped.
   **Verdict:** rejected — it is empty for all 4540 artifacts across every project,
   consistent with a newly-migrated table awaiting its first embedder-backed reindex.
   **Evidence:** E5.

5. **Hypothesis:** the specific run that trapped each artifact can be identified from
   the catalog.
   **Test:** look for a per-artifact embed-attempt record in the schema.
   **Verdict:** deferred — none of `artifact`, `artifact_vec_rowids` or `events` records
   an embed attempt, only its outcome. This is why the frontmatter carries a non-empty
   `unverified:`: the trap is proven, the door each artifact came through is not.

## Fix

**Plan — not yet implemented.** The defect is an ordering-and-observability problem, so
the candidate fixes differ in what they make *observable*, not only in what they repair:

- **(a) Reorder** — do not commit `file_sha256` until the embed for that artifact has
  been queued *and* accepted. Makes the stamp mean what `:309` reads it to mean. Costs a
  second write or a deferred stamp, and needs care so a failed embed does not livelock
  the row into re-processing every run.
- **(b) Separate the stamp from the claim** — add an `embedded_sha256` column distinct
  from `file_sha256`, and gate `:309` on *that*. The gate then reads a stamp written by
  the code that actually embeds. Larger change; removes the proxy entirely rather than
  tightening it.
- **(c) Make the hole reportable** — regardless of (a)/(b), `IndexReport` should carry a
  count of artifacts that are `want_embeddings && !has_vector`, so the condition stops
  being silent. Under the § *Observer Blindness* rule this is the part that must ship:
  a repair without it leaves the next occurrence just as unobservable.

**Backfill is required either way** — (a) and (b) both prevent new entries into the trap
and neither releases the 729 already in it. `librarian(action="reindex", reembed=true)`
is the existing lever (it sets `force_embed`), and it is the documented remedy in the
doc comment at `:105-114`.

SHA and patch-id to be recorded here at fix time.

## Tests added

None yet — bug is `open`, no fix written. The regression test this needs is a
two-run test asserting the *sequence*, not either run's outcome:
index once with `want_embeddings=false`, then once with `want_embeddings=true` and both
force levers `false`, and assert the embed queue is non-empty on the second run.

Note that the existing pair `index_repo_sync_force_embed_requeues_unchanged_content`
(`src/librarian/indexer.rs:1283`) and
`index_repo_sync_force_embed_alone_requeues_without_force_rewalk` (`:1327`) both pass
`force_embed=true` and so exercise only the escape hatch. **They are monotone under this
defect** — the trap state is exactly the state in which `force_embed` still works — so a
green suite here is not evidence about the un-forced path. That path currently has no
test.

## Workarounds

`librarian(action="reindex", reembed=true)` on the affected project. `reembed=true`
sets `force_embed`, which is the one condition at both `:276` and `:309` that does not
consult the stamp. Until it is run, semantic search over this repo silently omits 53%
of the corpus — a `semantic_search` result set here is a sample, not a search.

## Resume

Decide between fix (a) and fix (b) before writing code — they differ in whether
`file_sha256` keeps its current meaning, and Task 11 of the chunk-grain retrieval plan
backfills into whichever shape wins. Then, in order:

1. Write the failing two-run test described under **Tests added** and observe it RED
   before touching `src/librarian/indexer.rs`. The existing `force_embed` tests will stay
   green throughout and must not be read as coverage.
2. Implement (c) — the `IndexReport` count — in the same change, so the condition is
   observable whichever of (a)/(b) ships.
3. Re-run the E2 query after the backfill; the expected post-fix reading is
   `sha MATCHES disk (permanently trapped): 0`.

**Task 11 of the chunk-grain plan is blocked on this** — it backfills chunk rows from
artifacts the indexer declines to process, so running it first would backfill into the
hole and report success.

## References

- `src/librarian/indexer.rs` — `index_repo_sync:115-388` (gates at `:244`, `:262`,
  `:276`, `:302`, `:309`), `index_repo:676-727` (levers hardcoded at `:689`),
  `embed_queue_items:70-102`.
- [`docs/trackers/issue-clusters.md`](../trackers/issue-clusters.md) — `IC-2`
  (`cluster/gate-keyed-on-unobservable-event`), artifact `1b5a080fe2efcb6b`. This bug is
  a member: the proxy is the *monotone stamp* named in IC-2's own claim, and it fails
  silently rather than loudly, which is what IC-2's `Falsified by` clause requires.
  **Prediction worth recording:** it is the first *open* member of IC-2's
  filesystem/repo-scoped half, whose other seven are all closed.
- `docs/conventions/cross-machine-catalog-resume.md` — the adjacent failure mode (a
  catalog that never had vectors because it was built elsewhere). Distinct from this one:
  there the rows are absent, here they are present and stamped.
