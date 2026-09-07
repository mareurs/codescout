---
id: '9ea0a90867c85260'
kind: bug
status: open
title: 'BUG: `backfill-chunks` accepts `--project` but walks the whole catalog, across every repo on the host'
owners:
- marius
tags:
- cluster/blast-radius-exceeds-visibility
topic: backfill scope vs project flag
closed: ''
opened: 2026-09-07
owner: marius
related: []
severity: medium
---

# BUG: `backfill-chunks` accepts `--project` but walks the WHOLE catalog, across every repo on the host

## Summary

`codescout backfill-chunks` takes `--project <PROJECT>` ("Optional project root override, defaults
to cwd") and is naturally run from inside a repo. Its walk has no path filter at all, so it
processes every artifact in the catalog — and one catalog serves every project on the host.
Invoked to repair 10 artifacts in one repo, it rewrote 2807 across all of them.

## Symptom (Effect)

Asked to fix the 10 artifacts `librarian(action="reindex")` reported for this project, run from
`/home/marius/work/claude/codescout` with no `--project`:

```json
{"artifacts": 2807, "embedded": 61613, "skipped_empty": 0, "missing_file": 1}
```

10 was the project-scoped figure. 2807 is the catalog. The 280× gap is not visible anywhere
before the run, and the report does not name a scope.

Confirmed by running the two queries side by side:

```
codescout-rooted, no chunk rows : 10   (before)  ->  0   (after)
catalog-wide,     no chunk rows : 2807 (before)  ->  1   (after)
```

## Reproduction

```
git rev-parse HEAD          # 4b30601c at filing
```

1. From any project directory, `codescout backfill-chunks --json`.
2. Compare `artifacts` against the `vectorless` count `librarian(action="reindex")` reports for
   that project.

Observed live 2026-09-07.

## Environment

Linux (`ripper`), `experiments` @ `4b30601c`, shared catalog at
`~/.local/share/librarian/catalog.db` (404 MB, 4710 artifacts), umbrella
`codescout-ecosystem` plus unrelated repos under other roots.

## Root cause

Two facts that only compose into a defect at the interface.

`backfill_chunk_vectors` (`src/librarian/indexer.rs:1079`) has no scope parameter — there is
nothing to pass:

```rust
pub async fn backfill_chunk_vectors(
    catalog: &parking_lot::Mutex<Catalog>,
    svc: &crate::librarian::embedding::EmbeddingService,
    batch: usize,
) -> Result<BackfillReport>
```

and its page query has no `abs_path` predicate:

```sql
SELECT a.id, a.abs_path, a.title, a.updated_at FROM artifact a
WHERE NOT EXISTS (SELECT 1 FROM artifact_chunk c WHERE c.artifact_id = a.id)
  AND (a.updated_at > ?1 OR (a.updated_at = ?1 AND a.id > ?2))
ORDER BY a.updated_at, a.id LIMIT ?3
```

Contrast the sibling counter it is meant to empty, which **is** scoped
(`root_prefix` = the reindex target):

```sql
SELECT COUNT(*) FROM artifact a
WHERE a.abs_path LIKE ?1
  AND NOT EXISTS (SELECT 1 FROM artifact_chunk c WHERE c.artifact_id = a.id)
```

So the count that sends you here is per-project and the repair is per-host, and the CLI's
`--project` flag — which resolves *which catalog and config to open*, not which rows to walk —
reads as though it closes that gap.

measured 2026-09-07: the report above, plus both SQL forms run directly before and after.

## Evidence

### The blast radius is not stated anywhere the caller looks

`backfill-chunks --help` describes the population as *"every artifact with no chunk rows"* and
never says which artifacts are in scope. `--project`'s help is the generic
*"Optional project root override (defaults to cwd)"* shared with other subcommands, where it
genuinely does scope the operation.

### It is not merely cosmetic

The run took ~11 minutes, wrote ~7.4 GB, and issued 61,613 remote embedding round-trips against
a shared embedder. A user repairing ten artifacts has no way to anticipate that from the
interface, and the module's own header (`src/cli/backfill_chunks.rs:12-17`) reasons carefully
about lock contention while saying nothing about scope.

## Hypotheses tried

1. **Hypothesis:** `--project` scopes the walk and I omitted it.
   **Test:** read `backfill_chunk_vectors`' signature and its page query.
   **Verdict:** rejected — no scope parameter exists, and the query has no path predicate.
   Passing `--project` could not have changed the outcome.

## Fix

Either scope it or say it does not scope. Preferred: add an optional root prefix threaded from
`--project` into the page query's `WHERE`, defaulting to the resolved project — matching what
the flag already implies and what the `vectorless` counter it exists to empty already does. Add
`--all` for the deliberate catalog-wide run.

Minimum viable alternative if the whole-catalog walk is wanted by design: say so in `--help` and
carry a `scope` field in `BackfillReport` so the JSON names its own population, per
`docs/adrs/2026-08-27-negative-results-name-their-scope.md`.

- **SHA (experiments):** pending
- **patch-id:** pending

## Tests added

None yet. A test seeding two artifacts under different roots and asserting the scoped run touches
one would red on today's code, and does not depend on prose.

## Workarounds

None at the interface — there is no flag that narrows it. Know that the run is host-wide, and do
not treat a project-scoped `vectorless` count as an estimate of its cost.

## Resume

Add a root-prefix parameter to `backfill_chunk_vectors` (`src/librarian/indexer.rs:1079`) and
thread `args.project`'s resolved root into the page query's `WHERE`, mirroring `root_prefix` in
the `vectorless` query. Then add the two-root test in § *Tests added* and confirm it reds first.

## References

- `src/librarian/indexer.rs:1079` — signature and page query
- `src/cli/backfill_chunks.rs` — the CLI wrapper and its `--project` flag
- `docs/issues/2026-09-07-vectorless-note-prescribes-a-reembed-that-cannot-reach-it.md` — the note that sends you here
- `docs/adrs/2026-08-27-negative-results-name-their-scope.md`

